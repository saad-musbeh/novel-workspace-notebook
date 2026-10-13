//! Explicit character knowledge and reader reveals in ordinary manual-save pages.
//! Hosted as an app child module, just like chapter.rs. No prose is inspected.
use super::*;
use std::collections::HashSet;

const MAX_TEXT: usize = 100_000;
const MAX_ROWS: usize = 10_000;
const MAX_POSITION: u64 = 9_007_199_254_740_991;
const PAGE_SIZE: usize = 50;

fn rows<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn find<'a>(doc: &'a Value, key: &str, id: &str) -> Option<&'a Value> {
    rows(doc, key).iter().find(|row| row["id"] == id)
}
fn exact_fact(fact: &Value) -> Result<Value, String> {
    if !["subjectId", "property", "value"].iter().all(|key| {
        fact[*key]
            .as_str()
            .is_some_and(|s| s.chars().count() <= MAX_TEXT)
    }) || text(fact, "subjectId").is_empty()
    {
        return Err("The selected fact has unsupported exact text.".into());
    }
    Ok(json!({"subjectId":fact["subjectId"],"property":fact["property"],"value":fact["value"]}))
}
fn valid_text(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| s.chars().count() <= MAX_TEXT)
}

/// Validate only this explicit author action; unrelated historical rows stay exact.
fn author_knowledge(doc: &Value, entry: &Value) -> Result<(), String> {
    if !find(doc, "records", text(entry, "characterId")).is_some_and(|r| r["kind"] == "character") {
        return Err("Choose an authored character.".into());
    }
    let fact = find(doc, "facts", text(entry, "factId")).ok_or("Choose a retained fact.")?;
    if !matches!(
        entry["stance"].as_str(),
        Some("knows" | "believes" | "misunderstands")
    ) {
        return Err("Choose a character stance.".into());
    }
    if entry["stance"] == "knows" && (fact["confirmed"] != true || fact["status"] != "fact") {
        return Err("Knowing requires an explicitly approved fact.".into());
    }
    if !valid_text(&entry["source"]) || text(entry, "source").trim().is_empty() {
        return Err("Enter an author source / reason.".into());
    }
    if !valid_text(&entry["interpretation"])
        || entry["stance"] != "knows" && text(entry, "interpretation").trim().is_empty()
    {
        return Err("Beliefs and misunderstandings need an explicit interpretation.".into());
    }
    if entry["factSnapshot"] != exact_fact(fact)? {
        return Err("The frozen fact differs from this planning snapshot. Explicitly capture the current fact before reauthoring this entry.".into());
    }
    let scope = &entry["scope"];
    match text(scope, "kind") {
        "scenes" => {
            let ids = scope["sceneIds"]
                .as_array()
                .ok_or("Choose explicit scenes.")?;
            let mut seen = HashSet::new();
            if ids.is_empty()
                || ids.len() > MAX_ROWS
                || !ids.iter().all(|id| {
                    id.as_str()
                        .is_some_and(|id| !id.is_empty() && id.len() <= 200 && seen.insert(id))
                })
            {
                return Err("Choose at least one unique explicit scene.".into());
            }
        }
        "events" => {
            if find(doc, "events", text(scope, "fromEventId")).is_none() {
                return Err("Choose a start event (inclusive).".into());
            }
            if !scope["untilEventId"].is_null()
                && (find(doc, "events", text(scope, "untilEventId")).is_none()
                    || scope["fromEventId"] == scope["untilEventId"])
            {
                return Err("Choose a distinct end event (exclusive), or no declared end.".into());
            }
        }
        _ => return Err("Choose explicit scenes or a story-event range.".into()),
    }
    Ok(())
}
fn author_reveal(doc: &Value, entry: &Value, scenes: &[Value]) -> Result<(), String> {
    let fact = entry["factId"]
        .as_str()
        .and_then(|id| find(doc, "facts", id));
    let event = entry["eventId"]
        .as_str()
        .and_then(|id| find(doc, "events", id));
    if !(fact.is_some() && entry["eventId"].is_null()
        || event.is_some() && entry["factId"].is_null())
    {
        return Err("Choose exactly one retained fact or event.".into());
    }
    if entry["scene"]["kind"] != "scene" || !scenes.iter().any(|s| s["id"] == entry["scene"]["id"])
    {
        return Err("Choose an available manuscript scene.".into());
    }
    if !entry["position"]
        .as_u64()
        .is_some_and(|p| p <= MAX_POSITION)
    {
        return Err("Enter a whole reveal position from 1 to 9007199254740992.".into());
    }
    if !valid_text(&entry["notes"]) {
        return Err("Reveal notes exceed 100,000 characters.".into());
    }
    Ok(())
}
// Every newly authored scene must carry a live source captured by the checked
// planning read. The companion rechecks those exact heads in the save transaction.
fn scene_dependencies(ids: &[String], scenes: &[Value]) -> Result<Vec<Value>, String> {
    if ids.len() > 50 {
        return Err("A checked authored scope supports at most 50 scenes. Choose a smaller explicit scope or an event range; the saved draft is retained.".into());
    }
    let mut seen = HashSet::new();
    ids.iter().map(|id| {
        if !seen.insert(id) { return Err("Choose unique explicit scenes.".into()); }
        let scene = scenes.iter().find(|s| s["id"] == *id)
            .ok_or("A chosen scene is no longer in the captured manuscript. Reload after Save or Discard and choose available scenes explicitly.")?;
        if !scene["revisionId"].as_str().is_some_and(|id| !id.is_empty()) {
            return Err("A chosen scene has no checked saved revision. Reopen this page.".into());
        }
        Ok(json!({"entityId":scene["id"],"entityType":"scene","revisionId":scene["revisionId"]}))
    }).collect()
}
fn replace(doc: &Value, key: &str, id: &str, entry: Option<Value>) -> Result<Value, String> {
    if id.is_empty() || id.len() > 200 {
        return Err(
            "This historical row has no supported stable identity. Its saved data is retained."
                .into(),
        );
    }
    if doc.get(key).is_some_and(|v| !v.is_array()) {
        return Err("Unsupported saved collection. Its data is retained.".into());
    }
    let mut next = doc.clone();
    let mut entries = rows(doc, key).to_vec();
    let matches = entries.iter().filter(|r| r["id"] == id).count();
    if matches > 1 {
        return Err(
            "Duplicate saved identities require explicit project-data conflict handling.".into(),
        );
    }
    if let Some(entry) = entry {
        if let Some(index) = entries.iter().position(|r| r["id"] == id) {
            entries[index] = entry;
        } else {
            if entries.len() >= MAX_ROWS {
                return Err("This collection already has 10,000 entries.".into());
            }
            entries.push(entry);
        }
    } else {
        entries.retain(|r| r["id"] != id);
    }
    next[key] = json!(entries);
    Ok(next)
}
fn excerpt(value: &str) -> String {
    let mut chars = value.chars();
    let mut out: String = chars.by_ref().take(400).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    out
}
/// Saved reader order uses scene identity and explicit position, never story dates.
fn reader_order<'a>(doc: &'a Value, scenes: &[Value]) -> Vec<&'a Value> {
    let order: HashMap<&str, usize> = scenes
        .iter()
        .enumerate()
        .map(|(i, s)| (text(s, "id"), i))
        .collect();
    let mut reveals: Vec<_> = rows(doc, "readerReveals").iter().collect();
    reveals.sort_by_key(|r| {
        (
            order
                .get(text(&r["scene"], "id"))
                .copied()
                .unwrap_or(usize::MAX),
            r["position"].as_u64().unwrap_or(u64::MAX),
            text(r, "id"),
        )
    });
    reveals
}
fn reveal_label(doc: &Value, scenes: &[Value], row: &Value) -> String {
    let scene = scenes.iter().find(|s| s["id"] == row["scene"]["id"]);
    let scene = scene
        .map(|s| text(s, "title"))
        .unwrap_or("Missing scene · placement unresolved");
    let target = row["factId"]
        .as_str()
        .and_then(|id| find(doc, "facts", id))
        .map(|f| format!("Fact · {}: {}", text(f, "property"), text(f, "value")))
        .or_else(|| {
            row["eventId"]
                .as_str()
                .and_then(|id| find(doc, "events", id))
                .map(|e| format!("Event · {}", text(e, "title")))
        })
        .unwrap_or("Missing authored target".into());
    format!(
        "{} · position {} · {}",
        excerpt(scene),
        row["position"]
            .as_u64()
            .and_then(|p| p.checked_add(1))
            .map(|p| p.to_string())
            .unwrap_or("Unresolved".into()),
        excerpt(&target)
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AuthoringKind {
    Knowledge,
    ReaderTimeline,
}
impl AuthoringKind {
    fn key(self) -> &'static str {
        match self {
            Self::Knowledge => "knowledge",
            Self::ReaderTimeline => "readerReveals",
        }
    }
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Knowledge => "Character knowledge",
            Self::ReaderTimeline => "Reader learns timeline",
        }
    }
}

/// Both dedicated pages share loading/CAS/lifecycle, but author different records.
pub(super) struct KnowledgePage {
    pub widget: gtk::Box,
    pub dirty: Cell<bool>,
    pub saving: Cell<bool>,
    pub composing: Cell<bool>,
    pub loaded: Cell<bool>,
    pub kind: AuthoringKind,
    pub status: gtk::Label,
    pub entries: gtk::ComboBoxText,
    pub character: gtk::ComboBoxText,
    pub stance: gtk::ComboBoxText,
    pub fact: gtk::ComboBoxText,
    pub interpretation: gtk::TextView,
    pub source: gtk::TextView,
    pub scope: gtk::ComboBoxText,
    pub scene: gtk::ComboBoxText,
    pub from: gtk::ComboBoxText,
    pub until: gtk::ComboBoxText,
    pub target_kind: gtk::ComboBoxText,
    pub event: gtk::ComboBoxText,
    pub position: gtk::Entry,
    pub notes: gtk::TextView,
    pub snapshot_view: gtk::TextView,
    project_id: String,
    controls: gtk::Box,
    planning: RefCell<Value>,
    parent: RefCell<Value>,
    scenes: RefCell<Vec<Value>>,
    selected: RefCell<Option<String>>,
    snapshot: RefCell<Value>,
    scene_ids: RefCell<Vec<String>>,
    scene_list: gtk::Label,
    scene_controls: gtk::Box,
    removing: Cell<bool>,
    updating: Cell<bool>,
    offset: Cell<usize>,
    entry_count: gtk::Label,
}
fn field(body: &gtk::Box, caption: &str, widget: &impl IsA<gtk::Widget>) {
    let label = gtk::Label::new(Some(caption));
    label.set_xalign(0.);
    label.set_mnemonic_widget(Some(widget));
    body.pack_start(&label, false, false, 0);
    body.pack_start(widget, false, false, 0);
}
fn multiline() -> gtk::TextView {
    let view = gtk::TextView::new();
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_size_request(-1, 90);
    view
}
fn buffer_text(view: &gtk::TextView) -> String {
    let b = view.buffer().unwrap();
    b.text(&b.start_iter(), &b.end_iter(), true)
        .unwrap_or_default()
        .to_string()
}
fn chosen(combo: &gtk::ComboBoxText) -> String {
    combo
        .active_id()
        .map(|id| id.to_string())
        .unwrap_or_default()
}
fn select(combo: &gtk::ComboBoxText, id: &str) {
    if !combo.set_active_id(Some(id)) && !id.is_empty() {
        combo.append(
            Some(id),
            &format!("Missing retained link ({})", excerpt(id)),
        );
        combo.set_active_id(Some(id));
    }
}

impl KnowledgePage {
    pub(super) fn new(project_id: String, kind: AuthoringKind) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 8);
        widget.set_margin_start(16);
        widget.set_margin_end(16);
        widget.set_margin_top(12);
        widget.set_margin_bottom(12);
        let title = gtk::Label::new(Some(kind.title()));
        title.set_xalign(0.);
        title.style_context().add_class("page-title");
        widget.pack_start(&title, false, false, 0);
        let hint = gtk::Label::new(Some(if kind == AuthoringKind::Knowledge {
            "Author each claim explicitly. Frozen fact text is retained when canon changes. Event ranges use occurrence links; unknown or conflicting order stays unresolved."
        } else {
            "Saved reader reveals in manuscript scene order. Position is an author-declared order within the scene, not a text offset. Fictional occurrence dates remain separate."
        }));
        hint.set_line_wrap(true);
        hint.set_xalign(0.);
        widget.pack_start(&hint, false, false, 0);
        let status = gtk::Label::new(Some("Loading saved planning…"));
        status.set_xalign(0.);
        status.set_line_wrap(true);
        status.set_selectable(true);
        widget.pack_start(&status, false, false, 0);
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 6);
        widget.pack_start(&scroll(&controls), true, true, 0);
        let entries = gtk::ComboBoxText::new();
        field(&controls, "Saved entries", &entries);
        let entry_count = gtk::Label::new(None);
        entry_count.set_xalign(0.);
        controls.pack_start(&entry_count, false, false, 0);
        let paging = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        controls.pack_start(&paging, false, false, 0);
        let character = gtk::ComboBoxText::new();
        let stance = gtk::ComboBoxText::new();
        let fact = gtk::ComboBoxText::new();
        for (id, label) in [
            ("knows", "Knows approved fact"),
            ("believes", "Believes"),
            ("misunderstands", "Misunderstands"),
        ] {
            stance.append(Some(id), label);
        }
        stance.set_active(Some(0));
        let interpretation = multiline();
        let source = multiline();
        let snapshot_view = multiline();
        snapshot_view.set_editable(false);
        snapshot_view.set_cursor_visible(false);
        let scope = gtk::ComboBoxText::new();
        scope.append(Some("scenes"), "Explicit scenes");
        scope.append(Some("events"), "Story-event range");
        scope.set_active(Some(0));
        let scene = gtk::ComboBoxText::new();
        let from = gtk::ComboBoxText::new();
        let until = gtk::ComboBoxText::new();
        let scene_list = gtk::Label::new(None);
        scene_list.set_xalign(0.);
        scene_list.set_line_wrap(true);
        scene_list.set_selectable(true);
        let target_kind = gtk::ComboBoxText::new();
        target_kind.append(Some("fact"), "Fact");
        target_kind.append(Some("event"), "Event");
        target_kind.set_active(Some(0));
        let event = gtk::ComboBoxText::new();
        let position = gtk::Entry::new();
        position.set_text("1");
        let notes = multiline();
        let capture_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let scenes_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        if kind == AuthoringKind::Knowledge {
            field(&controls, "Character", &character);
            field(&controls, "Character stance", &stance);
            field(&controls, "Referenced fact", &fact);
            controls.pack_start(&capture_row, false, false, 0);
            field(&controls, "Exact frozen fact (read only)", &snapshot_view);
            field(
                &controls,
                "Character interpretation (required for believes / misunderstands)",
                &interpretation,
            );
            field(&controls, "Author source / reason", &source);
            field(&controls, "Applies in", &scope);
            field(&controls, "Choose an explicit scene", &scene);
            controls.pack_start(&scenes_row, false, false, 0);
            controls.pack_start(&scene_list, false, false, 0);
            field(&controls, "From event (inclusive)", &from);
            field(
                &controls,
                "Until event (exclusive; blank means no declared end)",
                &until,
            );
        } else {
            field(&controls, "Reveal target kind", &target_kind);
            field(&controls, "Referenced fact", &fact);
            field(&controls, "Referenced event", &event);
            field(&controls, "Reader learns in scene", &scene);
            field(&controls, "Reveal position (1 or more)", &position);
            field(&controls, "Reveal notes", &notes);
        }
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        controls.pack_start(&actions, false, false, 0);
        let p = Rc::new(Self {
            widget,
            dirty: Cell::new(false),
            saving: Cell::new(false),
            composing: Cell::new(false),
            loaded: Cell::new(false),
            kind,
            status,
            entries,
            character,
            stance,
            fact,
            interpretation,
            source,
            scope,
            scene,
            from,
            until,
            target_kind,
            event,
            position,
            notes,
            snapshot_view,
            project_id,
            controls,
            planning: RefCell::new(crate::tools::initial("planning")),
            parent: RefCell::new(Value::Null),
            scenes: RefCell::new(Vec::new()),
            selected: RefCell::new(None),
            snapshot: RefCell::new(Value::Null),
            scene_ids: RefCell::new(Vec::new()),
            scene_list,
            scene_controls: scenes_row.clone(),
            removing: Cell::new(false),
            updating: Cell::new(false),
            offset: Cell::new(0),
            entry_count,
        });
        for combo in [
            &p.character,
            &p.stance,
            &p.fact,
            &p.scope,
            &p.from,
            &p.until,
            &p.target_kind,
            &p.event,
        ] {
            let weak = Rc::downgrade(&p);
            combo.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.changed();
                }
            });
        }
        for combo in [&p.fact, &p.stance] {
            let weak = Rc::downgrade(&p);
            combo.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    if p.loaded.get() && !p.updating.get() {
                        p.render_snapshot();
                    }
                }
            });
        }
        if kind == AuthoringKind::ReaderTimeline {
            let weak = Rc::downgrade(&p);
            p.scene.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.changed();
                }
            });
        }
        let weak = Rc::downgrade(&p);
        p.position.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.changed();
            }
        });
        let weak = Rc::downgrade(&p);
        p.position.connect_preedit_changed(move |_, value| {
            if let Some(p) = weak.upgrade() {
                p.composing.set(!value.is_empty());
            }
        });
        for view in [&p.interpretation, &p.source, &p.notes] {
            let weak = Rc::downgrade(&p);
            view.buffer().unwrap().connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.changed();
                }
            });
            let weak = Rc::downgrade(&p);
            view.connect_preedit_changed(move |_, value| {
                if let Some(p) = weak.upgrade() {
                    p.composing.set(!value.is_empty());
                }
            });
        }
        let weak = Rc::downgrade(&p);
        p.entries.connect_changed(move |combo| {
            if let Some(p) = weak.upgrade() {
                if p.updating.get() {
                    return;
                }
                let id = combo.active_id().map(|s| s.to_string());
                if !p.ready_to_replace() {
                    p.updating.set(true);
                    combo.set_active_id(p.selected.borrow().as_deref());
                    p.updating.set(false);
                    return;
                }
                *p.selected.borrow_mut() = id;
                p.restore();
            }
        });
        for (label, step) in [("Previous entries", -1isize), ("Next entries", 1)] {
            let weak = Rc::downgrade(&p);
            button(&paging, label, move || {
                if let Some(p) = weak.upgrade() {
                    if p.ready_to_replace() {
                        let count = rows(&p.planning.borrow(), p.kind.key()).len();
                        let offset = p
                            .offset
                            .get()
                            .saturating_add_signed(step * PAGE_SIZE as isize);
                        if offset < count {
                            p.offset.set(offset);
                            p.list_entries();
                        }
                    }
                }
            });
        }
        let weak = Rc::downgrade(&p);
        button(&paging, "New entry", move || {
            if let Some(p) = weak.upgrade() {
                if p.ready_to_replace() {
                    *p.selected.borrow_mut() = None;
                    p.updating.set(true);
                    p.entries.set_active(None);
                    p.updating.set(false);
                    p.restore();
                }
            }
        });
        let weak = Rc::downgrade(&p);
        button(&capture_row, "Capture current fact", move || {
            if let Some(p) = weak.upgrade() {
                p.capture_fact();
            }
        });
        let weak = Rc::downgrade(&p);
        button(&scenes_row, "Add chosen scene", move || {
            if let Some(p) = weak.upgrade() {
                if p.saving.get() || p.composing.get() || !p.loaded.get() {
                    return;
                }
                let id = chosen(&p.scene);
                if !p.scenes.borrow().iter().any(|s| s["id"] == id) {
                    p.status.set_text("Choose an available scene.");
                    return;
                }
                if !p.scene_ids.borrow().contains(&id) {
                    p.scene_ids.borrow_mut().push(id);
                    p.changed();
                    p.render_scenes();
                }
            }
        });
        let weak = Rc::downgrade(&p);
        button(&scenes_row, "Remove chosen scene from scope", move || {
            if let Some(p) = weak.upgrade() {
                if p.loaded.get() && !p.saving.get() && !p.composing.get() {
                    p.scene_ids
                        .borrow_mut()
                        .retain(|id| id != &chosen(&p.scene));
                    p.changed();
                    p.render_scenes();
                }
            }
        });
        let weak = Rc::downgrade(&p);
        button(&actions, "Save entry", move || {
            if let Some(p) = weak.upgrade() {
                if let Some(owner) = desk::workspace_for_widget(p.widget.upcast_ref()) {
                    p.save(&owner, |_| {});
                }
            }
        });
        let weak = Rc::downgrade(&p);
        button(&actions, "Discard changes", move || {
            if let Some(p) = weak.upgrade() {
                p.discard();
            }
        });
        let weak = Rc::downgrade(&p);
        button(&actions, "Remove entry on Save", move || {
            if let Some(p) = weak.upgrade() {
                if p.ready_to_replace() && p.selected.borrow().is_some() {
                    p.removing.set(true);
                    p.dirty.set(true);
                    p.status.set_text("Entry marked for removal. Save to commit, or Discard to retain it. Earlier revisions remain in history.");
                }
            }
        });
        let weak = Rc::downgrade(&p);
        button(&actions, "Reload saved planning", move || {
            if let Some(p) = weak.upgrade() {
                if p.ready_to_replace() {
                    if let Some(owner) = desk::workspace_for_widget(p.widget.upcast_ref()) {
                        p.load(&owner);
                    }
                }
            }
        });
        p.controls.set_sensitive(false);
        p
    }
    fn changed(&self) {
        if !self.updating.get() && self.loaded.get() {
            self.dirty.set(true);
            self.refresh_controls();
            self.status.set_text("Unsaved entry · Manual Save required");
        }
    }
    fn refresh_controls(&self) {
        if self.kind == AuthoringKind::Knowledge {
            let scenes = chosen(&self.scope) == "scenes";
            self.scene.set_sensitive(scenes);
            self.scene_controls.set_sensitive(scenes);
            self.from.set_sensitive(!scenes);
            self.until.set_sensitive(!scenes);
            self.interpretation
                .set_sensitive(chosen(&self.stance) != "knows");
        } else {
            let fact = chosen(&self.target_kind) == "fact";
            self.fact.set_sensitive(fact);
            self.event.set_sensitive(!fact);
        }
    }
    fn ready_to_replace(&self) -> bool {
        if self.dirty.get() || self.saving.get() || self.composing.get() || !self.loaded.get() {
            self.status.set_text("Save or Discard this entry, and finish composition, before selecting or reloading another entry.");
            false
        } else {
            true
        }
    }
    pub(super) fn capture_fact(&self) {
        if self.saving.get() || self.composing.get() || !self.loaded.get() {
            return;
        }
        let result = find(&self.planning.borrow(), "facts", &chosen(&self.fact))
            .ok_or("Choose a retained fact.".to_owned())
            .and_then(exact_fact);
        match result {
            Ok(value) => {
                *self.snapshot.borrow_mut() = value;
                self.render_snapshot();
                self.changed();
            }
            Err(e) => self.status.set_text(&e),
        }
    }
    fn render_snapshot(&self) {
        let snapshot = self.snapshot.borrow();
        let doc = self.planning.borrow();
        let current = find(&doc, "facts", &chosen(&self.fact));
        let status = if snapshot.is_null() {
            "Choose a fact and explicitly capture its text."
        } else if current
            .and_then(|f| exact_fact(f).ok())
            .is_none_or(|f| f != *snapshot)
        {
            "Referenced fact changed or missing · frozen text retained"
        } else if self.stance.active_id().as_deref() == Some("knows")
            && current.is_some_and(|f| f["confirmed"] != true || f["status"] != "fact")
        {
            "Referenced fact is not approved · frozen text retained"
        } else {
            "Frozen authored fact"
        };
        self.snapshot_view.buffer().unwrap().set_text(&format!(
            "{status}\nSubject: {}\nProperty: {}\nValue: {}",
            text(&snapshot, "subjectId"),
            text(&snapshot, "property"),
            text(&snapshot, "value")
        ));
    }
    fn render_scenes(&self) {
        let scenes = self.scenes.borrow();
        let ids = self.scene_ids.borrow();
        self.scene_list.set_text(&format!(
            "Explicit scene scope ({}):\n{}",
            ids.len(),
            ids.iter()
                .map(|id| scenes
                    .iter()
                    .find(|s| s["id"] == *id)
                    .map(|s| excerpt(text(s, "title")))
                    .unwrap_or_else(|| format!("Missing scene ({}) · retained", excerpt(id))))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    fn list_entries(&self) {
        self.updating.set(true);
        self.entries.remove_all();
        let doc = self.planning.borrow();
        let scenes = self.scenes.borrow();
        let entries = if self.kind == AuthoringKind::ReaderTimeline {
            reader_order(&doc, &scenes)
        } else {
            rows(&doc, "knowledge").iter().collect()
        };
        let start = self
            .offset
            .get()
            .min(entries.len().saturating_sub(1) / PAGE_SIZE * PAGE_SIZE);
        self.offset.set(start);
        for row in entries.iter().skip(start).take(PAGE_SIZE) {
            let label = if self.kind == AuthoringKind::ReaderTimeline {
                reveal_label(&doc, &scenes, row)
            } else {
                format!(
                    "{} · {} · {}",
                    excerpt(
                        find(&doc, "records", text(row, "characterId"))
                            .map(|r| text(r, "name"))
                            .unwrap_or("Missing character")
                    ),
                    text(row, "stance"),
                    excerpt(text(&row["factSnapshot"], "property"))
                )
            };
            self.entries.append(Some(text(row, "id")), &label);
        }
        if let Some(id) = self.selected.borrow().as_deref() {
            if !self.entries.set_active_id(Some(id)) {
                if let Some(row) = find(&doc, self.kind.key(), id) {
                    self.entries.append(
                        Some(id),
                        &format!("Selected entry · {}", excerpt(text(row, "id"))),
                    );
                    self.entries.set_active_id(Some(id));
                }
            }
        }
        self.entry_count.set_text(&format!(
            "{} saved entries · showing {}–{} · captured planning revision {}",
            entries.len(),
            if entries.is_empty() { 0 } else { start + 1 },
            (start + PAGE_SIZE).min(entries.len()),
            self.parent.borrow().as_str().unwrap_or("none")
        ));
        self.updating.set(false);
    }
    fn restore(&self) {
        self.updating.set(true);
        self.removing.set(false);
        let doc = self.planning.borrow();
        let selected = self.selected.borrow();
        let row = selected
            .as_deref()
            .and_then(|id| find(&doc, self.kind.key(), id))
            .cloned()
            .unwrap_or(Value::Null);
        for (combo, id) in [
            (&self.character, text(&row, "characterId")),
            (&self.fact, text(&row, "factId")),
            (&self.event, text(&row, "eventId")),
            (&self.from, text(&row["scope"], "fromEventId")),
            (&self.until, text(&row["scope"], "untilEventId")),
        ] {
            select(combo, id);
        }
        select(&self.stance, row["stance"].as_str().unwrap_or("knows"));
        select(
            &self.scope,
            row["scope"]["kind"].as_str().unwrap_or("scenes"),
        );
        select(
            &self.target_kind,
            if row["eventId"].is_string() {
                "event"
            } else {
                "fact"
            },
        );
        select(&self.scene, text(&row["scene"], "id"));
        for (view, key) in [
            (&self.interpretation, "interpretation"),
            (&self.source, "source"),
            (&self.notes, "notes"),
        ] {
            view.buffer().unwrap().set_text(text(&row, key));
        }
        self.position.set_text(
            &row["position"]
                .as_u64()
                .and_then(|p| p.checked_add(1))
                .unwrap_or(1)
                .to_string(),
        );
        *self.snapshot.borrow_mut() = row["factSnapshot"].clone();
        *self.scene_ids.borrow_mut() = rows(&row["scope"], "sceneIds")
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        // Retained missing scene links can be explicitly removed from a scope.
        for id in self.scene_ids.borrow().iter() {
            select(&self.scene, id);
        }
        select(&self.scene, text(&row["scene"], "id"));
        self.dirty.set(false);
        self.updating.set(false);
        drop(doc);
        drop(selected);
        self.render_snapshot();
        self.render_scenes();
        self.refresh_controls();
        self.status
            .set_text("Saved planning snapshot · Manual saving");
    }
    pub(super) fn discard(&self) {
        if !self.saving.get() && !self.composing.get() {
            self.restore();
        }
    }
    pub(super) fn load(self: &Rc<Self>, owner: &Rc<Workspace>) {
        if self.dirty.get() || self.saving.get() || self.composing.get() {
            return;
        }
        self.loaded.set(false);
        self.controls.set_sensitive(false);
        self.status
            .set_text("Loading saved planning and manuscript order…");
        let p = self.clone();
        owner.client.rpc(
            "planning_read_snapshot",
            json!({"projectId":self.project_id}),
            move |reply| {
                if p.owner().is_none() { return; }
                match reply {
                    Ok(snapshot) => {
                        // The companion checks conflicts, checksums and placement
                        // in one read transaction. Never fall back to projections.
                        let index = &snapshot["index"];
                        let Some(saved) = snapshot.get("planning") else {
                            p.status.set_text("Unsupported checked planning snapshot. Reopen this page.");
                            return;
                        };
                        if index["project"]["id"] != p.project_id || !index["scenes"].is_array()
                            || !saved.is_null() && (!saved["payload"].is_object() || !saved["revisionId"].is_string())
                        {
                            p.status.set_text("Unsupported checked planning snapshot. Reopen this page.");
                            return;
                        }
                        *p.scenes.borrow_mut() = rows(index, "scenes").to_vec();
                        *p.planning.borrow_mut() = if saved.is_null() {
                            crate::tools::initial("planning")
                        } else {
                            saved["payload"].clone()
                        };
                        *p.parent.borrow_mut() = saved["revisionId"].clone();
                        p.populate(); p.loaded.set(true); p.controls.set_sensitive(true);
                        p.list_entries(); p.restore();
                    },
                    Err(e) => p.status.set_text(&format!("Could not load checked planning and manuscript order. Reopen this page after resolving any conflicts. {e}")),
                }
            },
        );
    }

    fn owner(&self) -> Option<Rc<Workspace>> {
        desk::workspace_for_widget(self.widget.upcast_ref()).filter(|a| {
            !a.desk.is_closed()
                && *a.pid.borrow() == self.project_id
                && a.pages.page_num(&self.widget).is_some()
        })
    }
    fn populate(&self) {
        self.updating.set(true);
        let doc = self.planning.borrow();
        for (combo, key, label) in [
            (&self.character, "records", "name"),
            (&self.fact, "facts", "property"),
            (&self.event, "events", "title"),
            (&self.from, "events", "title"),
            (&self.until, "events", "title"),
        ] {
            combo.remove_all();
            combo.append(Some(""), "Choose…");
            for row in rows(&doc, key)
                .iter()
                .filter(|row| key != "records" || row["kind"] == "character")
            {
                let description = if key == "facts" {
                    format!(
                        "{}: {} · {}",
                        excerpt(text(row, label)),
                        excerpt(text(row, "value")),
                        if row["confirmed"] == true && row["status"] == "fact" {
                            "Approved"
                        } else {
                            "Not approved for knows"
                        }
                    )
                } else {
                    excerpt(text(row, label))
                };
                combo.append(Some(text(row, "id")), &description);
            }
        }
        self.scene.remove_all();
        self.scene.append(Some(""), "Choose…");
        for (i, s) in self.scenes.borrow().iter().enumerate() {
            self.scene.append(
                Some(text(s, "id")),
                &format!("{}. {}", i + 1, excerpt(text(s, "title"))),
            );
        }
        self.updating.set(false);
    }
    fn candidate(&self) -> Result<(Value, Vec<Value>, String), String> {
        let doc = self.planning.borrow();
        let id = self
            .selected
            .borrow()
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if self.removing.get() {
            return Ok((replace(&doc, self.kind.key(), &id, None)?, Vec::new(), id));
        }
        let authored = if self.kind == AuthoringKind::Knowledge {
            let scope = if chosen(&self.scope) == "events" {
                let end = chosen(&self.until);
                json!({"kind":"events","fromEventId":chosen(&self.from),"untilEventId":if end.is_empty(){Value::Null}else{json!(end)}})
            } else {
                json!({"kind":"scenes","sceneIds":self.scene_ids.borrow().clone()})
            };
            let entry = json!({"id":id,"characterId":chosen(&self.character),"factId":chosen(&self.fact),"stance":chosen(&self.stance),"factSnapshot":self.snapshot.borrow().clone(),"interpretation":buffer_text(&self.interpretation),"source":buffer_text(&self.source),"scope":scope});
            entry
        } else {
            let position = self
                .position
                .text()
                .parse::<u64>()
                .ok()
                .and_then(|p| p.checked_sub(1))
                .filter(|p| *p <= MAX_POSITION)
                .ok_or("Enter a whole reveal position from 1 to 9007199254740992.")?;
            let fact = chosen(&self.target_kind) == "fact";
            let entry = json!({"id":id,"factId":if fact{json!(chosen(&self.fact))}else{Value::Null},"eventId":if fact{Value::Null}else{json!(chosen(&self.event))},"scene":{"kind":"scene","id":chosen(&self.scene)},"position":position,"notes":buffer_text(&self.notes)});
            entry
        };
        // Retain additional historical fields when explicitly editing a row.
        let mut entry = find(&doc, self.kind.key(), &id)
            .cloned()
            .unwrap_or_else(|| json!({}));
        for (key, value) in authored.as_object().unwrap() {
            entry[key] = value.clone();
        }
        if find(&doc, self.kind.key(), &id) != Some(&entry) {
            if self.kind == AuthoringKind::Knowledge {
                author_knowledge(&doc, &entry)?;
            } else {
                author_reveal(&doc, &entry, &self.scenes.borrow())?;
            }
        }
        let scene_ids = if self.kind == AuthoringKind::ReaderTimeline {
            vec![chosen(&self.scene)]
        } else if chosen(&self.scope) == "scenes" {
            self.scene_ids.borrow().clone()
        } else {
            Vec::new()
        };
        let dependencies = if find(&doc, self.kind.key(), &id) == Some(&entry) {
            // An unchanged historical row is retained, including its old scope.
            Vec::new()
        } else {
            scene_dependencies(&scene_ids, &self.scenes.borrow())?
        };
        Ok((
            replace(&doc, self.kind.key(), &id, Some(entry))?,
            dependencies,
            id,
        ))
    }
    pub(super) fn save(
        self: &Rc<Self>,
        owner: &Rc<Workspace>,
        callback: impl FnOnce(Result<(), String>) + 'static,
    ) {
        let result = (|| {
            if self.saving.get() || !self.loaded.get() {
                return Err("Wait for the page's saved planning snapshot.".to_owned());
            }
            if self.composing.get() {
                return Err("Finish the current input composition before saving.".to_owned());
            }
            let current = self
                .owner()
                .ok_or("This page no longer has a current novel owner.")?;
            if !Rc::ptr_eq(&current, owner) {
                return Err("Save this page through its current window.".into());
            }
            if !self.dirty.get() {
                return Ok(None);
            }
            self.candidate().map(Some)
        })();
        let (payload, dependencies, id) = match result {
            Ok(Some(value)) => value,
            Ok(None) => {
                callback(Ok(()));
                return;
            }
            Err(e) => {
                self.status.set_text(&e);
                callback(Err(e));
                return;
            }
        };
        if payload == *self.planning.borrow() {
            self.restore();
            callback(Ok(()));
            return;
        }
        self.saving.set(true);
        self.controls.set_sensitive(false);
        self.status.set_text("Saving authored entry…");
        let p = self.clone();
        owner.client.rpc("save_project_data_checked", json!({"projectId":self.project_id,"kind":"planning","payload":payload,"parent":self.parent.borrow().clone(),"dependencies":dependencies}), move |reply| {
            p.saving.set(false); p.controls.set_sensitive(true);
            match reply {
                Ok(saved) => {
                    *p.planning.borrow_mut() = saved["payload"].clone(); *p.parent.borrow_mut() = saved["revisionId"].clone();
                    *p.selected.borrow_mut() = if p.removing.get() { None } else { Some(id) }; p.dirty.set(false); p.list_entries(); p.restore(); p.status.set_text("Saved locally · Earlier planning revisions retained"); callback(Ok(()));
                }, Err(e) => { p.status.set_text(&format!("Entry not saved; draft retained. Reload after Save or Discard to review newer planning. {e}")); callback(Err(e)); },
            }
        });
    }
}
impl Workspace {
    pub(super) fn character_knowledge(self: &Rc<Self>) {
        self.open_authored_planning(AuthoringKind::Knowledge);
    }
    pub(super) fn reader_timeline(self: &Rc<Self>) {
        self.open_authored_planning(AuthoringKind::ReaderTimeline);
    }
    fn open_authored_planning(self: &Rc<Self>, kind: AuthoringKind) {
        let a = desk::effective(self);
        let pid = a.pid.borrow().clone();
        if pid.is_empty() {
            a.error("Choose a novel first.");
            return;
        }
        let out = a.clone();
        a.navigate(move || {
            let p = KnowledgePage::new(pid, kind);
            out.knowledge_pages.borrow_mut().push(p.clone());
            out.install(&p.widget, kind.title(), true);
            p.load(&out);
        });
    }
}

#[cfg(test)]
mod pure_tests {
    use super::*;
    fn fixture() -> Value {
        json!({"records":[{"id":"hero","kind":"character"}],"facts":[{"id":"f","subjectId":"hero","property":"Secret","value":"مِ 中文 𠀀 \u{e000} 👩🏽‍🚀","status":"fact","confirmed":true},{"id":"proposal","subjectId":"hero","property":"Guess","value":"unapproved","status":"proposal","confirmed":false}],"events":[{"id":"early","title":"Early occurrence","time":{"kind":"fixed","day":1}},{"id":"late","title":"Late occurrence","time":{"kind":"fixed","day":20}}],"knowledge":[],"readerReveals":[]})
    }
    fn claim(doc: &Value) -> Value {
        json!({"id":"k","characterId":"hero","factId":"f","stance":"knows","factSnapshot":exact_fact(&doc["facts"][0]).unwrap(),"interpretation":"","source":"Author says so مِ 中文","scope":{"kind":"scenes","sceneIds":["s"]}})
    }
    #[test]
    fn knowledge_freezes_exact_multilingual_text_without_mutating_canon() {
        let doc = fixture();
        let before = doc.clone();
        let entry = claim(&doc);
        author_knowledge(&doc, &entry).unwrap();
        let next = replace(&doc, "knowledge", "k", Some(entry.clone())).unwrap();
        assert_eq!(
            next["knowledge"][0]["factSnapshot"]["value"],
            doc["facts"][0]["value"]
        );
        assert_eq!(doc, before);
        assert_eq!(next["facts"], doc["facts"]);
        let mut changed = next.clone();
        changed["facts"][0]["value"] = json!("New canon");
        assert!(author_knowledge(&changed, &entry)
            .unwrap_err()
            .contains("frozen"));
        let saved = replace(&changed, "readerReveals", "r", Some(json!({"id":"r"}))).unwrap();
        assert_eq!(saved["knowledge"], next["knowledge"]);
    }
    #[test]
    fn only_approved_facts_can_be_known_and_beliefs_require_interpretation() {
        let doc = fixture();
        let mut entry = claim(&doc);
        entry["factId"] = json!("proposal");
        entry["factSnapshot"] = exact_fact(&doc["facts"][1]).unwrap();
        assert!(author_knowledge(&doc, &entry)
            .unwrap_err()
            .contains("approved"));
        for stance in ["believes", "misunderstands"] {
            entry["stance"] = json!(stance);
            assert!(author_knowledge(&doc, &entry)
                .unwrap_err()
                .contains("interpretation"));
            entry["interpretation"] = json!("Explicit interpretation مِ 中文");
            author_knowledge(&doc, &entry).unwrap();
            entry["interpretation"] = json!("");
        }
        entry = claim(&doc);
        entry["source"] = json!("  ");
        assert!(author_knowledge(&doc, &entry).is_err());
    }
    #[test]
    fn scope_requires_explicit_scenes_or_distinct_retained_events() {
        let doc = fixture();
        let mut entry = claim(&doc);
        entry["scope"]["sceneIds"] = json!([]);
        assert!(author_knowledge(&doc, &entry).is_err());
        entry["scope"]["sceneIds"] = json!(["s", "s"]);
        assert!(author_knowledge(&doc, &entry).is_err());
        entry["scope"] = json!({"kind":"events","fromEventId":"early","untilEventId":"late"});
        author_knowledge(&doc, &entry).unwrap();
        entry["scope"]["untilEventId"] = json!("early");
        assert!(author_knowledge(&doc, &entry).is_err());
        entry["scope"]["untilEventId"] = Value::Null;
        author_knowledge(&doc, &entry).unwrap();
    }
    #[test]
    fn reveal_order_tracks_manuscript_and_positions_separately_from_dates() {
        let mut doc = fixture();
        let scenes = vec![
            json!({"id":"first","title":"First"}),
            json!({"id":"second","title":"Second"}),
        ];
        doc["readerReveals"] = json!([{"id":"a","factId":null,"eventId":"early","scene":{"kind":"scene","id":"second"},"position":0,"notes":""},{"id":"b","factId":null,"eventId":"late","scene":{"kind":"scene","id":"first"},"position":9,"notes":""},{"id":"c","factId":"f","eventId":null,"scene":{"kind":"scene","id":"first"},"position":0,"notes":""}]);
        let before = doc.clone();
        for r in rows(&doc, "readerReveals") {
            author_reveal(&doc, r, &scenes).unwrap();
        }
        assert_eq!(
            reader_order(&doc, &scenes)
                .iter()
                .map(|r| text(r, "id"))
                .collect::<Vec<_>>(),
            ["c", "b", "a"]
        );
        let mut moved = scenes.clone();
        moved.reverse();
        assert_eq!(text(reader_order(&doc, &moved)[0], "id"), "a");
        assert_eq!(doc, before);
    }
    #[test]
    fn reveals_require_one_target_available_scene_and_safe_declared_position() {
        let doc = fixture();
        let scenes = vec![json!({"id":"s"})];
        let mut r = json!({"id":"r","factId":"f","eventId":null,"scene":{"kind":"scene","id":"s"},"position":MAX_POSITION,"notes":"مِ 中文"});
        author_reveal(&doc, &r, &scenes).unwrap();
        r["position"] = json!(MAX_POSITION + 1);
        assert!(author_reveal(&doc, &r, &scenes).is_err());
        r["position"] = json!(0);
        r["eventId"] = json!("early");
        assert!(author_reveal(&doc, &r, &scenes).is_err());
        r["eventId"] = Value::Null;
        r["scene"]["id"] = json!("missing");
        assert!(author_reveal(&doc, &r, &scenes).is_err());
        assert!(reveal_label(&doc, &scenes, &r).contains("unresolved"));
    }
    #[test]
    fn author_actions_preserve_unrelated_historical_rows_and_refuse_duplicate_ids() {
        let mut doc = fixture();
        doc["knowledge"] = json!([{"id":"old","factSnapshot":{"value":"Retained stale text"},"unknownLegacyField":true}]);
        let next = replace(&doc, "knowledge", "k", Some(claim(&doc))).unwrap();
        assert_eq!(next["knowledge"][0], doc["knowledge"][0]);
        let removed = replace(&next, "knowledge", "k", None).unwrap();
        assert_eq!(removed["knowledge"], doc["knowledge"]);
        doc["knowledge"] = json!([{"id":"old"},{"id":"old"}]);
        assert!(replace(&doc, "knowledge", "old", None).is_err());
    }
    #[test]
    fn new_scene_scopes_require_every_exact_live_captured_source() {
        let scenes = vec![json!({"id":"s","revisionId":"r","title":"مِ 中文"})];
        assert_eq!(scene_dependencies(&["s".into()], &scenes).unwrap(),
            vec![json!({"entityId":"s","entityType":"scene","revisionId":"r"})]);
        assert!(scene_dependencies(&["s".into(), "deleted".into()], &scenes).is_err());
        assert!(scene_dependencies(&["s".into(), "s".into()], &scenes).is_err());
        assert!(scene_dependencies(&["s".into()], &[json!({"id":"s"})]).is_err());
    }
    #[test]
    fn checked_scope_limit_never_truncates_authored_scenes() {
        let scenes: Vec<_> = (0..51).map(|i| json!({"id":format!("s{i}"),"revisionId":format!("r{i}")})).collect();
        let ids: Vec<_> = scenes.iter().map(|s| text(s,"id").to_owned()).collect();
        assert_eq!(scene_dependencies(&ids[..50], &scenes).unwrap().len(), 50);
        assert!(scene_dependencies(&ids, &scenes).unwrap_err().contains("50"));
    }

}
