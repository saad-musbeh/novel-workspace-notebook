//! Temporary read-only peeks. Holding a key never saves, edits or steals writer focus.
use super::*;
use std::rc::Weak;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub(crate) enum PeekKind{References,Changes}
#[derive(Default)]struct HoldState{serial:u64,held:Option<(u64,PeekKind)>}
impl HoldState{
    fn begin(&mut self,kind:PeekKind)->u64{self.serial=self.serial.wrapping_add(1);self.held=Some((self.serial,kind));self.serial}
    fn current(&self,id:u64,kind:PeekKind)->bool{self.held==Some((id,kind))}
    fn release(&mut self,kind:PeekKind)->bool{if self.held.is_some_and(|(_,held)|held==kind){self.cancel();true}else{false}}
    fn cancel(&mut self){self.serial=self.serial.wrapping_add(1);self.held=None;}
}
#[derive(Clone)]struct Capture{
    id:u64,kind:PeekKind,project:String,scene:String,generation:u64,pane:Weak<EditorPane>,
    document:Value,title:String,revision:String,source:Option<String>,dirty:bool,
}
struct View{popover:gtk::Popover,buffer:gtk::TextBuffer,previous:gtk::Button,next:gtk::Button,page:Cell<usize>,changes:RefCell<Option<Comparison>>}
pub(super) struct PeekRuntime{
    state:RefCell<HoldState>,capture:RefCell<Option<Capture>>,view:RefCell<Option<Rc<View>>>,
    comparison:RefCell<HashMap<String,String>>,
}
impl Default for PeekRuntime{fn default()->Self{Self{state:RefCell::new(HoldState::default()),capture:RefCell::new(None),view:RefCell::new(None),comparison:RefCell::new(HashMap::new())}}}
impl PeekRuntime{
    #[cfg(test)]pub(super) fn visible(&self)->bool{self.view.borrow().as_ref().is_some_and(|view|view.popover.is_visible())}
    #[cfg(test)]pub(super) fn text(&self)->String{self.view.borrow().as_ref().map(|view|view.buffer.text(&view.buffer.start_iter(),&view.buffer.end_iter(),true).map(|s|s.to_string()).unwrap_or_default()).unwrap_or_default()}
    #[cfg(test)]pub(super) fn popover(&self)->Option<gtk::Popover>{self.view.borrow().as_ref().map(|view|view.popover.clone())}
    fn hide(&self){let view=self.view.borrow_mut().take();if let Some(view)=view{view.popover.hide();view.popover.set_relative_to(None::<&gtk::Widget>);}}
    fn cancel(&self){self.state.borrow_mut().cancel();self.capture.borrow_mut().take();self.hide();}
    fn release(&self,kind:PeekKind){let released=self.state.borrow_mut().release(kind);if released{self.capture.borrow_mut().take();self.hide();}}
}
#[derive(Clone,Debug,PartialEq)]struct ParagraphChange{index:usize,before:String,after:String,formatting_only:bool}
#[derive(Clone)]struct Comparison{title:String,source:String,dirty:bool,changes:Vec<ParagraphChange>,title_changed:bool}
fn excerpt(text:&str)->String{let mut chars=text.chars();let mut result=chars.by_ref().take(2_000).collect::<String>();if chars.next().is_some(){result.push('…');}result}
fn paragraph_changes(before:&Value,after:&Value)->Vec<ParagraphChange>{
    let earlier=before["content"].as_array().map(Vec::as_slice).unwrap_or(&[]);let current=after["content"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    (0..earlier.len().max(current.len())).filter_map(|index|{
        let before=earlier.get(index);let after=current.get(index);if before==after{return None;}
        let old=before.map(novel_storage::plain_text).unwrap_or_default();let new=after.map(novel_storage::plain_text).unwrap_or_default();Some(ParagraphChange{index,before:excerpt(&old),after:excerpt(&new),formatting_only:old==new})
    }).collect()
}
fn comparison_page(comparison:&Comparison,page:usize)->String{
    let count=comparison.changes.len();let start=page.saturating_mul(12).min(count);let end=(start+12).min(count);
    let mut text=format!("{}\n{}\nEarlier: {}\n{} changed paragraphs{}\nCompared by paragraph position; insertions may shift later comparisons. Each excerpt is limited to 2,000 characters.\n\n",comparison.title,if comparison.dirty{"Unsaved draft captured at key press · Nothing saved or applied"}else{"Acknowledged editor document captured at key press · Nothing saved or applied"},comparison.source,count,if comparison.title_changed{" · Title changed"}else{""});
    if count==0{text.push_str("No paragraph changes against this saved version.\n");}
    for change in &comparison.changes[start..end]{text.push_str(&format!("Paragraph {}{}\n",change.index+1,if change.formatting_only{" · Formatting changed"}else{""}));if !change.formatting_only{text.push_str(&format!("Earlier\n{}\n\nAt key press\n{}\n\n",change.before,change.after));}}
    if count>0{text.push_str(&format!("Showing {}–{} of {} changed paragraphs.\n",start+1,end,count));}text
}
fn valid(owner:&Workspace,capture:&Capture)->bool{
    if owner.desk.is_closed()||*owner.pid.borrow()!=capture.project||owner.desk.generation.get()!=capture.generation||!owner.peek.state.borrow().current(capture.id,capture.kind){return false;}
    let Some(pane)=capture.pane.upgrade()else{return false;};
    !pane.composing.get()&&owner.current_widget()==Some(pane.widget.clone().upcast())&&desk::workspace_for_widget(pane.widget.upcast_ref()).is_some_and(|a|a.window==owner.window)
}
fn render_comparison(view:&View){if let Some(comparison)=view.changes.borrow().as_ref(){view.buffer.set_text(&comparison_page(comparison,view.page.get()));view.previous.set_sensitive(view.page.get()!=0);view.next.set_sensitive((view.page.get()+1)*12<comparison.changes.len());view.previous.set_visible(comparison.changes.len()>12);view.next.set_visible(comparison.changes.len()>12);}}
fn display(owner:&Rc<Workspace>,capture:&Capture,text:&str){
    if !valid(owner,capture){return;}
    if let Some(view)=owner.peek.view.borrow().as_ref(){view.buffer.set_text(text);return;}
    let Some(pane)=capture.pane.upgrade()else{return;};let popover=gtk::Popover::new(Some(&pane.area));popover.set_modal(false);popover.set_can_focus(false);popover.set_position(gtk::PositionType::Right);popover.set_constrain_to(gtk::PopoverConstraint::Window);
    let body=gtk::Box::new(gtk::Orientation::Vertical,6);let label=gtk::Label::new(Some(if capture.kind==PeekKind::References{"Hold F6 · Saved scene references"}else{"Hold F7 · Read-only paragraph comparison"}));label.set_xalign(0.);body.pack_start(&label,false,false,0);
    let text_view=gtk::TextView::new();text_view.set_editable(false);text_view.set_cursor_visible(false);text_view.set_can_focus(false);text_view.set_wrap_mode(gtk::WrapMode::WordChar);text_view.set_left_margin(12);text_view.set_right_margin(12);text_view.set_top_margin(8);text_view.set_bottom_margin(8);let buffer=text_view.buffer().unwrap();buffer.set_text(text);let scroller=scroll(&text_view);scroller.set_can_focus(false);scroller.set_size_request((pane.area.allocated_width()-40).clamp(280,600),(pane.area.allocated_height()-40).clamp(180,500));body.pack_start(&scroller,true,true,0);
    let row=gtk::Box::new(gtk::Orientation::Horizontal,6);body.pack_start(&row,false,false,0);let previous=gtk::Button::with_label("Previous paragraphs");previous.set_can_focus(false);row.pack_start(&previous,false,false,0);let next=gtk::Button::with_label("Next paragraphs");next.set_can_focus(false);row.pack_start(&next,false,false,0);
    let view=Rc::new(View{popover:popover.clone(),buffer,previous,next,page:Cell::new(0),changes:RefCell::new(None)});
    for (button,forward) in [(&view.previous,false),(&view.next,true)]{let weak=Rc::downgrade(&view);button.connect_clicked(move|_|if let Some(view)=weak.upgrade(){let count=view.changes.borrow().as_ref().map_or(0,|comparison|comparison.changes.len());view.page.set(if forward{view.page.get().saturating_add(1).min(count.saturating_sub(1)/12)}else{view.page.get().saturating_sub(1)});render_comparison(&view);});}
    let weak=Rc::downgrade(owner);let id=capture.id;let kind=capture.kind;popover.connect_closed(move|_|if let Some(owner)=weak.upgrade(){let current=owner.peek.state.borrow().current(id,kind);if current{owner.peek.cancel();}});
    popover.add(&body);owner.apply_desktop_to_widget(popover.upcast_ref());*owner.peek.view.borrow_mut()=Some(view.clone());popover.show_all();view.previous.hide();view.next.hide();popover.popup();
}
fn fail(owner:&Rc<Workspace>,capture:&Capture,error:&str){if valid(owner,capture){owner.peek.hide();owner.error(&format!("Read-only peek unavailable. {error}"));}}
impl Workspace{
    pub(super) fn cancel_peek(&self){self.peek.cancel();}
    pub(super) fn begin_peek(self:&Rc<Self>,pane:&Rc<EditorPane>,kind:PeekKind){
        if self.desk.busy.get()||self.desk.is_closed()||pane.composing.get()||pane.pending.get()!=0||pane.saving.get()||!pane.open.get()||self.current_widget()!=Some(pane.widget.clone().upcast()){return;}
        if self.peek.capture.borrow().as_ref().is_some_and(|capture|capture.kind==kind&&capture.scene==pane.scene_id&&valid(self,capture)){return;}
        self.peek.cancel();let id=self.peek.state.borrow_mut().begin(kind);let capture=Capture{id,kind,project:self.pid.borrow().clone(),scene:pane.scene_id.clone(),generation:self.desk.generation.get(),pane:Rc::downgrade(pane),document:if kind==PeekKind::Changes{pane.frame.borrow()["document"].clone()}else{Value::Null},title:pane.title.borrow().clone(),revision:pane.parent.borrow().clone(),source:self.peek.comparison.borrow().get(&pane.scene_id).cloned(),dirty:pane.dirty.get()};*self.peek.capture.borrow_mut()=Some(capture.clone());
        display(self,&capture,"Loading saved read-only information…");match kind{PeekKind::References=>self.peek_references(capture),PeekKind::Changes=>self.peek_comparison(capture)}
    }
    fn peek_references(self:&Rc<Self>,capture:Capture){let owner=self.clone();self.client.rpc("planning_read_snapshot",json!({"projectId":capture.project}),move|reply|{
        if !valid(&owner,&capture){return;}let snapshot=match reply{Ok(snapshot)=>snapshot,Err(error)=>{fail(&owner,&capture,&error);return;}};
        let planning=if snapshot["planning"].is_null(){crate::tools::initial("planning")}else{snapshot["planning"]["payload"].clone()};
        let metadata=if snapshot["sceneMetadata"].is_null(){crate::tools::initial("sceneMetadata")}else{snapshot["sceneMetadata"]["payload"].clone()};
        let scenes=snapshot["index"]["scenes"].as_array().into_iter().flatten().filter_map(|scene|Some((scene["id"].as_str()?.to_owned(),scene["title"].as_str().unwrap_or("Scene").to_owned()))).collect::<Vec<_>>();
        display(&owner,&capture,&scene_reference::references(&planning,&metadata,&scenes,&capture.scene));
    });}
    fn peek_comparison(self:&Rc<Self>,capture:Capture){let selected=capture.source.clone().unwrap_or_else(||capture.revision.clone());let a=self.clone();self.client.rpc("load_entity_revision",json!({"projectId":capture.project,"entityId":capture.scene,"entityType":"scene","revisionId":selected}),move|reply|{
        if !valid(&a,&capture){return;}let saved=match reply{Ok(saved)=>saved,Err(error)=>{fail(&a,&capture,&error);return;}};
        let comparison=Comparison{title:capture.title.clone(),source:format!("{} · {}{}",saved["createdAt"].as_str().unwrap_or("Saved revision"),saved["operation"].as_str().unwrap_or("Edit"),if capture.source.is_none(){" · last acknowledged saved version"}else{" · chosen saved version"}),dirty:capture.dirty,changes:paragraph_changes(&saved["payload"]["document"],&capture.document),title_changed:saved["payload"]["title"].as_str()!=Some(capture.title.as_str())};
        if let Some(view)=a.peek.view.borrow().as_ref(){*view.changes.borrow_mut()=Some(comparison);render_comparison(view);}
    });}
    pub(super) fn choose_peek_revision(self:&Rc<Self>){
        let owner=desk::effective(self);let Some(pane)=owner.context_pane()else{owner.error("Open a scene before choosing a comparison version.");return;};let project=owner.pid.borrow().clone();owner.comparison_sources(project,pane.scene_id.clone(),None);
    }
    fn comparison_sources(self:&Rc<Self>,project:String,scene:String,before:Option<i64>){
        let generation=self.desk.generation.get();let owner=self.clone();self.client.rpc("project_time_points",json!({"projectId":project,"beforeSequence":before,"limit":100}),move|reply|{
            if owner.desk.is_closed()||*owner.pid.borrow()!=project||owner.desk.generation.get()!=generation{return;}let page=match reply{Ok(page)=>page,Err(error)=>{owner.error(&error);return;}};let panel=owner.panel("Comparison version");let body=panel.content_area();let hint=gtk::Label::new(Some("Choose the saved version used while holding F7. Comparisons are read-only; holding never saves or applies writing."));hint.set_xalign(0.);hint.set_line_wrap(true);body.pack_start(&hint,false,false,0);
            let choices=gtk::ComboBoxText::new();choices.append(Some("last"),"Last acknowledged saved version");for revision in page["points"].as_array().into_iter().flatten().filter(|revision|revision["entityType"]=="scene"&&revision["entityId"]==scene){if let Some(id)=revision["revisionId"].as_str(){choices.append(Some(id),&format!("{} · {}",revision["createdAt"].as_str().unwrap_or("Saved revision"),revision["operation"].as_str().unwrap_or("Edit")));}}
            choices.set_active_id(Some("last"));if let Some(selected)=owner.peek.comparison.borrow().get(&scene){choices.set_active_id(Some(selected));}body.pack_start(&choices,false,false,0);
            let status=gtk::Label::new(None);status.set_xalign(0.);body.pack_start(&status,false,false,0);let widget=panel.widget.downgrade();let chosen_project=project.clone();let chosen_scene=scene.clone();button(&body,"Use for hold comparison",move||{
                let Some(widget)=widget.upgrade()else{return;};let Some(owner)=desk::workspace_for_widget(widget.upcast_ref()).filter(|owner|*owner.pid.borrow()==chosen_project)else{return;};let Some(selected)=choices.active_id()else{return;};if selected=="last"{owner.peek.comparison.borrow_mut().remove(&chosen_scene);}else{owner.peek.comparison.borrow_mut().insert(chosen_scene.clone(),selected.to_string());}status.set_text("Comparison source selected for this window. Hold F7 on the writing page.");
            });
            if page["hasMore"]==true{if let Some(before)=page["points"].as_array().and_then(|rows|rows.last()).and_then(|row|row["sequence"].as_i64()){
                let widget=panel.widget.downgrade();button(&body,"Older saved versions",move||{let Some(widget)=widget.upgrade()else{return;};if let Some(owner)=desk::workspace_for_widget(widget.upcast_ref()).filter(|owner|*owner.pid.borrow()==project){owner.comparison_sources(project.clone(),scene.clone(),Some(before));}});
            }}panel.show_all();
        });
    }

}
fn key_kind(key:gdk::keys::Key)->Option<PeekKind>{if key==gdk::keys::constants::F6{Some(PeekKind::References)}else if key==gdk::keys::constants::F7{Some(PeekKind::Changes)}else{None}}
pub(crate) fn key_press(pane:&Rc<EditorPane>,event:&gdk::EventKey)->Option<glib::Propagation>{let kind=key_kind(event.keyval())?;if event.state().intersects(gdk::ModifierType::CONTROL_MASK|gdk::ModifierType::MOD1_MASK|gdk::ModifierType::SHIFT_MASK|gdk::ModifierType::SUPER_MASK|gdk::ModifierType::META_MASK){return None;}if let Some(owner)=desk::workspace_for_widget(pane.widget.upcast_ref()){owner.begin_peek(pane,kind);}Some(glib::Propagation::Stop)}
pub(crate) fn key_release(pane:&Rc<EditorPane>,event:&gdk::EventKey)->Option<glib::Propagation>{let kind=key_kind(event.keyval())?;cancel_for_pane(pane,Some(kind));Some(glib::Propagation::Stop)}
pub(crate) fn cancel_for_pane(pane:&Rc<EditorPane>,kind:Option<PeekKind>){let owners=WINDOWS.with(|windows|windows.borrow().iter().cloned().collect::<Vec<_>>());for owner in owners{let matches=owner.peek.capture.borrow().as_ref().is_some_and(|capture|capture.pane.upgrade().is_some_and(|p|Rc::ptr_eq(&p,pane)));if matches{if let Some(kind)=kind{owner.peek.release(kind);}else{owner.peek.cancel();}}}}

#[cfg(test)]mod tests{use super::*;
    #[test]fn released_or_replaced_hold_rejects_late_replies(){let mut state=HoldState::default();let first=state.begin(PeekKind::References);assert!(state.current(first,PeekKind::References));assert!(!state.release(PeekKind::Changes));assert!(state.release(PeekKind::References));assert!(!state.current(first,PeekKind::References));let next=state.begin(PeekKind::Changes);assert!(!state.current(first,PeekKind::References));state.cancel();assert!(!state.current(next,PeekKind::Changes));}
    #[test]fn positional_comparison_preserves_unicode_and_detects_formatting_only_changes(){let before=json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl"},"content":[{"type":"text","text":"مِ 中文 𠀀 👩🏽‍🚀 e\u{301}","marks":[{"type":"bold"}]}]}]});let mut after=before.clone();after["content"][0]["content"][0]["marks"]=json!([{"type":"underline"}]);let changes=paragraph_changes(&before,&after);assert_eq!(changes.len(),1);assert!(changes[0].formatting_only);after["content"][0]["content"][0]["text"]=json!("New مِ 中文");let changes=paragraph_changes(&before,&after);assert!(!changes[0].formatting_only);assert_eq!(changes[0].before,"مِ 中文 𠀀 👩🏽‍🚀 e\u{301}");assert_eq!(changes[0].after,"New مِ 中文");}
    #[test]fn comparison_limits_each_scalar_excerpt_and_twelve_rows_per_page(){let old=json!({"type":"doc","content":[]});let new=json!({"type":"doc","content":(0..25).map(|i|json!({"type":"paragraph","content":[{"type":"text","text":format!("{i}{}","𠀀".repeat(2100))}]})).collect::<Vec<_>>()});let changes=paragraph_changes(&old,&new);assert_eq!(changes.len(),25);assert_eq!(changes[0].after.chars().count(),2001);let comparison=Comparison{title:"Scene".into(),source:"Version".into(),dirty:true,changes,title_changed:false};let page=comparison_page(&comparison,0);assert!(page.contains("Paragraph 12"));assert!(!page.contains("Paragraph 13"));assert!(page.contains("Unsaved draft"));assert!(comparison_page(&comparison,2).contains("Paragraph 25"));}
}
