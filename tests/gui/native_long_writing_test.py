"""Long input/viewport regression in an isolated debug instance; never personal writing."""
import json,os,signal,sqlite3,subprocess,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
class NativeLongWritingTest(unittest.TestCase):
 def test_long_paragraph_keeps_caret_painted_and_commits_exact_text(self):
  with tempfile.TemporaryDirectory(prefix='novel-long-writing-') as temporary:
   directory=Path(temporary)
   env=dict(os.environ,NOVEL_PERFORMANCE_PROBE=temporary,NOVEL_LONG_WRITING_PROBE='1')
   process=subprocess.Popen([str(ROOT/'target/debug/novel-workspace')],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
   try:out,err=process.communicate(timeout=60)
   except subprocess.TimeoutExpired:
    os.killpg(process.pid,signal.SIGKILL);out,err=process.communicate();self.fail('Own long-writing fixture timed out: '+err.decode(errors='replace')[-2000:])
   self.assertEqual(process.returncode,0,err.decode(errors='replace')[-2000:])
   result=json.loads((directory/'performance.json').read_text());self.assertNotIn('error',result,result)
   self.assertEqual(result['initialCharacters'],54000);self.assertEqual(result['failures'],0);self.assertEqual(result['viewportFailures'],0,result.get('viewportFailuresDetail',result))
   self.assertTrue(result['verticalNavigationCorrect'],result);self.assertTrue(result['manualScrollPreserved']);self.assertTrue(result['manualViewportPainted']);self.assertEqual(result['characterCount'],'51655');self.assertTrue(result['burst']['exact']);self.assertTrue(result['burst']['persistedExact'])
   expected='Long synthetic text مِمِ Chinese 中文 English. '*1200+' سلام مِمِمِ中文'*5
   with sqlite3.connect(f'file:{directory/"editor-spike.sqlite3"}?mode=ro',uri=True) as db:
    row=db.execute('SELECT payload FROM scene_revision JOIN scene_head ON scene_revision.id=scene_head.revision_id').fetchone()
    scene=json.loads(row[0]);actual=''.join(n.get('text','') for n in scene['document']['content'][0]['content'])
    self.assertEqual(actual,expected);self.assertEqual(db.execute('PRAGMA integrity_check').fetchone(),('ok',))
if __name__=='__main__':unittest.main()
