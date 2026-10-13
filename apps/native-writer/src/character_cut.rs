//! A manuscript-order navigation cut through explicit authored character links.
use super::*;
use std::collections::HashSet;

fn rows<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn name(value: &Value) -> String { value["name"].as_str().unwrap_or("Character").to_owned() }

/// Same canonical membership rules as retained book-model.ts. Neither aliases
/// nor manuscript text are inspected, and missing scene targets are excluded.
fn character_scenes(planning: &Value, scenes: &[(String, String)], character: &str) -> Vec<(String, String)> {
    let Some(record) = rows(planning, "records").iter().find(|r| r["id"].as_str() == Some(character) && r["kind"] == "character") else { return Vec::new(); };
    let events: HashMap<&str, &Value> = rows(planning, "events").iter().filter_map(|e| Some((e["id"].as_str()?, e))).collect();
    let mut scene_ids: HashSet<String> = HashSet::new();
    let mut event_ids: HashSet<String> = HashSet::new();
    let mut link = |link: &Value| {
        let Some(id) = link["id"].as_str() else { return; };
        if link["kind"] == "scene" { scene_ids.insert(id.to_owned()); }
        else if link["kind"] == "event" { event_ids.insert(id.to_owned()); }
    };
    for item in rows(record, "links") { link(item); }
    // Link-valued custom fields are equivalent to explicit record links.
    for value in record["values"].as_object().into_iter().flat_map(|v| v.values()) {
        for item in value.as_array().into_iter().flatten() { link(item); }
    }
    let mut fact_ids = HashSet::new();
    for fact in rows(planning, "facts").iter().filter(|f| f["subjectId"].as_str() == Some(character)) {
        if let Some(id) = fact["id"].as_str() { fact_ids.insert(id); }
        for key in ["eventId", "untilEventId"] { if let Some(id) = fact[key].as_str() { event_ids.insert(id.to_owned()); } }
    }
    for reveal in rows(planning, "readerReveals") {
        if reveal["factId"].as_str().is_some_and(|id| fact_ids.contains(id)) {
            if let Some(id) = reveal["scene"]["id"].as_str() { scene_ids.insert(id.to_owned()); }
        }
    }
    for entry in rows(planning, "knowledge").iter().filter(|k| k["characterId"].as_str() == Some(character)) {
        let scope = &entry["scope"];
        if scope["kind"] == "scenes" {
            for scene in rows(scope, "sceneIds").iter().filter_map(Value::as_str) { scene_ids.insert(scene.to_owned()); }
        } else if scope["kind"] == "events" {
            for key in ["fromEventId", "untilEventId"] { if let Some(id) = scope[key].as_str() { event_ids.insert(id.to_owned()); } }
        }
    }
    for id in event_ids {
        if let Some(event) = events.get(id.as_str()) {
            for scene in rows(event, "sceneIds").iter().filter_map(Value::as_str) { scene_ids.insert(scene.to_owned()); }
        }
    }
    scenes.iter().filter(|(id, _)| scene_ids.contains(id)).cloned().collect()
}

fn page_owner(origin: &Rc<Workspace>, widget: &gtk::Box, pid: &str) -> Option<Rc<Workspace>> {
    let mut result = None;
    desk::with_button_owner(widget.upcast_ref(), || {
        let a = desk::effective(origin);
        if !a.desk.is_closed() && *a.pid.borrow() == pid && a.pages.page_num(widget).is_some() { result = Some(a); }
    }); result
}

struct CharacterCutPage {
    panel: Rc<Panel>,
    planning: RefCell<Value>,
    scenes: RefCell<Vec<(String, String)>>,
    matched: RefCell<Vec<(String, String)>>,
    character: gtk::ComboBoxText,
    scene: gtk::ComboBoxText,
    status: gtk::Label,
    previous: gtk::Button,
    next: gtk::Button,
    loading: Cell<bool>,
    changing: Cell<bool>,
    loaded: Cell<bool>,
}
impl CharacterCutPage {
    fn render(&self) {
        if self.loading.get() || self.changing.get() || !self.loaded.get() { return; }
        let character = self.character.active_id();
        let cut = character.as_deref().map(|id| character_scenes(&self.planning.borrow(), &self.scenes.borrow(), id)).unwrap_or_default();
        let chosen = self.scene.active_id();
        self.changing.set(true); self.scene.remove_all();
        for (i, (id, title)) in cut.iter().enumerate() { self.scene.append(Some(id), &format!("{}. {title}", i + 1)); }
        if chosen.as_deref().is_some_and(|id| cut.iter().any(|(scene, _)| scene == id)) { self.scene.set_active_id(chosen.as_deref()); }
        else { self.scene.set_active(Some(0)); }
        *self.matched.borrow_mut() = cut; self.changing.set(false); self.selection();
    }
    fn selection(&self) {
        if self.loading.get() || self.changing.get() || !self.loaded.get() { return; }
        let cut = self.matched.borrow();
        let index = self.scene.active_id().and_then(|id| cut.iter().position(|(scene, _)| scene == id.as_str()));
        self.previous.set_sensitive(index.is_some_and(|i| i > 0));
        self.next.set_sensitive(index.is_some_and(|i| i + 1 < cut.len()));
        self.status.set_text(&if self.character.active_id().is_none() { "No character records are available. Add an authored character in Story notes.".into() }
            else if let Some(i) = index { format!("{} / {} · {} · Saved authored links", i + 1, cut.len(), cut[i].1) }
            else { "No current scenes are explicitly linked to this character. Missing scene targets are excluded; prose is not searched.".into() });
    }
    fn step(&self, offset: isize) {
        if self.loading.get() || !self.loaded.get() { return; }
        let cut = self.matched.borrow();
        let Some(i) = self.scene.active_id().and_then(|id| cut.iter().position(|(scene, _)| scene == id.as_str())) else { return; };
        if let Some(next) = i.checked_add_signed(offset).filter(|i| *i < cut.len()) { self.scene.set_active_id(Some(&cut[next].0)); }
    }
    fn selected(&self) -> Option<String> {
        if self.loading.get() || !self.loaded.get() { return None; }
        self.scene.active_id().map(|s| s.to_string()).filter(|id| self.matched.borrow().iter().any(|(scene, _)| scene == id))
    }
    fn load(self: &Rc<Self>, origin: &Rc<Workspace>, pid: String) {
        if self.loading.replace(true) { return; }
        self.loaded.set(false); self.status.set_text("Loading saved characters and manuscript order…");
        let next = self.clone(); let out = origin.clone();
        origin.client.rpc("planning_read_snapshot", json!({"projectId":pid}), move |reply| {
            if page_owner(&out, &next.panel.widget, &pid).is_none() { return; }
            next.loading.set(false);
            let snapshot = match reply { Ok(snapshot) => snapshot, Err(error) => { next.character.remove_all(); next.scene.remove_all(); next.matched.borrow_mut().clear(); next.status.set_text(&format!("Could not load saved character links. {error}")); return; } };
            let index = &snapshot["index"]; let saved = &snapshot["planning"];
            let planning = if saved.is_null() { crate::tools::initial("planning") } else { saved["payload"].clone() };
            next.changing.set(true); let chosen = next.character.active_id(); next.character.remove_all();
            for record in rows(&planning, "records").iter().filter(|r| r["kind"] == "character") {
                if let Some(id) = record["id"].as_str() { next.character.append(Some(id), &name(record)); }
            }
            if let Some(id) = chosen.as_deref() { if !rows(&planning, "records").iter().any(|r| r["kind"] == "character" && r["id"] == id) { next.character.append(Some(id), "Character no longer available"); } next.character.set_active_id(Some(id)); }
            else { next.character.set_active(Some(0)); }
            *next.scenes.borrow_mut() = rows(index, "scenes").iter().filter_map(|s| Some((s["id"].as_str()?.to_owned(), s["title"].as_str().unwrap_or("Scene").to_owned()))).collect();
            *next.planning.borrow_mut() = planning; next.loaded.set(true); next.changing.set(false); next.render();
        });
    }

}
impl Workspace {
    pub(super) fn character_cut(self: &Rc<Self>) {
        let a = desk::effective(self); let pid = a.pid.borrow().clone();
        if pid.is_empty() { a.error("Choose a novel before opening Character cut."); return; }
        let origin = a.clone();
        a.navigate(move || {
            if origin.desk.is_closed() || *origin.pid.borrow() != pid { return; }
            let panel = origin.panel("Character cut"); let body = panel.content_area();
            let hint = gtk::Label::new(Some("Scenes explicitly linked to one authored character, in manuscript order. Open the original scene to write; this cut creates no copy and does not infer presence from prose.")); hint.set_xalign(0.); hint.set_line_wrap(true); body.pack_start(&hint, false, false, 0);
            let character = gtk::ComboBoxText::new(); character.set_tooltip_text(Some("Character")); body.pack_start(&character, false, false, 0);
            let scene = gtk::ComboBoxText::new(); scene.set_tooltip_text(Some("Linked scene")); body.pack_start(&scene, false, false, 0);
            let controls = gtk::Box::new(gtk::Orientation::Vertical, 6); body.pack_start(&controls, false, false, 0);
            let previous = gtk::Button::with_label("Previous linked scene"); let next = gtk::Button::with_label("Next linked scene"); controls.pack_start(&previous, false, false, 0); controls.pack_start(&next, false, false, 0);
            let status = gtk::Label::new(None); status.set_xalign(0.); status.set_line_wrap(true); body.pack_start(&status, false, false, 0);
            let page = Rc::new(CharacterCutPage { panel: panel.clone(), planning: RefCell::new(Value::Null), scenes: RefCell::new(Vec::new()), matched: RefCell::new(Vec::new()), character, scene, status, previous, next, loading: Cell::new(false), changing: Cell::new(false), loaded: Cell::new(false) });
            let weak = Rc::downgrade(&page); page.character.connect_changed(move |_| if let Some(p) = weak.upgrade() { p.render(); });
            let weak = Rc::downgrade(&page); page.scene.connect_changed(move |_| if let Some(p) = weak.upgrade() { p.selection(); });
            let p = page.clone(); page.previous.connect_clicked(move |_| p.step(-1));
            let p = page.clone(); page.next.connect_clicked(move |_| p.step(1));
            for (title, separate) in [("Open scene", false), ("Open scene in new window", true)] {
                let p = page.clone(); let out = origin.clone(); let id = pid.clone();
                button(&controls, title, move || { if let Some(a) = page_owner(&out, &p.panel.widget, &id) { if let Some(scene) = p.selected() { a.open_scene(&scene, separate); } } });
            }
            let p = page.clone(); let out = origin.clone(); let id = pid.clone();
            button(&controls, "Refresh saved links", move || { if let Some(a) = page_owner(&out, &p.panel.widget, &id) { p.load(&a, id.clone()); } });
            panel.show_all(); page.load(&origin, pid);
        });
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn scenes() -> Vec<(String, String)> { ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "unlinked"].into_iter().map(|id| (id.into(), format!("{id} · مِ 中文"))).collect() }
    fn fixture() -> Value { json!({"records":[{"id":"hero","kind":"character","name":"نور 明","aliases":["unlinked"],"links":[{"kind":"scene","id":"second"},{"kind":"event","id":"record-event"},{"kind":"scene","id":"deleted"}],"values":{"custom":[{"kind":"scene","id":"first"},{"kind":"event","id":"custom-event"}]}},{"id":"other","kind":"character","links":[{"kind":"scene","id":"unlinked"}]}],"events":[{"id":"record-event","sceneIds":["third"]},{"id":"custom-event","sceneIds":["fourth"]},{"id":"fact-event","sceneIds":["fifth"]},{"id":"knowledge-event","sceneIds":["sixth"]}],"facts":[{"id":"fact","subjectId":"hero","eventId":"fact-event","untilEventId":"missing-event"},{"id":"other-fact","subjectId":"other"}],"readerReveals":[{"factId":"fact","scene":{"id":"seventh"}},{"factId":"other-fact","scene":{"id":"unlinked"}}],"knowledge":[{"characterId":"hero","scope":{"kind":"events","fromEventId":"knowledge-event","untilEventId":null}},{"characterId":"hero","scope":{"kind":"scenes","sceneIds":["eighth","deleted"]}},{"characterId":"other","scope":{"kind":"scenes","sceneIds":["unlinked"]}}]}) }
    #[test] fn cut_uses_every_authored_link_path_and_returns_original_ids_in_manuscript_order() {
        let planning = fixture(); let before = planning.clone(); let cut = character_scenes(&planning, &scenes(), "hero");
        assert_eq!(cut.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), vec!["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth"]); assert_eq!(planning, before);
        assert!(cut[0].1.contains("مِ 中文"));
    }
    #[test] fn reorder_affects_navigation_and_missing_characters_fail_closed() {
        let mut scenes = scenes(); scenes.reverse(); let cut = character_scenes(&fixture(), &scenes, "hero"); assert_eq!(cut[0].0, "eighth");
        assert!(character_scenes(&fixture(), &scenes, "deleted-character").is_empty());
    }
    #[test] fn aliases_unlinked_prose_and_noncharacter_records_do_not_establish_membership() {
        let mut planning = fixture(); planning["records"][0]["links"] = json!([]); planning["records"][0]["values"] = json!({"text":"unlinked نور 明", "choices":["unlinked"]}); planning["facts"] = json!([]); planning["readerReveals"] = json!([]); planning["knowledge"] = json!([]);
        assert!(character_scenes(&planning, &scenes(), "hero").is_empty()); planning["records"][0]["kind"] = json!("location"); planning["records"][0]["links"] = json!([{"kind":"scene","id":"first"}]); assert!(character_scenes(&planning, &scenes(), "hero").is_empty());
    }
    #[test] fn duplicate_links_do_not_create_duplicate_scene_entries() {
        let mut planning = fixture(); planning["records"][0]["links"].as_array_mut().unwrap().push(json!({"kind":"scene","id":"first"})); planning["knowledge"].as_array_mut().unwrap().push(json!({"characterId":"hero","scope":{"kind":"scenes","sceneIds":["first"]}})); assert_eq!(character_scenes(&planning, &scenes(), "hero").len(), 8);
    }
}
