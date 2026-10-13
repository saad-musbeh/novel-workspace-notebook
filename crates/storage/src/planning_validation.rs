//! Compatibility reads and strict *new author actions* are separate. A later
//! canon change must never rewrite or invalidate an unchanged frozen claim.
use serde_json::Value;
use std::collections::{HashMap, HashSet};

const MAX_ENTRIES: usize = 10_000;
const MAX_TEXT: usize = 100_000;
const MAX_SAFE_POSITION: u64 = 9_007_199_254_740_991;

fn rows<'a>(value: &'a Value, key: &str) -> &'a [Value] { value[key].as_array().map(Vec::as_slice).unwrap_or(&[]) }
fn id(value: &Value) -> Option<&str> { value["id"].as_str() }
fn valid_id(value: &Value) -> bool { value.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 200) }
fn text(value: &Value) -> bool { value.as_str().is_some_and(|s| s.chars().count() <= MAX_TEXT) }
fn nullable_id(value: &Value) -> bool { value.is_null() || valid_id(value) }
fn expect(condition: bool, message: &str) -> Result<(), String> { if condition { Ok(()) } else { Err(message.into()) } }
fn strings(value: &Value) -> bool {
    value.as_array().is_some_and(|values| values.len() <= MAX_ENTRIES && values.iter().all(valid_id) && values.iter().filter_map(Value::as_str).collect::<HashSet<_>>().len() == values.len())
}
fn present(value: &Value, key: &str, check: impl FnOnce(&Value) -> bool, message: &str) -> Result<(), String> {
    if let Some(value) = value.get(key) { expect(check(value), message)?; } Ok(())
}

/// Historical native/storage documents may omit fields that the retained
/// frontend required. Preserve those omissions and all frozen text; new author
/// actions are checked by `validate_transition`, never backfilled on a read.
pub fn validate_compatibility(document: &Value) -> Result<(), String> {
    let mut seen = HashSet::new();
    for key in ["records", "fields", "facts", "events", "plotPoints", "lanes", "boards", "constraints", "issues"] {
        for row in rows(document, key) { if let Some(id) = id(row) { seen.insert(id); } }
    }
    for key in ["readerReveals", "knowledge"] {
        let Some(array) = document.get(key) else { continue; };
        let array = array.as_array().ok_or_else(|| format!("Invalid planning {key} collection. Stored data was preserved."))?;
        expect(array.len() <= MAX_ENTRIES, "Planning reveals and knowledge are limited to 10,000 entries each.")?;
        for entry in array {
            expect(entry.is_object(), "Planning reveals and knowledge require object records.")?;
            if let Some(value) = entry.get("id") { expect(valid_id(value) && seen.insert(value.as_str().unwrap_or("")), "Planning reveal and knowledge IDs must be unique and stable.")?; }
            if key == "readerReveals" {
                for key in ["factId", "eventId"] { present(entry, key, nullable_id, "Invalid reader-reveal target identity.")?; }
                present(entry, "position", |v| v.as_u64().is_some_and(|p| p <= MAX_SAFE_POSITION), "Reader reveal position must be a nonnegative safe integer.")?;
                present(entry, "notes", text, "Invalid reader-reveal notes.")?;
                if let Some(scene) = entry.get("scene") {
                    expect(scene.is_object(), "Invalid reader-reveal scene link.")?;
                    present(scene, "kind", |v| v == "scene", "A reader reveal needs an explicit scene link.")?;
                    present(scene, "id", valid_id, "Invalid reader-reveal scene identity.")?;
                }
            } else {
                for key in ["characterId", "factId"] { present(entry, key, valid_id, "Invalid authored knowledge identity.")?; }
                present(entry, "stance", |v| matches!(v.as_str(), Some("knows" | "believes" | "misunderstands")), "Invalid authored knowledge stance.")?;
                for key in ["source", "interpretation"] { present(entry, key, text, "Invalid authored knowledge text.")?; }
                if let Some(snapshot) = entry.get("factSnapshot") {
                    expect(snapshot.is_object(), "Knowledge fact snapshots must be objects.")?;
                    for key in ["subjectId", "property", "value"] { present(snapshot, key, text, "Invalid frozen knowledge fact text.")?; }
                }
                if let Some(scope) = entry.get("scope") {
                    expect(scope.is_object(), "Invalid authored knowledge scope.")?;
                    present(scope, "kind", |v| v == "scenes" || v == "events", "Invalid authored knowledge scope kind.")?;
                    if scope["kind"] == "scenes" { present(scope, "sceneIds", strings, "Invalid authored knowledge scene scope.")?; }
                    if scope["kind"] == "events" {
                        present(scope, "fromEventId", valid_id, "Invalid authored knowledge start event.")?;
                        present(scope, "untilEventId", nullable_id, "Invalid authored knowledge end event.")?;
                    }
                }
            }
        }
    } Ok(())
}

fn targets<'a>(document: &'a Value, key: &str) -> HashMap<&'a str, &'a Value> { rows(document, key).iter().filter_map(|r| Some((id(r)?, r))).collect() }
fn author_reveal(entry: &Value, facts: &HashMap<&str, &Value>, events: &HashMap<&str, &Value>) -> Result<(), String> {
    expect(valid_id(&entry["id"]) && text(&entry["notes"]), "A new reader reveal needs a stable identity and notes.")?;
    expect(entry["scene"]["kind"] == "scene" && valid_id(&entry["scene"]["id"]), "A new reader reveal needs an explicit scene.")?;
    expect(entry["position"].as_u64().is_some_and(|p| p <= MAX_SAFE_POSITION), "A new reader reveal needs a nonnegative safe position.")?;
    let fact = entry["factId"].as_str(); let event = entry["eventId"].as_str();
    expect((entry.get("eventId").is_some_and(Value::is_null) && fact.is_some_and(|id| facts.contains_key(id))) || (entry.get("factId").is_some_and(Value::is_null) && event.is_some_and(|id| events.contains_key(id))), "A new reader reveal needs exactly one retained canonical fact or event.")
}
fn author_knowledge(document: &Value, entry: &Value, facts: &HashMap<&str, &Value>, events: &HashMap<&str, &Value>) -> Result<(), String> {
    expect(valid_id(&entry["id"]), "New knowledge needs a stable identity.")?;
    expect(entry["characterId"].as_str().is_some_and(|id| rows(document, "records").iter().any(|r| r["id"] == id && r["kind"] == "character")), "New knowledge needs a retained authored character.")?;
    expect(matches!(entry["stance"].as_str(), Some("knows" | "believes" | "misunderstands")), "New knowledge needs a supported stance.")?;
    expect(text(&entry["source"]) && entry["source"].as_str().is_some_and(|s| !s.trim().is_empty()), "New knowledge needs a nonempty author source.")?;
    expect(text(&entry["interpretation"]) && (entry["stance"] == "knows" || entry["interpretation"].as_str().is_some_and(|s| !s.trim().is_empty())), "Beliefs and misunderstandings need the character's authored interpretation.")?;
    let fact = entry["factId"].as_str().and_then(|id| facts.get(id)).copied().ok_or("New knowledge needs a retained canonical fact.")?;
    expect(valid_id(&fact["subjectId"]) && text(&fact["property"]) && text(&fact["value"]), "The referenced fact has invalid exact text.")?;
    expect(entry["stance"] != "knows" || fact["confirmed"] == true && fact["status"] == "fact", "Knowing requires an explicitly approved canonical fact.")?;
    expect(entry["factSnapshot"].is_object() && ["subjectId", "property", "value"].iter().all(|key| entry["factSnapshot"][*key] == fact[*key]), "New or reauthored knowledge must freeze the exact current fact. Historical snapshots were preserved.")?;
    let scope = &entry["scope"];
    if scope["kind"] == "scenes" { expect(strings(&scope["sceneIds"]) && !rows(scope, "sceneIds").is_empty(), "New knowledge needs at least one explicit, unique scene.")?; }
    else if scope["kind"] == "events" {
        expect(scope["fromEventId"].as_str().is_some_and(|id| events.contains_key(id)), "New knowledge needs a retained start event.")?;
        expect(scope.get("untilEventId").is_some_and(Value::is_null) || scope["untilEventId"].as_str().is_some_and(|id| events.contains_key(id) && scope["fromEventId"] != id), "New knowledge needs a distinct retained end event or an open end.")?;
    } else { return Err("New knowledge needs an explicit scene or event scope.".into()); }
    Ok(())
}

/// Ordinary author saves use their captured previous payload. Exact rows from
/// an acknowledged conflict head can be retained by `validate_resolution`.
pub fn validate_transition(previous: Option<&Value>, next: &Value) -> Result<(), String> {
    validate_resolution(&previous.into_iter().collect::<Vec<_>>(), next)
}
pub fn validate_resolution(previous: &[&Value], next: &Value) -> Result<(), String> {
    validate_compatibility(next)?;
    // Selecting an exact acknowledged historical version is recovery, not a
    // new author claim. It may already contain stale or missing targets.
    if previous.iter().any(|old| *old == next) { return Ok(()); }
    let facts = targets(next, "facts"); let events = targets(next, "events");
    for key in ["readerReveals", "knowledge"] {
        for entry in rows(next, key) {
            let unchanged = previous.iter().any(|old| rows(old, key).iter().any(|old_entry| id(old_entry).is_some() && id(old_entry) == id(entry) && old_entry == entry));
            if unchanged { continue; }
            if key == "readerReveals" { author_reveal(entry, &facts, &events)?; }
            else { author_knowledge(next, entry, &facts, &events)?; }
        }
    }
    // An unchanged stale claim is legitimate. Removing a currently retained
    // target while its dependent links remain is a separate destructive author
    // choice, and must be resolved explicitly before the ordinary save.
    for old in previous { guard_deletions(old, next)?; }
    Ok(())
}

fn guard_deletions(old: &Value, next: &Value) -> Result<(), String> {
    let removed = |key: &str| { let current: HashSet<_> = rows(next, key).iter().filter_map(id).collect(); rows(old, key).iter().filter_map(id).filter(|id| !current.contains(id)).collect::<HashSet<_>>() };
    let facts = removed("facts"); let events = removed("events"); let points = removed("plotPoints");
    if facts.is_empty() && events.is_empty() && points.is_empty() { return Ok(()); }
    fn visit(value: &Value, facts: &HashSet<&str>, events: &HashSet<&str>, points: &HashSet<&str>, depth: usize) -> Result<(), String> {
        expect(depth < 64, "Planning dependencies are too deeply nested.")?;
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    // A frozen snapshot is evidence, never a live dependency.
                    if key == "factSnapshot" { continue; }
                    let target = value.as_str();
                    expect(!(key == "factId" && target.is_some_and(|id| facts.contains(id)) || ["eventId", "fromEventId", "untilEventId", "beforeId", "afterId"].contains(&key.as_str()) && target.is_some_and(|id| events.contains(id)) || key == "payoffId" && target.is_some_and(|id| points.contains(id))), "Resolve retained planning dependencies before deleting their fact, event or payoff target.")?;
                    visit(value, facts, events, points, depth + 1)?;
                }
                if let Some(id) = map.get("id").and_then(Value::as_str) {
                    expect(!(map.get("kind").is_some_and(|v| v == "event") && events.contains(id) || map.get("kind").is_some_and(|v| v == "plot-point") && points.contains(id)), "Resolve retained authored links before deleting their event or plot target.")?;
                }
            },
            Value::Array(array) => for value in array { visit(value, facts, events, points, depth + 1)?; },
            _ => {},
        } Ok(())
    }
    visit(next, &facts, &events, &points, 0)
}
