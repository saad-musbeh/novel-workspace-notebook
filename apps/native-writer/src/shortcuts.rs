//! Saved native shortcuts. Window and editor handlers share one validated map.
use super::*;
use novel_storage::native_shortcuts::{NativeShortcutBinding,NativeShortcutSettings,NATIVE_SHORTCUT_ACTIONS,parse_native_shortcut_list};

pub(super) struct ShortcutRuntime {
    settings:RefCell<NativeShortcutSettings>,
    project_id:RefCell<String>,
    generation:Cell<u64>,
    warning:RefCell<Option<String>>,
    pub loaded:Cell<bool>,
}
impl Default for ShortcutRuntime {
    fn default()->Self{Self{settings:RefCell::new(NativeShortcutSettings::default()),project_id:RefCell::new(String::new()),generation:Cell::new(0),warning:RefCell::new(None),loaded:Cell::new(false)}}
}
impl ShortcutRuntime {
    fn action(&self,pid:&str,scope:&str,binding:&NativeShortcutBinding)->Option<String>{
        if !self.loaded.get()||*self.project_id.borrow()!=pid{return NativeShortcutSettings::default().action(scope,binding).map(str::to_owned);}
        self.settings.borrow().action(scope,binding).map(str::to_owned)
    }
}
fn event_binding(event:&gdk::EventKey)->Option<NativeShortcutBinding>{
    key_binding(event.keyval(),event.state())
}
fn key_binding(key:gdk::keys::Key,state:gdk::ModifierType)->Option<NativeShortcutBinding>{
    let ctrl=state.contains(gdk::ModifierType::CONTROL_MASK);let alt=state.contains(gdk::ModifierType::MOD1_MASK);
    if ctrl&&alt || state.intersects(gdk::ModifierType::SUPER_MASK|gdk::ModifierType::META_MASK|gdk::ModifierType::HYPER_MASK){return None;}
    let code=if key==gdk::keys::constants::Tab||key==gdk::keys::constants::ISO_Left_Tab{"Tab".into()}
    else if key==gdk::keys::constants::Left||key==gdk::keys::constants::Right{if !alt{return None;}if key==gdk::keys::constants::Left{"ArrowLeft".into()}else{"ArrowRight".into()}}
    else if key>=gdk::keys::constants::F1&&key<=gdk::keys::constants::F24{key.name()?.to_string()}
    else if let Some(ch)=key.to_unicode(){
        if ch.is_ascii_alphabetic(){format!("Key{}",ch.to_ascii_uppercase())}
        else if ch.is_ascii_digit(){format!("Digit{ch}")}
        else if let Some(position)=")!@#$%^&*(".chars().position(|symbol|symbol==ch){format!("Digit{position}")}
        else{return None;}
    }else{return None;};
    Some(NativeShortcutBinding{code,ctrl,alt,shift:state.contains(gdk::ModifierType::SHIFT_MASK),meta:false})
}
pub(super) fn publish(pid:&str,settings:&NativeShortcutSettings){
    WINDOWS.with(|windows|for owner in windows.borrow().iter().filter(|a|*a.pid.borrow()==pid&&!a.desk.is_closed()){
        owner.shortcuts.generation.set(owner.shortcuts.generation.get()+1);*owner.shortcuts.project_id.borrow_mut()=pid.into();*owner.shortcuts.settings.borrow_mut()=settings.clone();*owner.shortcuts.warning.borrow_mut()=None;owner.shortcuts.loaded.set(true);
    });
}
pub(super) struct ShortcutsPage {
    pub widget:gtk::Box,pub dirty:Cell<bool>,pub saving:Cell<bool>,pub composing:Cell<bool>,pub loaded:Cell<bool>,pub status:gtk::Label,
    pub entries:Vec<(String,gtk::Entry)>,
    project_id:String,controls:gtk::Box,value:RefCell<Value>,parent:RefCell<Value>,confirmed:RefCell<NativeShortcutSettings>,updating:Cell<bool>,
}
impl ShortcutsPage {
    fn new(project_id:String)->Rc<Self>{
        let widget=gtk::Box::new(gtk::Orientation::Vertical,10);widget.set_margin_start(32);widget.set_margin_end(32);widget.set_margin_top(26);widget.set_margin_bottom(20);
        let title=gtk::Label::new(Some("Keyboard shortcuts"));title.set_xalign(0.);title.style_context().add_class("page-title");widget.pack_start(&title,false,false,0);
        let hint=gtk::Label::new(Some("Enter up to three shortcuts separated by commas, for example Ctrl+Shift+S, F8. Leave blank to disable an action.\nLetters and numbers require Ctrl or Alt. Ctrl+Alt is reserved for AltGr. Normal editing keys and desktop shortcuts stay reserved."));hint.set_xalign(0.);hint.set_line_wrap(true);widget.pack_start(&hint,false,false,0);
        let controls=gtk::Box::new(gtk::Orientation::Vertical,8);widget.pack_start(&controls,true,true,0);let grid=gtk::Grid::new();grid.set_column_spacing(12);grid.set_row_spacing(6);let mut entries=vec![];
        for (row,(id,label)) in NATIVE_SHORTCUT_ACTIONS.iter().enumerate(){let label=gtk::Label::new(Some(label));label.set_xalign(0.);let entry=gtk::Entry::new();entry.set_hexpand(true);entry.set_placeholder_text(Some("Disabled"));label.set_mnemonic_widget(Some(&entry));grid.attach(&label,0,row as i32,1,1);grid.attach(&entry,1,row as i32,1,1);entries.push(((*id).to_owned(),entry));}
        controls.pack_start(&scroll(&grid),true,true,0);let row=gtk::Box::new(gtk::Orientation::Horizontal,8);controls.pack_start(&row,false,false,0);
        let save=button(&row,"Save shortcuts",||{});let reset=button(&row,"Restore defaults",||{});let discard=button(&row,"Discard changes",||{});
        let status=gtk::Label::new(Some("Loading saved shortcuts…"));status.set_xalign(0.);status.set_line_wrap(true);status.set_selectable(true);widget.pack_start(&status,false,false,0);
        let p=Rc::new(Self{widget,controls,status,entries,project_id,value:RefCell::new(json!({"schemaVersion":1})),parent:RefCell::new(Value::Null),confirmed:RefCell::new(NativeShortcutSettings::default()),updating:Cell::new(false),dirty:Cell::new(false),saving:Cell::new(false),composing:Cell::new(false),loaded:Cell::new(false)});
        for (_,entry) in &p.entries{let weak=Rc::downgrade(&p);entry.connect_changed(move|_|if let Some(p)=weak.upgrade(){if p.loaded.get()&&!p.updating.get(){p.dirty.set(true);p.status.set_text("Unsaved shortcuts · Save to use them");}});let weak=Rc::downgrade(&p);entry.connect_preedit_changed(move|_,text|if let Some(p)=weak.upgrade(){p.composing.set(!text.is_empty());});}
        let weak=Rc::downgrade(&p);save.connect_clicked(move|_|if let Some(p)=weak.upgrade(){if let Some(owner)=p.owner(){p.save(&owner,|_|{});}});
        let weak=Rc::downgrade(&p);reset.connect_clicked(move|_|if let Some(p)=weak.upgrade(){if !p.saving.get()&&!p.composing.get(){p.restore(&NativeShortcutSettings::default());p.dirty.set(true);p.status.set_text("Default shortcuts restored in this page · Save to use them");}});
        let weak=Rc::downgrade(&p);discard.connect_clicked(move|_|if let Some(p)=weak.upgrade(){if !p.saving.get()&&!p.composing.get(){p.discard();}});
        p.controls.set_sensitive(false);p
    }
    fn owner(&self)->Option<Rc<Workspace>>{desk::workspace_for_widget(self.widget.upcast_ref()).filter(|a|*a.pid.borrow()==self.project_id)}
    fn restore(&self,settings:&NativeShortcutSettings){self.updating.set(true);for (id,entry) in &self.entries{entry.set_text(&settings.bindings[id].iter().map(NativeShortcutBinding::display).collect::<Vec<_>>().join(", "));}self.updating.set(false);self.dirty.set(false);}
    pub fn discard(&self){self.restore(&self.confirmed.borrow());self.status.set_text("Shortcut changes discarded.");}
    fn load(self:&Rc<Self>,owner:&Rc<Workspace>){
        let weak=Rc::downgrade(self);owner.client.rpc("load_project_data_unambiguous",json!({"projectId":self.project_id,"kind":"settings"}),move|reply|{
            let Some(p)=weak.upgrade()else{return;};if p.owner().is_none(){return;}
            match reply.and_then(|saved|{let value=if saved.is_null(){json!({"schemaVersion":1})}else{saved["payload"].clone()};let settings=NativeShortcutSettings::from_project_settings(&value)?;Ok((saved["revisionId"].clone(),value,settings))}){
                Ok((parent,value,settings))=>{*p.parent.borrow_mut()=parent;*p.value.borrow_mut()=value;*p.confirmed.borrow_mut()=settings.clone();p.restore(&settings);p.loaded.set(true);p.controls.set_sensitive(true);p.status.set_text("Saved locally · Shortcuts apply to this novel's windows");},
                Err(e)=>p.status.set_text(&format!("Shortcut editing unavailable; resolve settings conflicts or reload after recovery. Built-in shortcuts remain available. {e}")),
            }
        });
    }
    pub fn save(self:&Rc<Self>,owner:&Rc<Workspace>,callback:impl FnOnce(Result<(),String>)+'static){
        let result=(||{
            if self.saving.get(){return Err("Shortcuts are still saving.".to_owned());}if self.composing.get(){return Err("Finish entering the shortcut before saving.".to_owned());}
            if !self.loaded.get(){return Err("Wait for the saved shortcuts to load.".to_owned());}if self.owner().is_none()||owner.desk.is_closed()||*owner.pid.borrow()!=self.project_id{return Err("The novel changed. Shortcut edits stay in this page.".to_owned());}
            let bindings=self.entries.iter().map(|(id,entry)|parse_native_shortcut_list(entry.text().as_str()).map(|bindings|(id.clone(),bindings))).collect::<Result<std::collections::BTreeMap<_,_>,_>>()?;
            let settings=NativeShortcutSettings{schema_version:1,bindings};settings.validate()?;Ok(settings)
        })();
        let settings=match result{Ok(s)=>s,Err(e)=>{self.status.set_text(&e);callback(Err(e));return;}};if !self.dirty.get(){callback(Ok(()));return;}
        let mut value=self.value.borrow().clone();value["nativeShortcuts"]=serde_json::to_value(&settings).unwrap();self.saving.set(true);self.controls.set_sensitive(false);self.status.set_text("Saving shortcuts…");let p=self.clone();
        owner.client.rpc("save_project_data_checked",json!({"projectId":self.project_id,"kind":"settings","payload":value,"parent":self.parent.borrow().clone(),"dependencies":[]}),move|reply|{
            p.saving.set(false);p.controls.set_sensitive(true);match reply{
                Ok(saved)=>{*p.parent.borrow_mut()=saved["revisionId"].clone();*p.value.borrow_mut()=saved["payload"].clone();*p.confirmed.borrow_mut()=settings.clone();p.dirty.set(false);publish(&p.project_id,&settings);p.status.set_text("Shortcuts saved locally.");callback(Ok(()));},
                Err(e)=>{p.status.set_text(&format!("Shortcuts not saved; changes retained. {e}"));callback(Err(e));}
            }
        });
    }
}
impl Workspace {
    pub(super) fn optional_preferences_notice(&self)->Option<String>{
        let shortcut=if *self.shortcuts.project_id.borrow()==*self.pid.borrow(){self.shortcuts.warning.borrow().clone()}else{None};
        let notices=shortcut.into_iter().chain(self.desktop_preferences_notice()).collect::<Vec<_>>();if notices.is_empty(){None}else{Some(notices.join("\n"))}
    }
    pub(super) fn fullscreen(&self){
        if self.window.window().is_some_and(|window|window.state().contains(gdk::WindowState::FULLSCREEN)){self.window.unfullscreen();}else{self.window.fullscreen();}
    }
    pub(super) fn reload_shortcuts(self:&Rc<Self>){
        let pid=self.pid.borrow().clone();let generation=self.shortcuts.generation.get()+1;self.shortcuts.generation.set(generation);self.shortcuts.loaded.set(false);*self.shortcuts.project_id.borrow_mut()=pid.clone();*self.shortcuts.settings.borrow_mut()=NativeShortcutSettings::default();*self.shortcuts.warning.borrow_mut()=None;if pid.is_empty(){self.shortcuts.loaded.set(true);return;}let weak=Rc::downgrade(self);
        self.client.rpc("load_project_data_unambiguous",json!({"projectId":pid,"kind":"settings"}),move|reply|{
            let Some(a)=weak.upgrade()else{return;};if a.desk.is_closed()||*a.pid.borrow()!=pid||a.shortcuts.generation.get()!=generation{return;}
            match reply.and_then(|saved|NativeShortcutSettings::from_project_settings(&saved["payload"])){
                Ok(settings)=>{*a.shortcuts.settings.borrow_mut()=settings;a.shortcuts.loaded.set(true);},Err(e)=>{a.shortcuts.loaded.set(true);let notice=format!("Using built-in keyboard shortcuts; custom settings are unavailable. Resolve settings conflicts or reload after recovery. {e}");*a.shortcuts.warning.borrow_mut()=Some(notice.clone());a.error(&notice);},
            }
        });
    }
    pub(super) fn keyboard_shortcuts(self:&Rc<Self>){let a=desk::effective(self);let out=a.clone();a.navigate(move||{let p=ShortcutsPage::new(out.pid.borrow().clone());out.shortcut_pages.borrow_mut().push(p.clone());out.install(&p.widget,"Keyboard shortcuts",true);p.load(&out);});}
    pub(super) fn workspace_shortcut(self:&Rc<Self>,event:&gdk::EventKey)->glib::Propagation{
        let Some(binding)=event_binding(event)else{return glib::Propagation::Proceed;};let Some(action)=self.shortcuts.action(&self.pid.borrow(),"workspace.",&binding)else{return glib::Propagation::Proceed;};
        if self.shortcut_composing(){return glib::Propagation::Stop;}
        match action.as_str(){
            "workspace.save"=>self.save_now(),"workspace.newTab"=>self.new_tab(),"workspace.closeTab"=>{if let Some(widget)=self.current_widget(){self.close_tab(&widget);}},"workspace.find"=>self.find(),"workspace.back"=>self.back(),"workspace.forward"=>self.forward(),"workspace.fullscreen"=>self.fullscreen(),
            "workspace.detachTab"=>self.detach_current(),"workspace.nextTab"|"workspace.previousTab"=>{let count=self.pages.n_pages();if count>1{let current=self.pages.current_page().unwrap_or(0);self.pages.set_current_page(Some(if action=="workspace.nextTab"{(current+1)%count}else{(current+count-1)%count}));}},_=>{}
        }glib::Propagation::Stop
    }
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn built_in_shortcuts_work_without_a_project_and_during_preference_loading(){
        let runtime=ShortcutRuntime::default();let save=NativeShortcutSettings::default().bindings["workspace.save"][0].clone();let new=NativeShortcutSettings::default().bindings["workspace.newTab"][0].clone();
        assert_eq!(runtime.action("","workspace.",&new),Some("workspace.newTab".into()));assert_eq!(runtime.action("project","workspace.",&save),Some("workspace.save".into()));
        runtime.settings.borrow_mut().bindings.insert("workspace.save".into(),vec![]);*runtime.project_id.borrow_mut()="project".into();runtime.loaded.set(true);assert_eq!(runtime.action("project","workspace.",&save),None);assert_eq!(runtime.action("other","workspace.",&save),Some("workspace.save".into()));runtime.loaded.set(false);assert_eq!(runtime.action("project","workspace.",&save),Some("workspace.save".into()));
    }
    #[test]fn only_alt_arrows_enter_shortcut_dispatch_and_editor_navigation_is_preserved(){
        for key in [gdk::keys::constants::Left,gdk::keys::constants::Right]{for modifiers in [gdk::ModifierType::empty(),gdk::ModifierType::CONTROL_MASK,gdk::ModifierType::SHIFT_MASK,gdk::ModifierType::CONTROL_MASK|gdk::ModifierType::SHIFT_MASK,gdk::ModifierType::CONTROL_MASK|gdk::ModifierType::MOD1_MASK]{assert!(key_binding(key,modifiers).is_none());}}
        let defaults=NativeShortcutSettings::default();let back=key_binding(gdk::keys::constants::Left,gdk::ModifierType::MOD1_MASK).unwrap();assert_eq!(defaults.action("workspace.",&back),Some("workspace.back"));assert_eq!(defaults.action("editor.",&back),None);let next=key_binding(gdk::keys::constants::Right,gdk::ModifierType::MOD1_MASK).unwrap();assert_eq!(defaults.action("workspace.",&next),Some("workspace.forward"));let full=key_binding(gdk::keys::constants::F11,gdk::ModifierType::empty()).unwrap();assert_eq!(defaults.action("workspace.",&full),Some("workspace.fullscreen"));
    }
}
/// Call before the editor's old hard-coded shortcut branches; remove those branches.
/// Navigation, IME, ordinary text and key-release processing keep their existing path.
pub(crate) fn editor_shortcut(pane:&Rc<EditorPane>,event:&gdk::EventKey)->Option<glib::Propagation>{
    let owner=desk::workspace_for_widget(pane.widget.upcast_ref())?;let binding=event_binding(event)?;
    let editor=owner.shortcuts.action(&owner.pid.borrow(),"editor.",&binding);
    // A workspace chord bypasses editor navigation/IME and bubbles once to the window.
    if owner.shortcuts.action(&owner.pid.borrow(),"workspace.",&binding).is_some(){return Some(glib::Propagation::Proceed);}
    if editor.is_none()&&binding.alt&&matches!(binding.code.as_str(),"ArrowLeft"|"ArrowRight"){return Some(glib::Propagation::Proceed);}
    let action=editor?;if pane.composing.get()||pane.saving.get(){return Some(glib::Propagation::Stop);}
    match action.as_str(){
        "editor.copy"|"editor.cut"=>{
            // The confirmed frame's selectedText must describe the selection that
            // the ordered worker will remove. Never cut while a newer frame is pending.
            if pane.pending.get()!=0{pane.error.set_text("Wait for the current selection to finish before copying or cutting.");return Some(glib::Propagation::Stop);}
            let text=pane.frame.borrow()["selectedText"].as_str().unwrap_or("").to_owned();if !text.is_empty(){gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).set_text(&text);if action=="editor.cut"{pane.operation(json!({"type":"text","text":""}),true);}}
        },
        "editor.paste"=>{let weak=Rc::downgrade(pane);gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).request_text(move|_,text|if let(Some(p),Some(text))=(weak.upgrade(),text){if !p.composing.get()&&!p.saving.get()&&desk::workspace_for_widget(p.widget.upcast_ref()).is_some(){p.operation(json!({"type":"text","text":text}),true);}});},
        _=>{let command=action.strip_prefix("editor.").unwrap();pane.operation(json!({"type":"command","command":command}),command!="selectAll");}
    }Some(glib::Propagation::Stop)
}
