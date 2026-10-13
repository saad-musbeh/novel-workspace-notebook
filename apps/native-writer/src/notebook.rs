//! Quiet writing aids. Explicit actions save notes; manuscript edits stay manual.
use super::*;

fn label(text:&str)->gtk::Label {
    let label=gtk::Label::new(Some(text));label.set_xalign(0.);label.set_line_wrap(true);label.set_selectable(true);label
}
fn valid(a:&Workspace,pid:&str,generation:u64)->bool {
    !a.desk.is_closed() && *a.pid.borrow()==pid && a.desk.generation.get()==generation
}
/// Convert a plain_text scalar offset (two newlines between blocks) to editor coordinates.
fn position(document:&Value,mut offset:usize)->Option<[usize;2]> {
    let blocks=document["content"].as_array()?;
    for (i,block) in blocks.iter().enumerate(){
        let length=novel_storage::plain_text(block).chars().count();
        if offset<=length{return Some([i,offset]);}
        if i+1<blocks.len() && offset<length+2{return None;}
        offset=offset.checked_sub(length+2)?;
    }None
}
fn page_owner(origin:&Rc<Workspace>,widget:&gtk::Widget,pid:&str)->Option<Rc<Workspace>> {
    let mut found=None;desk::with_button_owner(widget,||{let a=desk::effective(origin);
        if !a.desk.is_closed() && *a.pid.borrow()==pid && widget.toplevel().and_then(|w|w.downcast::<gtk::Window>().ok())==Some(a.window.clone()){found=Some(a);}
    });found
}
impl Workspace {
    pub(super) fn open_start_scene(self:&Rc<Self>,fallback:String){
        let a=self.clone();let pid=self.pid.borrow().clone();let generation=self.desk.generation.get();
        self.load_notebook_tools(move|r|{
            if !valid(&a,&pid,generation) || a.pages.n_pages()!=0{return;}
            let bookmark=r.as_ref().ok().and_then(|v|v["bookmarks"].as_array()).and_then(|v|v.last());
            if bookmark.is_some_and(|b|a.references().iter().any(|(id,_)|b["sceneId"]==*id)){a.return_to_bookmark();}else{a.open_scene(&fallback,false);}
        });
    }
    pub(super) fn clear_bookmark(self:&Rc<Self>){
        let a=desk::effective(self);let out=a.clone();a.update_notebook_tools(|v|{v["bookmarks"]=json!([]);Ok(())},move|r|match r{
            Ok(())=>{for p in out.panes.borrow().values(){p.return_note.hide();}out.status.set_text("Return bookmark cleared.");},Err(e)=>out.error(&e)
        });
    }

    pub(super) fn load_notebook_tools(self:&Rc<Self>,callback:impl FnOnce(Result<Value,String>)+'static){
        let pid=self.pid.borrow().clone();
        if pid.is_empty(){callback(Err("Create or open a novel first.".into()));return;}
        self.client.rpc("load_project_data",json!({"projectId":pid,"kind":"notebookTools"}),move|r|callback(r.map(|s|if s.is_null(){novel_storage::notebook_tools::initial()}else{s["payload"].clone()})));
    }
    pub(super) fn update_notebook_tools(self:&Rc<Self>,edit:impl Fn(&mut Value)->Result<(),String>+'static,callback:impl FnOnce(Result<(),String>)+'static){
        let pid=self.pid.borrow().clone();let a=self.clone();
        if pid.is_empty(){callback(Err("Create or open a novel first.".into()));return;}
        self.notebook_pending.set(self.notebook_pending.get()+1);
        let pending=self.clone();let callback=move|reply:Result<(),String>|{if reply.is_err(){pending.notebook_failures.set(pending.notebook_failures.get()+1);}pending.notebook_pending.set(pending.notebook_pending.get().saturating_sub(1));callback(reply);};
        self.client.rpc("load_project_data",json!({"projectId":pid,"kind":"notebookTools"}),move|r|{
            if a.desk.is_closed() || *a.pid.borrow()!=pid{callback(Err("The novel changed. Reopen this tool before saving.".into()));return;}
            let saved=match r{Ok(s)=>s,Err(e)=>{callback(Err(e));return;}};
            let mut payload=if saved.is_null(){novel_storage::notebook_tools::initial()}else{saved["payload"].clone()};
            if let Err(e)=edit(&mut payload){callback(Err(e));return;}
            a.client.rpc("save_project_data_checked",json!({"projectId":pid,"kind":"notebookTools","payload":payload,"parent":saved["revisionId"],"dependencies":[]}),move|r|callback(r.map(|_|())));
        });
    }
    pub(super) fn keep_scrap(self:&Rc<Self>){
        let a=desk::effective(self);let Some(p)=a.active.borrow().clone()else{a.error("Select some text in a writing page first.");return;};
        if p.composing.get(){a.error("Finish the current input composition first.");return;}
        let pid=a.pid.borrow().clone();let out=a.clone();let scene_id=p.scene_id.clone();
        p.request(json!({"type":"snapshot","followCursor":false}),false,move|r|{
            if *out.pid.borrow()!=pid || out.desk.is_closed(){return;}
            let text=match r{Ok(v)=>v["selectedText"].as_str().unwrap_or("").to_owned(),Err(e)=>{out.error(&e);return;}};
            if text.is_empty(){out.error("Select the passage you want to keep, then choose Keep selection as scrap.");return;}
            let done=out.clone();let id=uuid::Uuid::new_v4().to_string();
            out.update_notebook_tools(move|v|{v["scraps"].as_array_mut().unwrap().push(json!({"id":id,"sceneId":scene_id,"text":text}));Ok(())},move|r|match r{Ok(())=>done.status.set_text("Scrap saved. The selected text is still in your draft."),Err(e)=>done.error(&format!("Scrap not saved; selection retained. {e}"))});
        });
    }
    pub(super) fn cutting_room(self:&Rc<Self>){
        let a=desk::effective(self);let out=a.clone();a.navigate(move||out.open_cutting_room());
    }
    fn open_cutting_room(self:&Rc<Self>){
        let panel=self.panel("Cutting-room floor");let body=panel.content_area();
        body.pack_start(&label("Saved scraps and passages removed between saved versions. Drag text into a writing window, or append a copy to a scene. Unsaved deletions remain in Undo."),false,false,0);
        let list=gtk::Box::new(gtk::Orientation::Vertical,18);body.pack_start(&scroll(&list),true,true,0);
        let notice=label("Loading saved scraps…");list.pack_start(&notice,false,false,0);panel.show_all();
        let origin=self.clone();let pid=self.pid.borrow().clone();
        self.load_notebook_tools(move|r|{
            let Some(owner)=page_owner(&origin,list.upcast_ref(),&pid)else{return;};
            match r{Ok(data)=>{
                list.remove(&notice);
                for scrap in data["scraps"].as_array().into_iter().flatten(){owner.scrap_row(&list,scrap["text"].as_str().unwrap_or(""),"Kept selection");}
                notice.set_text("Looking through saved versions…");list.pack_start(&notice,false,false,0);list.show_all();
                owner.cut_history(owner.references(),0,list,notice,0);
            },Err(e)=>notice.set_text(&format!("Could not load scraps: {e}"))}
        });
    }
    fn scrap_row(self:&Rc<Self>,list:&gtk::Box,text:&str,source:&str){
        let row=gtk::Box::new(gtk::Orientation::Vertical,5);let heading=label(source);heading.style_context().add_class("notebook-hint");row.pack_start(&heading,false,false,0);
        let excerpt=label(text);excerpt.set_max_width_chars(80);excerpt.drag_source_set(gdk::ModifierType::BUTTON1_MASK,&[gtk::TargetEntry::new("UTF8_STRING",gtk::TargetFlags::empty(),0),gtk::TargetEntry::new("text/plain;charset=utf-8",gtk::TargetFlags::empty(),1)],gdk::DragAction::COPY);
        let drag_text=text.to_owned();excerpt.connect_drag_data_get(move|_,_,data,_,_|{data.set_text(&drag_text);});row.pack_start(&excerpt,false,false,0);
        let controls=gtk::Box::new(gtk::Orientation::Horizontal,6);let target=gtk::ComboBoxText::new();for (id,title) in self.references(){target.append(Some(&id),&title);}target.set_active(Some(0));controls.pack_start(&target,true,true,0);
        let a=self.clone();let text=text.to_owned();button(&controls,"Append to scene",move||{let a=desk::effective(&a);if let Some(id)=target.active_id(){a.append_scrap(id.to_string(),text.clone());}});row.pack_start(&controls,false,false,0);list.pack_start(&row,false,false,0);
    }
    fn cut_history(self:&Rc<Self>,scenes:Vec<(String,String)>,i:usize,list:gtk::Box,notice:gtk::Label,count:usize){
        let pid=self.pid.borrow().clone();let Some(owner)=page_owner(self,list.upcast_ref(),&pid)else{return;};
        if i==scenes.len(){notice.set_text(if count==0{"No passages removed from saved versions yet."}else{"All recovered passages remain in scene history."});return;}
        let a=owner.clone();let scene=scenes[i].clone();
        owner.client.rpc("history",json!({"sceneId":scene.0}),move|r|{
            let Some(a)=page_owner(&a,list.upcast_ref(),&pid)else{return;};
            let history=match r{Ok(v)=>v,Err(e)=>{notice.set_text(&format!("Could not read saved cuts: {e}"));return;}};
            let revisions=history.as_array().cloned().unwrap_or_default();let by_id=revisions.iter().filter_map(|r|Some((r["id"].as_str()?,r))).collect::<HashMap<_,_>>();
            let mut seen=std::collections::HashSet::new();let mut count=count;
            for revision in &revisions{for parent in revision["parents"].as_array().into_iter().flatten(){if let Some(old)=parent.as_str().and_then(|id|by_id.get(id)){
                for text in novel_storage::notebook_tools::removed_passages(&old["payload"]["document"],&revision["payload"]["document"]){
                    if !text.trim().is_empty() && seen.insert(text.clone()) {a.scrap_row(&list,&text,&format!("{} · removed in a saved edit",scene.1));count+=1;}
                }
            }}}
            list.show_all();a.cut_history(scenes,i+1,list,notice,count);
        });
    }
    fn append_scrap(self:&Rc<Self>,id:String,text:String){
        let a=self.clone();self.navigate(move||{let next=a.clone();let generation=a.desk.generation.get();let Some(permit)=sequence_host::window_input_permit(&a)else{a.error("Scene input changed; the scrap remains retained.");return;};let permit=Rc::new(permit);a.ensure_pane(&id,move|r|{
            if next.desk.is_closed() || next.desk.generation.get()!=generation || !sequence_host::window_input_allowed(&next,&permit){next.error("Scene input authorization changed; the scrap remains retained.");return;}
            match r{Ok(p)=>{next.show_pane(p.clone(),false);next.append_when_ready(p,text,std::time::Instant::now(),permit);},Err(e)=>next.error(&e)}
        });});
    }
    fn append_when_ready(self:&Rc<Self>,p:Rc<EditorPane>,text:String,start:std::time::Instant,permit:Rc<sequence_host::WindowInputPermit>){
        if self.desk.is_closed() || self.current_widget()!=Some(p.widget.clone().upcast()){return;}
        if !sequence_host::window_input_allowed(self,&permit)||!self.panes.borrow().get(&p.scene_id).is_some_and(|registered|Rc::ptr_eq(registered,&p)){self.error("Scene input authorization changed; the saved note remains retained.");return;}
        if !p.open.get() || p.pending.get()!=0{if start.elapsed()>Duration::from_secs(15){self.error("Scene is still busy. The scrap is safely retained.");return;}let a=self.clone();glib::timeout_add_local_once(Duration::from_millis(30),move||a.append_when_ready(p,text,start,permit));return;}
        if p.composing.get() || p.saving.get(){self.error("Finish the active edit before inserting a scrap.");return;}
        let separator=if novel_storage::plain_text(&p.frame.borrow()["document"]).is_empty(){""}else{"\n\n"};
        p.operation(json!({"type":"key","key":"End","ctrl":true,"shift":false}),false);p.operation(json!({"type":"text","text":format!("{separator}{text}")}),true);
        self.status.set_text("Scrap appended to your draft. Ctrl+S saves; Undo removes the insertion.");
    }
    pub(super) fn leave_bookmark(self:&Rc<Self>){
        let a=desk::effective(self);let Some(p)=a.active.borrow().clone()else{a.error("Open a scene before leaving a bookmark.");return;};
        let Some(note)=prompt_with_action(&a.window,"Next time, start here","","Save bookmark")else{return;};
        if note.trim().is_empty(){a.error("Write a short reminder for your next session.");return;}
        let out=a.clone();a.navigate(move||{
            let a=out.clone();let scene_id=p.scene_id.clone();let pane=p.clone();
            p.request(json!({"type":"command","command":"captureNavigation"}),false,move|r|match r{
                Ok(v)=>{let location=v["navigationCapture"].clone();let done=a.clone();let id=uuid::Uuid::new_v4().to_string();let retained=note.clone();
                    a.update_notebook_tools(move|v|{let bookmarks=v["bookmarks"].as_array_mut().unwrap();bookmarks.retain(|b|b["sceneId"]!=scene_id);bookmarks.push(json!({"id":id,"sceneId":scene_id,"note":note,"location":location}));Ok(())},move|r|match r{Ok(())=>{done.show_return_note(&pane);done.status.set_text("Return bookmark saved. You can close the notebook when ready.");},Err(e)=>done.error(&format!("Bookmark not saved: {e} · Your reminder: {retained}"))});
                },Err(e)=>a.error(&e)
            });
        });
    }
    pub(super) fn show_return_note(self:&Rc<Self>,pane:&Rc<EditorPane>){
        let weak=Rc::downgrade(pane);let a=self.clone();let pid=self.pid.borrow().clone();
        self.load_notebook_tools(move|r|{if *a.pid.borrow()!=pid || a.desk.is_closed(){return;}if let (Some(p),Ok(v))=(weak.upgrade(),r){
            let note=v["bookmarks"].as_array().and_then(|v|v.iter().rev().find(|b|b["sceneId"]==p.scene_id)).and_then(|b|b["note"].as_str());
            if let Some(note)=note{p.return_note.set_text(&format!("Next: {note}"));p.return_note.show();}else{p.return_note.hide();}
        }});
    }
    pub(super) fn return_to_bookmark(self:&Rc<Self>){
        let a=desk::effective(self);let pid=a.pid.borrow().clone();let generation=a.desk.generation.get();let Some(initial)=sequence_host::window_input_permit(&a)else{return;};let out=a.clone();a.load_notebook_tools(move|r|{
            if !valid(&out,&pid,generation)||!sequence_host::window_input_allowed(&out,&initial){return;}
            match r{Ok(v)=>if let Some(bookmark)=v["bookmarks"].as_array().and_then(|v|v.last()){
                let bookmark=bookmark.clone();let id=bookmark["sceneId"].as_str().unwrap_or("").to_owned();let a=out.clone();out.navigate(move||{let next=a.clone();let generation=a.desk.generation.get();let Some(permit)=sequence_host::window_input_permit(&a)else{return;};let permit=Rc::new(permit);a.ensure_pane(&id,move|r|{
                    if next.desk.is_closed() || next.desk.generation.get()!=generation || !sequence_host::window_input_allowed(&next,&permit){return;}
                    match r{Ok(p)=>{next.show_pane(p.clone(),false);next.restore_when_ready(p,bookmark["location"].clone(),std::time::Instant::now(),permit);},Err(e)=>next.error(&e)}
                });});
            }else{out.error("No return bookmark yet. Use Write → Keep & return → Leave return bookmark.");},Err(e)=>out.error(&e)}
        });
    }
    fn restore_when_ready(self:&Rc<Self>,p:Rc<EditorPane>,location:Value,start:std::time::Instant,permit:Rc<sequence_host::WindowInputPermit>){
        if self.desk.is_closed() || self.current_widget()!=Some(p.widget.clone().upcast()){return;}
        if !sequence_host::window_input_allowed(self,&permit)||!self.panes.borrow().get(&p.scene_id).is_some_and(|registered|Rc::ptr_eq(registered,&p)){self.error("Scene input authorization changed; the saved note remains retained.");return;}
        if !p.open.get() || p.pending.get()!=0{if start.elapsed()>Duration::from_secs(15){self.error("The scene is still opening. Your reminder is retained.");return;}let a=self.clone();glib::timeout_add_local_once(Duration::from_millis(30),move||a.restore_when_ready(p,location,start,permit));return;}
        if p.composing.get()||p.saving.get(){self.error("Finish the current scene operation before restoring its bookmark.");return;}
        let a=self.clone();p.request(json!({"type":"command","command":"restoreNavigation","bookmark":location,"followCursor":true}),false,move|r|if let Err(e)=r{a.error(&format!("Reminder kept; the old location was not applied. {e}"));});
    }
    pub(super) fn jump_to_passage(self:&Rc<Self>,scene_id:String,revision_id:String,quote:String,start:usize){
        let a=desk::effective(self);let out=a.clone();a.navigate(move||{let next=out.clone();let generation=out.desk.generation.get();let Some(permit)=sequence_host::window_input_permit(&out)else{return;};let permit=Rc::new(permit);out.ensure_pane(&scene_id,move|r|{
            if next.desk.is_closed() || next.desk.generation.get()!=generation || !sequence_host::window_input_allowed(&next,&permit){return;}
            match r{Ok(p)=>{next.show_pane(p.clone(),false);next.locate_passage(p,revision_id,quote,start,std::time::Instant::now(),permit);},Err(e)=>next.error(&e)}
        });});
    }
    fn locate_passage(self:&Rc<Self>,p:Rc<EditorPane>,revision_id:String,quote:String,start:usize,started:std::time::Instant,permit:Rc<sequence_host::WindowInputPermit>){
        if self.desk.is_closed() || self.current_widget()!=Some(p.widget.clone().upcast()){return;}
        if !sequence_host::window_input_allowed(self,&permit)||!self.panes.borrow().get(&p.scene_id).is_some_and(|registered|Rc::ptr_eq(registered,&p)){self.error("Scene input authorization changed; the saved note remains retained.");return;}
        if !p.open.get() || p.pending.get()!=0{if started.elapsed()>Duration::from_secs(15){self.error("Scene is busy. Reopen the passage when ready.");return;}let a=self.clone();glib::timeout_add_local_once(Duration::from_millis(30),move||a.locate_passage(p,revision_id,quote,start,started,permit));return;}
        let document=p.frame.borrow()["document"].clone();let plain=novel_storage::plain_text(&document);
        if p.dirty.get() || *p.parent.borrow()!=revision_id || plain.chars().skip(start).take(quote.chars().count()).collect::<String>()!=quote{self.error("This passage has changed since it was marked. The scene is open; the old selection was not applied.");return;}
        let (Some(anchor),Some(cursor))=(position(&document,start),position(&document,start+quote.chars().count()))else{self.error("The marked selection includes a paragraph boundary. Its scene is open.");return;};
        let a=self.clone();let pane=p.clone();p.request(json!({"type":"command","command":"captureNavigation"}),false,move|r|match r{Ok(v)=>{
            let mut location=v["navigationCapture"].clone();location["anchor"]=json!(anchor);location["cursor"]=json!(cursor);a.restore_when_ready(pane,location,std::time::Instant::now(),permit);
        },Err(e)=>a.error(&e)});
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn reader_offsets_respect_unicode_and_block_separators(){
        let doc=json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"A🦊ع"}]},{"type":"paragraph","content":[{"type":"text","text":"中文"}]}]});
        assert_eq!(position(&doc,2),Some([0,2]));assert_eq!(position(&doc,3),Some([0,3]));assert_eq!(position(&doc,4),None);assert_eq!(position(&doc,5),Some([1,0]));assert_eq!(position(&doc,7),Some([1,2]));assert_eq!(position(&doc,8),None);
    }
}
