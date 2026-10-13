//! Author-selected local dictionary alternatives applied to one exact live draft.
use super::*;
use sha2::{Digest,Sha256};
use std::rc::Weak;
fn hash(document:&Value)->String {format!("{:x}",Sha256::digest(serde_json::to_vec(document).unwrap()))}
fn position(value:&Value)->Result<[usize;2],String>{let a=value.as_array().filter(|a|a.len()==2).ok_or("Select a word in one paragraph first.")?;Ok([a[0].as_u64().ok_or("Invalid selection.")? as usize,a[1].as_u64().ok_or("Invalid selection.")? as usize])}
fn replacement(document:&Value,selection:&Value,term:&str,word:&str)->Result<(usize,Value),String>{
    novel_editor::model::validate_text(word)?;
    if word.is_empty()||word.chars().count()>1000||word.contains(['\n','\r']) {return Err("This alternative needs manual editing.".into());}
    let cursor=position(&selection["cursor"])?;let anchor=position(&selection["anchor"])?;let left=cursor.min(anchor);let right=cursor.max(anchor);
    if left[0]!=right[0]||left==right{return Err("Select the exact lookup text within one paragraph first.".into());}
    let block=document["content"].as_array().and_then(|a|a.get(left[0])).ok_or("The selected paragraph is unavailable.")?;
    let text=novel_storage::plain_text(block);let chars:Vec<char>=text.chars().collect();
    if right[1]>chars.len()||chars[left[1]..right[1]].iter().collect::<String>()!=term{return Err("The selected text does not exactly match the lookup term. Case, accents and Unicode forms remain distinct.".into());}
    let mut content=novel_editor::model::slice_runs(block,0,left[1]);
    let mut node=novel_editor::model::slice_runs(block,left[1],right[1]).into_iter().find(|n|n["type"]=="text").ok_or("Select text rather than a paragraph break.")?;
    node["text"]=json!(word);content.push(node);content.extend(novel_editor::model::slice_runs(block,right[1],chars.len()));
    let mut edited=block.clone();edited["content"]=json!(content);let mut next=document.clone();next["content"][left[0]]=edited.clone();
    novel_storage::validate(&novel_storage::Scene{schema_version:1,title:String::new(),document:next})?;
    Ok((left[0],edited))
}
fn pane_owner(pane:&Rc<EditorPane>,pid:&str)->Option<Rc<Workspace>> {
    let owners=WINDOWS.with(|windows|windows.borrow().iter().filter(|a|!a.desk.is_closed()&&*a.pid.borrow()==pid&&a.panes.borrow().get(&pane.scene_id).is_some_and(|p|Rc::ptr_eq(p,pane))).cloned().collect::<Vec<_>>());
    (owners.len()==1).then(||owners[0].clone())
}
fn ready(pane:&EditorPane)->bool {pane.open.get()&&!pane.composing.get()&&!pane.saving.get()&&pane.pending.get()==0}
fn exact(pane:&EditorPane,parent:&str,document:&Value,selection:&Value)->bool {ready(pane)&&*pane.parent.borrow()==parent&&pane.frame.borrow()["document"]==*document&&pane.frame.borrow()["position"]==*selection}
fn sensitive(button:&glib::WeakRef<gtk::Button>,value:bool){if let Some(b)=button.upgrade(){b.set_sensitive(value);}}
struct Preview {
    widget:glib::WeakRef<gtk::Box>,pid:String,pane:Weak<EditorPane>,parent:String,document:Value,selection:Value,anchor:Value,edited:Value,
    status:gtk::Label,apply:glib::WeakRef<gtk::Button>,undo:glib::WeakRef<gtk::Button>,cancel:glib::WeakRef<gtk::Button>,busy:Cell<bool>,applied:Cell<bool>,after:RefCell<Option<Value>>,
}
impl Preview {
    fn owner(&self)->Option<Rc<Workspace>> {let w=self.widget.upgrade()?;desk::workspace_for_widget(w.upcast_ref()).filter(|a|*a.pid.borrow()==self.pid&&a.pages.page_num(&w).is_some())}
    fn apply(self:&Rc<Self>) {
        if self.busy.get()||self.applied.get(){return;}
        let Some(page_owner)=self.owner() else{return;};let Some(window)=sequence_host::window_input_permit(&page_owner)else{return;};
        let Some(pane)=self.pane.upgrade()else{self.status.set_text("The captured draft closed. Capture a new passage.");return;};
        let Some(owner)=pane_owner(&pane,&self.pid)else{self.status.set_text("The captured draft has no unique owner. Capture a new passage.");return;};
        let Some(input)=sequence_host::tool_input_permit(&owner,&pane)else{self.status.set_text("Finish the current scene operation before Apply.");return;};
        if !exact(&pane,&self.parent,&self.document,&self.selection){self.status.set_text("The rich draft, selection or saved parent changed. Capture a new passage; no edit applied.");return;}
        self.busy.set(true);sensitive(&self.apply,false);let p=self.clone();let writing=pane.clone();
        pane.request(json!({"type":"command","command":"captureParagraph","blockIndex":self.anchor["blockIndex"]}),false,move|reply|{
            let result=reply.and_then(|v|if v["paragraphCapture"]==p.anchor{Ok(())}else{Err("The captured paragraph changed; no edit applied.".into())});
            if let Err(e)=result {p.busy.set(false);sensitive(&p.apply,true);p.status.set_text(&e);return;}
            if p.owner().is_none_or(|a|!sequence_host::window_input_allowed(&a,&window))||!sequence_host::tool_input_allowed(&writing,&input)||!exact(&writing,&p.parent,&p.document,&p.selection){p.busy.set(false);sensitive(&p.apply,true);p.status.set_text("Apply authorization or passage changed; no edit applied.");return;}
            writing.saving.set(true);let out=p.clone();let pane=writing.clone();
            writing.request(json!({"type":"command","command":"swapParagraph","anchor":p.anchor,"replacement":p.edited}),true,move|reply|{
                pane.saving.set(false);out.busy.set(false);
                match reply {Ok(_)=>{out.applied.set(true);*out.after.borrow_mut()=Some(pane.frame.borrow()["document"].clone());sensitive(&out.undo,true);if let Some(cancel)=out.cancel.upgrade(){cancel.set_label("Close preview (keep unsaved edit)");}out.status.set_text("Applied to the unsaved draft. Save the scene manually to retain it, or Undo this replacement.");},Err(e)=>{sensitive(&out.apply,true);out.status.set_text(&format!("No replacement applied. {e}"));}}
            });
        });
    }
    fn undo(self:&Rc<Self>) {
        if self.busy.get()||!self.applied.get(){return;}
        let Some(page)=self.owner()else{return;};let Some(window)=sequence_host::window_input_permit(&page)else{return;};let Some(pane)=self.pane.upgrade()else{return;};let Some(owner)=pane_owner(&pane,&self.pid)else{return;};let Some(input)=sequence_host::tool_input_permit(&owner,&pane)else{return;};
        if !ready(&pane)||*pane.parent.borrow()!=self.parent||self.after.borrow().as_ref()!=Some(&pane.frame.borrow()["document"])||!sequence_host::tool_input_allowed(&pane,&input)||!sequence_host::window_input_allowed(&page,&window){self.status.set_text("The draft changed after Apply. Use the scene's ordinary Undo history; this shortcut will not undo another edit.");return;}
        self.busy.set(true);pane.saving.set(true);let out=self.clone();let writing=pane.clone();
        pane.request(json!({"type":"command","command":"undo"}),true,move|reply|{writing.saving.set(false);out.busy.set(false);match reply{Ok(_)=>{sensitive(&out.undo,false);if let Some(cancel)=out.cancel.upgrade(){cancel.set_label("Close preview");}out.status.set_text("Replacement undone in the unsaved draft. Earlier Undo history remains available.");},Err(e)=>out.status.set_text(&e)}});
    }
}
pub(crate) fn open(owner:&Rc<Workspace>,original:Weak<EditorPane>,term:String,word:String,provenance:String) {
    let pid=owner.pid.borrow().clone();let Some(window)=sequence_host::window_input_permit(owner)else{return;};
    let Some(pane)=original.upgrade()else{owner.error("Open and select the exact word in the original scene before previewing a synonym.");return;};
    let Some(source_owner)=pane_owner(&pane,&pid)else{owner.error("The original scene has no unique live owner. Open it before previewing a synonym.");return;};
    let Some(action)=sequence_host::tool_action_permit(&source_owner,&pane.scene_id)else{owner.error("Finish the current scene operation before capturing a passage.");return;};
    let Some(input)=sequence_host::tool_input_after_ensure(&pane,&action)else{return;};
    if !ready(&pane){owner.error("Finish typing or composition before capturing a passage.");return;}
    let document=pane.frame.borrow()["document"].clone();let selection=pane.frame.borrow()["position"].clone();let parent=pane.parent.borrow().clone();
    let (index,edited)=match replacement(&document,&selection,&term,&word){Ok(v)=>v,Err(e)=>{owner.error(&e);return;}};
    let a=owner.clone();let writing=pane.clone();pane.request(json!({"type":"command","command":"captureParagraph","blockIndex":index}),false,move|reply|{
        let capture=match reply {Ok(v)=>v["paragraphCapture"].clone(),Err(e)=>{a.error(&e);return;}};
        if !sequence_host::window_input_allowed(&a,&window)||!sequence_host::tool_input_allowed(&writing,&input)||!exact(&writing,&parent,&document,&selection)||capture["documentHash"]!=hash(&document)||capture["block"]!=document["content"][index]{a.error("The passage changed while capturing. Select the exact word and try again.");return;}
        let panel=a.panel("Synonym in captured passage");let body=panel.content_area();
        for message in [provenance,format!("Selected text: {term}\nChosen dictionary alternative: {word}"),"Meaning and part of speech are supplied by the selected local dictionary sense. Adjust inflection or grammar manually if needed. This preview makes no linguistic inference and does not save the scene.".into(),format!("Original paragraph:\n{}\n\nProposed paragraph:\n{}",novel_storage::plain_text(&capture["block"]),novel_storage::plain_text(&edited))] {let label=gtk::Label::new(Some(&message));label.set_xalign(0.);label.set_line_wrap(true);label.set_selectable(true);body.pack_start(&label,false,false,0);}
        let status=gtk::Label::new(Some("Preview only · Apply is an unsaved edit with ordinary Undo."));status.set_xalign(0.);status.set_line_wrap(true);body.pack_start(&status,false,false,0);
        let apply=gtk::Button::with_label("Apply to unsaved draft");let undo=gtk::Button::with_label("Undo this replacement");undo.set_sensitive(false);let cancel=gtk::Button::with_label("Cancel preview");for b in [&apply,&undo,&cancel]{body.pack_start(b,false,false,0);}
        let page=Rc::new(Preview{widget:panel.widget.downgrade(),pid,pane:Rc::downgrade(&writing),parent,document,selection,anchor:capture,edited,status,apply:apply.downgrade(),undo:undo.downgrade(),cancel:cancel.downgrade(),busy:Cell::new(false),applied:Cell::new(false),after:RefCell::new(None)});
        let p=page.clone();apply.connect_clicked(move|_|p.apply());let p=page.clone();undo.connect_clicked(move|_|p.undo());let weak=panel.widget.downgrade();cancel.connect_clicked(move|_|if let Some(w)=weak.upgrade(){if let Some(a)=desk::workspace_for_widget(w.upcast_ref()){a.close_tab(w.upcast_ref());}});
        a.install(&panel.widget,"Synonym in captured passage",false);
    });
}
#[cfg(test)]
#[path="synonym_context_tests.rs"]
pub(crate) mod tests;
