"""Our model/shape RPC transaction checks, only private synthetic editor data."""
import sys,tempfile,unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from native_editor_engine import NativeEditor

class EngineTests(unittest.TestCase):
    def setUp(self):
        self.directory=tempfile.TemporaryDirectory()
        self.original={'type':'doc','content':[{'type':'paragraph','content':[{'type':'text','text':'مِ 中文 👩🏽‍🚀 e\u0301'}]}]}
        self.editor=NativeEditor(self.directory.name,{'scene':{'schemaVersion':1,'title':'Synthetic','document':self.original}})
    def tearDown(self):self.directory.cleanup()
    def test_large_viewport_limits_refuse_before_unicode_or_undo_mutation(self):
        before=self.editor.model._state();history=self.editor.model.undo[:]
        for request in ({'type':'text','text':'unsafe','width':4097},{'type':'text','text':'unsafe','viewportHeight':2161}):
            with self.assertRaises(ValueError):self.editor.request(request)
            self.assertEqual(self.editor.model._state(),before);self.assertEqual(self.editor.model.undo,history)

    def test_render_failure_rolls_back_text_selection_history_and_viewport(self):
        self.editor.model.move_to((0,2));self.editor.request({'type':'text','text':'X','preview':True})
        before=self.editor.model.document_snapshot();state=self.editor.model._state();history=self.editor.model.undo[:]
        render=self.editor.layout.frame
        def fail(*args,**kwargs):raise RuntimeError('Injected raster failure')
        self.editor.layout.frame=fail
        with self.assertRaisesRegex(RuntimeError,'Injected'):self.editor.request({'type':'text','text':'unacknowledged','top':30,'viewportHeight':400,'preview':True})
        self.assertEqual(self.editor.model._state(),state);self.assertEqual(self.editor.model.document_snapshot(),before);self.assertEqual(self.editor.model.undo,history)
        self.assertEqual((self.editor.top,self.editor.height),(0,680))
        self.editor.layout.frame=render
        self.assertEqual(self.editor.request({'type':'snapshot'})['document'],before)
    def test_invalid_viewport_and_pointer_refuse_before_mutation(self):
        for request in [{'type':'text','text':'X','top':True},{'type':'text','text':'X','viewportHeight':100},{'type':'text','text':'X','followCursor':'yes'},*[{'type':'mouse','x':0,'y':0,'phase':'down','count':count} for count in [0,4,True]]]:
            with self.assertRaises(ValueError):self.editor.request(request)
            self.assertEqual(self.editor.model.document_snapshot(),self.original);self.assertEqual(self.editor.model.undo,[])
    def test_large_layout_viewport_and_pointer_bounds_use_content_height(self):
        layout=self.editor.layout;layout.total_height=2_000_000.5
        observed=[]
        layout.locate_point=lambda x,y:observed.append((x,y)) or (0,0)
        layout.prepare=lambda:None
        layout.frame=lambda top,height,follow:{'top':top}
        self.editor.request({'type':'mouse','phase':'down','x':0,'y':30_000_000,'top':2_000_000,'preview':True})
        self.assertEqual(observed,[(0,2_000_000)]);self.assertEqual(self.editor.top,2_000_000)
        for fields in ({'top':2_000_682},{'top':float('nan')},{'top':True},{'y':30_010_216},{'y':float('nan')},{'y':True}):
            request={'type':'mouse','phase':'down','x':0,'y':0,'top':0,'preview':True};request.update(fields)
            with self.assertRaises(ValueError):self.editor.request(request)
            self.assertEqual(self.editor.model.document_snapshot(),self.original)
        self.assertEqual(len(observed),1)
    def test_preview_does_not_copy_or_publish_document_but_checkpoint_does(self):
        self.editor.model.move_to((0,0));frame=self.editor.request({'type':'text','text':'X','preview':True})
        self.assertNotIn('document',frame);self.assertTrue(frame['image'].startswith('data:image/png;base64,'))
        exact=self.editor.request({'type':'snapshot','preview':True})['document'];self.assertEqual(exact['content'][0]['content'][0]['text'],'Xمِ 中文 👩🏽‍🚀 e\u0301')
        self.assertEqual(self.editor.request({'type':'refresh'})['document'],exact)
    def test_resize_reflows_without_document_or_history_changes_and_rolls_back_failure(self):
        document={'type':'doc','content':[{'type':'paragraph','attrs':{},'content':[{'type':'text','text':'Exact مِ 中文 e\u0301 '*100}]}]}
        editor=NativeEditor(self.directory.name,{'scene':{'schemaVersion':1,'title':'Resize','document':document},'width':640})
        frame=editor.request({'type':'refresh','top':300,'viewportHeight':200})
        old_top=frame['top'];old_width=editor.layout.width
        old_point=editor.layout.locate_point(0,old_top)
        resized=editor.request({'type':'refresh','width':320,'viewportHeight':200})
        self.assertEqual(resized['document'],document);self.assertEqual(editor.model.undo,[])
        self.assertEqual(editor.layout.width,320);self.assertGreater(resized['top'],old_top)
        self.assertEqual(editor.layout.locate_point(0,resized['top'])[0],old_point[0])
        state=(editor.top,editor.layout.width,editor.model._state())
        render=editor.layout.frame
        def fail(*args,**kwargs):raise RuntimeError('Injected resize paint failure')
        editor.layout.frame=fail
        with self.assertRaisesRegex(RuntimeError,'resize'):editor.request({'type':'refresh','width':old_width})
        self.assertEqual((editor.top,editor.layout.width,editor.model._state()),state)
        editor.layout.frame=render
        self.assertEqual(editor.request({'type':'snapshot'})['document'],document)
    def test_unicode_invalid_and_guarded_replace_are_nonfatal_unchanged(self):
        self.editor.model.move_to((0,0));result=self.editor.request({'type':'command','command':'unicodeToggle'})
        self.assertFalse(result['unicodeResult']['converted']);self.assertEqual(result['document'],self.original)
        result=self.editor.request({'type':'command','command':'replace','query':'missing','replacement':'X'})
        self.assertFalse(result['searchResult']['replaced']);self.assertEqual(result['document'],self.original)
    def test_unicode_hexlike_latin_result_remains_reversible_across_checkpoint(self):
        self.editor.model.move_to((0,0));self.editor.request({'type':'text','text':'0041'})
        first=self.editor.request({'type':'command','command':'unicodeToggle'})
        self.assertTrue(first['unicodeResult']['converted']);self.assertEqual(first['selectedText'],'A')
        self.editor.request({'type':'snapshot'})
        second=self.editor.request({'type':'command','command':'unicodeToggle'})
        self.assertEqual(second['selectedText'],'0041')
    def test_two_owned_models_keep_independent_text_history_and_cursor(self):
        second=NativeEditor(self.directory.name,{'scene':{'schemaVersion':1,'title':'Second','document':self.original}})
        self.editor.request({'type':'text','text':'LEFT','preview':True});second.request({'type':'text','text':'RIGHT','preview':True})
        self.editor.request({'type':'command','command':'undo','preview':True})
        self.assertEqual(self.editor.model.document_snapshot(),self.original)
        self.assertTrue(second.model.flat_text().startswith('RIGHT'));self.assertEqual(second.model.cursor,(0,5));self.assertEqual(len(second.model.undo),1)

    def test_ordered_typing_batch_shapes_once_and_retains_individual_undo(self):
        inputs=['ي','س','م','ِ','中文','👩🏽‍🚀'];before=self.editor.model.document_snapshot()
        prepare=self.editor.layout.prepare;calls=[]
        def observe():calls.append(True);return prepare()
        self.editor.layout.prepare=observe
        frame=self.editor.request({'type':'text','text':''.join(inputs),'inputs':inputs,'preview':True})
        self.assertEqual(len(calls),1);self.assertNotIn('document',frame)
        self.assertTrue(self.editor.model.flat_text().startswith(''.join(inputs)))
        for index in range(len(inputs)-1,-1,-1):
            self.editor.request({'type':'command','command':'undo','preview':True})
            self.assertTrue(self.editor.model.flat_text().startswith(''.join(inputs[:index])))
        self.assertEqual(self.editor.model.document_snapshot(),before)
        for _ in inputs:self.editor.request({'type':'command','command':'redo','preview':True})
        self.assertTrue(self.editor.model.flat_text().startswith(''.join(inputs)))

    def test_batch_validation_and_render_failure_roll_back_all_inputs_and_history(self):
        state=self.editor.model._state();history=self.editor.model.undo[:]
        for request in [
            {'type':'text','text':'bad','inputs':['b','a']},
            {'type':'text','text':'','inputs':[]},
            {'type':'text','text':'x'*129,'inputs':['x']*129},
            {'type':'text','text':'x'*300_000,'inputs':['x'*300_000]},
            {'type':'text','text':'ok\x00','inputs':['ok','\x00']},
            {'type':'command','command':'undo','deferRender':True},
        ]:
            with self.assertRaises(ValueError):self.editor.request(request)
            self.assertEqual(self.editor.model._state(),state);self.assertEqual(self.editor.model.undo,history)
        render=self.editor.layout.frame
        self.editor.layout.frame=lambda *args:(_ for _ in ()).throw(RuntimeError('Batch raster failure'))
        with self.assertRaisesRegex(RuntimeError,'Batch'):self.editor.request({'type':'text','text':'يس中','inputs':['ي','س','中']})
        self.assertEqual(self.editor.model._state(),state);self.assertEqual(self.editor.model.undo,history)
        self.editor.layout.frame=render

    def test_deferred_ordinary_backlog_is_acknowledged_then_fully_shaped_before_snapshot(self):
        before=self.editor.model.document_snapshot();builds=self.editor.layout.layout_builds
        first=self.editor.request({'type':'text','text':'يس','inputs':['ي','س'],'deferRender':True,'preview':True})
        self.assertTrue(first['deferred']);self.assertNotIn('image',first)
        self.assertEqual(self.editor.layout.layout_builds,builds)
        final=self.editor.request({'type':'text','text':'مِ中','inputs':['م','ِ','中'],'preview':True})
        self.assertIn('image',final);self.assertEqual(self.editor.layout.layout_builds,builds+1)
        exact=self.editor.request({'type':'snapshot','preview':True})['document']
        self.assertEqual(self.editor.model.flat_text(),'يسمِ中مِ 中文 👩🏽‍🚀 e\u0301')
        self.assertNotEqual(exact,before)
        self.editor.request({'type':'command','command':'undo','preview':True})
        self.assertTrue(self.editor.model.flat_text().startswith('يسمِمِ'))

    def test_final_render_failure_retains_prior_apply_ack_and_refuses_partial_final_batch(self):
        self.editor.request({'type':'text','text':'يس','inputs':['ي','س'],'deferRender':True,'preview':True})
        acknowledged=self.editor.model.document_snapshot();state=self.editor.model._state();history=self.editor.model.undo[:]
        render=self.editor.layout.frame
        self.editor.layout.frame=lambda *args:(_ for _ in ()).throw(RuntimeError('Final paint failure'))
        with self.assertRaisesRegex(RuntimeError,'Final'):self.editor.request({'type':'text','text':'مِ','inputs':['م','ِ'],'preview':True})
        self.assertEqual(self.editor.model.document_snapshot(),acknowledged)
        self.assertEqual(self.editor.model._state(),state);self.assertEqual(self.editor.model.undo,history)
        self.editor.layout.frame=render
        checkpoint=self.editor.request({'type':'snapshot','preview':True})
        self.assertEqual(checkpoint['document'],acknowledged);self.assertIn('image',checkpoint)

    def test_batch_pixels_geometry_match_sequential_whole_paragraph_bidi(self):
        inputs=['Latin ','يس',' مِ',' 中文',' 👩‍💻',' \u202eabc','\u202c end']
        sequential=NativeEditor(self.directory.name,{'scene':{'schemaVersion':1,'title':'Sequential','document':self.original}})
        for text in inputs:expected=sequential.request({'type':'text','text':text,'preview':True})
        actual=self.editor.request({'type':'text','text':''.join(inputs),'inputs':inputs,'preview':True})
        for key in ('image','cursor','selection','position','totalHeight'):
            self.assertEqual(actual[key],expected[key])
        self.assertEqual(self.editor.model.document_snapshot(),sequential.model.document_snapshot())

    def test_paragraph_capture_is_readonly_and_swap_preserves_exact_rich_blocks_with_one_undo(self):
        rich={'type':'heading','attrs':{'level':2,'lang':'ar','dir':'rtl','lineSpacing':1.5},'content':[{'type':'text','text':'مِ 中文','marks':[{'type':'bold'}]},{'type':'hardBreak'},{'type':'text','text':'👩🏽‍🚀 e\u0301','marks':[{'type':'italic'},{'type':'underline'}]}]}
        original={'type':'doc','content':[rich,{'type':'paragraph','content':[{'type':'text','text':'Untouched 中文'}]}]}
        editor=NativeEditor(self.directory.name,{'scene':{'schemaVersion':1,'title':'Alternatives','document':original}})
        editor.model.move_to((0,2));state=editor.model._state()
        capture=editor.request({'type':'command','command':'captureParagraph','preview':True})['paragraphCapture']
        self.assertEqual(capture['block'],rich);self.assertEqual(editor.model._state(),state);self.assertEqual(editor.model.undo,[])
        replacement={'type':'paragraph','attrs':{'lang':'zh-Hant','dir':'auto','lineSpacing':1},'content':[{'type':'text','text':'替代 مِ 👩🏽‍🚀','marks':[{'type':'underline'}]}]}
        anchor={key:capture[key] for key in ('blockIndex','documentHash')}
        result=editor.request({'type':'command','command':'swapParagraph','anchor':anchor,'replacement':replacement,'preview':True})
        self.assertEqual(len(editor.model.undo),1);self.assertEqual(editor.model.document_snapshot()['content'],[replacement,original['content'][1]])
        self.assertNotEqual(result['paragraphCapture']['documentHash'],capture['documentHash'])
        editor.request({'type':'command','command':'undo'});self.assertEqual(editor.model.document_snapshot(),original)
        editor.request({'type':'command','command':'redo'});self.assertEqual(editor.model.document_snapshot()['content'][0],replacement)

    def test_paragraph_swap_refuses_stale_whole_scene_anchor_and_duplicate_text_targets(self):
        original={'type':'doc','content':[{'type':'paragraph','content':[{'type':'text','text':'Repeated مِ'}]},{'type':'paragraph','content':[{'type':'text','text':'Repeated مِ'}]}]}
        editor=NativeEditor(self.directory.name,{'scene':{'schemaVersion':1,'title':'Anchors','document':original}})
        editor.model.move_to((1,0));capture=editor.request({'type':'command','command':'captureParagraph'})['paragraphCapture']
        editor.request({'type':'text','text':'changed elsewhere '});state=editor.model._state();history=editor.model.undo[:]
        with self.assertRaisesRegex(ValueError,'scene changed'):editor.request({'type':'command','command':'swapParagraph','anchor':capture,'replacement':{'type':'paragraph','content':[{'type':'text','text':'Alternative'}]}})
        self.assertEqual(editor.model._state(),state);self.assertEqual(editor.model.undo,history)
        editor.request({'type':'command','command':'undo'});capture=editor.request({'type':'command','command':'captureParagraph'})['paragraphCapture']
        editor.request({'type':'command','command':'swapParagraph','anchor':capture,'replacement':{'type':'paragraph','content':[{'type':'text','text':'Only second'}]}})
        self.assertEqual(editor.model.block_text(0),'Repeated مِ');self.assertEqual(editor.model.block_text(1),'Only second')

    def test_paragraph_swap_render_failure_rolls_back_replacement_and_undo(self):
        capture=self.editor.request({'type':'command','command':'captureParagraph'})['paragraphCapture'];state=self.editor.model._state();history=self.editor.model.undo[:]
        render=self.editor.layout.frame;self.editor.layout.frame=lambda *args:(_ for _ in ()).throw(RuntimeError('Paragraph paint refused'))
        with self.assertRaisesRegex(RuntimeError,'Paragraph'):self.editor.request({'type':'command','command':'swapParagraph','anchor':capture,'replacement':{'type':'paragraph','content':[{'type':'text','text':'Unsaved alternative مِ'}]}})
        self.assertEqual(self.editor.model._state(),state);self.assertEqual(self.editor.model.undo,history)
        self.editor.layout.frame=render
        self.assertEqual(self.editor.request({'type':'snapshot'})['document'],self.original)

    def test_navigation_restore_preserves_exact_selection_document_and_undo_redo(self):
        self.editor.request({'type':'text','text':'يس مِ 中文'})
        self.editor.request({'type':'key','key':'Home','ctrl':True})
        self.editor.request({'type':'key','key':'ArrowRight','shift':True})
        saved=self.editor.request({'type':'command','command':'captureNavigation'})['navigationCapture']
        document=self.editor.model.document_snapshot();undo=self.editor.model.undo[:];redo=self.editor.model.redo[:]
        self.editor.request({'type':'key','key':'End','ctrl':True})
        restored=self.editor.request({'type':'command','command':'restoreNavigation','bookmark':saved})
        self.assertEqual(restored['position'],{key:saved[key] for key in ('cursor','anchor')})
        self.assertEqual(self.editor.model.document_snapshot(),document);self.assertEqual(self.editor.model.undo,undo);self.assertEqual(self.editor.model.redo,redo)
        self.editor.request({'type':'key','key':'ArrowDown'})
        self.editor.request({'type':'key','key':'ArrowUp'})
        self.assertEqual(self.editor.model.document_snapshot(),document);self.assertEqual(self.editor.model.undo,undo)
        self.editor.request({'type':'command','command':'undo'});self.assertEqual(self.editor.model.document_snapshot(),self.original)
        self.editor.request({'type':'command','command':'redo'});self.assertEqual(self.editor.model.document_snapshot(),document)
    def test_navigation_stale_or_invalid_positions_refuse_without_mutating_caret_or_history(self):
        saved=self.editor.request({'type':'command','command':'captureNavigation'})['navigationCapture']
        self.editor.request({'type':'text','text':'changed'})
        before=self.editor.model._state();history=self.editor.model.undo[:]
        with self.assertRaisesRegex(ValueError,'scene changed'):self.editor.request({'type':'command','command':'restoreNavigation','bookmark':saved})
        self.assertEqual(self.editor.model._state(),before);self.assertEqual(self.editor.model.undo,history)
        saved=self.editor.request({'type':'command','command':'captureNavigation'})['navigationCapture']
        for point in [[0,100000],[True,0],[-1,0],[0,False]]:
            broken={**saved,'cursor':point}
            with self.assertRaisesRegex(ValueError,'Invalid reading'):self.editor.request({'type':'command','command':'restoreNavigation','bookmark':broken})
            self.assertEqual(self.editor.model._state(),before)

    def test_navigation_paint_failure_rolls_back_only_the_unacknowledged_location(self):
        saved=self.editor.request({'type':'command','command':'captureNavigation'})['navigationCapture']
        self.editor.request({'type':'key','key':'End','ctrl':True})
        before=self.editor.model._state();undo=self.editor.model.undo[:]
        paint=self.editor.layout.frame
        self.editor.layout.frame=lambda *args,**kwargs:(_ for _ in ()).throw(RuntimeError('Injected navigation paint failure'))
        with self.assertRaisesRegex(RuntimeError,'navigation paint'):self.editor.request({'type':'command','command':'restoreNavigation','bookmark':saved})
        self.assertEqual(self.editor.model._state(),before);self.assertEqual(self.editor.model.undo,undo)
        self.editor.layout.frame=paint

if __name__=='__main__':unittest.main()
