"""Public native writer protocol regressions using synthetic Unicode documents."""
import base64
import io
import json
from pathlib import Path
import selectors
import subprocess
import tempfile
import unittest
import cairo
ROOT=Path(__file__).resolve().parents[1]
class EngineFixture(unittest.TestCase):
    def setUp(self):
        self.directory=tempfile.TemporaryDirectory()
        self.process=subprocess.Popen(['/usr/bin/python3',str(ROOT/'tools/native_editor_engine.py'),self.directory.name],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
    def tearDown(self):
        try:
            self.process.stdin.close();self.assertEqual(self.process.wait(timeout=15),0,'Editor helper crashed during shutdown')
        finally:self.process.stdout.close();self.directory.cleanup()
    def request(self,request):
        self.process.stdin.write(json.dumps(request)+'\n');self.process.stdin.flush()
        with selectors.DefaultSelector() as ready:
            ready.register(self.process.stdout,selectors.EVENT_READ)
            self.assertTrue(ready.select(30),'Native writer request timed out')
        response=json.loads(self.process.stdout.readline())
        self.assertTrue(response['ok'],response.get('error'));return response['frame']
    def dark_pixels(self,frame):
        surface=cairo.ImageSurface.create_from_png(io.BytesIO(base64.b64decode(frame['image'].split(',')[1])))
        self.assertEqual((surface.get_width(),surface.get_height()),(frame['width'],frame['height']))
        surface.flush();data=bytes(surface.get_data());stride=surface.get_stride();pixels=set()
        for y in range(surface.get_height()):
            for x in range(surface.get_width()):
                b,g,r,a=data[y*stride+x*4:y*stride+x*4+4]
                if (surface.get_format()==cairo.FORMAT_RGB24 or a>200) and max(r,g,b)<180:pixels.add((x,y))
        return pixels
    def scene(self,document):return {'schemaVersion':1,'title':'Synthetic test','document':document}
    def p(self,text,lang='ar',direction='rtl'):
        return {'type':'paragraph','attrs':{'dir':direction,'lang':lang},'content':[{'type':'text','text':text}]}

class NativeContractTests(EngineFixture):
    def test_omitted_paragraph_defaults_open_snapshot_and_undo_preserve_exact_source(self):
        text='Synthetic long project English Arabic مِ Chinese 中文. '*30
        document={'type':'doc','content':[{'type':'paragraph','content':[{'type':'text','text':text}]},
            {'type':'paragraph','attrs':{'lang':'ar'},'content':[{'type':'text','text':'مِمِمِ'}]},
            {'type':'paragraph','attrs':{'dir':'ltr'},'content':[{'type':'text','text':'中文 e\u0301 👩🏽‍🚀'}]}]}
        self.assertEqual(self.request({'type':'open','scene':self.scene(document)})['document'],document)
        self.assertEqual(self.request({'type':'snapshot'})['document'],document)
        self.request({'type':'key','key':'Home','ctrl':True})
        changed=self.request({'type':'text','text':'X'})['document']
        self.assertEqual(changed['content'][0]['content'][0]['text'],'X'+text)
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],document)
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],changed)
    def test_round_trip_gate_still_refuses_unrepresented_metadata(self):
        document={'type':'doc','content':[{'type':'paragraph','attrs':{'lang':'en','dir':'auto','unsupported':'retain'},'content':[{'type':'text','text':'مِ 中文'}]}]}
        self.process.stdin.write(json.dumps({'type':'open','scene':self.scene(document)})+'\n');self.process.stdin.flush()
        response=json.loads(self.process.stdout.readline());self.assertFalse(response['ok']);self.assertIn('Unsupported',response['error'])
    def test_empty_paragraph_content_and_attrs_are_retained_without_an_edit(self):
        document={'type':'doc','content':[{'type':'paragraph','attrs':{},'content':[]},{'type':'heading','content':[{'type':'text','text':'مِ 中文'}]}]}
        self.assertEqual(self.request({'type':'open','scene':self.scene(document)})['document'],document)
        self.assertEqual(self.request({'type':'refresh'})['document'],document)
        self.assertEqual(self.request({'type':'snapshot'})['document'],document)
        self.request({'type':'key','key':'Home','ctrl':True})
        edited=self.request({'type':'text','text':'X'})['document']
        self.assertEqual(edited['content'][0]['content'][0]['text'],'X')
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],document)
        self.process.stdin.close();self.assertEqual(self.process.wait(timeout=15),0);self.process.stdout.close()
        self.process=subprocess.Popen(['/usr/bin/python3',str(ROOT/'tools/native_editor_engine.py'),self.directory.name],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
        self.assertEqual(self.request({'type':'open','scene':self.scene(document)})['document'],document)
    def test_underline_combinations_round_trip_and_clear_preserves_paragraph_metadata(self):
        document={'type':'doc','content':[self.p('','ar','rtl')]}
        block=document['content'][0];block['attrs']['lineSpacing']=1.15;block['content']=[]
        for bits in range(8):
            node={'type':'text','text':f'{bits} مِ 中文 😀 '}
            marks=[{'type':name} for bit,name in [(1,'bold'),(2,'italic'),(4,'underline')] if bits&bit]
            if marks:node['marks']=marks
            block['content'].append(node)
        original=self.request({'type':'open','scene':self.scene(document)})
        self.assertEqual(original['document'],document)
        self.request({'type':'command','command':'selectAll'})
        cleared=self.request({'type':'command','command':'clearFormatting'})
        expected=json.loads(json.dumps(document));expected['content'][0]['content']=[{'type':'text','text':''.join(n['text'] for n in block['content'])}]
        self.assertEqual(cleared['document'],expected)
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],document)
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],expected)
        self.request({'type':'command','command':'selectAll'})
        underlined=self.request({'type':'command','command':'underline'})
        expected['content'][0]['content'][0]['marks']=[{'type':'underline'}]
        self.assertEqual(underlined['document'],expected)
        self.assertGreater(len(self.dark_pixels(underlined)),len(self.dark_pixels(cleared)))
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],cleared['document'])
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],expected)
        saved=self.request({'type':'snapshot'})['document']
        self.process.stdin.close();self.assertEqual(self.process.wait(timeout=15),0);self.process.stdout.close()
        self.process=subprocess.Popen(['/usr/bin/python3',str(ROOT/'tools/native_editor_engine.py'),self.directory.name],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
        self.assertEqual(self.request({'type':'open','scene':self.scene(saved)})['document'],saved)
    def test_native_double_and_triple_click_select_without_changing_document(self):
        document={'type':'doc','content':[self.p('alpha beta gamma','en','ltr'),self.p('مِمِمِ كلمة أخرى')]}
        self.request({'type':'open','scene':self.scene(document)})
        for last,word,paragraph in [(False,'alpha','alpha beta gamma'),(True,'أخرى','مِمِمِ كلمة أخرى')]:
            frame=self.request({'type':'key','key':'End' if last else 'Home','ctrl':True})
            frame=self.request({'type':'key','key':'ArrowLeft' if last else 'ArrowRight'})
            x,y,w,h=frame['cursor'];point={'x':x,'y':y+h//2}
            for count in [2,3]:
                self.request({'type':'mouse','phase':'down','buttons':True,'count':count,**point})
                selected=self.request({'type':'mouse','phase':'up','buttons':False,'count':count,**point})
                self.assertEqual(selected['selectedText'].strip(),word if count==2 else paragraph)
                self.assertEqual(selected['document'],document)
        for count in [0,4,True]:
            self.process.stdin.write(json.dumps({'type':'mouse','phase':'down','x':0,'y':0,'count':count})+'\n');self.process.stdin.flush()
            self.assertFalse(json.loads(self.process.stdout.readline())['ok'])
    def test_native_find_wrap_and_guarded_replace_preserve_exact_unicode_and_undo(self):
        query='مِ 中文 👩🏽‍🚀 e\u0301'
        text=query+' / '+query+' / é / literal.*'
        document={'type':'doc','content':[self.p(text,'en','ltr')]}
        document['content'][0]['content'][0]['marks']=[{'type':'underline'}]
        self.request({'type':'open','scene':self.scene(document)})
        self.request({'type':'key','key':'Home','ctrl':True})
        positions=[]
        for wrapped in [False,False,True]:
            found=self.request({'type':'command','command':'find','query':query})
            self.assertEqual(found['searchResult'],{'found':True,'wrapped':wrapped})
            self.assertEqual(found['selectedText'],query);self.assertEqual(found['document'],document);positions.append(found['selection'])
        self.assertNotEqual(positions[0],positions[1]);self.assertEqual(positions[0],positions[2])
        refused=self.request({'type':'command','command':'replace','query':'wrong','replacement':'x'})
        self.assertFalse(refused['searchResult']['replaced']);self.assertEqual(refused['document'],document)
        for bad_query,replacement in [(query,'\x00'),(query,'x'*10001)]:
            self.process.stdin.write(json.dumps({'type':'command','command':'replace','query':bad_query,'replacement':replacement})+'\n');self.process.stdin.flush()
            self.assertFalse(json.loads(self.process.stdout.readline())['ok']);self.assertEqual(self.request({'type':'snapshot'})['document'],document)
        changed=self.request({'type':'command','command':'replace','query':query,'replacement':'عَ 世界 😀'})['document']
        self.assertEqual(''.join(n['text'] for n in changed['content'][0]['content']),'عَ 世界 😀 / '+query+' / é / literal.*')
        self.assertEqual(changed['content'][0]['attrs'],document['content'][0]['attrs'])
        self.assertTrue(all(node.get('marks')==[{'type':'underline'}] for node in changed['content'][0]['content']))
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],document)
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],changed)
        literal=self.request({'type':'command','command':'find','query':'literal.*'})
        self.assertEqual(literal['selectedText'],'literal.*')
        missing=self.request({'type':'command','command':'find','query':'LITERAL.*'})
        self.assertFalse(missing['searchResult']['found']);self.assertEqual(missing['document'],changed)
        self.request({'type':'key','key':'End','ctrl':True})
        refused=self.request({'type':'command','command':'replace','query':query,'replacement':'x'})
        self.assertFalse(refused['searchResult']['replaced']);self.assertEqual(refused['document'],changed)
    def test_native_find_rejects_invalid_input_and_avoids_unicode_normalization(self):
        document={'type':'doc','content':[self.p('é مِ 😀','en','ltr')]}
        self.request({'type':'open','scene':self.scene(document)})
        for query in ['',True,'x'*1001,'a\n','\x00']:
            self.process.stdin.write(json.dumps({'type':'command','command':'find','query':query})+'\n');self.process.stdin.flush()
            self.assertFalse(json.loads(self.process.stdout.readline())['ok']);self.assertEqual(self.request({'type':'snapshot'})['document'],document)
        found=self.request({'type':'command','command':'find','query':'e\u0301'})
        self.assertFalse(found['searchResult']['found']);self.assertEqual(found['document'],document)
        self.request({'type':'key','key':'End','ctrl':True})
        edited=self.request({'type':'text','text':' new مِ'})['document']
        direct_undo=self.request({'type':'command','command':'undo'})['document']
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],edited)
        found=self.request({'type':'command','command':'find','query':'مِ'})
        self.assertEqual(found['selectedText'],'مِ')
        cancelled=self.request({'type':'key','key':'Escape'})
        self.assertEqual(cancelled['document'],edited);self.assertNotIn('searchResult',cancelled)
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],direct_undo)
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],edited)
    def test_explicit_line_spacing_preserves_mixed_paragraphs_and_undo(self):
        document={'type':'doc','content':[self.p('بِسْمِ اللَّهِ مِمِمِ'),self.p('English 中文','en','ltr')]}
        document['content'][0]['content'][0]['marks']=[{'type':'bold'}]
        self.request({'type':'open','scene':self.scene(document)})
        self.request({'type':'command','command':'selectAll'})
        for value in [1,1.15,1.5,2]:
            self.request({'type':'command','command':'selectAll'})
            changed=self.request({'type':'command','command':'lineSpacing','lineSpacing':value})['document']
            expected=json.loads(json.dumps(document))
            for block in expected['content']:block['attrs']['lineSpacing']=value
            self.assertEqual(changed,expected)
            self.assertEqual(self.request({'type':'command','command':'undo'})['document'],document)
            self.assertEqual(self.request({'type':'command','command':'redo'})['document'],expected)
            self.request({'type':'command','command':'undo'})
        # Reopening serializes the explicit Arabic 2x despite its legacy 2x default.
        self.request({'type':'command','command':'selectAll'})
        self.request({'type':'command','command':'lineSpacing','lineSpacing':2})
        saved=self.request({'type':'snapshot'})['document']
        self.process.stdin.close();self.assertEqual(self.process.wait(timeout=15),0);self.process.stdout.close()
        self.process=subprocess.Popen(['/usr/bin/python3',str(ROOT/'tools/native_editor_engine.py'),self.directory.name],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
        self.assertEqual(self.request({'type':'open','scene':self.scene(saved)})['document'],saved)
    def test_spacing_survives_language_change_and_invalid_input(self):
        document={'type':'doc','content':[self.p('مِمِمِ 中文')]}
        document['content'][0]['attrs']['lineSpacing']=1.15
        self.request({'type':'open','scene':self.scene(document)})
        changed=self.request({'type':'command','command':'blockStyle','language':'en','direction':'ltr','style':'heading'})['document']
        self.assertEqual(changed['content'][0]['attrs']['lineSpacing'],1.15)
        self.assertEqual(changed['content'][0]['content'],document['content'][0]['content'])
        for value in [True,'1',0,1.2,None]:
            self.process.stdin.write(json.dumps({'type':'command','command':'lineSpacing','lineSpacing':value})+'\n');self.process.stdin.flush()
            response=json.loads(self.process.stdout.readline());self.assertFalse(response['ok'])
            self.assertEqual(self.request({'type':'snapshot'})['document'],changed)
    def test_enter_inherits_selected_leading_without_changing_other_paragraphs(self):
        for text,lang,size in [('English quotation “yes”.','en',18),('يس مِ كلمات عربية','ar',23),('English يس مِ 中文','en',18)]:
            with self.subTest(lang=lang,text=text):
                document={'type':'doc','content':[self.p(text,lang,'rtl' if lang=='ar' else 'auto'),self.p('Unrelated paragraph','en','ltr')]}
                document['content'][1]['attrs']['lineSpacing']=2
                self.request({'type':'open','scene':self.scene(document)})
                self.request({'type':'key','key':'Home','ctrl':True})
                self.request({'type':'key','key':'End'})
                before=self.request({'type':'command','command':'lineSpacing','lineSpacing':1.15})
                entered=self.request({'type':'key','key':'Enter'})
                blocks=entered['document']['content']
                self.assertEqual([block['attrs']['lineSpacing'] for block in blocks],[1.15,1.15,2])
                self.assertEqual(blocks[0]['content'],document['content'][0]['content'])
                self.assertEqual(blocks[2],document['content'][1])
                # The empty new paragraph retains the selected size-based row
                # spacing. Mixed fallback baselines may add a few pixels.
                advance=(entered['cursor'][1]-before['cursor'][1])/15
                self.assertGreaterEqual(advance,size*1.15-1)
                self.assertLessEqual(advance,size*1.15+(6 if lang=='en' else 1))
                self.assertEqual(self.request({'type':'command','command':'undo'})['document'],before['document'])
    def test_spacing_changes_only_current_paragraph_and_retains_arabic_ink(self):
        document={'type':'doc','content':[self.p('بِسْمِ اللَّهِ مِمِمِ'),self.p('Another paragraph','en','ltr')]}
        before=self.request({'type':'open','scene':self.scene(document)})
        self.request({'type':'key','key':'Home','ctrl':True})
        after=self.request({'type':'command','command':'lineSpacing','lineSpacing':1})
        expected=json.loads(json.dumps(document));expected['content'][0]['attrs']['lineSpacing']=1
        self.assertEqual(after['document'],expected)
        # Isolate the paragraph for the ink check. Compact leading no longer
        # guarantees a large blank gap before the next paragraph's ink.
        isolated={'type':'doc','content':[document['content'][0]]}
        before=self.request({'type':'open','scene':self.scene(isolated)})
        after=self.request({'type':'command','command':'lineSpacing','lineSpacing':1})
        def first_ink(frame):
            pixels=self.dark_pixels(frame);left=min(x for x,_ in pixels);top=min(y for _,y in pixels)
            return {(x-left,y-top) for x,y in pixels}
        # Subpixel baseline changes may move an antialiased boundary by one
        # pixel; every dark glyph pixel must retain a neighbouring counterpart.
        old,new=first_ink(before),first_ink(after)
        def expanded(ink):return {(x+dx,y+dy) for x,y in ink for dx in (-1,0,1) for dy in (-1,0,1)}
        self.assertTrue(old<=expanded(new));self.assertTrue(new<=expanded(old))
    def test_mixed_explicit_spacing_survives_selected_language_change(self):
        document={'type':'doc','content':[self.p('مِمِمِ'),self.p('中文','zh-Hans','ltr')]}
        document['content'][0]['attrs']['lineSpacing']=1
        document['content'][1]['attrs']['lineSpacing']=2
        self.request({'type':'open','scene':self.scene(document)})
        self.request({'type':'command','command':'selectAll'})
        changed=self.request({'type':'command','command':'blockStyle','language':'ar','direction':'rtl','style':'paragraph'})['document']
        self.assertEqual([p['attrs']['lineSpacing'] for p in changed['content']],[1,2])
        self.assertEqual([p['content'] for p in changed['content']],[p['content'] for p in document['content']])
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],document)
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],changed)
    def test_vertical_arrows_move_in_visual_direction_and_selection_preserves_text(self):
        document={'type':'doc','content':[
            self.p('English line one','en','ltr'),self.p('English line two','en','ltr'),
            self.p('سلام مِمِمِ في السطر الأول'),self.p('سلام مِمِمِ في السطر الثاني'),
            self.p('中文第一行','zh-Hans','ltr'),self.p('中文第二行','zh-Hans','ltr')]}
        self.request({'type':'open','scene':self.scene(document)})
        first=self.request({'type':'key','key':'Home','ctrl':True})
        down=self.request({'type':'key','key':'ArrowDown'})
        self.assertGreater(down['cursor'][1],first['cursor'][1])
        up=self.request({'type':'key','key':'ArrowUp'})
        self.assertLess(up['cursor'][1],down['cursor'][1])
        self.assertEqual(up['cursor'][1],first['cursor'][1])
        self.request({'type':'key','key':'End','ctrl':True})
        rows=[]
        for _ in range(5):
            before=self.request({'type':'refresh'})
            after=self.request({'type':'key','key':'ArrowUp'})
            self.assertLess(after['cursor'][1],before['cursor'][1]);rows.append(after['cursor'][1])
        for y in reversed(rows[:-1]):
            after=self.request({'type':'key','key':'ArrowDown'})
            self.assertEqual(after['cursor'][1],y)
        self.request({'type':'key','key':'Home','ctrl':True})
        selected=self.request({'type':'key','key':'ArrowDown','shift':True})
        self.assertTrue(selected['selection']);self.assertTrue(selected['selectedText'])
        self.assertEqual(self.request({'type':'snapshot'})['document'],document)
    def test_kasra_keeps_base_and_cursor_baseline_and_undo_retains_exact_text(self):
        document={'type':'doc','content':[self.p('ممم')]}
        before=self.request({'type':'open','scene':self.scene(document)})
        self.request({'type':'command','command':'selectAll'})
        unmarked=self.request({'type':'text','text':'ممم'})
        marked=self.request({'type':'text','text':'ِ'})
        self.assertEqual(marked['document']['content'][0]['content'][0]['text'],'مممِ')
        self.assertEqual(unmarked['cursor'][1],marked['cursor'][1])
        # The base word's dark pixels stay at the same coordinates; kasra adds ink.
        base=self.dark_pixels(unmarked); with_mark=self.dark_pixels(marked)
        self.assertGreater(len(base),20)
        self.assertGreater(max(y for _,y in with_mark),max(y for _,y in base))
        self.assertEqual(min(y for _,y in base),min(y for _,y in with_mark))
        horizontal_tolerance={(x+dx,y) for x,y in with_mark for dx in [-1,0,1]}
        self.assertGreater(len(base & horizontal_tolerance)/len(base),0.98)
        self.assertEqual(self.request({'type':'snapshot'})['document']['content'][0]['content'][0]['text'],'مممِ')
        undo=self.request({'type':'command','command':'undo'})
        self.assertEqual(undo['document']['content'][0]['content'][0]['text'],'ممم')
        redo=self.request({'type':'command','command':'redo'})
        self.assertEqual(redo['document'],marked['document'])
        image=base64.b64decode(marked['image'].split(',')[1]);self.assertTrue(image.startswith(b'\x89PNG'))
    def test_exact_multilingual_formatted_round_trip(self):
        document={'type':'doc','content':[
            self.p('English  spaces \t end ','en','auto'),self.p('بِسْمِ اللَّهِ مِمِمِ'),
            self.p('中文 𠀀','zh-Hans','auto'),self.p('故事從這裡開始。','zh-Hant','auto'),
            {'type':'heading','attrs':{'level':2,'dir':'auto','lang':'und'},'content':[{'type':'text','text':'\ue000 👩🏽‍🚀','marks':[{'type':'bold'},{'type':'italic'}]}]},
            {'type':'paragraph','attrs':{'dir':'auto','lang':'en'},'content':[{'type':'text','text':'First','marks':[{'type':'italic'}]},{'type':'hardBreak'},{'type':'text','text':'Second'}]},
            {'type':'paragraph','attrs':{'dir':'auto','lang':'en'}}]}
        frame=self.request({'type':'open','scene':self.scene(document)})
        self.assertEqual(frame['document'],document)
    def test_typing_formatting_enter_and_delete(self):
        scene=self.scene({'type':'doc','content':[self.p('Start','en','ltr')]})
        self.request({'type':'open','scene':scene})
        self.request({'type':'command','command':'selectAll'})
        self.request({'type':'text','text':'سلام'})
        self.request({'type':'command','command':'bold'})
        bold=self.request({'type':'text','text':'مِ'})
        self.assertEqual(bold['document']['content'][0]['content'][-1]['marks'],[{'type':'bold'}])
        self.request({'type':'key','key':'Enter'})
        added=self.request({'type':'text','text':'中文'})
        self.assertEqual(len(added['document']['content']),2)
        deleted=self.request({'type':'key','key':'Backspace'})
        self.assertEqual(deleted['document']['content'][1]['content'][0]['text'],'中')
    def test_page_matches_host_width_without_extra_paper(self):
        document={'type':'doc','content':[self.p('سلام مِ') ]}
        frame=self.request({'type':'open','scene':self.scene(document),'width':420})
        self.assertAlmostEqual(frame['width'],420,delta=1)
        self.assertLess(frame['totalHeight'],300)
        self.assertLess((frame['cursor'][1]-frame['origin'][1])/15,40)
        self.assertEqual(frame['document'],document)
        # Input in a blank paragraph can extend the surface below the last ink.
        self.request({'type':'command','command':'selectAll'})
        self.request({'type':'text','text':'مِ'})
        for _ in range(6):frame=self.request({'type':'key','key':'Enter'})
        self.assertGreaterEqual(frame['totalHeight'],(frame['cursor'][1]+frame['cursor'][3]-frame['origin'][1])/15)
    def test_rapid_input_keeps_every_character_in_order(self):
        self.request({'type':'open','scene':self.scene({'type':'doc','content':[self.p('replace')]})})
        self.request({'type':'command','command':'selectAll'})
        text='سلام مِمِمِ 中文 English 👩🏽‍🚀 '+ 'مِ'*10
        # Queue input without waiting for each response, like a fast typist.
        for char in text:self.process.stdin.write(json.dumps({'type':'text','text':char})+'\n')
        self.process.stdin.flush();expected=''
        for char in text:
            expected+=char;response=json.loads(self.process.stdout.readline())
            self.assertTrue(response['ok'],response.get('error'))
            self.assertEqual(response['frame']['document']['content'][0]['content'][0]['text'],expected)
    def test_longer_paginated_scene_round_trips_and_edits_exactly(self):
        blocks=[self.p(('سلام محمد مِمِمِ '+str(i)+' ')*20) for i in range(20)]
        document={'type':'doc','content':blocks}
        frame=self.request({'type':'open','scene':self.scene(document),'width':748})
        self.assertEqual(frame['document'],document)
        self.assertGreater(frame['totalHeight'],1000)
        self.request({'type':'key','key':'End','ctrl':True})
        frame=self.request({'type':'text','text':' سلام مِ'})
        expected=json.loads(json.dumps(document));expected['content'][-1]['content'][0]['text']+=' سلام مِ'
        self.assertEqual(frame['document'],expected)
    def test_long_draft_caret_and_bitmap_follow_navigation_and_typing(self):
        document={'type':'doc','content':[self.p(('Long mixed text مِمِ 中文. '*1700),'en','auto')]}
        frame=self.request({'type':'open','scene':self.scene(document),'width':748})
        self.assertGreater(frame['totalHeight'],10000)
        end=self.request({'type':'key','key':'End','ctrl':True,'preview':True,'top':0,'viewportHeight':480,'followCursor':True})
        def covered(frame):
            top=(frame['cursor'][1]-frame['origin'][1])/15
            bottom=top+frame['cursor'][3]/15
            self.assertGreaterEqual(top,frame['top'])
            self.assertLessEqual(bottom,frame['top']+frame['height'])
        covered(end);self.assertGreater(end['top'],10000)
        for text in [' سلام','ِ','中文','👩🏽‍🚀']:
            frame=self.request({'type':'text','text':text,'preview':True,'top':end['top'],'viewportHeight':480,'followCursor':True})
            covered(frame)
        saved=self.request({'type':'snapshot','top':frame['top'],'viewportHeight':480})
        expected=json.loads(json.dumps(document));expected['content'][0]['content'][0]['text']+=' سلامِ中文👩🏽‍🚀'
        self.assertEqual(saved['document'],expected)
        manual=self.request({'type':'refresh','preview':True,'top':0,'viewportHeight':900,'followCursor':False})
        self.assertEqual(manual['top'],0);self.assertEqual(manual['height'],900)
        home=self.request({'type':'key','key':'Home','ctrl':True,'preview':True,'top':saved['top'],'viewportHeight':480,'followCursor':True})
        covered(home);self.assertEqual(home['top'],0)
    def test_invalid_viewport_is_refused_before_editing(self):
        document={'type':'doc','content':[self.p('Preserved','en','ltr')]}
        self.request({'type':'open','scene':self.scene(document)})
        for height in [-1,0,5000,'620']:
            self.process.stdin.write(json.dumps({'type':'text','text':'unsafe','viewportHeight':height})+'\n');self.process.stdin.flush()
            response=json.loads(self.process.stdout.readline());self.assertFalse(response['ok']);self.assertIn('viewport',response['error'])
        self.assertEqual(self.request({'type':'snapshot'})['document'],document)
    def test_preview_typing_and_checkpoint_preserve_kasra_without_per_key_export(self):
        scene=self.scene({'type':'doc','content':[self.p('ممم')]})
        self.request({'type':'open','scene':scene})
        self.request({'type':'command','command':'selectAll','preview':True})
        unmarked=self.request({'type':'text','text':'ممم','preview':True})
        marked=self.request({'type':'text','text':'ِ','preview':True})
        self.assertNotIn('document',marked)
        self.assertEqual(unmarked['cursor'][1],marked['cursor'][1])
        base=self.dark_pixels(unmarked);ink=self.dark_pixels(marked)
        self.assertEqual(min(y for _,y in base),min(y for _,y in ink))
        self.assertGreater(max(y for _,y in ink),max(y for _,y in base))
        snapshot=self.request({'type':'snapshot'})
        self.assertEqual(snapshot['document']['content'][0]['content'][0]['text'],'مممِ')
        self.request({'type':'command','command':'undo','preview':True})
        self.assertEqual(self.request({'type':'snapshot'})['document']['content'][0]['content'][0]['text'],'ممم')
    def test_control_character_refused_without_touching_source(self):
        scene=self.scene({'type':'doc','content':[self.p('source\u0000text')]})
        self.process.stdin.write(json.dumps({'type':'open','scene':scene})+'\n');self.process.stdin.flush()
        result=json.loads(self.process.stdout.readline())
        self.assertFalse(result['ok']);self.assertIn('preserved',result['error']);self.assertEqual(scene['document']['content'][0]['content'][0]['text'],'source\u0000text')
    def test_direction_controls_preserve_text_formatting_languages_and_headings(self):
        document={'type':'doc','content':[
            {'type':'heading','attrs':{'level':2,'dir':'rtl','lang':'ar'},'content':[{'type':'text','text':'سلام مِ 123 English','marks':[{'type':'bold'}]}]},
            self.p('English 中文 مِ','en','ltr'),self.p('中文 👩🏽‍🚀','zh-Hans','auto')]}
        self.request({'type':'open','scene':self.scene(document)})
        self.request({'type':'command','command':'ltr','preview':True})
        changed=json.loads(json.dumps(document));changed['content'][0]['attrs']['dir']='ltr'
        self.assertEqual(self.request({'type':'snapshot'})['document'],changed)
        self.request({'type':'command','command':'undo','preview':True})
        self.assertEqual(self.request({'type':'snapshot'})['document'],document)
        self.request({'type':'command','command':'redo','preview':True})
        self.assertEqual(self.request({'type':'snapshot'})['document'],changed)
        self.request({'type':'command','command':'selectAll','preview':True})
        self.request({'type':'command','command':'rtl','preview':True})
        selected=json.loads(json.dumps(document))
        for block in selected['content']:block['attrs']['dir']='rtl'
        self.assertEqual(self.request({'type':'snapshot'})['document'],selected)
        self.request({'type':'command','command':'undo','preview':True})
        self.assertEqual(self.request({'type':'snapshot'})['document'],changed)

class ParagraphStyleTests(EngineFixture):
    def test_heading_language_direction_preserve_kasra_cjk_marks_and_undo(self):
        block=self.p('سلام مِ 中文 👩🏽‍🚀','ar','rtl')
        block['content'][0]['marks']=[{'type':'bold'},{'type':'italic'}]
        document={'type':'doc','content':[block]}
        self.request({'type':'open','scene':self.scene(document)})
        self.request({'type':'command','command':'selectAll'})
        heading=self.request({'type':'command','command':'blockStyle','style':'heading','language':'ar','direction':'rtl'})
        expected={'type':'heading','attrs':{'level':2,'dir':'rtl','lang':'ar'},'content':block['content']}
        self.assertEqual(heading['document']['content'],[expected])
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],document)
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],heading['document'])
        paragraph=self.request({'type':'command','command':'blockStyle','style':'paragraph','language':'zh-Hant','direction':'ltr'})
        expected={'type':'paragraph','attrs':{'dir':'ltr','lang':'zh-Hant'},'content':block['content']}
        self.assertEqual(paragraph['document']['content'],[expected])
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],heading['document'])
    def test_explicit_language_supports_every_stored_language_without_conversion(self):
        document={'type':'doc','content':[self.p('繁體 简体 e\u0301 مِ \ue000','und','auto')]}
        self.request({'type':'open','scene':self.scene(document)})
        for language in ['en','ar','zh-Hans','zh-Hant','und']:
            updated=self.request({'type':'command','command':'blockStyle','style':'paragraph','language':language,'direction':'auto'})
            self.assertEqual(updated['document']['content'][0]['attrs'],{'lang':language,'dir':'auto'})
            self.assertEqual(updated['document']['content'][0]['content'],document['content'][0]['content'])
    def test_incomplete_block_style_is_rejected_without_editing(self):
        document={'type':'doc','content':[self.p('Preserved مِ')]}
        self.request({'type':'open','scene':self.scene(document)})
        self.process.stdin.write(json.dumps({'type':'command','command':'blockStyle','style':'heading'})+'\n');self.process.stdin.flush()
        response=json.loads(self.process.stdout.readline());self.assertFalse(response['ok']);self.assertIn('explicit',response['error'])
        self.assertEqual(self.request({'type':'snapshot'})['document'],document)
    def test_native_style_and_text_edits_share_ordered_undo_redo_history(self):
        document={'type':'doc','content':[self.p('Start مِ','en','ltr')]}
        self.request({'type':'open','scene':self.scene(document)})
        heading=self.request({'type':'command','command':'blockStyle','style':'heading','language':'en','direction':'ltr'})
        self.request({'type':'key','key':'End'})
        typed=self.request({'type':'text','text':'中文'})
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],heading['document'])
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],document)
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],heading['document'])
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],typed['document'])
    def test_selected_mixed_paragraphs_change_properties_preserving_all_runs(self):
        first=self.p('English مِ','en','ltr');first['content'][0]['marks']=[{'type':'bold'}]
        second=self.p('中文 👩🏽‍🚀','zh-Hans','auto');second['content'][0]['marks']=[{'type':'italic'}]
        document={'type':'doc','content':[first,second]}
        self.request({'type':'open','scene':self.scene(document)})
        self.request({'type':'command','command':'selectAll'})
        actual=self.request({'type':'command','command':'blockStyle','style':'heading','language':'ar','direction':'rtl'})
        expected=json.loads(json.dumps(document))
        for block in expected['content']:block['type']='heading';block['attrs']={'lang':'ar','dir':'rtl','level':2}
        self.assertEqual(actual['document'],expected)
        self.assertEqual(self.request({'type':'command','command':'undo'})['document'],document)
        self.assertEqual(self.request({'type':'command','command':'redo'})['document'],expected)

class ProjectFontTests(EngineFixture):
    def test_private_project_font_registers_and_preserves_exact_unicode_json(self):
        import hashlib
        # An installed font copied only into an ephemeral session verifies the
        # loading mechanism; it is not evidence for an absent invented font.
        source=Path('/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf')
        if not source.exists(): self.skipTest('Installed synthetic font-loading fixture unavailable')
        target=Path(self.directory.name)/'fonts'/'fixture.ttf';target.parent.mkdir();target.write_bytes(source.read_bytes())
        family=subprocess.check_output(['fc-scan','--format','%{family[0]}\n',str(target)],text=True).splitlines()[0]
        document={'type':'doc','content':[self.p('Exact \ue000 \U000f0000 e\u0301 👩🏽‍🚀','und','ltr')]}
        frame=self.request({'type':'open','scene':self.scene(document),'writingSystem':{'definitionId':'synthetic','mappingVersion':1,'direction':'ltr','fontFamily':family},'fonts':[{'path':str(target),'family':family,'checksum':hashlib.sha256(target.read_bytes()).hexdigest()}]})
        self.assertEqual(frame['document'],document)
        self.assertTrue(frame['image'].startswith('data:image/png;base64,'))
        self.assertFalse(list(Path(self.directory.name).glob('*.fodt')))
        self.request({'type':'command','command':'selectAll'})
        changed=self.request({'type':'text','text':'\ue000\U000f0000'})
        self.assertEqual(changed['document']['content'][0]['content'][0]['text'],'\ue000\U000f0000')
    def test_project_font_outside_session_is_rejected_before_open(self):
        self.process.stdin.write(json.dumps({'type':'open','scene':self.scene({'type':'doc','content':[self.p('Preserved')]}),'fonts':[{'path':'/etc/passwd','family':'Wrong','checksum':'0'*64}]})+'\n');self.process.stdin.flush()
        response=json.loads(self.process.stdout.readline());self.assertFalse(response['ok']);self.assertIn('outside',response['error'])
    def test_project_font_checksum_is_verified_before_native_font_parsing(self):
        target=Path(self.directory.name)/'fonts'/'bad.ttf';target.parent.mkdir();target.write_bytes(b'\x00\x01\x00\x00'+b'\0'*20)
        self.process.stdin.write(json.dumps({'type':'open','scene':self.scene({'type':'doc','content':[self.p('Preserved')]}),'fonts':[{'path':str(target),'family':'Wrong','checksum':'0'*64}]})+'\n');self.process.stdin.flush()
        response=json.loads(self.process.stdout.readline());self.assertFalse(response['ok']);self.assertIn('checksum',response['error'])

if __name__=='__main__':unittest.main()
