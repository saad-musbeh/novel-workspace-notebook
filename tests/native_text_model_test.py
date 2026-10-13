"""Synthetic exact-document tests for our own editing model; no office runtime."""
import random,sys,unittest,unicodedata
from copy import deepcopy
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from native_text_model import TextModel,validate_document

def paragraph(text,attrs=None,marks=None):
    block={'type':'paragraph','content':[{'type':'text','text':text}]}
    if attrs is not None:block['attrs']=attrs
    if marks:block['content'][0]['marks']=[{'type':mark} for mark in marks]
    return block
def document(*blocks):return {'type':'doc','content':list(blocks)}

class ModelTests(unittest.TestCase):
    def test_exact_original_omissions_empty_runs_and_snapshots(self):
        originals=[{'type':'doc'},document(),document({'type':'paragraph','attrs':{},'content':[]}),document(paragraph('مِ e\u0301 👩🏽‍🚀'),{'type':'heading','content':[{'type':'text','text':'中文'}]}),document({'type':'paragraph','content':[{'type':'text','text':'A'},{'type':'text','text':'B','marks':[]}]})]
        for original in originals:
            model=TextModel(original)
            self.assertEqual(model.document_snapshot(),original)
            model.move_to((0,0));model.insert('X');self.assertTrue(model.history())
            self.assertEqual(model.document_snapshot(),original)
            self.assertEqual(original,deepcopy(original))
    def test_grapheme_navigation_and_delete_preserve_arabic_cjk_emoji(self):
        clusters=['مِ','e\u0301','👩🏽‍🚀','🇸🇦','𠀀','\ue000']
        for cluster in clusters:
            original=document(paragraph('A'+cluster+'Z'))
            model=TextModel(original);model.move_to((0,1+len(cluster)))
            model.delete();self.assertEqual(model.flat_text(),'AZ')
            model.history();self.assertEqual(model.document_snapshot(),original)
            model.move_to((0,1));model.delete(True);self.assertEqual(model.flat_text(),'AZ')
            model.history();model.move_to((0,1));model.edit({'type':'key','key':'ArrowRight'})
            self.assertEqual(model.cursor,(0,1+len(cluster)))
    def test_multiblock_selection_insert_and_hard_break_undo(self):
        original=document(paragraph('first مِ',{'lang':'ar','dir':'rtl','lineSpacing':1.15},['underline']),paragraph('middle 中文'),paragraph('last 😀'))
        model=TextModel(original);model.anchor=(0,6);model.cursor=(2,5)
        expected=model.selection_text();self.assertEqual(expected,'مِ\nmiddle 中文\nlast ')
        model.insert('عَ\n𠀀');self.assertEqual(model.flat_text(),'first عَ\n𠀀😀')
        self.assertEqual(model.block_attrs(1),{'lang':'ar','dir':'rtl','lineSpacing':1.15})
        changed=model.document_snapshot();model.history();self.assertEqual(model.document_snapshot(),original)
        model.history(True);self.assertEqual(model.document_snapshot(),changed)
        model.insert_break();self.assertIn({'type':'hardBreak'},model.blocks[1]['content']);model.history();self.assertEqual(model.document_snapshot(),changed)
    def test_copy_on_write_preserves_unchanged_paragraph_objects(self):
        model=TextModel(document(*(paragraph(f'Paragraph {i} 中文') for i in range(1000))))
        before=model.blocks;model.move_to((501,5));model.insert('مِ')
        self.assertIsNot(before[501],model.blocks[501]);self.assertTrue(all(before[i] is model.blocks[i] for i in range(1000) if i!=501))
        model.history();self.assertIs(model.blocks,before)
    def test_formatting_exact_partial_selection_typing_and_clear(self):
        original=document(paragraph('A مِ 中文 😀 Z',{'dir':'rtl','lang':'ar','lineSpacing':1},['bold','italic','underline']))
        model=TextModel(original);model.anchor=(0,2);model.cursor=(0,len(model.block_text(0))-2)
        model.character_format(clear=True);cleared=model.document_snapshot()
        self.assertEqual(model.block_attrs(0),original['content'][0]['attrs'])
        self.assertEqual(model.flat_text(),'A مِ 中文 😀 Z')
        self.assertTrue(any('marks' not in node for node in model.blocks[0]['content']))
        model.history();self.assertEqual(model.document_snapshot(),original)
        model.history(True);self.assertEqual(model.document_snapshot(),cleared)
        model.move_to((0,4));model.character_format('underline');model.insert('X')
        node=next(node for node in model.blocks[0]['content'] if 'X' in node.get('text',''))
        self.assertIn({'type':'underline'},node['marks'])
    def test_paragraph_commands_preserve_each_spacing_and_marks(self):
        model=TextModel(document(paragraph('مِ',{'lang':'ar','dir':'rtl','lineSpacing':1},['underline']),paragraph('中文',{'lang':'zh-Hans','dir':'ltr','lineSpacing':2},['italic'])))
        original=model.document_snapshot();model.edit({'type':'command','command':'selectAll'})
        model.edit({'type':'command','command':'blockStyle','language':'en','direction':'auto','style':'heading'})
        self.assertEqual([model.block_attrs(i)['lineSpacing'] for i in range(2)],[1,2])
        self.assertEqual(model.blocks[0]['content'],original['content'][0]['content'])
        self.assertTrue(all(block['type']=='heading' for block in model.blocks));model.history();self.assertEqual(model.document_snapshot(),original)
        for value in [True,0,1.2,'1']:
            with self.assertRaises(ValueError):model.edit({'type':'command','command':'lineSpacing','lineSpacing':value})
            self.assertEqual(model.document_snapshot(),original)
    def test_literal_find_wrap_guard_replace_and_undo(self):
        query='مِ 中文 👩🏽‍🚀 e\u0301';original=document(paragraph(query+' / '+query+' / é / .*',None,['underline']))
        model=TextModel(original)
        for wrapped in [False,False,True]:
            self.assertEqual(model.find(query),{'found':True,'wrapped':wrapped});self.assertEqual(model.selection_text(),query)
        changed,result=model.replace_selection('wrong','X');self.assertFalse(changed);self.assertFalse(result['replaced']);self.assertEqual(model.document_snapshot(),original)
        model.replace_selection(query,'عَ 世界');self.assertEqual(model.flat_text(),'عَ 世界 / '+query+' / é / .*')
        self.assertTrue(all(node.get('marks')==[{'type':'underline'}] for node in model.blocks[0]['content']))
        model.history();self.assertEqual(model.document_snapshot(),original)
        self.assertFalse(model.find('Mِ')['found']);self.assertFalse(model.find('E\u0301')['found'])
        self.assertTrue(model.find('.*')['found'])
    def test_unicode_toggle_valid_scalars_exact_reversal_and_undo(self):
        for scalar in [0x41,0x645,0x20000,0x1F600,0xE000,0x650,0x10FFFF]:
            code=f'{scalar:04X}';original=document(paragraph('邻'+code))
            model=TextModel(original);model.move_to((0,len(model.block_text(0))))
            changed,result=model.unicode_toggle();self.assertTrue(changed);self.assertTrue(result['converted'])
            self.assertEqual(model.flat_text(),'邻'+chr(scalar));self.assertEqual(model.selection_text(),chr(scalar))
            model.unicode_toggle();self.assertEqual(model.flat_text(),'邻'+code);self.assertEqual(model.selection_text(),code)
            model.history();self.assertEqual(model.flat_text(),'邻'+chr(scalar));model.history();self.assertEqual(model.document_snapshot(),original)
        model=TextModel(document(paragraph('A0041B')));model.anchor=(0,1);model.cursor=(0,5);model.unicode_toggle();model.unicode_toggle();self.assertEqual(model.flat_text(),'A0041B')
    def test_unicode_toggle_invalid_values_are_nonfatal_and_keep_undo(self):
        for code in ['D800','DFFF','110000','1234567','0000','001B']:
            original=document(paragraph(code));model=TextModel(original);model.move_to((0,len(code)));changed,result=model.unicode_toggle()
            self.assertFalse(changed);self.assertFalse(result['converted']);self.assertEqual(model.document_snapshot(),original);self.assertEqual(model.undo,[])
    def test_every_unicode_letter_in_runtime_database_survives_input_snapshot_and_undo(self):
        letters=''.join(chr(value) for value in range(0x110000) if unicodedata.category(chr(value)).startswith('L'))
        self.assertGreater(len(letters),100_000)
        original=document(paragraph(''))
        model=TextModel(original);model.insert(letters)
        self.assertEqual(model.flat_text(),letters)
        self.assertEqual(model.document_snapshot()['content'][0]['content'][0]['text'],letters)
        model.history();self.assertEqual(model.document_snapshot(),original)
        model.history(True);self.assertEqual(model.flat_text(),letters)
    def test_replacing_a_formatted_run_retains_selected_style_at_run_boundary(self):
        original=document({'type':'paragraph','content':[{'type':'text','text':'plain '},{'type':'text','text':'مِ 中文','marks':[{'type':'bold'},{'type':'underline'}]},{'type':'text','text':' tail'}]})
        model=TextModel(original);model.find('مِ 中文');model.replace_selection('مِ 中文','عَ 世界')
        replacement=next(node for node in model.blocks[0]['content'] if 'عَ' in node.get('text',''))
        self.assertEqual(replacement['marks'],[{'type':'bold'},{'type':'underline'}]);self.assertEqual(model.flat_text(),'plain عَ 世界 tail')
        model.history();self.assertEqual(model.document_snapshot(),original)
    def test_unsupported_shapes_fail_before_editor_creation(self):
        for original in [document({'type':'image'}),document(paragraph('x',{'unknown':'retain'})),document(paragraph('x',{'lineSpacing':True})),document(paragraph('x',None,['strike'])),document(paragraph('\x00'))]:
            with self.assertRaises(ValueError):TextModel(original)
    def test_random_selected_replacements_match_exact_string_reference(self):
        randomizer=random.Random(926);model=TextModel(document(paragraph('مِ alpha\n中文 😀'),paragraph('second')));reference=model.flat_text()
        for _ in range(300):
            left=randomizer.randrange(len(reference)+1);right=randomizer.randrange(left,len(reference)+1);replacement=randomizer.choice(['','X','مِ','👩🏽‍🚀','\n','A\n中文'])
            model.anchor=model.from_flat(left);model.cursor=model.from_flat(right);old=model.document_snapshot()
            model.insert(replacement);expected=reference[:left]+replacement+reference[right:]
            self.assertEqual(model.flat_text(),expected)
            if old!=model.document_snapshot():model.history();self.assertEqual(model.document_snapshot(),old);model.history(True)
            reference=expected
    def test_undo_is_bounded_and_reports_trim_without_affecting_current_text(self):
        model=TextModel(document(paragraph('')))
        for _ in range(600):model.insert('X')
        self.assertEqual(model.flat_text(),'X'*600);self.assertEqual(len(model.undo),512);self.assertEqual(model.states()['undoTrimmed'],'true')
        for _ in range(512):model.history()
        self.assertEqual(model.flat_text(),'X'*88);self.assertFalse(model.history())
    def test_large_paragraph_versions_never_accumulate_grapheme_or_text_caches(self):
        model=TextModel(document(paragraph('word مِ 中文 😀 '*4000),paragraph('Unchanged neighbor')))
        neighbor=model.blocks[1];model.block_text(1);model.boundaries(1)
        for _ in range(160):
            model.boundaries(0);model.block_text(0);model.move_to((0,len(model.block_text(0))));model.insert('X')
            self.assertLessEqual(len(model._texts),2);self.assertLessEqual(len(model._graphemes),2)
            self.assertTrue(all(item[0] is model.blocks[0] or item[0] is neighbor for item in model._texts.values()))
        self.assertIs(model.blocks[1],neighbor);self.assertLessEqual(model.undo_bytes,64*1024*1024)
        model.history();model.boundaries(0);self.assertLessEqual(len(model._graphemes),2)
    def test_document_size_guard_is_incremental_and_rolls_back_before_history(self):
        model=TextModel(document(paragraph('مِ'),paragraph('stable')));model.max_document_bytes=model.wire_size()+5
        original=model.document_snapshot();before=model._state()
        with self.assertRaisesRegex(ValueError,'2 MB'):model.insert('中文😀')
        self.assertEqual(model._state(),before);self.assertEqual(model.document_snapshot(),original);self.assertEqual(model.undo,[])
        self.assertLessEqual(len(model._wire_sizes),2)

if __name__=='__main__':unittest.main()
