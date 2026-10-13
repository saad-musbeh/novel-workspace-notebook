"""Real Pango/Cairo checks, using synthetic manuscripts only."""
import base64
from io import BytesIO
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import cairo
from native_text_model import TextModel
from native_text_layout import NativeLayout, PangoCairo, PADDING, byte_offsets, codepoint_index


def model(*texts, attrs=None):
    return TextModel({'type':'doc','content':[{'type':'paragraph','attrs':attrs or {},'content':[{'type':'text','text':text}]} for text in texts]})


def pixels(frame):
    surface=cairo.ImageSurface.create_from_png(BytesIO(base64.b64decode(frame['image'].split(',')[1])))
    return bytes(surface.get_data())


class NativeTextLayoutTests(unittest.TestCase):
    def test_utf8_indices_and_cluster_trailing(self):
        text='Aمِ😀中'; offsets=byte_offsets(text)
        self.assertEqual(offsets,[0,1,3,5,9,12])
        self.assertEqual(codepoint_index(offsets,1,2),3)
        self.assertEqual(codepoint_index(offsets,9),4)

    def test_line_renderer_matches_full_layout_for_bidi_wrap_and_marks(self):
        for text,attrs in [('Hello wrapped words '*20,{}),('يس مِ العربية '*15,{'dir':'rtl','lang':'ar'}),('sfa يس 中文 مِ '*20,{})]:
            with self.subTest(text=text[:10]):
                layout=NativeLayout(model(text,attrs=attrs),340)
                actual=layout.frame(height=300)
                surface=cairo.ImageSurface(cairo.FORMAT_ARGB32,340,300);ctx=cairo.Context(surface)
                ctx.set_source_rgb(1,254/255,249/255);ctx.paint();ctx.set_source_rgb(62/255,73/255,64/255)
                paragraph=layout.paragraphs[0]
                # Reference the whole Pango layout: compact logical line boxes
                # can be smaller than Arabic ink, so per-row clips lose marks.
                ctx.move_to(PADDING,paragraph.y)
                PangoCairo.show_layout(ctx,paragraph.layout)
                self.assertEqual(pixels(actual),bytes(surface.get_data()))

    def test_incremental_arabic_matches_loaded_exactly(self):
        for text in ('يس','sfa يس','مِ العربية 中文'):
            incremental=model('');layout=NativeLayout(incremental,500)
            for char in text:
                incremental.edit({'type':'text','text':char});layout.frame(height=200)
            loaded=NativeLayout(model(text),500)
            self.assertEqual(pixels(layout.frame(height=200)),pixels(loaded.frame(height=200)))
            self.assertEqual(incremental.block_text(0),text)

    def test_equal_mark_nodes_do_not_split_arabic_shaping(self):
        split=TextModel({'type':'doc','content':[{'type':'paragraph','content':[{'type':'text','text':'ي'},{'type':'text','text':'س'}]}]})
        self.assertEqual(pixels(NativeLayout(split,400).frame(height=200)),pixels(NativeLayout(model('يس'),400).frame(height=200)))

    def test_arabic_mark_has_stable_baseline_and_real_joined_width(self):
        plain=NativeLayout(model('م',attrs={'lang':'ar','dir':'rtl'}),400)
        marked=NativeLayout(model('مِ',attrs={'lang':'ar','dir':'rtl'}),400)
        self.assertEqual(plain.paragraphs[0].lines[0][5],marked.paragraphs[0].lines[0][5])
        self.assertNotEqual(pixels(plain.frame(height=200)),pixels(marked.frame(height=200)))
        joined=NativeLayout(model('يس'),400).paragraphs[0].layout.get_pixel_size()[0]
        spaced=NativeLayout(model('ي س'),400).paragraphs[0].layout.get_pixel_size()[0]
        self.assertLess(joined,spaced)

    def test_visual_arrow_follows_rtl_and_graphemes(self):
        layout=NativeLayout(model('مِيس',attrs={'lang':'ar','dir':'rtl'}),400)
        cursor=(0,0);xs=[];visited=[]
        for _ in range(3):
            xs.append(layout.cursor_rectangle(cursor)[0]);visited.append(cursor[1]);cursor=layout.visual_target(cursor,-1)
        self.assertEqual(visited,[0,2,3]);self.assertEqual(cursor,(0,4))
        self.assertTrue(all(a>b for a,b in zip(xs,xs[1:])))
        emoji=NativeLayout(model('A👩‍💻B'),400)
        self.assertEqual(emoji.visual_target((0,1),1),(0,4))

    def test_hit_test_round_trip_and_selection_mixed_scripts(self):
        m=model('Hello العربية 中文');layout=NativeLayout(m,600)
        for offset in (1,3,15):
            x,y,_,height=layout.cursor_rectangle((0,offset))
            self.assertEqual(layout.locate_point(x,y+height/2),(0,offset))
        m.anchor=(0,6);m.cursor=(0,13)
        frame=layout.frame(height=200)
        self.assertEqual(frame['selectedText'],'العربية');self.assertTrue(frame['selection'])
        self.assertTrue(all(rect[2]>0 for rect in frame['selection']))
        self.assertEqual(frame['origin'],[0,0]);self.assertGreater(frame['cursor'][0],0)

    def test_vertical_navigation_softwrap_and_home_end(self):
        m=model('one two three four five six seven eight nine ten '*6,'short','long final paragraph')
        layout=NativeLayout(m,240);cursor=(0,4);initial=layout.cursor_rectangle(cursor)
        below=layout.vertical_target(cursor,1)
        self.assertGreater(layout.cursor_rectangle(below)[1],initial[1])
        above=layout.vertical_target(below,-1)
        self.assertEqual(above,cursor)
        end=layout.line_edge(cursor,True);end_rect=layout.cursor_rectangle(end)
        self.assertEqual(end_rect[1],initial[1])
        start=layout.line_edge(end,False);self.assertEqual(start,(0,0))

    def test_cache_reuses_unchanged_paragraphs_after_insertion(self):
        m=model('first','second','third');layout=NativeLayout(m,400)
        layout.frame(height=200);self.assertEqual(layout.layout_builds,3)
        m.move_to((0,5));m.edit({'type':'text','text':'!'});layout.frame(height=200)
        self.assertEqual(layout.layout_builds,4)
        m.edit({'type':'text','text':'\ninserted'});layout.frame(height=200)
        self.assertEqual(layout.layout_builds,5)
        layout.set_width(500);self.assertEqual(layout.layout_builds,9)

    def test_long_paragraph_renders_only_visible_lines(self):
        m=model('يس words 中文 '*4500);layout=NativeLayout(m,500)
        m.move_to((0,len(m.block_text(0))))
        frame=layout.frame(height=200,followCursor=True)
        self.assertGreater(frame['top'],10000)
        self.assertLess(layout.rendered_lines,10)
        self.assertEqual(len(pixels(frame)),500*200*4)
        self.assertGreaterEqual(frame['cursor'][1]/15,frame['top'])
        self.assertLessEqual((frame['cursor'][1]+frame['cursor'][3])/15,frame['top']+200+1)

    def test_real_fonts_follow_scripts_without_splitting_kasra(self):
        paragraph=NativeLayout(model('Latin مِ 中文'),600).paragraphs[0]
        iterator=paragraph.layout.get_iter();runs=[]
        while True:
            run=iterator.get_run_readonly()
            if run:
                runs.append((run.item.offset,run.item.length,run.item.analysis.font.describe().get_family()))
            if not iterator.next_run():break
        self.assertIn((0,6,'Noto Serif'),runs)
        self.assertIn((6,4,'Vazirmatn'),runs)
        self.assertIn((11,6,'Noto Serif CJK SC'),runs)

    def test_prepare_shapes_once_and_append_utf8_offsets_are_exact(self):
        m=model('Aمِ😀中');layout=NativeLayout(m,400)
        m.move_to((0,5));m.insert('يس👩‍💻')
        layout.prepare();builds=layout.layout_builds
        self.assertEqual(layout.paragraphs[0].offsets,byte_offsets(m.block_text(0)))
        layout.frame(height=200);layout.prepare()
        self.assertEqual(layout.layout_builds,builds)
        reference=NativeLayout(model(m.block_text(0)),400)
        self.assertEqual(pixels(layout.frame(height=200)),pixels(reference.frame(height=200)))

    def test_default_spacing_is_compact_without_rewriting_document(self):
        m=model('first line words '*20);before=m.document_snapshot()
        default=NativeLayout(m,240)
        explicit=NativeLayout(model(m.block_text(0),attrs={'lineSpacing':1.15}),240)
        loose=NativeLayout(model(m.block_text(0),attrs={'lineSpacing':2}),240)
        self.assertEqual(default.total_height,explicit.total_height)
        self.assertLess(default.total_height,loose.total_height)
        self.assertEqual(m.document_snapshot(),before)

    def test_requested_leading_matches_font_size_for_wraps_and_paragraph_breaks(self):
        for lang,text,size in [('en','English words and punctuation. ',18),
                               ('ar','يس مِ بِسْمِ اللَّهِ كلمات عربية. ',23)]:
            for spacing in (1,1.15,1.5,2):
                with self.subTest(lang=lang,spacing=spacing):
                    attrs={'lang':lang,'lineSpacing':spacing}
                    m=model(text*20,attrs=attrs);before=m.document_snapshot()
                    wrapped=NativeLayout(m,360).paragraphs[0]
                    expected=size*spacing
                    for a,b in zip(wrapped.lines,wrapped.lines[1:]):
                        self.assertAlmostEqual(b[5]-a[5],expected,delta=.01)
                    separate=NativeLayout(model(text,text,attrs=attrs),1000).paragraphs
                    self.assertAlmostEqual(separate[1].y+separate[1].lines[0][5]-
                                           separate[0].y-separate[0].lines[0][5],expected,delta=1)
                    self.assertEqual(m.document_snapshot(),before)

    def test_mixed_fallback_baselines_have_bounded_extra_metrics_without_paragraph_gap(self):
        text='English يس مِ 中文 words. '
        for spacing in (1,1.15,1.5,2):
            with self.subTest(spacing=spacing):
                attrs={'lang':'en','lineSpacing':spacing}
                paragraph=NativeLayout(model(text*20,attrs=attrs),360).paragraphs[0]
                for first,last in zip(paragraph.lines,paragraph.lines[1:]):
                    advance=last[5]-first[5]
                    self.assertGreaterEqual(advance,18*spacing-.01)
                    # Fallback fonts have different baselines even with the
                    # same em size; retain their shaping instead of clipping.
                    self.assertLessEqual(advance,18*spacing+6)
                separate=NativeLayout(model(text,text,attrs=attrs),1000).paragraphs
                self.assertAlmostEqual(separate[1].y,separate[0].y+separate[0].height)

    def test_combining_marks_never_move_the_initial_arabic_baseline(self):
        for start,suffix in [('م','ِ'),('يس','ِ'),('م','ُّْٓ')]:
            with self.subTest(start=start,suffix=suffix):
                m=model(start,attrs={'lang':'ar','dir':'rtl','lineSpacing':1.15})
                layout=NativeLayout(m,400);m.move_to((0,len(start)))
                baseline=layout.paragraphs[0].y+layout.paragraphs[0].lines[0][5]
                for mark in suffix:
                    m.insert(mark);layout.prepare();p=layout.paragraphs[0]
                    self.assertEqual(p.y+p.lines[0][5],baseline)
                    self.assertGreaterEqual(p.y+p.ink_top,0)
                self.assertEqual(m.block_text(0),start+suffix)

    def test_ink_outside_compact_boxes_survives_viewport_culling_and_caret_follow(self):
        m=model(*(['يس مِ بِسْمِ اللَّهِ مُّْٓ']*18),attrs={'lang':'ar','dir':'rtl','lineSpacing':1.15})
        before=m.document_snapshot();layout=NativeLayout(m,400)
        first=layout.paragraphs[0]
        self.assertGreater(first.ink_bottom,first.height)
        for top in (0,int(first.y+first.height+1),int(layout.paragraphs[8].y)):
            with self.subTest(top=top):
                frame=layout.frame(top=top,height=120)
                surface=cairo.ImageSurface(cairo.FORMAT_ARGB32,400,120);ctx=cairo.Context(surface)
                ctx.set_source_rgb(1,254/255,249/255);ctx.paint();ctx.set_source_rgb(62/255,73/255,64/255)
                for paragraph in layout.paragraphs:
                    ctx.move_to(PADDING,paragraph.y-frame['top'])
                    PangoCairo.show_layout(ctx,paragraph.layout)
                self.assertEqual(pixels(frame),bytes(surface.get_data()))
        for block in range(len(m.blocks)):
            m.move_to((block,len(m.block_text(block))))
            frame=layout.frame(height=120,followCursor=True)
            paragraph=layout.paragraphs[block]
            ink_y,ink_height=paragraph.ink_bounds[-1]
            self.assertGreaterEqual(paragraph.y+ink_y,frame['top'])
            self.assertLessEqual(paragraph.y+ink_y+ink_height,frame['top']+120)
        self.assertEqual(m.document_snapshot(),before)

    def test_reflow_keeps_logical_passage_visible_without_following_caret(self):
        for text,attrs in [('long English words with punctuation '*200,{}),('يس مِ كلمات عربية متصلة '*200,{'dir':'rtl','lang':'ar'}),('English يس 中文 mixed text '*200,{})]:
            for old_width,new_width in ((240,850),(850,240)):
                with self.subTest(text=text[:12],width=old_width):
                    m=model(text,attrs=attrs);m.move_to((0,len(text)))
                    before=m.document_snapshot();cursor=m.cursor;layout=NativeLayout(m,old_width)
                    old_row=len(layout.lines)//2
                    block,row,row_y,row_height=layout.lines[old_row]
                    paragraph=layout.paragraphs[block]
                    anchor=codepoint_index(paragraph.offsets,paragraph.lines[row][0].start_index)
                    top=round(row_y+3)
                    mapped=layout.reflow(top,new_width)
                    target_row=layout.paragraphs[block].layout.index_to_line_x(layout.paragraphs[block].offsets[anchor],False)[0]
                    target=layout.paragraphs[block].lines[target_row]
                    self.assertAlmostEqual(mapped,layout.paragraphs[block].y+target[2]+3,delta=1)
                    frame=layout.frame(top=mapped,height=200,followCursor=False)
                    self.assertEqual(frame['top'],mapped)
                    self.assertLess(frame['top'],frame['cursor'][1]/15)
                    self.assertEqual(m.document_snapshot(),before);self.assertEqual(m.cursor,cursor)
                    followed=layout.frame(top=mapped,height=200,followCursor=True)
                    self.assertGreater(followed['top'],mapped)
                    self.assertLessEqual((followed['cursor'][1]+followed['cursor'][3])/15,followed['top']+200+1)

    def test_reflow_origin_and_invalid_width_do_not_change_text(self):
        m=model('unchanged draft '*80);layout=NativeLayout(m,850)
        self.assertEqual(layout.reflow(0,240),0)
        self.assertEqual(layout.reflow(100,240),100)
        for width in (239,4097,True,240.0):
            with self.assertRaises(ValueError):layout.reflow(100,width)
        self.assertEqual(m.block_text(0),'unchanged draft '*80)

    def test_private_use_unknown_glyph_is_nonfatal(self):
        m=model('Original \ue000 \U0010ffff');layout=NativeLayout(m,400);frame=layout.frame(height=200)
        self.assertGreaterEqual(frame['missingGlyphs'],1)
        self.assertEqual(m.block_text(0),'Original \ue000 \U0010ffff')
        self.assertTrue(frame['image'].startswith('data:image/png;base64,'))

    def test_full_width_real_bitmap_and_geometry_preserve_multilingual_text(self):
        m=model('يس مِ English 中文 👩🏽‍🚀 '*180);before=m.document_snapshot()
        layout=NativeLayout(m,1200);old_height=layout.total_height
        layout.set_width(4096);frame=layout.frame(height=2160)
        surface=cairo.ImageSurface.create_from_png(BytesIO(base64.b64decode(frame['image'].split(',')[1])))
        self.assertEqual(surface.get_width(),4096);self.assertEqual(frame['width'],4096)
        self.assertLess(layout.total_height,old_height);self.assertEqual(m.document_snapshot(),before)
        cursor=layout.locate_point(*[v/15 for v in frame['cursor'][:2]])
        self.assertEqual(cursor,m.cursor)
        with self.assertRaises(ValueError):layout.frame(height=2161)
        with self.assertRaises(ValueError):layout.set_width(4097)

if __name__=='__main__':unittest.main()
