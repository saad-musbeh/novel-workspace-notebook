//! Exact Unicode manuscript model. Positions count scalar values, never UTF-8 bytes.
//! Immutable paragraphs are shared across bounded Undo states.
use regex::Regex;
use serde_json::{
    json, Map, Value
};
use std::{
    collections::{
        BTreeSet, HashSet
    }, sync::{
        Arc, LazyLock
    }
};
use unicode_segmentation::UnicodeSegmentation;
pub type Result<T> = std::result::Result<T, String>;
pub type Position = [usize;
2];
pub const MARKS: [&str;
3] = ["bold", "italic", "underline"];
pub const LANGUAGES: [&str;
5] = ["en", "ar", "zh-Hans", "zh-Hant", "und"];
pub const HISTORY_BYTES: usize = 64 * 1024 * 1024;
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}\p{N}\p{M}_]+").unwrap());
pub fn validate_text(text: &str) -> Result<()> {
    if text.len() > 2_000_000 || text.chars().any(|c| c < ' ' && !matches!(c, '\n' | '\r' | '\t')) {
        return Err("Unsupported text input. The manuscript was preserved.".into());
    }
    Ok(())
}
fn object<'a>(value: &'a Value, allowed: &[&str]) -> Result<&'a Map<String, Value>> {
    let map = value.as_object().ok_or("Invalid manuscript metadata.")?;
    if map.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("Unsupported manuscript metadata.".into());
    }
    Ok(map)
}
pub fn validate_document(document: &Value) -> Result<()> {
    object(document, &["type", "content"])?;
    if document["type"] != "doc" {
        return Err("Unsupported manuscript root.".into());
    }
    let empty = Vec::new();
    for block in document.get("content").map(|v| v.as_array().ok_or("Invalid manuscript paragraphs.")).transpose()?.unwrap_or(&empty) {
        object(block, &["type", "attrs", "content"])?;
        if !matches!(block["type"].as_str(), Some("paragraph" | "heading")) {
            return Err("Unsupported manuscript paragraph.".into());
        }
        if let Some(attrs) = block.get("attrs") {
            object(attrs, &["dir", "lang", "level", "lineSpacing"])?;
            if !["auto", "ltr", "rtl"].contains(&attrs.get("dir").map(|v| v.as_str().unwrap_or("")).unwrap_or("auto")) || !LANGUAGES.contains(&attrs.get("lang").map(|v| v.as_str().unwrap_or("")).unwrap_or("en")) {
                return Err("Unsupported paragraph language or direction.".into());
            }
            if let Some(level) = attrs.get("level") {
                if block["type"] != "heading" || !level.as_u64().is_some_and(|v| (1..=6).contains(&v)) {
                    return Err("Unsupported heading level.".into());
                }
            }
            if let Some(spacing) = attrs.get("lineSpacing") {
                if !spacing.as_f64().is_some_and(|v| [1., 1.15, 1.5, 2.].contains(&v)) {
                    return Err("Unsupported line spacing.".into());
                }
            }
        }
        for node in block.get("content").map(|v| v.as_array().ok_or("Invalid manuscript runs.")).transpose()?.unwrap_or(&empty) {
            match node["type"].as_str() {
                Some("hardBreak") => {
                    object(node, &["type"])?;
                }
                Some("text") => {
                    object(node, &["type", "text", "marks"])?;
                    validate_text(node["text"].as_str().ok_or("Unsupported manuscript text.")?)?;
                    let mut seen = HashSet::new();
                    for mark in node.get("marks").map(|v| v.as_array().ok_or("Invalid manuscript formatting.")).transpose()?.unwrap_or(&empty) {
                        object(mark, &["type"])?;
                        let name = mark["type"].as_str().ok_or("Unsupported manuscript formatting.")?;
                        if !MARKS.contains(&name) || !seen.insert(name) {
                            return Err("Unsupported or duplicate manuscript formatting.".into());
                        }
                    }
                }
                _ => return Err("Unsupported manuscript run.".into()),
            }
        }
    }
    Ok(())
}
pub fn node_text(node: &Value) -> &str {
    if node["type"] == "hardBreak" {
        "\n"
    } else {
        node["text"].as_str().unwrap_or("")
    }
}
pub fn text_of(block: &Value) -> String {
    runs(block).iter().map(node_text).collect()
}
pub fn runs(block: &Value) -> &[Value] {
    block["content"].as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn len(text: &str) -> usize {
    text.chars().count()
}
pub fn byte_at(text: &str, offset: usize) -> usize {
    text.char_indices().nth(offset).map(|(i, _)| i).unwrap_or(text.len())
}
pub fn slice(text: &str, start: usize, end: usize) -> &str {
    &text[byte_at(text, start)..byte_at(text, end)]
}
pub fn node_marks(node: &Value) -> BTreeSet<String> {
    node["marks"].as_array().into_iter().flatten().filter_map(|m| m["type"].as_str().map(str::to_owned)).collect()
}
fn marked(text: &str, marks: &BTreeSet<String>) -> Value {
    let mut result = json!({
        "type":"text", "text":text
    });
    if !marks.is_empty() {
        result["marks"] = json!(MARKS.iter().filter(|m| marks.contains(**m)).map(|m| json!({
            "type":m
        })).collect::<Vec<_>>());
    }
    result
}
fn compact(nodes: Vec<Value>) -> Vec<Value> {
    let mut output: Vec<Value> = Vec::new();
    for node in nodes {
        if node["type"] == "text" && node["text"] == "" {
            continue;
        }
        if let Some(last) = output.last_mut() {
            if node["type"] == "text" && last["type"] == "text" && node.get("marks").unwrap_or(&json!([])) == last.get("marks").unwrap_or(&json!([])) {
                last["text"] = json!(format!("{}{}", node_text(last), node_text(&node)));
                continue;
            }
        }
        output.push(node);
    }
    output
}
pub fn slice_runs(block: &Value, start: usize, end: usize) -> Vec<Value> {
    let mut output = Vec::new();
    let mut offset = 0;
    for node in runs(block) {
        let text = node_text(node);
        let stop = offset + len(text);
        let left = start.max(offset);
        let right = end.min(stop);
        if right > left {
            let mut part = node.clone();
            if node["type"] == "text" {
                part["text"] = json!(slice(text, left-offset, right-offset));
            }
            output.push(part);
        }
        offset = stop;
    }
    output
}
fn with_runs(block: &Value, nodes: Vec<Value>) -> Value {
    let mut result = block.clone();
    let nodes = compact(nodes);
    if nodes.is_empty() {
        result.as_object_mut().unwrap().remove("content");
    } else {
        result["content"] = json!(nodes);
    }
    result
}
#[derive(Clone, PartialEq, Debug)] pub struct State {
    pub blocks: Vec<Arc<Value>>, pub cursor: Position, pub anchor: Position,     pub typing_marks: Option<BTreeSet<String>>,
}
#[derive(Clone)] pub struct TextModel {
    pub original: Value, pub state: State,     pub undo: Vec<(State, usize)>, pub redo: Vec<(State, usize)>,     pub undo_bytes: usize, pub undo_trimmed: bool, pub version: u64, pub max_document_bytes: usize,     unicode_result: Option<(Position, Position, String, bool)>,
}
impl TextModel {
    pub fn new(document: Value) -> Result<Self> {
        validate_document(&document)?;
        let mut blocks: Vec<_> = document["content"].as_array().into_iter().flatten().cloned().map(Arc::new).collect();
        if blocks.is_empty() {
            blocks.push(Arc::new(json!({
                "type":"paragraph"
            })));
        }
        Ok(Self {
            original: document, state: State {
                blocks, cursor: [0,0], anchor: [0,0], typing_marks: None
            }, undo: Vec::new(), redo: Vec::new(), undo_bytes: 0, undo_trimmed: false, version: 0, max_document_bytes: 2_000_000, unicode_result: None
        })
    }
    pub fn text(&self, index: usize) -> String {
        text_of(&self.state.blocks[index])
    }
    pub fn clamp(&self, position: Position) -> Position {
        let block = position[0].min(self.state.blocks.len()-1);
        [block, position[1].min(len(&self.text(block)))]
    }
    pub fn move_to(&mut self, position: Position, extend: bool) {
        self.unicode_result=None;
        self.state.cursor=self.clamp(position);
        if !extend {
            self.state.anchor=self.state.cursor;
        } self.state.typing_marks=None;
    }
    pub fn selected_range(&self) -> (Position, Position) {
        (self.state.cursor.min(self.state.anchor), self.state.cursor.max(self.state.anchor))
    }
    pub fn selection_text(&self) -> String {
        let (start, end) = self.selected_range();
        (start[0]..=end[0]).map(|i| {
            let text=self.text(i);
            slice(&text, if i==start[0] {
                start[1]
            } else {
                0
            }, if i==end[0] {
                end[1]
            } else {
                len(&text)
            }).to_owned()
        }).collect::<Vec<_>>().join("\n")
    }
    pub fn marks_at(&self, position: Position, prefer_left: bool) -> BTreeSet<String> {
        let mut at=0;
        let mut previous=BTreeSet::new();
        for node in runs(&self.state.blocks[position[0]]) {
            let stop=at+len(node_text(node));
            let marks=node_marks(node);
            if position[1]==at && at>0 && prefer_left {
                return previous;
            }
            if at<=position[1] && position[1]<stop {
                return marks;
            }
            at=stop;
            previous=marks;
        }
        previous
    }
    pub fn document(&self) -> Value {
        if self.original["content"].as_array().is_none_or(Vec::is_empty) && self.state.blocks.len()==1 && *self.state.blocks[0]==json!({
            "type":"paragraph"
        }) {
            return self.original.clone();
        }
        json!({
            "type":"doc", "content": self.state.blocks.iter().map(|v| v.as_ref()).collect::<Vec<_>>()
        })
    }
    fn record(&mut self, before: State) -> Result<bool> {
        if before == self.state {
            return Ok(false);
        }
        if serde_json::to_vec(&self.document()).map_err(|e| e.to_string())?.len()>self.max_document_bytes {
            self.state=before;
            return Err("Scene exceeds the 2 MB limit. The manuscript was preserved.".into());
        }
        let changed=before.blocks!=self.state.blocks;
        let current: HashSet<_> = self.state.blocks.iter().map(Arc::as_ptr).collect();
        let cost=before.blocks.len()*8+before.blocks.iter().filter(|b| !current.contains(&Arc::as_ptr(b))).map(|b| len(&text_of(b))*4).sum::<usize>();
        self.undo.push((before,cost));
        self.undo_bytes+=cost;
        self.redo.clear();
        while self.undo.len()>512 || self.undo_bytes>HISTORY_BYTES {
            self.undo_bytes-=self.undo.remove(0).1;
            self.undo_trimmed=true;
        }
        self.version+=1;
        Ok(changed)
    }
    fn replace(&mut self, text: &str) {
        let (start,end)=self.selected_range();
        let first=&self.state.blocks[start[0]];
        let last=&self.state.blocks[end[0]];
        let mut prefix=slice_runs(first,0,start[1]);
        let suffix=slice_runs(last,end[1],len(&text_of(last)));
        let marks=self.state.typing_marks.clone().unwrap_or_else(||self.marks_at(start,start==end));
        let parts: Vec<_> = text.split('\n').collect();
        let mut new=Vec::new();
        if !parts[0].is_empty() {
            prefix.push(marked(parts[0],&marks));
        }
        if parts.len()==1 {
            prefix.extend(suffix);
            new.push(Arc::new(with_runs(first,prefix)));
            self.state.cursor=[start[0],start[1]+len(parts[0])];
        }
        else {
            new.push(Arc::new(with_runs(first,prefix)));
            for part in &parts[1..] {
                let mut template=json!({
                    "type":"paragraph"
                });
                if first["attrs"].as_object().is_some_and(|v|!v.is_empty()) {
                    template["attrs"]=first["attrs"].clone();
                    template["attrs"].as_object_mut().unwrap().remove("level");
                }
                new.push(Arc::new(with_runs(&template, if part.is_empty(){
                    vec![]
                }else{
                    vec![marked(part,&marks)]
                })));
            }
            let mut nodes=runs(new.last().unwrap()).to_vec();
            nodes.extend(suffix);
            let merged=with_runs(new.last().unwrap(),nodes);
            *new.last_mut().unwrap()=Arc::new(merged);
            self.state.cursor=[start[0]+new.len()-1,len(parts.last().unwrap())];
        }
        self.state.blocks.splice(start[0]..=end[0],new);
        self.state.anchor=self.state.cursor;
    }
    pub fn insert(&mut self, text: &str) -> Result<bool> {
        validate_text(text)?;
        let before=self.state.clone();
        if !text.is_empty() || self.state.cursor!=self.state.anchor {
            self.replace(text);
        }
        self.record(before)
    }
    fn insert_break(&mut self) -> Result<bool> {
        let before=self.state.clone();
        self.replace("");
        let [block,offset]=self.state.cursor;
        let node=&self.state.blocks[block];
        let mut nodes=slice_runs(node,0,offset);
        nodes.push(json!({
            "type":"hardBreak"
        }));
        nodes.extend(slice_runs(node,offset,len(&text_of(node))));
        self.state.blocks[block]=Arc::new(with_runs(node,nodes));
        self.state.cursor=[block,offset+1];
        self.state.anchor=self.state.cursor;
        self.record(before)
    }
    pub fn tokens(text: &str) -> Vec<(usize,usize)> {
        let mut result=Vec::new();
        let mut byte=0;
        let mut scalar=0;
        while byte<text.len() {
            if let Some(m)=WORD.find_at(text,byte).filter(|m|m.start()==byte) {
                let size=len(m.as_str());
                result.push((scalar,scalar+size));
                scalar+=size;
                byte=m.end();
            }
            else {
                let g=text[byte..].graphemes(true).next().unwrap();
                let size=len(g);
                result.push((scalar,scalar+size));
                scalar+=size;
                byte+=g.len();
            }
        }
        result
    }
    pub fn logical_target(&self, forward: bool, word: bool) -> Position {
        let [block,offset]=self.state.cursor;
        let text=self.text(block);
        let size=len(&text);
        if forward && offset==size {
            return if block+1<self.state.blocks.len(){
                [block+1,0]
            }else{
                self.state.cursor
            };
        }
        if !forward && offset==0 {
            return if block>0{
                [block-1,len(&self.text(block-1))]
            }else{
                self.state.cursor
            };
        }
        if word {
            if forward {
                let end=Self::tokens(&text).into_iter().find(|(s,e)|*e>offset && !slice(&text,*s,*e).chars().all(char::is_whitespace)).map(|(_,e)|e).unwrap_or(size);
                return [block,end+text.chars().skip(end).take_while(|c|c.is_whitespace()).count()];
            }
            let end=offset-text.chars().take(offset).collect::<Vec<_>>().iter().rev().take_while(|c|c.is_whitespace()).count();
            return [block,Self::tokens(&text).into_iter().filter(|(s,_)|*s<end).next_back().map(|(s,_)|s).unwrap_or(0)];
        }
        let boundaries: Vec<_>=std::iter::once(0).chain(text.graphemes(true).scan(0,|offset,g|{
            *offset+=len(g);
            Some(*offset)
        })).collect();
        [block, if forward {
            boundaries.into_iter().find(|i|*i>offset).unwrap_or(size)
        }else{
            boundaries.into_iter().filter(|i|*i<offset).next_back().unwrap_or(0)
        }]
    }
    pub fn select_word(&mut self, position: Position) {
        let [block,offset]=self.clamp(position);
        let text=self.text(block);
        if let Some((s,e))=Self::tokens(&text).into_iter().find(|(s,e)|(*s<=offset && offset<*e) || offset==len(&text) && offset==*e) {
            self.state.anchor=[block,s];
            self.state.cursor=[block,e];
            self.state.typing_marks=None;
        }
        else {
            self.move_to([block,offset],false);
        }
    }
    pub fn select_paragraph(&mut self, position: Position) {
        let block=self.clamp(position)[0];
        self.state.anchor=[block,0];
        self.state.cursor=[block,len(&self.text(block))];
        self.state.typing_marks=None;
    }
    fn paragraph_format(&mut self, attrs: Value) -> Result<bool> {
        let before=self.state.clone();
        let (start,end)=self.selected_range();
        let last=end[0]-usize::from(end[1]==0 && end[0]>start[0]);
        for i in start[0]..=last {
            let mut block=self.state.blocks[i].as_ref().clone();
            let kind=attrs["style"].as_str().unwrap_or(block["type"].as_str().unwrap()).to_owned();
            let mut properties=block["attrs"].as_object().cloned().unwrap_or_default();
            for (key,value) in attrs.as_object().unwrap() {
                if key!="style" {
                    properties.insert(key.clone(),value.clone());
                }
            }
            if kind=="heading" {
                properties.entry("level").or_insert(json!(2));
            }else{
                properties.remove("level");
            }
            block["type"]=json!(kind);
            block["attrs"]=json!(properties);
            if *self.state.blocks[i]!=block {
                self.state.blocks[i]=Arc::new(block);
            }
        }
        self.record(before)
    }
    fn character_format(&mut self, mark: Option<&str>) -> Result<bool> {
        let before=self.state.clone();
        if self.state.cursor==self.state.anchor {
            let mut marks=self.state.typing_marks.clone().unwrap_or_else(||self.marks_at(self.state.cursor,true));
            if let Some(mark)=mark {
                if !marks.remove(mark){
                    marks.insert(mark.to_owned());
                }
            }else{
                marks.clear();
            }
            self.state.typing_marks=Some(marks);
            self.record(before)?;
            return Ok(false);
        }
        let (start,end)=self.selected_range();
        let selected: Vec<_>=(start[0]..=end[0]).flat_map(|i|slice_runs(&self.state.blocks[i],if i==start[0]{
            start[1]
        }else{
            0
        },if i==end[0]{
            end[1]
        }else{
            len(&self.text(i))
        })).collect();
        let remove=mark.is_some_and(|m|selected.iter().filter(|n|n["type"]=="text").all(|n|node_marks(n).contains(m)));
        for i in start[0]..=end[0] {
            let block=&self.state.blocks[i];
            let left=if i==start[0]{
                start[1]
            }else{
                0
            };
            let right=if i==end[0]{
                end[1]
            }else{
                len(&text_of(block))
            };
            let mut nodes=slice_runs(block,0,left);
            for node in slice_runs(block,left,right) {
                if node["type"]=="hardBreak" {
                    nodes.push(node);
                    continue;
                }
                let mut marks=node_marks(&node);
                if let Some(mark)=mark {
                    if remove{
                        marks.remove(mark);
                    }else{
                        marks.insert(mark.into());
                    }
                }else{
                    marks.clear();
                }
                nodes.push(marked(node_text(&node),&marks));
            }
            nodes.extend(slice_runs(block,right,len(&text_of(block))));
            let changed=with_runs(block,nodes);
            if **block!=changed {
                self.state.blocks[i]=Arc::new(changed);
            }
        }
        self.record(before)
    }
    pub fn flat_text(&self) -> String {
        (0..self.state.blocks.len()).map(|i|self.text(i)).collect::<Vec<_>>().join("\n")
    }
    fn flat_offset(&self, position: Position) -> usize {
        (0..position[0]).map(|i|len(&self.text(i))+1).sum::<usize>()+position[1]
    }
    pub fn from_flat(&self, mut offset: usize) -> Position {
        for i in 0..self.state.blocks.len() {
            let size=len(&self.text(i));
            if offset<=size{
                return [i,offset];
            } offset-=size+1;
        } let last=self.state.blocks.len()-1;
        [last,len(&self.text(last))]
    }
    fn query(query: &str) -> Result<()> {
        if query.is_empty() || len(query)>1000 || query.chars().any(|c|c<' ') {
            Err("Enter 1–1000 characters without control characters to find.".into())
        }else{
            Ok(())
        }
    }
    pub fn find(&mut self, query: &str) -> Result<Value> {
        Self::query(query)?;
        self.unicode_result=None;
        let text=self.flat_text();
        let byte=byte_at(&text,self.flat_offset(self.selected_range().1));
        let first=text[byte..].find(query).map(|i|i+byte);
        let found=first.or_else(||text.find(query));
        if let Some(i)=found {
            self.state.anchor=self.from_flat(len(&text[..i]));
            self.state.cursor=self.from_flat(len(&text[..i+query.len()]));
            self.state.typing_marks=None;
        }
        Ok(json!({
            "found":found.is_some(),"wrapped":first.is_none() && found.is_some()
        }))
    }
    pub fn replace_selection(&mut self, query: &str, replacement: &str) -> Result<Value> {
        Self::query(query)?;
        if len(replacement)>10000 || replacement.chars().any(|c|c<' ') {
            return Err("Replacement must contain at most 10000 characters without control characters.".into());
        }
        if self.selection_text()!=query {
            return Ok(json!({
                "found":false,"replaced":false,"message":"Find the text again before replacing."
            }));
        }
        self.insert(replacement)?;
        Ok(json!({
            "found":true,"replaced":true
        }))
    }
    pub fn unicode_toggle(&mut self) -> Result<Value> {
        let (mut start,end)=self.selected_range();
        let mut selected=self.selection_text();
        let reverse=self.unicode_result.as_ref().is_some_and(|(s,e,text,character)|*s==start && *e==end && *text==selected && *character);
        if selected.is_empty() {
            let [block,offset]=self.state.cursor;
            if offset==0 {
                return Ok(json!({
                    "converted":false,"message":"Select a character or hexadecimal Unicode code."
                }));
            }
            let text=self.text(block);
            let preceding=slice(&text,0,offset);
            let digits=preceding.chars().rev().take_while(char::is_ascii_hexdigit).count();
            start=[block,offset-if digits>0{
                digits
            }else{
                1
            }];
            selected=slice(&text,start[1],offset).into();
        }
        let replacement=if selected.chars().all(|c|c.is_ascii_hexdigit()) && !reverse {
            if selected.len()>6 {
                return Ok(json!({
                    "converted":false,"message":"Unicode hexadecimal codes contain at most six digits."
                }));
            }
            let scalar=u32::from_str_radix(&selected,16).ok().and_then(char::from_u32);
            let Some(c)=scalar else{
                return Ok(json!({
                    "converted":false,"message":"Enter a valid Unicode scalar value."
                }));
            };
            let text=c.to_string();
            if validate_text(&text).is_err(){
                return Ok(json!({
                    "converted":false,"message":"This control character cannot be inserted into the manuscript."
                }));
            } text
        }else if len(&selected)==1 {
            format!("{:04X}",selected.chars().next().unwrap() as u32)
        }else{
            return Ok(json!({
                "converted":false,"message":"Select one character or a hexadecimal Unicode code."
            }));
        };
        let before=self.state.clone();
        self.state.anchor=start;
        self.state.cursor=end;
        self.replace(&replacement);
        self.state.anchor=start;
        self.unicode_result=Some((start,self.state.cursor,replacement.clone(),len(&replacement)==1));
        self.record(before)?;
        Ok(json!({
            "converted":true,"text":replacement
        }))
    }
    pub fn history(&mut self, redo: bool) -> bool {
        self.unicode_result=None;
        let entry=if redo {
            self.redo.pop()
        }else{
            self.undo.pop()
        };
        let Some((state,cost))=entry else{
            return false;
        };
        let previous=std::mem::replace(&mut self.state,state);
        let changed=previous.blocks!=self.state.blocks;
        if redo {
            self.undo.push((previous,cost));
            self.undo_bytes+=cost;
        }else{
            self.redo.push((previous,cost));
            self.undo_bytes-=cost;
        }
        self.version+=1;
        changed
    }
    pub fn states(&self) -> Value {
        let marks=self.state.typing_marks.clone().unwrap_or_else(||self.marks_at(self.state.cursor,true));
        let mut result=json!({
        });
        for name in MARKS {
            result[name]=json!(marks.contains(name).to_string());
        }
        result["undo"]=json!((!self.undo.is_empty()).to_string());
        result["redo"]=json!((!self.redo.is_empty()).to_string());
        result["undoTrimmed"]=json!(self.undo_trimmed.to_string());
        result["undoDepth"]=json!(self.undo.len().to_string());
        result
    }
    pub fn swap_paragraph(&mut self, index: usize, replacement: Value) -> Result<()> {
        if index>=self.state.blocks.len(){
            return Err("This paragraph is no longer in the scene.".into());
        }
        validate_document(&json!({
            "type":"doc","content":[replacement]
        }))?;
        if *self.state.blocks[index]==replacement{
            return Ok(());
        }
        let before=self.state.clone();
        self.state.blocks[index]=Arc::new(replacement);
        self.state.cursor=self.clamp(self.state.cursor);
        self.state.anchor=self.state.cursor;
        self.state.typing_marks=None;
        self.unicode_result=None;
        self.record(before)?;
        Ok(())
    }
    pub fn edit(&mut self, request: &Value) -> Result<()> {
        self.unicode_result=None;
        match request["type"].as_str() {
            Some("text") => {
                self.insert(request["text"].as_str().ok_or("Unsupported text input." )?)?;
            }
            Some("key") => {
                let key=request["key"].as_str().ok_or("Unsupported manuscript key.")?;
                let shift=request["shift"].as_bool().unwrap_or(false);
                let ctrl=request["ctrl"].as_bool().unwrap_or(false);
                match key {
                    "ArrowLeft"|"ArrowRight" => {
                        let forward=key=="ArrowRight";
                        let target=if self.state.cursor!=self.state.anchor && !shift && !ctrl {
                            if forward{
                                self.selected_range().1
                            }else{
                                self.selected_range().0
                            }
                        }else{
                            self.logical_target(forward,ctrl)
                        };
                        self.move_to(target,shift);
                    }
                    "Backspace"|"Delete" => {
                        let before=self.state.clone();
                        if self.state.cursor==self.state.anchor {
                            self.state.anchor=self.logical_target(key=="Delete",ctrl);
                        } if self.state.cursor!=self.state.anchor{
                            self.replace("");
                        } self.record(before)?;
                    }
                    "Enter" => {
                        if shift{
                            self.insert_break()?;
                        }else{
                            self.insert("\n")?;
                        }
                    }
                    "Tab" => {
                        self.insert("\t")?;
                    }
                    "Home" => {
                        self.move_to(if ctrl{
                            [0,0]
                        }else{
                            [self.state.cursor[0],0]
                        },shift);
                    }
                    "End" => {
                        let block=if ctrl{
                            self.state.blocks.len()-1
                        }else{
                            self.state.cursor[0]
                        };
                        self.move_to([block,len(&self.text(block))],shift);
                    }
                    "Escape" => {
                        self.state.anchor=self.state.cursor;
                    }
                    _ => return Err("Unsupported manuscript key.".into()),
                }
            }
            Some("command") => match request["command"].as_str().ok_or("Unsupported manuscript command.")? {
                "bold"|"italic"|"underline" => {
                    self.character_format(request["command"].as_str())?;
                }
                "clearFormatting" => {
                    self.character_format(None)?;
                }
                "undo"|"redo" => {
                    self.history(request["command"]=="redo");
                }
                "selectAll" => {
                    let last=self.state.blocks.len()-1;
                    self.state.anchor=[0,0];
                    self.state.cursor=[last,len(&self.text(last))];
                }
                "ltr"|"rtl" => {
                    self.paragraph_format(json!({
                        "dir":request["command"]
                    }))?;
                }
                "lineSpacing" => {
                    if !request["lineSpacing"].as_f64().is_some_and(|v|[1.,1.15,1.5,2.].contains(&v)){
                        return Err("Choose line spacing 1, 1.15, 1.5, or 2.".into());
                    }self.paragraph_format(json!({
                        "lineSpacing":request["lineSpacing"]
                    }))?;
                }
                "blockStyle" => {
                    if !LANGUAGES.contains(&request["language"].as_str().unwrap_or("")) || !["auto","ltr","rtl"].contains(&request["direction"].as_str().unwrap_or("")) || !["paragraph","heading"].contains(&request["style"].as_str().unwrap_or("")){
                        return Err("Choose an explicit language, direction and paragraph style.".into());
                    }self.paragraph_format(json!({
                        "lang":request["language"],"dir":request["direction"],"style":request["style"]
                    }))?;
                }
                _ => return Err("Unsupported manuscript command.".into()),
            },             Some("snapshot"|"refresh") => {
            },             _ => return Err("Unsupported manuscript operation.".into()),
        }
        Ok(())
    }
}
