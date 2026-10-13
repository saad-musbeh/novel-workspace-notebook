//! Explicit retained manuscript arrangement choices, separate from scene content recovery.
use super::*;
#[derive(Clone)]
struct Object {
    id: String,
    kind: String,
}
fn objects(index: &Value) -> Vec<(String, String, Object)> {
    let mut rows = Vec::new();
    if let Some(pid) = index["project"]["id"].as_str() {
        rows.push((
            format!("project:{pid}"),
            "Novel title and chapter order".into(),
            Object {
                id: pid.into(),
                kind: "project".into(),
            },
        ));
    }
    for (i, c) in index["chapters"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if let Some(id) = c["id"].as_str() {
            rows.push((
                format!("chapter:{id}"),
                format!(
                    "Chapter {}: {} — title and scene order",
                    i + 1,
                    c["title"].as_str().unwrap_or("Untitled")
                ),
                Object {
                    id: id.into(),
                    kind: "chapter".into(),
                },
            ));
        }
    }
    for (i, s) in index["scenes"].as_array().into_iter().flatten().enumerate() {
        if let Some(id) = s["id"].as_str() {
            rows.push((
                format!("scenePlacement:{id}"),
                format!(
                    "Scene {}: {} — chapter and position",
                    i + 1,
                    s["title"].as_str().unwrap_or("Untitled")
                ),
                Object {
                    id: id.into(),
                    kind: "scenePlacement".into(),
                },
            ));
        }
    }
    rows
}
fn item_title(index: &Value, kind: &str, id: &str) -> String {
    let collection = if kind == "project" {
        "chapters"
    } else {
        "scenes"
    };
    index[collection]
        .as_array()
        .into_iter()
        .flatten()
        .find(|r| r["id"].as_str() == Some(id))
        .and_then(|r| r["title"].as_str())
        .unwrap_or("Unavailable or deleted item (history retained)")
        .into()
}
fn describe_metadata(index: &Value, kind: &str, payload: &Value) -> String {
    if kind == "scenePlacement" {
        let destination = payload["chapterId"]
            .as_str()
            .and_then(|id| {
                index["chapters"]
                    .as_array()?
                    .iter()
                    .find(|c| c["id"].as_str() == Some(id))
            })
            .and_then(|c| c["title"].as_str())
            .unwrap_or("Unavailable chapter");
        return format!(
            "Destination chapter: {destination}\nScene position: {}",
            payload["position"]
                .as_u64()
                .and_then(|p| p.checked_add(1))
                .map(|p| p.to_string())
                .unwrap_or_else(|| "Unsupported position".into())
        );
    }
    let mut text = format!(
        "Title: {}",
        payload["title"]
            .as_str()
            .unwrap_or("No title change recorded")
    );
    if let Some(order) = payload["order"].as_array() {
        text.push_str(if kind == "project" {
            "\n\nChapter order:"
        } else {
            "\n\nScene order:"
        });
        for (i, id) in order.iter().filter_map(Value::as_str).enumerate() {
            text.push_str(&format!("\n{}. {}", i + 1, item_title(index, kind, id)));
        }
        text.push_str("\n\nItems omitted from this retained order remain after it in their existing relative order. No chapter or scene content is deleted.");
    }
    text
}
fn replay_needed(kind: &str, heads: &[String], payload: &Value, projection: &Value) -> bool {
    kind == "chapter"
        && heads.len() == 1
        && (payload.get("title") != projection.get("title")
            || payload.get("order") != projection.get("order"))
}
fn placement_blocked(snapshot: &Value, object: &Object, payload: &Value) -> bool {
    if object.kind != "chapter" {
        return false;
    }
    snapshot["placementHeads"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|p| {
            p["heads"].as_array().is_some_and(|h| h.len() > 1)
                && (p["chapterId"].as_str() == Some(object.id.as_str())
                    || payload["order"]
                        .as_array()
                        .is_some_and(|order| order.iter().any(|id| id == &p["sceneId"])))
        })
}
fn captured_heads(value: &Value) -> Result<Vec<String>, String> {
    let rows = value["heads"]
        .as_array()
        .ok_or("Unsupported arrangement snapshot heads.")?;
    let mut seen = std::collections::HashSet::new();
    rows.iter()
        .map(|v| {
            let id = v
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Unsupported arrangement revision.")?;
            if !seen.insert(id) {
                return Err("Duplicate retained arrangement heads.".into());
            }
            Ok(id.to_owned())
        })
        .collect()
}
fn text(parent: &gtk::Box, message: &str) {
    let label = gtk::Label::new(Some(message));
    label.set_xalign(0.);
    label.set_line_wrap(true);
    label.set_selectable(true);
    parent.pack_start(&label, false, false, 0);
}
struct MetadataPage {
    widget: glib::WeakRef<gtk::Box>,
    pid: String,
    objects: gtk::ComboBoxText,
    versions: gtk::ComboBoxText,
    status: gtk::Label,
    current: gtk::TextBuffer,
    selected_preview: gtk::TextBuffer,
    retained: gtk::Box,
    apply: glib::WeakRef<gtk::Button>,
    keep_current: glib::WeakRef<gtk::Button>,
    index: RefCell<Value>,
    choices: RefCell<HashMap<String, Object>>,
    capture: RefCell<Option<Value>>,
    revisions: RefCell<HashMap<String, Value>>,
    selected: RefCell<Option<Value>>,
    generation: Cell<u64>,
    updating: Cell<bool>,
    pending: Cell<bool>,
    remaining: Cell<usize>,
}
impl MetadataPage {
    fn owner(&self) -> Option<Rc<Workspace>> {
        let w = self.widget.upgrade()?;
        let a = desk::workspace_for_widget(w.upcast_ref())?;
        if a.pid.borrow().as_str() == self.pid && a.pages.page_num(&w).is_some() {
            Some(a)
        } else {
            None
        }
    }
    fn object(&self) -> Option<Object> {
        let id = self.objects.active_id()?;
        self.choices.borrow().get(id.as_str()).cloned()
    }
    fn update_apply(&self) {
        let mut enabled = false;
        let mut title = "Use this retained arrangement";
        if !self.pending.get() {
            if let (Some(capture), Some(selected), Some(object)) = (
                self.capture.borrow().as_ref(),
                self.selected.borrow().as_ref(),
                self.object(),
            ) {
                if let Ok(heads) = captured_heads(capture) {
                    if self.remaining.get() != 0
                        || self.revisions.borrow().len() != heads.len()
                        || placement_blocked(capture, &object, &selected["payload"])
                    {
                        enabled = false;
                    } else if heads.len() > 1 {
                        enabled = true;
                    } else if replay_needed(
                        &object.kind,
                        &heads,
                        &selected["payload"],
                        &capture["projection"],
                    ) {
                        enabled = true;
                        title = "Apply retained chapter order";
                    }
                }
            }
        }
        if let Some(b) = self.apply.upgrade() {
            b.set_label(title);
            b.set_sensitive(enabled);
        }
        let can_ack = !self.pending.get()
            && self.remaining.get() == 0
            && match (
                self.capture.borrow().as_ref(),
                self.selected.borrow().as_ref(),
                self.object(),
            ) {
                (Some(c), Some(r), Some(o)) => captured_heads(c).is_ok_and(|heads| {
                    self.revisions.borrow().len() == heads.len()
                        && replay_needed(&o.kind, &heads, &r["payload"], &c["projection"])
                        && !placement_blocked(c, &o, &c["projection"])
                }),
                _ => false,
            };
        if let Some(b) = self.keep_current.upgrade() {
            b.set_sensitive(can_ack);
        }
    }
    fn reset(&self) {
        *self.capture.borrow_mut() = None;
        self.revisions.borrow_mut().clear();
        *self.selected.borrow_mut() = None;
        self.current.set_text("");
        self.selected_preview.set_text("");
        self.updating.set(true);
        self.versions.remove_all();
        self.updating.set(false);
        for child in self.retained.children() {
            self.retained.remove(&child);
        }
        self.update_apply();
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
        self.reset();
        self.objects.set_sensitive(false);
        self.versions.set_sensitive(false);
        self.status
            .set_text("Reading saved manuscript arrangement…");
        let selected = self.objects.active_id().map(|v| v.to_string());
        let p = self.clone();
        a.client.rpc(
            "open_project_index",
            json!({"projectId":self.pid}),
            move |reply| {
                if p.owner().is_none() || p.generation.get() != generation {
                    return;
                }
                match reply {
                    Ok(index) => {
                        let rows = objects(&index);
                        *p.index.borrow_mut() = index;
                        p.choices.borrow_mut().clear();
                        p.updating.set(true);
                        p.objects.remove_all();
                        for (id, title, object) in rows {
                            p.objects.append(Some(&id), &title);
                            p.choices.borrow_mut().insert(id, object);
                        }
                        if !selected
                            .as_deref()
                            .is_some_and(|id| p.objects.set_active_id(Some(id)))
                        {
                            p.objects.set_active(Some(0));
                        }
                        p.updating.set(false);
                        p.objects.set_sensitive(true);
                        p.versions.set_sensitive(true);
                        p.load_object();
                    }
                    Err(e) => {
                        p.objects.set_sensitive(true);
                        p.versions.set_sensitive(true);
                        p.status.set_text(&e)
                    }
                }
            },
        );
    }
    fn load_object(self: &Rc<Self>) {
        if self.updating.get() || self.pending.get() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(object) = self.object() else {
            return;
        };
        self.generation.set(self.generation.get() + 1);
        let generation = self.generation.get();
        self.reset();
        self.versions.set_sensitive(false);
        self.status
            .set_text("Capturing retained heads and placement dependencies…");
        let p = self.clone();
        a.client.rpc("metadata_resolution_snapshot",json!({"projectId":self.pid,"entityId":object.id,"entityType":object.kind}),move|reply|{
            if p.owner().is_none()||p.generation.get()!=generation{return;}
            let capture=match reply{Ok(v)=>v,Err(e)=>{p.versions.set_sensitive(true);p.status.set_text(&e);return;}};let heads=match captured_heads(&capture){Ok(v)=>v,Err(e)=>{p.versions.set_sensitive(true);p.status.set_text(&e);return;}};
            p.current.set_text(&describe_metadata(&p.index.borrow(),&object.kind,&capture["projection"]));*p.capture.borrow_mut()=Some(capture);p.remaining.set(heads.len());
            if heads.is_empty(){p.versions.set_sensitive(true);p.status.set_text("No retained metadata exists for this item.");return;}
            p.status.set_text(if heads.len()>1{"Competing arrangements retained. Inspect each and explicitly choose a complete version. Resolve scene locations before dependent chapter orders."}else{"One retained arrangement. A deferred chapter order can be applied explicitly after its scene locations are resolved."});
            for (i,id) in heads.iter().enumerate(){p.load_revision(generation,&object,id,i+1);}p.update_apply();
        });
    }
    fn load_revision(self: &Rc<Self>, generation: u64, object: &Object, id: &str, number: usize) {
        let Some(a) = self.owner() else {
            return;
        };
        let p = self.clone();
        let object = object.clone();
        let id = id.to_owned();
        a.client.rpc("load_entity_revision",json!({"projectId":self.pid,"entityId":object.id,"entityType":object.kind,"revisionId":id}),move|reply|{
            if p.owner().is_none()||p.generation.get()!=generation{return;}
            match reply{
                Ok(revision)=>{let label=format!("Retained version {number} · {}",revision["createdAt"].as_str().unwrap_or("Saved revision"));p.updating.set(true);p.versions.append(Some(&id),&label);p.updating.set(false);let preview=describe_metadata(&p.index.borrow(),&object.kind,&revision["payload"]);let expander=gtk::Expander::new(Some(&label));let view=gtk::TextView::new();view.set_editable(false);view.set_cursor_visible(false);view.set_wrap_mode(gtk::WrapMode::WordChar);view.buffer().unwrap().set_text(&preview);let scroller=scroll(&view);scroller.set_min_content_height(130);expander.add(&scroller);p.retained.pack_start(&expander,false,false,0);p.retained.show_all();p.revisions.borrow_mut().insert(id,revision);},
                Err(e)=>p.status.set_text(&format!("A retained preview could not be checked. {e}")),
            }p.remaining.set(p.remaining.get()-1);if p.remaining.get()==0{p.versions.set_sensitive(true);}if p.remaining.get()==0&&p.versions.active_id().is_none(){p.versions.set_active(Some(0));}p.update_apply();
        });
    }
    fn select(&self) {
        if self.updating.get() || self.pending.get() {
            return;
        }
        let Some(object) = self.object() else {
            return;
        };
        let revision = self
            .versions
            .active_id()
            .and_then(|id| self.revisions.borrow().get(id.as_str()).cloned());
        if let Some(r) = &revision {
            self.selected_preview.set_text(&describe_metadata(
                &self.index.borrow(),
                &object.kind,
                &r["payload"],
            ));
        }
        *self.selected.borrow_mut() = revision;
        self.update_apply();
    }
    fn resolve(self: &Rc<Self>, current: bool) {
        if self.pending.get() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(capture) = self.capture.borrow().clone() else {
            return;
        };
        let Some(revision) = self.selected.borrow().clone() else {
            return;
        };
        let Some(object) = self.object() else {
            return;
        };
        let heads = match captured_heads(&capture) {
            Ok(v) => v,
            Err(e) => {
                self.status.set_text(&e);
                return;
            }
        };
        let payload = if current {
            capture["projection"].clone()
        } else {
            revision["payload"].clone()
        };
        if current
            && !replay_needed(
                &object.kind,
                &heads,
                &revision["payload"],
                &capture["projection"],
            )
        {
            return;
        }
        if self.remaining.get() != 0 || self.revisions.borrow().len() != heads.len() {
            self.status
                .set_text("Wait for every retained arrangement preview to be checked.");
            return;
        }
        if placement_blocked(&capture, &object, &payload) {
            self.status.set_text(
                "Resolve competing scene locations first, then refresh this chapter order.",
            );
            return;
        }
        if heads.len() < 2
            && !replay_needed(
                &object.kind,
                &heads,
                &revision["payload"],
                &capture["projection"],
            )
        {
            return;
        }
        self.pending.set(true);
        self.update_apply();
        self.objects.set_sensitive(false);
        self.versions.set_sensitive(false);
        self.status
            .set_text("Checking unsaved pages in this novel's windows…");
        let p = self.clone();
        a.confirm_project(move|accepted|{
            let Some(a)=p.owner()else{p.finish("The novel changed. No arrangement was applied.");return;};if !accepted{p.finish("Arrangement choice cancelled. Your drafts remain open.");return;}
            // Do not recapture after approval: the reviewed projection and every
            // placement dependency must still match the transaction's capture.
            let page=p.clone();p.status.set_text("Applying your explicit retained arrangement…");a.client.rpc("resolve_metadata_checked",json!({"snapshot":capture,"payload":payload}),move|reply|{
                match reply{Ok(_)=>{*page.capture.borrow_mut()=None;*page.selected.borrow_mut()=None;page.finish("Arrangement choice saved. Every retained version remains in history. Refresh to inspect dependent chapter orders.");if let Some(a)=page.owner(){a.load_index();a.refresh_projects(Some(a.pid.borrow().clone()));}},Err(e)=>page.finish(&format!("Nothing applied; all retained versions remain. {e} Refresh before choosing a changed arrangement."))}
            });
        });
    }
    fn finish(&self, message: &str) {
        self.pending.set(false);
        self.objects.set_sensitive(true);
        self.versions.set_sensitive(true);
        self.status.set_text(message);
        self.update_apply();
    }
}
fn view(parent: &gtk::Box, title: &str, height: i32) -> gtk::TextBuffer {
    text(parent, title);
    let view = gtk::TextView::new();
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_left_margin(12);
    view.set_right_margin(12);
    let scroller = scroll(&view);
    scroller.set_min_content_height(height);
    parent.pack_start(&scroller, true, true, 0);
    view.buffer().unwrap()
}
impl Workspace {
    pub(super) fn metadata_conflicts(self: &Rc<Self>) {
        let a = desk::effective(self);
        if a.pid.borrow().is_empty() {
            a.error("Choose a novel before inspecting arrangement conflicts.");
            return;
        }
        let out = a.clone();
        a.navigate(move || out.open_metadata_conflicts());
    }
    fn open_metadata_conflicts(self: &Rc<Self>) {
        let panel = self.panel("Manuscript arrangement conflicts");
        let body = panel.content_area();
        text(&body,"Choose retained novel/chapter titles, chapter/scene orders and scene locations explicitly. Resolve competing scene locations first, then refresh and apply compatible chapter orders. Scene text and deleted-scene decisions use their separate Recovery pages.");
        let objects = gtk::ComboBoxText::new();
        body.pack_start(&objects, false, false, 0);
        let status = gtk::Label::new(Some("Reading saved manuscript arrangement…"));
        status.set_xalign(0.);
        status.set_line_wrap(true);
        status.set_selectable(true);
        body.pack_start(&status, false, false, 0);
        let current = view(&body, "Current saved arrangement", 100);
        text(&body,"For a retained chapter order that no longer fits settled scene locations, review the current arrangement above and explicitly keep its membership and order. The old order remains in history.");
        let retained = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.pack_start(&scroll(&retained), true, true, 0);
        text(&body, "Choose a checked retained version");
        let versions = gtk::ComboBoxText::new();
        body.pack_start(&versions, false, false, 0);
        let selected_preview = view(&body, "Your selected retained arrangement", 130);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        body.pack_start(&actions, false, false, 0);
        let apply = gtk::Button::with_label("Use this retained arrangement");
        apply.set_sensitive(false);
        actions.pack_start(&apply, false, false, 0);
        let keep_current = gtk::Button::with_label("Keep current chapter membership and order");
        keep_current.set_sensitive(false);
        actions.pack_start(&keep_current, false, false, 0);
        let refresh = gtk::Button::with_label("Refresh arrangements");
        actions.pack_start(&refresh, false, false, 0);
        let p = Rc::new(MetadataPage {
            widget: panel.widget.downgrade(),
            pid: self.pid.borrow().clone(),
            objects: objects.clone(),
            versions: versions.clone(),
            status,
            current,
            selected_preview,
            retained,
            apply: apply.downgrade(),
            keep_current: keep_current.downgrade(),
            index: RefCell::new(Value::Null),
            choices: RefCell::new(HashMap::new()),
            capture: RefCell::new(None),
            revisions: RefCell::new(HashMap::new()),
            selected: RefCell::new(None),
            generation: Cell::new(0),
            updating: Cell::new(false),
            pending: Cell::new(false),
            remaining: Cell::new(0),
        });
        let weak = Rc::downgrade(&p);
        objects.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.load_object();
            }
        });
        let weak = Rc::downgrade(&p);
        versions.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.select();
            }
        });
        let page = p.clone();
        apply.connect_clicked(move |_| page.resolve(false));
        let page = p.clone();
        keep_current.connect_clicked(move |_| page.resolve(true));
        let page = p.clone();
        refresh.connect_clicked(move |_| page.refresh());
        self.install(&panel.widget, "Manuscript arrangement conflicts", true);
        p.refresh();
    }
}
#[cfg(test)]
#[path = "metadata_conflicts_tests.rs"]
pub(super) mod tests;
