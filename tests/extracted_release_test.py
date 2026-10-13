"""Exercise the exact RPM's production UI/IPC with external WebKit instrumentation.

Requires installed C compiler and WebKit headers. No probe is added to production
code. All XDG writing/config/cache paths and the seeded database are temporary.
This tests synthetic DOM input, not a physical keyboard or IME. --installed
verifies the installed payload and uses its binaries with temporary writing data.
"""
import argparse,hashlib,json,os,shlex,signal,sqlite3,subprocess,tempfile,time,shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def run(rpm,report_path,offline=False,installed=False,scenes=1,screenshot=None,probe=None,prefix=None):
 if installed and prefix:raise ValueError('Choose system installation or user prefix.')
 rpm=rpm.resolve()
 report={'rpmSha256':hashlib.sha256(rpm.read_bytes()).hexdigest(),'checks':[]}
 with tempfile.TemporaryDirectory(prefix='novel-extracted-release-') as directory:
  folder=Path(directory);extracted=folder/'extracted';extracted.mkdir()
  producer=subprocess.Popen(['rpm2cpio',str(rpm)],stdout=subprocess.PIPE)
  extraction=subprocess.run(['cpio','-idmu','--quiet'],stdin=producer.stdout,cwd=extracted,capture_output=True)
  producer.stdout.close();assert producer.wait()==0 and extraction.returncode==0
  executable_root=prefix.resolve() if prefix else Path('/') if installed else extracted
  if installed or prefix:
   for payload in (extracted/'usr').rglob('*'):
    if payload.is_file():
     installed_file=executable_root/payload.relative_to(extracted)
     assert installed_file.is_file() and installed_file.read_bytes()==payload.read_bytes(),f'Installed payload differs: {installed_file}'
   report['checks'].append(('User-folder' if prefix else 'Installed')+' application payload matches the verified RPM')
  library=folder/'release-test.so'
  flags=shlex.split(subprocess.check_output(['pkg-config','--cflags','--libs','webkit2gtk-4.1'],text=True))
  subprocess.run(['cc','-shared','-fPIC',str(ROOT/'tests/release_webkit_hook.c'),'-o',str(library),*flags,'-ldl'],check=True,capture_output=True)
  data=folder/'data';cache=folder/'cache';config=folder/'config'
  for path in [data,cache,config]:path.mkdir(mode=0o700)
  store=data/'local.novel.workspace.spike';store.mkdir(mode=0o700)
  seed={'schemaVersion':1,'title':'Synthetic Arabic 中文','document':{'type':'doc','content':[{'type':'paragraph','attrs':{'dir':'rtl','lang':'ar'},'content':[{'type':'text','text':'سلام مِمِمِ 中文 繁體 👩🏽‍🚀'}]},{'type':'paragraph','attrs':{'dir':'ltr','lang':'en'},'content':[{'type':'text','text':'Second synthetic paragraph'}]}]}}
  child=subprocess.Popen([str(executable_root/'usr/bin/novel-companion'),str(store)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  def rpc(op,args):
   child.stdin.write(json.dumps({'id':1,'op':op,'args':args},ensure_ascii=False)+'\n');child.stdin.flush()
   response=json.loads(child.stdout.readline());assert response['ok'],response
   return response['value']
  try:
   project=rpc('create_project',{'title':'Synthetic extracted release'})
   chapter=rpc('create_chapter',{'projectId':project['id'],'title':'Synthetic chapter'})
   saved=rpc('create_scene',{'chapterId':chapter['id'],'scene':seed})
   for index in range(1,scenes):
    rpc('create_scene',{'chapterId':chapter['id'],'scene':{'schemaVersion':1,'title':f'Synthetic scene {index}','document':{'type':'doc','content':[{'type':'paragraph','content':[{'type':'text','text':'Synthetic long project English Arabic مِ Chinese 中文. '*30}]}]}}})
  finally:
   child.stdin.close();assert child.wait(timeout=10)==0;child.stdout.close();child.stderr.close()
  result_path=folder/'result.json'
  environment={key:value for key,value in os.environ.items() if not key.startswith('NOVEL_') and key!='LD_PRELOAD'}
  script=(probe or ROOT/'tests/release_workspace_probe.js').resolve()
  assert script.is_file(),'External test probe is missing'
  progress_path=folder/'progress.json'
  environment.update(XDG_DATA_HOME=str(data),XDG_CACHE_HOME=str(cache),XDG_CONFIG_HOME=str(config),LD_PRELOAD=str(library),NOVEL_RELEASE_TEST_SCRIPT=str(script),NOVEL_RELEASE_TEST_RESULT=str(result_path),NOVEL_RELEASE_TEST_PROGRESS=str(progress_path))
  if screenshot:environment['NOVEL_RELEASE_TEST_SCREENSHOT']=str(folder/'screenshot.png')
  command=[str(executable_root/'usr/bin/novel-workspace')]
  launch_started=time.monotonic()
  if offline:command=['unshare','--user','--map-root-user','--net',*command]
  app=subprocess.Popen(command,cwd=extracted,env=environment,start_new_session=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  try:out,err=app.communicate(timeout=90)
  except subprocess.TimeoutExpired:
   os.killpg(app.pid,signal.SIGKILL);out,err=app.communicate()
   partial=result_path.read_text() if result_path.exists() else progress_path.read_text() if progress_path.exists() else 'No external UI result'
   raise AssertionError('Owned release fixture timed out: '+err.decode(errors='replace')[-1000:]+'\n'+partial[-6000:])
  assert app.returncode==0,err.decode(errors='replace')[-1000:]
  assert result_path.exists(),'External release hook did not return a report'
  ui=json.loads(result_path.read_text());assert 'error' not in ui,ui
  with sqlite3.connect(f'file:{store/"editor-spike.sqlite3"}?mode=ro',uri=True) as database:
   assert database.execute('PRAGMA integrity_check').fetchone()==('ok',)
   assert database.execute('PRAGMA foreign_key_check').fetchall()==[]
   row=database.execute('SELECT r.id,r.payload FROM manuscript_scene s JOIN revision_envelope r ON s.head=r.id WHERE s.id=?',(saved['id'],)).fetchone()
   assert row[0]==ui['savedRevision']
  initial_checks=report['checks']
  report.update(ui);report['checks']=initial_checks+report['checks'];report['sceneCount']=scenes;report['launchAndWorkflowSeconds']=time.monotonic()-launch_started;report['installed']=installed;report['checks'].append('Acknowledged revision and database integrity survive release exit')
  if screenshot:
   assert (folder/'screenshot.png').is_file(),'Native WebKit snapshot was not returned'
   screenshot.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(folder/'screenshot.png',screenshot)
  report['networkNamespaceDisabled']=offline
  report['installationKind']='user-folder' if prefix else 'system-rpm' if installed else 'extracted'
  report['limitations']=['External hook changes process instrumentation; timings are not an uninstrumented performance benchmark.','Synthetic DOM input is not physical keyboard/IME qualification.','Isolated fixture does not open personal writing, and does not test package updates.' if installed or prefix else 'RPM extracted independently of checkout; not system installation/update.']
 report_path.parent.mkdir(parents=True,exist_ok=True);report_path.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--rpm',type=Path,default=ROOT/'target/release/bundle/rpm/Novel Workspace-0.1.0-1.x86_64.rpm');parser.add_argument('--report',type=Path,default=ROOT/'work/extracted-release.json');parser.add_argument('--offline',action='store_true',help='Run only the owned app in an unprivileged network namespace');placement=parser.add_mutually_exclusive_group();placement.add_argument('--installed',action='store_true');placement.add_argument('--prefix',type=Path,help='Verify and run a user-folder RPM payload');parser.add_argument('--scenes',type=int,choices=[1,2,1000],default=1);parser.add_argument('--screenshot',type=Path);parser.add_argument('--probe',type=Path,help='External test-only WebKit probe');args=parser.parse_args();run(args.rpm,args.report,args.offline,args.installed,args.scenes,args.screenshot,args.probe,args.prefix)
