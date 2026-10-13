use crate::{
    checkpoint,fonts,layout::NativeLayout,model::{
        self,Position,Result,TextModel
    }
};
use serde_json::{
    json,Value
};
use sha2::{
    Digest,Sha256
};
use std::{
    path::{
        Path,PathBuf
    },time::Instant
};
pub struct NativeEditor {
    pub model:TextModel,pub layout:NativeLayout,pub opening:Value,pub directory:PathBuf,pub top:u32,pub height:u32,mouse_count:u64
}
fn integer(request:&Value,key:&str,default:u64,min:u64,max:u64)->Result<u64>{
    let n=match request.get(key){
        Some(v)=>v.as_u64().ok_or_else(||format!("Invalid editor {key}."))?,None=>default
    };
    if n<min || n>max{
        return Err(format!("Invalid editor {key}."));
    }Ok(n)
}
fn boolean(request:&Value,key:&str,default:bool)->Result<bool>{
    match request.get(key){
        Some(v)=>v.as_bool().ok_or_else(||format!("Invalid editor {key}.")),None=>Ok(default)
    }
}
fn string<'a>(request:&'a Value,key:&str)->Result<&'a str>{
    request[key].as_str().ok_or_else(||format!("Invalid editor {key}."))
}
impl NativeEditor {
    pub fn new(directory:&Path,request:Value)->Result<Self>{
        let scene=&request["scene"];
        if scene["schemaVersion"]!=1 || !scene["title"].as_str().is_some_and(|s|s.chars().count()<=500){
            return Err("Invalid manuscript scene.".into());
        }
        let serialized=serde_json::to_vec(scene).map_err(|e|e.to_string())?.len();
        if serialized>2_000_000{
            return Err("This editor supports scenes up to 2 MB.".into());
        }
        let width=integer(&request,"width",640,240,4096)? as u32;
        let families=fonts::register(directory,request.get("fonts").unwrap_or(&json!([])))?;
        let family=fonts::writing_system(&request["writingSystem"],&families)?;
        let mut model=TextModel::new(scene["document"].clone())?;
        model.max_document_bytes=2_000_000-(serialized-serde_json::to_vec(&scene["document"]).unwrap().len());
        let mut layout=NativeLayout::new(width,family)?;
        layout.prepare(&model)?;
        Ok(Self {
            model,layout,opening:request,directory:directory.into(),top:0,height:680,mouse_count:1
        })
    }
    pub fn resume(directory:&Path)->Result<Self>{
        let (saved,mut model)=checkpoint::read(directory)?;
        let mut candidate=Self::new(directory,saved["opening"].clone())?;
        model.max_document_bytes=candidate.model.max_document_bytes;
        candidate.model=model;
        candidate.layout=NativeLayout::new(saved["width"].as_u64().unwrap() as u32,candidate.layout.font_family.clone())?;
        candidate.layout.prepare(&candidate.model)?;
        candidate.top=saved["top"].as_u64().unwrap() as u32;
        candidate.height=saved["height"].as_u64().unwrap() as u32;
        Ok(candidate)
    }
    pub fn document_hash(&self)->String{
        format!("{:x}",Sha256::digest(serde_json::to_vec(&self.model.document()).unwrap()))
    }
    fn paragraph_capture(&self,index:usize)->Result<Value>{
        let block=self.model.state.blocks.get(index).ok_or("This paragraph is no longer in the scene.")?;
        Ok(json!({
            "blockIndex":index,"documentHash":self.document_hash(),"block":block.as_ref()
        }))
    }
    fn navigation_capture(&self)->Value{
        json!({
            "documentHash":self.document_hash(),"cursor":self.model.state.cursor,"anchor":self.model.state.anchor
        })
    }
    pub fn request(&mut self,request:&Value)->Result<Value>{
        let before=self.model.clone();
        let viewport=(self.top,self.height,self.layout.width);
        let mouse=self.mouse_count;
        let result=self.apply(request);
        if result.is_err(){
            self.model=before;
            self.top=viewport.0;
            self.height=viewport.1;
            self.layout.width=viewport.2;
            self.layout.reset_navigation();
            let _=self.layout.prepare(&self.model);
            self.mouse_count=mouse;
        }
        result
    }
    fn apply(&mut self,request:&Value)->Result<Value>{
        let started=Instant::now();
        if !request.is_object(){
            return Err("Invalid editor operation.".into());
        }
        let kind=string(request,"type")?;
        if !["open","text","key","mouse","command","refresh","snapshot"].contains(&kind){
            return Err("Unsupported editor operation.".into());
        }
        let height=integer(request,"viewportHeight",self.height as u64,120,2160)? as u32;
        let mut top=integer(request,"top",self.top as u64,0,1_000_000u64.max(self.layout.total_height as u64+height as u64))? as u32;
        let follow=boolean(request,"followCursor",false)?;
        let preview=boolean(request,"preview",false)?;
        let width=integer(request,"width",self.layout.width as u64,240,4096)? as u32;
        let defer=boolean(request,"deferRender",false)?;
        if defer && kind!="text"{
            return Err("Only ordinary text can defer rendering.".into());
        }
        let shift=boolean(request,"shift",false)?;
        let ctrl=boolean(request,"ctrl",false)?;
        let mut pointer=None;
        if kind=="mouse" {
            let x=integer(request,"x",u64::MAX,0,1_000_000)?;
            let y=integer(request,"y",u64::MAX,0,1_000_000u64.max((self.layout.total_height as u64+height as u64)*15))?;
            let count=integer(request,"count",1,1,3)?;
            let phase=string(request,"phase")?;
            if !["down","move","up"].contains(&phase){
                return Err("Invalid manuscript pointer operation.".into());
            }
            pointer=Some((x,y,count,phase,boolean(request,"buttons",false)?));
        }
        if kind=="key" && !["ArrowUp","ArrowDown","ArrowLeft","ArrowRight","Home","End","PageUp","PageDown","Enter","Backspace","Delete","Tab","Escape"].contains(&string(request,"key")?){
            return Err("Unsupported manuscript key.".into());
        }
        let inputs=if let Some(inputs)=request.get("inputs") {
            let inputs=inputs.as_array().filter(|a|!a.is_empty() && a.len()<=128).ok_or("Invalid ordered typing batch.")?;
            if kind!="text"{
                return Err("Invalid ordered typing batch.".into());
            }
            let texts=inputs.iter().map(|v|v.as_str().ok_or("Invalid ordered typing batch.")).collect::<std::result::Result<Vec<_>,_>>()?;
            if texts.iter().map(|t|t.len()).sum::<usize>()>256*1024 || texts.concat()!=string(request,"text")?{
                return Err("Typing batch exceeds its limit or does not match the original input.".into());
            }
            for text in &texts{
                model::validate_text(text)?;
            }Some(texts)
        }else{
            None
        };
        if width!=self.layout.width{
            if follow{
                self.layout.set_width(width,&self.model)?;
            }else{
                top=self.layout.reflow(top,width,&self.model)?;
            }
        }
        self.top=top;
        self.height=height;
        let mut capture=None;
        let mut navigation=None;
        let mut search=None;
        let mut unicode=None;
        match kind {
            "mouse"=>{
                let (x,y,count,phase,buttons)=pointer.unwrap();
                let point=self.layout.locate_point(&self.model,x as f64/15.,y as f64/15.)?;
                match phase{
                    "down"=>{
                        self.layout.reset_navigation();
                        self.mouse_count=count;
                        if shift{
                            self.model.move_to(point,true);
                        }else if count==2{
                            self.model.select_word(point);
                        }else if count==3{
                            self.model.select_paragraph(point);
                        }else{
                            self.model.move_to(point,false);
                        }
                    },"move" if buttons=>self.model.move_to(point,true),"up" if self.mouse_count==1=>self.model.move_to(point,true),_=>{
                    }
                }
            }
            "key"=>{
                let key=string(request,"key")?;
                let cursor=self.model.state.cursor;
                let target=match key {
                    "ArrowUp"|"ArrowDown"|"PageUp"|"PageDown"=>{
                        let mut delta=if key=="ArrowDown" || key=="PageDown"{
                            1
                        }else{
                            -1
                        };
                        if key.starts_with("Page"){
                            delta*=((height/28) as i32).max(1);
                        }Some(self.layout.vertical_target(&self.model,cursor,delta)?)
                    },                     "Home"|"End" if !ctrl=>Some(self.layout.line_edge(&self.model,cursor,key=="End")?),                     "ArrowLeft"|"ArrowRight" if !ctrl=>Some(if cursor!=self.model.state.anchor && !shift{
                        if key=="ArrowRight"{
                            self.model.selected_range().1
                        }else{
                            self.model.selected_range().0
                        }
                    }else{
                        self.layout.visual_target(&self.model,cursor,if key=="ArrowRight"{
                            1
                        }else{
                            -1
                        })?
                    }),                     _=>None,
                };
                if let Some(target)=target{
                    self.model.move_to(target,shift);
                }else{
                    self.layout.reset_navigation();
                    self.model.edit(request)?;
                }
            }
            "command"=>{
                match string(request,"command")? {
                    "park"=>checkpoint::write(&self.directory,self)?,                 "captureNavigation"=>navigation=Some(self.navigation_capture()),                 "restoreNavigation"=>{
                        let bookmark=&request["bookmark"];
                        if !bookmark.as_object().is_some_and(|m|m.len()==3 && m.contains_key("documentHash") && m.contains_key("cursor") && m.contains_key("anchor")){
                            return Err("Invalid reading location.".into());
                        }
                        if bookmark["documentHash"]!=self.document_hash(){
                            return Err("This scene changed since that reading location. Its current caret and selection are preserved.".into());
                        }
                        let position=|value:&Value|->Result<Position>{
                            let p=value.as_array().filter(|a|a.len()==2).ok_or("Invalid reading location.")?;
                            let b=p[0].as_u64().filter(|b|*b<(self.model.state.blocks.len() as u64)).ok_or("Invalid reading location.")? as usize;
                            let offset=p[1].as_u64().filter(|o|*o<=model::len(&self.model.text(b)) as u64).ok_or("Invalid reading location.")? as usize;
                            Ok([b,offset])
                        };
                        let anchor=position(&bookmark["anchor"])?;
                        let cursor=position(&bookmark["cursor"])?;
                        self.model.move_to(anchor,false);
                        self.model.move_to(cursor,true);
                        self.layout.reset_navigation();
                        navigation=Some(self.navigation_capture());
                    },                 "captureParagraph"=>capture=Some(self.paragraph_capture(self.model.state.cursor[0])?),                 "swapParagraph"=>{
                        let anchor=&request["anchor"];
                        let index=anchor["blockIndex"].as_u64().ok_or("Invalid paragraph anchor.")? as usize;
                        if !anchor.as_object().is_some_and(|m|m.contains_key("documentHash") && m.keys().all(|k|["blockIndex","documentHash","block"].contains(&k.as_str()))){
                            return Err("Invalid paragraph anchor.".into());
                        }
                        let current=self.paragraph_capture(index)?;
                        if current["documentHash"]!=anchor["documentHash"] || anchor.get("block").is_some_and(|b|*b!=current["block"]){
                            return Err("This scene changed. Capture the paragraph again before swapping alternatives.".into());
                        }
                        self.model.swap_paragraph(index,request["replacement"].clone())?;
                        self.layout.reset_navigation();
                        capture=Some(self.paragraph_capture(index)?);
                    },                 "find"=>{
                        search=Some(self.model.find(string(request,"query")?)?);
                        self.layout.reset_navigation();
                    },                 "replace"=>{
                        search=Some(self.model.replace_selection(string(request,"query")?,string(request,"replacement")?)?);
                        self.layout.reset_navigation();
                    },                 "unicodeToggle"=>{
                        unicode=Some(self.model.unicode_toggle()?);
                        self.layout.reset_navigation();
                    },                 _=>{
                        self.layout.reset_navigation();
                        self.model.edit(request)?;
                    },
                }
            },             "text"=>{
                self.layout.reset_navigation();
                if let Some(inputs)=inputs{
                    for text in inputs{
                        self.model.edit(&json!({
                            "type":"text","text":text
                        }))?;
                    }
                }else{
                    self.model.edit(request)?;
                }
            },             "open"|"refresh"|"snapshot"=>{
            },             _=>unreachable!(),
        }
        if defer{
            return Ok(json!({
                "deferred":true,"timing":{
                    "processingMs":started.elapsed().as_secs_f64()*1000.,"outputMs":0
                }
            }));
        }
        self.layout.prepare(&self.model)?;
        let applied=Instant::now();
        let mut frame=self.layout.frame(&self.model,top,height,follow)?;
        self.top=frame["top"].as_u64().unwrap() as u32;
        frame["position"]=json!({
            "cursor":self.model.state.cursor,"anchor":self.model.state.anchor
        });
        if let Some(v)=capture{
            frame["paragraphCapture"]=v;
        }if let Some(v)=navigation{
            frame["navigationCapture"]=v;
        }if let Some(v)=search{
            frame["searchResult"]=v;
        }if let Some(v)=unicode{
            frame["unicodeResult"]=v;
        }
        if kind=="open" || kind=="snapshot" || !preview{
            frame["document"]=self.model.document();
        }
        frame["timing"]=json!({
            "processingMs":applied.duration_since(started).as_secs_f64()*1000.,"outputMs":applied.elapsed().as_secs_f64()*1000.
        });
        Ok(frame)
    }
}
