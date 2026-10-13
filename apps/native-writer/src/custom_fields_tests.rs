use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn field(id: &str, kind: &str, record_kind: &str) -> Value {
    json!({"id":id,"label":format!("Field {id}"),"kind":kind,"recordKind":record_kind,"hidden":false,"options":["明 مِ","second","retired"]})
}
fn fixture() -> Value {
    let mut doc = initial("planning");
    doc["fields"] = json!([
        field("short", "text", "character"),
        field("place", "text", "location"),
        field("long", "long-text", "character"),
        field("numeric", "number", "character"),
        field("date", "date", "character"),
        field("choice", "single-choice", "character"),
        field("many", "multiple-choice", "character"),
        field("linked", "links", "character"),
        field("hidden", "text", "character")
    ]);
    doc["fields"][8]["hidden"] = json!(true);
    doc["records"] = json!([{"id":"hero","kind":"character","name":"نور 明","aliases":[],"notes":"","links":[],"values":{"short":"مِ 中文 𠀀 \u{e000} 👩🏽‍🚀","long":"Line one\nسطر ثانٍ 中文","numeric":1234567.890123,"date":"Third winter / 第三年","choice":"retired","many":["retired","明 مِ"],"linked":[],"hidden":"Hidden exact مِ 中文"}},{"id":"garden","kind":"location","name":"Garden","aliases":[],"notes":"","links":[],"values":{"place":"مكان"}}]);
    doc["events"] = json!([{"id":"event","title":"Explicit event","notes":"","sceneIds":[],"laneIds":[],"time":{"kind":"unknown"}}]);
    doc["plotPoints"] = json!([{"id":"point","title":"Plot point","notes":"","links":[],"requiresPayoff":false,"scopeClosed":false,"payoffId":null}]);
    doc
}
fn scenes() -> Vec<Value> {
    vec![json!({"id":"scene","title":"Scene 中文","revisionId":"saved-scene-head"})]
}

#[test]
fn finite_numbers_keep_precision_and_large_values_without_coercion() {
    for input in [
        "9007199254740993",
        "-1234567890.1234567",
        "1e100",
        "0.00000123456789",
    ] {
        let value = number_candidate(input).unwrap();
        assert_eq!(value, serde_json::from_str::<Value>(input).unwrap());
    }
    assert_eq!(
        number_candidate("9007199254740993").unwrap().as_u64(),
        Some(9007199254740993)
    );
    for input in ["-", "", "NaN", "1e999", "true", "\"2\"", "1,000"] {
        assert!(number_candidate(input).is_err());
    }
    let doc = fixture();
    let draft = set_draft_value(&doc, "hero", "numeric", json!("-")).unwrap();
    assert_eq!(draft["records"][0]["values"]["numeric"], "-");
    assert!(validate_transition(&doc, &draft, &scenes()).is_err());
    assert_eq!(
        doc["records"][0]["values"]["numeric"],
        json!(1234567.890123)
    );
}
#[test]
fn seven_typed_values_remain_exact_and_do_not_fill_absent_fields() {
    let doc = fixture();
    let before = doc.clone();
    for (id, value) in [
        ("short", json!("العَرَبيّة 中文 \u{e000} 👩🏽‍🚀")),
        ("long", json!("First\n中文\nمِ")),
        ("numeric", json!(9007199254740993u64)),
        ("date", json!("بعد الحدث / event + 2 winters")),
        ("choice", json!("明 مِ")),
        ("many", json!(["retired", "second"])),
        ("linked", json!([{"kind":"record","id":"garden"}])),
    ] {
        let next = set_value(&doc, "hero", id, value.clone(), &scenes()).unwrap();
        assert_eq!(next["records"][0]["values"][id], value);
        validate_transition(&doc, &next, &scenes()).unwrap();
    }
    assert_eq!(doc, before);
    assert!(!doc["records"][1]["values"]
        .as_object()
        .unwrap()
        .contains_key("short"));
}
#[test]
fn unavailable_choices_survive_definition_edits_and_require_explicit_new_picks() {
    let doc = fixture();
    let changed = edit_field(&doc, "choice", "options", json!(["second"])).unwrap();
    validate_transition(&doc, &changed, &scenes()).unwrap();
    assert_eq!(changed["records"], doc["records"]);
    assert!(set_value(&changed, "hero", "choice", json!("invented"), &scenes()).is_err());
    set_value(&changed, "hero", "choice", json!("second"), &scenes()).unwrap();
    let changed = edit_field(&changed, "many", "options", json!(["second"])).unwrap();
    let next = set_value(
        &changed,
        "hero",
        "many",
        json!(["retired", "明 مِ", "second"]),
        &scenes(),
    )
    .unwrap();
    assert_eq!(
        next["records"][0]["values"]["many"],
        json!(["retired", "明 مِ", "second"])
    );
    assert!(set_value(
        &changed,
        "hero",
        "many",
        json!(["new unavailable"]),
        &scenes()
    )
    .is_err());
}
#[test]
fn typed_links_resolve_canonical_kind_and_keep_unavailable_historical_rows() {
    let mut doc = fixture();
    doc["records"][0]["values"]["linked"] = json!([{"kind":"scene","id":"missing"}]);
    for (kind, id) in [
        ("record", "garden"),
        ("scene", "scene"),
        ("event", "event"),
        ("plot-point", "point"),
    ] {
        let next = link_candidate(
            &doc,
            "hero",
            "linked",
            &doc["records"][0]["values"]["linked"],
            None,
            Some(json!({"kind":kind,"id":id})),
            &scenes(),
        )
        .unwrap();
        assert_eq!(
            next["records"][0]["values"]["linked"][0],
            doc["records"][0]["values"]["linked"][0]
        );
    }
    for value in [
        json!(["scene"]),
        json!([{"kind":"plotPoint","id":"point"}]),
        json!([{"kind":"scene","id":"hero"}]),
        json!([{"kind":"record","id":"invented"}]),
    ] {
        assert!(set_value(&fixture(), "hero", "linked", value, &scenes()).is_err());
    }
}
#[test]
fn rename_hide_reorder_and_boundaries_preserve_hidden_and_visible_values() {
    let doc = fixture();
    let renamed = edit_field(&doc, "short", "label", json!("مُلاحظة 中文")).unwrap();
    let hidden = edit_field(&renamed, "short", "hidden", json!(true)).unwrap();
    let moved = move_field(&hidden, "short", 1).unwrap();
    validate_transition(&doc, &moved, &scenes()).unwrap();
    assert_eq!(moved["records"], doc["records"]);
    assert_eq!(moved["fields"][1], doc["fields"][1]);
    assert_eq!(moved["fields"][0]["id"], "long");
    assert_eq!(moved["fields"][2]["id"], "short");
    assert_eq!(move_field(&doc, "short", -1).unwrap(), doc);
    assert_eq!(move_field(&doc, "hidden", 1).unwrap(), doc);
    assert!(set_value(&hidden, "hero", "short", json!("overwrite"), &scenes()).is_err());
}
#[test]
fn used_definitions_cannot_be_deleted_or_reinterpreted() {
    let doc = fixture();
    assert!(remove_field(&doc, "hidden").is_err());
    assert!(remove_field(&doc, "linked").is_err());
    assert!(edit_field(&doc, "short", "kind", json!("number")).is_err());
    assert!(edit_field(&doc, "short", "recordKind", json!("location")).is_err());
    let long = edit_field(&doc, "short", "kind", json!("long-text")).unwrap();
    assert_eq!(long["records"], doc["records"]);
    let mut dropped = doc.clone();
    dropped["fields"]
        .as_array_mut()
        .unwrap()
        .retain(|f| f["id"] != "hidden");
    assert!(validate_transition(&doc, &dropped, &scenes()).is_err());
    let mut unused = doc.clone();
    unused["fields"]
        .as_array_mut()
        .unwrap()
        .push(field("empty", "text", "item"));
    let removed = remove_field(&unused, "empty").unwrap();
    assert_eq!(removed, doc);
}
#[test]
fn only_new_scene_links_capture_exact_saved_source_dependencies() {
    let doc = fixture();
    let next = set_value(
        &doc,
        "hero",
        "linked",
        json!([{"kind":"scene","id":"scene"}]),
        &scenes(),
    )
    .unwrap();
    let dependencies = validate_transition(&doc, &next, &scenes()).unwrap();
    assert_eq!(
        dependencies,
        vec![json!({"entityId":"scene","entityType":"scene","revisionId":"saved-scene-head"})]
    );
    assert!(validate_transition(&doc, &next, &[]).is_err());
    assert!(validate_transition(&next, &next, &[]).unwrap().is_empty());
}
#[test]
fn stale_link_rows_refuse_index_edits_without_mutating_anything() {
    let doc = fixture();
    let old = doc["records"][0]["values"]["linked"].clone();
    let changed = set_value(
        &doc,
        "hero",
        "linked",
        json!([{"kind":"record","id":"garden"}]),
        &scenes(),
    )
    .unwrap();
    assert!(
        link_candidate(&changed, "hero", "linked", &old, Some(0), None, &scenes())
            .unwrap_err()
            .contains("changed")
    );
    assert_eq!(
        changed["records"][0]["values"]["linked"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn unchanged_malformed_history_is_not_coerced_by_unrelated_field_edits() {
    let mut doc = fixture();
    doc["records"][0]["values"]["numeric"] = json!("Historical number text");
    doc["records"][0]["values"]["linked"] = json!(["Legacy untyped link"]);
    let next = edit_field(&doc, "short", "label", json!("Renamed")).unwrap();
    validate_transition(&doc, &next, &scenes()).unwrap();
    assert_eq!(next["records"], doc["records"]);
}

fn wait(condition: impl Fn() -> bool) {
    let start = Instant::now();
    let context = glib::MainContext::default();
    while !condition() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "Custom-field operation timed out"
        );
        std::thread::sleep(Duration::from_millis(8));
    }
    while context.pending() {
        context.iteration(false);
    }
}
fn rpc(client: &Process, op: &str, args: Value) -> Value {
    rpc_result(client, op, args).unwrap()
}
fn rpc_result(client: &Process, op: &str, args: Value) -> Result<Value, String> {
    let result = Rc::new(RefCell::new(None));
    let output = result.clone();
    client.rpc(op, args, move |reply| *output.borrow_mut() = Some(reply));
    wait(|| result.borrow().is_some());
    let value = result.borrow_mut().take().unwrap();
    value
}

#[test]
#[ignore = "Headless real-worker bridge; set NOVEL_RUST_TEST_BINARIES to rebuilt workers."]
fn checked_custom_fields_real_companion() {
    let binaries = std::env::var_os("NOVEL_RUST_TEST_BINARIES")
        .map(std::path::PathBuf::from)
        .expect("Explicit synthetic worker folder required");
    let temporary = tempfile::tempdir().unwrap();
    let client = Process::start(
        &binaries.join(if binaries.join("novel-companion-rust").is_file() {
            "novel-companion-rust"
        } else {
            "novel-companion"
        }),
        temporary.path(),
        true,
    )
    .unwrap();
    let novel = rpc(
        &client,
        "create_novel",
        json!({"title":"Headless typed-field bridge"}),
    );
    let pid = novel["project"]["id"].as_str().unwrap();
    let sid = novel["scene"]["id"].as_str().unwrap();
    let seed = fixture();
    let saved = rpc(
        &client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":"planning","payload":seed,"parent":null,"dependencies":[]}),
    );
    let capture = rpc(&client, "planning_read_snapshot", json!({"projectId":pid}));
    assert_eq!(capture["planning"]["payload"], seed);
    let catalog = capture["index"]["scenes"].as_array().unwrap();
    let candidate = set_value(
        &seed,
        "hero",
        "linked",
        json!([{"kind":"scene","id":sid}]),
        catalog,
    )
    .unwrap();
    let dependencies = validate_transition(&seed, &candidate, catalog).unwrap();
    let cursor = rpc(&client, "sync_cursor", json!({}));
    assert_eq!(data(&client, pid), saved);
    assert_eq!(rpc(&client, "sync_cursor", json!({})), cursor);
    // Synthetic writing advances the captured scene head; metadata must refuse it.
    let scene = rpc(&client, "load_scene", json!({"sceneId":sid}));
    let mut edited = scene["scene"].clone();
    edited["document"] = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Synthetic changed source مِ 中文"}]}]});
    let writing = rpc(
        &client,
        "save_scene",
        json!({"sceneId":sid,"scene":edited,"parent":scene["revisionId"]}),
    );
    assert!(rpc_result(&client, "save_project_data_checked", json!({"projectId":pid,"kind":"planning","payload":candidate,"parent":saved["revisionId"],"dependencies":dependencies})).unwrap_err().contains("Conflict"));
    assert_eq!(data(&client, pid), saved);
    let current = rpc(&client, "planning_read_snapshot", json!({"projectId":pid}));
    let catalog = current["index"]["scenes"].as_array().unwrap();
    let candidate = set_value(&seed, "hero", "linked", json!([{"kind":"record","id":"garden"},{"kind":"scene","id":sid},{"kind":"event","id":"event"},{"kind":"plot-point","id":"point"}]), catalog).unwrap();
    let dependencies = validate_transition(&seed, &candidate, catalog).unwrap();
    let accepted = rpc(
        &client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":"planning","payload":candidate,"parent":saved["revisionId"],"dependencies":dependencies}),
    );
    let renamed = edit_field(&seed, "short", "label", json!("Stale field rename")).unwrap();
    assert!(rpc_result(&client, "save_project_data_checked", json!({"projectId":pid,"kind":"planning","payload":renamed,"parent":saved["revisionId"],"dependencies":[]})).unwrap_err().contains("Conflict"));
    assert_eq!(data(&client, pid), accepted);
    let options = edit_field(&accepted["payload"], "choice", "options", json!(["second"])).unwrap();
    let reordered = move_field(&options, "short", 1).unwrap();
    let hidden = edit_field(&reordered, "short", "hidden", json!(true)).unwrap();
    let dependencies = validate_transition(&accepted["payload"], &hidden, catalog).unwrap();
    let final_saved = rpc(
        &client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":"planning","payload":hidden,"parent":accepted["revisionId"],"dependencies":dependencies}),
    );
    assert_eq!(
        final_saved["payload"]["records"],
        accepted["payload"]["records"]
    );
    let retained = rpc(
        &client,
        "load_project_data_revision",
        json!({"projectId":pid,"kind":"planning","revisionId":saved["revisionId"]}),
    );
    assert_eq!(retained["payload"], seed);
    assert_eq!(rpc(&client, "load_scene", json!({"sceneId":sid})), writing);
}
fn save(page: &Rc<DataPage>) -> Result<(), String> {
    let result = Rc::new(RefCell::new(None));
    let output = result.clone();
    page.save(move |reply| *output.borrow_mut() = Some(reply));
    wait(|| result.borrow().is_some());
    let value = result.borrow_mut().take().unwrap();
    value
}
fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut result = vec![widget.clone()];
    if let Ok(container) = widget.clone().downcast::<gtk::Container>() {
        for child in container.children() {
            result.extend(descendants(&child));
        }
    }
    result
}
fn named(widget: &impl IsA<gtk::Widget>, name: &str) -> gtk::Widget {
    descendants(widget.as_ref())
        .into_iter()
        .find(|w| w.widget_name() == name)
        .unwrap_or_else(|| panic!("Missing custom field control {name}"))
}
fn button(widget: &impl IsA<gtk::Widget>, label: &str) -> gtk::Button {
    descendants(widget.as_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| b.label().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("Missing button {label}"))
}
fn expand(page: &DataPage, name: &str) {
    let expander = descendants(page.widget.upcast_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Expander>().ok())
        .find(|e| e.label().as_deref() == Some(name))
        .unwrap();
    expander.set_expanded(true);
}
fn entry(widget: &impl IsA<gtk::Widget>) -> gtk::Entry {
    descendants(widget.as_ref())
        .into_iter()
        .find_map(|w| w.downcast().ok())
        .unwrap()
}
fn text_view(widget: &impl IsA<gtk::Widget>) -> gtk::TextView {
    descendants(widget.as_ref())
        .into_iter()
        .find_map(|w| w.downcast().ok())
        .unwrap()
}
fn data(client: &Process, pid: &str) -> Value {
    rpc(client, "planning_read_snapshot", json!({"projectId":pid}))["planning"].clone()
}

#[test]
#[ignore = "Root runs this serialized GTK helper with actual workers and synthetic stores."]
fn custom_fields_actual_data_page() {
    gtk::init().unwrap();
    let binaries = std::env::var_os("NOVEL_RUST_TEST_BINARIES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug")
        });
    run_gtk(&binaries);
}

/// Parent calls this from its serialized opt-in GTK workflow with real workers.
/// No GUI test runs during ordinary headless test discovery.
pub(crate) fn run_gtk(binaries: &Path) {
    let fixture_dir = tempfile::tempdir().unwrap();
    let store = fixture_dir.path().join("custom-fields-data");
    std::fs::create_dir_all(&store).unwrap();
    let executable = binaries.join(if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    });
    let client = Process::start(&executable, &store, true).unwrap();
    let novel = rpc(
        &client,
        "create_novel",
        json!({"title":"Typed custom fields"}),
    );
    let pid = novel["project"]["id"].as_str().unwrap();
    let sid = novel["scene"]["id"].as_str().unwrap();
    let mut seed = fixture();
    rpc(
        &client,
        "save_project_data",
        json!({"projectId":pid,"kind":"planning","payload":seed,"parent":null}),
    );
    let initial = data(&client, pid);
    seed["fields"][5]["options"] = json!(["second"]);
    seed["fields"][6]["options"] = json!(["second"]);
    rpc(
        &client,
        "save_project_data",
        json!({"projectId":pid,"kind":"planning","payload":seed,"parent":initial["revisionId"]}),
    );
    let page = DataPage::new(client.clone(), "planning");
    page.load(pid, Vec::new());
    wait(|| page.loaded.get());
    let host = gtk::Window::new(gtk::WindowType::Toplevel);
    host.set_title("Synthetic typed fields");
    host.set_accept_focus(false);
    host.set_focus_on_map(false);
    let notebook = gtk::Notebook::new();
    notebook.append_page(&page.widget, Some(&gtk::Label::new(Some("Story notes"))));
    host.add(&notebook);
    host.show_all();
    let baseline = data(&client, pid);
    let scene = rpc(&client, "load_scene", json!({"sceneId":sid}));
    let cursor = rpc(&client, "sync_cursor", json!({}));
    expand(&page, "Open Records");
    expand(&page, "Open Values");
    assert!(!page.dirty.get());
    assert_eq!(data(&client, pid), baseline);
    assert!(descendants(page.widget.upcast_ref())
        .iter()
        .all(|w| w.widget_name() != "custom-value-hidden"));
    let short = named(&page.widget, "custom-value-short");
    assert_eq!(
        entry(&short).text(),
        seed["records"][0]["values"]["short"].as_str().unwrap()
    );
    let choices = named(&page.widget, "custom-value-choice");
    let combo = descendants(&choices)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::ComboBoxText>().ok())
        .unwrap();
    assert_eq!(combo.active_id().as_deref(), Some("retained"));
    assert_eq!(
        combo.active_text().as_deref(),
        Some("Unavailable historical choice: retired")
    );
    let links = named(&page.widget, "custom-value-linked");
    let kind = named(&links, "custom-link-kind")
        .downcast::<gtk::ComboBoxText>()
        .unwrap();
    let target = named(&links, "custom-link-target")
        .downcast::<gtk::ComboBoxText>()
        .unwrap();
    assert!(kind.active_id().is_none());
    assert!(target.active_id().is_none());
    button(&links, "Add typed link").clicked();
    assert!(!page.dirty.get());
    assert_eq!(rpc(&client, "sync_cursor", json!({})), cursor);
    entry(&short).set_text("New exact مِ 中文 𠀀 \u{e000} 👩🏽‍🚀");
    text_view(&named(&page.widget, "custom-value-long"))
        .buffer()
        .unwrap()
        .set_text("Edited line\nسطر 中文");
    entry(&named(&page.widget, "custom-value-date")).set_text("After event + 3 winters / 第四年");
    let numeric = entry(&named(&page.widget, "custom-value-numeric"));
    numeric.set_text("-");
    assert!(save(&page).is_err());
    assert_eq!(data(&client, pid), baseline);
    assert_eq!(numeric.text(), "-");
    numeric.set_text("9007199254740993");
    combo.set_active_id(Some("0"));
    let many = named(&page.widget, "custom-value-many");
    let choose = descendants(&many)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::CheckButton>().ok())
        .find(|b| b.label().as_deref() == Some("second"))
        .unwrap();
    choose.set_active(true);
    for (k, id) in [
        ("record", "garden"),
        ("scene", sid),
        ("event", "event"),
        ("plot-point", "point"),
    ] {
        kind.set_active_id(Some(k));
        assert!(target.active_id().is_none());
        target.set_active_id(Some(id));
        button(&links, "Add typed link").clicked();
    }
    assert!(page.dirty.get());
    assert_eq!(data(&client, pid), baseline);
    save(&page).unwrap();
    let saved = data(&client, pid);
    assert_eq!(
        saved["payload"]["records"][0]["values"]["numeric"].as_u64(),
        Some(9007199254740993)
    );
    assert_eq!(
        saved["payload"]["records"][0]["values"]["hidden"],
        seed["records"][0]["values"]["hidden"]
    );
    assert_eq!(
        saved["payload"]["records"][0]["values"]["linked"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(rpc(&client, "load_scene", json!({"sceneId":sid})), scene);
    // Definition actions retain all stored values and only save manually.
    expand(&page, "Open Fields");
    let selected = named(&page.widget, "custom-field-selector")
        .downcast::<gtk::ComboBoxText>()
        .unwrap();
    selected.set_active_id(Some("short"));
    named(&page.widget, "custom-field-label")
        .downcast::<gtk::Entry>()
        .unwrap()
        .set_text("مُلاحظة 中文");
    button(&page.widget, "Move field down").clicked();
    assert_eq!(page.value.borrow()["records"], saved["payload"]["records"]);
    let type_combo = named(&page.widget, "custom-field-kind")
        .downcast::<gtk::ComboBoxText>()
        .unwrap();
    type_combo.set_active_id(Some("number"));
    assert_eq!(type_combo.active_id().as_deref(), Some("text"));
    button(&page.widget, "Remove unused field").clicked();
    assert!(find(&page.value.borrow(), "fields", "short").is_some());
    let hide = descendants(page.widget.upcast_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::CheckButton>().ok())
        .find(|b| b.label().as_deref() == Some("Hide field (retain all stored values)"))
        .unwrap();
    hide.set_active(true);
    assert_eq!(data(&client, pid), saved);
    page.discard();
    assert!(!page.dirty.get());
    assert_eq!(*page.value.borrow(), saved["payload"]);
    expand(&page, "Open Fields");
    named(&page.widget, "custom-field-selector")
        .downcast::<gtk::ComboBoxText>()
        .unwrap()
        .set_active_id(Some("choice"));
    button(&page.widget, "Add option").clicked();
    let option = named(&page.widget, "custom-option-text")
        .downcast::<gtk::TextView>()
        .unwrap();
    option
        .buffer()
        .unwrap()
        .set_text("New option مِ 中文\nExact second line");
    assert!(page.dirty.get());
    assert_eq!(data(&client, pid), saved);
    save(&page).unwrap();
    // A stale CAS retains typed pending values, including hidden values.
    let other = DataPage::new(client.clone(), "planning");
    other.load(pid, Vec::new());
    wait(|| other.loaded.get());
    expand(&other, "Open Records");
    expand(&other, "Open Values");
    entry(&named(&other.widget, "custom-value-short")).set_text("Competing value");
    save(&other).unwrap();
    expand(&page, "Open Records");
    expand(&page, "Open Values");
    let pending = entry(&named(&page.widget, "custom-value-short"));
    pending.set_text("Retained stale draft مِ 中文");
    let newer = data(&client, pid);
    assert!(save(&page).is_err());
    assert!(page.dirty.get());
    assert_eq!(pending.text(), "Retained stale draft مِ 中文");
    assert_eq!(data(&client, pid), newer);
    page.discard();
    // Reparent the same page, reload explicitly, then save through its own worker.
    notebook.remove_page(Some(0));
    let detached = gtk::Notebook::new();
    detached.append_page(
        &page.widget,
        Some(&gtk::Label::new(Some("Moved Story notes"))),
    );
    page.load(pid, Vec::new());
    wait(|| page.loaded.get());
    expand(&page, "Open Records");
    expand(&page, "Open Values");
    entry(&named(&page.widget, "custom-value-short")).set_text("Saved after page transfer");
    save(&page).unwrap();
    assert_eq!(
        data(&client, pid)["payload"]["records"][0]["values"]["short"],
        "Saved after page transfer"
    );
    let retained = rpc(
        &client,
        "load_project_data_revision",
        json!({"projectId":pid,"kind":"planning","revisionId":baseline["revisionId"]}),
    );
    assert_eq!(retained["payload"], baseline["payload"]);
    assert_eq!(rpc(&client, "load_scene", json!({"sceneId":sid})), scene);
    host.hide();
    unsafe {
        host.destroy();
    }
}
