//! Read-only local lexical lookup from saved, licensed packs and alphabet glossaries.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
struct Sense {
    definition: String,
    part_of_speech: String,
    synonyms: Vec<String>,
    related: Vec<String>,
    translations: Vec<(String, String)>,
}
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    term: String,
    senses: Vec<Sense>,
    notes: String,
}
#[derive(Clone, Debug, PartialEq)]
struct Source {
    name: String,
    language: String,
    version: String,
    source: String,
    license: String,
    attribution: String,
    entries: Vec<Entry>,
}
fn string(value: &Value, key: &str, max: usize) -> Result<String, String> {
    value[key]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.encode_utf16().count() <= max)
        .map(str::to_owned)
        .ok_or_else(|| format!("Invalid dictionary {key}."))
}
fn array<'a>(value: &'a Value, key: &str, max: usize) -> Result<&'a Vec<Value>, String> {
    value[key]
        .as_array()
        .filter(|a| a.len() <= max)
        .ok_or_else(|| format!("Invalid or oversized dictionary {key}."))
}
fn words(value: &Value, key: &str) -> Result<Vec<String>, String> {
    array(value, key, 100)?
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|s| !s.trim().is_empty() && s.encode_utf16().count() <= 200)
                .map(str::to_owned)
                .ok_or_else(|| format!("Invalid dictionary {key}."))
        })
        .collect()
}
/// Match the retained pack format's JavaScript UTF-16 object-key ordering.
/// Lexical manifests use integer schema versions and strings for all other scalars.
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|k| format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap(),
                        canonical(&map[k])
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Value::Array(rows) => format!(
            "[{}]",
            rows.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        _ => value.to_string(),
    }
}
fn checksum(value: &Value) -> Result<String, String> {
    let mut content = value
        .as_object()
        .cloned()
        .ok_or("A dictionary pack must be an object.")?;
    content.remove("checksum");
    Ok(format!(
        "{:x}",
        Sha256::digest(canonical(&Value::Object(content)).as_bytes())
    ))
}
fn parse_pack(value: &Value) -> Result<Source, String> {
    if serde_json::to_vec(value)
        .map_err(|_| "Unsupported dictionary data.")?
        .len()
        > 2_000_000
    {
        return Err("A dictionary pack exceeds 2 MB.".into());
    }
    if value["schemaVersion"].as_u64() != Some(1) {
        return Err("Unsupported dictionary schema version.".into());
    }
    let id = string(value, "id", 100)?;
    if !id.bytes().next().is_some_and(|c| c.is_ascii_alphanumeric())
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err("Invalid dictionary identity.".into());
    }
    let mut source = Source {
        name: string(value, "name", 500)?,
        language: string(value, "language", 500)?,
        version: string(value, "version", 500)?,
        source: string(value, "source", 500)?,
        license: string(value, "license", 500)?,
        attribution: string(value, "attribution", 5000)?,
        entries: Vec::new(),
    };
    string(value, "variant", 500)?;
    let operations = array(value, "operations", 3)?;
    let mut ops = HashSet::new();
    if operations.is_empty()
        || operations.iter().any(|v| {
            !matches!(v.as_str(), Some("dictionary" | "synonyms" | "glossary"))
                || !ops.insert(v.as_str().unwrap())
        })
    {
        return Err("Unsupported dictionary operations.".into());
    }
    let mut terms = HashSet::new();
    for entry in array(value, "entries", 10000)? {
        let term = string(entry, "term", 200)?;
        if !terms.insert(term.clone()) {
            return Err("Duplicate dictionary term.".into());
        }
        let notes = match entry.get("notes") {
            None => String::new(),
            Some(v) => v
                .as_str()
                .filter(|s| s.encode_utf16().count() <= 2000)
                .ok_or("Invalid dictionary notes.")?
                .to_owned(),
        };
        let items = array(entry, "senses", 100)?;
        if items.is_empty() {
            return Err("A dictionary term needs at least one sense.".into());
        }
        let mut senses = Vec::new();
        let mut identities = HashSet::new();
        for sense in items {
            if !identities.insert(string(sense, "id", 200)?) {
                return Err("Duplicate dictionary sense.".into());
            }
            let synonyms = words(sense, "synonyms")?;
            if !ops.contains("synonyms") && !synonyms.is_empty() {
                return Err("Synonyms are not declared by this dictionary.".into());
            }
            let translations = array(sense, "translations", 100)?
                .iter()
                .map(|t| Ok((string(t, "language", 200)?, string(t, "text", 200)?)))
                .collect::<Result<Vec<_>, String>>()?;
            senses.push(Sense {
                definition: string(sense, "definition", 3000)?,
                part_of_speech: string(sense, "partOfSpeech", 200)?,
                synonyms,
                related: words(sense, "related")?,
                translations,
            });
        }
        source.entries.push(Entry {
            term,
            senses,
            notes,
        });
    }
    let recorded = string(value, "checksum", 64)?;
    if recorded.len() != 64
        || !recorded
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || checksum(value)? != recorded
    {
        return Err("Dictionary checksum mismatch. This source was not used.".into());
    }
    Ok(source)
}
fn pack_sources(value: &Value) -> Result<Vec<Source>, String> {
    if value["schemaVersion"].as_u64() != Some(1) {
        return Err("Unsupported saved language-pack schema.".into());
    }
    let mut ids = HashSet::new();
    let mut sources = Vec::new();
    for pack in array(value, "packs", 20)? {
        if !ids.insert(string(pack, "id", 100)?) {
            return Err("Duplicate saved dictionary identity.".into());
        }
        sources.push(parse_pack(pack)?);
    }
    Ok(sources)
}
fn alphabet_sources(value: &Value) -> Result<Vec<Source>, String> {
    if value["schemaVersion"].as_u64() != Some(1) {
        return Err("Unsupported saved alphabet schema.".into());
    }
    let mut result = Vec::new();
    for alphabet in array(value, "systems", 4096)? {
        let glossary = match alphabet.get("glossary") {
            None => continue,
            Some(_) => array(alphabet, "glossary", 4096)?,
        };
        if glossary.is_empty() {
            continue;
        }
        let version = alphabet["mappingVersion"]
            .as_u64()
            .filter(|v| *v > 0)
            .ok_or("Invalid alphabet version.")?;
        let entries = glossary
            .iter()
            .map(|g| {
                Ok(Entry {
                    term: string(g, "symbol", 500)?,
                    notes: String::new(),
                    senses: vec![Sense {
                        definition: g["meaning"]
                            .as_str()
                            .filter(|s| s.encode_utf16().count() <= 2000)
                            .ok_or("Invalid alphabet glossary meaning.")?
                            .to_owned(),
                        part_of_speech: "Project term".into(),
                        synonyms: Vec::new(),
                        related: Vec::new(),
                        translations: Vec::new(),
                    }],
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        result.push(Source {
            name: format!("{} glossary", string(alphabet, "name", 500)?),
            language: "und".into(),
            version: format!("mapping {version}"),
            source: "Saved project alphabet glossary".into(),
            license: "Project text; no external dictionary license recorded".into(),
            attribution: "Project author".into(),
            entries,
        });
    }
    Ok(result)
}
fn lookup(
    sources: &[Source],
    term: &str,
    language: &str,
    ignore_case: bool,
) -> Vec<(usize, usize)> {
    let query = if ignore_case {
        term.to_lowercase()
    } else {
        term.into()
    };
    sources
        .iter()
        .enumerate()
        .filter(|(_, s)| language == "any" || s.language == language)
        .flat_map(|(source, s)| {
            s.entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    if ignore_case {
                        e.term.to_lowercase() == query
                    } else {
                        e.term == query
                    }
                })
                .map(move |(entry, _)| (source, entry))
        })
        .collect()
}
fn text(container: &gtk::Box, content: &str) {
    let l = gtk::Label::new(Some(content));
    l.set_xalign(0.);
    l.set_line_wrap(true);
    l.set_selectable(true);
    container.pack_start(&l, false, false, 0);
}
struct DictionaryPage {
    widget: glib::WeakRef<gtk::Box>,
    pid: String,
    query: gtk::Entry,
    language: gtk::ComboBoxText,
    ignore_case: gtk::CheckButton,
    status: gtk::Label,
    results: gtk::Box,
    sources: RefCell<Vec<Source>>,
    warnings: RefCell<Vec<String>>,
    generation: Cell<u64>,
    loading: Cell<bool>,
}
impl DictionaryPage {
    fn owner(&self) -> Option<Rc<Workspace>> {
        let w = self.widget.upgrade()?;
        let a = desk::workspace_for_widget(w.upcast_ref())?;
        if a.pid.borrow().as_str() == self.pid && a.pages.page_num(&w).is_some() {
            Some(a)
        } else {
            None
        }
    }
    fn render(&self) {
        for child in self.results.children() {
            self.results.remove(&child);
        }
        let sources = self.sources.borrow();
        let warning = self.warnings.borrow().join("\n");
        if self.loading.get() {
            self.status.set_text("Loading saved local dictionaries…");
            return;
        }
        let count = sources.iter().map(|s| s.entries.len()).sum::<usize>();
        self.status.set_text(&format!(
            "{} saved sources · {count} entries{}",
            sources.len(),
            if warning.is_empty() {
                String::new()
            } else {
                format!("\n{warning}")
            }
        ));
        let query = self.query.text();
        if query.is_empty() {
            text(&self.results,"Enter a word or project symbol. Lookup preserves exact text; ignoring case is optional.");
            self.results.show_all();
            return;
        }
        let language = self
            .language
            .active_id()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "any".into());
        let found = lookup(&sources, &query, &language, self.ignore_case.is_active());
        if found.is_empty() {
            text(
                &self.results,
                if sources.is_empty() {
                    "No saved dictionary or alphabet glossary is available in this novel."
                } else {
                    "No matching entry in the selected language. Spelling, accents and Unicode forms remain distinct."
                },
            );
        }
        if found.len() > 100 {
            text(
                &self.results,
                &format!(
                    "Showing the first 100 of {} matching entries. Every source remains saved.",
                    found.len()
                ),
            );
        }
        for (source, entry) in found.into_iter().take(100) {
            let source = &sources[source];
            let entry = &source.entries[entry];
            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            text(&card, &entry.term);
            text(
                &card,
                &format!("{} · {} · {}", source.name, source.version, source.language),
            );
            text(
                &card,
                &format!(
                    "Source: {}\nDeclared license: {}\nAttribution: {}",
                    source.source, source.license, source.attribution
                ),
            );
            for (i, sense) in entry.senses.iter().enumerate() {
                text(
                    &card,
                    &format!(
                        "Sense {} · {}\n{}",
                        i + 1,
                        sense.part_of_speech,
                        if sense.definition.is_empty() {
                            "No meaning recorded."
                        } else {
                            &sense.definition
                        }
                    ),
                );
                if !sense.synonyms.is_empty() {
                    text(
                        &card,
                        &format!("Synonyms for this sense: {}", sense.synonyms.join(" · ")),
                    );
                }
                if !sense.related.is_empty() {
                    text(
                        &card,
                        &format!("Related words: {}", sense.related.join(" · ")),
                    );
                }
                if !sense.translations.is_empty() {
                    text(
                        &card,
                        &format!(
                            "Translations: {}",
                            sense
                                .translations
                                .iter()
                                .map(|(l, t)| format!("{l}: {t}"))
                                .collect::<Vec<_>>()
                                .join(" · ")
                        ),
                    );
                }
            }
            if !entry.notes.is_empty() {
                text(&card, &format!("Notes: {}", entry.notes));
            }
            self.results.pack_start(&card, false, false, 10);
        }
        self.results.show_all();
    }
    fn load(self: &Rc<Self>) {
        if self.loading.replace(true) {
            return;
        }
        self.generation.set(self.generation.get() + 1);
        self.sources.borrow_mut().clear();
        self.warnings.borrow_mut().clear();
        self.render();
        self.load_kind(0);
    }
    fn load_kind(self: &Rc<Self>, index: usize) {
        let kinds = ["languagePacks", "writingSystems"];
        if index == kinds.len() {
            self.loading.set(false);
            self.render();
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let kind = kinds[index];
        let generation = self.generation.get();
        let p = self.clone();
        a.client.rpc("project_data_heads",json!({"projectId":self.pid,"kind":kind}),move|reply|{
            if p.owner().is_none() || p.generation.get()!=generation{return;}
            let heads=match reply.and_then(|v|serde_json::from_value::<novel_storage::RevisionHeads>(v).map_err(|_|"Unsupported source heads.".into())){Ok(h)=>h.revision_ids,Err(e)=>{p.warnings.borrow_mut().push(format!("{kind}: {e}"));p.load_kind(index+1);return;}};
            if heads.len()>1{p.warnings.borrow_mut().push(format!("{kind}: competing versions; resolve them in Project data conflicts before lookup."));p.load_kind(index+1);return;}
            let Some(head)=heads.first().cloned()else{p.load_kind(index+1);return;};let Some(a)=p.owner()else{return;};let page=p.clone();
            a.client.rpc("load_project_data_revision",json!({"projectId":p.pid,"kind":kind,"revisionId":head}),move|reply|{
                if page.owner().is_none() || page.generation.get()!=generation{return;}
                match reply.and_then(|v|if kind=="languagePacks"{pack_sources(&v["payload"])}else{alphabet_sources(&v["payload"])}){Ok(sources)=>page.sources.borrow_mut().extend(sources),Err(e)=>page.warnings.borrow_mut().push(format!("{kind}: {e}"))}
                page.load_kind(index+1);
            });
        });
    }
}
impl Workspace {
    pub(super) fn dictionary(self: &Rc<Self>) {
        let a = desk::effective(self);
        if a.pid.borrow().is_empty() {
            a.error("Choose a novel before looking up local words.");
            return;
        }
        let out = a.clone();
        a.navigate(move||{
            let panel=out.panel("Dictionary and synonyms");let body=panel.content_area();text(&body,"Read saved local dictionaries and your project alphabet glossaries. Meanings and synonyms stay grouped by sense. Refresh after saving source changes.");
            let controls=gtk::Box::new(gtk::Orientation::Horizontal,8);let query=gtk::Entry::new();query.set_placeholder_text(Some("Word or project symbol"));query.set_max_length(500);controls.pack_start(&query,true,true,0);
            let language=gtk::ComboBoxText::new();language.append(Some("any"),"All languages");for (id,name) in LANGUAGES{language.append(Some(id),name);}language.append(Some("und"),"Project / unspecified language");language.set_active(Some(0));controls.pack_start(&language,false,false,0);body.pack_start(&controls,false,false,0);
            let ignore_case=gtk::CheckButton::with_label("Ignore case for lookup");body.pack_start(&ignore_case,false,false,0);
            let status=gtk::Label::new(None);status.set_xalign(0.);status.set_line_wrap(true);status.set_selectable(true);body.pack_start(&status,false,false,0);let results=gtk::Box::new(gtk::Orientation::Vertical,10);body.pack_start(&scroll(&results),true,true,0);
            let actions=gtk::Box::new(gtk::Orientation::Horizontal,8);body.pack_start(&actions,false,false,0);let reload=gtk::Button::with_label("Reload saved dictionaries");actions.pack_start(&reload,false,false,0);
            let page=Rc::new(DictionaryPage{widget:panel.widget.downgrade(),pid:out.pid.borrow().clone(),query:query.clone(),language:language.clone(),ignore_case:ignore_case.clone(),status,results,sources:RefCell::new(Vec::new()),warnings:RefCell::new(Vec::new()),generation:Cell::new(0),loading:Cell::new(false)});
            let weak=Rc::downgrade(&page);query.connect_changed(move|_|if let Some(p)=weak.upgrade(){p.render();});let weak=Rc::downgrade(&page);language.connect_changed(move|_|if let Some(p)=weak.upgrade(){p.render();});let weak=Rc::downgrade(&page);ignore_case.connect_toggled(move|_|if let Some(p)=weak.upgrade(){p.render();});
            let p=page.clone();reload.connect_clicked(move|_|p.load());out.install(&panel.widget,"Dictionary and synonyms",true);page.load();
        });
    }
}
#[cfg(test)]
#[path = "dictionary_tests.rs"]
mod tests;
