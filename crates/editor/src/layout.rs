//! Pango shapes whole paragraphs; Cairo paints only visible lines.
use crate::model::{
    self, Position, Result, TextModel
};
use base64::{
    engine::general_purpose::STANDARD, Engine
};
use cairo::{
    Context, Format, ImageSurface
};
use glib::translate::{
    from_glib_full, ToGlibPtr
};
use pango::{
    AttrFontDesc, AttrInt, AttrList, FontDescription, Layout, LayoutLine
};
use serde_json::{
    json, Value
};
use std::{
    collections::HashMap, sync::Arc
};
use unicode_segmentation::UnicodeSegmentation;
const PADDING: f64 = 8.;
const SCALE: f64 = pango::SCALE as f64;
const GEOMETRY: f64 = 15.;
#[link(name = "pangocairo-1.0")] extern "C" {
    fn pango_cairo_create_layout(context: *mut cairo::ffi::cairo_t) -> *mut pango::ffi::PangoLayout;
    fn pango_cairo_show_layout_line(context: *mut cairo::ffi::cairo_t, line: *mut pango::ffi::PangoLayoutLine);
}
#[derive(Clone)] struct Line {
    line: LayoutLine, x: f64, y: f64, height: f64, baseline: f64, ink_y: f64, ink_height: f64
}
#[derive(Clone)] struct Paragraph {
    block: Arc<Value>, width: u32, offsets: Vec<i32>, text: String, layout: Layout, lines: Vec<Line>, y: f64, height: f64, ink_top: f64, ink_bottom: f64
}
#[derive(Clone)] struct Affinity {
    cursor: Position, identity: usize, row: usize, x: f64, index: i32, trailing: i32
}
pub struct NativeLayout {
    pub width: u32, pub font_family: Option<String>, pub total_height: u32,     context: Context, paragraphs: Vec<Paragraph>, lines: Vec<(usize,usize,f64,f64)>,     preferred_x: Option<f64>, affinity: Option<Affinity>,     pub layout_builds: usize, pub rendered_lines: usize,
}
fn codepoint(offsets: &[i32], index: i32, trailing: i32) -> usize {
    (offsets.partition_point(|b|*b<=index).saturating_sub(1)+trailing.max(0) as usize).min(offsets.len()-1)
}
fn units(value: f64) -> i32 {
    (value*SCALE).round() as i32
}
fn geometry(rect: [f64;
4]) -> Vec<i64> {
    rect.into_iter().map(|v|(v*GEOMETRY).round_ties_even() as i64).collect()
}
fn family(language: &str) -> &str {
    if language=="ar" {
        "Vazirmatn,Noto Serif,Noto Serif CJK SC"
    }else if language.starts_with("zh"){
        "Noto Serif CJK SC,Noto Serif,Vazirmatn"
    }else{
        "Noto Serif,Vazirmatn,Noto Serif CJK SC"
    }
}
fn line_ranges(line: &LayoutLine, start: i32, end: i32) -> Vec<i32> {
    // Pango returns n pairs. The cached binding copies n integers; explicitly
    // copy both endpoints before freeing the allocation with GLib.
    unsafe {
        let mut ranges=std::ptr::null_mut();
        let mut count=0;
        pango::ffi::pango_layout_line_get_x_ranges(line.to_glib_none().0,start,end,&mut ranges,&mut count);
        let result=if count>0 && !ranges.is_null(){
            std::slice::from_raw_parts(ranges,count as usize*2).to_vec()
        }else{
            vec![]
        };
        glib::ffi::g_free(ranges.cast());
        result
    }
}
impl NativeLayout {
    pub fn new(width: u32, font_family: Option<String>) -> Result<Self> {
        if !(240..=4096).contains(&width){
            return Err("Invalid editor width.".into());
        }
        let surface=ImageSurface::create(Format::ARgb32,1,1).map_err(|e|e.to_string())?;
        Ok(Self {
            width,font_family,total_height:220,context:Context::new(&surface).map_err(|e|e.to_string())?,paragraphs:Vec::new(),lines:Vec::new(),preferred_x:None,affinity:None,layout_builds:0,rendered_lines:0
        })
    }
    pub fn reset_navigation(&mut self) {
        self.preferred_x=None;
        self.affinity=None;
    }
    fn build(&mut self, block: Arc<Value>) -> Result<Paragraph> {
        // Cairo context is valid for the call; Pango transfers the new object's
        // ownership to the Rust wrapper. Both stay in this single-thread helper.
        let layout: Layout=unsafe {
            from_glib_full(pango_cairo_create_layout(self.context.to_raw_none()))
        };
        let context=layout.context();
        let attrs=&block["attrs"];
        let language=attrs["lang"].as_str().unwrap_or("en");
        let direction=attrs["dir"].as_str().unwrap_or("auto");
        context.set_language(Some(&pango::Language::from_string(if language=="und"{
            "en"
        }else{
            language
        })));
        context.set_base_dir(if direction=="rtl"{
            pango::Direction::Rtl
        }else{
            pango::Direction::Ltr
        });
        layout.set_auto_dir(direction=="auto");
        layout.set_alignment(if direction=="rtl"{
            pango::Alignment::Right
        }else{
            pango::Alignment::Left
        });
        layout.set_width(units(self.width as f64-2.*PADDING));
        layout.set_wrap(pango::WrapMode::WordChar);
        let text=model::text_of(&block);
        layout.set_text(&text);
        let size=if language=="ar"{
            23.
        }else if block["type"]=="heading"{
            26.
        }else{
            18.
        };
        let mut font=FontDescription::new();
        font.set_family(self.font_family.as_deref().unwrap_or(family(language)));
        font.set_absolute_size(size*SCALE);
        layout.set_font_description(Some(&font));
        let offsets: Vec<i32>=text.char_indices().map(|(i,_)|i as i32).chain(std::iter::once(text.len() as i32)).collect();
        let attributes=AttrList::new();
        let spacing=attrs["lineSpacing"].as_f64().unwrap_or(1.15);
        attributes.insert(AttrInt::new_line_height_absolute(units(size*spacing)));
        let mut position=0;
        for node in model::runs(&block) {
            let end=position+model::len(model::node_text(node));
            let marks=model::node_marks(node);
            if !marks.is_empty() && end>position {
                let mut font=font.clone();
                font.set_weight(if marks.contains("bold"){
                    pango::Weight::Bold
                }else{
                    pango::Weight::Normal
                });
                font.set_style(if marks.contains("italic"){
                    pango::Style::Italic
                }else{
                    pango::Style::Normal
                });
                let mut attr=AttrFontDesc::new(&font);
                attr.set_start_index(offsets[position] as u32);
                attr.set_end_index(offsets[end] as u32);
                attributes.insert(attr);
                if marks.contains("underline") {
                    let mut attr=AttrInt::new_underline(pango::Underline::Single);
                    attr.set_start_index(offsets[position] as u32);
                    attr.set_end_index(offsets[end] as u32);
                    attributes.insert(attr);
                }
            }
            position=end;
        }
        layout.set_attributes(Some(&attributes));
        let mut iter=layout.iter();
        let mut lines=Vec::new();
        let mut index=0;
        loop {
            let (ink,logical)=iter.line_extents();
            let line=layout.line_readonly(index).ok_or("Missing shaped line.")?;
            lines.push(Line {
                line,x:logical.x() as f64/SCALE,y:logical.y() as f64/SCALE,height:logical.height() as f64/SCALE,baseline:iter.baseline() as f64/SCALE,ink_y:ink.y() as f64/SCALE,ink_height:ink.height() as f64/SCALE
            });
            index+=1;
            if !iter.next_line(){
                break;
            }
        }
        let height=layout.pixel_size().1.max(1) as f64;
        let ink_top=lines.iter().map(|l|l.ink_y).fold(f64::INFINITY,f64::min);
        let ink_bottom=lines.iter().map(|l|l.ink_y+l.ink_height).fold(0.,f64::max);
        self.layout_builds+=1;
        Ok(Paragraph {
            block,width:self.width,offsets,text,layout,lines,y:0.,height,ink_top,ink_bottom
        })
    }
    pub fn prepare(&mut self, model: &TextModel) -> Result<()> {
        let old: HashMap<usize,Paragraph>=std::mem::take(&mut self.paragraphs).into_iter().map(|p|(Arc::as_ptr(&p.block) as usize,p)).collect();
        let mut paragraphs=Vec::new();
        let mut lines=Vec::new();
        let mut y=PADDING;
        for (index,block) in model.state.blocks.iter().enumerate() {
            let mut paragraph=match old.get(&(Arc::as_ptr(block) as usize)).filter(|p|p.width==self.width) {
                Some(p)=>p.clone(),None=>self.build(block.clone())?
            };
            if index==0 {
                y=PADDING+(paragraph.layout.font_description().unwrap().size() as f64/SCALE/2.).ceil();
            }
            paragraph.y=y;
            for (row,line) in paragraph.lines.iter().enumerate(){
                lines.push((index,row,y+line.y,line.height));
            }
            y+=paragraph.height;
            paragraphs.push(paragraph);
        }
        let bottom=paragraphs.iter().map(|p|p.y+p.ink_bottom).fold(y,f64::max);
        self.total_height=(bottom+PADDING).ceil().max(220.) as u32;
        self.paragraphs=paragraphs;
        self.lines=lines;
        Ok(())
    }
    pub fn set_width(&mut self, width: u32, model: &TextModel) -> Result<()> {
        if !(240..=4096).contains(&width){
            return Err("Invalid editor width.".into());
        }if width!=self.width{
            self.width=width;
            self.reset_navigation();
        } self.prepare(model)
    }
    pub fn reflow(&mut self, top: u32, width: u32, model: &TextModel) -> Result<u32> {
        self.prepare(model)?;
        if width==self.width{
            return Ok(top);
        }
        if top==0{
            self.set_width(width,model)?;
            return Ok(0);
        }
        let line_index=self.lines.partition_point(|l|l.2<=top as f64).saturating_sub(1);
        let (block,row,old_y,old_height)=self.lines[line_index];
        let p=&self.paragraphs[block];
        let offset=codepoint(&p.offsets,p.lines[row].line.start_index(),0);
        let within=(top as f64-old_y).clamp(0.,(old_height-1.).max(0.));
        self.set_width(width,model)?;
        let p=&self.paragraphs[block];
        let new_row=p.layout.index_to_line_x(p.offsets[offset],false).0 as usize;
        let line=&p.lines[new_row];
        Ok((p.y+line.y+within.min((line.height-1.).max(0.))).max(0.).round_ties_even() as u32)
    }
    fn affinity_for(&self, cursor: Position) -> Option<&Affinity> {
        self.affinity.as_ref().filter(|a|a.cursor==cursor && a.identity==Arc::as_ptr(&self.paragraphs[cursor[0]].block) as usize)
    }
    fn row(&self, cursor: Position) -> usize {
        self.affinity_for(cursor).map(|a|a.row).unwrap_or_else(|| {
            let p=&self.paragraphs[cursor[0]];
            p.layout.index_to_line_x(p.offsets[cursor[1]],false).0 as usize
        })
    }
    pub fn cursor_rectangle(&self, cursor: Position) -> [f64;
    4] {
        let p=&self.paragraphs[cursor[0]];
        let strong=p.layout.cursor_pos(p.offsets[cursor[1]]).0;
        if let Some(affinity)=self.affinity_for(cursor) {
            let natural=p.layout.index_to_line_x(p.offsets[cursor[1]],false).0 as usize;
            [PADDING+affinity.x,p.y+strong.y() as f64/SCALE+p.lines[affinity.row].y-p.lines[natural].y,1.,(strong.height() as f64/SCALE).max(1.)]
        }else{
            [PADDING+strong.x() as f64/SCALE,p.y+strong.y() as f64/SCALE,1.,(strong.height() as f64/SCALE).max(1.)]
        }
    }
    pub fn locate_point(&mut self, model: &TextModel, x: f64, y: f64) -> Result<Position> {
        self.prepare(model)?;
        let index=self.paragraphs.partition_point(|p|p.y<=y).saturating_sub(1);
        let p=&self.paragraphs[index];
        let (_,byte,trailing)=p.layout.xy_to_index(units(x-PADDING),units(y-p.y));
        let offset=codepoint(&p.offsets,byte,trailing);
        self.reset_navigation();
        Ok([index,offset])
    }
    fn set_affinity(&mut self, cursor: Position, row: usize, end: bool) {
        let p=&self.paragraphs[cursor[0]];
        let line=&p.lines[row].line;
        let start=codepoint(&p.offsets,line.start_index(),0);
        let mut finish=codepoint(&p.offsets,line.start_index()+line.length(),0);
        while finish>start && model::slice(&p.text,finish-1,finish)=="\n"{
            finish-=1;
        }
        let (index,trailing,x)=if end && finish>start {
            let passage=model::slice(&p.text,start,finish);
            let cluster=start+model::len(passage.graphemes(true).next_back().map(|g|&passage[..passage.len()-g.len()]).unwrap_or(""));
            (p.offsets[cluster],(finish-cluster) as i32,p.layout.index_to_line_x(p.offsets[cluster],true).1)
        }else{
            (p.offsets[cursor[1]],0,p.layout.index_to_line_x(p.offsets[cursor[1]],false).1)
        };
        self.affinity=Some(Affinity {
            cursor,identity:Arc::as_ptr(&p.block) as usize,row,x:x as f64/SCALE,index,trailing
        });
    }
    pub fn line_edge(&mut self, model: &TextModel, cursor: Position, end: bool) -> Result<Position> {
        self.prepare(model)?;
        let row=self.row(cursor);
        let p=&self.paragraphs[cursor[0]];
        let line=&p.lines[row].line;
        let mut offset=codepoint(&p.offsets,line.start_index()+if end{
            line.length()
        }else{
            0
        },0);
        if end{
            while offset>0 && model::slice(&p.text,offset-1,offset)=="\n"{
                offset-=1;
            }
        }
        let target=[cursor[0],offset];
        self.preferred_x=None;
        self.set_affinity(target,row,end);
        Ok(target)
    }
    pub fn visual_target(&mut self, model: &TextModel, cursor: Position, direction: i32) -> Result<Position> {
        self.prepare(model)?;
        let p=&self.paragraphs[cursor[0]];
        let affinity=self.affinity_for(cursor);
        let index=affinity.map(|a|a.index).unwrap_or(p.offsets[cursor[1]]);
        let trailing=affinity.map(|a|a.trailing).unwrap_or(0);
        let (byte,trailing)=p.layout.move_cursor_visually(true,index,trailing,direction);
        let target=if byte<0 {
            if cursor[0]>0{
                [cursor[0]-1,model::len(&self.paragraphs[cursor[0]-1].text)]
            }else{
                [0,0]
            }
        }
        else if byte>*p.offsets.last().unwrap() {
            if cursor[0]+1<self.paragraphs.len(){
                [cursor[0]+1,0]
            }else{
                [cursor[0],p.offsets.len()-1]
            }
        }
        else{
            [cursor[0],codepoint(&p.offsets,byte,trailing)]
        };
        self.reset_navigation();
        Ok(target)
    }
    pub fn vertical_target(&mut self, model: &TextModel, cursor: Position, delta: i32) -> Result<Position> {
        self.prepare(model)?;
        let row=self.row(cursor);
        let global=self.lines.iter().position(|l|l.0==cursor[0] && l.1==row).ok_or("Missing manuscript row.")?;
        let x=self.preferred_x.unwrap_or_else(||self.cursor_rectangle(cursor)[0]);
        self.preferred_x=Some(x);
        let index=(global as i64+delta as i64).clamp(0,self.lines.len() as i64-1) as usize;
        let (block,row,top,height)=self.lines[index];
        let p=&self.paragraphs[block];
        let (_,byte,trailing)=p.layout.xy_to_index(units(x-PADDING),units(top-p.y+height/2.));
        let offset=codepoint(&p.offsets,byte,trailing);
        let line=&p.lines[row].line;
        let end=offset==codepoint(&p.offsets,line.start_index()+line.length(),0);
        self.set_affinity([block,offset],row,end);
        Ok([block,offset])
    }
    fn selection_rectangles(&self, model: &TextModel, top: u32, height: u32) -> Vec<Vec<i64>> {
        if model.state.cursor==model.state.anchor{
            return Vec::new();
        }
        let (first,last)=model.selected_range();
        let mut rectangles=Vec::new();
        for block in first[0]..=last[0] {
            let p=&self.paragraphs[block];
            let start=if block==first[0]{
                first[1]
            }else{
                0
            };
            let end=if block==last[0]{
                last[1]
            }else{
                p.offsets.len()-1
            };
            for line in &p.lines {
                let y=p.y+line.y;
                if y+line.height<top as f64 || y>(top+height) as f64{
                    continue;
                }
                for range in line_ranges(&line.line,p.offsets[start],p.offsets[end]).chunks_exact(2) {
                    let left=range[0].min(range[1]) as f64/SCALE;
                    let right=range[0].max(range[1]) as f64/SCALE;
                    if right>left{
                        rectangles.push(geometry([PADDING+left,y,right-left,line.height]));
                    }
                }
            }
            if block<last[0] {
                let line=p.lines.last().unwrap();
                let y=p.y+line.y;
                if y+line.height>=top as f64 && y<=(top+height) as f64{
                    rectangles.push(geometry([self.width as f64-PADDING-4.,y,4.,line.height]));
                }
            }
        }
        rectangles
    }
    pub fn frame(&mut self, model: &TextModel, mut top: u32, height: u32, follow: bool) -> Result<Value> {
        self.prepare(model)?;
        let caret=self.cursor_rectangle(model.state.cursor);
        if follow {
            let p=&self.paragraphs[model.state.cursor[0]];
            let line=&p.lines[self.row(model.state.cursor)];
            let caret_top=caret[1].min(p.y+line.ink_y);
            let caret_bottom=(caret[1]+caret[3]).max(p.y+line.ink_y+line.ink_height);
            if caret_top<top as f64{
                top=caret_top.floor().max(0.) as u32;
                if model.state.cursor==[0,0]{
                    top=0;
                }
            }
            else if caret_bottom>(top+height) as f64{
                top=(caret_bottom-height as f64).ceil().max(0.) as u32;
            }
        }
        top=top.min(self.total_height.saturating_sub(height));
        let surface=ImageSurface::create(Format::ARgb32,self.width as i32,height as i32).map_err(|e|e.to_string())?;
        let context=Context::new(&surface).map_err(|e|e.to_string())?;
        context.set_source_rgb(1.,1.,1.);
        context.paint().map_err(|e|e.to_string())?;
        context.set_source_rgb(62./255.,73./255.,64./255.);
        self.rendered_lines=0;
        for p in &self.paragraphs {
            if p.y+p.ink_bottom<top as f64 || p.y+p.ink_top>(top+height) as f64{
                continue;
            }
            for line in &p.lines {
                if p.y+line.ink_y+line.ink_height<top as f64 || p.y+line.ink_y>(top+height) as f64{
                    continue;
                }
                context.move_to(PADDING+line.x,p.y+line.baseline-top as f64);
                // Both handles are owned and valid through this synchronous call.
                unsafe{
                    pango_cairo_show_layout_line(context.to_raw_none(),line.line.to_glib_none().0);
                }self.rendered_lines+=1;
            }
        }
        context.status().map_err(|e|e.to_string())?;
        let mut png=Vec::new();
        surface.write_to_png(&mut png).map_err(|e|e.to_string())?;
        Ok(json!({
            "image":format!("data:image/png;base64,{}",STANDARD.encode(png)),"width":self.width,"height":height,"totalHeight":self.total_height,"top":top,"origin":[0,0],"cursor":geometry(caret),"selection":self.selection_rectangles(model,top,height),"selectedText":model.selection_text(),"states":model.states(),"missingGlyphs":self.paragraphs.iter().map(|p|p.layout.unknown_glyphs_count()).sum::<i32>()
        }))
    }
}
