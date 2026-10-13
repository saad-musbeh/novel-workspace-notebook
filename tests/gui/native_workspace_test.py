"""Own debug-instance integration hooks; never operates personal app windows."""
import json,os,signal,sqlite3,subprocess,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
class NativeWorkspaceTest(unittest.TestCase):
 def test_desktop_destinations_and_native_archive_delivery(self):
  with tempfile.TemporaryDirectory(prefix='novel-desktop-fixture-') as d:
   env=dict(os.environ,NOVEL_PERFORMANCE_PROBE=d,NOVEL_WORKSPACE_PROBE='1',NOVEL_FILE_PROBE_DIRECTORY=d)
   p=subprocess.Popen([str(ROOT/'target/debug/novel-workspace')],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
   try:out,err=p.communicate(timeout=45)
   except subprocess.TimeoutExpired:
    os.killpg(p.pid,signal.SIGKILL);out,err=p.communicate();self.fail('Integrated desktop fixture timed out: '+err.decode(errors='replace')[-2000:])
   self.assertEqual(p.returncode,0,err.decode(errors='replace')[-2000:]);report=Path(d)/'performance.json';self.assertTrue(report.exists(),err.decode(errors='replace')[-2000:]);result=json.loads(report.read_text());self.assertNotIn('error',result,result)
   for width in [1280,900,760]:
    for destination in ['write','plan','review','alphabets','library']:self.assertTrue(any(c.startswith(f'{destination} at {width}px') for c in result['checks']),result)
   self.assertIn('Partial peer failure preserves error, reconciles acknowledged writing/details and releases input',result['checks'])
   self.assertTrue((Path(d)/'synthetic-export.html').is_file())
   self.assertTrue((Path(d)/'synthetic-export.novel.json').is_file())
   with sqlite3.connect(f'file:{Path(d)/"editor-spike.sqlite3"}?mode=ro',uri=True) as db:self.assertEqual(db.execute('PRAGMA integrity_check').fetchone(),('ok',));self.assertFalse(db.execute('PRAGMA foreign_key_check').fetchall())
if __name__=='__main__':unittest.main()
