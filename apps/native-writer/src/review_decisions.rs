//! Revision-bound author decisions. Root registers this page with normal notebook
//! drafts/ownership; only the explicit Apply button edits a scene, never its store.
//!
//! Read settings through load_project_data_unambiguous. Immediately before any
//! editor mutation, review_sources_check atomically verifies every proposal source
//! in one coherent read: project ownership, sole current heads and SHA-256 over
//! exact UTF-8 plain-text bytes. Its ordered source echo must match our manifest.
//! Individual source reads below validate quotations; they are not that guard.
//! Acceptance then uses save_project_data_checked with every captured revision
//! as a SourceDependency, checking sources again together with settings CAS.
//! Failure after the swap retains the unsaved scene draft and Undo and explicitly
//! says the review decision was not accepted. No manuscript save is performed.
use super::*;
use crate::process::Reply;

fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v[key]
        .as_str()
        .ok_or_else(|| format!("Missing review {key}."))
}
fn offset(v: &Value) -> Result<usize, String> {
    usize::try_from(v.as_u64().ok_or("Invalid review offset.")?)
        .map_err(|_| "Review offset exceeds this platform.".into())
}
/// UTF-16 evidence must land on exact scalar boundaries, including astral text.
fn scalar_at(text: &str, units: usize) -> Result<usize, String> {
    let mut position = 0;
    for (index, c) in text.chars().enumerate() {
        if position == units {
            return Ok(index);
        }
        position += c.len_utf16();
        if position > units {
            return Err("Review evidence splits a Unicode scalar.".into());
        }
    }
    if position == units {
        Ok(text.chars().count())
    } else {
        Err("Review evidence is outside the saved text.".into())
    }
}
fn same_source(a: &Value, b: &Value) -> bool {
    ["sceneId", "revisionId", "hash"]
        .iter()
        .all(|k| a[*k].is_string() && a[*k] == b[*k])
}
fn validate_source(source: &Value) -> Result<(), String> {
    for key in ["sceneId", "revisionId"] {
        let s = string(source, key)?;
        if s.is_empty() || s.len() > 200 {
            return Err("Invalid review source identity.".into());
        }
    }
    let hash = string(source, "hash")?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("Review requires an exact SHA-256 hash.".into());
    }
    Ok(())
}
/// Canonical wire manifest for the root's atomic all-source guard. A scene
/// cannot supply two current revisions; reject repeated scene IDs even when
/// their revision IDs differ. Model-supplied extra source fields are not sent.
fn apply_sources(s: &Value) -> Result<Value, String> {
    let sources = s["sources"]
        .as_array()
        .filter(|sources| !sources.is_empty() && sources.len() <= 50)
        .ok_or("Apply requires one to fifty unique saved sources.")?;
    let mut seen = std::collections::HashSet::new();
    let mut manifest = Vec::with_capacity(sources.len());
    for source in sources {
        validate_source(source)?;
        if !seen.insert(string(source, "sceneId")?) {
            return Err("Apply requires unique scene sources.".into());
        }
        manifest.push(json!({"sceneId":source["sceneId"],"revisionId":source["revisionId"],"hash":source["hash"]}));
    }
    Ok(json!(manifest))
}
/// Produces the only mutating command after the atomic reply has verified ALL
/// sources in their captured order. Errors, subsets, changed hashes/revisions
/// and out-of-order echoes cannot produce an editor mutation.
fn checked_swap(
    sources: &Value,
    reply: Reply,
    anchor: &Value,
    replacement: &Value,
) -> Result<Value, String> {
    let expected = apply_sources(&json!({"sources":sources}))?;
    if &expected != sources {
        return Err("Invalid Apply source manifest.".into());
    }
    let checked =
        reply.map_err(|e| format!("Saved sources were not verified; no edit applied. {e}"))?;
    if checked["sources"] != expected {
        return Err("Saved review sources did not match the atomic check; no edit applied. Run a fresh review.".into());
    }
    Ok(
        json!({"type":"command","command":"swapParagraph","anchor":anchor,"replacement":replacement}),
    )
}
fn save_failure(applied: bool, error: &str) -> String {
    let prefix = if applied {
        "Applied to the unsaved scene draft, but the review decision was not accepted: its record save failed. Undo and manual scene Save remain available."
    } else {
        "Review record save failed; proposals and pending decisions retained."
    };
    format!("{prefix} Retry Save review records or discard pending decisions. {error}")
}
fn validate_suggestion(s: &Value) -> Result<(), String> {
    for (key, max) in [("id", 300), ("title", 300), ("explanation", 5000)] {
        let text = string(s, key)?;
        if text.encode_utf16().count() > max || (key != "explanation" && text.is_empty()) {
            return Err("Invalid review description.".into());
        }
    }
    if !matches!(
        s["origin"].as_str(),
        Some("manual" | "rule" | "proofreading" | "llm" | "lexical")
    ) || !matches!(
        s["status"].as_str(),
        Some("pending" | "accepted" | "rejected")
    ) {
        return Err("Invalid review origin or decision.".into());
    }
    let sources = s["sources"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 50)
        .ok_or("Invalid review scope.")?;
    let mut seen = std::collections::HashSet::new();
    for source in sources {
        validate_source(source)?;
        if !seen.insert((string(source, "sceneId")?, string(source, "revisionId")?)) {
            return Err("Duplicate review source.".into());
        }
    }
    let evidence = s["evidence"]
        .as_array()
        .filter(|a| a.len() <= 100)
        .ok_or("Invalid review evidence.")?;
    for anchor in evidence {
        validate_source(anchor)?;
        if !sources.iter().any(|source| same_source(source, anchor))
            || anchor["offsetUnit"] != "utf16"
        {
            return Err("Evidence is outside the review scope.".into());
        }
        let start = offset(&anchor["start"])?;
        let end = offset(&anchor["end"])?;
        if start >= end
            || string(anchor, "quote")?.encode_utf16().count() != end - start
            || string(anchor, "prefix")?.encode_utf16().count() > 80
            || string(anchor, "suffix")?.encode_utf16().count() > 80
        {
            return Err("Invalid review quote or context.".into());
        }
    }
    match s["action"]["kind"].as_str() {
        Some("replace")
            if evidence.len() == 1
                && string(&s["action"], "replacement")?.encode_utf16().count() <= 10000 =>
        {
            ()
        }
        Some("question") => (),
        Some("fact") if !evidence.is_empty() => {
            for k in ["subject", "property", "value"] {
                let v = string(&s["action"], k)?;
                if v.is_empty() || v.encode_utf16().count() > 1000 {
                    return Err("Invalid proposed fact.".into());
                }
            }
        }
        _ => return Err("Unsupported review action.".into()),
    }
    Ok(())
}
/// Returns one rich paragraph replacement. Hash is computed by the existing
/// text_hash RPC from the freshly loaded saved document, never from model claims.
fn checked_replacement(
    s: &Value,
    saved: &Value,
    verified_hash: &Value,
) -> Result<(usize, Value), String> {
    validate_suggestion(s)?;
    if s["status"] != "pending" || s["action"]["kind"] != "replace" {
        return Err(
            "Choose a pending text replacement. Questions and facts remain proposals.".into(),
        );
    }
    let anchor = &s["evidence"][0];
    if anchor["sceneId"] != saved["id"]
        || anchor["revisionId"] != saved["revisionId"]
        || anchor["hash"] != *verified_hash
    {
        return Err("The saved scene revision or hash changed. Run a new review.".into());
    }
    let document = &saved["scene"]["document"];
    let text = novel_storage::plain_text(document);
    let start = scalar_at(&text, offset(&anchor["start"])?)?;
    let end = scalar_at(&text, offset(&anchor["end"])?)?;
    let chars = text.chars().collect::<Vec<_>>();
    if chars[start..end].iter().collect::<String>() != string(anchor, "quote")?
        || !chars[..start]
            .iter()
            .collect::<String>()
            .ends_with(string(anchor, "prefix")?)
        || !chars[end..]
            .iter()
            .collect::<String>()
            .starts_with(string(anchor, "suffix")?)
    {
        return Err("Review quote or context does not match the saved scene.".into());
    }
    let replacement = string(&s["action"], "replacement")?;
    let mut position = 0;
    for (index, block) in document["content"]
        .as_array()
        .ok_or("Missing document paragraphs.")?
        .iter()
        .enumerate()
    {
        let length = novel_storage::plain_text(block).chars().count();
        if start >= position && end <= position + length {
            if !matches!(block["type"].as_str(), Some("paragraph" | "heading")) {
                return Err("This block must be edited manually.".into());
            }
            let left = start - position;
            let right = end - position;
            let mut content = novel_editor::model::slice_runs(block, 0, left);
            let template = novel_editor::model::slice_runs(block, left, right)
                .into_iter()
                .find(|n| n["type"] == "text")
                .unwrap_or(json!({"type":"text"}));
            for (i, part) in replacement.split('\n').enumerate() {
                if i > 0 {
                    content.push(json!({"type":"hardBreak"}));
                }
                if !part.is_empty() {
                    let mut node = template.clone();
                    node["text"] = json!(part);
                    content.push(node);
                }
            }
            content.extend(novel_editor::model::slice_runs(block, right, length));
            let mut edited = block.clone();
            edited["content"] = json!(content);
            let mut next = document.clone();
            next["content"][index] = edited.clone();
            novel_storage::validate(&novel_storage::Scene {
                schema_version: 1,
                title: String::new(),
                document: next.clone(),
            })?;
            let expected = format!(
                "{}{}{}",
                chars[..start].iter().collect::<String>(),
                replacement,
                chars[end..].iter().collect::<String>()
            );
            if novel_storage::plain_text(&next) != expected {
                return Err("Replacement cannot preserve the rich document.".into());
            }
            return Ok((index, edited));
        }
        position += length + 2; // storage's exact paragraph separator, not editor caret units
    }
    Err("This replacement crosses paragraphs. Edit those paragraphs manually.".into())
}
fn pane_matches(pane: &EditorPane, saved: &Value) -> bool {
    pane.open.get()
        && !pane.dirty.get()
        && !pane.composing.get()
        && !pane.saving.get()
        && pane.pending.get() == 0
        && *pane.parent.borrow() == saved["revisionId"]
        && pane.frame.borrow()["document"] == saved["scene"]["document"]
}
fn suggestions(value: &Value) -> Vec<Value> {
    value["assistance"]["suggestions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}
fn initialized_settings(mut value: Value) -> Value {
    if value.is_null() {
        return crate::tools::initial("settings");
    }
    if value.get("assistance").is_none() {
        value["assistance"] = crate::tools::initial("settings")["assistance"].clone();
    }
    value
}
fn decided(value: &Value, id: &str, status: &str) -> Result<Value, String> {
    if !matches!(status, "accepted" | "rejected") {
        return Err("Invalid author decision.".into());
    }
    let mut next = value.clone();
    let target = next["assistance"]["suggestions"]
        .as_array_mut()
        .and_then(|v| v.iter_mut().find(|s| s["id"] == id))
        .ok_or("Choose a retained proposal.")?;
    validate_suggestion(target)?;
    if target["status"] != "pending" {
        return Err("This proposal already has an author decision.".into());
    }
    target["status"] = json!(status);
    Ok(next)
}

pub(super) struct ReviewDecisionsPage {
    pub widget: gtk::Box,
    pub status: gtk::Label,
    pub choices: gtk::ComboBoxText,
    pub dirty: Cell<bool>,
    pub saving: Cell<bool>,
    pub composing: Cell<bool>,
    pub loaded: Cell<bool>,
    project_id: String,
    scene_pane: RefCell<std::rc::Weak<EditorPane>>,
    value: RefCell<Value>,
    confirmed: RefCell<Value>,
    parent: RefCell<Value>,
    details: gtk::TextView,
    apply_button: gtk::Button,
    reject_button: gtk::Button,
    updating: Cell<bool>,
    applied_pending: Cell<bool>,
    dependencies: RefCell<Vec<Value>>,
    #[cfg(test)]
    before_sources_check: RefCell<Option<Box<dyn FnOnce()>>>,
    #[cfg(test)]
    before_decision_save: RefCell<Option<Box<dyn FnOnce()>>>,
}
impl ReviewDecisionsPage {
    pub(super) fn new(project_id: String) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        widget.set_margin_start(32);
        widget.set_margin_end(32);
        widget.set_margin_top(26);
        let heading = gtk::Label::new(Some("Local review · author decisions"));
        heading.set_xalign(0.);
        heading.style_context().add_class("page-title");
        widget.pack_start(&heading, false, false, 0);
        let note=gtk::Label::new(Some("Choose a proposal to inspect. Apply edits an unsaved scene draft once; use Undo or manual Save. Questions and proposed facts remain review records."));
        note.set_line_wrap(true);
        note.set_xalign(0.);
        widget.pack_start(&note, false, false, 0);
        let choices = gtk::ComboBoxText::new();
        widget.pack_start(&choices, false, false, 0);
        let details = gtk::TextView::new();
        details.set_editable(false);
        details.set_cursor_visible(false);
        details.set_wrap_mode(gtk::WrapMode::WordChar);
        widget.pack_start(&scroll(&details), true, true, 0);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        widget.pack_start(&row, false, false, 0);
        let apply_button = button(&row, "Apply selected replacement to draft", || {});
        let reject_button = button(&row, "Reject selected proposal", || {});
        let retry = button(&row, "Save review records", || {});
        let open_scene = button(&row, "Open reviewed scene", || {});
        let discard = button(&row, "Discard pending decisions", || {});
        let status = gtk::Label::new(Some("Loading retained proposals…"));
        status.set_line_wrap(true);
        status.set_xalign(0.);
        status.set_selectable(true);
        widget.pack_start(&status, false, false, 0);
        let initial = crate::tools::initial("settings");
        let p = Rc::new(Self {
            widget,
            status,
            choices,
            details,
            apply_button,
            reject_button,
            project_id,
            scene_pane: RefCell::new(std::rc::Weak::new()),
            value: RefCell::new(initial.clone()),
            confirmed: RefCell::new(initial),
            parent: RefCell::new(Value::Null),
            dirty: Cell::new(false),
            saving: Cell::new(false),
            composing: Cell::new(false),
            loaded: Cell::new(false),
            updating: Cell::new(false),
            applied_pending: Cell::new(false),
            dependencies: RefCell::new(Vec::new()),
            #[cfg(test)]
            before_sources_check: RefCell::new(None),
            #[cfg(test)]
            before_decision_save: RefCell::new(None),
        });
        let weak = Rc::downgrade(&p);
        p.choices.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() {
                    p.render_details();
                }
            }
        });
        for (b, action) in [
            (&p.apply_button, 0),
            (&p.reject_button, 1),
            (&retry, 2),
            (&discard, 3),
            (&open_scene, 4),
        ] {
            let weak = Rc::downgrade(&p);
            b.connect_clicked(move |_| {
                if let Some(p) = weak.upgrade() {
                    if let Some(a) = p.owner() {
                        match action {
                            0 => p.apply(&a),
                            1 => p.reject(&a),
                            2 => p.save(&a, |_| {}),
                            3 => {
                                if !p.saving.get() {
                                    p.discard();
                                }
                            }
                            _ => {
                                if let Some(s) = p.selected() {
                                    if let Some(id) = s["evidence"][0]["sceneId"].as_str() {
                                        a.open_scene(id, false);
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }
        p.refresh();
        p
    }
    /// Root binds the existing owned scene context when opening or moving this
    /// page. A weak reference preserves that scene's single session and Undo.
    pub(super) fn bind_scene(&self, pane: &Rc<EditorPane>) {
        *self.scene_pane.borrow_mut() = Rc::downgrade(pane);
    }
    fn owner(&self) -> Option<Rc<Workspace>> {
        desk::workspace_for_widget(self.widget.upcast_ref())
            .filter(|a| *a.pid.borrow() == self.project_id)
    }
    fn selected(&self) -> Option<Value> {
        let id = self.choices.active_id()?;
        suggestions(&self.value.borrow())
            .into_iter()
            .find(|s| s["id"] == id.as_str())
    }
    fn refresh(&self) {
        let id = self.choices.active_id();
        self.updating.set(true);
        self.choices.remove_all();
        for s in suggestions(&self.value.borrow()) {
            self.choices.append(
                s["id"].as_str(),
                &format!(
                    "{} · {}",
                    s["status"].as_str().unwrap_or("invalid"),
                    s["title"].as_str().unwrap_or("Proposal")
                ),
            );
        }
        if let Some(id) = id {
            self.choices.set_active_id(Some(&id));
        }
        if self.choices.active_id().is_none() {
            self.choices.set_active(Some(0));
        }
        self.updating.set(false);
        self.render_details();
    }
    fn render_details(&self) {
        let selected = self.selected();
        let text = selected
            .as_ref()
            .map(|s| {
                format!(
                    "{}\n{}\n\nQuoted original:\n{}\n\nProposed action:\n{}",
                    s["title"].as_str().unwrap_or("Proposal"),
                    s["explanation"].as_str().unwrap_or(""),
                    s["evidence"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|e| e["quote"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                    serde_json::to_string_pretty(&s["action"]).unwrap_or_default()
                )
            })
            .unwrap_or_else(|| {
                "No retained proposals. Run a local review explicitly to create proposals.".into()
            });
        self.details.buffer().unwrap().set_text(&text);
        let ready = self.loaded.get() && !self.saving.get() && !self.dirty.get();
        self.choices.set_sensitive(ready);
        self.apply_button.set_sensitive(
            ready
                && selected
                    .as_ref()
                    .is_some_and(|s| s["status"] == "pending" && s["action"]["kind"] == "replace"),
        );
        self.reject_button
            .set_sensitive(ready && selected.is_some_and(|s| s["status"] == "pending"));
    }
    pub(super) fn load(self: &Rc<Self>, owner: &Rc<Workspace>) {
        let weak = Rc::downgrade(self);
        owner.client.rpc(
            "load_project_data_unambiguous",
            json!({"projectId":self.project_id,"kind":"settings"}),
            move |r| {
                let Some(p) = weak.upgrade() else {
                    return;
                };
                if p.owner().is_none() {
                    return;
                }
                match r {
                    Ok(v) => {
                        let value = initialized_settings(v["payload"].clone());
                        *p.parent.borrow_mut() = v["revisionId"].clone();
                        *p.value.borrow_mut() = value.clone();
                        *p.confirmed.borrow_mut() = value;
                        p.loaded.set(true);
                        p.status.set_text(
                            "Retained proposals · Apply requires exact current saved sources",
                        );
                        p.refresh();
                    }
                    Err(e) => p.status.set_text(&e),
                }
            },
        );
    }
    /// Root calls only after an explicit provider_run reply. IDs are namespaced so
    /// repeated backend IDs cannot erase earlier proposals or decisions.
    pub(super) fn append_result(
        self: &Rc<Self>,
        owner: &Rc<Workspace>,
        result: &Value,
    ) -> Result<(), String> {
        if !self.loaded.get() || self.dirty.get() || self.saving.get() || self.owner().is_none() {
            return Err("Finish loading or saving review records first.".into());
        }
        let incoming = result["suggestions"]
            .as_array()
            .filter(|v| v.len() <= 100)
            .ok_or("Invalid local review result.")?;
        let mut value = self.value.borrow().clone();
        let records = value["assistance"]["suggestions"]
            .as_array_mut()
            .ok_or("Invalid retained proposals.")?;
        if records.len() + incoming.len() > 10000 {
            return Err("Retained review proposal limit reached.".into());
        }
        for item in incoming {
            validate_suggestion(item)?;
            if item["status"] != "pending" {
                return Err("Provider results must remain pending proposals.".into());
            }
            let mut s = item.clone();
            s["id"] = json!(uuid::Uuid::new_v4().to_string());
            records.push(s);
        }
        *self.value.borrow_mut() = value;
        self.dirty.set(true);
        self.refresh();
        self.save(owner, |_| {});
        Ok(())
    }
    pub(super) fn discard(&self) {
        if self.saving.get() {
            return;
        }
        *self.value.borrow_mut() = self.confirmed.borrow().clone();
        self.dirty.set(false);
        self.dependencies.borrow_mut().clear();
        self.applied_pending.set(false);
        self.refresh();
        self.status.set_text(
            "Pending review decisions discarded. Scene edits keep their own Undo and manual Save.",
        );
    }
    fn fail(&self, e: &str) {
        self.saving.set(false);
        self.status.set_text(e);
        self.render_details();
    }
    pub(super) fn save(
        self: &Rc<Self>,
        _owner: &Rc<Workspace>,
        callback: impl FnOnce(Result<(), String>) + 'static,
    ) {
        let Some(a) = self.owner() else {
            callback(Err(
                "The review page is no longer open in this novel.".into()
            ));
            return;
        };
        if !self.loaded.get() || self.saving.get() {
            callback(Err("Review records are still busy.".into()));
            return;
        }
        if !self.dirty.get() {
            callback(Ok(()));
            return;
        }
        self.saving.set(true);
        self.render_details();
        let p = self.clone();
        a.client.rpc("save_project_data_checked",json!({"projectId":self.project_id,"kind":"settings","payload":self.value.borrow().clone(),"parent":self.parent.borrow().clone(),"dependencies":self.dependencies.borrow().clone()}),move|r|{p.saving.set(false);match r{Ok(v)=>{*p.parent.borrow_mut()=v["revisionId"].clone();*p.value.borrow_mut()=v["payload"].clone();*p.confirmed.borrow_mut()=v["payload"].clone();p.dirty.set(false);p.dependencies.borrow_mut().clear();p.applied_pending.set(false);p.refresh();p.status.set_text("Review decision saved. Scene writing still requires manual Save.");callback(Ok(()));},Err(e)=>{p.fail(&save_failure(p.applied_pending.get(),&e));callback(Err(e));}}});
    }
    fn reject(self: &Rc<Self>, a: &Rc<Workspace>) {
        if self.dirty.get() || self.saving.get() || !self.loaded.get() {
            return;
        }
        let Some(s) = self.selected() else {
            return;
        };
        let decision = decided(
            &self.value.borrow(),
            s["id"].as_str().unwrap_or(""),
            "rejected",
        );
        match decision {
            Ok(v) => {
                *self.value.borrow_mut() = v;
                self.dirty.set(true);
                self.save(a, |_| {});
            }
            Err(e) => self.status.set_text(&e),
        }
    }
    fn apply(self: &Rc<Self>, a: &Rc<Workspace>) {
        if self.dirty.get() || self.saving.get() || !self.loaded.get() {
            return;
        }
        let Some(s) = self.selected() else {
            return;
        };
        if let Err(e) = validate_suggestion(&s) {
            self.status.set_text(&e);
            return;
        }
        if s["status"] != "pending" || s["action"]["kind"] != "replace" {
            return;
        }
        if let Err(e) = apply_sources(&s) {
            self.status.set_text(&e);
            return;
        }
        let sid=s["evidence"][0]["sceneId"].as_str().unwrap_or("");
        let target=self.scene_pane.borrow().upgrade().filter(|pane|pane.scene_id==sid).or_else(||a.panes.borrow().get(sid).cloned());
        let Some(target)=target else{self.fail("Open the reviewed scene in this window before Apply.");return;};
        let Some(permit)=sequence_host::tool_input_permit(a,&target)else{self.fail("Finish the current scene operation before Apply.");return;};let input=Rc::new((target,permit));
        self.saving.set(true);
        self.render_details();
        self.status
            .set_text("Checking every saved source before Apply…");
        self.verify_sources(a.clone(), s, 0, Vec::new(), None, input);
    }
    fn verify_sources(
        self: &Rc<Self>,
        a: Rc<Workspace>,
        s: Value,
        index: usize,
        dependencies: Vec<Value>,
        target: Option<(Value, Value)>,
        input: Rc<(Rc<EditorPane>,sequence_host::InputPermit)>,
    ) {
        if !sequence_host::tool_input_allowed(&input.0,&input.1){self.fail("Apply authorization changed; proposal and writing retained.");return;}
        if self.owner().is_none() {
            self.fail("Review page closed before Apply.");
            return;
        }
        let sources = s["sources"].as_array().unwrap();
        if index == sources.len() {
            if let Some((saved, hash)) = target {
                self.apply_checked(a, s, dependencies, saved, hash, input);
            } else {
                self.fail("No manuscript source is available for this replacement.");
            }
            return;
        }
        let source = sources[index].clone();
        let sid = source["sceneId"].as_str().unwrap().to_owned();
        let p = self.clone();
        let client = a.client.clone();
        client.rpc("load_project_scene",json!({"projectId":self.project_id,"sceneId":sid}),move|r|{
            let saved=match r{Ok(v)if v["id"]==source["sceneId"]&&v["revisionId"]==source["revisionId"]=>v,Ok(_)=>{p.fail("A reviewed scene changed. Run a new review before Apply.");return;},Err(e)=>{p.fail(&e);return;}};
            let out=p.clone();let client=a.client.clone();client.rpc("scene_heads",json!({"sceneId":sid}),move|r|{
                match r{Ok(v)if v["revisionIds"]==json!([source["revisionId"]])=>(),Ok(_)=>{out.fail("A reviewed scene has competing or changed heads. Resolve it before Apply.");return;},Err(e)=>{out.fail(&e);return;}}
                let p=out.clone();let client=a.client.clone();client.rpc("text_hash",json!({"text":novel_storage::plain_text(&saved["scene"]["document"])}),move|r|{
                    let hash=match r{Ok(v)if v==source["hash"]=>v,Ok(_)=>{p.fail("A reviewed scene hash changed.");return;},Err(e)=>{p.fail(&e);return;}};
                    let mut deps=dependencies;deps.push(json!({"entityId":sid,"entityType":"scene","revisionId":source["revisionId"]}));let captured=if s["evidence"][0]["sceneId"]==source["sceneId"]{Some((saved,hash))}else{target};p.verify_sources(a,s,index+1,deps,captured,input);
                });
            });
        });
    }
    fn apply_checked(
        self: &Rc<Self>,
        a: Rc<Workspace>,
        s: Value,
        dependencies: Vec<Value>,
        saved: Value,
        hash: Value,
        input: Rc<(Rc<EditorPane>,sequence_host::InputPermit)>,
    ) {
        if !sequence_host::tool_input_allowed(&input.0,&input.1){self.fail("Apply authorization changed; proposal and writing retained.");return;}
        let (index, replacement) = match checked_replacement(&s, &saved, &hash) {
            Ok(v) => v,
            Err(e) => {
                self.fail(&e);
                return;
            }
        };
        let p = self.clone();
        let sid = saved["id"].as_str().unwrap().to_owned();
        let a_ = a.clone();
        // Use an already owned editor. Opening a second hidden session here would
        // lose the source window's draft/Undo ownership. Root routes this page's
        // Apply through the scene owner (or supplies global pane reuse in glue).
        let existing = self
            .scene_pane
            .borrow()
            .upgrade()
            .filter(|pane| pane.scene_id == sid)
            .or_else(|| a.panes.borrow().get(&sid).cloned());
        let Some(existing) = existing else {
            self.fail("Open the reviewed scene in this window before applying its proposal.");
            return;
        };
        let apply_to_pane = move |r: Result<Rc<EditorPane>, String>| {
            let pane = match r {
                Ok(v) => v,
                Err(e) => {
                    p.fail(&e);
                    return;
                }
            };
            if !Rc::ptr_eq(&pane,&input.0) || !sequence_host::tool_input_allowed(&pane,&input.1) || p.owner().is_none() || !pane_matches(&pane, &saved) {
                p.fail("Open and save the current scene; finish pending input before Apply.");
                return;
            }
            let out = p.clone();
            let captured = pane.clone();
            pane.request(json!({"type":"command","command":"captureParagraph","blockIndex":index}),false,move|r:Reply|{
                let anchor=match r{Ok(v)=>v["paragraphCapture"].clone(),Err(e)=>{out.fail(&e);return;}};
                if !sequence_host::tool_input_allowed(&captured,&input.1)||out.owner().is_none()||!pane_matches(&captured,&saved)||anchor["block"]!=saved["scene"]["document"]["content"][index]{out.fail("The scene draft changed before Apply. Run a fresh review.");return;}
                // These read-only captures do not authorize mutation: every
                // source must pass the root's one-transaction check together.
                let sources=match apply_sources(&s){Ok(v)=>v,Err(e)=>{out.fail(&e);return;}};
                #[cfg(test)] {let action=out.before_sources_check.borrow_mut().take();if let Some(action)=action{action();}}
                let p=out.clone();let writing=captured.clone();let client=a_.client.clone();
                out.status.set_text("Checking all saved sources atomically before Apply…");
                client.rpc("review_sources_check",json!({"projectId":out.project_id,"sources":sources}),move|reply|{
                    let command=match checked_swap(&sources,reply,&anchor,&replacement){Ok(v)=>v,Err(e)=>{p.fail(&e);return;}};
                    if !sequence_host::tool_input_allowed(&writing,&input.1) || p.owner().is_none() || !pane_matches(&writing,&saved) {
                        p.fail("The scene became busy or changed before Apply; no edit applied.");return;
                    }
                    // The complete editor document is checked again by swap.
                    // Final acceptance still needs the checked dependency CAS.
                    writing.saving.set(true);let out=p.clone();let pane=writing.clone();
                    writing.request(command,true,move|r|{
                        pane.saving.set(false);if let Err(e)=r{out.fail(&e);return;}
                        let decision=decided(&out.value.borrow(),s["id"].as_str().unwrap(),"accepted");
                        match decision{
                            Ok(v)=>{
                                *out.value.borrow_mut()=v;*out.dependencies.borrow_mut()=dependencies;
                                out.dirty.set(true);out.applied_pending.set(true);out.saving.set(false);
                                #[cfg(test)] {let action=out.before_decision_save.borrow_mut().take();if let Some(action)=action{action();}}
                                out.save(&a_,|_|{});
                            },
                            Err(e)=>out.fail(&format!("Scene draft changed, but the review decision was not accepted. Undo remains available. {e}"))
                        }
                    });
                });
            });
        };
        apply_to_pane(Ok(existing));
    }
}
#[cfg(test)]
#[path = "review_decisions_tests.rs"]
pub(super) mod tests;
