"""Private synthetic park/resume through the actual stdio helper boundary."""
import json, subprocess, sys, tempfile, unittest
from pathlib import Path

ENGINE = Path(__file__).resolve().parents[1] / 'tools/native_editor_engine.py'

class ResumeIntegrationTests(unittest.TestCase):
    def test_resumed_helper_preserves_rich_paragraph_swap_undo_and_redo(self):
        with tempfile.TemporaryDirectory() as directory:
            process = subprocess.Popen([sys.executable,str(ENGINE),directory],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
            def request(value):
                process.stdin.write(json.dumps(value,ensure_ascii=False)+'\n');process.stdin.flush()
                reply=json.loads(process.stdout.readline())
                self.assertTrue(reply['ok'],reply.get('error'))
                return reply['frame']
            original={'type':'doc','content':[{'type':'paragraph','attrs':{'lang':'ar','dir':'rtl','lineSpacing':1.15},'content':[{'type':'text','text':'يس مِ 中文 👩🏽‍🚀','marks':[{'type':'italic'}]}]},{'type':'paragraph','content':[{'type':'text','text':'untouched'}]}]}
            try:
                request({'type':'open','width':425,'scene':{'schemaVersion':1,'title':'Synthetic','document':original}})
                capture=request({'type':'command','command':'captureParagraph'})['paragraphCapture']
                replacement={'type':'paragraph','attrs':{'lang':'ar','dir':'rtl','lineSpacing':1.15},'content':[{'type':'text','text':'يس alternative مِ 中文','marks':[{'type':'bold'}]}]}
                changed=request({'type':'command','command':'swapParagraph','anchor':capture,'replacement':replacement,'preview':False})['document']
                self.assertEqual(changed['content'][1],original['content'][1])
                request({'type':'command','command':'park'})
            finally:
                process.stdin.close();process.wait(timeout=10);process.stdout.close()
            process = subprocess.Popen([sys.executable,str(ENGINE),directory],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
            try:
                resumed=request({'type':'resume'})
                self.assertEqual(resumed['document'],changed)
                self.assertEqual(resumed['width'],425);self.assertTrue(resumed['image'].startswith('data:image/png;base64,'))
                undone=request({'type':'command','command':'undo','preview':False})
                self.assertEqual(undone['document'],original)
                redone=request({'type':'command','command':'redo','preview':False})
                self.assertEqual(redone['document'],changed)
            finally:
                process.stdin.close();process.wait(timeout=10);process.stdout.close()

if __name__=='__main__':unittest.main()
