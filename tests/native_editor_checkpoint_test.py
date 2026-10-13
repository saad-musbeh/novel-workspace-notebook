import json,os,stat,sys,tempfile,unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from native_editor_engine import NativeEditor
from native_editor_checkpoint import read_checkpoint,MAX_BYTES

class CheckpointTests(unittest.TestCase):
    def fixture(self,directory):
        opening={'type':'open','width':640,'scene':{'schemaVersion':1,'title':'Synthetic','document':{'type':'doc','content':[{'type':'paragraph','attrs':{'lang':'ar','dir':'rtl','lineSpacing':1.15},'content':[{'type':'text','text':'يس مِ hello 中文 👩🏽‍🚀','marks':[{'type':'bold'}]}]},{'type':'paragraph','content':[{'type':'text','text':'unchanged'}]}]}}}
        editor=NativeEditor(directory,opening);editor.request(opening,True);return editor
    def test_rich_unicode_undo_redo_selection_and_shared_blocks_survive_parking(self):
        with tempfile.TemporaryDirectory() as directory:
            editor=self.fixture(directory);before=editor.model.document_snapshot()
            editor.model.move_to((0,len(editor.model.block_text(0))))
            editor.request({'type':'text','text':' مِ𐐷'});after=editor.model.document_snapshot()
            editor.request({'type':'key','key':'ArrowLeft','shift':True})
            selected=editor.model.selection_text();position=editor.model._state()[1:]
            editor.request({'type':'command','command':'park'})
            checkpoint,restored=read_checkpoint(directory)
            self.assertEqual(restored.document_snapshot(),after)
            self.assertEqual(restored._state()[1:],position);self.assertEqual(restored.selection_text(),selected)
            self.assertIs(restored.blocks[1],restored.undo[0][0][0][1])
            self.assertTrue(restored.history());self.assertEqual(restored.document_snapshot(),before)
            self.assertTrue(restored.history(True));self.assertEqual(restored.document_snapshot(),after)
            self.assertEqual(stat.S_IMODE(os.stat(Path(directory)/'checkpoint.json').st_mode),0o600)
            self.assertEqual(checkpoint['opening'],editor.opening)
    def test_corrupt_reference_and_document_fail_without_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            editor=self.fixture(directory);editor.request({'type':'command','command':'park'})
            path=Path(directory)/'checkpoint.json';original=json.loads(path.read_text())
            broken=json.loads(path.read_text());broken['state'][0][0]=-1;path.write_text(json.dumps(broken))
            with self.assertRaisesRegex(ValueError,'reference'):read_checkpoint(directory)
            broken=original;broken['document']['content'][0]['content'][0]['text']='silently changed';path.write_text(json.dumps(broken))
            with self.assertRaisesRegex(ValueError,'does not match'):read_checkpoint(directory)
    def test_park_write_failure_preserves_open_text_history_and_cursor(self):
        with tempfile.TemporaryDirectory() as directory:
            editor=self.fixture(directory);editor.request({'type':'text','text':'x'});before=editor.model._state();undo=editor.model.undo[:]
            (Path(directory)/'checkpoint.pending').mkdir()
            with self.assertRaises(OSError):editor.request({'type':'command','command':'park'})
            self.assertEqual(editor.model._state(),before);self.assertEqual(editor.model.undo,undo)
    def test_redo_stack_survives_checkpoint(self):
        with tempfile.TemporaryDirectory() as directory:
            editor=self.fixture(directory);editor.request({'type':'text','text':'new'});after=editor.model.document_snapshot();editor.request({'type':'command','command':'undo'});editor.request({'type':'command','command':'park'})
            _,restored=read_checkpoint(directory);self.assertTrue(restored.history(True));self.assertEqual(restored.document_snapshot(),after)

    def test_wide_viewport_checkpoint_retains_exact_unicode_history_and_old_widths(self):
        for width in (640,1200,4096):
            with tempfile.TemporaryDirectory() as directory:
                editor=self.fixture(directory);before=editor.model.document_snapshot()
                editor.request({'type':'text','text':' مِ𐐷'});after=editor.model.document_snapshot()
                frame=editor.request({'type':'refresh','width':width,'viewportHeight':2160})
                self.assertEqual(frame['width'],width)
                editor.request({'type':'command','command':'park'})
                checkpoint,restored=read_checkpoint(directory)
                self.assertEqual(checkpoint['width'],width);self.assertEqual(checkpoint['height'],2160)
                self.assertEqual(restored.document_snapshot(),after)
                self.assertTrue(restored.history());self.assertEqual(restored.document_snapshot(),before)

if __name__=='__main__':unittest.main()
