//! Typed custom fields. Reading a historical value never converts or removes it.
use super::*;
use std::collections::HashSet;

const TEXT_LIMIT: usize = 100_000;
const ROW_LIMIT: usize = 10_000;
const KINDS: &[&str] = &[
    "text",
    "long-text",
    "number",
    "date",
    "single-choice",
    "multiple-choice",
    "links",
];
const RECORD_KINDS: &[&str] = &["character", "location", "item"];
const LINK_KINDS: &[&str] = &["record", "scene", "event", "plot-point"];

fn rows<'a>(doc: &'a Value, key: &str) -> &'a [Value] {
    doc[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn find<'a>(doc: &'a Value, key: &str, id: &str) -> Option<&'a Value> {
    rows(doc, key).iter().find(|r| r["id"] == id)
}
fn valid_text(v: &Value) -> bool {
    v.as_str().is_some_and(|s| s.chars().count() <= TEXT_LIMIT)
}
fn valid_id(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 200)
}
fn shape(kind: &str, value: &Value) -> bool {
    match kind {
        "text" | "long-text" | "date" | "single-choice" => valid_text(value),
        "number" => value.is_number() && value.as_f64().is_some_and(f64::is_finite),
        "multiple-choice" => value
            .as_array()
            .is_some_and(|a| a.len() <= ROW_LIMIT && a.iter().all(valid_text)),
        "links" => value.as_array().is_some_and(|a| {
            a.len() <= ROW_LIMIT
                && a.iter().all(|link| {
                    link.is_object()
                        && LINK_KINDS.contains(&text(link, "kind"))
                        && valid_id(&link["id"])
                })
        }),
        _ => false,
    }
}
fn definition(field: &Value) -> Result<(), String> {
    if !valid_id(&field["id"])
        || !valid_text(&field["label"])
        || text(field, "label").trim().is_empty()
        || !KINDS.contains(&text(field, "kind"))
        || !RECORD_KINDS.contains(&text(field, "recordKind"))
        || !field["hidden"].is_boolean()
        || !field["options"]
            .as_array()
            .is_some_and(|a| a.len() <= ROW_LIMIT && a.iter().all(valid_text))
    {
        return Err("A field needs a stable identity, label, supported type/record kind, hide flag and text options.".into());
    }
    Ok(())
}
fn used(doc: &Value, id: &str) -> bool {
    rows(doc, "records")
        .iter()
        .any(|r| r["values"].get(id).is_some())
}

/// Move among the same record kind, keeping unrelated definitions and all values exact.
fn move_field(doc: &Value, id: &str, step: isize) -> Result<Value, String> {
    if ![-1, 1].contains(&step) {
        return Err("Move one field up or down.".into());
    }
    let mut next = doc.clone();
    let fields = next["fields"]
        .as_array_mut()
        .ok_or("Unsupported field collection.")?;
    let index = fields
        .iter()
        .position(|f| f["id"] == id)
        .ok_or("The selected field is no longer available.")?;
    let group: Vec<usize> = fields
        .iter()
        .enumerate()
        .filter_map(|(i, f)| (f["recordKind"] == fields[index]["recordKind"]).then_some(i))
        .collect();
    let position = group.iter().position(|i| *i == index).unwrap();
    if let Some(target) = position.checked_add_signed(step).and_then(|p| group.get(p)) {
        fields.swap(index, *target);
    }
    Ok(next)
}
fn edit_field(doc: &Value, id: &str, key: &str, value: Value) -> Result<Value, String> {
    if !["label", "kind", "recordKind", "hidden", "options"].contains(&key) {
        return Err("Unsupported field edit.".into());
    }
    let old = find(doc, "fields", id).ok_or("The field is no longer available.")?;
    let mut candidate = old.clone();
    candidate[key] = value;
    if key == "kind" && candidate["kind"] != old["kind"] {
        if !KINDS.contains(&text(&candidate, "kind"))
            || rows(doc, "records")
                .iter()
                .filter_map(|r| r["values"].get(id))
                .any(|v| !shape(text(&candidate, "kind"), v))
        {
            return Err("This type would reinterpret stored values. Retain the field type or add a separate field.".into());
        }
    }
    if key == "recordKind" && candidate["recordKind"] != old["recordKind"] && used(doc, id) {
        return Err("A used field must retain its record kind. Its hidden and visible values are preserved.".into());
    }
    let mut next = doc.clone();
    let fields = next["fields"]
        .as_array_mut()
        .ok_or("Unsupported field collection.")?;
    let row = fields
        .iter_mut()
        .find(|f| f["id"] == id)
        .ok_or("The field is no longer available.")?;
    *row = candidate;
    Ok(next)
}
fn remove_field(doc: &Value, id: &str) -> Result<Value, String> {
    if used(doc, id) {
        return Err("This field has stored values. Hide it to preserve them; the definition cannot be removed here.".into());
    }
    if find(doc, "fields", id).is_none() {
        return Err("The field is no longer available.".into());
    }
    let mut next = doc.clone();
    next["fields"]
        .as_array_mut()
        .ok_or("Unsupported field collection.")?
        .retain(|f| f["id"] != id);
    Ok(next)
}
fn number_candidate(input: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(input)
        .map_err(|_| "Enter a finite JSON number, without rounding or a range clamp.".to_owned())?;
    if shape("number", &value) {
        Ok(value)
    } else {
        Err("Enter a finite JSON number, without rounding or a range clamp.".into())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Target {
    kind: String,
    id: String,
    label: String,
}
fn targets(doc: &Value, scenes: &[Value], kind: &str) -> Vec<Target> {
    let (source, label) = match kind {
        "record" => (rows(doc, "records"), "name"),
        "scene" => (scenes, "title"),
        "event" => (rows(doc, "events"), "title"),
        "plot-point" => (rows(doc, "plotPoints"), "title"),
        _ => return Vec::new(),
    };
    source
        .iter()
        .filter(|r| {
            valid_id(&r["id"]) && (kind != "record" || RECORD_KINDS.contains(&text(r, "kind")))
        })
        .map(|r| Target {
            kind: kind.into(),
            id: text(r, "id").into(),
            label: text(r, label).into(),
        })
        .collect()
}
fn has_target(doc: &Value, scenes: &[Value], link: &Value) -> bool {
    let kind = text(link, "kind");
    let source = match kind {
        "record" => rows(doc, "records"),
        "scene" => scenes,
        "event" => rows(doc, "events"),
        "plot-point" => rows(doc, "plotPoints"),
        _ => return false,
    };
    source.iter().any(|row| {
        row["id"] == link["id"]
            && valid_id(&row["id"])
            && (kind != "record" || RECORD_KINDS.contains(&text(row, "kind")))
    })
}
fn value_candidate(
    field: &Value,
    old: Option<&Value>,
    value: &Value,
    doc: &Value,
    scenes: &[Value],
) -> Result<(), String> {
    if !shape(text(field, "kind"), value) {
        return Err("This value does not match its declared custom field type. Historical values stay exact until explicitly edited.".into());
    }
    let options = rows(field, "options");
    match text(field, "kind") {
        "single-choice" => {
            if old != Some(value) && !options.contains(value) {
                return Err("Choose a currently declared option. Unavailable historical choices remain retained.".into());
            }
        }
        "multiple-choice" => {
            for option in value.as_array().unwrap() {
                if !options.contains(option)
                    && !old
                        .and_then(Value::as_array)
                        .is_some_and(|a| a.contains(option))
                {
                    return Err("New choices must use currently declared options.".into());
                }
            }
        }
        "links" => {
            for link in value.as_array().unwrap() {
                if !old
                    .and_then(Value::as_array)
                    .is_some_and(|a| a.contains(link))
                    && !has_target(doc, scenes, link)
                {
                    return Err("Choose a retained canonical target of the selected link kind. Missing historical links remain retained.".into());
                }
            }
        }
        _ => {}
    }
    Ok(())
}
fn set_value(
    doc: &Value,
    record: &str,
    field: &str,
    value: Value,
    scenes: &[Value],
) -> Result<Value, String> {
    let definition =
        find(doc, "fields", field).ok_or("The field definition changed or is unavailable.")?;
    let row = find(doc, "records", record).ok_or("The record is no longer available.")?;
    if definition["recordKind"] != row["kind"] || definition["hidden"] == true {
        return Err("This field is hidden or belongs to another record kind. Refresh custom fields before editing.".into());
    }
    value_candidate(definition, row["values"].get(field), &value, doc, scenes)?;
    set_draft_value(doc, record, field, value)
}
fn set_draft_value(doc: &Value, record: &str, field: &str, value: Value) -> Result<Value, String> {
    let definition = find(doc, "fields", field).ok_or("The field definition is unavailable.")?;
    let source = find(doc, "records", record).ok_or("The record is no longer available.")?;
    if definition["hidden"] == true || definition["recordKind"] != source["kind"] {
        return Err(
            "The field is hidden or belongs to another record kind. Its value stays exact.".into(),
        );
    }
    let mut next = doc.clone();
    let row = next["records"]
        .as_array_mut()
        .and_then(|a| a.iter_mut().find(|r| r["id"] == record))
        .ok_or("The record is no longer available.")?;
    let values = row["values"]
        .as_object_mut()
        .ok_or("Unsupported historical values map. Its bytes are preserved.")?;
    values.insert(field.into(), value);
    Ok(next)
}
fn link_candidate(
    doc: &Value,
    record: &str,
    field: &str,
    expected: &Value,
    index: Option<usize>,
    replacement: Option<Value>,
    scenes: &[Value],
) -> Result<Value, String> {
    let row = find(doc, "records", record).ok_or("The record is no longer available.")?;
    if &row["values"][field] != expected {
        return Err(
            "These link rows changed. Refresh custom fields before editing this selection.".into(),
        );
    }
    let mut links = if expected.is_null() {
        Vec::new()
    } else {
        expected
            .as_array()
            .cloned()
            .ok_or("Historical links have an unsupported shape; their exact value is retained.")?
    };
    match (index, replacement) {
        (None, Some(link)) => links.push(link),
        (Some(index), Some(link)) if index < links.len() => links[index] = link,
        (Some(index), None) if index < links.len() => {
            links.remove(index);
        }
        _ => return Err("Select a retained link row or choose a new target.".into()),
    }
    set_value(doc, record, field, json!(links), scenes)
}

/// UX guards for custom fields only; the companion owns all planning core guards.
pub(super) fn validate_transition(
    previous: &Value,
    next: &Value,
    scenes: &[Value],
) -> Result<Vec<Value>, String> {
    let mut dependencies = Vec::new();
    let mut seen_scenes = HashSet::new();
    for old in rows(previous, "fields") {
        if find(next, "fields", text(old, "id")).is_none() && used(next, text(old, "id")) {
            return Err(
                "A field definition with retained values cannot be removed. Hide it instead."
                    .into(),
            );
        }
    }
    for field in rows(next, "fields") {
        let old = find(previous, "fields", text(field, "id"));
        if old != Some(field) {
            definition(field)?;
            if old.is_some_and(|old| old["recordKind"] != field["recordKind"])
                && used(next, text(field, "id"))
            {
                return Err("Used fields must retain their record kind.".into());
            }
            if old.is_none_or(|old| old["kind"] != field["kind"])
                && rows(next, "records")
                    .iter()
                    .filter_map(|r| r["values"].get(text(field, "id")))
                    .any(|v| !shape(text(field, "kind"), v))
            {
                return Err("The field type is incompatible with retained values.".into());
            }
        }
    }
    for record in rows(next, "records") {
        let old = find(previous, "records", text(record, "id"));
        if let Some(values) = record["values"].as_object() {
            for (id, value) in values {
                let prior = old.and_then(|r| r["values"].get(id));
                if prior == Some(value) {
                    continue;
                }
                let field = find(next, "fields", id)
                    .ok_or("Changed values require their retained field definition.")?;
                if field["recordKind"] != record["kind"] {
                    return Err("Changed values must use a field of the record's kind.".into());
                }
                value_candidate(field, prior, value, next, scenes)?;
                if field["kind"] == "links" {
                    for link in value.as_array().unwrap().iter().filter(|link| {
                        link["kind"] == "scene"
                            && !prior
                                .and_then(Value::as_array)
                                .is_some_and(|a| a.contains(link))
                    }) {
                        let scene = scenes
                            .iter()
                            .find(|s| s["id"] == link["id"])
                            .ok_or("The selected scene is unavailable in the checked catalog.")?;
                        if !valid_id(&scene["revisionId"]) {
                            return Err("The selected scene has no checked saved revision.".into());
                        }
                        if seen_scenes.insert(text(scene, "id")) {
                            dependencies.push(json!({"entityId":scene["id"],"entityType":"scene","revisionId":scene["revisionId"]}));
                        }
                    }
                }
            }
        }
    }
    if dependencies.len() > 50 {
        return Err("One Save can introduce links to at most 50 checked scene sources.".into());
    }
    Ok(dependencies)
}

fn caption(body: &gtk::Box, name: &str) {
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.);
    label.set_line_wrap(true);
    body.pack_start(&label, false, false, 0);
}
fn preview(value: &str) -> String {
    let mut chars = value.chars();
    let mut out: String = chars.by_ref().take(300).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    out
}
fn status() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.);
    label.set_line_wrap(true);
    label
}
fn apply(form: &Form, candidate: Result<Value, String>, status: &gtk::Label) -> bool {
    match candidate {
        Ok(next) => {
            if next != *form.value.borrow() {
                form.change(&[], next);
            }
            status.set_text("Unsaved custom field changes · Manual Save required");
            true
        }
        Err(e) => {
            status.set_text(&e);
            false
        }
    }
}
fn choice_combo(options: &[&str], current: &str) -> gtk::ComboBoxText {
    let combo = gtk::ComboBoxText::new();
    for option in options {
        combo.append(Some(option), option);
    }
    if !options.contains(&current) {
        combo.append(
            Some(current),
            &format!("Unavailable historical type: {}", preview(current)),
        );
    }
    combo.set_active_id(Some(current));
    combo
}

pub(super) fn definitions(form: Form) -> gtk::Widget {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 5);
    let selected = gtk::ComboBoxText::new();
    selected.set_widget_name("custom-field-selector");
    for field in rows(&form.value.borrow(), "fields") {
        selected.append(Some(text(field, "id")), text(field, "label"));
    }
    body.pack_start(&selected, false, false, 0);
    let inspector = gtk::Box::new(gtk::Orientation::Vertical, 5);
    body.pack_start(&inspector, false, false, 0);
    let note = status();
    body.pack_start(&note, false, false, 0);
    let f = form.clone();
    let holder = inspector.clone();
    selected.connect_changed(move |c| {
        for child in holder.children() {
            holder.remove(&child);
        }
        if let Some(id) = c.active_id() {
            holder.pack_start(&definition_form(f.clone(), id.to_string()), false, false, 0);
            holder.show_all();
        }
    });
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    body.pack_start(&actions, false, false, 0);
    let f = form.clone();
    let c = selected.clone();
    let n = note.clone();
    let add = gtk::Button::with_label("Add field");
    actions.pack_start(&add, false, false, 0);
    add.connect_clicked(move |_| {
        let mut next = f.value.borrow().clone(); let Some(fields) = next["fields"].as_array_mut() else { n.set_text("Unsupported field collection; retained."); return; };
        if fields.len() >= ROW_LIMIT { n.set_text("This page already contains 10,000 fields."); return; }
        let id = uuid::Uuid::new_v4().to_string(); fields.push(json!({"id":id,"label":"New field","kind":"text","recordKind":"character","hidden":false,"options":[]}));
        if apply(&f, Ok(next), &n) { refresh_definitions(&c, &f, Some(&id)); }
    });
    for (label, step) in [("Move field up", -1), ("Move field down", 1)] {
        let f = form.clone();
        let c = selected.clone();
        let n = note.clone();
        let button = gtk::Button::with_label(label);
        actions.pack_start(&button, false, false, 0);
        button.connect_clicked(move |_| {
            let Some(id) = c.active_id() else {
                return;
            };
            let candidate = move_field(&f.value.borrow(), &id, step);
            if apply(&f, candidate, &n) {
                refresh_definitions(&c, &f, Some(&id));
            }
        });
    }
    let has_fields = !rows(&form.value.borrow(), "fields").is_empty();
    let f = form;
    let c = selected.clone();
    let n = note;
    let remove = gtk::Button::with_label("Remove unused field");
    actions.pack_start(&remove, false, false, 0);
    remove.connect_clicked(move |_| {
        let Some(id) = c.active_id() else {
            return;
        };
        let candidate = remove_field(&f.value.borrow(), &id);
        if apply(&f, candidate, &n) {
            refresh_definitions(&c, &f, None);
        }
    });
    if has_fields {
        selected.set_active(Some(0));
    }
    body.upcast()
}
fn refresh_definitions(combo: &gtk::ComboBoxText, form: &Form, id: Option<&str>) {
    combo.remove_all();
    for field in rows(&form.value.borrow(), "fields") {
        combo.append(Some(text(field, "id")), text(field, "label"));
    }
    combo.set_active_id(id);
}
fn definition_form(form: Form, id: String) -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 5);
    let field = find(&form.value.borrow(), "fields", &id)
        .cloned()
        .unwrap_or(Value::Null);
    let note = status();
    caption(&body, "Field label");
    let label = gtk::Entry::new();
    label.set_widget_name("custom-field-label");
    label.set_text(text(&field, "label"));
    body.pack_start(&label, false, false, 0);
    let f = form.clone();
    let i = id.clone();
    let n = note.clone();
    label.connect_changed(move |e| {
        let candidate = edit_field(&f.value.borrow(), &i, "label", json!(e.text().as_str()));
        apply(&f, candidate, &n);
    });
    for (name, key, options) in [
        ("Field type", "kind", KINDS),
        ("Applies to record kind", "recordKind", RECORD_KINDS),
    ] {
        caption(&body, name);
        let combo = choice_combo(options, text(&field, key));
        combo.set_widget_name(&format!("custom-field-{key}"));
        body.pack_start(&combo, false, false, 0);
        let f = form.clone();
        let i = id.clone();
        let n = note.clone();
        let changing = Cell::new(false);
        combo.connect_changed(move |c| {
            if changing.get() {
                return;
            }
            let Some(value) = c.active_id() else {
                return;
            };
            let previous = find(&f.value.borrow(), "fields", &i)
                .map(|r| text(r, key).to_owned())
                .unwrap_or_default();
            let candidate = edit_field(&f.value.borrow(), &i, key, json!(value.as_str()));
            if !apply(&f, candidate, &n) {
                changing.set(true);
                c.set_active_id(Some(&previous));
                changing.set(false);
            }
        });
    }
    let hidden = gtk::CheckButton::with_label("Hide field (retain all stored values)");
    hidden.set_active(field["hidden"] == true);
    body.pack_start(&hidden, false, false, 0);
    let f = form.clone();
    let i = id.clone();
    let n = note.clone();
    hidden.connect_toggled(move |c| {
        let candidate = edit_field(&f.value.borrow(), &i, "hidden", json!(c.is_active()));
        apply(&f, candidate, &n);
    });
    caption(&body, "Choice options (used by single / multiple choice)");
    body.pack_start(&option_editor(form, id), false, false, 0);
    body.pack_start(&note, false, false, 0);
    body
}
fn option_editor(form: Form, id: String) -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let options = gtk::ComboBoxText::new();
    options.set_widget_name("custom-option-selector");
    if let Some(field) = find(&form.value.borrow(), "fields", &id) {
        for (i, value) in rows(field, "options").iter().enumerate() {
            options.append(
                Some(&i.to_string()),
                &preview(value.as_str().unwrap_or("Unsupported historical option")),
            );
        }
    }
    body.pack_start(&options, false, false, 0);
    let entry = gtk::TextView::new();
    entry.set_widget_name("custom-option-text");
    entry.set_wrap_mode(gtk::WrapMode::WordChar);
    entry.set_size_request(-1, 70);
    entry.set_editable(false);
    body.pack_start(&entry, false, false, 0);
    let buffer = entry.buffer().unwrap();
    let changing = Rc::new(Cell::new(false));
    let f = form.clone();
    let i = id.clone();
    let b = buffer.clone();
    let view = entry.clone();
    let updating = changing.clone();
    options.connect_changed(move |c| {
        updating.set(true);
        let selected = c.active_id().and_then(|s| s.parse::<usize>().ok());
        let doc = f.value.borrow();
        let value = find(&doc, "fields", &i)
            .and_then(|field| selected.and_then(|index| rows(field, "options").get(index)))
            .and_then(Value::as_str);
        view.set_editable(value.is_some());
        b.set_text(value.unwrap_or(""));
        updating.set(false);
    });
    let note = status();
    let f = form.clone();
    let i = id.clone();
    let c = options.clone();
    let n = note.clone();
    let updating = changing.clone();
    buffer.connect_changed(move |b| {
        if updating.get() {
            return;
        }
        let Some(index) = c.active_id().and_then(|s| s.parse::<usize>().ok()) else {
            return;
        };
        let raw = b
            .text(&b.start_iter(), &b.end_iter(), true)
            .unwrap_or_default()
            .to_string();
        let doc = f.value.borrow();
        let Some(field) = find(&doc, "fields", &i) else {
            return;
        };
        let mut values = rows(field, "options").to_vec();
        if index >= values.len() {
            n.set_text("This option is no longer available. Select it again.");
            return;
        }
        values[index] = json!(raw);
        let candidate = edit_field(&doc, &i, "options", json!(values));
        drop(doc);
        apply(&f, candidate, &n);
    });
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    body.pack_start(&row, false, false, 0);
    for (label, action) in [("Add option", "add"), ("Remove option", "remove")] {
        let f = form.clone();
        let i = id.clone();
        let c = options.clone();
        let n = note.clone();
        let button = gtk::Button::with_label(label);
        row.pack_start(&button, false, false, 0);
        button.connect_clicked(move |_| {
            let doc = f.value.borrow(); let Some(field) = find(&doc, "fields", &i) else { return; };
            let mut values = rows(field, "options").to_vec(); let selected = c.active_id().and_then(|s| s.parse::<usize>().ok());
            let chosen = match action {
                "add" if values.len() < ROW_LIMIT => { values.push(json!("")); Some(values.len() - 1) },
                "remove" if selected.is_some_and(|s| s < values.len()) => { values.remove(selected.unwrap()); None },
                _ => { n.set_text("Select an option to remove, or use Add option."); return; }
            };
            let candidate = edit_field(&doc, &i, "options", json!(values)); drop(doc);
            if apply(&f, candidate, &n) {
                c.remove_all(); for (index, option) in values.iter().enumerate() { c.append(Some(&index.to_string()), &preview(option.as_str().unwrap_or("Unsupported historical option"))); }
                c.set_active(chosen.map(|n| n as u32));
                n.set_text("Option edits use manual page Save. Existing record choices are retained, including removed option text. Refresh custom fields to show new choices.");
            }
        });
    }
    caption(&body, "Add an option, then enter its exact text. Select a saved option to rename it. Neither action changes stored record choices.");
    body.pack_start(&note, false, false, 0);
    body
}

pub(super) fn values(form: Form, record: String) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 5);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 5);
    root.pack_start(&body, false, false, 0);
    let refresh = gtk::Button::with_label("Refresh custom fields");
    root.pack_start(&refresh, false, false, 0);
    let f = form.clone();
    let id = record.clone();
    let holder = body.clone();
    refresh.connect_clicked(move |_| {
        for child in holder.children() {
            holder.remove(&child);
        }
        populate_values(&holder, f.clone(), &id);
        holder.show_all();
    });
    populate_values(&body, form, &record);
    root.upcast()
}
fn populate_values(body: &gtk::Box, form: Form, record: &str) {
    let doc = form.value.borrow();
    let Some(row) = find(&doc, "records", record) else {
        caption(body, "Missing record; values retained.");
        return;
    };
    let fields = rows(&doc, "fields").to_vec();
    let kind = text(row, "kind").to_owned();
    drop(doc);
    for field in fields
        .iter()
        .filter(|f| f["recordKind"] == kind && f["hidden"] != true)
    {
        caption(body, text(field, "label"));
        let widget = value_editor(form.clone(), record.to_owned(), field.clone());
        body.pack_start(&widget, false, false, 0);
    }
    caption(body, "Hidden values remain saved. Refresh after definition edits; no value is inferred or filled automatically.");
}
fn value_editor(form: Form, record: String, field: Value) -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let id = text(&field, "id").to_owned();
    body.set_widget_name(&format!("custom-value-{id}"));
    let current = find(&form.value.borrow(), "records", &record)
        .and_then(|r| r["values"].get(&id))
        .cloned();
    let note = status();
    if let Some(value) = &current {
        if !shape(text(&field, "kind"), value) {
            caption(
                &body,
                &format!(
                    "Stored or pending value has an unavailable shape; retained exactly: {}",
                    preview(&value.to_string())
                ),
            );
        }
    }
    match text(&field, "kind") {
        "text" | "date" | "number" => {
            let entry = gtk::Entry::new();
            let raw = current
                .as_ref()
                .map(|v| {
                    if v.is_number() {
                        v.to_string()
                    } else {
                        v.as_str().unwrap_or("").into()
                    }
                })
                .unwrap_or_default();
            entry.set_text(&raw);
            body.pack_start(&entry, false, false, 0);
            if field["kind"] == "date" {
                caption(&body, "Authored date / relative-time text. No date or fictional calendar is inferred.");
            }
            let f = form.clone();
            let i = id.clone();
            let r = record.clone();
            let n = note.clone();
            let numeric = field["kind"] == "number";
            entry.connect_changed(move |e| {
                let raw = e.text().to_string();
                let value = if numeric {
                    number_candidate(&raw)
                } else {
                    Ok(json!(raw))
                };
                match value {
                    Ok(value) => {
                        let candidate =
                            set_value(&f.value.borrow(), &r, &i, value, &f.scenes.borrow());
                        apply(&f, candidate, &n);
                    }
                    Err(error) => {
                        let candidate = set_draft_value(&f.value.borrow(), &r, &i, json!(raw));
                        apply(&f, candidate, &n);
                        n.set_text(&format!(
                            "Incomplete number retained. Changed values must be corrected before Save. {error}"
                        ));
                    }
                }
            });
        }
        "long-text" => {
            let view = gtk::TextView::new();
            view.set_wrap_mode(gtk::WrapMode::WordChar);
            view.set_size_request(-1, 120);
            let buffer = view.buffer().unwrap();
            buffer.set_text(current.as_ref().and_then(Value::as_str).unwrap_or(""));
            body.pack_start(&view, false, false, 0);
            let f = form.clone();
            let i = id.clone();
            let r = record.clone();
            let n = note.clone();
            buffer.connect_changed(move |b| {
                let value = json!(b
                    .text(&b.start_iter(), &b.end_iter(), true)
                    .unwrap_or_default()
                    .as_str());
                let candidate = set_value(&f.value.borrow(), &r, &i, value, &f.scenes.borrow());
                apply(&f, candidate, &n);
            });
        }
        "single-choice" => {
            let combo = gtk::ComboBoxText::new();
            for (index, option) in rows(&field, "options").iter().enumerate() {
                combo.append(
                    Some(&index.to_string()),
                    &preview(option.as_str().unwrap_or("Unsupported historical option")),
                );
            }
            if let Some(value) = &current {
                if let Some(index) = rows(&field, "options")
                    .iter()
                    .position(|option| option == value)
                {
                    combo.set_active_id(Some(&index.to_string()));
                } else {
                    combo.append(
                        Some("retained"),
                        &format!(
                            "Unavailable historical choice: {}",
                            preview(value.as_str().unwrap_or("Unsupported shape"))
                        ),
                    );
                    combo.set_active_id(Some("retained"));
                }
            }
            let options = rows(&field, "options").to_vec();
            let f = form.clone();
            let i = id.clone();
            let r = record.clone();
            let n = note.clone();
            let reverting = Cell::new(false);
            combo.connect_changed(move |c| {
                if reverting.get() {
                    return;
                }
                let Some(value) = c
                    .active_id()
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|index| options.get(index))
                    .cloned()
                else {
                    return;
                };
                let candidate = set_value(&f.value.borrow(), &r, &i, value, &f.scenes.borrow());
                if !apply(&f, candidate, &n) {
                    let prior = find(&f.value.borrow(), "records", &r)
                        .and_then(|r| r["values"].get(&i))
                        .cloned();
                    reverting.set(true);
                    c.remove_all();
                    for (index, option) in options.iter().enumerate() {
                        c.append(
                            Some(&index.to_string()),
                            &preview(option.as_str().unwrap_or("Unsupported historical option")),
                        );
                    }
                    if let Some(prior) = prior {
                        if let Some(index) = options.iter().position(|value| value == &prior) {
                            c.set_active(Some(index as u32));
                        } else {
                            c.append(
                                Some("retained"),
                                &format!(
                                    "Unavailable historical choice: {}",
                                    preview(prior.as_str().unwrap_or("Unsupported shape"))
                                ),
                            );
                            c.set_active_id(Some("retained"));
                        }
                    } else {
                        c.set_active(None);
                    }
                    reverting.set(false);
                }
            });
            body.pack_start(&combo, false, false, 0);
        }
        "multiple-choice" => {
            for option in rows(&field, "options").iter().filter(|v| v.is_string()) {
                let check = gtk::CheckButton::with_label(option.as_str().unwrap());
                check.set_active(
                    current
                        .as_ref()
                        .and_then(Value::as_array)
                        .is_some_and(|a| a.contains(option)),
                );
                body.pack_start(&check, false, false, 0);
                let option = option.clone();
                let f = form.clone();
                let i = id.clone();
                let r = record.clone();
                let n = note.clone();
                let reverting = Cell::new(false);
                check.connect_toggled(move |c| {
                    if reverting.get() {
                        return;
                    }
                    let doc = f.value.borrow();
                    let old = find(&doc, "records", &r).map(|r| &r["values"][&i]);
                    let previous = old
                        .and_then(Value::as_array)
                        .is_some_and(|values| values.contains(&option));
                    let mut values = old.and_then(Value::as_array).cloned().unwrap_or_default();
                    if c.is_active() {
                        if !values.contains(&option) {
                            values.push(option.clone());
                        }
                    } else {
                        values.retain(|v| v != &option);
                    }
                    let candidate = set_value(&doc, &r, &i, json!(values), &f.scenes.borrow());
                    drop(doc);
                    if !apply(&f, candidate, &n) {
                        reverting.set(true);
                        c.set_active(previous);
                        reverting.set(false);
                    }
                });
            }
            if let Some(array) = current.as_ref().and_then(Value::as_array) {
                for option in array
                    .iter()
                    .filter(|v| !rows(&field, "options").contains(v))
                {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                    caption(
                        &row,
                        &format!(
                            "Unavailable historical choice: {}",
                            preview(option.as_str().unwrap_or("Unsupported shape"))
                        ),
                    );
                    let remove = gtk::Button::with_label("Remove unavailable choice");
                    row.pack_start(&remove, false, false, 0);
                    body.pack_start(&row, false, false, 0);
                    let option = option.clone();
                    let f = form.clone();
                    let i = id.clone();
                    let r = record.clone();
                    let n = note.clone();
                    remove.connect_clicked(move |button| {
                        let doc = f.value.borrow();
                        let mut values = find(&doc, "records", &r)
                            .and_then(|r| r["values"][&i].as_array())
                            .cloned()
                            .unwrap_or_default();
                        values.retain(|v| v != &option);
                        let candidate = set_value(&doc, &r, &i, json!(values), &f.scenes.borrow());
                        drop(doc);
                        if apply(&f, candidate, &n) {
                            button.set_sensitive(false);
                        }
                    });
                }
            }
        }
        "links" => body.pack_start(&link_editor(form.clone(), record, id), false, false, 0),
        _ => caption(
            &body,
            "Unsupported historical field type. The exact stored value is retained.",
        ),
    }
    body.pack_start(&note, false, false, 0);
    body
}
fn link_editor(form: Form, record: String, field: String) -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let kind = gtk::ComboBoxText::new();
    for k in LINK_KINDS {
        kind.append(Some(k), k);
    }
    kind.set_widget_name("custom-link-kind");
    body.pack_start(&kind, false, false, 0);
    let target = gtk::ComboBoxText::new();
    target.set_widget_name("custom-link-target");
    body.pack_start(&target, false, false, 0);
    let retained = gtk::ComboBoxText::new();
    retained.set_widget_name("custom-link-retained");
    body.pack_start(&retained, false, false, 0);
    let expected = Rc::new(RefCell::new(Value::Null));
    fill_links(&retained, &form, &record, &field, &expected);
    let f = form.clone();
    let t = target.clone();
    kind.connect_changed(move |c| {
        t.remove_all();
        if let Some(kind) = c.active_id() {
            for candidate in targets(&f.value.borrow(), &f.scenes.borrow(), &kind) {
                t.append(Some(&candidate.id), &preview(&candidate.label));
            }
        }
        t.set_active(None);
    });
    let note = status();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    body.pack_start(&row, false, false, 0);
    for (label, action) in [
        ("Add typed link", "add"),
        ("Replace selected link", "replace"),
        ("Remove selected link", "remove"),
    ] {
        let f = form.clone();
        let r = record.clone();
        let i = field.clone();
        let k = kind.clone();
        let t = target.clone();
        let c = retained.clone();
        let e = expected.clone();
        let n = note.clone();
        let button = gtk::Button::with_label(label);
        row.pack_start(&button, false, false, 0);
        button.connect_clicked(move |_| {
            let index = c.active_id().and_then(|s| s.parse::<usize>().ok());
            let replacement = if action == "remove" {
                None
            } else {
                let Some(kind) = k.active_id() else {
                    n.set_text("Choose a link kind explicitly.");
                    return;
                };
                let Some(id) = t.active_id() else {
                    n.set_text("Choose an available canonical target explicitly.");
                    return;
                };
                Some(json!({"kind":kind.as_str(),"id":id.as_str()}))
            };
            if action != "add" && index.is_none() {
                n.set_text("Select the retained link row to replace/remove.");
                return;
            }
            let candidate = link_candidate(
                &f.value.borrow(),
                &r,
                &i,
                &e.borrow(),
                if action == "add" { None } else { index },
                replacement,
                &f.scenes.borrow(),
            );
            if apply(&f, candidate, &n) {
                fill_links(&c, &f, &r, &i, &e);
            }
        });
    }
    caption(&body, "Scene targets use the checked saved scene catalog. Missing historical links remain exact; no first target is selected automatically.");
    body.pack_start(&note, false, false, 0);
    body
}
fn fill_links(
    combo: &gtk::ComboBoxText,
    form: &Form,
    record: &str,
    field: &str,
    expected: &RefCell<Value>,
) {
    combo.remove_all();
    let doc = form.value.borrow();
    let scenes = form.scenes.borrow();
    let value = find(&doc, "records", record)
        .map(|r| r["values"][field].clone())
        .unwrap_or(Value::Null);
    *expected.borrow_mut() = value.clone();
    if let Some(links) = value.as_array() {
        for (i, link) in links.iter().enumerate() {
            let target = targets(&doc, &scenes, text(link, "kind"))
                .into_iter()
                .find(|t| t.id == text(link, "id"));
            let label = target
                .map(|t| format!("{} · {}", t.kind, preview(&t.label)))
                .unwrap_or_else(|| {
                    format!(
                        "Unavailable historical link · {}",
                        preview(&link.to_string())
                    )
                });
            combo.append(Some(&i.to_string()), &label);
        }
    } else if !value.is_null() {
        combo.append(
            Some("historical"),
            &format!(
                "Unsupported historical links · {}",
                preview(&value.to_string())
            ),
        );
    }
    combo.set_active(None);
}

#[cfg(test)]
#[path = "custom_fields_tests.rs"]
pub(crate) mod tests;
