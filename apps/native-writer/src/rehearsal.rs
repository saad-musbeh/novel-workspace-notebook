//! Read-only rehearsal of explicit author reveals, never inferred from prose.
use super::*;
use std::collections::{HashSet, BTreeMap};

const WORK_LIMIT: usize = 20_000;
const ROW_LIMIT: usize = 100;

#[derive(Clone, Debug, Default, PartialEq)]
struct Rehearsal {
    progress: Option<usize>,
    facts: Vec<String>,
    events: Vec<String>,
    notes: Vec<String>,
    threads: Vec<String>,
    missing: usize,
    limited: bool,
}

fn rows<'a>(document: &'a Value, key: &str) -> &'a [Value] {
    document[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}
fn preview(value: &str) -> String {
    let mut chars = value.chars();
    let mut out: String = chars.by_ref().take(2_000).collect();
    if chars.next().is_some() { out.push_str("…"); }
    out
}

/// The returned view contains only reached information. In particular it does
/// not retain the future payoff's title, links, notes, or any future reveal.
fn rehearse(document: &Value, scenes: &[(String, String)], scene: Option<&str>, position: Option<u64>) -> Rehearsal {
    let mut view = Rehearsal::default();
    view.progress = scene.and_then(|id| scenes.iter().position(|(candidate, _)| candidate == id));
    let Some(progress) = view.progress else { return view; };
    let order: HashMap<&str, usize> = scenes.iter().enumerate().map(|(i, (id, _))| (id.as_str(), i)).collect();
    let facts: HashMap<&str, &Value> = rows(document, "facts").iter().filter_map(|f| Some((f["id"].as_str()?, f))).collect();
    let events: HashMap<&str, &Value> = rows(document, "events").iter().filter_map(|e| Some((e["id"].as_str()?, e))).collect();
    let mut revealed_facts: BTreeMap<String, (&Value, Vec<String>)> = BTreeMap::new();
    let mut revealed_events: BTreeMap<String, (&Value, Vec<String>)> = BTreeMap::new();
    let mut work = 0;
    for reveal in rows(document, "readerReveals") {
        work += 1;
        if work > WORK_LIMIT { view.limited = true; break; }
        let Some(at) = reveal["scene"]["id"].as_str().and_then(|id| order.get(id)).copied() else { view.missing += 1; continue; };
        if at > progress || at == progress && position.is_some_and(|p| reveal["position"].as_u64().is_none_or(|r| r > p)) { continue; }
        // Facts and events become visible only through their own explicit reveal.
        let (id, target, destination) = if let Some(id) = reveal["factId"].as_str() {
            (id, facts.get(id).copied(), &mut revealed_facts)
        } else if let Some(id) = reveal["eventId"].as_str() {
            (id, events.get(id).copied(), &mut revealed_events)
        } else { view.missing += 1; continue; };
        let Some(target) = target else { view.missing += 1; continue; };
        destination.entry(id.to_owned()).or_insert_with(|| (target, Vec::new())).1.push(preview(&text(reveal, "notes")));
    }
    let mut groups: HashMap<(String, String), HashSet<String>> = HashMap::new();
    for (fact, _) in revealed_facts.values() {
        if fact["confirmed"] == true && fact["status"] == "fact" {
            groups.entry((text(fact, "subjectId"), text(fact, "property"))).or_default().insert(text(fact, "value"));
        }
    }
    for (fact, reveals) in revealed_facts.values().take(ROW_LIMIT) {
        let approved = fact["confirmed"] == true && fact["status"] == "fact";
        let status = if approved { "Approved fact".into() } else { text(fact, "status") };
        let conflict = approved && groups.get(&(text(fact, "subjectId"), text(fact, "property"))).is_some_and(|values| values.len() > 1);
        view.facts.push(format!("{}\n{}\n{status}{}{}", preview(&text(fact, "property")), preview(&text(fact, "value")), if conflict { " · Different revealed values; applicability unresolved" } else { "" }, reveal_notes(reveals)));
    }
    for (event, reveals) in revealed_events.values().take(ROW_LIMIT) {
        view.events.push(format!("{}\n{}{}", preview(&text(event, "title")), if event["time"]["kind"] == "unknown" { "Story time unknown" } else { "Authored story time; reveal order is separate" }, reveal_notes(reveals)));
    }
    let mut visible_points = Vec::new();
    for point in rows(document, "plotPoints") {
        work += 1;
        if work > WORK_LIMIT { view.limited = true; break; }
        let mut visible = false;
        for link in rows(point, "links") {
            work += 1;
            if work > WORK_LIMIT { view.limited = true; break; }
            if link["kind"] == "scene" {
                if let Some(at) = link["id"].as_str().and_then(|id| order.get(id)).copied() {
                    // Scene-level notes have no within-scene reveal coordinate.
                    visible |= at < progress || at == progress && position.is_none();
                } else { view.missing += 1; }
            } else if link["kind"] == "event" {
                visible |= link["id"].as_str().is_some_and(|id| revealed_events.contains_key(id));
            }
        }
        if visible { visible_points.push(point); }
    }
    let point_ids: HashSet<&str> = visible_points.iter().filter_map(|p| p["id"].as_str()).collect();
    for point in visible_points.iter().take(ROW_LIMIT) {
        view.notes.push(format!("{}\n{}", preview(&text(point, "title")), preview(&text(point, "notes"))));
        if point["requiresPayoff"] == true && !point["payoffId"].as_str().is_some_and(|id| point_ids.contains(id)) {
            let status = if point["scopeClosed"] == true { "Closed scope; no payoff revealed here" } else if point["payoffId"].as_str().is_some() { "Payoff not yet placed in visible scope" } else { "Open setup" };
            view.threads.push(format!("{}\n{}\n{status}", preview(&text(point, "title")), preview(&text(point, "notes"))));
        }
    }
    view.limited |= revealed_facts.len() > ROW_LIMIT || revealed_events.len() > ROW_LIMIT || visible_points.len() > ROW_LIMIT;
    view
}
fn reveal_notes(notes: &[String]) -> String {
    let mut result = String::new();
    for note in notes.iter().filter(|s| !s.is_empty()).take(ROW_LIMIT) { result.push_str("\nReveal note: "); result.push_str(note); }
    if notes.len() > ROW_LIMIT { result.push_str("\nFurther reveal notes omitted."); }
    result
}
fn report(view: &Rehearsal) -> String {
    if view.progress.is_none() { return "This progress scene is missing. No rehearsal information is shown.".into(); }
    let mut result = String::new();
    if view.missing > 0 { result.push_str("Some authored links are missing and cannot be placed.\n\n"); }
    if view.limited { result.push_str("Rehearsal limit reached. Some authored links are omitted.\n\n"); }
    for (heading, entries, empty) in [("Facts", &view.facts, "No authored fact reveals yet."), ("Events", &view.events, "No authored event reveals yet."), ("Open setup/payoff threads", &view.threads, "No unresolved authored setup/payoff threads here."), ("Plot notes", &view.notes, "No plot notes linked to this progress.")] {
        result.push_str(heading); result.push_str("\n\n");
        result.push_str(&if entries.is_empty() { empty.to_owned() } else { entries.join("\n\n") }); result.push_str("\n\n");
    }
    result
}

fn owner(origin: &Rc<Workspace>, widget: &gtk::Box, pid: &str) -> Option<Rc<Workspace>> {
    let mut owner = None;
    desk::with_button_owner(widget.upcast_ref(), || {
        let a = desk::effective(origin);
        if !a.desk.is_closed() && *a.pid.borrow() == pid && a.pages.page_num(widget).is_some() { owner = Some(a); }
    }); owner
}

struct RehearsalPage {
    panel: Rc<Panel>,
    scenes: RefCell<Vec<(String, String)>>,
    document: RefCell<Value>,
    progress: gtk::ComboBoxText,
    position: gtk::Entry,
    status: gtk::Label,
    buffer: gtk::TextBuffer,
    previous: gtk::Button,
    next: gtk::Button,
    changing: Cell<bool>,
    loading: Cell<bool>,
    loaded: Cell<bool>,
}
impl RehearsalPage {
    fn render(&self) {
        if self.changing.get() || self.loading.get() || !self.loaded.get() { return; }
        let position = self.position.text();
        let position = if position.is_empty() { None } else {
            match position.parse::<u64>().ok().and_then(|n| n.checked_sub(1)) {
                Some(n) => Some(n), None => { self.status.set_text("Use a whole reveal position of 1 or more, or leave it blank for the whole scene."); self.buffer.set_text(""); return; }
            }
        };
        let scenes = self.scenes.borrow();
        let selected = self.progress.active_id();
        let view = rehearse(&self.document.borrow(), &scenes, selected.as_deref(), position);
        self.previous.set_sensitive(view.progress.is_some_and(|p| p > 0));
        self.next.set_sensitive(view.progress.is_some_and(|p| p + 1 < scenes.len()));
        self.status.set_text(&view.progress.map(|p| format!("{} / {} · {} · Saved planning snapshot", p + 1, scenes.len(), scenes[p].1)).unwrap_or_else(|| "Choose an available progress scene.".into()));
        self.buffer.set_text(&report(&view));
    }
    fn step(&self, offset: isize) {
        if self.loading.get() || !self.loaded.get() { return; }
        let scenes = self.scenes.borrow();
        let Some(index) = self.progress.active_id().and_then(|id| scenes.iter().position(|(scene, _)| scene == id.as_str())) else { return; };
        let Some(next) = index.checked_add_signed(offset).filter(|n| *n < scenes.len()) else { return; };
        self.position.set_text(""); self.progress.set_active_id(Some(&scenes[next].0));
    }
    fn load(self: &Rc<Self>, origin: &Rc<Workspace>, pid: String) {
        if self.loading.replace(true) { return; }
        self.loaded.set(false); self.status.set_text("Loading saved planning and manuscript order…"); self.buffer.set_text("");
        let next = self.clone(); let out = origin.clone();
        origin.client.rpc("planning_read_snapshot", json!({"projectId":pid}), move |reply| {
            if owner(&out, &next.panel.widget, &pid).is_none() { return; }
            next.loading.set(false);
            let snapshot = match reply { Ok(snapshot) => snapshot, Err(error) => { next.status.set_text(&format!("Could not load saved rehearsal. {error}")); return; } };
            let index = &snapshot["index"]; let saved = &snapshot["planning"];
            next.changing.set(true); let chosen = next.progress.active_id().map(|id| id.to_string());
            let scenes: Vec<(String, String)> = rows(index, "scenes").iter().filter_map(|s| Some((s["id"].as_str()?.to_owned(), text(s, "title")))).collect();
            next.progress.remove_all();
            for (i, (id, title)) in scenes.iter().enumerate() { next.progress.append(Some(id), &format!("{}. {title}", i + 1)); }
            if let Some(id) = chosen.as_deref() {
                if !scenes.iter().any(|(s, _)| s == id) { next.progress.append(Some(id), "Missing progress scene"); }
                next.progress.set_active_id(Some(id));
            } else { next.progress.set_active(Some(0)); }
            *next.scenes.borrow_mut() = scenes;
            *next.document.borrow_mut() = if saved.is_null() { crate::tools::initial("planning") } else { saved["payload"].clone() };
            next.loaded.set(true); next.changing.set(false); next.render();
        });
    }

}
impl Workspace {
    pub(super) fn reader_rehearsal(self: &Rc<Self>) {
        let a = desk::effective(self); let pid = a.pid.borrow().clone();
        if pid.is_empty() { a.error("Choose a novel before opening Reader rehearsal."); return; }
        let selected = a.context_pane().map(|p| p.scene_id.clone());
        let origin = a.clone();
        a.navigate(move || {
            if origin.desk.is_closed() || *origin.pid.borrow() != pid { return; }
            let panel = origin.panel("Reader rehearsal"); let body = panel.content_area();
            let hint = gtk::Label::new(Some("Authored reveals and plot links through the chosen manuscript progress. Future information stays hidden. Refresh reads saved planning; it does not change your chosen progress.")); hint.set_xalign(0.); hint.set_line_wrap(true); body.pack_start(&hint, false, false, 0);
            let controls = gtk::Box::new(gtk::Orientation::Vertical, 6); body.pack_start(&controls, false, false, 0);
            let progress = gtk::ComboBoxText::new(); progress.set_tooltip_text(Some("Progress scene"));
            if let Some(id) = selected.as_deref() { progress.append(Some(id), "Selected scene"); progress.set_active_id(Some(id)); }
            body.pack_start(&progress, false, false, 0);
            let position = gtk::Entry::new(); position.set_placeholder_text(Some("Through reveal position (blank = whole scene)")); body.pack_start(&position, false, false, 0);
            let status = gtk::Label::new(None); status.set_xalign(0.); status.set_line_wrap(true); body.pack_start(&status, false, false, 0);
            let view = gtk::TextView::new(); view.set_editable(false); view.set_cursor_visible(false); view.set_wrap_mode(gtk::WrapMode::WordChar); view.set_left_margin(12); view.set_right_margin(12); body.pack_start(&scroll(&view), true, true, 0);
            let previous = gtk::Button::with_label("Previous scene"); let next = gtk::Button::with_label("Next scene"); controls.pack_start(&previous, false, false, 0); controls.pack_start(&next, false, false, 0);
            let page = Rc::new(RehearsalPage { panel: panel.clone(), scenes: RefCell::new(Vec::new()), document: RefCell::new(Value::Null), progress, position, status, buffer: view.buffer().unwrap(), previous, next, changing: Cell::new(false), loading: Cell::new(false), loaded: Cell::new(false) });
            let weak = Rc::downgrade(&page); page.progress.connect_changed(move |_| if let Some(page) = weak.upgrade() { page.render(); });
            let weak = Rc::downgrade(&page); page.position.connect_changed(move |_| if let Some(page) = weak.upgrade() { page.render(); });
            let p = page.clone(); page.previous.connect_clicked(move |_| p.step(-1));
            let p = page.clone(); page.next.connect_clicked(move |_| p.step(1));
            for (title, separate) in [("Open scene", false), ("Open scene in new window", true)] {
                let p = page.clone(); let out = origin.clone(); let id = pid.clone();
                button(&controls, title, move || { if let Some(a) = owner(&out, &p.panel.widget, &id) { if p.loaded.get() && !p.loading.get() { if let Some(scene) = p.progress.active_id().filter(|s| p.scenes.borrow().iter().any(|(id, _)| id == s.as_str())) { a.open_scene(&scene, separate); } } } });
            }
            let p = page.clone(); let out = origin.clone(); let id = pid.clone();
            button(&controls, "Refresh saved planning", move || { if let Some(a) = owner(&out, &p.panel.widget, &id) { p.load(&a, id.clone()); } });
            panel.show_all(); page.load(&origin, pid);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scenes() -> Vec<(String, String)> { [("first", "مِ 中文"), ("second", "Present"), ("third", "Future")].into_iter().map(|(id, title)| (id.into(), title.into())).collect() }
    fn fixture() -> Value {
        json!({"facts":[{"id":"known","subjectId":"hero","property":"Name","value":"مِ 中文 \u{e000} 👩🏽‍🚀","status":"fact","confirmed":true},{"id":"hidden","subjectId":"hero","property":"SECRET FUTURE PROPERTY","value":"SECRET FUTURE VALUE","status":"proposal","confirmed":false}],"events":[{"id":"early","title":"Old event","time":{"kind":"fixed","day":1}},{"id":"late","title":"SECRET FUTURE EVENT","time":{"kind":"unknown"}}],"readerReveals":[{"id":"r1","factId":"known","scene":{"id":"first"},"position":0,"notes":"Authored visible note"},{"id":"r2","eventId":"early","scene":{"id":"second"},"position":1,"notes":"Flashback told later"},{"id":"r3","factId":"hidden","scene":{"id":"third"},"position":0,"notes":"SECRET FUTURE REVEAL NOTE"},{"id":"r4","eventId":"late","scene":{"id":"third"},"position":1}],"plotPoints":[{"id":"setup","title":"Visible setup","notes":"A clue","links":[{"kind":"scene","id":"first"}],"requiresPayoff":true,"payoffId":"payoff","scopeClosed":false},{"id":"payoff","title":"SECRET FUTURE PAYOFF","notes":"SECRET FUTURE PLOT NOTE","links":[{"kind":"scene","id":"third"}],"requiresPayoff":false}]})
    }
    #[test] fn future_values_and_payoff_metadata_never_enter_the_view() {
        let doc = fixture(); let original = doc.clone(); let view = rehearse(&doc, &scenes(), Some("first"), None);
        assert_eq!(view.facts.len(), 1); assert!(view.events.is_empty()); assert_eq!(view.threads.len(), 1);
        assert!(report(&view).contains("مِ 中文 \u{e000} 👩🏽‍🚀")); assert!(!format!("{view:?}").contains("SECRET FUTURE")); assert_eq!(doc, original);
    }
    #[test] fn authored_positions_hide_current_scene_notes_until_whole_scene_is_reached() {
        assert!(rehearse(&fixture(), &scenes(), Some("second"), Some(0)).events.is_empty());
        assert_eq!(rehearse(&fixture(), &scenes(), Some("second"), Some(1)).events.len(), 1);
        assert!(rehearse(&fixture(), &scenes(), Some("first"), Some(0)).notes.is_empty());
        assert!(rehearse(&fixture(), &scenes(), Some("third"), None).threads.is_empty());
    }
    #[test] fn reorder_changes_disclosure_without_changing_story_time_or_canon() {
        let doc = fixture(); let original = doc.clone(); let mut reordered = scenes(); reordered.rotate_right(1);
        let view = rehearse(&doc, &reordered, Some("first"), None); assert_eq!(view.facts.len(), 2); assert_eq!(view.events.len(), 1); assert_eq!(doc, original);
    }
    #[test] fn missing_progress_and_dangling_links_fail_closed_without_hidden_names() {
        let mut doc = fixture(); doc["readerReveals"].as_array_mut().unwrap().push(json!({"factId":"hidden","scene":{"id":"deleted"},"position":0,"notes":"SECRET FUTURE DANGLING"}));
        let view = rehearse(&doc, &scenes(), Some("deleted"), None); assert!(view.facts.is_empty()); assert!(view.notes.is_empty());
        let visible = rehearse(&doc, &scenes(), Some("first"), None); assert_eq!(visible.missing, 1); assert!(!report(&visible).contains("SECRET FUTURE"));
    }
    #[test] fn conflicting_revealed_values_remain_an_authored_review_cue() {
        let mut doc = fixture(); doc["facts"].as_array_mut().unwrap().push(json!({"id":"other","subjectId":"hero","property":"Name","value":"Different","status":"fact","confirmed":true}));
        doc["readerReveals"].as_array_mut().unwrap().push(json!({"factId":"other","scene":{"id":"first"},"position":0}));
        let view = rehearse(&doc, &scenes(), Some("first"), None); assert_eq!(view.facts.len(), 2); assert!(view.facts.iter().all(|f| f.contains("applicability unresolved")));
    }
    #[test] fn bounded_rendering_reports_omissions_without_exposing_future_links() {
        let mut doc = fixture(); doc["readerReveals"] = json!((0..WORK_LIMIT + 1).map(|i| json!({"id":format!("r{i}"),"factId":"hidden","scene":{"id":"third"},"position":0,"notes":"SECRET FUTURE"})).collect::<Vec<_>>());
        let view = rehearse(&doc, &scenes(), Some("first"), None); assert!(view.limited); assert!(!report(&view).contains("SECRET FUTURE"));
        assert_eq!(preview("مِ 中文"), "مِ 中文"); assert!(preview(&"界".repeat(2_001)).ends_with('…'));
    }
}
