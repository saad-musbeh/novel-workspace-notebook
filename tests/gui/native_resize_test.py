"""Resize only the owned synthetic debug window; never private writing/windows."""
import json,os,signal,sqlite3,subprocess,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
class NativeResizeTest(unittest.TestCase):
 def test_fullscreen_and_resize_preserve_viewport_input_and_controls(self):
  with tempfile.TemporaryDirectory(prefix='novel-resize-fixture-') as temporary:
   directory=Path(temporary)
   env=dict(os.environ,NOVEL_PERFORMANCE_PROBE=temporary,NOVEL_LONG_WRITING_PROBE='1',NOVEL_RESIZE_PROBE='1',XDG_CACHE_HOME=str(directory/'cache'))
   process=subprocess.Popen([str(ROOT/'target/debug/novel-workspace')],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
   try:out,err=process.communicate(timeout=60)
   except subprocess.TimeoutExpired:
    os.killpg(process.pid,signal.SIGKILL);out,err=process.communicate();self.fail('Own resize fixture timed out: '+err.decode(errors='replace')[-2000:])
   self.assertEqual(process.returncode,0,err.decode(errors='replace')[-2000:])
   result=json.loads((directory/'performance.json').read_text());self.assertNotIn('error',result,result)
   self.assertTrue(result['exact'],result);self.assertTrue(result['persistedExact'],result);self.assertFalse(result['failed'],result);self.assertEqual(result['pendingOperations'],0,result)
   self.assertEqual(len(result['resizeSamples']),8)
   self.assertTrue(result['rapidCovered'],result);self.assertLessEqual(result['rapidRequests'],6,result)
   for sample in result['resizeSamples']:
    if sample['nativeSizeApplied'] is not None:self.assertTrue(sample['nativeSizeApplied'],sample)
    for check in ['covered','anchorPreserved','controlsVisible','boundedPaint','caretVisible']:self.assertTrue(sample[check],sample)
    self.assertEqual(sample['idleRequests'],0,sample);self.assertLessEqual(sample['resizeRequests'],4,sample)
   expected='Long synthetic text مِمِ Chinese 中文 English. '*1200+' مِ中文'*8
   with sqlite3.connect(f'file:{directory/"editor-spike.sqlite3"}?mode=ro',uri=True) as db:
    row=db.execute('SELECT payload FROM scene_revision JOIN scene_head ON scene_revision.id=scene_head.revision_id').fetchone()
    scene=json.loads(row[0]);actual=''.join(n.get('text','') for n in scene['document']['content'][0]['content'])
    self.assertEqual(actual,expected);self.assertEqual(db.execute('PRAGMA integrity_check').fetchone(),('ok',))
if __name__=='__main__':unittest.main()
