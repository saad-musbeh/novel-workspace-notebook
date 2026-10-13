use super::*;
fn capture() -> Value {
    json!({"projectId":"hidden-project","sceneId":"hidden-scene","chapterId":"hidden-chapter","chapterTitle":"Chapter مِ 中文","position":0,"deleted":true,"projectedRevisionId":"ancestor","projectedScene":{"schemaVersion":1,"title":"Retained ancestor","document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Exact مِ 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"}]}]}]}},"heads":[{"revisionId":"delete-left","entityType":"tombstone","operation":"delete","createdAt":"Saved","scene":null},{"revisionId":"delete-right","entityType":"tombstone","operation":"resolveDelete","createdAt":"Saved","scene":null}],"placementHeads":["placement"]})
}
#[test]
fn deleted_capture_retains_checked_writing_without_treating_deletion_as_a_scene() {
    let v = capture();
    let c = checked_capture(&v, "hidden-project", "hidden-scene").unwrap();
    let choices = writing_choices(&c);
    assert_eq!(choices.len(), 1);
    assert!(choices[0].1.contains("before deletion"));
    assert_eq!(choices[0].2, v["projectedScene"]);
    assert!(checked_capture(&v, "other-project", "hidden-scene").is_err());
    assert!(checked_capture(&v, "hidden-project", "other-scene").is_err());
    let label = entry_label(
        &json!({"sceneId":"hidden-scene","title":"Retained مِ 中文","chapterTitle":"Chapter","deleted":true,"headCount":2}),
    );
    assert!(label.contains("deleted"));
    assert!(!label.contains("hidden-scene"));
}
#[test]
fn malformed_duplicate_and_foreign_recovery_shapes_refuse() {
    let v = capture();
    let mut wrong = v.clone();
    wrong["heads"][1] = wrong["heads"][0].clone();
    assert!(checked_capture(&wrong, "hidden-project", "hidden-scene").is_err());
    let mut wrong = v.clone();
    wrong["heads"][0]["scene"] = v["projectedScene"].clone();
    assert!(checked_capture(&wrong, "hidden-project", "hidden-scene").is_err());
    let mut wrong = v.clone();
    wrong["heads"] = json!([]);
    assert!(checked_capture(&wrong, "hidden-project", "hidden-scene").is_err());
    let mut wrong = v.clone();
    wrong["projectedScene"]["schemaVersion"] = json!(99);
    assert!(checked_capture(&wrong, "hidden-project", "hidden-scene").is_err());
    let mut wrong = v;
    wrong["extraForeignValue"] = json!(true);
    assert!(checked_capture(&wrong, "hidden-project", "hidden-scene").is_err());
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
            "Scene recovery timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn rpc(client: &Process, op: &str, args: Value) -> Value {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    client.rpc(op, args, move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some());
    let result = result.borrow_mut().take().unwrap().unwrap();
    result
}
fn button_named(widget: &impl IsA<gtk::Widget>, label: &str) -> Option<gtk::Button> {
    if let Ok(b) = widget.as_ref().clone().downcast::<gtk::Button>() {
        if b.label().as_deref() == Some(label) {
            return Some(b);
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .and_then(|c| c.children().iter().find_map(|w| button_named(w, label)))
}
fn combos(widget: &impl IsA<gtk::Widget>) -> Vec<gtk::ComboBoxText> {
    if let Ok(c) = widget.as_ref().clone().downcast::<gtk::ComboBoxText>() {
        return vec![c];
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .map(|c| c.children().iter().flat_map(combos).collect())
        .unwrap_or_default()
}
fn checkbox(widget: &impl IsA<gtk::Widget>) -> Option<gtk::CheckButton> {
    if let Ok(c) = widget.as_ref().clone().downcast::<gtk::CheckButton>() {
        return Some(c);
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .and_then(|c| c.children().iter().find_map(checkbox))
}
fn label_contains(widget: &impl IsA<gtk::Widget>, text: &str) -> bool {
    if let Ok(c) = widget.as_ref().clone().downcast::<gtk::Label>() {
        if c.text().contains(text) {
            return true;
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .is_some_and(|c| c.children().iter().any(|w| label_contains(w, text)))
}
fn scene(title: &str) -> Value {
    json!({"schemaVersion":1,"title":title,"document":{"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"Exact مِ e\u{301} 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"},{"type":"underline"}]}]}]}})
}
fn exchange(from: &Process, to: &Process, pid: &str) {
    let source = rpc(from, "sync_cursor", json!({}));
    let local = rpc(to, "sync_cursor", json!({}));
    let page = rpc(
        from,
        "pull_revisions",
        json!({"projectId":pid,"cursor":{"epoch":source["epoch"],"sequence":0},"limit":1000}),
    );
    rpc(
        to,
        "ingest_revisions",
        json!({"projectId":pid,"expectedEpoch":local["epoch"],"remoteEpoch":source["epoch"],"manifest":rpc(from,"open_project",json!({"projectId":pid})),"revisions":page["revisions"]}),
    );
}
fn choose(widget: &gtk::Widget, sid: &str) {
    assert!(combos(widget)[0].set_active_id(Some(sid)));
    wait(|| {
        label_contains(widget, "Inspect every retained")
            || label_contains(widget, "One deleted version")
    });
}
fn keep_deleted(widget: &gtk::Widget) -> gtk::Button {
    button_named(widget, "Keep deleted · acknowledge all versions").unwrap()
}
fn keep_writing(widget: &gtk::Widget) -> gtk::Button {
    button_named(widget, "Keep writing · save selected version").unwrap()
}
// Invoked only from the root opt-in GTK test's initialized thread.
pub(crate) fn run(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("recovery-data");
    let peer_data = fixture.path().join("recovery-peer");
    let cache = fixture.path().join("recovery-cache");
    for p in [&data, &peer_data, &cache] {
        std::fs::create_dir_all(p).unwrap();
    }
    let worker = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(worker), &data, true).unwrap();
    let peer = Process::start(&binaries.join(worker), &peer_data, true).unwrap();
    let project = rpc(
        &client,
        "create_novel",
        json!({"title":"Synthetic deletion recovery"}),
    );
    let pid = project["project"]["id"].as_str().unwrap();
    let sid = project["scene"]["id"].as_str().unwrap();
    let cid = project["scene"]["chapterId"].as_str().unwrap();
    let original = rpc(&client, "load_scene", json!({"sceneId":sid}));
    let deleted = rpc(
        &client,
        "create_scene",
        json!({"chapterId":cid,"scene":scene("Both delete مِ 中文")}),
    );
    let dsid = deleted["id"].as_str().unwrap();
    exchange(&client, &peer, pid);
    let edited = rpc(
        &client,
        "save_scene",
        json!({"sceneId":sid,"scene":scene("Retained edited writing"),"parent":original["revisionId"],"requestId":"scene-recovery-test-save"}),
    );
    rpc(
        &peer,
        "delete_scene",
        json!({"sceneId":sid,"parent":original["revisionId"]}),
    );
    rpc(
        &client,
        "delete_scene",
        json!({"sceneId":dsid,"parent":deleted["revisionId"]}),
    );
    rpc(
        &peer,
        "delete_scene",
        json!({"sceneId":dsid,"parent":deleted["revisionId"]}),
    );
    exchange(&peer, &client, pid);
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    a.scene_conflicts();
    wait(|| {
        a.current_widget()
            .is_some_and(|w| combos(&w).len() == 2 && label_contains(&w, "Inspect every retained"))
    });
    let widget = a.current_widget().unwrap();
    choose(&widget, dsid);
    assert!(!label_contains(&widget, dsid));
    assert!(!keep_deleted(&widget).is_sensitive());
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    keep_deleted(&widget).clicked();
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    // A new tombstone leaf invalidates the reviewed complete capture.
    let peer_heads = rpc(&peer, "scene_heads", json!({"sceneId":dsid}));
    let restored = rpc(
        &peer,
        "restore_deleted_scene",
        json!({"sceneId":dsid,"scene":deleted["scene"],"heads":peer_heads["revisionIds"]}),
    );
    rpc(
        &peer,
        "delete_scene",
        json!({"sceneId":dsid,"parent":restored["revisionId"]}),
    );
    exchange(&peer, &a.client, pid);
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    checkbox(&widget).unwrap().set_active(true);
    keep_deleted(&widget).clicked();
    wait(|| label_contains(&widget, "changed after this preview"));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    button_named(&widget, "Refresh scene recovery")
        .unwrap()
        .clicked();
    wait(|| label_contains(&widget, "Inspect every retained"));
    let child = a.spawn_window();
    child.open_scene(sid, false);
    wait(|| {
        child
            .active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let draft = child.active.borrow().clone().unwrap();
    draft.operation(
        json!({"type":"text","text":"Temporary unsaved recovery draft مِ 中文 "}),
        true,
    );
    wait(|| draft.pending.get() == 0 && draft.dirty.get());
    let draft_before = draft.frame.borrow()["document"].clone();
    child
        .desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    checkbox(&widget).unwrap().set_active(true);
    keep_deleted(&widget).clicked();
    wait(|| label_contains(&widget, "Scene recovery cancelled"));
    assert!(draft.dirty.get());
    assert_eq!(draft.frame.borrow()["document"], draft_before);
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    // Detach before approval so callbacks must follow the page's new owner.
    a.detach_current();
    let detached = desk::workspace_for_widget(&widget).unwrap();
    a.close();
    wait(|| a.desk.is_closed());
    child
        .desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Discard);
    keep_deleted(&widget).clicked();
    wait(|| label_contains(&widget, "Deletion choice saved"));
    assert!(!draft.dirty.get());
    assert_eq!(
        rpc(&detached.client, "scene_heads", json!({"sceneId":dsid}))["revisionIds"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(!rpc(
        &detached.client,
        "open_project_index",
        json!({"projectId":pid})
    )["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == dsid));
    child.close();
    wait(|| child.desk.is_closed());
    button_named(&widget, "Refresh scene recovery")
        .unwrap()
        .clicked();
    wait(|| label_contains(&widget, "One deleted version"));
    checkbox(&widget).unwrap().set_active(true);
    assert!(keep_writing(&widget).is_sensitive());
    assert!(!keep_deleted(&widget).is_sensitive());
    keep_writing(&widget).clicked();
    wait(|| label_contains(&widget, "Writing choice saved"));
    assert_eq!(
        rpc(
            &detached.client,
            "load_project_scene",
            json!({"projectId":pid,"sceneId":dsid})
        )["scene"],
        deleted["scene"]
    );
    button_named(&widget, "Refresh scene recovery")
        .unwrap()
        .clicked();
    wait(|| combos(&widget)[0].is_sensitive());
    choose(&widget, sid);
    // Recovery must evict a clean cached editor while preserving input queued
    // after global approval in another window, including its original Undo.
    let clean_window = detached.spawn_window();
    clean_window.open_scene(sid, false);
    let typed_window = detached.spawn_window();
    typed_window.open_scene(sid, false);
    wait(|| {
        [&clean_window, &typed_window].iter().all(|a| {
            a.active
                .borrow()
                .as_ref()
                .is_some_and(|p| p.open.get() && p.pending.get() == 0)
        })
    });
    let clean = clean_window.active.borrow().clone().unwrap();
    let typed = typed_window.active.borrow().clone().unwrap();
    let before = typed.frame.borrow()["document"].clone();
    let parent = typed.parent.borrow().clone();
    let typing = typed.clone();
    RECOVERY_BEFORE_REPLY.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            typing.operation(
                json!({"type":"text","text":"After approval مِ 中文 𠀀 👩🏽‍🚀 "}),
                true,
            );
        }));
    });
    checkbox(&widget).unwrap().set_active(true);
    keep_writing(&widget).clicked();
    wait(|| label_contains(&widget, "Writing choice saved"));
    wait(|| typed.pending.get() == 0 && typed.dirty.get());
    assert!(!clean_window.panes.borrow().contains_key(sid));
    assert!(clean_window.pages.page_num(&clean.widget).is_none());
    assert!(clean_window.active.borrow().is_none());
    assert!(Rc::ptr_eq(
        typed_window.panes.borrow().get(sid).unwrap(),
        &typed
    ));
    assert_eq!(*typed.parent.borrow(), parent);
    assert!(typed.return_note.is_visible());
    assert!(typed
        .return_note
        .text()
        .contains("Your draft and Undo remain"));
    let after = typed.frame.borrow()["document"].clone();
    assert!(novel_storage::plain_text(&after).contains("After approval مِ 中文 𠀀 👩🏽‍🚀"));
    typed.operation(json!({"type":"command","command":"undo"}), true);
    wait(|| typed.pending.get() == 0);
    assert_eq!(typed.frame.borrow()["document"], before);
    typed.operation(json!({"type":"command","command":"redo"}), true);
    wait(|| typed.pending.get() == 0);
    assert_eq!(typed.frame.borrow()["document"], after);
    let saved = rpc(
        &detached.client,
        "load_project_scene",
        json!({"projectId":pid,"sceneId":sid}),
    );
    assert_ne!(saved["revisionId"].as_str().unwrap(), parent);
    assert_eq!(saved["scene"], edited["scene"]);
    clean_window.open_scene(sid, false);
    wait(|| {
        clean_window
            .active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let reopened = clean_window.active.borrow().clone().unwrap();
    assert!(!Rc::ptr_eq(&clean, &reopened));
    assert_eq!(
        reopened.frame.borrow()["document"],
        saved["scene"]["document"]
    );
    assert_eq!(
        reopened.parent.borrow().as_str(),
        saved["revisionId"].as_str().unwrap()
    );
    typed_window
        .desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Discard);
    typed_window.close();
    wait(|| typed_window.desk.is_closed());
    clean_window.close();
    wait(|| clean_window.desk.is_closed());
    // Recreate the retained deletion choice, then exercise Keep deleted after
    // proving both safe cache paths above. Old writing remains immutable.
    let peer_heads = rpc(&peer, "scene_heads", json!({"sceneId":sid}));
    rpc(
        &peer,
        "restore_deleted_scene",
        json!({"sceneId":sid,"scene":edited["scene"],"heads":peer_heads["revisionIds"]}),
    );
    exchange(&peer, &detached.client, pid);
    button_named(&widget, "Refresh scene recovery")
        .unwrap()
        .clicked();
    wait(|| label_contains(&widget, "Inspect every retained"));
    checkbox(&widget).unwrap().set_active(true);
    keep_deleted(&widget).clicked();
    wait(|| label_contains(&widget, "Deletion choice saved"));
    assert_eq!(
        rpc(
            &detached.client,
            "load_revision",
            json!({"sceneId":sid,"revisionId":edited["revisionId"]})
        )["scene"],
        edited["scene"]
    );
    assert!(
        rpc(&detached.client, "deleted_scenes", json!({"projectId":pid}))
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == sid)
    );
    detached.close();
    wait(|| detached.desk.is_closed());
    peer.stop();
}
