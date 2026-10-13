"""Compare the Rust helper to the retained Python reference using synthetic stdio.
No personal store, installed shortcut, provider or network is opened.
"""
import json,random,subprocess,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
class Parity(unittest.TestCase):
 def test_exact_unicode_commands_layout_frames_and_checkpoint_resume(self):
  with tempfile.TemporaryDirectory() as directory:
   processes=[]
   try:
    for name,command in [('python',['/usr/bin/python3',str(ROOT/'tools/native_editor_engine.py')]),('rust',[str(ROOT/'target/debug/novel-editor')])]:
     folder=Path(directory)/name;folder.mkdir()
     processes.append(subprocess.Popen(command+[str(folder)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True))
    operations=[]
    def compare(operation):
     replies=[]
     for process in processes:
      process.stdin.write(json.dumps(operation,ensure_ascii=False)+'\n');process.stdin.flush();line=process.stdout.readline();self.assertTrue(line,'Editor exited unexpectedly');replies.append(json.loads(line))
     self.assertEqual(replies[0]['ok'],replies[1]['ok'],(operation,replies))
     if replies[0]['ok']:
      for key in ['document','position','selectedText','states','searchResult','unicodeResult','navigationCapture','paragraphCapture','width','height','totalHeight','top','cursor','selection','missingGlyphs']:
       self.assertEqual(replies[0]['frame'].get(key),replies[1]['frame'].get(key),(operation,key,replies[0]['frame'].get(key),replies[1]['frame'].get(key)))
     operations.append(operation['type'])
     return replies[1]
    original={'type':'doc','content':[{'type':'heading','attrs':{'lang':'ar','dir':'rtl','level':2,'lineSpacing':1.15},'content':[{'type':'text','text':'سلام مِ 中文 繁體 👩🏽‍🚀 e\u0301','marks':[{'type':'underline'}]}]},{'type':'paragraph','content':[{'type':'text','text':'English 0041 second'}]}]}
    compare({'type':'open','scene':{'schemaVersion':1,'title':'Synthetic','document':original},'width':640})
    for operation in [
     {'type':'key','key':'End','ctrl':True},
     {'type':'command','command':'unicodeToggle'},
     {'type':'text','text':'\nمِ\nChinese 𠀀'},
     {'type':'command','command':'selectAll'},
     {'type':'command','command':'bold'},
     {'type':'command','command':'lineSpacing','lineSpacing':1},
     {'type':'command','command':'undo'},
     {'type':'command','command':'redo'},
     {'type':'command','command':'find','query':'中文'},
     {'type':'command','command':'replace','query':'中文','replacement':'世界'},
     {'type':'command','command':'find','query':'not present'},
     {'type':'key','key':'Home','ctrl':True},
     {'type':'key','key':'ArrowRight'},
     {'type':'key','key':'ArrowDown','shift':True},
     {'type':'key','key':'End'},
     {'type':'key','key':'ArrowLeft'},
     {'type':'mouse','phase':'down','x':900,'y':550,'count':2},
     {'type':'mouse','phase':'up','x':900,'y':550},
     {'type':'refresh','width':320,'viewportHeight':200},
     {'type':'text','text':'يس中','inputs':['ي','س','中'],'deferRender':True},
     {'type':'snapshot'},
     {'type':'command','command':'captureParagraph'},
     {'type':'command','command':'captureNavigation'},
     {'type':'command','command':'park'},
     {'type':'resume'},
     {'type':'command','command':'undo'},
    ]:compare(operation)
    rng=random.Random(20261010)
    for _ in range(120):
     operation=rng.choice([{'type':'text','text':rng.choice(['مِ','中文','👩🏽‍🚀','e\u0301','\n','X'])},{'type':'key','key':rng.choice(['Backspace','Delete','ArrowLeft','ArrowRight','Home','End']),'ctrl':rng.choice([False,True]),'shift':rng.choice([False,True])},{'type':'command','command':rng.choice(['undo','redo','bold','italic','underline'])}])
     compare(operation)
    print('Rust/Python parity: '+str(len(operations))+' operations matched manuscript, selection, Undo and layout geometry')
   finally:
    for process in processes:
     process.stdin.close();process.wait(timeout=5);process.stdout.close();process.stderr.close()
if __name__=='__main__':unittest.main()
