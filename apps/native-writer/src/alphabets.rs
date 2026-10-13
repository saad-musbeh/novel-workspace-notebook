//! Native alphabet drafts and a callable notebook page. The host owns RPCs,
//! navigation, detached ownership and editor sessions; this module owns no store.
use gtk::prelude::*;
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

type Result<T> = std::result::Result<T, String>;
const MAX_VERSION: u64 = 9_007_199_254_740_991;

pub(crate) fn empty_document() -> Value {
    json!({"schemaVersion":1,"systems":[],"assignments":[]})
}
fn string<'a>(v: &'a Value, field: &str, max: usize, required: bool) -> Result<&'a str> {
    v[field]
        .as_str()
        .filter(|s| s.encode_utf16().count() <= max && (!required || !s.trim().is_empty()))
        .ok_or_else(|| format!("Invalid alphabet {field}."))
}
fn array<'a>(v: &'a Value, field: &str, max: usize) -> Result<&'a Vec<Value>> {
    v[field]
        .as_array()
        .filter(|a| a.len() <= max)
        .ok_or_else(|| format!("Invalid alphabet {field} collection."))
}
fn scalar(v: &Value) -> Result<char> {
    let mut chars = v
        .as_str()
        .ok_or("Enter one Unicode scalar per mapping key and symbol.")?
        .chars();
    let c = chars
        .next()
        .ok_or("Enter one Unicode scalar per mapping key and symbol.")?;
    if chars.next().is_some() || c.is_control() || invisible_format(c) {
        return Err("Mapping keys and symbols must be single visible Unicode scalars.".into());
    }
    Ok(c)
}
// Unicode format characters are excluded as in the retained TypeScript model.
// Combining marks remain exact and permitted. This is not shaping support.
fn invisible_format(c: char) -> bool {
    matches!(c as u32, 0x00ad | 0x0600..=0x0605 | 0x061c | 0x06dd | 0x070f |
        0x0890..=0x0891 | 0x08e2 | 0x180e | 0x200b..=0x200f | 0x202a..=0x202e |
        0x2060..=0x2064 | 0x2066..=0x206f | 0xfeff | 0xfff9..=0xfffb |
        0x110bd | 0x110cd | 0x13430..=0x1343f | 0x1bca0..=0x1bca3 |
        0x1d173..=0x1d17a | 0xe0001 | 0xe0020..=0xe007f)
}
pub(crate) fn parse_codepoint(input: &str) -> Result<String> {
    let input = input.trim();
    let hex = input
        .strip_prefix("U+")
        .or_else(|| input.strip_prefix("u+"))
        .or_else(|| input.strip_prefix("0x"))
        .or_else(|| input.strip_prefix("0X"))
        .unwrap_or(input);
    if hex.is_empty() || hex.len() > 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Enter one Unicode codepoint such as U+E000.".into());
    }
    u32::from_str_radix(hex, 16)
        .ok()
        .and_then(char::from_u32)
        .map(|c| c.to_string())
        .ok_or_else(|| "This codepoint is not a Unicode scalar.".into())
}
pub(crate) fn codepoints(text: &str) -> String {
    text.chars()
        .map(|c| format!("U+{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}
fn key(d: &Value) -> Result<String> {
    let id = string(d, "id", 199, true)?;
    let version = d["mappingVersion"]
        .as_u64()
        .filter(|v| *v > 0 && *v <= MAX_VERSION)
        .ok_or("Invalid alphabet mapping version.")?;
    Ok(format!("{id}@{version}"))
}
pub(crate) fn validate_definition(d: &Value) -> Result<()> {
    if d["schemaVersion"] != 1 {
        return Err("Unsupported alphabet definition version.".into());
    }
    key(d)?;
    string(d, "name", 200, true)?;
    let script = string(d, "scriptId", 64, true)?;
    if !script.as_bytes()[0].is_ascii_alphanumeric()
        || !script
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(
            "Use an ASCII script identifier with letters, digits, underscores or hyphens.".into(),
        );
    }
    let encoding = string(d, "encoding", 32, true)?;
    if !["unicode", "private-use", "font-presentation"].contains(&encoding)
        || !["auto", "ltr", "rtl"].contains(&d["direction"].as_str().unwrap_or(""))
    {
        return Err("Invalid alphabet encoding or direction preference.".into());
    }
    string(d, "notes", 100_000, false)?;
    let mut inputs = HashSet::new();
    let mut outputs = HashSet::new();
    for m in array(d, "mappings", 4096)? {
        let input = scalar(&m["input"])?;
        let output = scalar(&m["output"])?;
        string(m, "transliteration", 200, false)?;
        if !inputs.insert(input) || !outputs.insert(output) {
            return Err("Mapping input keys and output symbols must be unique.".into());
        }
        if encoding == "private-use"
            && !matches!(output as u32, 0xe000..=0xf8ff | 0xf0000..=0xffffd | 0x100000..=0x10fffd)
        {
            return Err("Private-use output must use a documented Unicode PUA range.".into());
        }
        if encoding == "font-presentation" && input != output {
            return Err("Font presentation keeps underlying Unicode unchanged.".into());
        }
    }
    for g in array(d, "glossary", 4096)? {
        string(g, "symbol", 500, false)?;
        string(g, "meaning", 2000, false)?;
    }
    let font = d.get("font").ok_or("Missing alphabet font reference.")?;
    if !font.is_null() {
        string(font, "fontAssetId", 200, true)?;
        string(font, "family", 200, true)?;
        let checksum = string(font, "checksum", 64, true)?;
        if checksum.len() != 64
            || !checksum
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !["ttf", "otf"].contains(&font["format"].as_str().unwrap_or(""))
        {
            return Err("Invalid project font format or SHA-256 checksum.".into());
        }
        string(&font["license"], "name", 5000, true)?;
        string(&font["license"], "source", 5000, true)?;
        if font["license"]["permissionConfirmed"] != true {
            return Err("Record permission to store and transfer this font.".into());
        }
    }
    Ok(())
}
pub(crate) fn validate_document(doc: &Value) -> Result<()> {
    if doc["schemaVersion"] != 1 {
        return Err("Unsupported alphabet document version.".into());
    }
    if serde_json::to_vec(doc).map_err(|e| e.to_string())?.len() > 8_000_000 {
        return Err("Alphabet project data exceeds 8 MB.".into());
    }
    let mut keys = HashSet::new();
    for d in array(doc, "systems", 4096)? {
        validate_definition(d)?;
        if !keys.insert(key(d)?) {
            return Err("Duplicate immutable alphabet version.".into());
        }
    }
    let mut scenes = HashSet::new();
    for a in array(doc, "assignments", 10_000)? {
        let id = string(&a["scene"], "id", 200, true)?;
        if a["scene"]["kind"] != "scene" || !scenes.insert(id) {
            return Err("Scene alphabet assignments must be unique.".into());
        }
        let reference = json!({"id":a["definitionId"],"mappingVersion":a["mappingVersion"]});
        if !keys.contains(&key(&reference)?) {
            return Err(
                "An assigned mapping version is unavailable. Original writing is preserved.".into(),
            );
        }
    }
    Ok(())
}
fn retain_versions(previous: &Value, next: &Value) -> Result<()> {
    for d in array(previous, "systems", 4096)? {
        if !array(next, "systems", 4096)?.contains(d) {
            return Err(
                "Saved alphabet versions cannot be changed or removed. Publish a new version."
                    .into(),
            );
        }
    }
    Ok(())
}

/// Host supplies checksum/ownership-checked snapshots of EVERY retained revision,
/// an exact current-head capture, and checked live scene revisions from this novel.
/// These are capability results, not values accepted from an imported file.
#[derive(Clone, Debug)]
pub(crate) struct Capture {
    project_id: String,
    document: Value,
    parent: Option<String>,
    pub heads: Vec<String>,
    scenes: HashMap<String, String>,
}
impl Capture {
    pub fn checked(
        project_id: String,
        heads: Vec<String>,
        current: Option<Value>,
        history: &[Value],
        scenes: &[(String, String)],
    ) -> Result<Self> {
        if project_id.trim().is_empty() || project_id.len() > 200 {
            return Err("Invalid novel identity.".into());
        }
        if history.len() > 4096 || scenes.len() > 20_000 {
            return Err("The checked alphabet capture exceeds its history or scene bound.".into());
        }
        let parent = current
            .as_ref()
            .and_then(|v| v["revisionId"].as_str())
            .map(str::to_owned);
        let expected: Vec<String> = parent.clone().into_iter().collect();
        if heads != expected || heads.iter().any(|h| h.is_empty()) {
            return Err("Conflict: alphabet heads changed or have competing versions. Use project data recovery.".into());
        }
        let document = current
            .as_ref()
            .map(|v| v["payload"].clone())
            .unwrap_or_else(empty_document);
        validate_document(&document)?;
        if current.is_some() && parent.is_none() {
            return Err("Missing saved alphabet revision.".into());
        }
        let mut ids = HashSet::new();
        for snapshot in history {
            let id = string(snapshot, "revisionId", 200, true)?;
            if !ids.insert(id) {
                return Err("Duplicate retained alphabet revision.".into());
            }
            validate_document(&snapshot["payload"])?;
            retain_versions(&snapshot["payload"], &document)?;
        }
        if let Some(parent) = &parent {
            if !history
                .iter()
                .any(|v| v["revisionId"] == *parent && v["payload"] == document)
            {
                return Err("The checked current version is absent from retained history.".into());
            }
        } else if !history.is_empty() {
            return Err(
                "Resolve retained alphabet history before creating a first version.".into(),
            );
        }
        let mut sources = HashMap::new();
        for (scene, revision) in scenes {
            if scene.trim().is_empty()
                || scene.len() > 200
                || revision.is_empty()
                || revision.len() > 200
                || sources.insert(scene.clone(), revision.clone()).is_some()
            {
                return Err("Invalid or duplicate checked scene source.".into());
            }
        }
        for a in array(&document, "assignments", 10_000)? {
            if !sources.contains_key(a["scene"]["id"].as_str().unwrap()) {
                return Err("An alphabet assignment has no checked live scene source.".into());
            }
        }
        Ok(Self {
            project_id,
            document,
            parent,
            heads,
            scenes: sources,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Draft {
    capture: Capture,
    pub document: Value,
    pub definition: Option<Value>,
    definition_dirty: bool,
    pub saving: bool,
    pending: Option<Value>,
}
impl Draft {
    pub fn new(capture: Capture) -> Self {
        Self {
            document: capture.document.clone(),
            capture,
            definition: None,
            definition_dirty: false,
            saving: false,
            pending: None,
        }
    }
    pub fn dirty(&self) -> bool {
        self.definition_dirty || self.document != self.capture.document
    }
    fn editable(&self) -> Result<()> {
        if self.saving {
            Err("The alphabet page is saving.".into())
        } else {
            Ok(())
        }
    }
    pub fn choose(&mut self, selected: Option<&str>) -> Result<()> {
        self.editable()?;
        if self.dirty() {
            return Err(
                "Save or discard the alphabet draft before choosing another version.".into(),
            );
        }
        self.definition = if let Some(selected) = selected {
            Some(
                array(&self.document, "systems", 4096)?
                    .iter()
                    .find(|d| key(d).as_deref() == Ok(selected))
                    .ok_or("Choose a saved alphabet version.")?
                    .clone(),
            )
        } else {
            None
        };
        Ok(())
    }
    pub fn create(&mut self) -> Result<()> {
        self.choose(None)?;
        self.definition = Some(
            json!({"schemaVersion":1,"id":uuid::Uuid::new_v4().to_string(),"mappingVersion":1,"name":"New alphabet","scriptId":"custom","encoding":"private-use","direction":"ltr","mappings":[],"font":null,"glossary":[],"notes":""}),
        );
        self.definition_dirty = true;
        Ok(())
    }
    pub fn edit(&mut self, definition: Value) -> Result<()> {
        self.editable()?;
        let old = self
            .definition
            .as_ref()
            .ok_or("Choose or create an alphabet draft.")?;
        if definition["id"] != old["id"] || definition["mappingVersion"] != old["mappingVersion"] {
            return Err("Draft identity and source version cannot be edited.".into());
        }
        self.definition = Some(definition);
        self.definition_dirty = true;
        Ok(())
    }
    pub fn assign(&mut self, scene: &str, selected: Option<&str>) -> Result<()> {
        self.editable()?;
        if !self.capture.scenes.contains_key(scene) {
            return Err("Choose a checked scene in this novel.".into());
        }
        let definition = selected
            .map(|selected| {
                array(&self.document, "systems", 4096)?
                    .iter()
                    .find(|d| key(d).as_deref() == Ok(selected))
                    .cloned()
                    .ok_or_else(|| "Choose an existing immutable alphabet version.".to_owned())
            })
            .transpose()?;
        let mut next = self.document.clone();
        let assignments = next["assignments"].as_array_mut().unwrap();
        assignments.retain(|a| a["scene"]["id"] != scene);
        if let Some(d) = definition {
            assignments.push(json!({"scene":{"kind":"scene","id":scene},"definitionId":d["id"],"mappingVersion":d["mappingVersion"]}));
        }
        validate_document(&next)?;
        self.document = next;
        Ok(())
    }
    /// Stages one new immutable version once. Failed saves keep this exact candidate.
    pub fn begin_save(&mut self) -> Result<Value> {
        self.editable()?;
        if !self.dirty() {
            return Err("There are no alphabet changes to save.".into());
        }
        validate_document(&self.document)?;
        let mut candidate = self.document.clone();
        if self.definition_dirty {
            let mut version = self.definition.clone().ok_or("Missing alphabet draft.")?;
            validate_definition(&version)?;
            let latest = array(&candidate, "systems", 4096)?
                .iter()
                .filter(|d| d["id"] == version["id"])
                .map(|d| d["mappingVersion"].as_u64().unwrap())
                .max()
                .unwrap_or(0);
            if latest >= MAX_VERSION {
                return Err("Alphabet version limit reached.".into());
            }
            version["mappingVersion"] = json!(latest + 1);
            candidate["systems"].as_array_mut().unwrap().push(version);
        }
        validate_document(&candidate)?;
        retain_versions(&self.capture.document, &candidate)?;
        let mut dependencies = Vec::new();
        for (scene, revision) in &self.capture.scenes {
            let assignment = |d: &Value| {
                d["assignments"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|a| a["scene"]["id"] == *scene)
                    .cloned()
            };
            if assignment(&candidate) != assignment(&self.capture.document) {
                dependencies
                    .push(json!({"entityId":scene,"entityType":"scene","revisionId":revision}));
            }
        }
        if dependencies.len() > 50 {
            return Err("Save at most 50 scene assignment changes at a time.".into());
        }
        dependencies.sort_by(|a, b| a["entityId"].as_str().cmp(&b["entityId"].as_str()));
        self.pending = Some(candidate.clone());
        self.saving = true;
        Ok(
            json!({"projectId":self.capture.project_id,"kind":"writingSystems","payload":candidate,"parent":self.capture.parent,"dependencies":dependencies}),
        )
    }
    pub fn complete_save(&mut self, reply: Result<Value>) -> Result<()> {
        if !self.saving {
            return Err("No alphabet save is pending.".into());
        }
        self.saving = false;
        let saved = reply?;
        let revision = string(&saved, "revisionId", 200, true)?.to_owned();
        if Some(&revision) == self.capture.parent.as_ref()
            || Some(&saved["payload"]) != self.pending.as_ref()
        {
            return Err("Alphabet save acknowledgement differs; pending values retained.".into());
        }
        self.capture.document = saved["payload"].clone();
        self.capture.parent = Some(revision.clone());
        self.capture.heads = vec![revision];
        self.document = self.capture.document.clone();
        self.definition = None;
        self.definition_dirty = false;
        self.pending = None;
        Ok(())
    }
    pub fn discard(&mut self) -> Result<()> {
        self.editable()?;
        self.document = self.capture.document.clone();
        self.definition = None;
        self.definition_dirty = false;
        self.pending = None;
        Ok(())
    }
    #[cfg(test)]
    pub fn session_profile(&self, scene: &str) -> Result<SessionProfile> {
        SessionProfile::checked(&self.capture, scene)
    }
    pub fn saved_document(&self) -> &Value {
        &self.capture.document
    }
}

/// Call ONLY for a committed IME/ordinary typing event with a saved session profile.
/// Clipboard, drop, palette insertion, undo, saved text and replacement bypass it.
pub(crate) fn map_ordinary_typing(text: &str, definition: Option<&Value>) -> Result<String> {
    let Some(d) = definition else {
        return Ok(text.into());
    };
    validate_definition(d)?;
    let map: HashMap<char, char> = d["mappings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["input"].as_str().unwrap().chars().next().unwrap(),
                m["output"].as_str().unwrap().chars().next().unwrap(),
            )
        })
        .collect();
    Ok(text
        .chars()
        .map(|c| map.get(&c).copied().unwrap_or(c))
        .collect())
}
pub(crate) fn selected_definition<'a>(doc: &'a Value, scene: &str) -> Result<Option<&'a Value>> {
    validate_document(doc)?;
    Ok(doc["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["scene"]["id"] == scene)
        .and_then(|a| {
            doc["systems"].as_array().unwrap().iter().find(|d| {
                d["id"] == a["definitionId"] && d["mappingVersion"] == a["mappingVersion"]
            })
        }))
}

/// Explicit origin keeps literal input out of mapping, even if a profile is invalid.
#[derive(Clone, Copy, Debug)]
pub(crate) enum InputOrigin {
    OrdinaryTyping,
    Literal,
}
pub(crate) fn input_text(
    text: &str,
    definition: Option<&Value>,
    origin: InputOrigin,
) -> Result<String> {
    match origin {
        InputOrigin::OrdinaryTyping => map_ordinary_typing(text, definition),
        InputOrigin::Literal => Ok(text.to_owned()),
    }
}
/// Immutable per-pane profile captured from the SAVED document. Carry this with
/// the editor session during detachment; draft assignments never alter a session.
#[derive(Clone, Debug)]
pub(crate) struct SessionProfile {
    pub project_id: String,
    pub scene_id: String,
    pub scene_revision_id: String,
    pub alphabet_revision_id: Option<String>,
    pub definition: Option<Value>,
}
impl SessionProfile {
    pub fn checked(capture: &Capture, scene_id: &str) -> Result<Self> {
        let revision = capture
            .scenes
            .get(scene_id)
            .ok_or("The scene has no checked source in this novel.")?;
        Ok(Self {
            project_id: capture.project_id.clone(),
            scene_id: scene_id.into(),
            scene_revision_id: revision.clone(),
            alphabet_revision_id: capture.parent.clone(),
            definition: selected_definition(&capture.document, scene_id)?.cloned(),
        })
    }
    /// Root passes true only after verified private font staging. false gives a
    /// default-font presentation; text/input identity and saving stay independent.
    pub fn editor_selection(&self, verified_font_available: bool) -> Value {
        match &self.definition {
            Some(d) => {
                json!({"definitionId":d["id"],"mappingVersion":d["mappingVersion"],"direction":d["direction"],"fontFamily":if verified_font_available { d["font"]["family"].clone() } else { Value::Null }})
            }
            None => Value::Null,
        }
    }
}

pub(crate) type SaveDone = Box<dyn FnOnce(Result<Value>)>;
pub(crate) type SaveHandler = Rc<dyn Fn(Value, SaveDone)>;
/// (scene ID, immutable definition, exact output). Resolve current widget owner in
/// the host; refuse composition/pending editor work and perform one unsaved edit.
pub(crate) type InsertHandler = Rc<dyn Fn(String, Value, String) -> Result<()>>;

pub(crate) struct AlphabetPage {
    pub widget: gtk::Box,
    pub dirty: Cell<bool>,
    pub saving: Cell<bool>,
    pub composing: Cell<bool>,
    pub loaded: Cell<bool>,
    pub status: gtk::Label,
    pub draft: RefCell<Draft>,
    controls: gtk::Box,
    pub versions: gtk::ComboBoxText,
    pub scenes: gtk::ComboBoxText,
    pub assignment: gtk::ComboBoxText,
    pub name: gtk::Entry,
    pub script: gtk::Entry,
    pub encoding: gtk::ComboBoxText,
    pub direction: gtk::ComboBoxText,
    mappings: gtk::Box,
    glossary: gtk::Box,
    pub notes: gtk::TextView,
    updating: Cell<bool>,
    compositions: RefCell<HashSet<usize>>,
    pending_entries: Vec<gtk::Entry>,
    save_handler: SaveHandler,
    insert_handler: InsertHandler,
}
fn caption(row: &gtk::Box, text: &str) {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.);
    label.set_line_wrap(true);
    row.pack_start(&label, false, false, 0);
}
fn entry(row: &gtk::Box, label: &str) -> gtk::Entry {
    caption(row, label);
    let e = gtk::Entry::new();
    row.pack_start(&e, false, false, 0);
    e
}
fn combo(row: &gtk::Box, label: &str, choices: &[&str]) -> gtk::ComboBoxText {
    caption(row, label);
    let c = gtk::ComboBoxText::new();
    for s in choices {
        c.append(Some(s), s);
    }
    row.pack_start(&c, false, false, 0);
    c
}
impl AlphabetPage {
    /// Construct on the GTK main thread AFTER checked loading. No RPC/UI is run
    /// during pure tests. The root attaches normal notebook lifecycle and ownership.
    pub fn new(
        capture: Capture,
        save_handler: SaveHandler,
        insert_handler: InsertHandler,
    ) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        widget.set_margin_start(32);
        widget.set_margin_end(32);
        widget.set_margin_top(26);
        widget.set_margin_bottom(20);
        caption(
            &widget,
            "Alphabets · saved mapping versions remain immutable",
        );
        caption(&widget,"Changes use manual Save. Mapping applies to new ordinary typing; paste stays exact. Direction is a preference, not a shaping guarantee.");
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
        widget.pack_start(&controls, false, false, 0);
        let versions = combo(&controls, "Saved version", &[]);
        let new_button = gtk::Button::with_label("New alphabet");
        controls.pack_start(&new_button, false, false, 0);
        let name = entry(&controls, "Name");
        let script = entry(&controls, "Script identifier");
        let encoding = combo(
            &controls,
            "Encoding",
            &["unicode", "private-use", "font-presentation"],
        );
        let direction = combo(&controls, "Direction preference", &["auto", "ltr", "rtl"]);
        caption(
            &controls,
            "Symbols · click a saved symbol to insert its exact codepoint",
        );
        let mappings = gtk::Box::new(gtk::Orientation::Vertical, 4);
        controls.pack_start(&mappings, false, false, 0);
        let input = entry(&controls, "One input character");
        let output = entry(&controls, "Output codepoint (for example U+E000)");
        let roman = entry(&controls, "Optional transliteration");
        let add_mapping = gtk::Button::with_label("Add symbol");
        controls.pack_start(&add_mapping, false, false, 0);
        caption(&controls, "Glossary");
        let glossary = gtk::Box::new(gtk::Orientation::Vertical, 4);
        controls.pack_start(&glossary, false, false, 0);
        let symbol = entry(&controls, "Exact glossary symbol / word");
        let meaning = entry(&controls, "Meaning");
        let add_glossary = gtk::Button::with_label("Add glossary entry");
        controls.pack_start(&add_glossary, false, false, 0);
        caption(&controls, "Notes");
        let notes = gtk::TextView::new();
        notes.set_wrap_mode(gtk::WrapMode::WordChar);
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.set_min_content_height(100);
        scroll.add(&notes);
        controls.pack_start(&scroll, true, true, 0);
        caption(&controls,"Use Licensed fonts & dictionaries to import a font for a saved alphabet. Existing writing pages keep their current mapping until reopened.");
        let scenes = combo(&controls, "Scene (checked ID)", &[]);
        let assignment = combo(&controls, "Scene alphabet version", &[]);
        let save_button = gtk::Button::with_label("Save / publish new version");
        controls.pack_start(&save_button, false, false, 0);
        let discard_button = gtk::Button::with_label("Discard changes");
        controls.pack_start(&discard_button, false, false, 0);
        let status = gtk::Label::new(Some(
            "Saved locally · choose a version or create an alphabet",
        ));
        status.set_xalign(0.);
        status.set_line_wrap(true);
        status.set_selectable(true);
        widget.pack_start(&status, false, false, 0);
        let p = Rc::new(Self {
            widget,
            controls,
            versions,
            scenes,
            assignment,
            name,
            script,
            encoding,
            direction,
            mappings,
            glossary,
            notes,
            status,
            draft: RefCell::new(Draft::new(capture)),
            dirty: Cell::new(false),
            saving: Cell::new(false),
            composing: Cell::new(false),
            loaded: Cell::new(true),
            updating: Cell::new(false),
            compositions: RefCell::new(HashSet::new()),
            pending_entries: vec![
                input.clone(),
                output.clone(),
                roman.clone(),
                symbol.clone(),
                meaning.clone(),
            ],
            save_handler,
            insert_handler,
        });
        let weak = Rc::downgrade(&p);
        new_button.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if !p.guard_selection() {
                    return;
                }
                let r = p.draft.borrow_mut().create();
                p.result(r);
                p.sync();
            }
        });
        let weak = Rc::downgrade(&p);
        p.versions.connect_changed(move |c| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() {
                    if !p.guard_selection() {
                        // Restore only the selector; keep active preedit untouched.
                        let selected = p
                            .draft
                            .borrow()
                            .definition
                            .as_ref()
                            .and_then(|d| key(d).ok());
                        p.updating.set(true);
                        p.versions.set_active_id(selected.as_deref());
                        p.updating.set(false);
                        return;
                    }
                    let id = c.active_id();
                    let r = p.draft.borrow_mut().choose(id.as_deref());
                    p.result(r);
                    p.sync();
                }
            }
        });
        for (index, field) in [
            &p.name, &p.script, &input, &output, &roman, &symbol, &meaning,
        ]
        .into_iter()
        .enumerate()
        {
            let weak = Rc::downgrade(&p);
            field.connect_preedit_changed(move |_, text| {
                if let Some(p) = weak.upgrade() {
                    p.composition(index, !text.is_empty());
                }
            });
        }
        for field in &p.pending_entries {
            let weak = Rc::downgrade(&p);
            field.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.dirty
                        .set(p.draft.borrow().dirty() || p.has_pending_entries());
                }
            });
        }
        let weak = Rc::downgrade(&p);
        p.notes.connect_preedit_changed(move |_, text| {
            if let Some(p) = weak.upgrade() {
                p.composition(7, !text.is_empty());
            }
        });
        for field in [&p.name, &p.script] {
            let weak = Rc::downgrade(&p);
            field.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.fields_changed();
                }
            });
        }
        for field in [&p.encoding, &p.direction] {
            let weak = Rc::downgrade(&p);
            field.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.fields_changed();
                }
            });
        }
        let weak = Rc::downgrade(&p);
        p.notes.buffer().unwrap().connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.fields_changed();
            }
        });
        let weak = Rc::downgrade(&p);
        add_mapping.connect_clicked(move |_| if let Some(p)=weak.upgrade() {
            if p.composing.get() { p.status.set_text("Finish the current input composition."); return; }
            let r=(|| { let mut d=p.draft.borrow().definition.clone().ok_or("Choose or create an alphabet.")?; let out=parse_codepoint(&output.text())?; d["mappings"].as_array_mut().unwrap().push(json!({"input":input.text().as_str(),"output":out,"transliteration":roman.text().as_str()})); validate_definition(&d)?; p.draft.borrow_mut().edit(d) })();
            let ok=r.is_ok(); p.result(r); if ok { input.set_text(""); output.set_text(""); roman.set_text(""); } p.sync();
        });
        let weak = Rc::downgrade(&p);
        add_glossary.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.composing.get() {
                    p.status.set_text("Finish the current input composition.");
                    return;
                }
                let r = (|| {
                    let mut d = p
                        .draft
                        .borrow()
                        .definition
                        .clone()
                        .ok_or("Choose or create an alphabet.")?;
                    if symbol.text().is_empty() {
                        return Err("Enter a glossary symbol or word.".into());
                    }
                    d["glossary"].as_array_mut().unwrap().push(
                        json!({"symbol":symbol.text().as_str(),"meaning":meaning.text().as_str()}),
                    );
                    if d["glossary"].as_array().unwrap().len() > 4096
                        || symbol.text().encode_utf16().count() > 500
                        || meaning.text().encode_utf16().count() > 2000
                    {
                        return Err("Glossary entry exceeds its bound.".into());
                    }
                    p.draft.borrow_mut().edit(d)
                })();
                let ok = r.is_ok();
                p.result(r);
                if ok {
                    symbol.set_text("");
                    meaning.set_text("");
                }
                p.sync();
            }
        });
        let weak = Rc::downgrade(&p);
        p.scenes.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() {
                    p.sync_assignment();
                }
            }
        });
        let weak = Rc::downgrade(&p);
        p.assignment.connect_changed(move |c| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() {
                    if let Some(scene) = p.scenes.active_id() {
                        let id = c.active_id();
                        let r = p
                            .draft
                            .borrow_mut()
                            .assign(&scene, id.as_deref().filter(|s| !s.is_empty()));
                        p.result(r);
                    }
                }
            }
        });
        let weak = Rc::downgrade(&p);
        save_button.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.save(Box::new(|_| {}));
            }
        });
        let weak = Rc::downgrade(&p);
        discard_button.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.discard();
            }
        });
        p.sync();
        p
    }
    fn has_pending_entries(&self) -> bool {
        self.pending_entries.iter().any(|e| !e.text().is_empty())
    }
    fn guard_selection(&self) -> bool {
        if self.composing.get() || self.has_pending_entries() {
            self.status.set_text("Finish composition and add or clear pending symbol/glossary fields before changing versions.");
            false
        } else {
            true
        }
    }
    fn composition(&self, index: usize, active: bool) {
        let mut states = self.compositions.borrow_mut();
        if active {
            states.insert(index);
        } else {
            states.remove(&index);
        }
        self.composing.set(!states.is_empty());
    }
    fn result(&self, result: Result<()>) {
        self.dirty
            .set(self.draft.borrow().dirty() || self.has_pending_entries());
        match result {
            Ok(()) => self.status.set_text(if self.dirty.get() {
                "Unsaved alphabet changes · Save to keep them"
            } else {
                "Saved locally"
            }),
            Err(e) => self.status.set_text(&e),
        }
    }
    fn fields_changed(&self) {
        if self.updating.get() {
            return;
        }
        let Some(mut d) = self.draft.borrow().definition.clone() else {
            return;
        };
        d["name"] = json!(self.name.text().as_str());
        d["scriptId"] = json!(self.script.text().as_str());
        d["encoding"] = json!(self
            .encoding
            .active_id()
            .as_deref()
            .unwrap_or("private-use"));
        d["direction"] = json!(self.direction.active_id().as_deref().unwrap_or("ltr"));
        let buffer = self.notes.buffer().unwrap();
        d["notes"] = json!(buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .as_deref()
            .unwrap_or(""));
        let r = self.draft.borrow_mut().edit(d);
        self.result(r);
    }
    fn sync_assignment(&self) {
        self.updating.set(true);
        self.assignment.remove_all();
        self.assignment.append(Some(""), "Ordinary Unicode typing");
        let draft = self.draft.borrow();
        for d in draft.document["systems"].as_array().unwrap() {
            self.assignment.append(
                Some(&key(d).unwrap()),
                &format!(
                    "{} · version {}",
                    d["name"].as_str().unwrap(),
                    d["mappingVersion"]
                ),
            );
        }
        let selected = self.scenes.active_id().and_then(|scene| {
            selected_definition(&draft.document, &scene)
                .ok()
                .flatten()
                .and_then(|d| key(d).ok())
        });
        self.assignment
            .set_active_id(Some(selected.as_deref().unwrap_or("")));
        self.updating.set(false);
    }
    fn sync(self: &Rc<Self>) {
        self.updating.set(true);
        self.versions.remove_all();
        let draft = self.draft.borrow();
        for d in draft.document["systems"].as_array().unwrap() {
            self.versions.append(
                Some(&key(d).unwrap()),
                &format!(
                    "{} · version {}",
                    d["name"].as_str().unwrap(),
                    d["mappingVersion"]
                ),
            );
        }
        let definition = draft.definition.clone();
        let has_definition = definition.is_some();
        self.name.set_sensitive(has_definition);
        self.script.set_sensitive(has_definition);
        self.encoding.set_sensitive(has_definition);
        self.direction.set_sensitive(has_definition);
        self.notes.set_sensitive(has_definition);
        for field in &self.pending_entries {
            field.set_sensitive(has_definition);
        }
        if let Some(d) = &definition {
            self.versions.set_active_id(Some(&key(d).unwrap()));
        }
        self.name.set_text(
            definition
                .as_ref()
                .and_then(|d| d["name"].as_str())
                .unwrap_or(""),
        );
        self.script.set_text(
            definition
                .as_ref()
                .and_then(|d| d["scriptId"].as_str())
                .unwrap_or(""),
        );
        self.encoding
            .set_active_id(definition.as_ref().and_then(|d| d["encoding"].as_str()));
        self.direction
            .set_active_id(definition.as_ref().and_then(|d| d["direction"].as_str()));
        self.notes.buffer().unwrap().set_text(
            definition
                .as_ref()
                .and_then(|d| d["notes"].as_str())
                .unwrap_or(""),
        );
        let scene = self.scenes.active_id();
        self.scenes.remove_all();
        let mut sources = draft.capture.scenes.keys().collect::<Vec<_>>();
        sources.sort();
        for id in sources {
            self.scenes.append(Some(id), id);
        }
        if let Some(scene) = scene {
            self.scenes.set_active_id(Some(&scene));
        } else {
            self.scenes.set_active(Some(0));
        }
        for child in self.mappings.children() {
            self.mappings.remove(&child);
        }
        for child in self.glossary.children() {
            self.glossary.remove(&child);
        }
        if let Some(d) = definition {
            let saved = draft.capture.document["systems"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| key(v).ok() == key(&d).ok())
                .cloned();
            for (i, m) in d["mappings"].as_array().unwrap().iter().enumerate() {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                self.mappings.pack_start(&row, false, false, 0);
                let text = m["output"].as_str().unwrap();
                let insert = gtk::Button::with_label(&format!(
                    "{} · {} ← {} · {}",
                    text,
                    codepoints(text),
                    m["input"].as_str().unwrap(),
                    m["transliteration"].as_str().unwrap()
                ));
                row.pack_start(&insert, false, false, 0);
                let exact = saved
                    .as_ref()
                    .is_some_and(|v| v["mappings"].as_array().unwrap().contains(m));
                insert.set_sensitive(exact);
                let weak = Rc::downgrade(self);
                let saved = saved.clone();
                let text = text.to_owned();
                insert.connect_clicked(move |_| {
                    if let Some(p) = weak.upgrade() {
                        if p.composing.get() || p.saving.get() {
                            p.status.set_text(
                                "Finish composition or saving before inserting a symbol.",
                            );
                            return;
                        }
                        if let (Some(scene), Some(d)) = (p.scenes.active_id(), saved.clone()) {
                            p.result((p.insert_handler)(scene.to_string(), d, text.clone()));
                        }
                    }
                });
                self.remove_button(&row, "mappings", i);
            }
            for (i, g) in d["glossary"].as_array().unwrap().iter().enumerate() {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                self.glossary.pack_start(&row, false, false, 0);
                caption(
                    &row,
                    &format!(
                        "{} · {} — {}",
                        g["symbol"].as_str().unwrap(),
                        codepoints(g["symbol"].as_str().unwrap()),
                        g["meaning"].as_str().unwrap()
                    ),
                );
                self.remove_button(&row, "glossary", i);
            }
        }
        drop(draft);
        self.updating.set(false);
        self.sync_assignment();
        self.controls.set_sensitive(!self.saving.get());
        self.widget.show_all();
    }
    fn remove_button(self: &Rc<Self>, row: &gtk::Box, field: &'static str, index: usize) {
        let b = gtk::Button::with_label("Remove");
        row.pack_start(&b, false, false, 0);
        let weak = Rc::downgrade(self);
        b.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.composing.get() {
                    p.status.set_text("Finish the current input composition.");
                    return;
                }
                let r = (|| {
                    let mut d = p
                        .draft
                        .borrow()
                        .definition
                        .clone()
                        .ok_or("Choose a draft.")?;
                    d[field].as_array_mut().unwrap().remove(index);
                    p.draft.borrow_mut().edit(d)
                })();
                p.result(r);
                p.sync();
            }
        });
    }
    pub fn discard(self: &Rc<Self>) {
        if self.saving.get() {
            self.status.set_text("The alphabet page is saving.");
            return;
        }
        if self.composing.get() {
            self.status
                .set_text("Finish the current input composition.");
            return;
        }
        let r = self.draft.borrow_mut().discard();
        if r.is_ok() {
            for e in &self.pending_entries {
                e.set_text("");
            }
        }
        self.result(r);
        self.sync();
    }
    pub fn save(self: &Rc<Self>, callback: Box<dyn FnOnce(Result<()>)>) {
        if self.composing.get() {
            self.status
                .set_text("Finish the current input composition.");
            callback(Err("Finish the current input composition.".into()));
            return;
        }
        if self.has_pending_entries() {
            let e = "Add or clear pending symbol/glossary fields before saving.".to_owned();
            self.status.set_text(&e);
            callback(Err(e));
            return;
        }
        if !self.dirty.get() {
            callback(Ok(()));
            return;
        }
        let request = match self.draft.borrow_mut().begin_save() {
            Ok(v) => v,
            Err(e) => {
                self.status.set_text(&e);
                callback(Err(e));
                return;
            }
        };
        self.saving.set(true);
        self.sync();
        self.status.set_text("Saving alphabet changes…");
        let p = self.clone();
        (self.save_handler)(
            request,
            Box::new(move |reply| {
                let r = p.draft.borrow_mut().complete_save(reply);
                p.saving.set(false);
                let error = r.as_ref().err().cloned();
                p.result(r.clone());
                p.sync();
                if let Some(e) = error {
                    p.status.set_text(&format!(
                        "Alphabet changes not saved; pending values retained. {e}"
                    ));
                }
                callback(r);
            }),
        );
    }
}

#[cfg(test)]
#[path = "alphabet_tests.rs"]
mod tests;
