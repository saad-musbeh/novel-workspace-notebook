//! Saved, read-only authored references. No manuscript bodies or inference.
use super::*;
use std::collections::{HashSet, BTreeSet};
use std::rc::Weak;

const LIMIT: usize = 20_000;
fn rows<'a>(value: &'a Value, key: &str) -> &'a [Value] { value[key].as_array().map(Vec::as_slice).unwrap_or(&[]) }
fn string(value: &Value, key: &str) -> String { value[key].as_str().unwrap_or("").to_owned() }
fn excerpt(text: &str) -> String { let mut chars = text.chars(); let mut result: String = chars.by_ref().take(2_000).collect(); if chars.next().is_some() { result.push('…'); } result }
fn has_scene(value: &Value, key: &str, scene: &str) -> bool { rows(value, key).iter().any(|id| id.as_str() == Some(scene)) }

#[derive(Default)] struct Budget { work: usize, limited: bool }
impl Budget { fn visit(&mut self) -> bool { self.work += 1; if self.work > LIMIT { self.limited = true; false } else { true } } }

fn event_bounds(events: &HashMap<&str, &Value>, id: &str, budget: &mut Budget) -> Option<(f64, f64)> {
    let mut current = id; let mut offset = 0.; let mut seen = HashSet::new();
    loop {
        if !budget.visit() || !seen.insert(current) { return None; }
        let event = events.get(current)?; let time = &event["time"];
        let result = match time["kind"].as_str()? {
            "fixed" => { let n = time["day"].as_f64()?; Some((n + offset, n + offset)) },
            "bounds" | "interval" => { let a = time["start"].as_f64()?; let b = time["end"].as_f64()?; if a > b { return None; } Some((a + offset, b + offset)) },
            "relative" => { offset += time["offsetDays"].as_f64()?; current = time["eventId"].as_str()?; None },
            _ => return None,
        };
        if let Some((a, b)) = result { return (a.is_finite() && b.is_finite()).then_some((a, b)); }
        if !offset.is_finite() { return None; }
    }
}
#[derive(Clone, Copy, PartialEq, Debug)] enum Order { Before, After, Same, Unknown, Unordered, Conflict }
fn strict_path(doc: &Value, from: &str, to: &str, budget: &mut Budget) -> Option<bool> {
    let mut queue = vec![(from, false)]; let mut seen = HashSet::new(); let mut cursor = 0;
    while cursor < queue.len() {
        if !budget.visit() { return None; } let (id, strict) = queue[cursor]; cursor += 1;
        if id == to && strict { return Some(true); }
        if !seen.insert((id, strict)) { continue; }
        for edge in rows(doc, "constraints") {
            if !budget.visit() { return None; }
            if edge["confirmed"] == true && edge["beforeId"].as_str() == Some(id) {
                if let Some(next) = edge["afterId"].as_str() { queue.push((next, strict || edge["strict"] == true || edge["minimumDays"].as_f64().is_some_and(|n| n > 0.))); }
            }
        }
    } Some(false)
}
fn conflicting_bounds<'a>(doc: &'a Value, events: &HashMap<&'a str, &'a Value>, budget: &mut Budget) -> Option<HashSet<&'a str>> {
    let mut bounds = HashMap::new(); let mut lower: HashMap<&str, (f64, bool)> = HashMap::new(); let mut queue = Vec::new();
    for id in events.keys() {
        if !budget.visit() { return None; }
        let range = event_bounds(events, id, budget); bounds.insert(*id, range);
        if let Some((start, _)) = range { lower.insert(*id, (start, false)); queue.push(*id); }
    }
    let mut conflicting = HashSet::new(); let mut cursor = 0;
    while cursor < queue.len() {
        if !budget.visit() { return None; }
        let from = queue[cursor]; cursor += 1; let (day, strict) = *lower.get(from)?;
        for edge in rows(doc, "constraints") {
            if !budget.visit() { return None; }
            if edge["confirmed"] != true || edge["beforeId"].as_str() != Some(from) { continue; }
            let Some(to) = edge["afterId"].as_str().filter(|id| events.contains_key(id)) else { continue; };
            let Some(days) = edge["minimumDays"].as_f64().filter(|n| *n >= 0.) else { return None; };
            let candidate = (day + days, strict || edge["strict"] == true);
            if !candidate.0.is_finite() { return None; }
            if bounds.get(to).copied().flatten().is_some_and(|(_, end)| candidate.0 > end || candidate.0 == end && candidate.1) { conflicting.insert(from); conflicting.insert(to); continue; }
            if lower.get(to).is_none_or(|old| candidate.0 > old.0 || candidate.0 == old.0 && candidate.1 && !old.1) { lower.insert(to, candidate); queue.push(to); }
        }
    } Some(conflicting)
}
fn event_order(doc: &Value, events: &HashMap<&str, &Value>, left: &str, right: &str, budget: &mut Budget) -> Order {
    if !events.contains_key(left) || !events.contains_key(right) { return Order::Unknown; }
    let Some(conflicting) = conflicting_bounds(doc, events, budget) else { return Order::Unknown; };
    if conflicting.contains(left) || conflicting.contains(right) { return Order::Conflict; }
    let a = event_bounds(events, left, budget); let b = event_bounds(events, right, budget);
    let Some(forward) = strict_path(doc, left, right, budget) else { return Order::Unknown; };
    let Some(backward) = strict_path(doc, right, left, budget) else { return Order::Unknown; };
    if forward && backward || forward && a.zip(b).is_some_and(|((start, _), (_, end))| start >= end) || backward && a.zip(b).is_some_and(|((_, end), (start, _))| start >= end) { return Order::Conflict; }
    // Check explicit minimum-day contradictions in this component. There is no
    // automatic rescheduling or assumption that unspecified dates are false.
    for edge in rows(doc, "constraints").iter().filter(|e| e["confirmed"] == true) {
        if !budget.visit() { return Order::Unknown; }
        if let (Some(from), Some(until), Some(days)) = (edge["beforeId"].as_str(), edge["afterId"].as_str(), edge["minimumDays"].as_f64()) {
            if from == left || from == right || until == left || until == right {
                if event_bounds(events, from, budget).zip(event_bounds(events, until, budget)).is_some_and(|((start, _), (_, end))| end - start < days || end - start == days && edge["strict"] == true) { return Order::Conflict; }
            }
        }
    }
    if left == right { Order::Same } else if forward || a.zip(b).is_some_and(|((_, end), (start, _))| end < start) { Order::Before }
    else if backward || a.zip(b).is_some_and(|((start, _), (_, end))| end < start) { Order::After }
    else if a.is_some() && b.is_some() { Order::Unordered } else { Order::Unknown }
}
fn event_scope(doc: &Value, events: &HashMap<&str, &Value>, scene_events: &[&Value], from: &str, until: Option<&str>, budget: &mut Budget) -> &'static str {
    if let Some(until) = until { if matches!(event_order(doc, events, from, until, budget), Order::After | Order::Same | Order::Conflict) { return "Conflicting order"; } }
    if scene_events.is_empty() { return "Timing unknown"; }
    let mut states = HashSet::new();
    for event in scene_events {
        if !budget.visit() { return "Timing unknown"; }
        let Some(id) = event["id"].as_str() else { return "Timing unknown"; };
        let start = event_order(doc, events, from, id, budget); let end = until.map(|until| event_order(doc, events, id, until, budget)).unwrap_or(Order::Before);
        let status = if start == Order::Conflict || end == Order::Conflict { return "Conflicting order"; }
            else if start == Order::After { "Not yet" } else if matches!(end, Order::After | Order::Same) { "Ended" }
            else if start == Order::Unknown || end == Order::Unknown { "Timing unknown" }
            else if start == Order::Unordered || end == Order::Unordered { "Order unresolved" } else { "Applies" };
        states.insert(status);
    }
    if states.len() == 1 { states.into_iter().next().unwrap() } else if states.contains("Timing unknown") { "Timing unknown" } else { "Order unresolved" }
}
fn knowledge_status(doc: &Value, entry: &Value, fact: Option<&Value>, events: &HashMap<&str, &Value>, scene_events: &[&Value], scene: &str, budget: &mut Budget) -> &'static str {
    let Some(fact) = fact else { return "Referenced fact changed or missing"; };
    if entry["stance"] == "knows" && (fact["confirmed"] != true || fact["status"] != "fact") || ["subjectId", "property", "value"].iter().any(|key| fact[*key] != entry["factSnapshot"][*key]) { return "Referenced fact changed"; }
    let scope = &entry["scope"];
    if scope["kind"] == "scenes" { if has_scene(scope, "sceneIds", scene) { "Applies" } else { "Outside selected scenes" } }
    else if scope["kind"] == "events" { if let Some(from) = scope["fromEventId"].as_str() { event_scope(doc, events, scene_events, from, scope["untilEventId"].as_str(), budget) } else { "Timing unknown" } }
    else { "Scope missing" }
}

/// Display-only projection. Canonical facts and frozen authored knowledge are
/// distinct rows; changes never rewrite the recorded snapshot or approve it.
pub(super) fn references(doc: &Value, metadata: &Value, scenes: &[(String, String)], scene: &str) -> String {
    let Some((_, title)) = scenes.iter().find(|(id, _)| id == scene) else { return "This reference scene is missing. No authored reference information is shown.".into(); };
    let mut budget = Budget::default(); let mut missing = 0;
    let events: HashMap<&str, &Value> = rows(doc, "events").iter().filter_map(|e| Some((e["id"].as_str()?, e))).collect();
    let records: HashMap<&str, &Value> = rows(doc, "records").iter().filter_map(|r| Some((r["id"].as_str()?, r))).collect();
    let facts: HashMap<&str, &Value> = rows(doc, "facts").iter().filter_map(|f| Some((f["id"].as_str()?, f))).collect();
    let details = rows(metadata, "scenes").iter().find(|s| s["sceneId"].as_str() == Some(scene));
    let mut scene_events: Vec<&Value> = rows(doc, "events").iter().filter(|e| has_scene(e, "sceneIds", scene)).collect();
    if let Some(details) = details {
        for id in rows(details, "eventIds").iter().filter_map(Value::as_str) {
            if let Some(event) = events.get(id) { if !scene_events.iter().any(|e| e["id"] == id) { scene_events.push(event); } } else { missing += 1; }
        }
    }
    let mut record_ids: BTreeSet<&str> = BTreeSet::new(); let mut fact_ids: BTreeSet<&str> = BTreeSet::new();
    if let Some(id) = details.and_then(|d| d["viewpointId"].as_str()) { record_ids.insert(id); }
    for record in rows(doc, "records") {
        if !budget.visit() { break; }
        let Some(id) = record["id"].as_str() else { continue; };
        let mut links: Vec<&Value> = rows(record, "links").iter().collect();
        for value in record["values"].as_object().into_iter().flat_map(|v| v.values()) { links.extend(value.as_array().into_iter().flatten()); }
        for link in links {
            if !budget.visit() { break; }
            let applies = link["kind"] == "scene" && link["id"].as_str() == Some(scene) || link["kind"] == "event" && scene_events.iter().any(|e| e["id"] == link["id"]);
            if applies { record_ids.insert(id); }
        }
    }
    let reveals: Vec<&Value> = rows(doc, "readerReveals").iter().filter(|r| r["scene"]["id"].as_str() == Some(scene)).take(100).collect();
    for reveal in &reveals { if let Some(id) = reveal["factId"].as_str() { fact_ids.insert(id); } }
    for fact in rows(doc, "facts") {
        if !budget.visit() { break; }
        if scene_events.iter().any(|e| e["id"] == fact["eventId"] || e["id"] == fact["untilEventId"]) { if let Some(id) = fact["id"].as_str() { fact_ids.insert(id); } if let Some(id) = fact["subjectId"].as_str() { record_ids.insert(id); } }
    }
    let mut knowledge = Vec::new();
    for entry in rows(doc, "knowledge") {
        if !budget.visit() { break; }
        let status = knowledge_status(doc, entry, entry["factId"].as_str().and_then(|id| facts.get(id)).copied(), &events, &scene_events, scene, &mut budget);
        if status == "Applies" || entry["characterId"].as_str().is_some_and(|id| record_ids.contains(id)) || entry["scope"]["kind"] == "scenes" && has_scene(&entry["scope"], "sceneIds", scene) {
            if let Some(id) = entry["characterId"].as_str() { record_ids.insert(id); } if let Some(id) = entry["factId"].as_str() { fact_ids.insert(id); } knowledge.push((entry, status));
        }
    }
    for fact in rows(doc, "facts") { if !budget.visit() { break; } if fact["subjectId"].as_str().is_some_and(|id| record_ids.contains(id)) { if let Some(id) = fact["id"].as_str() { fact_ids.insert(id); } } }
    let mut output = format!("{title}\nSaved authored references · Story occurrence and reader reveal are separate.\n\nPeople, places and items\n\n");
    let mut shown = 0;
    for id in &record_ids {
        if shown >= 100 { budget.limited = true; break; }
        let Some(record) = records.get(id) else { missing += 1; continue; }; shown += 1;
        output.push_str(&format!("{} · {}\n{}\n", excerpt(&string(record, "name")), string(record, "kind"), excerpt(&string(record, "notes"))));
        for field in rows(doc, "fields").iter().filter(|f| f["hidden"] != true && f["recordKind"] == record["kind"]) {
            if !budget.visit() { break; }
            if let Some(value) = field["id"].as_str().and_then(|id| record["values"].get(id)) {
                // Linked IDs have their own explicit projection; never turn
                // JSON internals into manuscript/canon claims.
                let display = if let Some(s) = value.as_str() { s.to_owned() } else if value.is_number() { value.to_string() } else if let Some(values) = value.as_array().filter(|v| v.iter().all(Value::is_string)) { values.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" · ") } else { continue; };
                output.push_str(&format!("{}: {}\n", excerpt(&string(field, "label")), excerpt(&display)));
            }
        } output.push('\n');
    }
    if shown == 0 { output.push_str("No explicitly linked people, places or items.\n\n"); }
    output.push_str("Facts\n\n"); shown = 0;
    for id in &fact_ids {
        if shown >= 100 { budget.limited = true; break; }
        let Some(fact) = facts.get(id) else { missing += 1; continue; }; shown += 1;
        let applicability = fact["eventId"].as_str().map(|from| event_scope(doc, &events, &scene_events, from, fact["untilEventId"].as_str(), &mut budget)).unwrap_or("No declared start");
        let status = if fact["confirmed"] == true && fact["status"] == "fact" { "Approved fact".into() } else { string(fact, "status") };
        output.push_str(&format!("{}: {}\n{status} · {applicability}\nSource: {}\n\n", excerpt(&string(fact, "property")), excerpt(&string(fact, "value")), excerpt(&string(fact, "source"))));
    }
    if shown == 0 { output.push_str("No explicitly linked facts.\n\n"); }
    output.push_str("Story occurrence events\n\n");
    for event in scene_events.iter().take(100) {
        let time = event_bounds(&events, event["id"].as_str().unwrap_or(""), &mut budget).map(|(a,b)| if a == b { format!("Day {a}") } else { format!("Days {a}–{b}") }).unwrap_or_else(|| "Unknown or cyclic story time".into());
        output.push_str(&format!("{} · {time}\n{}\n\n", excerpt(&string(event, "title")), excerpt(&string(event, "notes"))));
    }
    if scene_events.is_empty() { output.push_str("No explicitly linked occurrence events.\n\n"); }
    output.push_str("Reader reveals in this scene\n\n");
    let mut reveals = reveals; reveals.sort_by_key(|r| r["position"].as_u64().unwrap_or(u64::MAX));
    for reveal in &reveals {
        let target = if let Some(id) = reveal["factId"].as_str() { facts.get(id).map(|f| format!("{}: {}", string(f, "property"), string(f, "value"))) } else { reveal["eventId"].as_str().and_then(|id| events.get(id)).map(|e| string(e, "title")) };
        if target.is_none() { missing += 1; }
        let position = reveal["position"].as_u64().and_then(|p| p.checked_add(1)).map(|p| p.to_string()).unwrap_or_else(|| "Unspecified".into());
        output.push_str(&format!("Reveal position {position} · {}\n{}\n\n", excerpt(&target.unwrap_or_else(|| "Missing authored target".into())), excerpt(&string(reveal, "notes"))));
    }
    if reveals.is_empty() { output.push_str("No authored reader reveals in this scene.\n\n"); }
    output.push_str("Character knowledge\n\n");
    for (entry, status) in knowledge.iter().take(100) {
        let character = entry["characterId"].as_str().and_then(|id| records.get(id)).map(|r| string(r, "name")).unwrap_or_else(|| { missing += 1; "Missing character".into() });
        output.push_str(&format!("{} · {} · {status}\nRecorded fact: {}: {}\nInterpretation: {}\nAuthor source: {}\n\n", excerpt(&character), string(entry, "stance"), excerpt(&string(&entry["factSnapshot"], "property")), excerpt(&string(&entry["factSnapshot"], "value")), excerpt(&string(entry, "interpretation")), excerpt(&string(entry, "source"))));
    }
    if knowledge.is_empty() { output.push_str("No authored character knowledge linked to this scene.\n\n"); }
    if scene_events.len() > 100 || knowledge.len() > 100 || rows(doc, "readerReveals").iter().filter(|r| r["scene"]["id"].as_str() == Some(scene)).count() > 100 { budget.limited = true; }
    if missing > 0 { output.push_str("Some authored links are missing; their values are not inferred.\n"); }
    if budget.limited { output.push_str("Reference limit reached. Some authored data is omitted or timing remains unresolved.\n"); }
    output
}

thread_local! { static FOLLOWERS: RefCell<Vec<Weak<ReferencePage>>> = const { RefCell::new(Vec::new()) }; }
/// Called only when an actual writing page becomes active. Followers in other
/// windows read their saved snapshot; pinning keeps their chosen scene intact.
pub(super) fn scene_changed(project: &str, scene: &str) {
    FOLLOWERS.with(|pages| {
        let live: Vec<Rc<ReferencePage>> = { let mut pages = pages.borrow_mut(); pages.retain(|p| p.strong_count() > 0); pages.iter().filter_map(Weak::upgrade).collect() };
        for page in live { if page.project == project && page.owner().is_some() { *page.followed.borrow_mut() = Some(scene.to_owned()); if !page.pin.is_active() { page.select(scene); } } }
    });
}
struct ReferencePage {
    origin: Rc<Workspace>, project: String, panel: Rc<Panel>, followed: RefCell<Option<String>>,
    scenes: RefCell<Vec<(String,String)>>, planning: RefCell<Value>, metadata: RefCell<Value>,
    scene: gtk::ComboBoxText, pin: gtk::CheckButton, buffer: gtk::TextBuffer,
    status: gtk::Label, changing: Cell<bool>, loading: Cell<bool>, loaded: Cell<bool>,
}
impl ReferencePage {
    fn owner(&self) -> Option<Rc<Workspace>> {
        let origin = self.origin.clone(); let mut result = None;
        desk::with_button_owner(self.panel.widget.upcast_ref(), || { let a = desk::effective(&origin); if !a.desk.is_closed() && *a.pid.borrow() == self.project && a.pages.page_num(&self.panel.widget).is_some() { result = Some(a); } }); result
    }
    fn select(&self, id: &str) {
        if self.loading.get() { return; }
        self.changing.set(true);
        if !self.scenes.borrow().iter().any(|(scene, _)| scene == id) { self.scene.append(Some(id), "Missing reference scene"); }
        self.scene.set_active_id(Some(id)); self.changing.set(false); self.render();
    }
    fn render(&self) {
        if self.loading.get() || self.changing.get() || !self.loaded.get() { return; }
        let selected = self.scene.active_id();
        self.status.set_text(if self.pin.is_active() { "Pinned scene · Saved authored reference snapshot" } else { "Following active writing scenes in this novel · Saved authored reference snapshot" });
        self.buffer.set_text(&references(&self.planning.borrow(), &self.metadata.borrow(), &self.scenes.borrow(), selected.as_deref().unwrap_or("")));
    }
    fn load(self: &Rc<Self>, origin: &Rc<Workspace>) {
        if self.loading.replace(true) { return; } self.loaded.set(false); self.status.set_text("Loading saved references…"); self.buffer.set_text("");
        let page = self.clone(); let project = self.project.clone();
        origin.client.rpc("planning_read_snapshot", json!({"projectId":project}), move |reply| {
            if page.owner().is_none() { return; }
            page.loading.set(false);
            let snapshot = match reply { Ok(snapshot) => snapshot, Err(error) => { page.status.set_text(&format!("Could not load saved references. {error}")); return; } };
            let index = &snapshot["index"];
            let planning = if snapshot["planning"].is_null() { crate::tools::initial("planning") } else { snapshot["planning"]["payload"].clone() };
            let metadata = if snapshot["sceneMetadata"].is_null() { crate::tools::initial("sceneMetadata") } else { snapshot["sceneMetadata"]["payload"].clone() };
            let selected = if !page.pin.is_active() { page.followed.borrow().clone().or_else(|| page.scene.active_id().map(|id| id.to_string())) } else { page.scene.active_id().map(|id| id.to_string()) }; page.changing.set(true); page.scene.remove_all();
            let scenes: Vec<(String,String)> = rows(index, "scenes").iter().filter_map(|s| Some((s["id"].as_str()?.into(), string(s, "title")))).collect();
            for (id, title) in &scenes { page.scene.append(Some(id), title); }
            if let Some(id) = selected.as_deref() { if !scenes.iter().any(|(scene, _)| scene == id) { page.scene.append(Some(id), "Missing reference scene"); } page.scene.set_active_id(Some(id)); } else { page.scene.set_active(Some(0)); }
            *page.scenes.borrow_mut() = scenes; *page.planning.borrow_mut() = planning; *page.metadata.borrow_mut() = metadata;
            page.loaded.set(true); page.changing.set(false); page.render();
        });
    }

}
impl Workspace {
    pub(super) fn scene_references(self: &Rc<Self>) {
        let a = desk::effective(self); let project = a.pid.borrow().clone(); if project.is_empty() { a.error("Choose a novel before opening Scene references."); return; }
        let selected = a.context_pane().map(|p| p.scene_id.clone()); let origin = a.clone();
        a.navigate(move || {
            if origin.desk.is_closed() || *origin.pid.borrow() != project { return; }
            let panel = origin.panel("Scene references"); let body = panel.content_area();
            let hint = gtk::Label::new(Some("Read-only saved references from authored links. Detach this page to follow writing in another window, or pin a scene. Refresh reads saved notes; no manuscript text is loaded or inferred.")); hint.set_line_wrap(true); hint.set_xalign(0.); body.pack_start(&hint, false, false, 0);
            let scene = gtk::ComboBoxText::new(); scene.set_tooltip_text(Some("Reference scene")); if let Some(id) = selected.as_deref() { scene.append(Some(id), "Active scene"); scene.set_active_id(Some(id)); } body.pack_start(&scene, false, false, 0);
            let controls = gtk::Box::new(gtk::Orientation::Vertical, 6); body.pack_start(&controls, false, false, 0);
            let pin = gtk::CheckButton::with_label("Pin scene"); controls.pack_start(&pin, false, false, 0);
            let status = gtk::Label::new(None); status.set_xalign(0.); status.set_line_wrap(true); body.pack_start(&status, false, false, 0);
            let view = gtk::TextView::new(); view.set_editable(false); view.set_cursor_visible(false); view.set_wrap_mode(gtk::WrapMode::WordChar); view.set_left_margin(12); view.set_right_margin(12); body.pack_start(&scroll(&view), true, true, 0);
            let page = Rc::new(ReferencePage { origin: origin.clone(), project, panel: panel.clone(), followed: RefCell::new(selected), scenes: RefCell::new(Vec::new()), planning: RefCell::new(Value::Null), metadata: RefCell::new(Value::Null), scene, pin, buffer: view.buffer().unwrap(), status, changing: Cell::new(false), loading: Cell::new(false), loaded: Cell::new(false) });
            FOLLOWERS.with(|pages| pages.borrow_mut().push(Rc::downgrade(&page)));
            let weak = Rc::downgrade(&page); page.scene.connect_changed(move |_| if let Some(p) = weak.upgrade() { if !p.changing.get() { p.pin.set_active(true); p.render(); } });
            let weak = Rc::downgrade(&page); page.pin.connect_toggled(move |_| if let Some(p) = weak.upgrade() { p.render(); });
            let p = page.clone(); button(&controls, "Follow current scene", move || { if let Some(a) = p.owner() { p.pin.set_active(false); let current = p.followed.borrow().clone().or_else(|| a.context_pane().map(|scene| scene.scene_id.clone())); if let Some(scene) = current { p.select(&scene); } else { p.render(); } } });
            let p = page.clone(); button(&controls, "Refresh saved references", move || { if let Some(a) = p.owner() { p.load(&a); } });
            let p = page.clone(); button(&controls, "Open scene in new window", move || { if let Some(a) = p.owner() { if let Some(scene) = p.scene.active_id().filter(|id| p.scenes.borrow().iter().any(|(scene,_)| scene == id.as_str())) { a.open_scene(&scene, true); } } });
            panel.show_all(); page.load(&origin);
        });
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn fixture() -> Value { json!({"records":[{"id":"hero","kind":"character","name":"نور 明","notes":"Authored character","links":[{"kind":"scene","id":"scene"}],"values":{}},{"id":"place","kind":"location","name":"Garden","notes":"","links":[{"kind":"event","id":"middle"}],"values":{}},{"id":"unlinked","kind":"character","name":"SECRET UNLINKED","links":[],"values":{}}],"facts":[{"id":"fact","subjectId":"hero","property":"Name","value":"مِ 中文 \u{e000} 👩🏽‍🚀","status":"fact","confirmed":true,"source":"Author","eventId":"start","untilEventId":"end"}],"events":[{"id":"start","title":"Start","sceneIds":[],"time":{"kind":"fixed","day":1}},{"id":"middle","title":"Middle","sceneIds":["scene"],"time":{"kind":"fixed","day":2}},{"id":"end","title":"End","sceneIds":[],"time":{"kind":"fixed","day":3}}],"readerReveals":[{"factId":"fact","scene":{"id":"scene"},"position":4,"notes":"Explicitly told here"}],"knowledge":[{"characterId":"hero","factId":"fact","stance":"knows","factSnapshot":{"subjectId":"hero","property":"Name","value":"مِ 中文 \u{e000} 👩🏽‍🚀"},"source":"Authored knowledge","interpretation":"","scope":{"kind":"events","fromEventId":"start","untilEventId":"end"}}],"constraints":[],"fields":[]}) }
    fn scenes() -> Vec<(String,String)> { vec![("scene".into(), "Scene · 中文".into()), ("other".into(), "Other".into())] }
    #[test] fn references_show_only_explicit_links_and_keep_exact_unicode_without_mutation() {
        let doc = fixture(); let before = doc.clone(); let view = references(&doc, &json!({}), &scenes(), "scene"); assert!(view.contains("نور 明")); assert!(view.contains("Garden")); assert!(view.contains("مِ 中文 \u{e000} 👩🏽‍🚀")); assert!(view.contains("Reveal position 5")); assert!(view.contains("Day 2")); assert!(view.contains("knows · Applies")); assert!(!view.contains("SECRET UNLINKED")); assert_eq!(doc, before);
    }
    #[test] fn stale_fact_labels_retain_the_authored_snapshot_without_turning_it_into_current_canon() {
        let mut doc = fixture(); doc["facts"][0]["value"] = json!("New canon"); let view = references(&doc, &json!({}), &scenes(), "scene"); assert!(view.contains("Name: New canon")); assert!(view.contains("Referenced fact changed")); assert!(view.contains("Recorded fact: Name: مِ 中文"));
        doc["facts"][0]["confirmed"] = json!(false); assert!(references(&doc, &json!({}), &scenes(), "scene").contains("Referenced fact changed"));
    }
    #[test] fn scene_spanning_both_boundaries_has_no_invented_internal_order() {
        let mut doc = fixture(); doc["events"][0]["sceneIds"] = json!(["scene"]); doc["events"][2]["sceneIds"] = json!(["scene"]); assert!(references(&doc, &json!({}), &scenes(), "scene").contains("knows · Order unresolved"));
    }
    #[test] fn unknown_cyclic_and_conflicting_times_are_not_inferred() {
        let mut doc = fixture(); doc["events"][1]["time"] = json!({"kind":"relative","eventId":"middle","offsetDays":1}); assert!(references(&doc, &json!({}), &scenes(), "scene").contains("knows · Timing unknown"));
        doc["events"][1]["time"] = json!({"kind":"fixed","day":2}); doc["constraints"] = json!([{"beforeId":"start","afterId":"middle","minimumDays":5,"strict":false,"confirmed":true}]); assert!(references(&doc, &json!({}), &scenes(), "scene").contains("knows · Conflicting order"));
        doc["events"].as_array_mut().unwrap().push(json!({"id":"unknown-between","sceneIds":[],"time":{"kind":"unknown"}})); doc["constraints"] = json!([{"beforeId":"start","afterId":"unknown-between","minimumDays":2,"strict":false,"confirmed":true},{"beforeId":"unknown-between","afterId":"middle","minimumDays":2,"strict":false,"confirmed":true}]); assert!(references(&doc, &json!({}), &scenes(), "scene").contains("knows · Conflicting order"));
    }
    #[test] fn missing_scene_fails_closed_and_missing_fact_retains_stale_authored_evidence() {
        assert!(!references(&fixture(), &json!({}), &scenes(), "deleted").contains("نور 明")); let mut doc = fixture(); doc["facts"] = json!([]); let view = references(&doc, &json!({}), &scenes(), "scene"); assert!(view.contains("Referenced fact changed or missing")); assert!(view.contains("Some authored links are missing")); assert!(view.contains("Recorded fact: Name: مِ 中文"));
    }
    #[test] fn custom_field_links_and_scene_metadata_remain_explicit_sources() {
        let mut doc = fixture(); doc["records"][0]["links"] = json!([]); doc["records"][0]["values"] = json!({"links":[{"kind":"scene","id":"other"}]}); doc["knowledge"] = json!([]); let view = references(&doc, &json!({}), &scenes(), "other"); assert!(view.contains("نور 明")); assert!(!view.contains("Garden"));
        let details = json!({"scenes":[{"sceneId":"other","viewpointId":"hero","eventIds":["middle"]}]}); let view = references(&doc, &details, &scenes(), "other"); assert!(view.contains("Middle · Day 2"));
    }
}
