//! Local, explicit export; derived text never replaces any manuscript revision.
use super::*;
use novel_storage::AlphabetExportSnapshot;
use std::collections::HashMap;

pub(crate) const NOTICE: &str = "This is a saved manuscript snapshot with immutable alphabet mapping metadata. Font and dictionary asset bytes are excluded. Transliteration is a derived copy made only from the author's stored rules; it need not be reversible and never replaces the original text. Titles remain original. This file is not a complete project archive or revision-history backup.";

#[cfg(test)]
thread_local! {
    static DESTINATIONS:RefCell<std::collections::VecDeque<Option<PathBuf>>>=RefCell::new(std::collections::VecDeque::new());
    static BEFORE_DELIVERY:RefCell<Option<Box<dyn FnOnce()>>>=RefCell::new(None);
}
fn destination(a:&Workspace)->Option<PathBuf> {
    #[cfg(test)] if let Some(path)=DESTINATIONS.with(|d|d.borrow_mut().pop_front()) {return path;}
    files::choose(&a.window,true,"manuscript.alphabet-manuscript.json",&["*.alphabet-manuscript.json"])
}

fn map_document(node: &mut Value, rules: &HashMap<char, &str>, remaining:&mut usize) -> Result<(),String> {
    if node["type"] == "text" {
        if let Some(text) = node["text"].as_str() {
            let mut mapped=String::new();
            for c in text.chars() {
                let mut scalar=[0u8;4];let replacement=rules.get(&c).copied().unwrap_or_else(||c.encode_utf8(&mut scalar));
                *remaining=remaining.checked_sub(replacement.len()).ok_or("Alphabet export exceeds 64 MiB; no file was written.")?;
                mapped.push_str(replacement);
            }
            node["text"] = json!(mapped);
        }
    }
    if let Some(children) = node.get_mut("content").and_then(Value::as_array_mut) {
        for child in children { map_document(child, rules, remaining)?; }
    }
    Ok(())
}
struct BoundedOutput { bytes:Vec<u8> }
impl std::io::Write for BoundedOutput {
    fn write(&mut self, bytes:&[u8])->std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len())>files::LIMIT { return Err(std::io::Error::other("Alphabet export exceeds 64 MiB; no file was written.")); }
        self.bytes.extend_from_slice(bytes);Ok(bytes.len())
    }
    fn flush(&mut self)->std::io::Result<()> { Ok(()) }
}
fn metadata_fields(value:&Value, allowed:&[&str])->Result<(),String> {
    if value.as_object().is_none_or(|m|m.keys().any(|k|!allowed.contains(&k.as_str()))) {
        return Err("Unsupported alphabet metadata fields. Export accepts schema 1 mapping and license descriptors, without resource bytes.".into());
    }
    Ok(())
}
fn checked_metadata(document:&Value)->Result<(),String> {
    crate::alphabets::validate_document(document)?;
    metadata_fields(document,&["schemaVersion","systems","assignments"])?;
    for definition in document["systems"].as_array().unwrap() {
        metadata_fields(definition,&["schemaVersion","id","mappingVersion","name","scriptId","encoding","direction","mappings","font","glossary","notes"])?;
        for mapping in definition["mappings"].as_array().unwrap() { metadata_fields(mapping,&["input","output","transliteration"])?; }
        for term in definition["glossary"].as_array().unwrap() { metadata_fields(term,&["symbol","meaning"])?; }
        if !definition["font"].is_null() {
            metadata_fields(&definition["font"],&["fontAssetId","checksum","family","format","license"])?;
            metadata_fields(&definition["font"]["license"],&["name","source","permissionConfirmed"])?;
        }
    }
    for assignment in document["assignments"].as_array().unwrap() {
        metadata_fields(assignment,&["scene","definitionId","mappingVersion"])?;
        metadata_fields(&assignment["scene"],&["kind","id"])?;
    }
    Ok(())
}

/// The source is a checked backend capability, never an imported candidate.
pub(crate) fn encode(snapshot: &AlphabetExportSnapshot, transliteration: bool) -> Result<Vec<u8>, String> {
    let empty = crate::alphabets::empty_document();
    let document = snapshot.writing_systems.as_ref().map(|s| &s.payload).unwrap_or(&empty);
    checked_metadata(document)?;
    let versions:HashMap<(&str,u64),&Value>=document["systems"].as_array().unwrap().iter().map(|d|((d["id"].as_str().unwrap(),d["mappingVersion"].as_u64().unwrap()),d)).collect();
    let assignments:HashMap<&str,&Value>=document["assignments"].as_array().unwrap().iter().map(|a| (a["scene"]["id"].as_str().unwrap(),versions[&(a["definitionId"].as_str().unwrap(),a["mappingVersion"].as_u64().unwrap())])).collect();
    let mut derived = Vec::new();
    let mut remaining=files::LIMIT;
    for source in &snapshot.bundle.scenes {
        novel_storage::validate(&source.scene)?;
        let definition = assignments.get(source.id.as_str()).copied();
        if !transliteration { continue; }
        let mut scene = source.scene.clone();
        if let Some(definition) = definition {
            let mut rules = HashMap::new();
            for mapping in definition["mappings"].as_array().ok_or("Invalid alphabet mapping collection.")? {
                let output = mapping["output"].as_str().and_then(|s| s.chars().next()).ok_or("Invalid alphabet output scalar.")?;
                let replacement = mapping["transliteration"].as_str().ok_or("Invalid authored transliteration.")?;
                if !replacement.is_empty() { rules.insert(output, replacement); }
            }
            map_document(&mut scene.document, &rules, &mut remaining)?;
        } else {
            let literal=novel_storage::plain_text(&scene.document);
            remaining=remaining.checked_sub(literal.len()).ok_or("Alphabet export exceeds 64 MiB; no file was written.")?;
        }
        derived.push(json!({"sceneId":source.id,"sceneRevisionId":source.revision_id,"definitionId":definition.map(|d| &d["id"]),"mappingVersion":definition.map(|d| &d["mappingVersion"]),"scene":scene}));
    }
    let package = json!({"schemaVersion":1,"format":"novel-alphabet-manuscript","notice":NOTICE,"original":snapshot.bundle,"writingSystems":snapshot.writing_systems,"transliterated":if transliteration { json!({"scenes":derived}) } else { Value::Null }});
    let mut output=BoundedOutput{bytes:Vec::new()};
    serde_json::to_writer_pretty(&mut output,&package).map_err(|e|e.to_string())?;
    Ok(output.bytes)
}

impl Workspace {
    pub(crate) fn alphabet_export(self: &Rc<Self>) {
        let a = desk::effective(self);
        if a.sequence_window_blocked() { return; }
        let Some(choice) = choose_option(&a.window, "Alphabet manuscript export", &[("original", "Original text and mapping metadata"), ("transliterated", "Also include an author-rule transliterated copy")], "original") else { return; };
        let project = a.pid.borrow().clone();
        let generation = a.desk.generation.get();
        let out = a.clone();
        a.confirm_project_id(&project.clone(), move |accepted| {
            if !accepted || out.desk.is_closed() || out.desk.generation.get() != generation || *out.pid.borrow() != project { return; }
            let a = out.clone();
            out.client.rpc("alphabet_export_snapshot", json!({"projectId":project}), move |reply| {
                if a.desk.is_closed() || a.desk.generation.get() != generation || *a.pid.borrow() != project { return; }
                let source = match reply {
                    Ok(source) => source,
                    Err(e) => { a.error(&e);return; }
                };
                let include=choice=="transliterated";
                let (sender,receiver)=std::sync::mpsc::sync_channel(1);
                let expected=project.clone();
                a.status.set_text("Preparing saved alphabet manuscript…");
                std::thread::spawn(move || {
                    let result=serde_json::from_value::<AlphabetExportSnapshot>(source).map_err(|e|e.to_string()).and_then(|snapshot| {
                        if snapshot.bundle.project.id!=expected { return Err("Export source belongs to a different novel.".into()); }
                        encode(&snapshot,include)
                    });
                    let _=sender.send(result);
                });
                glib::timeout_add_local(std::time::Duration::from_millis(15),move || {
                    let reply=match receiver.try_recv() {
                        Ok(reply)=>reply,
                        Err(std::sync::mpsc::TryRecvError::Empty)=>return glib::ControlFlow::Continue,
                        Err(_)=>{if !a.desk.is_closed(){a.error("Alphabet export worker stopped; no file was written.");}return glib::ControlFlow::Break;}
                    };
                    #[cfg(test)] if let Some(callback)=BEFORE_DELIVERY.with(|hook|hook.borrow_mut().take()) {callback();}
                    if a.desk.is_closed() || a.desk.generation.get()!=generation || *a.pid.borrow()!=project { return glib::ControlFlow::Break; }
                    let bytes=match reply {
                        Ok(bytes)=>bytes,
                        Err(e)=>{a.error(&e);return glib::ControlFlow::Break;},
                    };
                    let Some(path)=destination(&a) else { return glib::ControlFlow::Break; };
                    if a.desk.is_closed() || *a.pid.borrow()!=project || a.desk.generation.get()!=generation { return glib::ControlFlow::Break; }
                    match files::write(&path,&bytes) { Ok(())=>a.status.set_text(&format!("Exported saved alphabet manuscript to {}",path.display())),Err(e)=>a.error(&e) }
                    glib::ControlFlow::Break
                });
            });
        });
    }
}

#[cfg(test)]
#[path = "alphabet_export_tests.rs"]
pub(crate) mod tests;
