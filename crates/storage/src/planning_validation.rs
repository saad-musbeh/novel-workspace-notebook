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
    crate::manuscript_parts::validate_planning_parts(document)?;
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

const CORE_KEYS: [&str; 9] = [
    "records",
    "fields",
    "facts",
    "events",
    "plotPoints",
    "lanes",
    "boards",
    "constraints",
    "issues",
];
const CORE_LIMIT: usize = 20_000;
#[derive(Clone, Copy)]
struct OwnedRow<'a> {
    key: &'static str,
    owner: Option<&'a Value>,
    value: &'a Value,
}
impl OwnedRow<'_> {
    fn same_owner(&self, other: &Self) -> bool {
        self.key == other.key && self.owner == other.owner
    }
    fn index_key(&self) -> String {
        id(self.value)
            .map(|id| format!("id:{id}"))
            .unwrap_or_else(|| format!("anonymous:{}", self.value))
    }
}
fn owned_rows(document: &Value) -> Vec<OwnedRow<'_>> {
    let mut result = Vec::new();
    for key in CORE_KEYS.into_iter().chain(["readerReveals", "knowledge"]) {
        result.extend(rows(document, key).iter().map(|value| OwnedRow {
            key,
            owner: None,
            value,
        }));
    }
    for board in rows(document, "boards") {
        for key in ["cards", "groups", "edges"] {
            result.extend(rows(board, key).iter().map(|value| OwnedRow {
                key,
                owner: Some(&board["id"]),
                value,
            }));
        }
    }
    for issue in rows(document, "issues") {
        result.extend(rows(issue, "evidence").iter().map(|value| OwnedRow {
            key: "evidence",
            owner: Some(&issue["id"]),
            value,
        }));
    }
    result
}
struct Prior<'a> {
    fields: HashMap<&'a str, &'a Value>,
    index: HashMap<String, Vec<OwnedRow<'a>>>,
}
impl<'a> Prior<'a> {
    fn new(document: &'a Value) -> Self {
        let mut index: HashMap<String, Vec<OwnedRow<'a>>> = HashMap::new();
        for row in owned_rows(document) {
            index.entry(row.index_key()).or_default().push(row);
        }
        Self {
            fields: targets(document, "fields"),
            index,
        }
    }
    fn matching(&self, row: OwnedRow<'a>) -> Vec<&'a Value> {
        self.index
            .get(&row.index_key())
            .into_iter()
            .flatten()
            .filter(|old| old.same_owner(&row))
            .map(|old| old.value)
            .collect()
    }
}
fn row_bag(rows: &[OwnedRow<'_>]) -> HashMap<String, usize> {
    let mut bag = HashMap::new();
    for row in rows {
        // Equality is exact JSON/Unicode equality, never normalized prose or IDs.
        let key = serde_json::json!([row.key, row.owner, row.value]).to_string();
        *bag.entry(key).or_default() += 1;
    }
    bag
}
fn text_list(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|a| a.len() <= CORE_LIMIT && a.iter().all(text))
}
fn links(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|a| a.len() <= CORE_LIMIT && a.iter().all(link))
}
fn link(value: &Value) -> bool {
    value.is_object()
        && matches!(
            value["kind"].as_str(),
            Some("record" | "scene" | "event" | "plot-point")
        )
        && valid_id(&value["id"])
}
fn finite(value: &Value) -> bool {
    value.as_f64().is_some_and(f64::is_finite)
}
fn variant(value: &Value, variants: &[&str]) -> bool {
    value.as_str().is_some_and(|v| variants.contains(&v))
}
fn member(value: &Value, members: &HashMap<&str, &Value>) -> bool {
    valid_id(value) && value.as_str().is_some_and(|id| members.contains_key(id))
}
fn array(value: &Value) -> bool {
    value.as_array().is_some_and(|a| a.len() <= CORE_LIMIT)
}
/// Sparse native payloads may omit properties. Supplied new/changed properties
/// must obey the retained model; unrelated edits do not reauthor stale evidence.
fn author_property(
    row: &Value,
    old: &[&Value],
    key: &str,
    check: impl FnOnce(&Value) -> bool,
) -> Result<(), String> {
    if old.iter().any(|old| old.get(key) == row.get(key)) {
        return Ok(());
    }
    match row.get(key) {
        Some(value) => expect(
            check(value),
            &format!("Invalid newly authored planning {key}."),
        ),
        None => expect(
            !old.iter().any(|old| old.get(key).is_some()),
            &format!("Removing authored planning {key} requires a valid replacement."),
        ),
    }
}
struct Symbols<'a> {
    records: HashMap<&'a str, &'a Value>,
    events: HashMap<&'a str, &'a Value>,
    points: HashMap<&'a str, &'a Value>,
    lanes: HashMap<&'a str, &'a Value>,
    boards: HashMap<&'a str, (HashMap<&'a str, &'a Value>, HashMap<&'a str, &'a Value>)>,
}
impl<'a> Symbols<'a> {
    fn new(document: &'a Value) -> Self {
        Self {
            records: targets(document, "records"),
            events: targets(document, "events"),
            points: targets(document, "plotPoints"),
            lanes: targets(document, "lanes"),
            boards: rows(document, "boards")
                .iter()
                .filter_map(|board| {
                    Some((
                        id(board)?,
                        (targets(board, "cards"), targets(board, "groups")),
                    ))
                })
                .collect(),
        }
    }
}
fn author_core_row(
    symbols: &Symbols<'_>,
    owned: OwnedRow<'_>,
    old: &[&Value],
) -> Result<(), String> {
    let row = owned.value;
    let property = |key, check: fn(&Value) -> bool| author_property(row, old, key, check);
    let records = &symbols.records;
    let events = &symbols.events;
    let points = &symbols.points;
    let lanes = &symbols.lanes;
    match owned.key {
        "fields" => {
            author_property(row, old, "kind", |v| {
                variant(
                    v,
                    &[
                        "text",
                        "long-text",
                        "number",
                        "date",
                        "single-choice",
                        "multiple-choice",
                        "links",
                    ],
                )
            })?;
            author_property(row, old, "recordKind", |v| {
                variant(v, &["character", "location", "item"])
            })?;
            property("label", text)?;
            property("hidden", Value::is_boolean)?;
            property("options", text_list)?;
        }
        "records" => {
            author_property(row, old, "kind", |v| {
                variant(v, &["character", "location", "item"])
            })?;
            property("name", text)?;
            property("notes", text)?;
            property("aliases", text_list)?;
            property("links", links)?;
            property("values", Value::is_object)?;
        }
        "facts" => {
            for key in ["property", "value", "source"] {
                property(key, text)?;
            }
            author_property(row, old, "status", |v| {
                variant(v, &["fact", "belief", "rumor", "proposal"])
            })?;
            property("confirmed", Value::is_boolean)?;
            if !old.iter().any(|old| {
                old.get("status") == row.get("status")
                    && old.get("confirmed") == row.get("confirmed")
            }) {
                expect(
                    row.get("status").is_none()
                        || row["confirmed"] != true
                        || row["status"] == "fact",
                    "Only an explicitly approved fact may be confirmed canon.",
                )?;
            }
            author_property(row, old, "subjectId", |v| member(v, &records))?;
            author_property(row, old, "observerId", |v| {
                v.is_null() || member(v, &records)
            })?;
            for key in ["eventId", "untilEventId"] {
                author_property(row, old, key, |v| v.is_null() || member(v, &events))?;
            }
        }
        "plotPoints" => {
            for key in ["title", "notes"] {
                property(key, text)?;
            }
            property("links", links)?;
            for key in ["requiresPayoff", "scopeClosed"] {
                property(key, Value::is_boolean)?;
            }
            author_property(row, old, "payoffId", |v| v.is_null() || member(v, &points))?;
        }
        "lanes" => {
            property("name", text)?;
            author_property(row, old, "kind", |v| variant(v, &["main", "subplot"]))?;
        }
        "events" => {
            for key in ["title", "notes"] {
                property(key, text)?;
            }
            author_property(row, old, "laneIds", |v| {
                v.as_array().is_some_and(|a| {
                    a.len() <= CORE_LIMIT
                        && a.iter().all(|id| member(id, &lanes))
                        && a.iter()
                            .filter_map(Value::as_str)
                            .collect::<HashSet<_>>()
                            .len()
                            == a.len()
                })
            })?;
            author_property(row, old, "sceneIds", |v| {
                v.as_array()
                    .is_some_and(|a| a.len() <= CORE_LIMIT && a.iter().all(valid_id))
            })?;
            // Root's separate fictional-time transition guard below stays intact.
        }
        "constraints" => {
            for key in ["beforeId", "afterId"] {
                author_property(row, old, key, |v| member(v, &events))?;
            }
            author_property(row, old, "minimumDays", |v| {
                finite(v) && v.as_f64().is_some_and(|v| v >= 0.)
            })?;
            for key in ["strict", "confirmed"] {
                property(key, Value::is_boolean)?;
            }
            property("notes", text)?;
        }
        "boards" => {
            property("name", text)?;
            for key in ["cards", "groups", "edges"] {
                property(key, array)?;
            }
        }
        "groups" => {
            property("name", text)?;
        }
        "cards" => {
            for key in ["title", "body"] {
                property(key, text)?;
            }
            author_property(row, old, "kind", |v| {
                variant(v, &["plot-point", "note", "record", "scene", "event"])
            })?;
            for key in ["x", "y"] {
                property(key, finite)?;
            }
            author_property(row, old, "reference", |v| v.is_null() || link(v))?;
            property("labels", text_list)?;
            let groups = owned
                .owner
                .and_then(Value::as_str)
                .and_then(|id| symbols.boards.get(id))
                .map(|(_, groups)| groups);
            author_property(row, old, "groupId", |v| {
                v.is_null() || groups.is_some_and(|groups| member(v, groups))
            })?;
        }
        "edges" => {
            let cards = owned
                .owner
                .and_then(Value::as_str)
                .and_then(|id| symbols.boards.get(id))
                .map(|(cards, _)| cards);
            for key in ["fromId", "toId"] {
                author_property(row, old, key, |v| {
                    cards.is_some_and(|cards| member(v, cards))
                })?;
            }
            author_property(row, old, "kind", |v| {
                variant(
                    v,
                    &[
                        "reference",
                        "causes",
                        "happens-before",
                        "reveals",
                        "depends-on",
                        "setup/payoff",
                        "custom",
                    ],
                )
            })?;
            property("label", text)?;
            property("confirmed", Value::is_boolean)?;
        }
        "issues" => {
            for key in ["category", "description", "resolution"] {
                property(key, text)?;
            }
            author_property(row, old, "importance", |v| {
                variant(v, &["low", "medium", "high"])
            })?;
            author_property(row, old, "status", |v| {
                variant(
                    v,
                    &[
                        "open",
                        "investigating",
                        "intentional",
                        "resolved",
                        "dismissed",
                        "stale",
                    ],
                )
            })?;
            property("links", links)?;
            property("evidence", array)?;
        }
        "evidence" => {
            author_property(row, old, "link", |v| v.is_null() || link(v))?;
            for key in ["quote", "sourceRevision", "notes"] {
                property(key, text)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn author_custom_values(next: &Value, previous: &[Prior<'_>]) -> Result<(), String> {
    let fields = targets(next, "fields");
    for record in rows(next, "records") {
        let Some(values) = record["values"].as_object() else {
            continue;
        };
        for (field_id, value) in values {
            let field = fields.get(field_id.as_str()).copied();
            let owned = OwnedRow {
                key: "records",
                owner: None,
                value: record,
            };
            let unchanged = previous.iter().any(|prior| {
                let old_field = prior.fields.get(field_id.as_str()).copied();
                let same_definition = match (old_field, field) {
                    (None, None) => true,
                    (Some(a), Some(b)) => {
                        a.get("kind") == b.get("kind") && a.get("recordKind") == b.get("recordKind")
                    }
                    _ => false,
                };
                same_definition
                    && prior.matching(owned).iter().any(|old| {
                        old.get("kind") == record.get("kind")
                            && old["values"].get(field_id) == Some(value)
                    })
            });
            if unchanged {
                continue;
            }
            let field = field.filter(|f| f["recordKind"].as_str().is_some() && f["recordKind"] == record["kind"]).ok_or("Custom values require a retained field definition for this record kind. Resolve values before removing their field.")?;
            expect(
                valid_id(&field["id"]),
                "New custom values require a stable owned field identity.",
            )?;
            let valid = match field["kind"].as_str() {
                Some("number") => finite(value),
                Some("multiple-choice") => text_list(value),
                Some("links") => links(value),
                Some("text" | "long-text" | "date" | "single-choice") => text(value),
                _ => false,
            };
            expect(
                valid,
                "New custom field values must match their retained field type.",
            )?;
        }
    }
    Ok(())
}
fn author_core(previous: &[&Value], next: &Value) -> Result<(), String> {
    let previous: Vec<_> = previous.iter().map(|doc| Prior::new(doc)).collect();
    for key in CORE_KEYS {
        // Array omissions are valid sparse native data, not inferred empty rows.
        if let Some(value) = next.get(key) {
            expect(array(value), "Invalid authored planning collection.")?;
        }
    }
    let next_rows = owned_rows(next);
    let mut identities: HashMap<String, Vec<OwnedRow<'_>>> = HashMap::new();
    for row in &next_rows {
        identities.entry(row.index_key()).or_default().push(*row);
    }
    for (key, occurrences) in identities.iter().filter(|(_, rows)| rows.len() > 1) {
        let bag = row_bag(occurrences);
        expect(previous.iter().any(|prior| prior.index.get(key).is_some_and(|old| old.len() == occurrences.len() && row_bag(old) == bag)), "New planning IDs must be unique across canonical records, board identities and evidence. Historical collisions must not be increased or merged.")?;
    }
    let symbols = Symbols::new(next);
    for owned in next_rows {
        let old: Vec<_> = previous
            .iter()
            .flat_map(|prior| prior.matching(owned))
            .collect();
        if old.iter().any(|old| *old == owned.value) {
            continue;
        }
        expect(
            owned.value.is_object() && valid_id(&owned.value["id"]),
            "New planning rows require stable nonempty identities.",
        )?;
        author_core_row(&symbols, owned, &old)?;
    }
    author_custom_values(next, &previous)
}

/// Fictional days may be negative or fractional. Match the retained frontend
/// time variants; never infer a date or enforce feasibility on historical data.
fn author_event_time(entry: &Value, time: &Value, events: &HashMap<&str, &Value>) -> Result<(), String> {
    expect(valid_id(&entry["id"]) && time.is_object(), "New event time needs a stable event identity and an explicit time object.")?;
    present(time, "label", text, "Invalid authored fictional-time label.")?;
    let finite = |value: &Value| value.as_f64().is_some_and(f64::is_finite);
    let valid = match time["kind"].as_str() {
        Some("unknown") => true,
        Some("fixed") => finite(&time["day"]),
        Some("interval" | "bounds") => finite(&time["start"]) && finite(&time["end"]) && time["start"].as_f64() <= time["end"].as_f64(),
        Some("relative") => time["eventId"].as_str().is_some_and(|target| events.contains_key(target) && entry["id"] != target) && finite(&time["offsetDays"]),
        _ => false,
    };
    expect(valid, "New or changed fictional time needs finite dates, an ordered range, or a retained distinct relative event.")
}
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
    author_core(previous, next)?;
    let facts = targets(next, "facts"); let events = targets(next, "events");
    for entry in rows(next, "events") {
        // Older native rows may omit time entirely. The guard concerns new or
        // changed explicit time, not unrelated edits to preserved legacy rows.
        let Some(time) = entry.get("time") else { continue; };
        let unchanged = previous.iter().any(|old| rows(old, "events").iter().any(|old_entry| id(old_entry).is_some() && id(old_entry) == id(entry) && old_entry.get("time") == Some(time)));
        if !unchanged { author_event_time(entry, time, &events)?; }
    }
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
