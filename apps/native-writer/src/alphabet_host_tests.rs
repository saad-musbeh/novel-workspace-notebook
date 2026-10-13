use super::*;
use std::collections::VecDeque;
fn definition(version: u64) -> Value {
    json!({"schemaVersion":1,"id":"symbols","mappingVersion":version,"name":"Symbols مِ 中文","scriptId":"synthetic","encoding":"private-use","direction":"rtl","mappings":[{"input":"a","output":"\u{e000}","transliteration":"a"},{"input":"b","output":"\u{f0000}","transliteration":"b"}],"font":null,"glossary":[],"notes":"Exact e\u{301}"})
}
fn document() -> Value {
    json!({"schemaVersion":1,"systems":[definition(1)],"assignments":[{"scene":{"kind":"scene","id":"s"},"definitionId":"symbols","mappingVersion":1}]})
}
fn pd_heads(ids: Value) -> Value {
    json!({"entityId":"p:writingSystems","entityType":"projectData","revisionIds":ids})
}
fn raw(id: &str) -> Value {
    json!({"id":id,"projectId":"p","entityId":"p:writingSystems","entityType":"projectData"})
}
fn valid_script() -> Vec<(&'static str, Reply)> {
    vec![
        ("project_data_heads", Ok(pd_heads(json!(["r1"])))),
        ("project_data_history", Ok(json!([raw("r1")]))),
        (
            "load_project_data_revision",
            Ok(json!({"revisionId":"r1","payload":document()})),
        ),
        (
            "load_project_data_unambiguous",
            Ok(json!({"revisionId":"r1","payload":document()})),
        ),
        (
            "load_project_scene",
            Ok(json!({"id":"s","revisionId":"sr1"})),
        ),
        (
            "scene_heads",
            Ok(json!({"entityId":"s","entityType":"scene","revisionIds":["sr1"]})),
        ),
        ("project_data_heads", Ok(pd_heads(json!(["r1"])))),
        ("project_data_history", Ok(json!([raw("r1")]))),
    ]
}
fn run_script(
    script: Vec<(&'static str, Reply)>,
) -> (Result<Capture, String>, Vec<(String, Value)>) {
    let script = Rc::new(RefCell::new(VecDeque::from(script)));
    let events = Rc::new(RefCell::new(Vec::new()));
    let queue = script.clone();
    let calls = events.clone();
    let rpc: Rpc = Rc::new(move |op, args, done| {
        calls.borrow_mut().push((op.to_owned(), args));
        let (expected, reply) = queue.borrow_mut().pop_front().expect("Unexpected host RPC");
        assert_eq!(op, expected);
        done(reply);
    });
    let answer = Rc::new(RefCell::new(None));
    let out = answer.clone();
    capture_with(
        rpc,
        "p".into(),
        Some("s".into()),
        Box::new(move |r| *out.borrow_mut() = Some(r)),
    );
    assert!(script.borrow().is_empty());
    let result = answer.borrow_mut().take().expect("Capture never completed");
    let calls = events.borrow().clone();
    (result, calls)
}
#[test]
fn concrete_rpc_capture_checks_every_history_revision_and_live_source() {
    let (result, calls) = run_script(valid_script());
    let capture = result.unwrap();
    let profile = SessionProfile::checked(&capture, "s").unwrap();
    assert_eq!(profile.alphabet_revision_id.as_deref(), Some("r1"));
    assert_eq!(profile.scene_revision_id, "sr1");
    assert_eq!(profile.definition, Some(definition(1)));
    check_profile(&profile, "p", "s", "sr1").unwrap();
    for (project, scene, revision) in [
        ("other", "s", "sr1"),
        ("p", "other", "sr1"),
        ("p", "s", "changed"),
    ] {
        assert!(check_profile(&profile, project, scene, revision).is_err());
    }
    assert_eq!(calls[2].1["revisionId"], "r1");
    assert_eq!(calls[4].1, json!({"projectId":"p","sceneId":"s"}));
    let mut draft = crate::alphabets::Draft::new(capture);
    draft.assign("s", None).unwrap();
    let save = draft.begin_save().unwrap();
    assert_eq!(save["parent"], "r1");
    assert_eq!(
        save["dependencies"],
        json!([{"entityId":"s","entityType":"scene","revisionId":"sr1"}])
    );
}
#[test]
fn capture_refuses_competing_or_foreign_head_identities() {
    let (result, _) = run_script(vec![(
        "project_data_heads",
        Ok(pd_heads(json!(["one", "two"]))),
    )]);
    assert!(result.unwrap_err().contains("competing"));
    let (result, _) = run_script(vec![(
        "project_data_heads",
        Ok(
            json!({"entityId":"foreign:writingSystems","entityType":"projectData","revisionIds":[]}),
        ),
    )]);
    assert!(result.is_err());
}
#[test]
fn rechecks_refuse_changed_heads_or_history_without_retry() {
    let mut script = valid_script();
    script.truncate(7);
    script[6].1 = Ok(pd_heads(json!(["r2"])));
    let (result, _) = run_script(script);
    assert!(result.unwrap_err().contains("heads changed"));
    let mut script = valid_script();
    script[7].1 = Ok(json!([raw("r1"), raw("r2")]));
    let (result, _) = run_script(script);
    assert!(result.unwrap_err().contains("history changed"));
}
#[test]
fn checked_revision_failure_or_projected_mismatch_never_creates_defaults() {
    let mut script = valid_script();
    script.truncate(3);
    script[2].1 = Err("Checksum failed".into());
    let (result, _) = run_script(script);
    assert!(result.unwrap_err().contains("Checksum"));
    let mut script = valid_script();
    script.truncate(4);
    script[3].1 = Ok(json!({"revisionId":"other","payload":document()}));
    assert!(run_script(script).0.is_err());
    let mut script = valid_script();
    script.truncate(2);
    script[1].1 = Ok(
        json!([{"id":"r1","projectId":"foreign","entityId":"p:writingSystems","entityType":"projectData"}]),
    );
    assert!(run_script(script).0.is_err());
}
#[test]
fn unavailable_foreign_or_competing_scene_sources_refuse_capture() {
    let mut script = valid_script();
    script.truncate(5);
    script[4].1 = Err("Scene is no longer in this manuscript".into());
    assert!(run_script(script).0.is_err());
    let mut script = valid_script();
    script.truncate(5);
    script[4].1 = Ok(json!({"id":"foreign","revisionId":"sr1"}));
    assert!(run_script(script).0.is_err());
    let mut script = valid_script();
    script.truncate(6);
    script[5].1 = Ok(json!({"entityId":"s","entityType":"scene","revisionIds":["sr1","deleted"]}));
    assert!(run_script(script).0.unwrap_err().contains("competing"));
}
#[test]
fn missing_first_version_is_distinct_from_retained_unprojected_history() {
    let script = vec![
        ("project_data_heads", Ok(pd_heads(json!([])))),
        ("project_data_history", Ok(json!([]))),
        ("load_project_data_unambiguous", Ok(Value::Null)),
        (
            "load_project_scene",
            Ok(json!({"id":"s","revisionId":"sr1"})),
        ),
        (
            "scene_heads",
            Ok(json!({"entityId":"s","entityType":"scene","revisionIds":["sr1"]})),
        ),
        ("project_data_heads", Ok(pd_heads(json!([])))),
        ("project_data_history", Ok(json!([]))),
    ];
    let (result, _) = run_script(script);
    assert!(SessionProfile::checked(&result.unwrap(), "s")
        .unwrap()
        .definition
        .is_none());
}

fn wait(condition: impl Fn() -> bool) {
    let start = std::time::Instant::now();
    let context = glib::MainContext::default();
    while !condition() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "Alphabet host worker/UI timeout"
        );
        std::thread::sleep(Duration::from_millis(8));
    }
    while context.pending() {
        context.iteration(false);
    }
}
fn rpc(client: &Process, op: &str, args: Value) -> Value {
    let answer = Rc::new(RefCell::new(None));
    let out = answer.clone();
    client.rpc(op, args, move |r| *out.borrow_mut() = Some(r));
    wait(|| answer.borrow().is_some());
    let value = answer.borrow_mut().take().unwrap().unwrap();
    value
}
fn named(widget: &impl IsA<gtk::Widget>, caption: &str) -> Option<gtk::Button> {
    if let Ok(button) = widget.as_ref().clone().downcast::<gtk::Button>() {
        if button.label().as_deref() == Some(caption) {
            return Some(button);
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .and_then(|container| container.children().iter().find_map(|w| named(w, caption)))
}
fn form(a: &Workspace) -> Rc<AlphabetPage> {
    let widget = a.current_widget().unwrap();
    a.alphabet_pages
        .borrow()
        .iter()
        .find(|p| p.widget.upcast_ref::<gtk::Widget>() == &widget)
        .unwrap()
        .clone()
}
fn open(a: &Rc<Workspace>) -> Rc<AlphabetPage> {
    let old = a.current_widget();
    a.alphabet_palette();
    wait(|| {
        a.current_widget() != old
            && a.alphabet_pages
                .borrow()
                .iter()
                .any(|p| a.current_widget() == Some(p.widget.clone().upcast()))
    });
    form(a)
}
fn save(a: &Rc<Workspace>) -> Result<(), String> {
    let answer = Rc::new(RefCell::new(None));
    let out = answer.clone();
    a.save_current(move |r| *out.borrow_mut() = Some(r));
    wait(|| answer.borrow().is_some());
    let value = answer.borrow_mut().take().unwrap();
    value
}
fn captured(client: &Process, pid: &str, sid: &str) -> Result<Capture, String> {
    let answer = Rc::new(RefCell::new(None));
    let out = answer.clone();
    capture(client, pid.into(), Some(sid.into()), move |r| {
        *out.borrow_mut() = Some(r)
    });
    wait(|| answer.borrow().is_some());
    let result = answer.borrow_mut().take().unwrap();
    result
}
/// Called from the parent's existing single-threaded synthetic GTK workflow.
pub(crate) fn run(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("alphabet-data");
    let cache = fixture.path().join("alphabet-cache");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let worker = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(worker), &data, true).unwrap();
    let novel = rpc(
        &client,
        "create_novel",
        json!({"title":"Literal alphabet palette مِ 中文"}),
    );
    let pid = novel["project"]["id"].as_str().unwrap().to_owned();
    let sid = novel["scene"]["id"].as_str().unwrap().to_owned();
    let original = rpc(
        &client,
        "load_project_scene",
        json!({"projectId":pid,"sceneId":sid}),
    );
    let mut doc = document();
    doc["assignments"][0]["scene"]["id"] = json!(sid);
    let saved = rpc(
        &client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":KIND,"payload":doc,"parent":null,"dependencies":[{"entityId":sid,"entityType":"scene","revisionId":original["revisionId"]}]}),
    );
    let capability = captured(&client, &pid, &sid).unwrap();
    let profile = SessionProfile::checked(&capability, &sid).unwrap();
    assert_eq!(profile.definition, Some(definition(1)));
    assert_eq!(
        profile.alphabet_revision_id,
        saved["revisionId"].as_str().map(str::to_owned)
    );
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.scene_id == sid && p.open.get() && p.pending.get() == 0)
    });
    let pane = a.active.borrow().clone().unwrap();
    let original_document = pane.frame.borrow()["document"].clone();
    let p = open(&a);
    p.versions.set_active_id(Some("symbols@1"));
    crate::app::desk::tests::capture(&a,"notebook-alphabets.png");
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    // This is the real palette button -> weak current host -> one existing worker edit.
    let insert = p
        .widget
        .children()
        .iter()
        .find_map(|w| find_symbol(w, "U+E000"))
        .expect("Saved symbol button");
    insert.clicked();
    wait(|| pane.pending.get() == 0 && pane.dirty.get());
    let inserted = pane.frame.borrow()["document"].clone();
    assert!(novel_storage::plain_text(&inserted).contains('\u{e000}'));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    assert_eq!(
        rpc(&a.client, "load_project_scene", json!({"projectId":pid,"sceneId":sid})),
        original
    );
    pane.operation(json!({"type":"command","command":"undo"}), true);
    wait(|| pane.pending.get() == 0);
    assert_eq!(pane.frame.borrow()["document"], original_document);
    pane.operation(json!({"type":"command","command":"redo"}), true);
    wait(|| pane.pending.get() == 0);
    assert_eq!(pane.frame.borrow()["document"], inserted);
    p.name.set_text("Version two مِ 中文");
    assert!(p.dirty.get());
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.new_tab();
    assert_eq!(a.current_widget(), Some(p.widget.clone().upcast()));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    save(&a).unwrap();
    assert!(!p.dirty.get());
    let second = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":pid,"kind":KIND}),
    );
    assert_eq!(second["payload"]["systems"][0], definition(1));
    assert_eq!(second["payload"]["systems"].as_array().unwrap().len(), 2);
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data_revision",
            json!({"projectId":pid,"kind":KIND,"revisionId":saved["revisionId"]})
        ),
        saved
    );
    // Literal insertion and mapping remain separate, even with a missing font.
    p.versions.set_active_id(Some("symbols@1"));
    pane.composing.set(true);
    let before = pane.frame.borrow()["document"].clone();
    find_symbol(&p.widget, "U+E000").unwrap().clicked();
    assert_eq!(pane.frame.borrow()["document"], before);
    assert!(p.status.text().contains("Finish input"));
    pane.composing.set(false);
    // Stale definition saves retain the entire draft, including immutable history.
    p.name.set_text("Stale local draft");
    let mut other = second["payload"].clone();
    other["unrelated"] = json!("Another window's exact metadata");
    let newer = rpc(
        &a.client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":KIND,"payload":other,"parent":second["revisionId"],"dependencies":[]}),
    );
    assert!(save(&a).is_err());
    assert!(p.dirty.get());
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":pid,"kind":KIND})
        ),
        newer
    );
    p.discard();
    // A captured scene deleted before assignment Save is rejected atomically.
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Discard);
    let fresh = open(&a);
    fresh.scenes.set_active_id(Some(&sid));
    fresh.assignment.set_active_id(Some(""));
    rpc(
        &a.client,
        "delete_scene",
        json!({"sceneId":sid,"parent":original["revisionId"]}),
    );
    assert!(save(&a).is_err());
    assert!(fresh.dirty.get());
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":pid,"kind":KIND})
        ),
        newer
    );
    fresh.discard();
    assert!(captured(&a.client, &pid, &sid).is_err());
    // Detachment carries the same draft, and Save uses its new current owner.
    fresh.versions.set_active_id(Some("symbols@1"));
    fresh.name.set_text("Detached version مِ 中文");
    a.detach_current();
    let child = WINDOWS.with(|windows| windows.borrow().last().unwrap().clone());
    assert!(Rc::ptr_eq(&form(&child), &fresh));
    assert!(!a
        .alphabet_pages
        .borrow()
        .iter()
        .any(|other| Rc::ptr_eq(other, &fresh)));
    named(&fresh.widget, "Save / publish new version")
        .unwrap()
        .clicked();
    wait(|| !fresh.saving.get());
    assert!(!fresh.dirty.get());
    assert_eq!(
        rpc(
            &child.client,
            "load_project_data_unambiguous",
            json!({"projectId":pid,"kind":KIND})
        )["payload"]["systems"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Discard);
    a.close();
    wait(|| WINDOWS.with(|windows| windows.borrow().len()) == 1);
    fresh.versions.set_active_id(Some("symbols@1"));
    fresh.name.set_text("After original closes");
    save(&child).unwrap();
    child.close();
    wait(|| WINDOWS.with(|windows| windows.borrow().is_empty()));
}
fn find_symbol(widget: &impl IsA<gtk::Widget>, codepoint: &str) -> Option<gtk::Button> {
    if let Ok(button) = widget.as_ref().clone().downcast::<gtk::Button>() {
        if button
            .label()
            .is_some_and(|label| label.contains(codepoint))
        {
            return Some(button);
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .and_then(|container| {
            container
                .children()
                .iter()
                .find_map(|w| find_symbol(w, codepoint))
        })
}

#[test]
#[ignore = "Requires already built companion; uses only synthetic temporary stores and no GUI."]
fn real_worker_checks_retained_history_and_live_scene_sources() {
    let binaries = std::env::var_os("NOVEL_RUST_TEST_BINARIES").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug"));
    let fixture = tempfile::tempdir().unwrap();
    let worker = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(worker), fixture.path(), true).unwrap();
    let novel = rpc(
        &client,
        "create_novel",
        json!({"title":"Checked alphabet host RPC"}),
    );
    let pid = novel["project"]["id"].as_str().unwrap();
    let sid = novel["scene"]["id"].as_str().unwrap();
    let scene = rpc(
        &client,
        "load_project_scene",
        json!({"projectId":pid,"sceneId":sid}),
    );
    let mut doc = document();
    doc["assignments"][0]["scene"]["id"] = json!(sid);
    let first = rpc(
        &client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":KIND,"payload":doc,"parent":null,"dependencies":[{"entityId":sid,"entityType":"scene","revisionId":scene["revisionId"]}]}),
    );
    let mut doc = first["payload"].clone();
    doc["systems"].as_array_mut().unwrap().push(definition(2));
    doc["assignments"][0]["mappingVersion"] = json!(2);
    let second = rpc(
        &client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":KIND,"payload":doc,"parent":first["revisionId"],"dependencies":[{"entityId":sid,"entityType":"scene","revisionId":scene["revisionId"]}]}),
    );
    let capture = captured(&client, pid, sid).unwrap();
    let profile = SessionProfile::checked(&capture, sid).unwrap();
    assert_eq!(profile.definition, Some(definition(2)));
    assert_eq!(
        profile.alphabet_revision_id,
        second["revisionId"].as_str().map(str::to_owned)
    );
    // Concrete get_asset -> checked descriptor -> private stage -> initial
    // disposable worker open. No GTK or system-font registration is involved.
    let bytes = crate::local_resources::tests::synthetic_sfnt(false);
    let (asset, font) = crate::local_resources::prepare_font(
        &bytes,
        &crate::local_resources::tests::license_fixture(),
    )
    .unwrap();
    rpc(&client, "put_asset", json!({"projectId":pid,"asset":asset}));
    let mut doc = second["payload"].clone();
    let mut version = definition(3);
    version["font"] = font.clone();
    doc["systems"].as_array_mut().unwrap().push(version);
    doc["assignments"][0]["mappingVersion"] = json!(3);
    let third = rpc(
        &client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":KIND,"payload":doc,"parent":second["revisionId"],"dependencies":[{"entityId":sid,"entityType":"scene","revisionId":scene["revisionId"]}]}),
    );
    let answer = Rc::new(RefCell::new(None));
    let out = answer.clone();
    load_editor_profile(&client, pid.into(), scene.clone(), move |loaded| {
        *out.borrow_mut() = Some(loaded)
    });
    wait(|| answer.borrow().is_some());
    let loaded = answer.borrow_mut().take().unwrap();
    assert!(loaded.notice.is_none(), "{:?}", loaded.notice);
    assert_eq!(loaded.asset.as_ref(), Some(&asset));
    assert_eq!(
        loaded.profile.as_ref().unwrap().alphabet_revision_id,
        third["revisionId"].as_str().map(str::to_owned)
    );
    let directory = fixture.path().join("private-editor");
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory).unwrap();
    let options = stage_editor_options(&directory, &loaded);
    assert!(options.notice.is_none(), "{:?}", options.notice);
    let executable = if binaries.join("novel-editor-rust").is_file() {
        "novel-editor-rust"
    } else {
        "novel-editor"
    };
    let editor = Process::start(&binaries.join(executable), &directory, false).unwrap();
    let frame = editor_request(
        &editor,
        json!({"type":"open","scene":scene["scene"],"width":360,"fonts":options.fonts,"writingSystem":options.writing_system}),
    );
    assert_eq!(frame["document"], scene["scene"]["document"]);
    let literal = "Literal a مِ 中文 𠀀 \u{e000} 👩🏽‍🚀 e\u{301}";
    let typed = editor_request(&editor, json!({"type":"text","text":literal}));
    assert_eq!(novel_storage::plain_text(&typed["document"]), literal);
    let undone = editor_request(&editor, json!({"type":"command","command":"undo"}));
    assert_eq!(undone["document"], frame["document"]);
    editor.stop();
    let marker = rpc(&client, "sync_cursor", json!({}));
    assert!(captured(&client, "foreign", sid).is_err());
    assert_eq!(rpc(&client, "sync_cursor", json!({})), marker);
    rpc(
        &client,
        "delete_scene",
        json!({"sceneId":sid,"parent":scene["revisionId"]}),
    );
    assert!(captured(&client, pid, sid).is_err());
    assert_eq!(
        rpc(
            &client,
            "load_project_data_revision",
            json!({"projectId":pid,"kind":KIND,"revisionId":first["revisionId"]})
        ),
        first
    );
    client.stop();
}

fn font_script(
    reply: Reply,
) -> (
    Vec<(&'static str, Reply)>,
    novel_storage::ProjectAsset,
    Value,
) {
    let bytes = crate::local_resources::tests::synthetic_sfnt(false);
    let license = crate::local_resources::tests::license_fixture();
    let (asset, font) = crate::local_resources::prepare_font(&bytes, &license).unwrap();
    let mut script = valid_script();
    let mut doc = document();
    doc["systems"][0]["font"] = font.clone();
    for index in [2, 3] {
        script[index].1 = Ok(json!({"revisionId":"r1","payload":doc}));
    }
    script.push(("get_asset", reply));
    (script, asset, font)
}
fn run_profile_script(
    script: Vec<(&'static str, Reply)>,
    revision: &str,
) -> (EditorProfile, Vec<(String, Value)>) {
    let script = Rc::new(RefCell::new(VecDeque::from(script)));
    let events = Rc::new(RefCell::new(Vec::new()));
    let queue = script.clone();
    let calls = events.clone();
    let rpc: Rpc = Rc::new(move |op, args, done| {
        calls.borrow_mut().push((op.to_owned(), args));
        let (expected, reply) = queue
            .borrow_mut()
            .pop_front()
            .expect("Unexpected editor-profile RPC");
        assert_eq!(op, expected);
        done(reply);
    });
    let answer = Rc::new(RefCell::new(None));
    let out = answer.clone();
    load_editor_profile_with(
        rpc,
        "p".into(),
        json!({"id":"s","revisionId":revision}),
        Box::new(move |r| *out.borrow_mut() = Some(r)),
    );
    assert!(script.borrow().is_empty());
    let result = answer.borrow_mut().take().expect("Profile never completed");
    let calls = events.borrow().clone();
    (result, calls)
}
#[test]
fn private_font_bridge_fetches_only_bound_asset_and_stages_verified_bytes() {
    let (mut script, asset, font) = font_script(Ok(Value::Null));
    script.last_mut().unwrap().1 = Ok(serde_json::to_value(&asset).unwrap());
    let (loaded, calls) = run_profile_script(script, "sr1");
    assert!(loaded.notice.is_none());
    assert_eq!(loaded.asset.as_ref().unwrap(), &asset);
    assert_eq!(
        calls.last().unwrap().1,
        json!({"projectId":"p","assetId":font["fontAssetId"]})
    );
    let directory = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let options = stage_editor_options(directory.path(), &loaded);
    assert!(options.notice.is_none(), "{:?}", options.notice);
    assert_eq!(options.writing_system["fontFamily"], font["family"]);
    let path = PathBuf::from(options.fonts[0]["path"].as_str().unwrap());
    assert!(path.starts_with(directory.path()));
    assert_eq!(std::fs::read(&path).unwrap(), asset.content);
    assert_eq!(options.fonts[0]["checksum"], font["checksum"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
            0
        );
    }
    assert_eq!(
        stage_editor_options(directory.path(), &loaded).fonts,
        options.fonts
    );
}
#[test]
fn missing_or_corrupt_font_preserves_saved_mapping_and_default_font_editing() {
    for unavailable in [Ok(Value::Null), Err("Project asset checksum failed".into())] {
        let (script, _, _) = font_script(unavailable);
        let (loaded, _) = run_profile_script(script, "sr1");
        assert!(loaded.asset.is_none());
        assert!(loaded.profile.as_ref().unwrap().definition.is_some());
        assert!(loaded.notice.as_ref().unwrap().contains("default fonts"));
        let directory = tempfile::tempdir().unwrap();
        let options = stage_editor_options(directory.path(), &loaded);
        assert_eq!(options.fonts, json!([]));
        assert_eq!(options.writing_system["definitionId"], "symbols");
        assert!(options.writing_system["fontFamily"].is_null());
        assert!(!directory.path().join("fonts").exists());
    }
    let (mut script, mut asset, _) = font_script(Ok(Value::Null));
    asset.content[0] ^= 1;
    script.last_mut().unwrap().1 = Ok(serde_json::to_value(asset).unwrap());
    let (loaded, _) = run_profile_script(script, "sr1");
    assert!(loaded.asset.is_none());
    assert!(loaded.notice.as_ref().unwrap().contains("checksum"));
}
#[test]
fn wrong_scene_profile_falls_back_to_literal_before_fetching_any_font() {
    let (loaded, calls) = run_profile_script(valid_script(), "wrong");
    assert!(loaded.profile.is_none());
    assert!(loaded.notice.as_ref().unwrap().contains("literal"));
    assert!(calls.iter().all(|(op, _)| op != "get_asset"));
    let directory = tempfile::tempdir().unwrap();
    let options = stage_editor_options(directory.path(), &loaded);
    assert_eq!(options.fonts, json!([]));
    assert!(options.writing_system.is_null());
    assert!(!directory.path().join("fonts").exists());
}
#[test]
fn unsafe_font_stage_refuses_privileged_path_and_keeps_default_font_options() {
    let (mut script, asset, _) = font_script(Ok(Value::Null));
    script.last_mut().unwrap().1 = Ok(serde_json::to_value(asset).unwrap());
    let (loaded, _) = run_profile_script(script, "sr1");
    let directory = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let options = stage_editor_options(directory.path(), &loaded);
        assert_eq!(options.fonts, json!([]));
        assert!(options.writing_system["fontFamily"].is_null());
        assert!(options.notice.as_ref().unwrap().contains("privately"));
        assert!(!directory.path().join("fonts").exists());
    }
}

fn editor_request(editor: &Process, args: Value) -> Value {
    let answer = Rc::new(RefCell::new(None));
    let out = answer.clone();
    editor.request(args, move |r| *out.borrow_mut() = Some(r));
    wait(|| answer.borrow().is_some());
    let result = answer.borrow_mut().take().unwrap().unwrap();
    result
}
