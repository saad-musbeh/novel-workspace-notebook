//! Explicit whole-version project-data recovery. No revision is merged or deleted.
use super::*;
use novel_storage::ProjectDataSnapshot;

const KINDS: &[(&str, &str)] = &[
    ("planning", "Story bible and plot board"),
    ("notebookTools", "Notebook scraps, reactions and reminders"),
    ("sceneMetadata", "Scene details"),
    ("paragraphAlternatives", "Paragraph alternatives"),
    ("sceneTranslations", "Translation drafts"),
    ("writingSystems", "Writing systems"),
    ("languagePacks", "Language packs"),
    ("settings", "Review settings"),
    ("assistanceJobs", "Review jobs"),
    ("universeCatalogue", "Universe catalogue"),
    ("chapterNumbering", "Chapter numbering"),
    ("exportProfile", "Word styles"),
    ("branchLineage", "Branch lineage (inspection only)"),
];
const COMBINED_KINDS: &[&str] = &[
    "writingSystems",
    "sceneTranslations",
    "paragraphAlternatives",
];
fn rows<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("Unsupported {key} collection in retained version."))
}
fn id(value: &Value) -> Result<&str, String> {
    value["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("An immutable item is missing its identity.".into())
}
fn union_immutable(
    target: &mut Vec<Value>,
    item: &Value,
    key: impl Fn(&Value) -> Result<String, String>,
) -> Result<(), String> {
    let identity = key(item)?;
    if let Some(existing) = target
        .iter()
        .find(|v| key(v).ok().as_deref() == Some(identity.as_str()))
    {
        if existing != item {
            return Err("Retained immutable items share an identity but differ in content. Nothing was combined.".into());
        }
    } else {
        target.push(item.clone());
    }
    Ok(())
}
fn group_header(value: &Value) -> Value {
    let mut v = value.clone();
    if let Some(object) = v.as_object_mut() {
        object.remove("versions");
    }
    v
}
/// Runs only after the author explicitly chooses a base and requests preparation.
/// Every copied source has already passed immutable revision/source validation.
fn combined_candidate(kind: &str, base: &Value, heads: &[Value]) -> Result<Value, String> {
    if !COMBINED_KINDS.contains(&kind) {
        return Err("Combined candidates support writing systems, translations and paragraph alternatives. Other categories use explicit whole-version choices.".into());
    }
    if heads.len() > 64 {
        return Err("A combined resolution exceeds the 64-parent revision limit. All heads remain untouched.".into());
    }
    if base["schemaVersion"] != 1 || heads.iter().any(|v| v["schemaVersion"] != 1) {
        return Err("Unsupported retained project-data schema.".into());
    }
    let mut candidate = base.clone();
    match kind {
        "writingSystems" => {
            if rows(base, "assignments")?.len() > 10000 {
                return Err("Writing-system assignments exceed the saved limit.".into());
            }
            let systems = candidate["systems"]
                .as_array_mut()
                .ok_or("Unsupported writing-system collection.")?;
            let key = |v: &Value| -> Result<String, String> {
                if v["schemaVersion"] != 1 {
                    return Err("Unsupported writing-system schema.".into());
                }
                let version = v["mappingVersion"]
                    .as_u64()
                    .filter(|v| *v > 0)
                    .ok_or("Invalid mapping version.")?;
                Ok(format!("{}:{version}", id(v)?))
            };
            for head in heads {
                for system in rows(head, "systems")? {
                    union_immutable(systems, system, &key)?;
                }
            }
            let mut seen = std::collections::HashSet::new();
            for system in systems.iter() {
                if !seen.insert(key(system)?) {
                    return Err("Duplicate immutable writing-system mapping identity.".into());
                }
            }
            if systems.len() > 4096 {
                return Err("Combined writing-system versions exceed the 4096-version limit. All retained heads remain untouched.".into());
            }
        }
        "sceneTranslations" => {
            let chosen = rows(base, "drafts")?
                .iter()
                .map(|d| id(d).map(str::to_owned))
                .collect::<Result<std::collections::HashSet<_>, _>>()?;
            let drafts = candidate["drafts"]
                .as_array_mut()
                .ok_or("Unsupported translation drafts.")?;
            let immutable = [
                "sceneId",
                "sceneRevisionId",
                "sourceLanguage",
                "targetLanguage",
                "sourceText",
            ];
            for head in heads {
                for draft in rows(head, "drafts")? {
                    let identity = id(draft)?;
                    for field in immutable {
                        if !draft[field].is_string() {
                            return Err("Malformed immutable translation source.".into());
                        }
                    }
                    if !draft["text"].is_string() {
                        return Err("Malformed translation draft text.".into());
                    }
                    if let Some(existing) =
                        drafts.iter().find(|d| d["id"].as_str() == Some(identity))
                    {
                        if immutable
                            .iter()
                            .any(|field| existing[*field] != draft[*field])
                        {
                            return Err("Translation drafts share an identity but differ in immutable source. Nothing was combined.".into());
                        }
                        if !chosen.contains(identity) && existing != draft {
                            return Err("Select a retained base containing this shared translation draft before choosing its edited text.".into());
                        }
                    } else {
                        drafts.push(draft.clone());
                    }
                }
            }
            let mut seen = std::collections::HashSet::new();
            for draft in drafts.iter() {
                if !seen.insert(id(draft)?) {
                    return Err("Duplicate translation draft identity.".into());
                }
                for field in [
                    "sceneId",
                    "sceneRevisionId",
                    "sourceLanguage",
                    "targetLanguage",
                    "sourceText",
                ] {
                    if !draft[field].as_str().is_some_and(|s| !s.is_empty()) {
                        return Err("Malformed immutable translation source.".into());
                    }
                }
                if !draft["sourceText"]
                    .as_str()
                    .is_some_and(|s| s.encode_utf16().count() <= 500000)
                    || !draft["text"]
                        .as_str()
                        .is_some_and(|s| s.encode_utf16().count() <= 500000)
                {
                    return Err("Translation source or draft exceeds its saved text limit.".into());
                }
            }
            if drafts.len() > 1000 {
                return Err("Combined translation sources exceed the 1000-draft limit. All retained heads remain untouched.".into());
            }
        }
        "paragraphAlternatives" => {
            let chosen = rows(base, "scenes")?
                .iter()
                .flat_map(|s| s["groups"].as_array().into_iter().flatten())
                .map(|g| id(g).map(str::to_owned))
                .collect::<Result<std::collections::HashSet<_>, _>>()?;
            for head in heads {
                for scene in rows(head, "scenes")? {
                    let scene_id = scene["sceneId"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .ok_or("Malformed retained alternative scene.")?;
                    for group in rows(scene, "groups")? {
                        let group_id = id(group)?;
                        let scenes = candidate["scenes"]
                            .as_array_mut()
                            .ok_or("Unsupported alternative scenes.")?;
                        let found = scenes.iter().enumerate().find_map(|(si, s)| {
                            s["groups"]
                                .as_array()?
                                .iter()
                                .position(|g| g["id"].as_str() == Some(group_id))
                                .map(|gi| (si, gi))
                        });
                        if let Some((si, gi)) = found {
                            if scenes[si]["sceneId"].as_str() != Some(scene_id)
                                || scenes[si]["groups"][gi]["originalId"] != group["originalId"]
                            {
                                return Err("Paragraph groups share an identity but differ in scene or immutable original. Nothing was combined.".into());
                            }
                            if !chosen.contains(group_id)
                                && group_header(&scenes[si]["groups"][gi]) != group_header(group)
                            {
                                return Err("Select a retained base containing this shared paragraph group before choosing its anchor and active version.".into());
                            }
                            let versions = scenes[si]["groups"][gi]["versions"]
                                .as_array_mut()
                                .ok_or("Unsupported retained paragraph versions.")?;
                            for version in rows(group, "versions")? {
                                union_immutable(versions, version, |v| id(v).map(str::to_owned))?;
                            }
                        } else {
                            let si = if let Some(i) = scenes
                                .iter()
                                .position(|s| s["sceneId"].as_str() == Some(scene_id))
                            {
                                i
                            } else {
                                scenes.push(json!({"sceneId":scene_id,"groups":[]}));
                                scenes.len() - 1
                            };
                            scenes[si]["groups"]
                                .as_array_mut()
                                .ok_or("Unsupported paragraph groups.")?
                                .push(group.clone());
                        }
                    }
                }
            }
            let scenes = rows(&candidate, "scenes")?;
            if scenes.len() > 10000 {
                return Err("Combined alternative scenes exceed the saved limit.".into());
            }
            let mut group_ids = std::collections::HashSet::new();
            let mut version_ids = std::collections::HashSet::new();
            let mut scene_ids = std::collections::HashSet::new();
            for scene in scenes {
                if !scene_ids.insert(
                    scene["sceneId"]
                        .as_str()
                        .ok_or("Malformed scene identity.")?,
                ) {
                    return Err("Duplicate alternative scene identity.".into());
                }
                let groups = rows(scene, "groups")?;
                if groups.len() > 100 {
                    return Err("Combined paragraph groups exceed the 100-per-scene limit.".into());
                }
                for group in groups {
                    if !group_ids.insert(id(group)?) {
                        return Err("Duplicate paragraph group identity.".into());
                    }
                    let versions = rows(group, "versions")?;
                    if versions.is_empty() || versions.len() > 20 {
                        return Err("Combined paragraph versions exceed the 20-per-group limit. All retained heads remain untouched.".into());
                    }
                    for version in versions {
                        if !version_ids.insert(id(version)?) {
                            return Err(
                                "Duplicate paragraph version identity across groups.".into()
                            );
                        }
                        let block = &version["block"];
                        if !matches!(block["type"].as_str(), Some("paragraph" | "heading")) {
                            return Err("Unsupported retained paragraph block.".into());
                        }
                        novel_storage::validate(&novel_storage::Scene {
                            schema_version: 1,
                            title: String::new(),
                            document: json!({"type":"doc","content":[block]}),
                        })?;
                    }
                    for key in ["originalId", "activeId"] {
                        if !versions.iter().any(|v| v["id"] == group[key]) {
                            return Err("Original or active paragraph version is missing.".into());
                        }
                    }
                }
            }
        }
        _ => unreachable!(),
    }
    if serde_json::to_vec(&candidate)
        .map_err(|_| "Unsupported combined candidate.")?
        .len()
        > 8_000_000
    {
        return Err(
            "The combined candidate exceeds the saved 8 MB limit. No source was changed.".into(),
        );
    }
    Ok(candidate)
}
#[derive(Clone)]
struct Capture {
    kind: String,
    heads: Vec<String>,
    history: Vec<Value>,
}
struct ConflictPage {
    // Button closures own this controller. Its container and those buttons are
    // weak references so closing the tab releases the controller and its previews.
    widget: glib::WeakRef<gtk::Box>,
    kinds: gtk::ComboBoxText,
    versions: gtk::ComboBoxText,
    status: gtk::Label,
    preview: gtk::TextBuffer,
    heads: gtk::Box,
    resolve: glib::WeakRef<gtk::Button>,
    prepare: glib::WeakRef<gtk::Button>,
    save_candidate: glib::WeakRef<gtk::Button>,
    candidate_section: glib::WeakRef<gtk::Expander>,
    candidate_preview: gtk::TextBuffer,
    reviewed: gtk::CheckButton,
    candidate: RefCell<Option<Value>>,
    head_values: RefCell<HashMap<String, ProjectDataSnapshot>>,
    pid: String,
    capture: RefCell<Option<Capture>>,
    selected: RefCell<Option<ProjectDataSnapshot>>,
    generation: Cell<u64>,
    preview_generation: Cell<u64>,
    updating: Cell<bool>,
    pending: Cell<bool>,
}
fn label(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.);
    l.set_line_wrap(true);
    l.set_selectable(true);
    l
}
impl ConflictPage {
    fn owner(&self) -> Option<Rc<Workspace>> {
        let widget = self.widget.upgrade()?;
        let a = desk::workspace_for_widget(widget.upcast_ref())?;
        if a.pid.borrow().as_str() == self.pid && a.pages.page_num(&widget).is_some() {
            Some(a)
        } else {
            None
        }
    }
    fn can_apply(&self) -> bool {
        !self.pending.get()
            && self.selected.borrow().is_some()
            && self.capture.borrow().as_ref().is_some_and(|c| {
                c.heads.len() > 1
                    && c.kind != "branchLineage"
                    && self.head_values.borrow().len() == c.heads.len()
            })
    }
    fn update_apply(&self) {
        if let Some(b) = self.resolve.upgrade() {
            b.set_sensitive(self.can_apply());
        }
        let prepare = self.can_apply()
            && self
                .capture
                .borrow()
                .as_ref()
                .is_some_and(|c| COMBINED_KINDS.contains(&c.kind.as_str()));
        if let Some(b) = self.prepare.upgrade() {
            b.set_sensitive(prepare);
        }
        if let Some(b) = self.save_candidate.upgrade() {
            b.set_sensitive(
                prepare && self.candidate.borrow().is_some() && self.reviewed.is_active(),
            );
        }
        self.reviewed
            .set_sensitive(!self.pending.get() && self.candidate.borrow().is_some());
    }
    fn clear_candidate(&self) {
        *self.candidate.borrow_mut() = None;
        self.candidate_preview.set_text("");
        self.reviewed.set_active(false);
    }
    fn prepare_candidate(&self) {
        if !self.can_apply() {
            return;
        }
        let Some(c) = self.capture.borrow().clone() else {
            return;
        };
        let Some(base) = self.selected.borrow().clone() else {
            return;
        };
        let heads = c
            .heads
            .iter()
            .filter_map(|id| self.head_values.borrow().get(id).map(|v| v.payload.clone()))
            .collect::<Vec<_>>();
        if heads.len() != c.heads.len() {
            self.status
                .set_text("Wait for every current retained version to be checked.");
            return;
        }
        match combined_candidate(&c.kind, &base.payload, &heads) {
            Ok(candidate) => {
                self.clear_candidate();
                self.candidate_preview
                    .set_text(&serde_json::to_string_pretty(&candidate).unwrap_or_default());
                *self.candidate.borrow_mut() = Some(candidate);
                if let Some(section) = self.candidate_section.upgrade() {
                    section.set_expanded(true);
                }
                self.status.set_text("Combined candidate prepared. Mutable values come from your selected base; immutable additions from all checked heads are retained. Review before explicitly saving.");
            }
            Err(e) => {
                self.clear_candidate();
                self.status.set_text(&format!(
                    "Candidate not prepared. All retained versions remain unchanged. {e}"
                ));
            }
        }
        self.update_apply();
    }
    fn apply_candidate(self: &Rc<Self>) {
        if !self.reviewed.is_active() {
            self.status
                .set_text("Review the complete combined candidate and confirm it before saving.");
            return;
        }
        let Some(candidate) = self.candidate.borrow().clone() else {
            return;
        };
        self.apply_payload(candidate, true);
    }

    fn load(self: &Rc<Self>) {
        if self.pending.get() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(kind) = self.kinds.active_id().map(|s| s.to_string()) else {
            return;
        };
        let epoch = self.generation.get() + 1;
        self.generation.set(epoch);
        self.preview_generation
            .set(self.preview_generation.get() + 1);
        self.updating.set(true);
        self.versions.remove_all();
        self.updating.set(false);
        *self.capture.borrow_mut() = None;
        *self.selected.borrow_mut() = None;
        self.head_values.borrow_mut().clear();
        self.clear_candidate();
        self.preview.set_text("");
        for child in self.heads.children() {
            self.heads.remove(&child);
        }
        self.update_apply();
        self.status.set_text("Loading retained versions…");
        let p = self.clone();
        a.client.rpc("project_data_heads",json!({"projectId":self.pid,"kind":kind}),move|reply|{
            if p.owner().is_none() || p.generation.get()!=epoch{return;}
            let heads=match reply.and_then(|v|serde_json::from_value::<novel_storage::RevisionHeads>(v).map_err(|_|"Unsupported retained heads.".into())){
                Ok(h)=>h.revision_ids,Err(e)=>{p.status.set_text(&e);return;}
            };
            let Some(a)=p.owner()else{return;};let next=p.clone();
            a.client.rpc("project_data_history",json!({"projectId":p.pid,"kind":kind}),move|reply|{
                if next.owner().is_none() || next.generation.get()!=epoch{return;}
                let history=match reply{Ok(v)=>match v.as_array(){Some(rows)=>rows.clone(),None=>{next.status.set_text("Unsupported saved history.");return;}},Err(e)=>{next.status.set_text(&e);return;}};
                let count=heads.len();
                *next.capture.borrow_mut()=Some(Capture{kind:kind.clone(),heads:heads.clone(),history:history.clone()});
                next.updating.set(true);
                for revision in &history{
                    let Some(id)=revision["id"].as_str()else{continue;};
                    let current=heads.iter().position(|h|h==id);
                    let name=format!("{} · {}",current.map(|i|format!("Competing version {}",i+1)).unwrap_or_else(||"Earlier retained version".into()),revision["createdAt"].as_str().unwrap_or("Saved revision"));
                    next.versions.append(Some(id),&name);
                }
                next.updating.set(false);
                if count>1{
                    next.status.set_text(&format!("{count} competing versions. Preview each, then explicitly choose the complete version to keep. All earlier revisions remain in history."));
                }else{next.status.set_text(if count==0{"No saved data in this category."}else{"No competing versions. Saved history is available for inspection."});}
                if kind=="branchLineage"{next.status.set_text("Branch lineage is immutable. Its retained history is available for inspection; generic conflict resolution is disabled.");}
                // Each current head receives an independently checksum-checked preview.
                for (i,id) in heads.iter().enumerate(){next.head_preview(epoch,&kind,id,i+1);}
                let first=heads.first().cloned().or_else(||history.first().and_then(|r|r["id"].as_str()).map(str::to_owned));
                if let Some(id)=first{next.versions.set_active_id(Some(&id));}
                next.update_apply();
            });
        });
    }
    fn head_preview(self: &Rc<Self>, epoch: u64, kind: &str, id: &str, number: usize) {
        let Some(a) = self.owner() else {
            return;
        };
        let expander = gtk::Expander::new(Some(&format!("Competing version {number}")));
        let view = gtk::TextView::new();
        view.set_editable(false);
        view.set_cursor_visible(false);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        let buffer = view.buffer().unwrap();
        buffer.set_text("Loading checked preview…");
        let scroller = scroll(&view);
        scroller.set_min_content_height(140);
        expander.add(&scroller);
        self.heads.pack_start(&expander, false, false, 0);
        self.heads.show_all();
        let p = self.clone();
        let id = id.to_owned();
        a.client.rpc(
            "load_project_data_revision",
            json!({"projectId":self.pid,"kind":kind,"revisionId":id}),
            move |reply| {
                if p.owner().is_none() || p.generation.get() != epoch {
                    return;
                }
                match reply.and_then(snapshot) {
                    Ok(v) => {
                        buffer.set_text(&describe(&v.payload));
                        p.head_values.borrow_mut().insert(id, v);
                        p.update_apply();
                    }
                    Err(e) => buffer.set_text(&format!("Preview unavailable. {e}")),
                }
            },
        );
    }
    fn select_version(self: &Rc<Self>) {
        if self.updating.get() || self.pending.get() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(capture) = self.capture.borrow().clone() else {
            return;
        };
        let Some(id) = self.versions.active_id() else {
            return;
        };
        if !capture
            .history
            .iter()
            .any(|r| r["id"].as_str() == Some(id.as_str()))
        {
            return;
        }
        *self.selected.borrow_mut() = None;
        self.clear_candidate();
        self.update_apply();
        self.preview.set_text("Loading checked preview…");
        let generation = self.generation.get();
        let epoch = self.preview_generation.get() + 1;
        self.preview_generation.set(epoch);
        let p = self.clone();
        a.client.rpc(
            "load_project_data_revision",
            json!({"projectId":self.pid,"kind":capture.kind,"revisionId":id.as_str()}),
            move |reply| {
                if p.owner().is_none()
                    || p.generation.get() != generation
                    || p.preview_generation.get() != epoch
                {
                    return;
                }
                match reply.and_then(snapshot) {
                    Ok(v) => {
                        p.preview.set_text(&describe(&v.payload));
                        *p.selected.borrow_mut() = Some(v);
                    }
                    Err(e) => p.preview.set_text(&format!("Preview unavailable. {e}")),
                }
                p.update_apply();
            },
        );
    }
    fn apply(self: &Rc<Self>) {
        let Some(selected) = self.selected.borrow().clone() else {
            return;
        };
        self.apply_payload(selected.payload, false);
    }
    fn apply_payload(self: &Rc<Self>, payload: Value, combined: bool) {
        if !self.can_apply() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(capture) = self.capture.borrow().clone() else {
            return;
        };
        self.pending.set(true);
        self.kinds.set_sensitive(false);
        self.versions.set_sensitive(false);
        self.update_apply();
        self.status
            .set_text("Checking unsaved pages in this novel’s windows…");
        let p = self.clone();
        a.confirm_project(move|accepted|{
            let Some(owner)=p.owner()else{p.finish("The novel changed. Nothing was resolved.");return;};
            if !accepted{p.finish("Resolution cancelled. The retained versions and your drafts stay available.");return;}
            // Use only the head set captured when these previews were loaded.
            // The storage transaction compares every current head atomically.
            p.status.set_text("Saving your explicit version choice…");let page=p.clone();
            owner.client.rpc("resolve_project_data",json!({"projectId":p.pid,"kind":capture.kind,"payload":payload,"heads":capture.heads}),move|reply|{
                match reply{
                    Ok(_)=>{
                        page.clear_candidate();page.finish(if combined{"Combined resolution saved. Every previous version remains in history."}else{"Version choice saved. Every previous version remains in history."});
                        if let Some(owner)=page.owner(){owner.load_index();}
                        // The consumed capture must never permit a second resolution.
                        *page.capture.borrow_mut()=None;*page.selected.borrow_mut()=None;page.update_apply();
                    },
                    Err(e)=>{if e.starts_with("Conflict"){*page.capture.borrow_mut()=None;}page.finish(&format!("Nothing resolved; all retained versions remain. {e} Refresh versions before choosing again if the heads changed."));},
                }
            });
        });
    }
    fn finish(&self, message: &str) {
        self.pending.set(false);
        self.kinds.set_sensitive(true);
        self.versions.set_sensitive(true);
        self.status.set_text(message);
        self.update_apply();
    }
}
fn snapshot(value: Value) -> Result<ProjectDataSnapshot, String> {
    serde_json::from_value(value).map_err(|_| "Unsupported saved version.".into())
}
impl Workspace {
    pub(super) fn data_conflicts(self: &Rc<Self>) {
        let a = desk::effective(self);
        if a.pid.borrow().is_empty() {
            a.error("Choose a novel before inspecting its project versions.");
            return;
        }
        let origin = a.clone();
        a.navigate(move || origin.open_data_conflicts());
    }
    fn open_data_conflicts(self: &Rc<Self>) {
        let panel = self.panel("Project data conflicts");
        let host = panel.content_area();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        host.pack_start(&scroll(&body), true, true, 0);
        body.pack_start(&label("Inspect complete retained versions of your novel’s notes and settings. Choosing a version creates a new revision; it does not erase any history or merge anything automatically."),false,false,0);
        body.pack_start(&label("Immutable originals, translation sources and writing-system versions must remain intact. A choice that drops or changes them is refused, with all versions preserved."),false,false,0);
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let kinds = gtk::ComboBoxText::new();
        for (kind, title) in KINDS {
            kinds.append(Some(kind), title);
        }
        kinds.set_active(Some(0));
        controls.pack_start(&kinds, true, true, 0);
        body.pack_start(&controls, false, false, 0);
        let status = label("Loading retained versions…");
        body.pack_start(&status, false, false, 0);
        let heads = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.pack_start(&scroll(&heads), true, true, 0);
        body.pack_start(&label("Preview a saved version"), false, false, 0);
        let versions = gtk::ComboBoxText::new();
        body.pack_start(&versions, false, false, 0);
        let view = gtk::TextView::new();
        view.set_editable(false);
        view.set_cursor_visible(false);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_left_margin(12);
        view.set_right_margin(12);
        let preview = view.buffer().unwrap();
        let scroller = scroll(&view);
        scroller.set_min_content_height(170);
        body.pack_start(&scroller, true, true, 0);
        let candidate_section =
            gtk::Expander::new(Some("Review an explicitly combined immutable resolution"));
        let candidate_body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        candidate_body.pack_start(&label("Supported: writing-system versions, translation drafts and paragraph alternatives. Choose a base version, then prepare a candidate retaining every immutable addition from all heads. Mutable values follow your base. No merge happens until you request preparation and explicitly save the reviewed candidate. Other categories retain whole-version choices."),false,false,0);
        let candidate_view = gtk::TextView::new();
        candidate_view.set_widget_name("data-conflicts-combined-preview");
        candidate_view.set_editable(false);
        candidate_view.set_cursor_visible(false);
        candidate_view.set_wrap_mode(gtk::WrapMode::WordChar);
        let candidate_preview = candidate_view.buffer().unwrap();
        let candidate_scroll = scroll(&candidate_view);
        candidate_scroll.set_min_content_height(180);
        candidate_body.pack_start(&candidate_scroll, true, true, 0);
        let reviewed = gtk::CheckButton::with_label(
            "I reviewed this complete candidate and its selected base",
        );
        reviewed.set_sensitive(false);
        candidate_body.pack_start(&reviewed, false, false, 0);
        let candidate_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        candidate_body.pack_start(&candidate_actions, false, false, 0);
        let save_candidate = gtk::Button::with_label("Save reviewed combined resolution");
        save_candidate.set_sensitive(false);
        candidate_actions.pack_start(&save_candidate, false, false, 0);
        candidate_section.add(&candidate_body);
        body.pack_start(&candidate_section, false, false, 0);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        body.pack_start(&actions, false, false, 0);
        let resolve = gtk::Button::with_label("Use this retained version");
        actions.pack_start(&resolve, false, false, 0);
        resolve.set_sensitive(false);
        let prepare = gtk::Button::with_label("Prepare combined resolution");
        prepare.set_sensitive(false);
        actions.pack_start(&prepare, false, false, 0);
        let refresh = gtk::Button::with_label("Refresh versions");
        controls.pack_start(&refresh, false, false, 0);
        let page = Rc::new(ConflictPage {
            widget: panel.widget.downgrade(),
            kinds: kinds.clone(),
            versions: versions.clone(),
            status,
            preview,
            heads,
            resolve: resolve.downgrade(),
            prepare: prepare.downgrade(),
            save_candidate: save_candidate.downgrade(),
            candidate_section: candidate_section.downgrade(),
            candidate_preview,
            reviewed: reviewed.clone(),
            candidate: RefCell::new(None),
            head_values: RefCell::new(HashMap::new()),
            pid: self.pid.borrow().clone(),
            capture: RefCell::new(None),
            selected: RefCell::new(None),
            generation: Cell::new(0),
            preview_generation: Cell::new(0),
            updating: Cell::new(false),
            pending: Cell::new(false),
        });
        let weak = Rc::downgrade(&page);
        kinds.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.load();
            }
        });
        let weak = Rc::downgrade(&page);
        versions.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.select_version();
            }
        });
        let p = page.clone();
        resolve.connect_clicked(move |_| p.apply());
        let p = page.clone();
        refresh.connect_clicked(move |_| p.load());
        let p = page.clone();
        prepare.connect_clicked(move |_| p.prepare_candidate());
        let p = page.clone();
        save_candidate.connect_clicked(move |_| p.apply_candidate());
        let weak = Rc::downgrade(&page);
        reviewed.connect_toggled(move |_| {
            if let Some(p) = weak.upgrade() {
                p.update_apply();
            }
        });
        self.install(&panel.widget, "Project data conflicts", true);
        page.load();
    }
}

#[cfg(test)]
#[path = "data_conflicts_candidate_tests.rs"]
mod candidate_tests;
