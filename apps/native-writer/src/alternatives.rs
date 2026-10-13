//! Immutable rich paragraph versions; applying one changes only an undoable draft.
use super::*;

fn with_text(block: &Value, text: &str) -> Result<Value, String> {
    if !matches!(block["type"].as_str(), Some("paragraph" | "heading")) {
        return Err("Choose a paragraph or heading.".into());
    }
    if text.chars().count() > 500_000 {
        return Err("An alternative exceeds 500,000 characters.".into());
    }
    let original = novel_storage::plain_text(block);
    if original == text {
        return Ok(block.clone());
    }
    let old = original.chars().collect::<Vec<_>>();
    let new = text.chars().collect::<Vec<_>>();
    let mut prefix = 0;
    while prefix < old.len().min(new.len()) && old[prefix] == new[prefix] {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old.len() - prefix
        && suffix < new.len() - prefix
        && old[old.len() - 1 - suffix] == new[new.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let runs = block["content"].as_array().cloned().unwrap_or_default();
    let slice = |start: usize, end: usize| {
        let mut position = 0;
        let mut result = Vec::new();
        for run in &runs {
            let chars = novel_storage::plain_text(run).chars().collect::<Vec<_>>();
            let left = start.max(position);
            let right = end.min(position + chars.len());
            if right > left {
                let mut part = run.clone();
                if run["type"] == "text" {
                    part["text"] = json!(chars[left - position..right - position]
                        .iter()
                        .collect::<String>());
                }
                result.push(part);
            }
            position += chars.len();
        }
        result
    };
    let mut position = 0;
    let marks = runs
        .iter()
        .find_map(|run| {
            let end = position + novel_storage::plain_text(run).chars().count();
            let found = prefix >= position && prefix <= end;
            position = end;
            if found {
                Some(run.get("marks").cloned().unwrap_or(Value::Null))
            } else {
                None
            }
        })
        .unwrap_or(Value::Null);
    let inserted = new[prefix..new.len() - suffix].iter().collect::<String>();
    let mut content = slice(0, prefix);
    for (i, part) in inserted.split('\n').enumerate() {
        if i > 0 {
            content.push(json!({"type":"hardBreak"}));
        }
        if !part.is_empty() {
            let mut run = json!({"type":"text","text":part});
            if !marks.is_null() {
                run["marks"] = marks.clone();
            }
            content.push(run);
        }
    }
    content.extend(slice(old.len() - suffix, old.len()));
    let mut result = block.clone();
    result["content"] = json!(content);
    novel_storage::validate(&novel_storage::Scene {
        schema_version: 1,
        title: String::new(),
        document: json!({"type":"doc","content":[result]}),
    })?;
    Ok(result)
}
fn group_mut<'a>(value: &'a mut Value, scene: &str, group: &str) -> Result<&'a mut Value, String> {
    value["scenes"]
        .as_array_mut()
        .and_then(|scenes| scenes.iter_mut().find(|s| s["sceneId"] == scene))
        .and_then(|s| s["groups"].as_array_mut())
        .and_then(|groups| groups.iter_mut().find(|g| g["id"] == group))
        .ok_or("Choose a captured paragraph.".into())
}
pub(super) struct AlternativesPage {
    pub widget: gtk::Box,
    pub dirty: Cell<bool>,
    pub saving: Cell<bool>,
    pub composing: Cell<bool>,
    pub loaded: Cell<bool>,
    pub status: gtk::Label,
    pub groups: gtk::ComboBoxText,
    pub versions: gtk::ComboBoxText,
    pub name: gtk::Entry,
    pub text: gtk::TextView,
    pub paragraph: gtk::SpinButton,
    original: gtk::TextView,
    controls: gtk::Box,
    capture_button: gtk::Button,
    rebind_button: gtk::Button,
    apply_button: gtk::Button,
    project_id: String,
    scene_id: String,
    value: RefCell<Value>,
    confirmed: RefCell<Value>,
    parent: RefCell<Value>,
    group_id: RefCell<String>,
    version_id: RefCell<String>,
    updating: Cell<bool>,
    fields_dirty: Cell<bool>,
    metadata_dirty: Cell<bool>,
    dependencies: RefCell<Vec<Value>>,
    applied_pending: Cell<bool>,
}
fn text_view(editable: bool) -> gtk::TextView {
    let view = gtk::TextView::new();
    view.set_editable(editable);
    view.set_cursor_visible(editable);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_left_margin(12);
    view.set_right_margin(12);
    view.set_top_margin(8);
    view.set_bottom_margin(8);
    view
}
fn caption(container: &gtk::Box, text: &str) {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.);
    label.set_line_wrap(true);
    container.pack_start(&label, false, false, 0);
}
impl AlternativesPage {
    fn new(project_id: String, pane: &EditorPane) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        widget.set_margin_start(32);
        widget.set_margin_end(32);
        widget.set_margin_top(26);
        widget.set_margin_bottom(20);
        let heading = gtk::Label::new(Some("Paragraph alternatives"));
        heading.set_xalign(0.);
        heading.style_context().add_class("page-title");
        widget.pack_start(&heading, false, false, 0);
        caption(&widget, &pane.title.borrow());
        caption(&widget,"Keep rich originals and named versions. Apply changes the scene draft; Undo and manual Save still apply.");
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
        widget.pack_start(&controls, true, true, 0);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.pack_start(&row, false, false, 0);
        caption(&row, "Paragraph number");
        let count = pane.frame.borrow()["document"]["content"]
            .as_array()
            .map_or(1, Vec::len)
            .max(1);
        let paragraph = gtk::SpinButton::with_range(1., count as f64, 1.);
        let index = pane.frame.borrow()["position"]["cursor"][0]
            .as_u64()
            .unwrap_or(0);
        paragraph.set_value((index + 1).min(count as u64) as f64);
        row.pack_start(&paragraph, false, false, 0);
        let capture_button = button(&row, "Capture paragraph", || {});
        let rebind_button = button(&row, "Rebind saved group", || {});
        let groups = gtk::ComboBoxText::new();
        caption(&controls, "Captured paragraph");
        controls.pack_start(&groups, false, false, 0);
        let versions = gtk::ComboBoxText::new();
        caption(&controls, "Saved version");
        controls.pack_start(&versions, false, false, 0);
        caption(&controls, "Saved original");
        let original = text_view(false);
        let scroller = scroll(&original);
        scroller.set_min_content_height(100);
        controls.pack_start(&scroller, true, true, 0);
        caption(&controls, "Name for a new version");
        let name = gtk::Entry::new();
        controls.pack_start(&name, false, false, 0);
        caption(
            &controls,
            "Alternative text — unchanged passages retain formatting",
        );
        let text = text_view(true);
        let scroller = scroll(&text);
        scroller.set_min_content_height(140);
        controls.pack_start(&scroller, true, true, 0);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.pack_start(&row, false, false, 0);
        let save_button = button(&row, "Save alternative", || {});
        let apply_button = button(&row, "Apply saved version to draft", || {});
        let discard_button = button(&row, "Discard changes", || {});
        let scene_button = button(&row, "Open scene", || {});
        let status = gtk::Label::new(Some("Loading saved alternatives…"));
        status.set_xalign(0.);
        status.set_line_wrap(true);
        status.set_selectable(true);
        widget.pack_start(&status, false, false, 0);
        let initial = json!({"schemaVersion":1,"scenes":[]});
        let p = Rc::new(Self {
            widget,
            controls,
            capture_button,
            rebind_button,
            apply_button,
            original,
            groups,
            versions,
            name,
            text,
            paragraph,
            status,
            project_id,
            scene_id: pane.scene_id.clone(),
            value: RefCell::new(initial.clone()),
            confirmed: RefCell::new(initial),
            parent: RefCell::new(Value::Null),
            group_id: RefCell::new(String::new()),
            version_id: RefCell::new(String::new()),
            updating: Cell::new(false),
            fields_dirty: Cell::new(false),
            metadata_dirty: Cell::new(false),
            dependencies: RefCell::new(Vec::new()),
            applied_pending: Cell::new(false),
            dirty: Cell::new(false),
            saving: Cell::new(false),
            composing: Cell::new(false),
            loaded: Cell::new(false),
        });
        let weak = Rc::downgrade(&p);
        p.name.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.changed();
            }
        });
        let weak = Rc::downgrade(&p);
        p.text.buffer().unwrap().connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.changed();
            }
        });
        let weak = Rc::downgrade(&p);
        p.name.connect_preedit_changed(move |_, text| {
            if let Some(p) = weak.upgrade() {
                p.composing.set(!text.is_empty());
            }
        });
        let weak = Rc::downgrade(&p);
        p.text.connect_preedit_changed(move |_, text| {
            if let Some(p) = weak.upgrade() {
                p.composing.set(!text.is_empty());
            }
        });
        let weak = Rc::downgrade(&p);
        p.groups.connect_changed(move |combo| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() && !p.dirty.get() {
                    *p.group_id.borrow_mut() =
                        combo.active_id().map(|s| s.to_string()).unwrap_or_default();
                    p.version_id.borrow_mut().clear();
                    p.sync_view();
                }
            }
        });
        let weak = Rc::downgrade(&p);
        p.versions.connect_changed(move |combo| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() && !p.dirty.get() {
                    *p.version_id.borrow_mut() =
                        combo.active_id().map(|s| s.to_string()).unwrap_or_default();
                    p.sync_view();
                }
            }
        });
        for (button, action) in [
            (&p.capture_button, 0),
            (&p.rebind_button, 1),
            (&save_button, 2),
            (&p.apply_button, 3),
            (&discard_button, 4),
            (&scene_button, 5),
        ] {
            let weak = Rc::downgrade(&p);
            button.connect_clicked(move |_| {
                if let Some(p) = weak.upgrade() {
                    if let Some(owner) = p.owner() {
                        match action {
                            0 => p.capture(&owner, false),
                            1 => p.capture(&owner, true),
                            2 => p.save(&owner, |_| {}),
                            3 => p.apply(&owner),
                            4 => {
                                if !p.saving.get() && !p.composing.get() {
                                    p.discard();
                                }
                            }
                            _ => owner.open_scene(&p.scene_id, false),
                        }
                    }
                }
            });
        }
        p.controls.set_sensitive(false);
        p
    }
    fn owner(&self) -> Option<Rc<Workspace>> {
        desk::workspace_for_widget(self.widget.upcast_ref())
            .filter(|a| *a.pid.borrow() == self.project_id)
    }
    fn group(&self) -> Option<Value> {
        self.value.borrow()["scenes"]
            .as_array()?
            .iter()
            .find(|s| s["sceneId"] == self.scene_id)?["groups"]
            .as_array()?
            .iter()
            .find(|g| g["id"] == *self.group_id.borrow())
            .cloned()
    }
    fn selected(&self) -> Option<(Value, Value)> {
        let group = self.group()?;
        let version = group["versions"]
            .as_array()?
            .iter()
            .find(|v| v["id"] == *self.version_id.borrow())?
            .clone();
        Some((group, version))
    }
    fn changed(&self) {
        if !self.updating.get() && self.loaded.get() {
            self.fields_dirty.set(true);
            self.dirty.set(true);
            self.status
                .set_text("Unsaved alternative · Save to keep a new version");
            self.sensitivity();
        }
    }
    fn sensitivity(&self) {
        let ready = self.loaded.get() && !self.saving.get();
        let clean = ready && !self.dirty.get();
        let selected = self.selected();
        let editable = ready
            && !self.metadata_dirty.get()
            && selected
                .as_ref()
                .is_some_and(|(g, _)| g["versions"].as_array().is_some_and(|v| v.len() < 20));
        self.controls.set_sensitive(ready);
        self.groups.set_sensitive(clean);
        self.versions.set_sensitive(clean);
        self.capture_button.set_sensitive(clean);
        self.rebind_button
            .set_sensitive(clean && selected.is_some());
        self.apply_button.set_sensitive(clean && selected.is_some());
        self.text.set_sensitive(editable);
        self.name.set_sensitive(editable);
    }
    fn sync_view(&self) {
        self.updating.set(true);
        self.groups.remove_all();
        let groups = self.value.borrow()["scenes"]
            .as_array()
            .and_then(|s| s.iter().find(|s| s["sceneId"] == self.scene_id))
            .and_then(|s| s["groups"].as_array())
            .cloned()
            .unwrap_or_default();
        for group in &groups {
            self.groups.append(
                group["id"].as_str(),
                group["label"].as_str().unwrap_or("Paragraph"),
            );
        }
        if !groups.iter().any(|g| g["id"] == *self.group_id.borrow()) {
            *self.group_id.borrow_mut() = groups
                .first()
                .and_then(|g| g["id"].as_str())
                .unwrap_or("")
                .into();
            self.version_id.borrow_mut().clear();
        }
        self.groups.set_active_id(Some(&self.group_id.borrow()));
        self.versions.remove_all();
        if let Some(group) = self.group() {
            let versions = group["versions"].as_array().cloned().unwrap_or_default();
            for v in &versions {
                self.versions
                    .append(v["id"].as_str(), v["label"].as_str().unwrap_or("Version"));
            }
            if !versions
                .iter()
                .any(|v| v["id"] == *self.version_id.borrow())
            {
                *self.version_id.borrow_mut() = group["activeId"].as_str().unwrap_or("").into();
            }
            self.versions.set_active_id(Some(&self.version_id.borrow()));
            if let Some((_, version)) = self.selected() {
                self.name
                    .set_text(version["label"].as_str().unwrap_or("Alternative"));
                self.text
                    .buffer()
                    .unwrap()
                    .set_text(&novel_storage::plain_text(&version["block"]));
            }
            let original = versions.iter().find(|v| v["id"] == group["originalId"]);
            self.original.buffer().unwrap().set_text(
                &original
                    .map(|v| novel_storage::plain_text(&v["block"]))
                    .unwrap_or_default(),
            );
        } else {
            self.name.set_text("");
            self.text.buffer().unwrap().set_text("");
            self.original.buffer().unwrap().set_text("");
        }
        self.updating.set(false);
        self.sensitivity();
    }
    fn load(self: &Rc<Self>, owner: &Rc<Workspace>) {
        let weak = Rc::downgrade(self);
        owner.client.rpc(
            "load_project_data",
            json!({"projectId":self.project_id,"kind":"paragraphAlternatives"}),
            move |reply| {
                let Some(p) = weak.upgrade() else {
                    return;
                };
                if p.owner().is_none() {
                    return;
                }
                match reply {
                    Ok(saved) => {
                        let value = if saved.is_null() {
                            json!({"schemaVersion":1,"scenes":[]})
                        } else {
                            saved["payload"].clone()
                        };
                        *p.parent.borrow_mut() = saved["revisionId"].clone();
                        *p.value.borrow_mut() = value.clone();
                        *p.confirmed.borrow_mut() = value;
                        p.loaded.set(true);
                        p.sync_view();
                        p.status.set_text(
                            "Saved locally · Capture a paragraph or choose a saved version",
                        );
                    }
                    Err(e) => p
                        .status
                        .set_text(&format!("Could not load alternatives. {e}")),
                }
            },
        );
    }
    pub fn discard(&self) {
        *self.value.borrow_mut() = self.confirmed.borrow().clone();
        self.fields_dirty.set(false);
        self.metadata_dirty.set(false);
        self.dependencies.borrow_mut().clear();
        self.applied_pending.set(false);
        self.dirty.set(false);
        self.sync_view();
        self.status.set_text(
            "Alternative changes discarded. Scene drafts keep their own Undo and manual Save.",
        );
    }
    fn failed(&self, e: &str) {
        self.saving.set(false);
        self.status.set_text(e);
        self.sensitivity();
    }
    fn commit(
        self: &Rc<Self>,
        owner: &Rc<Workspace>,
        callback: impl FnOnce(Result<(), String>) + 'static,
    ) {
        self.saving.set(true);
        self.metadata_dirty.set(true);
        self.dirty.set(true);
        self.sensitivity();
        self.status.set_text("Saving alternatives…");
        let p = self.clone();
        owner.client.rpc("save_project_data_checked",json!({"projectId":self.project_id,"kind":"paragraphAlternatives","payload":self.value.borrow().clone(),"parent":self.parent.borrow().clone(),"dependencies":self.dependencies.borrow().clone()}),move|reply|match reply{
            Ok(saved)=>{*p.parent.borrow_mut()=saved["revisionId"].clone();*p.value.borrow_mut()=saved["payload"].clone();*p.confirmed.borrow_mut()=saved["payload"].clone();p.saving.set(false);p.fields_dirty.set(false);p.metadata_dirty.set(false);p.dependencies.borrow_mut().clear();p.applied_pending.set(false);p.dirty.set(false);p.sync_view();p.status.set_text("Alternative saved locally. Scene writing still uses manual Save.");callback(Ok(()));},
            Err(e)=>{let prefix=if p.applied_pending.get(){"Version applied to the scene draft, but alternatives were not saved. Undo and manual Save remain available."}else{"Alternatives not saved; values retained."};p.failed(&format!("{prefix} Save to retry or Discard changes. {e}"));callback(Err(e));}
        });
    }
    pub fn save(
        self: &Rc<Self>,
        owner: &Rc<Workspace>,
        callback: impl FnOnce(Result<(), String>) + 'static,
    ) {
        let result = (|| {
            if self.saving.get() {
                return Err("Alternatives are still busy.".to_owned());
            }
            if self.composing.get() {
                return Err("Finish the current input composition.".to_owned());
            }
            if self.owner().is_none() {
                return Err("The alternatives page is no longer in this novel.".to_owned());
            }
            if self.fields_dirty.get() {
                let (group, version) = self.selected().ok_or("Choose a captured paragraph.")?;
                if group["versions"].as_array().map_or(20, Vec::len) >= 20 {
                    return Err("This paragraph already has twenty saved versions.".into());
                }
                let name = self.name.text().to_string();
                if name.chars().count() > 200 {
                    return Err("A version name exceeds 200 characters.".into());
                }
                let name = if name.trim().is_empty() {
                    "Alternative".to_owned()
                } else {
                    name
                };
                let buffer = self.text.buffer().unwrap();
                let text = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), true)
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let block = with_text(&version["block"], &text)?;
                let id = uuid::Uuid::new_v4().to_string();
                let mut value = self.value.borrow().clone();
                group_mut(&mut value, &self.scene_id, &self.group_id.borrow())?["versions"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"id":id,"label":name,"block":block}));
                *self.value.borrow_mut() = value;
                *self.version_id.borrow_mut() = id;
                self.fields_dirty.set(false);
                self.metadata_dirty.set(true);
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.status.set_text(&e);
            callback(Err(e));
            return;
        }
        if !self.dirty.get() {
            callback(Ok(()));
            return;
        }
        self.commit(owner, callback);
    }
    fn ready(&self) -> bool {
        if !self.loaded.get() || self.dirty.get() || self.saving.get() || self.composing.get() {
            self.status.set_text(
                "Save or discard pending alternatives and finish composition before this action.",
            );
            false
        } else {
            true
        }
    }
    fn capture(self: &Rc<Self>, owner: &Rc<Workspace>, rebind: bool) {
        if !self.ready() {
            return;
        }
        if rebind && self.group().is_none() {
            return;
        }
        let Some(action)=sequence_host::tool_action_permit(owner,&self.scene_id)else{self.failed("Finish the scene operation before capturing a paragraph.");return;};
        self.saving.set(true);
        self.sensitivity();
        let p = self.clone();
        let a = owner.clone();
        owner.ensure_pane(&self.scene_id,move|reply|{
            let pane=match reply{Ok(p)=>p,Err(e)=>{p.failed(&e);return;}};
            let Some(permit)=sequence_host::tool_input_after_ensure(&pane,&action)else{p.failed("Paragraph capture authorization changed; existing groups retained.");return;};let permit=Rc::new(permit);
            if pane.dirty.get() || pane.composing.get() || pane.saving.get() || pane.pending.get()!=0{p.failed("Open the scene and save its writing before capturing a paragraph.");return;}
            let out=p.clone();let a_=a.clone();a.client.rpc("load_project_scene",json!({"projectId":p.project_id,"sceneId":p.scene_id}),move|reply|{
                let saved=match reply{Ok(v) if v["revisionId"]==*pane.parent.borrow()=>v,Ok(_)=>{out.failed("The saved scene changed. Reopen a current scene before capturing.");return;},Err(e)=>{out.failed(&e);return;}};
                if !sequence_host::tool_input_allowed(&pane,&permit) || pane.dirty.get() || pane.composing.get() || pane.saving.get() || pane.pending.get()!=0 || pane.frame.borrow()["document"]!=saved["scene"]["document"]{out.failed("The scene draft changed while capturing. Save its current writing first.");return;}
                let number=match out.paragraph.text().parse::<usize>(){Ok(n) if n>0 && n<=saved["scene"]["document"]["content"].as_array().map_or(0,Vec::len)=>n,_=>{out.failed("Enter a whole paragraph number within this scene.");return;}};
                let index=number-1;let p=out.clone();let parent=saved["revisionId"].clone();let captured_pane=pane.clone();let saved_document=saved["scene"]["document"].clone();
                pane.request(json!({"type":"command","command":"captureParagraph","blockIndex":index}),false,move|reply|{
                    let capture=match reply{Ok(v)=>v["paragraphCapture"].clone(),Err(e)=>{p.failed(&e);return;}};
                    if !sequence_host::tool_input_allowed(&captured_pane,&permit) || captured_pane.dirty.get() || captured_pane.composing.get() || captured_pane.saving.get() || captured_pane.pending.get()!=0 || *captured_pane.parent.borrow()!=parent || captured_pane.frame.borrow()["document"]!=saved_document{p.failed("The scene changed before capture finished. Save its current writing first.");return;}
                    let mut value=p.value.borrow().clone();
                    if rebind{if let Ok(group)=group_mut(&mut value,&p.scene_id,&p.group_id.borrow()){group["anchor"]=capture;group["sceneRevisionId"]=parent.clone();}else{p.failed("Choose a saved paragraph group.");return;}}
                    else{let scenes=value["scenes"].as_array_mut().unwrap();let position=scenes.iter().position(|s|s["sceneId"]==p.scene_id).unwrap_or_else(||{scenes.push(json!({"sceneId":p.scene_id,"groups":[]}));scenes.len()-1});let groups=scenes[position]["groups"].as_array_mut().unwrap();if groups.len()>=100{p.failed("This scene already has one hundred captured groups.");return;}
                        let id=uuid::Uuid::new_v4().to_string();let original=uuid::Uuid::new_v4().to_string();groups.push(json!({"id":id,"label":format!("Paragraph {}",index+1),"anchor":capture,"sceneRevisionId":parent,"originalId":original,"activeId":original,"versions":[{"id":original,"label":"Original","block":capture["block"]}]}));*p.group_id.borrow_mut()=id;*p.version_id.borrow_mut()=original;
                    }
                    *p.value.borrow_mut()=value;*p.dependencies.borrow_mut()=vec![json!({"entityId":p.scene_id,"entityType":"scene","revisionId":parent})];p.metadata_dirty.set(true);p.dirty.set(true);p.sync_view();p.commit(&a_,|_|{});
                });
            });
        });
    }
    fn apply(self: &Rc<Self>, owner: &Rc<Workspace>) {
        if !self.ready() {
            return;
        }
        let Some((group, version)) = self.selected() else {
            return;
        };
        let Some(action)=sequence_host::tool_action_permit(owner,&self.scene_id)else{self.failed("Finish the scene operation before applying a version.");return;};
        self.saving.set(true);
        self.sensitivity();
        let p = self.clone();
        let a = owner.clone();
        owner.ensure_pane(&self.scene_id,move|reply|{
            let pane=match reply{Ok(p)=>p,Err(e)=>{p.failed(&e);return;}};
            let Some(permit)=sequence_host::tool_input_after_ensure(&pane,&action)else{p.failed("Version application authorization changed; draft and alternatives retained.");return;};let permit=Rc::new(permit);
            if pane.composing.get() || pane.saving.get() || pane.pending.get()!=0{p.failed("Finish the current scene operation before applying a version.");return;}
            let out=p.clone();let a_=a.clone();a.client.rpc("load_project_scene",json!({"projectId":p.project_id,"sceneId":p.scene_id}),move|reply|{
                match reply{Ok(saved) if saved["revisionId"]==group["sceneRevisionId"] && saved["revisionId"]==*pane.parent.borrow()=>{},Ok(_)=>{out.failed("This scene changed since capture. Save the current writing and explicitly rebind the group before applying.");return;},Err(e)=>{out.failed(&e);return;}}
                if !sequence_host::tool_input_allowed(&pane,&permit) || pane.composing.get() || pane.saving.get() || pane.pending.get()!=0{out.failed("The scene became busy or its authorization changed before applying the version.");return;}
                let p=out.clone();pane.request(json!({"type":"command","command":"swapParagraph","anchor":group["anchor"],"replacement":version["block"]}),true,move|reply|{
                    let capture=match reply{Ok(v)=>v["paragraphCapture"].clone(),Err(e)=>{p.failed(&e);return;}};let mut value=p.value.borrow().clone();
                    if let Ok(target)=group_mut(&mut value,&p.scene_id,&p.group_id.borrow()){target["anchor"]=capture;target["activeId"]=version["id"].clone();}
                    *p.value.borrow_mut()=value;*p.dependencies.borrow_mut()=vec![json!({"entityId":p.scene_id,"entityType":"scene","revisionId":group["sceneRevisionId"]})];p.applied_pending.set(true);p.metadata_dirty.set(true);p.dirty.set(true);p.commit(&a_,|_|{});
                });
            });
        });
    }
}
impl Workspace {
    pub(super) fn alternatives(self: &Rc<Self>) {
        let a = desk::effective(self);
        let Some(pane) = a.context_pane() else {
            a.error("Open a scene before using paragraph alternatives.");
            return;
        };
        let out = a.clone();
        a.navigate(move || {
            let page = AlternativesPage::new(out.pid.borrow().clone(), &pane);
            out.alternative_pages.borrow_mut().push(page.clone());
            out.install(&page.widget, "Paragraph alternatives", true);
            page.load(&out);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alternative_edit_preserves_rich_scalar_runs_and_breaks() {
        let block = json!({"type":"heading","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"مِ 𠀀 ","marks":[{"type":"bold"}]},{"type":"text","text":"old","marks":[{"type":"italic"}]},{"type":"hardBreak"},{"type":"text","text":"中文 👩🏽‍🚀","marks":[{"type":"underline"}]}]});
        assert_eq!(
            with_text(&block, &novel_storage::plain_text(&block)).unwrap(),
            block
        );
        let edited = with_text(&block, "مِ 𠀀 newer\n中文 👩🏽‍🚀").unwrap();
        assert_eq!(novel_storage::plain_text(&edited), "مِ 𠀀 newer\n中文 👩🏽‍🚀");
        assert_eq!(edited["attrs"], block["attrs"]);
        assert_eq!(edited["content"][0], block["content"][0]);
        assert_eq!(
            edited["content"].as_array().unwrap().last(),
            block["content"].as_array().unwrap().last()
        );
    }
    #[test]
    fn alternative_text_retains_combining_marks_and_newline_structure() {
        let block = json!({"type":"paragraph","content":[{"type":"text","text":"a𠀀z","marks":[{"type":"bold"}]}]});
        let edited = with_text(&block, "a👩🏽‍🚀\nمِz").unwrap();
        assert_eq!(novel_storage::plain_text(&edited), "a👩🏽‍🚀\nمِz");
        assert!(edited["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["type"] == "hardBreak"));
        assert_eq!(with_text(&block, "").unwrap()["content"], json!([]));
    }
}
