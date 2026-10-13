#!/usr/bin/env python3
"""Opt-in actual LanguageTool/companion pilot with synthetic text only."""
import argparse,datetime,hashlib,json,os,signal,socket,subprocess,tempfile,time,urllib.request,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];SCRATCH=ROOT.parent.parent/'work';ENDPOINT='http://127.0.0.1:8081'
VALIDATOR="""import {validateSuggestion} from './apps/writer/src/assistance/model.ts';let input='';for await(const chunk of process.stdin)input+=chunk;const data=JSON.parse(input);for(const s of data.suggestions){if(s.origin!=='proofreading'||s.status!=='pending')throw Error('Wrong suggestion origin');validateSuggestion(s,data.sources);}console.log(JSON.stringify({valid:true,count:data.suggestions.length}));"""
class Workspace:
 def __init__(self,directory):
  self.p=subprocess.Popen([str(ROOT/'target/debug/novel-companion'),str(directory)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True);self.sequence=0
 def call(self,op,**args):
  self.sequence+=1;self.p.stdin.write(json.dumps({'id':self.sequence,'op':op,'args':args},ensure_ascii=False)+'\n');self.p.stdin.flush();r=json.loads(self.p.stdout.readline())
  if not r.get('ok'):raise RuntimeError(r.get('error','Companion request failed'))
  return r['value']
 def close(self):
  self.p.stdin.close()
  try:self.p.wait(timeout=10)
  except subprocess.TimeoutExpired:self.p.terminate();self.p.wait(timeout=10)
  self.p.stdout.close()
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--run',action='store_true');a=parser.parse_args()
 if not a.run:parser.error('--run is required; nothing starts on import')
 SCRATCH.mkdir(exist_ok=True)
 with socket.socket() as available:available.bind(('127.0.0.1',8081))
 stamp=datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ');report={'version':'6.6','results':[]};server=None
 try:
  with (SCRATCH/f'proofreading-{stamp}.log').open('wb') as log,tempfile.TemporaryDirectory(prefix='proofreading-fixture-',dir=SCRATCH) as directory:
   server=subprocess.Popen(['/usr/bin/python3',str(ROOT/'tools/local-proofreading.py'),'--serve'],cwd=ROOT,stdout=log,stderr=log,start_new_session=True);start=time.monotonic()
   for _ in range(120):
    if server.poll() is not None:raise RuntimeError('Own local server exited; inspect synthetic pilot log')
    try:
     with urllib.request.urlopen(ENDPOINT+'/v2/languages',timeout=1) as reply:report['installedLanguages']=json.load(reply)
     break
    except Exception:time.sleep(.25)
   else:raise TimeoutError('Local server readiness timeout')
   report['loadSeconds']=round(time.monotonic()-start,3)
   bindings=[]
   for table in ['tcp','tcp6']:
    for row in Path(f'/proc/{server.pid}/net/{table}').read_text().splitlines()[1:]:
     columns=row.split()
     if columns[3]=='0A' and int(columns[1].split(':')[1],16)==8081:bindings.append(columns[1])
   if not bindings or any(value.split(':')[0] not in ['0100007F','00000000000000000000000001000000','0000000000000000FFFF00000100007F'] for value in bindings):raise RuntimeError('Server is not bound exclusively to loopback')
   report['loopbackBindings']=bindings;workspace=Workspace(directory)
   try:
    report['status']=workspace.call('provider_status',kind='languagetool',endpoint=ENDPOINT)
    p=workspace.call('create_project',title='Synthetic proofreading');ch=workspace.call('create_chapter',projectId=p['id'],title='Synthetic chapter')
    fixtures=[('en','She go to school every day.',True),('en','She goes to school every day.',False),('ar','ذهبت الفتاة إلى المدرسه.',True),('ar','ذهبت الفتاة إلى المدرسة.',False),('zh-Hans','她每天去学校。',False),('zh-Hant','她每天去學校。',False)]
    for lang,text,seed in fixtures:
     scene={'schemaVersion':1,'title':'Synthetic '+lang,'document':{'type':'doc','content':[{'type':'paragraph','attrs':{'lang':lang,'dir':'rtl' if lang=='ar' else 'auto'},'content':[{'type':'text','text':text}]}]}}
     saved=workspace.call('create_scene',chapterId=ch['id'],scene=scene);source={'sceneId':saved['id'],'revisionId':saved['revisionId'],'hash':hashlib.sha256(text.encode()).hexdigest(),'text':text,'language':lang};identity='proofreading-'+uuid.uuid4().hex
     request={'schemaVersion':1,'id':identity,'idempotencyKey':identity,'projectId':p['id'],'provider':'languagetool','operation':'grammar','language':lang,'sources':[source],'contextBudget':4096,'timeoutMs':120000,'createdAt':datetime.datetime.now(datetime.timezone.utc).isoformat()};result={'language':lang,'text':text,'seeded':seed};start=time.monotonic()
     try:
      result['response']=workspace.call('provider_run',endpoint=ENDPOINT,runToken=identity+':1',request=request)
      checked=subprocess.run(['node','--experimental-strip-types','--input-type=module','-e',VALIDATOR],cwd=ROOT,input=json.dumps({'suggestions':result['response']['suggestions'],'sources':[source]}),text=True,capture_output=True,timeout=10,check=True);result['validation']=json.loads(checked.stdout)
     except Exception as error:result['error']=str(error)
     after=workspace.call('load_scene',sceneId=saved['id']);result['manuscriptUnchanged']=after['scene']==scene and after['revisionId']==saved['revisionId'];result['seconds']=round(time.monotonic()-start,3);report['results'].append(result)
   finally:workspace.close()
 finally:
  if server is not None and server.poll() is None:
   os.killpg(server.pid,signal.SIGTERM)
   try:server.wait(timeout=15)
   except subprocess.TimeoutExpired:os.killpg(server.pid,signal.SIGKILL);server.wait()
  report['ownServerStopped']=server is None or server.poll() is not None
  path=SCRATCH/f'proofreading-{stamp}.json';path.write_text(json.dumps(report,ensure_ascii=False,indent=2));print(json.dumps({'report':str(path),'status':report.get('status'),'loadSeconds':report.get('loadSeconds'),'results':[{k:r.get(k) for k in ['language','seeded','validation','error','seconds','manuscriptUnchanged']} for r in report['results']],'ownServerStopped':report['ownServerStopped']},ensure_ascii=False,indent=2))
if __name__=='__main__':main()
