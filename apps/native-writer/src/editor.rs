use crate::app::alphabet_host::{EditorProfile,EditorOptions,stage_editor_options};
use crate::app::sequence_host::{self,InputPermit};
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
    #[cfg(test)] pub(crate) test_input_method:RefCell<Option<gtk::IMMulticontext>>,
    pub surface:Rc<crate::app::desktop_preferences::WritingSurface>,
    pub(crate) alphabet_profile:Option<crate::alphabets::SessionProfile>,
    pub(crate) editor_options:RefCell<EditorOptions>,
    pub widget:gtk::Box,pub return_note:gtk::Label,pub area:gtk::DrawingArea,pub process:Process,pub frame:RefCell<Value>,     pub scene_id:String,pub title:RefCell<String>,pub parent:RefCell<String>,pub dirty:Cell<bool>,pub pending:Cell<usize>,     pub saving:Cell<bool>,pub open:Cell<bool>,pub composing:Cell<bool>,     image:RefCell<Option<cairo::ImageSurface>>,adjustment:gtk::Adjustment,adjusting:Cell<bool>,directory:PathBuf,     pub saved_document:RefCell<Value>,pub saved_title:RefCell<String>,pub error:gtk::Label,pub on_change:RefCell<Option<Box<dyn Fn()>>>,
}
impl Drop for EditorPane{
    fn drop(&mut self){
        self.process.stop();
        let _=std::fs::remove_dir_all(&self.directory);
    }
}
impl EditorPane {
    #[cfg(test)]
    pub fn new(executable:&Path,cache:&Path,record:&Value)->Result<Rc<Self>,String>{
        Self::new_with_profile(executable,cache,record,EditorProfile{profile:None,asset:None,notice:None})
    }
    pub(crate) fn new_with_profile(executable:&Path,cache:&Path,record:&Value,mut loaded:EditorProfile)->Result<Rc<Self>,String>{
        // The host has already checked the novel. Refuse a profile replayed onto
        // a different saved scene while keeping literal editing available.
        if loaded.profile.as_ref().is_some_and(|profile|profile.project_id.is_empty() || (profile.definition.is_some() && profile.alphabet_revision_id.as_deref().is_none_or(str::is_empty)) || Some(profile.scene_id.as_str())!=record["id"].as_str() || Some(profile.scene_revision_id.as_str())!=record["revisionId"].as_str()){
            loaded=EditorProfile{profile:None,asset:None,notice:Some("Alphabet profile differs from this saved scene. Literal Unicode editing remains available.".into())};
        }
        let directory=cache.join(format!("rust-editor-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory,std::fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;
        }
        let options=stage_editor_options(&directory,&loaded);
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
            #[cfg(test)] test_input_method:RefCell::new(None),
            alphabet_profile:loaded.profile,editor_options:RefCell::new(options),surface,widget,return_note,area,process,frame:RefCell::new(Value::Null),scene_id:record["id"].as_str().ok_or("Missing scene identity.")?.into(),title:RefCell::new(record["scene"]["title"].as_str().unwrap_or("Scene").into()),parent:RefCell::new(record["revisionId"].as_str().ok_or("Missing scene revision.")?.into()),dirty:Cell::new(false),pending:Cell::new(0),saving:Cell::new(false),open:Cell::new(false),composing:Cell::new(false),image:RefCell::new(None),adjustment,adjusting:Cell::new(false),directory,saved_document:RefCell::new(record["scene"]["document"].clone()),saved_title:RefCell::new(record["scene"]["title"].as_str().unwrap_or("Scene").into()),error,on_change:RefCell::new(None)
        });
        Self::connect(&pane);
        pane.open_reset(record["scene"].clone(),640,true,Box::new(|_|{}));
        Ok(pane)
    }
    fn open_value(&self,scene:Value,width:u32)->Value{
        let options=self.editor_options.borrow();
        json!({"type":"open","scene":scene,"width":width,"fonts":options.fonts,"writingSystem":options.writing_system})
    }
    fn font_fallback(&self,error:&str){
        let mut options=self.editor_options.borrow_mut();options.fonts=json!([]);
        options.writing_system=self.alphabet_profile.as_ref().map(|profile|profile.editor_selection(false)).unwrap_or(Value::Null);
        options.notice=Some(format!("Custom font registration failed; default-font editing and saving remain available. {error}"));
    }
    // A reset is used only before initial input or after an explicit Discard.
    // No metadata refresh can reopen an already edited helper or clear its Undo.
    fn open_reset(self:&Rc<Self>,scene:Value,width:u32,may_fallback:bool,done:Box<dyn FnOnce(Result<(),String>)>){
        self.open.set(false);self.pending.set(self.pending.get()+1);
        let had_fonts=self.editor_options.borrow().fonts.as_array().is_some_and(|fonts|!fonts.is_empty());
        let weak=Rc::downgrade(self);let value=self.open_value(scene.clone(),width);
        self.process.request(value,move|reply|{
            let Some(pane)=weak.upgrade()else{done(Err("Writing page closed during open.".into()));return;};
            pane.pending.set(pane.pending.get().saturating_sub(1));
            match reply{
                Err(error) if may_fallback&&had_fonts=>{pane.font_fallback(&error);pane.open_reset(scene,width,false,done);},
                Err(error)=>{pane.accept(Err(error.clone()),false);done(Err(error));},
                Ok(frame)=>{pane.accept(Ok(frame),false);pane.open.set(true);pane.refresh_allocation_then(done);}
            }
        });
    }
    /// The committed ordinary input path only; every literal action bypasses this.
    pub(crate) fn commit_ordinary(self:&Rc<Self>,text:&str)->Result<(),String>{
        if !self.open.get()||self.saving.get(){return Err("Wait for the writing page to finish opening or saving.".into());}
        let definition=self.alphabet_profile.as_ref().and_then(|profile|profile.definition.as_ref());
        let text=crate::alphabets::input_text(text,definition,crate::alphabets::InputOrigin::OrdinaryTyping)?;
        self.operation(json!({"type":"text","text":text}),true);Ok(())
    }
    fn refresh_allocation(self:&Rc<Self>){
        if !self.open.get(){return;}
        self.refresh_allocation_then(Box::new(|_|{}));
    }
    fn refresh_allocation_then(self:&Rc<Self>,done:Box<dyn FnOnce(Result<(),String>)>){
        if !self.open.get(){done(Err("The writing page is still opening.".into()));return;}
        let width=self.area.allocated_width().clamp(240,4096) as u32;
        let height=self.area.allocated_height().clamp(120,2160) as u32;
        let different={let frame=self.frame.borrow();frame["width"]!=width || frame["height"]!=height};
        if different{self.request(json!({"type":"refresh","width":width,"viewportHeight":height}),false,move|reply|done(reply.map(|_|())));}else{done(Ok(()));}
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
                let mut notices=self.editor_options.borrow().notice.clone().into_iter().collect::<Vec<_>>();
                if frame["missingGlyphs"].as_i64().unwrap_or(0)>0{notices.push("Some characters need a font with matching glyphs. Original text is preserved.".into());}
                self.error.set_text(&notices.join("\n"));
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
        if !sequence_host::request_allowed(self,&value){callback(Err("Command sequence playback has paused editor input; draft and Undo retained.".into()));return;}
        let recording=sequence_host::record_before(self,&value,editing);
        value["preview"]=json!(!editing);
        if value.get("followCursor").is_none(){value["followCursor"]=json!(editing);}
        value["viewportHeight"]=json!(self.area.allocated_height().clamp(120,2160));
        self.pending.set(self.pending.get()+1);
        let weak=Rc::downgrade(self);
        self.process.request(value,move|reply|{
            if let Some(pane)=weak.upgrade(){pane.pending.set(pane.pending.get().saturating_sub(1));pane.accept(reply.clone(),editing);sequence_host::record_after(&pane,recording,&reply);}
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
        let permit=match sequence_host::authorize_draft_mutation(self,sequence_host::DraftMutation::Discard){Ok(permit)=>permit,Err(error)=>{callback(Err(error));return;}};
        let pane=self.clone();let scene=json!({"schemaVersion":1,"title":self.saved_title.borrow().clone(),"document":self.saved_document.borrow().clone()});
        if !sequence_host::draft_mutation_allowed(self,&permit,sequence_host::DraftMutation::Discard){callback(Err("Discard authorization changed; draft and Undo retained.".into()));return;}
        self.open_reset(scene,self.area.allocated_width().max(240) as u32,true,Box::new(move|reply|{
            if reply.is_ok(){*pane.title.borrow_mut()=pane.saved_title.borrow().clone();pane.dirty.set(false);}callback(reply);
        }));
    }
    fn connect(pane:&Rc<Self>){
        // Capture at physical event scheduling, never freshly inside a late callback.
        let im_permit=Rc::new(RefCell::new(None::<InputPermit>));
        let pointer_permit=Rc::new(RefCell::new(None::<InputPermit>));
        let drop_permit=Rc::new(RefCell::new(None::<InputPermit>));
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
        let weak=Rc::downgrade(pane);let permit=drop_permit.clone();
        pane.area.connect_drag_drop(move|_,_,_,_,_|{
            *permit.borrow_mut()=weak.upgrade().and_then(|p|sequence_host::input_permit(&p));
            false // Gtk's default handler requests the negotiated data.
        });
        let weak=Rc::downgrade(pane);let permit=drop_permit;
        pane.area.connect_drag_data_received(move|_,context,x,y,data,_,time|{
            let accepted=if let (Some(p),Some(text))=(weak.upgrade(),data.text()){
                let captured=permit.borrow_mut().take();
                if !captured.as_ref().is_some_and(|permit|sequence_host::input_allowed(&p,permit)) || p.composing.get(){false}else{
                    p.operation(json!({"type":"mouse","phase":"down","x":x.max(0)*15,"y":((y as f64+p.adjustment.value()).max(0.)*15.).round() as u64,"count":1,"shift":false}),false);
                    p.operation(json!({"type":"mouse","phase":"up","x":x.max(0)*15,"y":((y as f64+p.adjustment.value()).max(0.)*15.).round() as u64}),false);
                    p.operation(json!({"type":"text","text":text.as_str()}),true);true
                }
            }else{false};context.drag_finish(accepted,false,time);
        });
        let im=gtk::IMMulticontext::new();
        #[cfg(test)] {*pane.test_input_method.borrow_mut()=Some(im.clone());}
        let im_realize=im.clone();
        pane.area.connect_realize(move|area|im_realize.set_client_window(area.window().as_ref()));
        let im_focus=im.clone();let weak=Rc::downgrade(pane);
        pane.area.connect_focus_in_event(move|area,_|{
            if weak.upgrade().is_some_and(|p|sequence_host::input_ready(&p)){im_focus.focus_in();}
            area.queue_draw();
            glib::Propagation::Proceed
        });
        let im_focus=im.clone();
        let weak=Rc::downgrade(pane);
        pane.area.connect_focus_out_event(move|area,_|{
            if let Some(p)=weak.upgrade(){crate::app::peek::cancel_for_pane(&p,None);}
            im_focus.focus_out();
            area.queue_draw();
            glib::Propagation::Proceed
        });
        let weak=Rc::downgrade(pane);let permit=im_permit.clone();
        im.connect_preedit_start(move|im|if let Some(p)=weak.upgrade(){
            crate::app::peek::cancel_for_pane(&p,None);
            let allowed=permit.borrow().as_ref().is_some_and(|permit|sequence_host::input_allowed(&p,permit));
            if allowed{p.composing.set(true);}else{im.reset();}
        });
        let weak=Rc::downgrade(pane);
        im.connect_preedit_end(move|_|if let Some(p)=weak.upgrade(){
            p.composing.set(false);
        });
        let weak=Rc::downgrade(pane);let permit=im_permit.clone();
        im.connect_commit(move|im,text|if let Some(p)=weak.upgrade(){
            let allowed=permit.borrow().as_ref().is_some_and(|permit|sequence_host::composition_allowed(&p,permit));
            if !allowed{im.reset();return;}
            if let Err(error)=p.commit_ordinary(text){p.error.set_text(&error);}
        });
        let weak=Rc::downgrade(pane);
        let im_keys=im.clone();let permit=im_permit;
        pane.area.connect_key_press_event(move|_,event|{
            let Some(p)=weak.upgrade()else{
                return glib::Propagation::Proceed;
            };
            let Some(captured)=sequence_host::input_permit(&p)else{return glib::Propagation::Stop;};
            let key=event.keyval();
            let ctrl=event.state().contains(gdk::ModifierType::CONTROL_MASK);
            let shift=event.state().contains(gdk::ModifierType::SHIFT_MASK);
            if let Some(result)=crate::app::peek::key_press(&p,event){return result;}
            if key==gdk::keys::constants::Escape{crate::app::peek::cancel_for_pane(&p,None);}
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
            if !p.composing.get(){*permit.borrow_mut()=Some(captured);}
            if im_keys.filter_keypress(event){
                glib::Propagation::Stop
            }else{
                glib::Propagation::Proceed
            }
        });
        let im_release=im.clone();
        let weak=Rc::downgrade(pane);
        pane.area.connect_key_release_event(move|_,event|{
            if let Some(p)=weak.upgrade(){if let Some(result)=crate::app::peek::key_release(&p,event){return result;}if !sequence_host::input_ready(&p){return glib::Propagation::Stop;}}
            if im_release.filter_keypress(event){glib::Propagation::Stop}else{glib::Propagation::Proceed}
        });
        let weak=Rc::downgrade(pane);
        pane.widget.connect_unmap(move|_|if let Some(p)=weak.upgrade(){crate::app::peek::cancel_for_pane(&p,None);});
        let weak=Rc::downgrade(pane);
        pane.area.connect_unmap(move|_|if let Some(p)=weak.upgrade(){crate::app::peek::cancel_for_pane(&p,None);});
        let weak=Rc::downgrade(pane);let permit=pointer_permit.clone();
        pane.area.connect_button_press_event(move|area,event|{
            if let Some(p)=weak.upgrade(){
                let Some(captured)=sequence_host::input_permit(&p)else{*permit.borrow_mut()=None;return glib::Propagation::Stop;};
                *permit.borrow_mut()=Some(captured);area.grab_focus();
                let(x,y)=event.position();
                let count=match event.event_type(){
                    gdk::EventType::DoubleButtonPress=>2,gdk::EventType::TripleButtonPress=>3,_=>1
                };
                p.operation(json!({
                    "type":"mouse","phase":"down","x":(x.max(0.)*15.).round() as u64,"y":((y+p.adjustment.value()).max(0.)*15.).round() as u64,"count":count,"shift":event.state().contains(gdk::ModifierType::SHIFT_MASK)
                }),false);
            }glib::Propagation::Stop
        });
        let weak=Rc::downgrade(pane);let permit=pointer_permit.clone();
        pane.area.connect_motion_notify_event(move|_,event|{
            if event.state().contains(gdk::ModifierType::BUTTON1_MASK){
                if let Some(p)=weak.upgrade(){
                    if !permit.borrow().as_ref().is_some_and(|permit|sequence_host::input_allowed(&p,permit)){return glib::Propagation::Stop;}
                    let(x,y)=event.position();
                    p.operation(json!({
                        "type":"mouse","phase":"move","buttons":true,"x":(x.max(0.)*15.).round() as u64,"y":((y+p.adjustment.value()).max(0.)*15.).round() as u64
                    }),false);
                }
            }glib::Propagation::Stop
        });
        let weak=Rc::downgrade(pane);let permit=pointer_permit;
        pane.area.connect_button_release_event(move|_,event|{
            let captured=permit.borrow_mut().take();
            if let Some(p)=weak.upgrade(){
                if !captured.as_ref().is_some_and(|permit|sequence_host::input_allowed(&p,permit)){return glib::Propagation::Stop;}
                let(x,y)=event.position();
                p.operation(json!({
                    "type":"mouse","phase":"up","x":(x.max(0.)*15.).round() as u64,"y":((y+p.adjustment.value()).max(0.)*15.).round() as u64
                }),false);
            }glib::Propagation::Stop
        });
        let weak=Rc::downgrade(pane);
        pane.adjustment.connect_value_changed(move|adjustment|if let Some(p)=weak.upgrade(){
            if !p.adjusting.get(){
                if !sequence_host::input_ready(&p){let top=p.frame.borrow()["top"].as_f64().unwrap_or(0.);p.adjusting.set(true);adjustment.set_value(top);p.adjusting.set(false);return;}
                p.operation(json!({
                    "type":"refresh","top":adjustment.value().round() as u64
                }),false);
            }
        });
        let weak=Rc::downgrade(pane);
        pane.area.connect_scroll_event(move|_,event|{
            if let Some(p)=weak.upgrade(){
                if !sequence_host::input_ready(&p){return glib::Propagation::Stop;}
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
