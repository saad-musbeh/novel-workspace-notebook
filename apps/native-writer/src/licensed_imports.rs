//! Ordinary notebook import page with concrete existing companion RPCs.
//! Included as app::licensed_imports; shared notebook/editor hooks belong to root.
use super::*;
use crate::{
    alphabets,
    local_resources::{self as resources, PackVersions},
};
use std::collections::{HashSet, VecDeque};
type Result<T> = std::result::Result<T, String>;
const HISTORY_LIMIT: usize = 256;
const HISTORY_BYTES: usize = 32_000_000;

fn snapshot(saved: &Value, kind: &str) -> Result<(Value, Value)> {
    if saved.is_null() {
        return Ok((
            if kind == "languagePacks" {
                json!({"schemaVersion":1,"packs":[]})
            } else {
                alphabets::empty_document()
            },
            Value::Null,
        ));
    }
    if !saved["revisionId"]
        .as_str()
        .is_some_and(|s| !s.is_empty() && s.len() <= 200)
    {
        return Err("Missing checked resource revision.".into());
    }
    let doc = saved["payload"].clone();
    if kind == "languagePacks" {
        resources::validate_pack_document(&doc)?;
    } else {
        alphabets::validate_document(&doc)?;
    }
    Ok((doc, saved["revisionId"].clone()))
}
#[derive(Clone, Debug)]
pub(super) enum PendingImport {
    Pack {
        pack: Value,
        candidate: Value,
    },
    Font {
        asset: novel_storage::ProjectAsset,
        font: Value,
        candidate: Value,
    },
}
#[derive(Clone, Debug)]
pub(super) struct ImportDraft {
    project_id: String,
    packs: Value,
    packs_parent: Value,
    alphabets: Value,
    alphabets_parent: Value,
    versions: PackVersions,
    pub pending: Option<PendingImport>,
    pub saving: bool,
}
impl ImportDraft {
    /// Snapshots must come from load_project_data_unambiguous, and history from
    /// per-revision checksum/ownership-checked reads, never the selected import.
    pub fn checked(
        project_id: String,
        packs_snapshot: Value,
        alphabet_snapshot: Value,
        history: &[Value],
    ) -> Result<Self> {
        if project_id.trim().is_empty() || project_id.len() > 200 || history.len() > HISTORY_LIMIT {
            return Err("Invalid project identity or oversized checked pack history.".into());
        }
        let (packs, packs_parent) = snapshot(&packs_snapshot, "languagePacks")?;
        let (alphabets, alphabets_parent) = snapshot(&alphabet_snapshot, "writingSystems")?;
        let mut history_bytes = 0usize;
        for saved in history {
            history_bytes = history_bytes
                .checked_add(serde_json::to_vec(saved).map_err(|e| e.to_string())?.len())
                .ok_or("Pack history size overflow.")?;
            if history_bytes > HISTORY_BYTES {
                return Err(
                    "Complete pack history exceeds 32 MB. No history was truncated.".into(),
                );
            }
        }
        let mut versions = PackVersions::default();
        let mut ids = HashSet::new();
        for saved in history {
            let (doc, parent) = snapshot(saved, "languagePacks")?;
            let id = parent.as_str().ok_or("Missing retained pack revision.")?;
            if !ids.insert(id.to_owned()) {
                return Err("Duplicate retained pack revision.".into());
            }
            versions.observe(&doc)?;
        }
        if packs_parent.is_null() && !history.is_empty()
            || !packs_parent.is_null()
                && !history
                    .iter()
                    .any(|s| s["revisionId"] == packs_parent && s["payload"] == packs)
        {
            return Err("Checked pack history does not include the current snapshot.".into());
        }
        versions.observe(&packs)?;
        Ok(Self {
            project_id,
            packs,
            packs_parent,
            alphabets,
            alphabets_parent,
            versions,
            pending: None,
            saving: false,
        })
    }
    fn can_preview(&self) -> Result<()> {
        if self.saving || self.pending.is_some() {
            Err("Save or discard the existing import preview first.".into())
        } else {
            Ok(())
        }
    }
    pub fn preview_pack(&mut self, bytes: &[u8]) -> Result<()> {
        self.can_preview()?;
        let pack = resources::import_pack(bytes)?;
        let candidate = self.versions.install(&self.packs, &pack)?;
        self.pending = Some(PendingImport::Pack { pack, candidate });
        Ok(())
    }
    pub fn preview_font(
        &mut self,
        bytes: &[u8],
        license: &Value,
        definition_key: &str,
    ) -> Result<()> {
        self.can_preview()?;
        let mut d = self.alphabets["systems"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| {
                format!("{}@{}", d["id"].as_str().unwrap(), d["mappingVersion"]) == definition_key
            })
            .cloned()
            .ok_or("Choose a saved alphabet version before importing its font.")?;
        let (asset, font) = resources::prepare_font(bytes, license)?;
        let latest = self.alphabets["systems"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["id"] == d["id"])
            .filter_map(|v| v["mappingVersion"].as_u64())
            .max()
            .unwrap();
        if latest >= 9_007_199_254_740_991 {
            return Err("Alphabet version limit reached.".into());
        }
        d["mappingVersion"] = json!(latest + 1);
        d["font"] = font.clone();
        let mut candidate = self.alphabets.clone();
        candidate["systems"].as_array_mut().unwrap().push(d);
        alphabets::validate_document(&candidate)?;
        self.pending = Some(PendingImport::Font {
            asset,
            font,
            candidate,
        });
        Ok(())
    }
    /// Real companion requests, in order. Font metadata is committed only AFTER
    /// licensed asset storage. A failed metadata CAS may retain a licensed asset.
    pub fn begin_save(&mut self, pack_permission: bool) -> Result<Vec<(&'static str, Value)>> {
        if self.saving {
            return Err("The import is still saving.".into());
        }
        let mut requests = Vec::new();
        let (kind, payload, parent) = match self
            .pending
            .as_ref()
            .ok_or("Choose a local file and preview it before Save.")?
        {
            PendingImport::Pack { pack, candidate } => {
                if !pack_permission {
                    return Err("Review the data license and confirm permission to store and transfer this pack.".into());
                }
                resources::validate_pack(pack)?;
                resources::validate_pack_document(candidate)?;
                self.versions.install(&self.packs, pack)?;
                ("languagePacks", candidate, &self.packs_parent)
            }
            PendingImport::Font {
                asset,
                font,
                candidate,
            } => {
                resources::verify_font(font, asset)?;
                alphabets::validate_document(candidate)?;
                requests.push((
                    "put_asset",
                    json!({"projectId":self.project_id,"asset":asset}),
                ));
                ("writingSystems", candidate, &self.alphabets_parent)
            }
        };
        requests.push(("save_project_data_checked",json!({"projectId":self.project_id,"kind":kind,"payload":payload,"parent":parent,"dependencies":[]})));
        self.saving = true;
        Ok(requests)
    }
    pub fn complete_save(&mut self, reply: Result<Value>) -> Result<()> {
        if !self.saving {
            return Err("No import save is pending.".into());
        }
        self.saving = false;
        let saved = reply?;
        let pending = self.pending.as_ref().ok_or("Missing pending import.")?;
        let (kind, candidate, old_parent) = match pending {
            PendingImport::Pack { candidate, .. } => {
                ("languagePacks", candidate, &self.packs_parent)
            }
            PendingImport::Font { candidate, .. } => {
                ("writingSystems", candidate, &self.alphabets_parent)
            }
        };
        let (doc, parent) = snapshot(&saved, kind)?;
        if parent.is_null() || &parent == old_parent || &doc != candidate {
            return Err(
                "Import acknowledgement differs. Preview and pending values retained.".into(),
            );
        }
        if kind == "languagePacks" {
            self.versions.observe(&doc)?;
            self.packs = doc;
            self.packs_parent = parent;
        } else {
            self.alphabets = doc;
            self.alphabets_parent = parent;
        }
        self.pending = None;
        Ok(())
    }
    pub fn discard(&mut self) -> Result<()> {
        if self.saving {
            return Err("The import is still saving.".into());
        }
        self.pending = None;
        Ok(())
    }
    pub fn preview_text(&self) -> String {
        match &self.pending {
            Some(PendingImport::Pack{pack,..})=>{let mut lines=vec![format!("{} · {} / {} · version {}",pack["name"].as_str().unwrap(),pack["language"].as_str().unwrap(),pack["variant"].as_str().unwrap(),pack["version"].as_str().unwrap()),format!("Source: {}\nData license: {}\nAttribution: {}\nSHA-256: {}",pack["source"].as_str().unwrap(),pack["license"].as_str().unwrap(),pack["attribution"].as_str().unwrap(),pack["checksum"].as_str().unwrap()),format!("Operations: {} · {} exact terms",pack["operations"],pack["entries"].as_array().unwrap().len())];for e in pack["entries"].as_array().unwrap().iter().take(5) {for s in e["senses"].as_array().unwrap().iter().take(2){lines.push(format!("{} · {}: {}\nSynonyms: {}\nRelated: {}\nTranslations: {}",e["term"].as_str().unwrap(),s["partOfSpeech"].as_str().unwrap(),s["definition"].as_str().unwrap(),s["synonyms"],s["related"],s["translations"]));}}lines.push("Preview only. Save explicitly to keep the licensed pack. Earlier versions remain in project history.".into());lines.join("\n\n")},
            Some(PendingImport::Font{font,candidate,..})=>format!("{} · {}\nSHA-256: {}\nFont license: {}\nSource / permission record: {}\nAuthor confirmed permission to store and transfer.\n\nSave stores the asset and publishes alphabet version {}. Existing scene assignments stay on their exact versions.\n\nThis structural preview does not register a font or establish glyph/shaping coverage.",font["family"].as_str().unwrap(),font["format"].as_str().unwrap(),font["checksum"].as_str().unwrap(),font["license"]["name"].as_str().unwrap(),font["license"]["source"].as_str().unwrap(),candidate["systems"].as_array().unwrap().last().unwrap()["mappingVersion"]),
            None=>"Choose a licensed local JSON pack or a permitted TTF/OTF to preview. No file is stored until explicit Save.".into()
        }
    }
}

fn history_ids(value: &Value) -> Result<Vec<String>> {
    let rows = value
        .as_array()
        .filter(|v| v.len() <= HISTORY_LIMIT)
        .ok_or("Pack history exceeds the complete checked import limit.")?;
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    let mut history_bytes = 0usize;
    for row in rows {
        if let Some(payload) = row.get("payload") {
            history_bytes = history_bytes
                .checked_add(
                    serde_json::to_vec(payload)
                        .map_err(|e| e.to_string())?
                        .len(),
                )
                .ok_or("Pack history size overflow.")?;
            if history_bytes > HISTORY_BYTES {
                return Err(
                    "Complete pack history exceeds 32 MB. No history was truncated.".into(),
                );
            }
        }
        let id = row["id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 200)
            .ok_or("Unsupported retained pack history identity.")?;
        if !seen.insert(id) {
            return Err("Duplicate pack history identity.".into());
        }
        ids.push(id.to_owned());
    }
    ids.sort();
    Ok(ids)
}
fn check_revisions(
    client: Process,
    pid: String,
    mut ids: VecDeque<String>,
    mut snapshots: Vec<Value>,
    used_bytes: usize,
    done: Box<dyn FnOnce(Result<Vec<Value>>)>,
) {
    let Some(id) = ids.pop_front() else {
        done(Ok(snapshots));
        return;
    };
    let next = client.clone();
    client.rpc(
        "load_project_data_revision",
        json!({"projectId":pid,"kind":"languagePacks","revisionId":id}),
        move |reply| match reply.and_then(|v| {
            if v["revisionId"] != id {
                return Err("Checked pack revision identity differs.".into());
            }
            snapshot(&v, "languagePacks")?;
            Ok(v)
        }) {
            Ok(saved) => {
                let total = match serde_json::to_vec(&saved)
                    .ok()
                    .and_then(|b| used_bytes.checked_add(b.len()))
                    .filter(|n| *n <= HISTORY_BYTES)
                {
                    Some(n) => n,
                    None => {
                        done(Err("Complete checked pack history exceeds 32 MB.".into()));
                        return;
                    }
                };
                snapshots.push(saved);
                check_revisions(next, pid, ids, snapshots, total, done);
            }
            Err(e) => done(Err(e)),
        },
    );
}
fn checked_pack_history(
    client: Process,
    pid: String,
    current: Value,
    done: Box<dyn FnOnce(Result<Vec<Value>>)>,
) {
    let next = client.clone();
    let initial_pid = pid.clone();
    client.rpc("project_data_history",json!({"projectId":pid,"kind":"languagePacks"}),move|reply|{
        let ids=match reply.and_then(|v|history_ids(&v)){Ok(v)=>v,Err(e)=>{done(Err(e));return;}};
        let again=next.clone();let retained_ids=ids.clone();
        check_revisions(next,initial_pid.clone(),ids.into(),Vec::new(),0,Box::new(move|reply|{
            let history=match reply{Ok(h)=>h,Err(e)=>{done(Err(e));return;}};let final_client=again.clone();
            again.rpc("project_data_history",json!({"projectId":initial_pid,"kind":"languagePacks"}),move|reply|{
                match reply.and_then(|v|history_ids(&v)){Ok(ids) if ids==retained_ids=>{},Ok(_)=>{done(Err("Pack history changed while loading. Reopen the import page.".into()));return;},Err(e)=>{done(Err(e));return;}}
                final_client.rpc("load_project_data_unambiguous",json!({"projectId":initial_pid,"kind":"languagePacks"}),move|reply|match reply{Ok(v) if v==current=>done(Ok(history)),Ok(_)=>done(Err("Pack heads changed while loading. Reopen the import page.".into())),Err(e)=>done(Err(e))});
            });
        }));
    });
}
fn send_requests(
    client: Process,
    mut requests: VecDeque<(&'static str, Value)>,
    done: Box<dyn FnOnce(Result<Value>)>,
) {
    let Some((op, args)) = requests.pop_front() else {
        done(Err("Missing import commit request.".into()));
        return;
    };
    let next = client.clone();
    client.rpc(op, args, move |reply| match reply {
        Err(e) => done(Err(e)),
        Ok(v) if requests.is_empty() => done(Ok(v)),
        Ok(v) if op == "put_asset" && v.is_null() => send_requests(next, requests, done),
        Ok(_) => done(Err(
            "Unsupported asset-store acknowledgement. Pending import retained.".into(),
        )),
    });
}

pub(super) struct LicensedImportsPage {
    pub widget: gtk::Box,
    pub dirty: Cell<bool>,
    pub saving: Cell<bool>,
    pub composing: Cell<bool>,
    pub loaded: Cell<bool>,
    pub status: gtk::Label,
    pub license_name: gtk::Entry,
    pub license_source: gtk::Entry,
    pub font_permission: gtk::CheckButton,
    pub pack_permission: gtk::CheckButton,
    pub definition: gtk::ComboBoxText,
    pub preview: gtk::TextView,
    pub draft: RefCell<Option<ImportDraft>>,
    project_id: String,
    loading_started: Cell<bool>,
    controls: gtk::Box,
    choose_pack: gtk::Button,
    choose_font: gtk::Button,
    compositions: RefCell<HashSet<usize>>,
    updating: Cell<bool>,
    form_dirty: Cell<bool>,
    packs_snapshot: RefCell<Option<Value>>,
    alphabet_snapshot: RefCell<Option<Value>>,
    history: RefCell<Option<Vec<Value>>>,
}
impl LicensedImportsPage {
    pub fn new(project_id: String) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        widget.set_margin_start(32);
        widget.set_margin_end(32);
        widget.set_margin_top(26);
        widget.set_margin_bottom(20);
        caption(&widget, "Licensed local fonts & dictionary packs");
        caption(&widget,"Choose a local file you are permitted to store and transfer. Inspect its source, license and preview, then Save. Imported fonts are used privately by notebook writing.");
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
        widget.pack_start(&controls, true, true, 0);
        let choose_pack = gtk::Button::with_label("Choose local dictionary pack JSON");
        controls.pack_start(&choose_pack, false, false, 0);
        let pack_permission = gtk::CheckButton::with_label(
            "I reviewed the data license and may store and transfer this pack with the project.",
        );
        controls.pack_start(&pack_permission, false, false, 0);
        caption(&controls,"Font import publishes a new alphabet version. Assign a scene to that version explicitly in Alphabets.");
        let definition = gtk::ComboBoxText::new();
        controls.pack_start(&definition, false, false, 0);
        caption(&controls, "Font license / permission name");
        let license_name = gtk::Entry::new();
        controls.pack_start(&license_name, false, false, 0);
        caption(&controls, "Font license source or permission record");
        let license_source = gtk::Entry::new();
        controls.pack_start(&license_source, false, false, 0);
        let font_permission =
            gtk::CheckButton::with_label("I may store and transfer this font with this project.");
        controls.pack_start(&font_permission, false, false, 0);
        let choose_font = gtk::Button::with_label("Choose licensed local TTF / OTF");
        controls.pack_start(&choose_font, false, false, 0);
        let preview = gtk::TextView::new();
        preview.set_editable(false);
        preview.set_cursor_visible(false);
        preview.set_wrap_mode(gtk::WrapMode::WordChar);
        let scroller = scroll(&preview);
        scroller.set_min_content_height(240);
        controls.pack_start(&scroller, true, true, 0);
        let save = gtk::Button::with_label("Save previewed import");
        controls.pack_start(&save, false, false, 0);
        let discard = gtk::Button::with_label("Discard import draft");
        controls.pack_start(&discard, false, false, 0);
        let status = gtk::Label::new(Some("Loading checked project resource data…"));
        status.set_xalign(0.);
        status.set_line_wrap(true);
        status.set_selectable(true);
        widget.pack_start(&status, false, false, 0);
        let p = Rc::new(Self {
            widget,
            controls,
            choose_pack,
            choose_font,
            pack_permission,
            font_permission,
            definition,
            license_name,
            license_source,
            preview,
            status,
            project_id,
            loading_started: Cell::new(false),
            dirty: Cell::new(false),
            saving: Cell::new(false),
            composing: Cell::new(false),
            loaded: Cell::new(false),
            draft: RefCell::new(None),
            compositions: RefCell::new(HashSet::new()),
            updating: Cell::new(false),
            form_dirty: Cell::new(false),
            packs_snapshot: RefCell::new(None),
            alphabet_snapshot: RefCell::new(None),
            history: RefCell::new(None),
        });
        for (i, entry) in [&p.license_name, &p.license_source].into_iter().enumerate() {
            let weak = Rc::downgrade(&p);
            entry.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.form_changed();
                }
            });
            let weak = Rc::downgrade(&p);
            entry.connect_preedit_changed(move |_, text| {
                if let Some(p) = weak.upgrade() {
                    let mut states = p.compositions.borrow_mut();
                    if text.is_empty() {
                        states.remove(&i);
                    } else {
                        states.insert(i);
                    }
                    p.composing.set(!states.is_empty());
                }
            });
        }
        for check in [&p.font_permission, &p.pack_permission] {
            let weak = Rc::downgrade(&p);
            check.connect_toggled(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.form_changed();
                }
            });
        }
        let weak = Rc::downgrade(&p);
        p.choose_pack.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.choose(false);
            }
        });
        let weak = Rc::downgrade(&p);
        p.choose_font.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.choose(true);
            }
        });
        let weak = Rc::downgrade(&p);
        save.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.save(|_| {});
            }
        });
        let weak = Rc::downgrade(&p);
        discard.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.discard();
            }
        });
        p.controls.set_sensitive(false);
        p
    }
    fn owner(&self) -> Option<Rc<Workspace>> {
        desk::workspace_for_widget(self.widget.upcast_ref())
            .filter(|a| a.pid.borrow().as_str() == self.project_id)
    }
    pub fn load(self: &Rc<Self>) {
        if self.loaded.get() || self.draft.borrow().is_some() {
            return;
        }
        let Some(owner) = self.owner() else {
            self.status
                .set_text("Mount this ordinary page in its novel before loading.");
            return;
        };
        if self.loading_started.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        owner.client.rpc(
            "load_project_data_unambiguous",
            json!({"projectId":self.project_id,"kind":"writingSystems"}),
            move |reply| {
                if let Some(p) = weak.upgrade() {
                    if p.owner().is_none() {
                        return;
                    }
                    match reply.and_then(|v| {
                        snapshot(&v, "writingSystems")?;
                        Ok(v)
                    }) {
                        Ok(v) => {
                            *p.alphabet_snapshot.borrow_mut() = Some(v);
                            p.finish_load();
                        }
                        Err(e) => p.status.set_text(&e),
                    }
                }
            },
        );
        let weak = Rc::downgrade(self);
        let client = owner.client.clone();
        owner.client.rpc(
            "load_project_data_unambiguous",
            json!({"projectId":self.project_id,"kind":"languagePacks"}),
            move |reply| {
                if let Some(p) = weak.upgrade() {
                    if p.owner().is_none() {
                        return;
                    }
                    let current = match reply.and_then(|v| {
                        snapshot(&v, "languagePacks")?;
                        Ok(v)
                    }) {
                        Ok(v) => v,
                        Err(e) => {
                            p.status.set_text(&e);
                            return;
                        }
                    };
                    let weak = Rc::downgrade(&p);
                    let pid = p.project_id.clone();
                    checked_pack_history(
                        client,
                        pid,
                        current.clone(),
                        Box::new(move |reply| {
                            if let Some(p) = weak.upgrade() {
                                if p.owner().is_none() {
                                    return;
                                }
                                match reply {
                                    Ok(history) => {
                                        *p.packs_snapshot.borrow_mut() = Some(current);
                                        *p.history.borrow_mut() = Some(history);
                                        p.finish_load();
                                    }
                                    Err(e) => p.status.set_text(&e),
                                }
                            }
                        }),
                    );
                }
            },
        );
    }
    fn finish_load(&self) {
        if self.loaded.get() {
            return;
        }
        let (Some(packs), Some(alphabets), Some(history)) = (
            self.packs_snapshot.borrow().clone(),
            self.alphabet_snapshot.borrow().clone(),
            self.history.borrow().clone(),
        ) else {
            return;
        };
        match ImportDraft::checked(self.project_id.clone(), packs, alphabets, &history) {
            Ok(d) => {
                self.definition.remove_all();
                for v in d.alphabets["systems"].as_array().unwrap() {
                    self.definition.append(
                        Some(&format!(
                            "{}@{}",
                            v["id"].as_str().unwrap(),
                            v["mappingVersion"]
                        )),
                        &format!(
                            "{} · version {}",
                            v["name"].as_str().unwrap(),
                            v["mappingVersion"]
                        ),
                    );
                }
                self.definition.set_active(Some(0));
                *self.draft.borrow_mut() = Some(d);
                self.loaded.set(true);
                self.packs_snapshot.borrow_mut().take();
                self.alphabet_snapshot.borrow_mut().take();
                self.history.borrow_mut().take();
                self.sync();
                self.status
                    .set_text("Choose a licensed local file to preview. No import has been saved.");
            }
            Err(e) => self.status.set_text(&e),
        }
    }
    fn form_changed(&self) {
        if !self.updating.get() && self.loaded.get() {
            self.form_dirty.set(true);
            self.dirty.set(true);
            self.status.set_text(
                "Unsaved import preparation · preview a selected file, then Save or Discard",
            );
        }
    }
    fn license(&self) -> Value {
        json!({"name":self.license_name.text().as_str(),"source":self.license_source.text().as_str(),"permissionConfirmed":self.font_permission.is_active()})
    }
    fn ready(&self) -> Result<()> {
        if !self.loaded.get() || self.saving.get() {
            return Err("Resource imports are loading or saving.".into());
        }
        if self.composing.get() {
            return Err("Finish the current input composition.".into());
        }
        if self.owner().is_none() {
            return Err("The import page no longer belongs to this novel.".into());
        }
        Ok(())
    }
    fn choose(self: &Rc<Self>, font: bool) {
        if let Err(e) = self.ready() {
            self.status.set_text(&e);
            return;
        }
        let Some(owner) = self.owner() else {
            return;
        };
        if self.draft.borrow().as_ref().unwrap().pending.is_some() {
            self.status
                .set_text("Save or discard the existing preview first.");
            return;
        }
        if !font
            && (!self.license_name.text().is_empty()
                || !self.license_source.text().is_empty()
                || self.font_permission.is_active())
        {
            self.status.set_text("Choose the prepared font or discard its license fields before choosing a dictionary pack.");
            return;
        }
        let license = self.license();
        let selected = self.definition.active_id();
        if font {
            if let Err(e) = resources::validate_license(&license) {
                self.status.set_text(&e);
                return;
            }
            if selected.is_none() {
                self.status.set_text("Create and save an alphabet first.");
                return;
            }
        }
        let path = if font {
            files::choose(
                &owner.window,
                false,
                "Licensed TTF / OTF",
                &["*.ttf", "*.otf", "*.TTF", "*.OTF"],
            )
        } else {
            files::choose(
                &owner.window,
                false,
                "Licensed language-pack JSON",
                &["*.json"],
            )
        };
        let Some(path) = path else {
            return;
        };
        if let Err(e) = self.ready() {
            self.status.set_text(&e);
            return;
        }
        let result = resources::read_selected(
            &path,
            if font {
                resources::FONT_LIMIT
            } else {
                resources::PACK_LIMIT
            },
        )
        .and_then(|bytes| {
            let mut draft = self.draft.borrow_mut();
            let draft = draft.as_mut().unwrap();
            if font {
                draft.preview_font(&bytes, &license, selected.as_deref().unwrap())
            } else {
                draft.preview_pack(&bytes)
            }
        });
        match result {
            Ok(()) => {
                self.dirty.set(true);
                self.sync();
                self.status
                    .set_text("Preview only · inspect the license and Save explicitly to import");
            }
            Err(e) => self.status.set_text(&e),
        }
    }
    fn sync(&self) {
        let draft = self.draft.borrow();
        let selected = self.definition.active_id();
        self.definition.remove_all();
        if let Some(d) = draft.as_ref() {
            for v in d.alphabets["systems"].as_array().unwrap() {
                self.definition.append(
                    Some(&format!(
                        "{}@{}",
                        v["id"].as_str().unwrap(),
                        v["mappingVersion"]
                    )),
                    &format!(
                        "{} · version {}",
                        v["name"].as_str().unwrap(),
                        v["mappingVersion"]
                    ),
                );
            }
        }
        if let Some(selected) = selected {
            self.definition.set_active_id(Some(&selected));
        } else {
            self.definition.set_active(Some(0));
        }
        let pending = draft.as_ref().and_then(|d| d.pending.as_ref());
        let ready = self.loaded.get() && !self.saving.get();
        let choosing = ready && pending.is_none();
        self.controls.set_sensitive(ready);
        self.choose_pack.set_sensitive(choosing);
        self.choose_font.set_sensitive(choosing);
        for field in [&self.license_name, &self.license_source] {
            field.set_sensitive(choosing);
        }
        self.font_permission.set_sensitive(choosing);
        self.definition.set_sensitive(choosing);
        self.pack_permission
            .set_sensitive(ready && matches!(pending, Some(PendingImport::Pack { .. })));
        self.preview.buffer().unwrap().set_text(
            &draft
                .as_ref()
                .map_or_else(|| "Loading…".into(), ImportDraft::preview_text),
        );
        self.widget.show_all();
    }
    pub fn discard(&self) {
        if let Err(e) = self.ready() {
            self.status.set_text(&e);
            return;
        }
        let result = self.draft.borrow_mut().as_mut().unwrap().discard();
        match result {
            Ok(()) => {
                self.updating.set(true);
                self.license_name.set_text("");
                self.license_source.set_text("");
                self.font_permission.set_active(false);
                self.pack_permission.set_active(false);
                self.updating.set(false);
                self.form_dirty.set(false);
                self.dirty.set(false);
                self.sync();
                self.status.set_text("Import draft discarded. Any asset accepted by an earlier failed Save remains licensed project history.");
            }
            Err(e) => self.status.set_text(&e),
        }
    }
    pub fn save(self: &Rc<Self>, callback: impl FnOnce(Result<()>) + 'static) {
        if let Err(e) = self.ready() {
            self.status.set_text(&e);
            callback(Err(e));
            return;
        }
        if !self.dirty.get() {
            callback(Ok(()));
            return;
        }
        let requests = match self
            .draft
            .borrow_mut()
            .as_mut()
            .unwrap()
            .begin_save(self.pack_permission.is_active())
        {
            Ok(v) => v,
            Err(e) => {
                self.status.set_text(&e);
                callback(Err(e));
                return;
            }
        };
        let Some(owner) = self.owner() else {
            let e = "Import page owner disappeared.".to_owned();
            self.draft
                .borrow_mut()
                .as_mut()
                .unwrap()
                .complete_save(Err(e.clone()))
                .ok();
            callback(Err(e));
            return;
        };
        let is_font = matches!(
            self.draft.borrow().as_ref().unwrap().pending,
            Some(PendingImport::Font { .. })
        );
        self.saving.set(true);
        self.sync();
        self.status.set_text("Saving licensed resource import…");
        let p = self.clone();
        send_requests(
            owner.client.clone(),
            requests.into(),
            Box::new(move |reply| {
                let result = p.draft.borrow_mut().as_mut().unwrap().complete_save(reply);
                p.saving.set(false);
                match &result {
                    Ok(()) => {
                        p.updating.set(true);
                        p.license_name.set_text("");
                        p.license_source.set_text("");
                        p.font_permission.set_active(false);
                        p.pack_permission.set_active(false);
                        p.updating.set(false);
                        p.form_dirty.set(false);
                        p.dirty.set(false);
                        p.status.set_text(if is_font{"Licensed font asset saved and a new alphabet version published. Assign that version explicitly; scene writing still uses manual Save."}else{"Licensed dictionary pack saved locally. Scene writing is unchanged."});
                    }
                    Err(e) => {
                        p.dirty.set(true);
                        p.status.set_text(&format!("Import not fully saved; preview retained. {} Retry or Discard and reopen for changed heads. {e}",if is_font {"A licensed font asset may already be stored."} else {""}));
                    }
                }
                p.sync();
                callback(result);
            }),
        );
    }
}
impl Workspace {
    pub(super) fn licensed_imports(self: &Rc<Self>) {
        let a = desk::effective(self);
        let pid = a.pid.borrow().clone();
        if pid.is_empty() {
            a.error("Choose a novel before importing licensed resources.");
            return;
        }
        let out = a.clone();
        a.navigate(move || {
            if out.desk.is_closed() || *out.pid.borrow() != pid {
                return;
            }
            let p = LicensedImportsPage::new(pid);
            out.licensed_import_pages.borrow_mut().push(p.clone());
            out.install(&p.widget, "Licensed fonts & dictionaries", true);
            p.load();
        });
    }
}
fn caption(row: &gtk::Box, text: &str) {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.);
    label.set_line_wrap(true);
    row.pack_start(&label, false, false, 0);
}

#[cfg(test)]
#[path = "licensed_import_tests.rs"]
pub(super) mod tests;
