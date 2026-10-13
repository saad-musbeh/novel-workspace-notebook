//! Exact, language-scoped author exceptions; generic settings draft owns persistence.
use super::*;
use std::rc::Weak;
fn field<'a>(v:&'a Value,key:&str,max:usize)->Result<&'a str,String>{v[key].as_str().filter(|s|!s.is_empty()&&s.encode_utf16().count()<=max).ok_or_else(||format!("Invalid spelling exception {key}."))}
fn exceptions(settings:&Value)->Result<Vec<Value>,String>{
    if !settings.is_object()||settings.get("schemaVersion").is_some_and(|v|v!=1){return Err("Unsupported review settings.".into());}
    let Some(assistance)=settings.get("assistance")else{return Ok(Vec::new());};
    if assistance["schemaVersion"]!=1{return Err("Unsupported assistance settings.".into());}
    let rows=assistance["exceptions"].as_array().filter(|a|a.len()<=10000).ok_or("Invalid spelling exception collection.")?;
    for row in rows {field(row,"term",200)?;field(row,"language",50)?;field(row,"reason",1000)?;}
    Ok(rows.clone())
}
fn changed(settings:&Value,term:&str,language:&str,reason:&str,allow:bool)->Result<Value,String>{
    let incoming=json!({"term":term,"language":language,"reason":reason});field(&incoming,"term",200)?;field(&incoming,"language",50)?;
    if term.trim().is_empty()||language.trim().is_empty(){return Err("Enter an exact term and select its language.".into());}
    if allow {field(&incoming,"reason",1000)?;if reason.trim().is_empty(){return Err("Record a reason for this spelling exception.".into());}}
    let mut rows=exceptions(settings)?;let before=rows.len();rows.retain(|e|e["term"]!=term||e["language"]!=language);if !allow&&before==rows.len(){return Err("No exact spelling exception matches this term and language. Nothing changed.".into());}if allow{rows.push(incoming);}if rows.len()>10000{return Err("At most 10,000 exact exceptions are supported.".into());}
    let mut next=settings.clone();if next.get("assistance").is_none(){next["assistance"]=crate::tools::initial("settings")["assistance"].clone();}next["assistance"]["exceptions"]=json!(rows);Ok(next)
}
/// Reproduces retained filterProofreadingExceptions, without changing decisions.
pub(crate) fn filter(suggestions:&[Value],settings:&Value,language:&str)->Result<Vec<Value>,String>{
    if language.is_empty()||language.encode_utf16().count()>50{return Err("Select an exact review language.".into());}
    let exceptions=exceptions(settings)?;
    let terms:std::collections::HashSet<&str>=exceptions.iter().filter(|e|e["language"]==language).map(|e|e["term"].as_str().unwrap()).collect();
    Ok(suggestions.iter().filter(|s|s["origin"]!="proofreading"||!s["evidence"].as_array().into_iter().flatten().any(|a|a["quote"].as_str().is_some_and(|quote|terms.contains(quote)))).cloned().collect())
}
struct Controls {widget:glib::WeakRef<gtk::Box>,controls:glib::WeakRef<gtk::Box>,settings:Weak<DataPage>,pid:String,term:gtk::Entry,language:gtk::ComboBoxText,reason:gtk::Entry,status:gtk::Label,busy:Cell<bool>,composing:Cell<bool>}
#[cfg(test)]
thread_local! {static BEFORE_SETTINGS_REPLY:RefCell<Option<Box<dyn FnOnce()>>>=RefCell::new(None);}
impl Controls {
    fn owner(&self)->Option<Rc<Workspace>> {let w=self.widget.upgrade()?;let controls=self.controls.upgrade()?;if controls.parent().as_ref()!=Some(w.upcast_ref()){return None;}desk::workspace_for_widget(w.upcast_ref()).filter(|a|*a.pid.borrow()==self.pid&&a.pages.page_num(&w).is_some())}
    fn edit(self:&Rc<Self>,allow:bool) {
        if self.busy.get()||self.composing.get(){return;}
        let Some(owner)=self.owner()else{return;};let Some(permit)=sequence_host::window_input_permit(&owner)else{self.status.set_text("Finish the current command sequence before changing exceptions.");return;};
        let Some(settings)=self.settings.upgrade()else{return;};if !settings.loaded.get()||settings.saving.get()||*settings.project_id.borrow()!=self.pid {self.status.set_text("Wait for Review settings to load or finish saving.");return;}
        let term=self.term.text().to_string();let language=self.language.active_id().map(|s|s.to_string()).unwrap_or_default();let reason=self.reason.text().to_string();let base=settings.value.borrow().clone();
        let candidate=match changed(&base,&term,&language,&reason,allow){Ok(v)=>v,Err(e)=>{self.status.set_text(&e);return;}};
        let parent=settings.parent.borrow().clone();let epoch=settings.epoch.get();self.busy.set(true);let page=self.clone();let a=owner.clone();self.status.set_text("Checking saved settings before changing the local exception draft…");
        owner.client.rpc("load_project_data_unambiguous",json!({"projectId":self.pid,"kind":"settings"}),move|reply|{
            page.busy.set(false);
            #[cfg(test)] if let Some(callback)=BEFORE_SETTINGS_REPLY.with(|h|h.borrow_mut().take()){callback();}
            if page.owner().is_none_or(|current|!Rc::ptr_eq(&current,&a)||!sequence_host::window_input_allowed(&current,&permit))||page.composing.get()||page.term.text()!=term||page.reason.text()!=reason||page.language.active_id().as_deref()!=Some(language.as_str())||settings.saving.get()||settings.epoch.get()!=epoch||*settings.parent.borrow()!=parent||*settings.value.borrow()!=base||*settings.project_id.borrow()!=page.pid {page.status.set_text("Settings, entries, ownership or command authorization changed. Pending entries were retained; no exception changed.");return;}
            match reply {Ok(saved) if saved["revisionId"]==parent=>{settings.update(candidate);page.status.set_text("Unsaved spelling exception draft · Save Review settings explicitly to use it. Unrelated settings are preserved.");},Ok(_)=>page.status.set_text("Saved settings changed. Pending entries and the existing draft were retained; reload or resolve settings before retrying."),Err(e)=>page.status.set_text(&format!("Exception unchanged; entries retained. {e}"))}
        });
    }
    fn save(self:&Rc<Self>) {
        if self.busy.get()||self.composing.get(){return;}
        let Some(owner)=self.owner()else{return;};if sequence_host::window_input_permit(&owner).is_none(){return;}let Some(settings)=self.settings.upgrade()else{return;};if *settings.project_id.borrow()!=self.pid{return;}
        let p=self.clone();settings.save(move|r|p.status.set_text(&match r{Ok(()) if p.settings.upgrade().is_some_and(|s|s.dirty.get())=>"Review settings saved; more edits remain unsaved.".into(),Ok(())=>"Review settings saved locally. Exact exceptions affect proofreading only.".into(),Err(e)=>format!("Settings not saved; exception draft and entries retained. {e}")}));
    }
}
impl Workspace {
    pub(crate) fn proofreading_exceptions(self:&Rc<Self>) {
        let a=desk::effective(self);if a.sequence_window_blocked()||a.desk.is_closed(){return;}let pid=a.pid.borrow().clone();if pid.is_empty(){return;}
        let settings=a.page("settings");
        for child in settings.widget.children(){if child.widget_name()=="exact-spelling-exceptions"{settings.widget.remove(&child);}}
        let box_=gtk::Box::new(gtk::Orientation::Vertical,6);box_.set_widget_name("exact-spelling-exceptions");
        let hint=gtk::Label::new(Some("Spelling exceptions · Exact term and selected language only. Allow/Remove changes the unsaved Review settings draft; Save explicitly to use it. Case, accents and Unicode normalization are distinct. Only proofreading issues quoting the exact term are suppressed; LLM and other review issues remain visible."));hint.set_line_wrap(true);hint.set_xalign(0.);box_.pack_start(&hint,false,false,0);
        let term=gtk::Entry::new();term.set_placeholder_text(Some("Exact spelling exception term"));term.set_max_length(200);box_.pack_start(&term,false,false,0);
        let language=gtk::ComboBoxText::new();for (id,name)in LANGUAGES{language.append(Some(id),name);}language.set_active_id(Some("en"));box_.pack_start(&language,false,false,0);
        let reason=gtk::Entry::new();reason.set_placeholder_text(Some("Author's reason for allowing this spelling"));reason.set_max_length(1000);box_.pack_start(&reason,false,false,0);
        let status=gtk::Label::new(Some("Enter the exact term and its language. Existing exceptions are shown in the Review settings form below."));status.set_line_wrap(true);status.set_xalign(0.);box_.pack_start(&status,false,false,0);
        let page=Rc::new(Controls{widget:settings.widget.downgrade(),controls:box_.downgrade(),settings:Rc::downgrade(&settings),pid,term:term.clone(),language:language.clone(),reason:reason.clone(),status,busy:Cell::new(false),composing:Cell::new(false)});
        for (label,allow)in[("Allow exact spelling (unsaved)",true),("Remove exact spelling (unsaved)",false)]{let button=gtk::Button::with_label(label);let p=page.clone();button.connect_clicked(move|_|p.edit(allow));box_.pack_start(&button,false,false,0);}
        let save=gtk::Button::with_label("Save Review settings with exceptions");let p=page.clone();save.connect_clicked(move|_|p.save());box_.pack_start(&save,false,false,0);
        for entry in [&term,&reason]{let weak=Rc::downgrade(&page);entry.connect_preedit_changed(move|_,text|if let Some(p)=weak.upgrade(){p.composing.set(!text.is_empty());});}
        settings.widget.pack_start(&box_,false,false,0);settings.widget.reorder_child(&box_,0);a.install(&settings.widget,"Spelling exceptions · Review settings",false);
    }
}
#[cfg(test)]
#[path="proofreading_exceptions_tests.rs"]
pub(crate) mod tests;
