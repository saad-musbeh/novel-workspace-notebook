use super::*;
fn index() -> Value {
    json!({"project":{"id":"hidden-project","title":"Synthetic"},"chapters":[{"id":"hidden-chapter","title":"Chapter مِ 中文"}],"scenes":[{"id":"hidden-scene","chapterId":"hidden-chapter","title":"Scene 𠀀 👩🏽‍🚀"}]})
}
#[test]
fn previews_describe_titles_orders_and_positions_without_exposing_internal_ids() {
    let index = index();
    let choices = objects(&index);
    assert_eq!(choices.len(), 3);
    for (_, label, _) in &choices {
        assert!(!label.contains("hidden-"));
    }
    let order = describe_metadata(
        &index,
        "chapter",
        &json!({"title":"Chapter مِ 中文","order":["hidden-scene","missing-private-id"]}),
    );
    assert!(order.contains("Scene 𠀀 👩🏽‍🚀"));
    assert!(order.contains("Unavailable or deleted item"));
    assert!(order.contains("No chapter or scene content is deleted"));
    assert!(!order.contains("hidden-scene"));
    assert!(!order.contains("missing-private-id"));
    let placement = describe_metadata(
        &index,
        "scenePlacement",
        &json!({"chapterId":"hidden-chapter","position":0}),
    );
    assert!(placement.contains("Chapter مِ 中文"));
    assert!(placement.contains("Scene position: 1"));
    assert!(!placement.contains("hidden-chapter"));
    assert!(describe_metadata(
        &index,
        "scenePlacement",
        &json!({"chapterId":"missing","position":u64::MAX})
    )
    .contains("Unsupported position"));
}
#[test]
fn snapshot_retains_complete_unique_heads_and_single_head_replay_is_narrow() {
    let value = json!({"heads":["one","two"]});
    assert_eq!(captured_heads(&value).unwrap(), ["one", "two"]);
    assert!(captured_heads(&json!({"heads":["one","one"]})).is_err());
    assert!(captured_heads(&json!({"heads":[null]})).is_err());
    let saved = json!({"title":"Chapter","order":["a","b"]});
    let projection = json!({"title":"Chapter","order":["b","a"]});
    assert!(replay_needed(
        "chapter",
        &["one".into()],
        &saved,
        &projection
    ));
    assert!(!replay_needed("chapter", &["one".into()], &saved, &saved));
    assert!(!replay_needed(
        "project",
        &["one".into()],
        &saved,
        &projection
    ));
    assert!(!replay_needed(
        "scenePlacement",
        &["one".into()],
        &saved,
        &projection
    ));
}
#[test]
fn chapter_recovery_waits_for_current_and_listed_competing_scene_locations() {
    let object = Object {
        id: "target".into(),
        kind: "chapter".into(),
    };
    let capture = json!({"placementHeads":[{"sceneId":"current","chapterId":"target","heads":["a","b"]},{"sceneId":"listed","chapterId":"elsewhere","heads":["a","b"]}]});
    assert!(placement_blocked(&capture, &object, &json!({"order":[]})));
    assert!(placement_blocked(
        &capture,
        &Object {
            id: "empty".into(),
            kind: "chapter".into()
        },
        &json!({"order":["listed"]})
    ));
    assert!(!placement_blocked(
        &capture,
        &Object {
            id: "empty".into(),
            kind: "chapter".into()
        },
        &json!({"order":[]})
    ));
    assert!(!placement_blocked(
        &capture,
        &Object {
            id: "current".into(),
            kind: "scenePlacement".into()
        },
        &json!({"chapterId":"target"})
    ));
}

// Invoked from the existing opt-in GTK test's one initialized GTK thread.
fn wait(condition: impl Fn() -> bool) {
    let start = std::time::Instant::now();
    let context = glib::MainContext::default();
    while !condition() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "Metadata recovery UI timed out"
        );
        std::thread::sleep(Duration::from_millis(8));
    }
    while context.pending() {
        context.iteration(false);
    }
}
fn rpc(client: &Process, operation: &str, args: Value) -> Value {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    client.rpc(operation, args, move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some());
    let value = result.borrow_mut().take().unwrap().unwrap();
    value
}
fn button_named(widget: &impl IsA<gtk::Widget>, name: &str) -> Option<gtk::Button> {
    if let Ok(b) = widget.as_ref().clone().downcast::<gtk::Button>() {
        if b.label().as_deref() == Some(name) {
            return Some(b);
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .and_then(|c| c.children().iter().find_map(|w| button_named(w, name)))
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
fn label_contains(widget: &impl IsA<gtk::Widget>, text: &str) -> bool {
    if let Ok(l) = widget.as_ref().clone().downcast::<gtk::Label>() {
        if l.text().contains(text) {
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
fn apply(widget: &impl IsA<gtk::Widget>) -> gtk::Button {
    button_named(widget, "Use this retained arrangement")
        .or_else(|| button_named(widget, "Apply retained chapter order"))
        .unwrap()
}
fn choose_object(widget: &gtk::Widget, kind: &str, id: &str) {
    wait(|| combos(widget)[0].is_sensitive());
    assert!(combos(widget)[0].set_active_id(Some(&format!("{kind}:{id}"))));
    wait(|| combos(widget)[1].active_id().is_some());
}
fn choose_version(widget: &gtk::Widget, id: &str) {
    assert!(combos(widget)[1].set_active_id(Some(id)));
    wait(|| combos(widget)[1].active_id().as_deref() == Some(id));
}
fn heads(client: &Process, pid: &str, id: &str, kind: &str) -> Vec<String> {
    rpc(
        client,
        "metadata_heads",
        json!({"projectId":pid,"entityId":id,"entityType":kind}),
    )["revisionIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}
fn revision(client: &Process, pid: &str, id: &str, kind: &str, rid: &str) -> Value {
    rpc(
        client,
        "load_entity_revision",
        json!({"projectId":pid,"entityId":id,"entityType":kind,"revisionId":rid}),
    )
}
fn replicate(from: &Process, to: &Process, pid: &str, after: i64) {
    let source = rpc(from, "sync_cursor", json!({}));
    let target = rpc(to, "sync_cursor", json!({}));
    let page = rpc(
        from,
        "pull_revisions",
        json!({"projectId":pid,"cursor":{"epoch":source["epoch"],"sequence":after},"limit":1000}),
    );
    rpc(
        to,
        "ingest_revisions",
        json!({"projectId":pid,"expectedEpoch":target["epoch"],"remoteEpoch":source["epoch"],"manifest":rpc(from,"open_project",json!({"projectId":pid})),"revisions":page["revisions"]}),
    );
}
fn scene(title: &str) -> Value {
    json!({"schemaVersion":1,"title":title,"document":{"type":"doc","content":[{"type":"paragraph","attrs":{"lang":"ar","dir":"auto"},"content":[{"type":"text","text":"Synthetic مِ 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"}]}]}]}})
}
pub(crate) fn run(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("metadata-data");
    let peer_data = fixture.path().join("metadata-peer");
    let cache = fixture.path().join("metadata-cache");
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
        json!({"title":"Synthetic metadata recovery"}),
    );
    let pid = project["project"]["id"].as_str().unwrap();
    let sid = project["scene"]["id"].as_str().unwrap();
    let first = rpc(&client, "open_project_index", json!({"projectId":pid}))["chapters"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    rpc(
        &client,
        "save_scene",
        json!({"sceneId":sid,"scene":scene("Moving scene مِ 中文"),"parent":project["scene"]["revisionId"]}),
    );
    let b = rpc(
        &client,
        "create_chapter",
        json!({"projectId":pid,"title":"Destination B"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let c = rpc(
        &client,
        "create_chapter",
        json!({"projectId":pid,"title":"Destination C"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    rpc(
        &client,
        "create_scene",
        json!({"chapterId":b,"scene":scene("B scene")}),
    );
    let c_first = rpc(
        &client,
        "create_scene",
        json!({"chapterId":c,"scene":scene("C first")}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let c_second = rpc(
        &client,
        "create_scene",
        json!({"chapterId":c,"scene":scene("C second")}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    replicate(&client, &peer, pid, 0);
    let baseline = rpc(&peer, "sync_cursor", json!({}))["sequence"]
        .as_i64()
        .unwrap();
    rpc(
        &client,
        "reorder_chapters",
        json!({"projectId":pid,"ids":[b,first,c]}),
    );
    rpc(
        &peer,
        "reorder_chapters",
        json!({"projectId":pid,"ids":[c,first,b]}),
    );
    replicate(&peer, &client, pid, baseline);
    assert_eq!(heads(&client, pid, pid, "project").len(), 2);
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    a.metadata_conflicts();
    wait(|| {
        a.current_widget().is_some_and(|w| {
            combos(&w).len() == 2 && combos(&w)[1].active_id().is_some() && apply(&w).is_sensitive()
        })
    });
    let widget = a.current_widget().unwrap();
    assert!(!label_contains(&widget, pid));
    // Unseen competing resolutions invalidate the exact arrangement snapshot.
    let capture = rpc(
        &a.client,
        "metadata_resolution_snapshot",
        json!({"projectId":pid,"entityId":pid,"entityType":"project"}),
    );
    let selected = capture["heads"][0].as_str().unwrap();
    rpc(
        &a.client,
        "resolve_metadata_checked",
        json!({"snapshot":capture,"payload":revision(&a.client,pid,pid,"project",selected)["payload"]}),
    );
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    apply(&widget).clicked();
    wait(|| label_contains(&widget, "changed after this preview"));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    replicate(&a.client, &peer, pid, 0);
    let baseline = rpc(&peer, "sync_cursor", json!({}))["sequence"]
        .as_i64()
        .unwrap();
    let source = rpc(
        &a.client,
        "scene_arrangement_snapshot",
        json!({"projectId":pid,"sceneId":sid}),
    );
    rpc(
        &a.client,
        "arrange_scene",
        json!({"snapshot":source,"targetChapterId":b,"position":0}),
    );
    let source = rpc(
        &peer,
        "scene_arrangement_snapshot",
        json!({"projectId":pid,"sceneId":sid}),
    );
    rpc(
        &peer,
        "arrange_scene",
        json!({"snapshot":source,"targetChapterId":c,"position":0}),
    );
    rpc(
        &peer,
        "reorder_scenes",
        json!({"chapterId":c,"ids":[c_second,c_first,sid]}),
    );
    replicate(&peer, &a.client, pid, baseline);
    assert_eq!(heads(&a.client, pid, sid, "scenePlacement").len(), 2);
    assert_eq!(heads(&a.client, pid, &c, "chapter").len(), 1);
    button_named(&widget, "Refresh arrangements")
        .unwrap()
        .clicked();
    wait(|| combos(&widget)[0].active_id().is_some());
    choose_object(&widget, "chapter", &c);
    assert!(!apply(&widget).is_sensitive());
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    apply(&widget).clicked();
    assert!(label_contains(
        &widget,
        "Resolve competing scene locations first"
    ));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    choose_object(&widget, "scenePlacement", sid);
    wait(|| apply(&widget).is_sensitive());
    let destination = heads(&a.client, pid, sid, "scenePlacement")
        .into_iter()
        .find(|id| revision(&a.client, pid, sid, "scenePlacement", id)["payload"]["chapterId"] == c)
        .unwrap();
    choose_version(&widget, &destination);
    let child = a.spawn_window();
    child.open_scene(sid, false);
    wait(|| {
        child
            .active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let manuscript = child.active.borrow().clone().unwrap();
    manuscript.operation(json!({"type":"text","text":"Retain draft مِ 中文 "}), true);
    wait(|| manuscript.pending.get() == 0 && manuscript.dirty.get());
    let expected = manuscript.frame.borrow()["document"].clone();
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    child
        .desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    apply(&widget).clicked();
    wait(|| label_contains(&widget, "Arrangement choice cancelled"));
    assert!(manuscript.dirty.get());
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    child
        .desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Save);
    apply(&widget).clicked();
    wait(|| label_contains(&widget, "Arrangement choice saved."));
    assert_eq!(heads(&a.client, pid, sid, "scenePlacement").len(), 1);
    let saved = rpc(
        &a.client,
        "load_project_scene",
        json!({"projectId":pid,"sceneId":sid}),
    );
    assert_eq!(saved["scene"]["document"], expected);
    assert_eq!(saved["chapterId"], c);
    child.close();
    wait(|| child.desk.is_closed());
    // The deferred one-head chapter order becomes an explicit replay, including after detach.
    a.detach_current();
    let detached = desk::workspace_for_widget(&widget).unwrap();
    a.close();
    wait(|| a.desk.is_closed());
    button_named(&widget, "Refresh arrangements")
        .unwrap()
        .clicked();
    wait(|| combos(&widget)[0].active_id().is_some());
    choose_object(&widget, "chapter", &c);
    wait(|| {
        button_named(&widget, "Apply retained chapter order").is_some_and(|b| b.is_sensitive())
    });
    apply(&widget).clicked();
    wait(|| label_contains(&widget, "Arrangement choice saved."));
    let result = rpc(
        &detached.client,
        "open_project_index",
        json!({"projectId":pid}),
    );
    let ordered = result["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["chapterId"] == c)
        .map(|s| s["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(ordered, [c_second, c_first, sid.to_owned()]);
    let after = rpc(&detached.client, "load_scene", json!({"sceneId":sid}));
    assert_eq!(after["scene"], saved["scene"]);
    assert_eq!(after["revisionId"], saved["revisionId"]);
    // B's old retained order listed the scene now settled in C. Explicitly
    // acknowledge B's checked current membership rather than changing that head.
    button_named(&widget, "Refresh arrangements")
        .unwrap()
        .clicked();
    choose_object(&widget, "chapter", &b);
    wait(|| {
        button_named(&widget, "Keep current chapter membership and order")
            .is_some_and(|b| b.is_sensitive())
    });
    button_named(&widget, "Keep current chapter membership and order")
        .unwrap()
        .clicked();
    wait(|| label_contains(&widget, "Arrangement choice saved."));
    let b_snapshot = rpc(
        &detached.client,
        "metadata_resolution_snapshot",
        json!({"projectId":pid,"entityId":b,"entityType":"chapter"}),
    );
    assert_eq!(b_snapshot["heads"].as_array().unwrap().len(), 1);
    let b_head = b_snapshot["heads"][0].as_str().unwrap();
    assert_eq!(
        revision(&detached.client, pid, &b, "chapter", b_head)["payload"],
        b_snapshot["projection"]
    );
    button_named(&widget, "Refresh arrangements")
        .unwrap()
        .clicked();
    wait(|| combos(&widget)[0].active_id().is_some());
    choose_object(&widget, "chapter", &first);
    wait(|| apply(&widget).is_sensitive());
    apply(&widget).clicked();
    wait(|| label_contains(&widget, "Arrangement choice saved."));
    assert_eq!(heads(&detached.client, pid, &first, "chapter").len(), 1);
    assert_eq!(
        rpc(
            &detached.client,
            "scene_arrangement_snapshot",
            json!({"projectId":pid,"sceneId":sid})
        )["sourceChapterId"],
        c
    );
    detached.close();
    wait(|| detached.desk.is_closed());
    peer.stop();
}
