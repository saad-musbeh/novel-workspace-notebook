"""Real bundled WebViews and native sessions, instrumented only in owned temp stores."""
import argparse, hashlib, json, os, shlex, shutil, signal, sqlite3, subprocess, tempfile, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

# Test-only native GTK close; marker is private to the owned process tree and
# contains the main PID. No application IPC permission or product code changes.
GTK_CLOSE_HOOK = r"""
#define _GNU_SOURCE
#include <dlfcn.h>
#include <gtk/gtk.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
static GWeakRef original_window;
static gboolean captured_original;
static gboolean request_original_close(gpointer unused) {
    (void)unused;
    const char *marker = getenv("NOVEL_ORIGINAL_CLOSE_REQUEST");
    const char *ack = getenv("NOVEL_ORIGINAL_CLOSE_ACK");
    if (!marker || !ack) return G_SOURCE_REMOVE;
    FILE *input = fopen(marker, "r");
    if (!input) return G_SOURCE_CONTINUE;
    long expected = 0; int valid = fscanf(input, "%ld", &expected); fclose(input);
    if (valid != 1 || expected != (long)getpid()) return G_SOURCE_CONTINUE;
    GObject *window = g_weak_ref_get(&original_window);
    if (!window) return G_SOURCE_REMOVE;
    gtk_window_close(GTK_WINDOW(window));
    g_object_unref(window);
    FILE *output = fopen(ack, "w");
    if (output) {fprintf(output, "%ld\n", (long)getpid()); fclose(output);}
    return G_SOURCE_REMOVE;
}
void gtk_window_set_title(GtkWindow *window, const gchar *title) {
    void (*original)(GtkWindow *, const gchar *) = dlsym(RTLD_NEXT, "gtk_window_set_title");
    original(window, title);
    /* Tao creates GtkApplicationWindow through GObject, then explicitly sets
     * its title. gtk_window_new itself would miss this construction path. */
    if (GTK_IS_APPLICATION_WINDOW(window) && !captured_original && getenv("NOVEL_ORIGINAL_CLOSE_REQUEST")) {
        captured_original = TRUE;
        g_weak_ref_init(&original_window, G_OBJECT(window));
        g_timeout_add(50, request_original_close, NULL);
    }
}

"""

def run(binary, rpm, report_path, close_original=False, probe=None):
 with tempfile.TemporaryDirectory(prefix='novel-multi-window-') as temporary:
  folder=Path(temporary)
  if rpm:
   extracted=folder/'rpm';extracted.mkdir()
   producer=subprocess.Popen(['rpm2cpio',str(rpm.resolve())],stdout=subprocess.PIPE)
   extract=subprocess.run(['cpio','-idmu','--quiet'],stdin=producer.stdout,cwd=extracted,capture_output=True)
   producer.stdout.close();assert producer.wait()==0 and extract.returncode==0
   binary=extracted/'usr/bin/novel-workspace';companion=extracted/'usr/bin/novel-companion'
  else:
   binary=binary.resolve();companion=binary.with_name('novel-companion')
  library=folder/'multi-window-hook.so'
  flags=shlex.split(subprocess.check_output(['pkg-config','--cflags','--libs','webkit2gtk-4.1'],text=True))
  hook_sources=[str(ROOT/'tests/multi_window_webkit_hook.c')]
  if close_original:
   close_hook=folder/'close-original-hook.c';close_hook.write_text(GTK_CLOSE_HOOK);hook_sources.append(str(close_hook))
  subprocess.run(['cc','-shared','-fPIC',*hook_sources,'-o',str(library),*flags,'-ldl'],check=True,capture_output=True)
  for name in ['data','cache','config']:(folder/name).mkdir(mode=0o700)
  store=folder/'data/local.novel.workspace.spike';store.mkdir(mode=0o700)
  child=subprocess.Popen([str(companion),str(store)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  def rpc(op,args):
   child.stdin.write(json.dumps({'id':1,'op':op,'args':args},ensure_ascii=False)+'\n');child.stdin.flush()
   response=json.loads(child.stdout.readline());assert response['ok'],response
   return response['value']
  try:
   project=rpc('create_project',{'title':'Synthetic multi-window project'})
   chapter=rpc('create_chapter',{'projectId':project['id'],'title':'Synthetic chapter'})
   first=rpc('create_scene',{'chapterId':chapter['id'],'scene':{'schemaVersion':1,'title':'Long Arabic 中文','document':{'type':'doc','content':[{'type':'paragraph','attrs':{'lang':'ar','dir':'rtl'},'content':[{'type':'text','text':'Long English مِمِ Chinese 中文. '*1200}]}]}}})
   second=rpc('create_scene',{'chapterId':chapter['id'],'scene':{'schemaVersion':1,'title':'Independent second scene','document':{'type':'doc','content':[{'type':'paragraph','attrs':{'lang':'en','dir':'ltr'},'content':[{'type':'text','text':'Second independent مِ 中文'}]}]}}})
  finally:
   child.stdin.close();assert child.wait(timeout=10)==0;child.stdout.close();child.stderr.close()
  result_base=folder/'result.json';destroy_log=folder/'window-destroy.log'
  environment={key:value for key,value in os.environ.items() if not key.startswith('NOVEL_') and key!='LD_PRELOAD'}
  script=probe.resolve() if probe else ROOT/'tests/multi_window_probe.js'
  if probe:environment['NOVEL_PROBE_DIAGNOSTICS']='1'
  close_request=folder/'original-close.request';close_ack=folder/'original-close.ack'
  if close_original:
   script=folder/'close-original-probe.js';script.write_text('globalThis.__novelCloseOriginalFixture=true;\n'+(ROOT/'tests/multi_window_probe.js').read_text())
   environment.update(NOVEL_ORIGINAL_CLOSE_REQUEST=str(close_request),NOVEL_ORIGINAL_CLOSE_ACK=str(close_ack))
  environment.update(XDG_DATA_HOME=str(folder/'data'),XDG_CACHE_HOME=str(folder/'cache'),XDG_CONFIG_HOME=str(folder/'config'),LD_PRELOAD=str(library),NOVEL_RELEASE_TEST_SCRIPT=str(script),NOVEL_RELEASE_TEST_RESULT=str(result_base),NOVEL_WINDOW_DESTROY_LOG=str(destroy_log))
  command=['unshare','--user','--map-root-user','--net',str(binary)]
  process=subprocess.Popen(command,cwd=ROOT,env=environment,start_new_session=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  started=time.monotonic();max_companions=0;max_editors=0;peak_rss=0;post_close_companions=[];child_destroy_observed_alive=False
  try:
   while True:
    companions=editors=rss=0
    for path in Path('/proc').iterdir():
     if not path.name.isdecimal():continue
     try:
      if os.getpgid(int(path.name))!=process.pid:continue
      arguments=(path/'cmdline').read_bytes().split(b'\0')
      status=(path/'status').read_text()
      rss+=next((int(line.split()[1]) for line in status.splitlines() if line.startswith('VmRSS:')),0)
      if arguments and Path(os.fsdecode(arguments[0])).name=='novel-companion':companions+=1
      if any(b'native_editor_engine.py' in arg for arg in arguments):editors+=1
     except (FileNotFoundError,ProcessLookupError,PermissionError):continue
    max_companions=max(max_companions,companions);max_editors=max(max_editors,editors);peak_rss=max(peak_rss,rss)
    if destroy_log.exists() and process.poll() is None:
     child_destroy_observed_alive=child_destroy_observed_alive or any(line.split()[0]=='1' for line in destroy_log.read_text().splitlines())
    if close_original:
     for result_file in folder.glob('result.json.*'):
      try:partial=json.loads(result_file.read_text())
      except json.JSONDecodeError:continue
      assert 'error' not in partial,partial
      if partial.get('windowLabel')=='main' and partial.get('originalCloseReady') and not close_request.exists():
       close_request.write_text(str(process.pid)+'\n')
      if partial.get('originalWindowClosed') and process.poll() is None:post_close_companions.append(companions)

    try:
     out,err=process.communicate(timeout=.2);break
    except subprocess.TimeoutExpired:
     if time.monotonic()-started>100:raise AssertionError('Owned multi-window fixture timed out')
   assert process.returncode==0,err.decode(errors='replace')[-2000:]
   result_files=list(folder.glob('result.json.*'))
   assert {path.name for path in result_files}=={'result.json.0','result.json.1'},('Expected two reported WebViews during refusal/handoff', {path.name:path.read_text() for path in result_files})
   results=[json.loads(path.read_text()) for path in result_files]
   assert len(results)==2,results
   assert all('error' not in result for result in results),results
   main=next(result for result in results if result['windowLabel']=='main')
   target=next(result for result in results if result['windowLabel']!='main')
   if probe:
    assert main.get('mode')==target.get('mode')=='book-transfer',results
    assert main['bookId']==target['bookId'] and target['returnCommitted'],results
    assert main['beforeMoveRevision']==target['incomingRevision'],'Moving Book alone created a revision'
    assert main['sourceText']==target['incomingText'],'Book handoff changed exact Unicode text'
    for position in (target['incomingPosition'],main['returnPosition']):
     assert position['character']==main['sourcePosition']['character'] and position['scene']==first['id'],position
     assert abs(position['offset']-main['sourcePosition']['offset'])<4,(position,main['sourcePosition'])
    assert max_companions==1,max_companions
    assert max_editors>=2,max_editors
    with sqlite3.connect(f'file:{store/"editor-spike.sqlite3"}?mode=ro',uri=True) as database:
     assert database.execute('PRAGMA integrity_check').fetchone()==('ok',)
     assert not database.execute('PRAGMA foreign_key_check').fetchall()
     assert database.execute('SELECT head FROM manuscript_scene WHERE id=?',(first['id'],)).fetchone()[0]==main['savedRevision']
    report={'mode':'book-transfer','checks':main['checks']+target['checks']+['Exact Book filter and logical scene offset survive detach and return','One companion and independent native child sessions','Guarded quit preserves acknowledged scene head and SQLite integrity'],'windowCount':len(results),'maxCompanions':max_companions,'maxEditorProcesses':max_editors,'peakOwnedRssKiB':peak_rss,'workflowSeconds':time.monotonic()-started,'networkNamespaceDisabled':True,'limitations':['Synthetic events do not qualify physical keyboard, IME, drag or compositor behavior.'],'probe':str(probe.resolve())}
    if rpm:report['rpmSha256']=hashlib.sha256(rpm.read_bytes()).hexdigest()
    report_path.parent.mkdir(parents=True,exist_ok=True);report_path.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
    return
   assert main['beforeMoveRevision']==target['incomingRevision'],'Moving alone created a revision'
   assert main['sourceFrame']==target['incomingFrame'],('Caret or selection changed during handoff',main['sourceFrame'],target['incomingFrame'])
   assert main['sourceUndoHash']==target['incomingUndoHash'],'Source undo history changed during handoff'
   if not close_original:assert target['childUndoHash']==main['returnUndoHash'],'Child undo history changed when returning'
   assert abs(main['sourceScrollTop']-target['incomingScrollTop'])<2,(main['sourceScrollTop'],target['incomingScrollTop'])
   assert max_companions==1,max_companions
   assert max_editors>=(1 if close_original else 2),max_editors
   if close_original:
    assert close_ack.exists() and close_ack.read_text().strip()==str(process.pid),'Original native GTK close was not requested by the fixture'
    assert target.get('originalWindowClosed'),'Child did not observe original-window destruction'
    assert post_close_companions and post_close_companions[0]==1,post_close_companions
   with sqlite3.connect(f'file:{store/"editor-spike.sqlite3"}?mode=ro',uri=True) as database:
    assert database.execute('PRAGMA integrity_check').fetchone()==('ok',)
    assert not database.execute('PRAGMA foreign_key_check').fetchall()
    row=database.execute('SELECT s.head FROM manuscript_scene s WHERE s.id=?',(first['id'],)).fetchone()
    assert row[0]==(target if close_original else main)['savedRevision'],'Exit lost the acknowledged head'
   report={'checks':main['checks']+target['checks']+['Handoff preserves exact caret, selection, scroll and revision','One companion owns the temporary store across windows','App quit drains every window and preserves SQLite integrity'], 'windowCount':len(results),'maxCompanions':max_companions,'maxEditorProcesses':max_editors,'peakOwnedRssKiB':peak_rss,'workflowSeconds':time.monotonic()-started,'networkNamespaceDisabled':True,'limitations':['External instrumentation is not a typing or frame latency benchmark.','Synthetic events do not qualify physical keyboard, IME or compositor resize.']}
   report['mode']='close-original' if close_original else 'return-transfer'
   if close_original:
    report['observedCompanionsAfterOriginalClose']=post_close_companions
    report['checks'].append('Real original GTK window close leaves detached editing and acknowledged saving usable')
    report['limitations'].append('Original close is initiated by a scoped external GTK main-loop helper rather than a physical titlebar click; companion counts are sampled.')
   if not close_original:
    assert main.get('autoCloseSourceWindow')==target['windowLabel'],'Last-tab move was not exercised'
    assert destroy_log.exists() and any(line.split()[0]=='1' for line in destroy_log.read_text().splitlines()),'Empty source native window did not close'
    assert child_destroy_observed_alive,'Source destruction was not observed while the destination app remained running'
    report['checks'].append('Acknowledged last-tab move destroys empty source native window while destination remains alive')
    report['emptySourceNativeCloseObserved']=True
   if rpm:report['rpmSha256']=hashlib.sha256(rpm.read_bytes()).hexdigest()
   report_path.parent.mkdir(parents=True,exist_ok=True);report_path.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
  except BaseException:
   if process.poll() is None:os.killpg(process.pid,signal.SIGKILL)
   out,err=process.communicate()
   print('Owned reports:',[json.loads(path.read_text()) for path in folder.glob('result.json.*')])
   print(err.decode(errors='replace')[-2000:]);raise

if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/novel-workspace')
 parser.add_argument('--rpm',type=Path)
 parser.add_argument('--report',type=Path,required=True)
 parser.add_argument('--probe',type=Path,help='Owned Book handoff probe; default transfer fixture remains unchanged')
 parser.add_argument('--close-original',action='store_true',help='Close the original native window, then save in the detached window')
 args=parser.parse_args()
 if args.probe and args.close_original:parser.error('--probe cannot be combined with --close-original')
 run(args.binary,args.rpm,args.report,args.close_original,args.probe)
