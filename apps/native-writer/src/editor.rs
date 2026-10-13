use crate::process::{
    Process,Reply
};
use base64::{
    engine::general_purpose::STANDARD,Engine
};
use gtk::{
    gdk,prelude::*
};
use serde_json::{
    json,Value
};
use std::{
    cell::{
        Cell,RefCell
    },io::Cursor,path::{
        Path,PathBuf
    },rc::Rc
};
pub struct EditorPane {
    pub surface:Rc<crate::app::desktop_preferences::WritingSurface>,
    pub widget:gtk::Box,pub return_note:gtk::Label,pub area:gtk::DrawingArea,pub process:Process,pub frame:RefCell<Value>,     pub scene_id:String,pub title:RefCell<String>,pub parent:RefCell<String>,pub dirty:Cell<bool>,pub pending:Cell<usize>,     pub saving:Cell<bool>,pub open:Cell<bool>,pub composing:Cell<bool>,     image:RefCell<Option<cairo::ImageSurface>>,adjustment:gtk::Adjustment,adjusting:Cell<bool>,directory:PathBuf,     pub saved_document:RefCell<Value>,pub saved_title:RefCell<String>,pub error:gtk::Label,pub on_change:RefCell<Option<Box<dyn Fn()>>>,
}
impl Drop for EditorPane{
    fn drop(&mut self){
        self.process.stop();
        let _=std::fs::remove_dir_all(&self.directory);
    }
}
impl EditorPane {
    pub fn new(executable:&Path,cache:&Path,record:&Value)->Result<Rc<Self>,String>{
        let directory=cache.join(format!("rust-editor-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory,std::fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;
        }
        let process=Process::start(executable,&directory,false)?;
        let widget=gtk::Box::new(gtk::Orientation::Vertical,0);
        let return_note=gtk::Label::new(None);return_note.set_xalign(0.);return_note.set_line_wrap(true);return_note.set_margin_start(32);return_note.set_margin_end(24);return_note.set_margin_top(8);return_note.set_margin_bottom(8);return_note.set_no_show_all(true);widget.pack_start(&return_note,false,false,0);
        let area=gtk::DrawingArea::new();
        area.set_can_focus(true);
        area.set_hexpand(true);
        area.set_vexpand(true);
        area.set_size_request(240,240);
        area.add_events(gdk::EventMask::BUTTON_PRESS_MASK|gdk::EventMask::BUTTON_RELEASE_MASK|gdk::EventMask::POINTER_MOTION_MASK|gdk::EventMask::SCROLL_MASK|gdk::EventMask::SMOOTH_SCROLL_MASK|gdk::EventMask::KEY_PRESS_MASK|gdk::EventMask::KEY_RELEASE_MASK|gdk::EventMask::FOCUS_CHANGE_MASK);
        let adjustment=gtk::Adjustment::new(0.,0.,680.,28.,200.,680.);
        let scrollbar=gtk::Scrollbar::new(gtk::Orientation::Vertical,Some(&adjustment));
        let surface=crate::app::desktop_preferences::WritingSurface::new(&area,&scrollbar);
        let error=gtk::Label::new(None);
        error.set_xalign(0.);
        error.set_line_wrap(true);
        widget.pack_start(&surface.widget,true,true,0);
        widget.pack_start(&error,false,false,2);
        let pane=Rc::new(Self {
            surface,widget,return_note,area,process,frame:RefCell::new(Value::Null),scene_id:record["id"].as_str().ok_or("Missing scene identity.")?.into(),title:RefCell::new(record["scene"]["title"].as_str().unwrap_or("Scene").into()),parent:RefCell::new(record["revisionId"].as_str().ok_or("Missing scene revision.")?.into()),dirty:Cell::new(false),pending:Cell::new(0),saving:Cell::new(false),open:Cell::new(false),composing:Cell::new(false),image:RefCell::new(None),adjustment,adjusting:Cell::new(false),directory,saved_document:RefCell::new(record["scene"]["document"].clone()),saved_title:RefCell::new(record["scene"]["title"].as_str().unwrap_or("Scene").into()),error,on_change:RefCell::new(None)
        });
        Self::connect(&pane);
        let weak=Rc::downgrade(&pane);
        pane.process.request(json!({
            "type":"open","scene":record["scene"],"width":640
        }),move|reply|if let Some(pane)=weak.upgrade(){
            pane.accept(reply,false);
            pane.open.set(true);
            pane.refresh_allocation();
        });
        Ok(pane)
    }
    fn refresh_allocation(self:&Rc<Self>){
        if !self.open.get(){return;}
        let width=self.area.allocated_width().clamp(240,4096) as u32;
        let height=self.area.allocated_height().clamp(120,2160) as u32;
        let different={let frame=self.frame.borrow();frame["width"]!=width || frame["height"]!=height};
        if different{self.operation(json!({"type":"refresh","width":width,"viewportHeight":height}),false);}
    }
    fn accept(&self,reply:Reply,editing:bool){
        match reply{
            Ok(mut frame)=>{
                // Preview replies update layout/selection without repeating the document.
                // Keep the last full editor document for save state and tool-page captures.
                if frame.get("document").is_none(){
                    if let Some(document)=self.frame.borrow().get("document"){frame["document"]=document.clone();}
                }
                if let Some(image)=frame["image"].as_str().and_then(|s|s.strip_prefix("data:image/png;base64,")).and_then(|s|STANDARD.decode(s).ok()).and_then(|b|cairo::ImageSurface::create_from_png(&mut Cursor::new(b)).ok()){
                    *self.image.borrow_mut()=Some(image);
                }
                if editing{
                    self.dirty.set(true);
                }
                if let Some(height)=frame["totalHeight"].as_f64(){
                    self.adjusting.set(true);
                    self.adjustment.set_upper(height);
                    self.adjustment.set_page_size(frame["height"].as_f64().unwrap_or(680.));
                    self.adjustment.set_value(frame["top"].as_f64().unwrap_or(0.));
                    self.adjusting.set(false);
                }
                if frame["missingGlyphs"].as_i64().unwrap_or(0)>0{
                    self.error.set_text("Some characters need a font with matching glyphs. Original text is preserved.");
                }else{
                    self.error.set_text("");
                }
                *self.frame.borrow_mut()=frame;
                self.area.queue_draw();
                if let Some(callback)=self.on_change.borrow().as_ref(){
                    callback();
                }
            }
            Err(error)=>self.error.set_text(&error),
        }
    }
    pub fn operation(self:&Rc<Self>,value:Value,editing:bool){
        self.request(value,editing,|_|{});
    }
    pub fn request(self:&Rc<Self>,mut value:Value,editing:bool,callback:impl FnOnce(Reply)+'static){
        if !self.open.get(){callback(Err("The scene is still opening.".into()));return;}
        value["preview"]=json!(!editing);
        if value.get("followCursor").is_none(){value["followCursor"]=json!(editing);}
        value["viewportHeight"]=json!(self.area.allocated_height().clamp(120,2160));
        self.pending.set(self.pending.get()+1);
        let weak=Rc::downgrade(self);
        self.process.request(value,move|reply|{
            if let Some(pane)=weak.upgrade(){pane.pending.set(pane.pending.get().saturating_sub(1));pane.accept(reply.clone(),editing);}
            callback(reply);
        });
    }
    pub fn snapshot(self:&Rc<Self>,callback:impl FnOnce(Result<Value,String>)+'static){
        self.pending.set(self.pending.get()+1);
        let weak=Rc::downgrade(self);
        self.process.request(json!({
            "type":"snapshot","followCursor":false
        }),move|reply|{
            if let Some(pane)=weak.upgrade(){
                pane.pending.set(pane.pending.get().saturating_sub(1));
                if let Ok(frame)=&reply{
                    pane.accept(Ok(frame.clone()),false);
                }
            }
            callback(reply.map(|v|v["document"].clone()));
        });
    }
    pub fn discard(self:&Rc<Self>,callback:impl FnOnce(Result<(),String>)+'static){
        let pane=self.clone();
        self.process.request(json!({"type":"open","scene":{"schemaVersion":1,"title":self.saved_title.borrow().clone(),"document":self.saved_document.borrow().clone()},"width":self.area.allocated_width().max(240)}),move|reply|{
            match reply{
                Ok(frame)=>{pane.accept(Ok(frame),false);*pane.title.borrow_mut()=pane.saved_title.borrow().clone();pane.dirty.set(false);callback(Ok(()));},
                Err(e)=>callback(Err(e))
            }
        });
    }
    fn connect(pane:&Rc<Self>){
        let weak=Rc::downgrade(pane);
        pane.area.connect_draw(move|area,context|{
            if let Some(pane)=weak.upgrade(){
                if let Some(image)=pane.image.borrow().as_ref(){
                    let _=context.set_source_surface(image,0.,0.);
                    let _=context.paint();
                }
                let frame=pane.frame.borrow();
                let top=frame["top"].as_f64().unwrap_or(0.);
                context.set_source_rgba(0.25,0.45,0.75,0.28);
                for rect in frame["selection"].as_array().into_iter().flatten(){
                    if let Some(r)=rect.as_array().filter(|r|r.len()==4){
                        context.rectangle(r[0].as_f64().unwrap_or(0.)/15.,r[1].as_f64().unwrap_or(0.)/15.-top,r[2].as_f64().unwrap_or(0.)/15.,r[3].as_f64().unwrap_or(0.)/15.);
                        let _=context.fill();
                    }
                }
                if area.has_focus(){
                    if let Some(r)=frame["cursor"].as_array().filter(|r|r.len()==4){
                        context.set_source_rgb(0.1,0.15,0.12);
                        context.rectangle(r[0].as_f64().unwrap_or(0.)/15.,r[1].as_f64().unwrap_or(0.)/15.-top,1.5,r[3].as_f64().unwrap_or(0.)/15.);
                        let _=context.fill();
                    }
                }
            }glib::Propagation::Proceed
        });
        pane.area.drag_dest_set(gtk::DestDefaults::ALL,&[gtk::TargetEntry::new("UTF8_STRING",gtk::TargetFlags::empty(),0),gtk::TargetEntry::new("text/plain;charset=utf-8",gtk::TargetFlags::empty(),1)],gdk::DragAction::COPY);
        let weak=Rc::downgrade(pane);
        pane.area.connect_drag_data_received(move|_,context,x,y,data,_,time|{
            let accepted=if let (Some(p),Some(text))=(weak.upgrade(),data.text()){
                if !p.open.get() || p.composing.get() || p.saving.get(){false}else{
                    p.operation(json!({"type":"mouse","phase":"down","x":x.max(0)*15,"y":((y as f64+p.adjustment.value()).max(0.)*15.).round() as u64,"count":1,"shift":false}),false);
                    p.operation(json!({"type":"mouse","phase":"up","x":x.max(0)*15,"y":((y as f64+p.adjustment.value()).max(0.)*15.).round() as u64}),false);
                    p.operation(json!({"type":"text","text":text.as_str()}),true);true
                }
            }else{false};context.drag_finish(accepted,false,time);
        });
        let im=gtk::IMMulticontext::new();
        let im_realize=im.clone();
        pane.area.connect_realize(move|area|im_realize.set_client_window(area.window().as_ref()));
        let im_focus=im.clone();
        pane.area.connect_focus_in_event(move|area,_|{
            im_focus.focus_in();
            area.queue_draw();
            glib::Propagation::Proceed
        });
        let im_focus=im.clone();
        pane.area.connect_focus_out_event(move|area,_|{
            im_focus.focus_out();
            area.queue_draw();
            glib::Propagation::Proceed
        });
        let weak=Rc::downgrade(pane);
        im.connect_preedit_start(move|_|if let Some(p)=weak.upgrade(){
            p.composing.set(true);
        });
        let weak=Rc::downgrade(pane);
        im.connect_preedit_end(move|_|if let Some(p)=weak.upgrade(){
            p.composing.set(false);
        });
        let weak=Rc::downgrade(pane);
        im.connect_commit(move|_,text|if let Some(p)=weak.upgrade(){
            p.operation(json!({
                "type":"text","text":text
            }),true);
        });
        let weak=Rc::downgrade(pane);
        let im_keys=im.clone();
        pane.area.connect_key_press_event(move|_,event|{
            let Some(p)=weak.upgrade()else{
                return glib::Propagation::Proceed;
            };
            let key=event.keyval();
            let ctrl=event.state().contains(gdk::ModifierType::CONTROL_MASK);
            let shift=event.state().contains(gdk::ModifierType::SHIFT_MASK);
            if let Some(result)=crate::app::shortcuts::editor_shortcut(&p,event){return result;}
            let name=match key{
                gdk::keys::constants::Left=>Some("ArrowLeft"),gdk::keys::constants::Right=>Some("ArrowRight"),gdk::keys::constants::Up=>Some("ArrowUp"),gdk::keys::constants::Down=>Some("ArrowDown"),gdk::keys::constants::Home=>Some("Home"),gdk::keys::constants::End=>Some("End"),gdk::keys::constants::Page_Up=>Some("PageUp"),gdk::keys::constants::Page_Down=>Some("PageDown"),gdk::keys::constants::Return|gdk::keys::constants::KP_Enter=>Some("Enter"),gdk::keys::constants::BackSpace=>Some("Backspace"),gdk::keys::constants::Delete=>Some("Delete"),gdk::keys::constants::Tab=>Some("Tab"),gdk::keys::constants::Escape=>Some("Escape"),_=>None
            };
            if let Some(key)=name{
                if !p.composing.get(){
                    p.operation(json!({
                        "type":"key","key":key,"shift":shift,"ctrl":ctrl
                    }),["Enter","Backspace","Delete","Tab"].contains(&key));
                    return glib::Propagation::Stop;
                }
            }
            if im_keys.filter_keypress(event){
                glib::Propagation::Stop
            }else{
                glib::Propagation::Proceed
            }
        });
        let im_release=im.clone();
        pane.area.connect_key_release_event(move|_,event|if im_release.filter_keypress(event){
            glib::Propagation::Stop
        }else{
            glib::Propagation::Proceed
        });
        let weak=Rc::downgrade(pane);
        pane.area.connect_button_press_event(move|area,event|{
            area.grab_focus();
            if let Some(p)=weak.upgrade(){
                let(x,y)=event.position();
                let count=match event.event_type(){
                    gdk::EventType::DoubleButtonPress=>2,gdk::EventType::TripleButtonPress=>3,_=>1
                };
                p.operation(json!({
                    "type":"mouse","phase":"down","x":(x.max(0.)*15.).round() as u64,"y":((y+p.adjustment.value()).max(0.)*15.).round() as u64,"count":count,"shift":event.state().contains(gdk::ModifierType::SHIFT_MASK)
                }),false);
            }glib::Propagation::Stop
        });
        let weak=Rc::downgrade(pane);
        pane.area.connect_motion_notify_event(move|_,event|{
            if event.state().contains(gdk::ModifierType::BUTTON1_MASK){
                if let Some(p)=weak.upgrade(){
                    let(x,y)=event.position();
                    p.operation(json!({
                        "type":"mouse","phase":"move","buttons":true,"x":(x.max(0.)*15.).round() as u64,"y":((y+p.adjustment.value()).max(0.)*15.).round() as u64
                    }),false);
                }
            }glib::Propagation::Stop
        });
        let weak=Rc::downgrade(pane);
        pane.area.connect_button_release_event(move|_,event|{
            if let Some(p)=weak.upgrade(){
                let(x,y)=event.position();
                p.operation(json!({
                    "type":"mouse","phase":"up","x":(x.max(0.)*15.).round() as u64,"y":((y+p.adjustment.value()).max(0.)*15.).round() as u64
                }),false);
            }glib::Propagation::Stop
        });
        let weak=Rc::downgrade(pane);
        pane.adjustment.connect_value_changed(move|adjustment|if let Some(p)=weak.upgrade(){
            if !p.adjusting.get(){
                p.operation(json!({
                    "type":"refresh","top":adjustment.value().round() as u64
                }),false);
            }
        });
        let weak=Rc::downgrade(pane);
        pane.area.connect_scroll_event(move|_,event|{
            if let Some(p)=weak.upgrade(){
                let delta=match event.direction(){
                    gdk::ScrollDirection::Up=>-60.,gdk::ScrollDirection::Down=>60.,_=>event.scroll_deltas().map(|(_,y)|y*60.).unwrap_or(0.)
                };
                p.adjustment.set_value((p.adjustment.value()+delta).max(0.));
            }glib::Propagation::Stop
        });
        let weak=Rc::downgrade(pane);
        pane.area.connect_size_allocate(move|_,_|if let Some(p)=weak.upgrade(){p.refresh_allocation();});
    }
}
