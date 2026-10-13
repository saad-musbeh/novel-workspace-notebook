//! Project-wide, explicit scene/deletion recovery without an open editor.
use super::*;

fn text(parent: &gtk::Box, message: &str) {
    let label = gtk::Label::new(Some(message));
    label.set_xalign(0.);
    label.set_line_wrap(true);
    label.set_selectable(true);
    parent.pack_start(&label, false, false, 0);
}

#[cfg(test)]
thread_local! {
    static RECOVERY_BEFORE_REPLY: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

fn checked_capture(
    value: &Value,
    pid: &str,
    sid: &str,
) -> Result<novel_storage::SceneConflictSnapshot, String> {
    let capture: novel_storage::SceneConflictSnapshot =
        serde_json::from_value(value.clone()).map_err(|_| "Invalid scene recovery capture.")?;
    if capture.project_id != pid
        || capture.scene_id != sid
        || capture.heads.is_empty()
        || capture.heads.len() > 64
    {
        return Err("Scene recovery capture belongs to a different scene or scope.".into());
    }
    novel_storage::validate(&capture.projected_scene)?;
    let mut seen = std::collections::HashSet::new();
    for head in &capture.heads {
        if head.revision_id.is_empty() || !seen.insert(&head.revision_id) {
            return Err("Invalid or duplicate retained scene head.".into());
        }
        match (&head.scene, head.entity_type.as_str()) {
            (Some(scene), "scene") => novel_storage::validate(scene)?,
            (None, "tombstone") => (),
            _ => return Err("Unsupported retained scene recovery version.".into()),
        }
    }
    Ok(capture)
}
fn writing_choices(c: &novel_storage::SceneConflictSnapshot) -> Vec<(String, String, Value)> {
    let mut choices = vec![(
        "projected".into(),
        if c.deleted {
            "Retained writing before deletion".into()
        } else {
            "Current saved writing".into()
        },
        serde_json::to_value(&c.projected_scene).unwrap(),
    )];
    for (i, h) in c.heads.iter().enumerate() {
        if h.revision_id != c.projected_revision_id {
            if let Some(scene) = &h.scene {
                choices.push((
                    h.revision_id.clone(),
                    format!("Retained writing {} · {}", i + 1, h.created_at),
                    serde_json::to_value(scene).unwrap(),
                ));
            }
        }
    }
    choices
}
fn entry_label(v: &Value) -> String {
    format!(
        "{} · {} · {}",
        v["chapterTitle"].as_str().unwrap_or("Chapter"),
        v["title"].as_str().unwrap_or("Scene"),
        if v["deleted"] == true {
            "deleted; retained history"
        } else if v["headCount"].as_i64().unwrap_or(0) > 1 {
            "competing versions"
        } else {
            "saved writing"
        }
    )
}
fn read_view(parent: &gtk::Box, title: &str, contents: &str, height: i32) -> gtk::TextBuffer {
    text(parent, title);
    let v = gtk::TextView::new();
    v.set_editable(false);
    v.set_cursor_visible(false);
    v.set_wrap_mode(gtk::WrapMode::WordChar);
    v.buffer().unwrap().set_text(contents);
    let s = scroll(&v);
    s.set_min_content_height(height);
    parent.pack_start(&s, true, true, 0);
    v.buffer().unwrap()
}
struct RecoveryPage {
    widget: glib::WeakRef<gtk::Box>,
    pid: String,
    objects: gtk::ComboBoxText,
    versions: gtk::ComboBoxText,
    status: gtk::Label,
    current: gtk::TextBuffer,
    selected: gtk::TextBuffer,
    retained: gtk::Box,
    reviewed: gtk::CheckButton,
    writing: glib::WeakRef<gtk::Button>,
    deletion: glib::WeakRef<gtk::Button>,
    capture: RefCell<Option<Value>>,
    choices: RefCell<HashMap<String, Value>>,
    generation: Cell<u64>,
    updating: Cell<bool>,
    pending: Cell<bool>,
}
impl RecoveryPage {
    fn owner(&self) -> Option<Rc<Workspace>> {
        let w = self.widget.upgrade()?;
        let a = desk::workspace_for_widget(w.upcast_ref())?;
        if a.pid.borrow().as_str() == self.pid && a.pages.page_num(&w).is_some() {
            Some(a)
        } else {
            None
        }
    }
    fn selected_scene(&self) -> Option<Value> {
        self.versions
            .active_id()
            .and_then(|id| self.choices.borrow().get(id.as_str()).cloned())
    }
    fn update(&self) {
        let c = self.capture.borrow();
        let c = c.as_ref();
        let count = c.and_then(|v| v["heads"].as_array()).map_or(0, Vec::len);
        let ready = !self.pending.get() && self.reviewed.is_active();
        if let Some(b) = self.writing.upgrade() {
            b.set_sensitive(
                ready
                    && self.selected_scene().is_some()
                    && (count > 1 || c.is_some_and(|v| v["deleted"] == true)),
            );
        }
        if let Some(b) = self.deletion.upgrade() {
            b.set_sensitive(ready && count > 1);
        }
    }
    fn reset(&self) {
        *self.capture.borrow_mut() = None;
        self.choices.borrow_mut().clear();
        self.reviewed.set_active(false);
        self.current.set_text("");
        self.selected.set_text("");
        self.updating.set(true);
        self.versions.remove_all();
        self.updating.set(false);
        for w in self.retained.children() {
            self.retained.remove(&w);
        }
        self.update();
    }
    fn refresh(self: &Rc<Self>) {
        if self.pending.get() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        self.generation.set(self.generation.get() + 1);
        let generation = self.generation.get();
        let selected = self.objects.active_id();
        self.reset();
        self.objects.set_sensitive(false);
        self.status
            .set_text("Reading every scene, including deleted retained writing…");
        let p = self.clone();
        a.client.rpc(
            "scene_conflict_index",
            json!({"projectId":self.pid}),
            move |r| {
                if p.owner().is_none() || p.generation.get() != generation {
                    return;
                }
                p.objects.set_sensitive(true);
                match r {
                    Ok(v) => {
                        p.updating.set(true);
                        p.objects.remove_all();
                        for entry in v.as_array().into_iter().flatten() {
                            if let Some(id) = entry["sceneId"].as_str() {
                                p.objects.append(Some(id), &entry_label(entry));
                            }
                        }
                        if !selected
                            .as_deref()
                            .is_some_and(|id| p.objects.set_active_id(Some(id)))
                        {
                            p.objects.set_active(Some(0));
                        }
                        p.updating.set(false);
                        if p.objects.active_id().is_some() {
                            p.load();
                        } else {
                            p.status.set_text("This novel has no retained scenes.");
                        }
                    }
                    Err(e) => p.status.set_text(&e),
                }
            },
        );
    }
    fn load(self: &Rc<Self>) {
        if self.updating.get() || self.pending.get() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(sid) = self.objects.active_id().map(|s| s.to_string()) else {
            return;
        };
        self.generation.set(self.generation.get() + 1);
        let generation = self.generation.get();
        self.reset();
        self.versions.set_sensitive(false);
        self.status
            .set_text("Checking complete scene and deletion heads with retained rich writing…");
        let p = self.clone();
        a.client.rpc("scene_conflict_snapshot",json!({"projectId":self.pid,"sceneId":sid}),move|r|{
            if p.owner().is_none()||p.generation.get()!=generation{return;}p.versions.set_sensitive(true);
            let value=match r{Ok(v)=>v,Err(e)=>{p.status.set_text(&e);return;}};let c=match checked_capture(&value,&p.pid,&sid){Ok(c)=>c,Err(e)=>{p.status.set_text(&e);return;}};
            p.current.set_text(&format!("{} · {}\n{}\n\n{}",c.chapter_title,c.projected_scene.title,if c.deleted{"Deleted; last writing remains in history"}else{"Current saved writing"},novel_storage::plain_text(&c.projected_scene.document)));
            for (i,h) in c.heads.iter().enumerate(){let label=format!("Retained version {} · {} · {}",i+1,if h.entity_type=="tombstone"{"deletion"}else{"writing"},h.created_at);let exp=gtk::Expander::new(Some(&label));let body=gtk::Box::new(gtk::Orientation::Vertical,4);let preview=h.scene.as_ref().map(|s|format!("{}\n\n{}",s.title,novel_storage::plain_text(&s.document))).unwrap_or_else(||"Deletion recorded. Immutable writing versions remain in history.".into());read_view(&body,&label,&preview,120);exp.add(&body);p.retained.pack_start(&exp,false,false,0);}p.retained.show_all();
            p.updating.set(true);for(id,label,scene)in writing_choices(&c){p.versions.append(Some(&id),&label);p.choices.borrow_mut().insert(id,scene);}p.versions.set_active(Some(0));p.updating.set(false);*p.capture.borrow_mut()=Some(value);p.select();p.status.set_text(if c.heads.len()>1{"Inspect every retained writing and deletion version, then explicitly review one choice."}else if c.deleted{"One deleted version. You may explicitly restore its retained writing."}else{"This scene has no competing versions. Its writing is unchanged."});
        });
    }
    fn select(&self) {
        if self.updating.get() || self.pending.get() {
            return;
        }
        self.reviewed.set_active(false);
        self.selected.set_text(
            &self
                .selected_scene()
                .map(|s| {
                    format!(
                        "{}\n\n{}",
                        s["title"].as_str().unwrap_or("Scene"),
                        novel_storage::plain_text(&s["document"])
                    )
                })
                .unwrap_or_default(),
        );
        self.update();
    }
    fn finish(&self, message: &str) {
        self.pending.set(false);
        self.objects.set_sensitive(true);
        self.versions.set_sensitive(true);
        self.reviewed.set_sensitive(true);
        self.status.set_text(message);
        self.update();
    }
    fn resolve(self: &Rc<Self>, delete: bool) {
        if self.pending.get() || !self.reviewed.is_active() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(capture) = self.capture.borrow().clone() else {
            return;
        };
        let count = capture["heads"].as_array().map_or(0, Vec::len);
        if count < 2 && (delete || capture["deleted"] != true) {
            return;
        }
        let scene = if delete {
            Value::Null
        } else {
            let Some(s) = self.selected_scene() else {
                return;
            };
            s
        };
        self.pending.set(true);
        self.objects.set_sensitive(false);
        self.versions.set_sensitive(false);
        self.reviewed.set_sensitive(false);
        self.update();
        self.status
            .set_text("Checking unsaved pages in every window of this novel…");
        let p = self.clone();
        a.confirm_project(move|accepted|{
            let Some(a)=p.owner()else{p.finish("Recovery page closed. No choice was applied.");return;};if !accepted{p.finish("Scene recovery cancelled. Your drafts remain open.");return;}
            let page=p.clone();p.status.set_text("Saving the explicit recovery choice against the original complete capture…");a.client.rpc("resolve_scene_conflict_checked",json!({"snapshot":capture,"scene":scene}),move|r|{
                #[cfg(test)]
                RECOVERY_BEFORE_REPLY.with(|hook| {let action=hook.borrow_mut().take();if let Some(action)=action{action();}});
                match r{
                Ok(saved)=>{if let Some(a)=page.owner(){a.invalidate_recovered_scene(&saved);}*page.capture.borrow_mut()=None;page.reviewed.set_active(false);page.finish(if delete{"Deletion choice saved. Every writing version remains in history. Refresh to inspect or restore."}else{"Writing choice saved. Every retained writing and deletion remains in history. Refresh to inspect."});if let Some(a)=page.owner(){a.load_index();}},
                Err(e)=>{if e.starts_with("Conflict"){*page.capture.borrow_mut()=None;page.reviewed.set_active(false);}page.finish(&format!("Nothing applied; all retained writing remains. {e}"));}
                }
            });
        });
    }
}
impl Workspace {
    // A recovery changes the saved head beneath every cached session. Remove only
    // clean, idle sessions synchronously; a later input after draft approval keeps
    // its original parent and draft, so its eventual Save explicitly conflicts.
    fn invalidate_recovered_scene(self: &Rc<Self>, saved: &Value) {
        let Some(sid) = saved["sceneId"].as_str() else {
            return;
        };
        let pid = self.pid.borrow().clone();
        let windows = desk::WINDOWS.with(|windows| windows.borrow().clone());
        for a in windows {
            if a.pid.borrow().as_str() != pid {
                continue;
            }
            let pane = a.panes.borrow().get(sid).cloned();
            let Some(p) = pane else {
                continue;
            };
            if p.dirty.get()
                || p.pending.get() != 0
                || p.saving.get()
                || p.composing.get()
                || !p.open.get()
            {
                p.return_note.set_text("Saved scene recovery changed this scene. Your draft and Undo remain; Save checks its original revision. Reopen after your manual draft decision to see the recovered state.");
                p.return_note.show();
                continue;
            }
            a.panes.borrow_mut().remove(sid);
            if let Some(index) = a.pages.page_num(&p.widget) {
                a.pages.remove_page(Some(index));
            }
            if a.active
                .borrow()
                .as_ref()
                .is_some_and(|active| Rc::ptr_eq(active, &p))
            {
                *a.active.borrow_mut() = None;
            }
            p.process.stop();
            a.status.set_text("Saved scene recovery changed a clean cached scene. Open it again to read the recovered state; its immutable history remains.");
        }
    }
    pub(super) fn scene_conflicts(self: &Rc<Self>) {
        let a = desk::effective(self);
        if a.pid.borrow().is_empty() {
            a.error("Choose a novel before scene recovery.");
            return;
        }
        let out = a.clone();
        a.navigate(move || out.open_scene_conflicts());
    }
    fn open_scene_conflicts(self: &Rc<Self>) {
        let panel = self.panel("Scene writing and deletion recovery");
        let outer = panel.content_area();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        outer.pack_start(&scroll(&body), true, true, 0);
        text(&body,"Inspect retained scene writing and deletion versions, including scenes that cannot be opened. Keep writing or keep deleted only through your explicit reviewed choice. No versions are merged or erased.");
        let objects = gtk::ComboBoxText::new();
        body.pack_start(&objects, false, false, 0);
        let status = gtk::Label::new(Some("Reading scene recovery…"));
        status.set_xalign(0.);
        status.set_line_wrap(true);
        status.set_selectable(true);
        body.pack_start(&status, false, false, 0);
        let current = read_view(&body, "Current saved state and retained writing", "", 140);
        text(
            &body,
            "Every current retained head (checked from the same saved snapshot)",
        );
        let retained = gtk::Box::new(gtk::Orientation::Vertical, 6);
        body.pack_start(&retained, false, false, 0);
        text(&body, "Choose retained writing for Keep writing");
        let versions = gtk::ComboBoxText::new();
        body.pack_start(&versions, false, false, 0);
        let selected = read_view(&body, "Selected writing preview", "", 140);
        let reviewed = gtk::CheckButton::with_label(
            "I reviewed the retained writing, deletion versions and this choice",
        );
        body.pack_start(&reviewed, false, false, 0);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        body.pack_start(&row, false, false, 0);
        let writing = gtk::Button::with_label("Keep writing · save selected version");
        let deletion = gtk::Button::with_label("Keep deleted · acknowledge all versions");
        let refresh = gtk::Button::with_label("Refresh scene recovery");
        for b in [&writing, &deletion, &refresh] {
            row.pack_start(b, false, false, 0);
        }
        let p = Rc::new(RecoveryPage {
            widget: panel.widget.downgrade(),
            pid: self.pid.borrow().clone(),
            objects: objects.clone(),
            versions: versions.clone(),
            status,
            current,
            selected,
            retained,
            reviewed: reviewed.clone(),
            writing: writing.downgrade(),
            deletion: deletion.downgrade(),
            capture: RefCell::new(None),
            choices: RefCell::new(HashMap::new()),
            generation: Cell::new(0),
            updating: Cell::new(false),
            pending: Cell::new(false),
        });
        let weak = Rc::downgrade(&p);
        objects.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.load();
            }
        });
        let weak = Rc::downgrade(&p);
        versions.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.select();
            }
        });
        let weak = Rc::downgrade(&p);
        reviewed.connect_toggled(move |_| {
            if let Some(p) = weak.upgrade() {
                p.update();
            }
        });
        let page = p.clone();
        writing.connect_clicked(move |_| page.resolve(false));
        let page = p.clone();
        deletion.connect_clicked(move |_| page.resolve(true));
        let page = p.clone();
        refresh.connect_clicked(move |_| page.refresh());
        self.install(&panel.widget, "Scene writing and deletion recovery", true);
        p.refresh();
    }
}
#[cfg(test)]
#[path = "scene_conflicts_tests.rs"]
pub(super) mod tests;
