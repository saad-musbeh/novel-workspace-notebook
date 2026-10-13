"""Isolated real stdio companion tests; no personal database or service."""
import hashlib,json,os,subprocess,tempfile,time,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
TEXT='مِمِ 中文 👩🏽‍🚀 \ue000'
def scene(text=TEXT):
 return {'schemaVersion':1,'title':'Synthetic','document':{'type':'doc','content':[{'type':'paragraph','attrs':{'dir':'rtl','lang':'ar'},'content':[{'type':'text','text':text,'marks':[{'type':'bold'}]}]}]}}
class Client:
 def __init__(self,directory):
  self.p=subprocess.Popen([str(ROOT/'target/debug/novel-companion'),str(directory)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  self.next=0
 def send(self,op,**args):
  self.next+=1; self.p.stdin.write(json.dumps({'id':self.next,'op':op,'args':args},ensure_ascii=False)+'\n');self.p.stdin.flush();return self.next
 def read(self):
  line=self.p.stdout.readline()
  if not line:raise RuntimeError('Companion exited: '+self.p.stderr.read()[-1000:])
  return json.loads(line)
 def call(self,op,**args):
  ident=self.send(op,**args);r=self.read();assert r['id']==ident,r
  if not r['ok']:raise RuntimeError(r['error'])
  return r['value']
 def close(self):
  self.p.stdin.close();code=self.p.wait(timeout=10);self.p.stdout.close();self.p.stderr.close();assert code==0,'Companion shutdown failed'
class CompanionIntegration(unittest.TestCase):
 def test_optional_recovery_and_sync_metadata_failures_leave_writing_available(self):
  with tempfile.TemporaryDirectory(prefix='novel-optional-services-') as directory:
   root=Path(directory)
   (root/'recovery').write_text('Synthetic blocked backup directory')
   for name in ['sync-auth.json','peer-clients.json']:
    path=root/name;path.write_text('{invalid synthetic metadata');path.chmod(0o600)
   c=Client(directory)
   try:
    self.assertIn('private-sync',c.call('runtime_info')['features'])
    self.assertTrue(c.call('backup_status')['dailyBackupError'])
    self.assertTrue(c.call('sync_status')['setupError'])
    for op in ['backup_now','sync_devices','peer_list']:
     with self.assertRaises(RuntimeError):c.call(op)
    original=c.call('save',scene=scene(),parent=None)
    changed=c.call('save',scene=scene(TEXT+' still writing'),parent=original['revisionId'])
    self.assertEqual(c.call('load'),changed)
   finally:c.close()
   reopened=Client(directory)
   try:self.assertEqual(reopened.call('load'),changed)
   finally:reopened.close()
 def test_revision_bound_patch_archives_and_crash_reopen(self):
  with tempfile.TemporaryDirectory(prefix='novel-companion-fixture-') as directory:
   c=Client(directory)
   p=c.call('create_project',title='Synthetic project');ch=c.call('create_chapter',projectId=p['id'],title='Chapter')
   s=c.call('create_scene',chapterId=ch['id'],scene=scene())
   sha=hashlib.sha256(TEXT.encode()).hexdigest();source={'sceneId':s['id'],'revisionId':s['revisionId'],'hash':sha}
   anchor={**source,'offsetUnit':'utf16','start':0,'end':4,'quote':'مِمِ','prefix':'','suffix':' 中文'}
   patch={'suggestionId':'fixture','source':source,'anchor':anchor,'replacement':'سلام'}
   receipt=c.call('apply_scene_patch',patch=patch);self.assertNotEqual(receipt['revisionId'],s['revisionId'])
   saved=c.call('load_scene',sceneId=s['id']);self.assertEqual(saved['scene']['document']['content'][0]['content'][0]['marks'],[{'type':'bold'}])
   self.assertEqual(saved['scene']['document']['content'][0]['content'][0]['text'],'سلام')
   with self.assertRaisesRegex(RuntimeError,'Conflict'):c.call('apply_scene_patch',patch=patch)
   c.call('snapshot_scene',sceneId=s['id'],parent=saved['revisionId'])
   archive=c.call('export_archive',projectId=p['id']);imported=c.call('import_archive',archive=archive)
   self.assertNotEqual(imported['project']['id'],p['id']);self.assertEqual(len(imported['scenes']),1)
   c.p.kill();c.p.wait();c.p.stdin.close();c.p.stdout.close();c.p.stderr.close()
   reopened=Client(directory);self.assertEqual(reopened.call('open_project',projectId=p['id'])['scenes'][0]['scene']['document'],saved['scene']['document']);reopened.close()
 def test_single_owner_and_idempotent_saves(self):
  with tempfile.TemporaryDirectory(prefix='novel-owner-fixture-') as directory:
   c=Client(directory);c.call('list_projects');other=Client(directory);self.assertNotEqual(other.p.wait(timeout=10),0);other.p.stdin.close();other.p.stdout.close();other.p.stderr.close()
   first=c.call('save',scene=scene(),parent=None);bundle=c.call('list_projects');b=c.call('open_project',projectId=bundle[0]['id']);sid=b['scenes'][0]['id']
   changed=scene(TEXT+' suffix');a=c.call('save_scene',sceneId=sid,scene=changed,parent=first['revisionId'],requestId='fixed-operation')
   duplicate=c.call('save_scene',sceneId=sid,scene=changed,parent=first['revisionId'],requestId='fixed-operation');self.assertEqual(a,duplicate)
   c.close()
if __name__=='__main__':unittest.main()
