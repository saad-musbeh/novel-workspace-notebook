use super::*;

fn combos(widget: &impl IsA<gtk::Widget>) -> Vec<gtk::ComboBoxText> {
    if let Ok(combo) = widget.as_ref().clone().downcast::<gtk::ComboBoxText>() {
        return vec![combo];
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .map(|c| c.children().iter().flat_map(combos).collect())
        .unwrap_or_default()
}
fn contains_label(widget: &impl IsA<gtk::Widget>, text: &str) -> bool {
    if let Ok(label) = widget.as_ref().clone().downcast::<gtk::Label>() {
        if label.text().contains(text) {
            return true;
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .is_some_and(|c| c.children().iter().any(|w| contains_label(w, text)))
}
fn contains_preview(widget: &impl IsA<gtk::Widget>, text: &str) -> bool {
    if let Ok(view) = widget.as_ref().clone().downcast::<gtk::TextView>() {
        let b = view.buffer().unwrap();
        if b.text(&b.start_iter(), &b.end_iter(), true)
            .is_some_and(|v| v.contains(text))
        {
            return true;
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .is_some_and(|c| c.children().iter().any(|w| contains_preview(w, text)))
}
fn replicate(from: &Process, to: &Process, pid: &str, after: i64) {
    let cursor = rpc(from, "sync_cursor", json!({}));
    let local = rpc(to, "sync_cursor", json!({}));
    let revisions = rpc(
        from,
        "pull_revisions",
        json!({"projectId":pid,"cursor":{"epoch":cursor["epoch"],"sequence":after},"limit":1000}),
    );
    rpc(
        to,
        "ingest_revisions",
        json!({"projectId":pid,"expectedEpoch":local["epoch"],"remoteEpoch":cursor["epoch"],"manifest":rpc(from,"open_project",json!({"projectId":pid})),"revisions":revisions["revisions"]}),
    );
}
fn conflict(
    primary: &Process,
    peer: &Process,
    pid: &str,
    kind: &str,
    base: Value,
    left: Value,
    right: Value,
) -> (String, String) {
    let saved = rpc(
        primary,
        "save_project_data",
        json!({"projectId":pid,"kind":kind,"payload":base,"parent":null}),
    );
    replicate(primary, peer, pid, 0);
    let before = rpc(peer, "sync_cursor", json!({}))["sequence"]
        .as_i64()
        .unwrap();
    let left = rpc(
        primary,
        "save_project_data",
        json!({"projectId":pid,"kind":kind,"payload":left,"parent":saved["revisionId"]}),
    );
    let right = rpc(
        peer,
        "save_project_data",
        json!({"projectId":pid,"kind":kind,"payload":right,"parent":saved["revisionId"]}),
    );
    replicate(peer, primary, pid, before);
    assert_eq!(
        rpc(
            primary,
            "project_data_heads",
            json!({"projectId":pid,"kind":kind})
        )["revisionIds"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    (
        left["revisionId"].as_str().unwrap().to_owned(),
        right["revisionId"].as_str().unwrap().to_owned(),
    )
}
fn page(a: &Rc<Workspace>, kind: &str) -> gtk::Widget {
    a.data_conflicts();
    wait(|| {
        a.current_widget()
            .is_some_and(|w| has_label(&w, "Project data conflicts"))
    });
    let widget = a.current_widget().unwrap();
    let controls = combos(&widget);
    assert_eq!(controls.len(), 2);
    controls[0].set_active_id(Some(kind));
    wait(|| {
        contains_label(&widget, "competing versions.")
            && find_button(&widget, "Use this retained version")
                .unwrap()
                .is_sensitive()
    });
    widget
}
fn choose(widget: &gtk::Widget, rid: &str, text: &str) {
    combos(widget)[1].set_active_id(Some(rid));
    wait(|| {
        contains_preview(widget, text)
            && find_button(widget, "Use this retained version")
                .unwrap()
                .is_sensitive()
    });
}
fn snapshot(client: &Process, pid: &str, kind: &str) -> Value {
    rpc(
        client,
        "load_project_data",
        json!({"projectId":pid,"kind":kind}),
    )
}
fn tools(sid: &str, note: &str) -> Value {
    json!({"schemaVersion":1,"scraps":[],"reactions":[],"bookmarks":[],"notes":[{"id":"note","sceneId":sid,"text":note,"resolved":false}]})
}
pub(super) fn run(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("conflict-data");
    let peer_data = fixture.path().join("conflict-peer");
    let cache = fixture.path().join("conflict-cache");
    for p in [&data, &peer_data, &cache] {
        std::fs::create_dir_all(p).unwrap();
    }
    let companion = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(companion), &data, true).unwrap();
    let peer = Process::start(&binaries.join(companion), &peer_data, true).unwrap();
    let project = rpc(
        &client,
        "create_novel",
        json!({"title":"Synthetic retained version recovery"}),
    );
    let pid = project["project"]["id"].as_str().unwrap();
    let sid = project["scene"]["id"].as_str().unwrap();
    let local = "Local note مِ 中文 𠀀 👩🏽‍🚀";
    let remote = "Peer note مَرْحَبًا 世界 😀";
    let (left, _right) = conflict(
        &client,
        &peer,
        pid,
        "notebookTools",
        tools(sid, "Original"),
        tools(sid, local),
        tools(sid, remote),
    );
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let widget = page(&a, "notebookTools");
    wait(|| contains_preview(&widget, local) && contains_preview(&widget, remote));
    choose(&widget, &left, local);
    let before = rpc(
        &a.client,
        "project_data_heads",
        json!({"projectId":pid,"kind":"notebookTools"}),
    );
    let history = rpc(
        &a.client,
        "project_data_history",
        json!({"projectId":pid,"kind":"notebookTools"}),
    );
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    // A changed head set invalidates the old preview even when the chosen revision remains retained.
    rpc(
        &a.client,
        "resolve_project_data",
        json!({"projectId":pid,"kind":"notebookTools","heads":before["revisionIds"],"payload":tools(sid,remote)}),
    );
    let committed = rpc(&a.client, "sync_cursor", json!({}));
    find_button(&widget, "Use this retained version")
        .unwrap()
        .clicked();
    wait(|| contains_label(&widget, "heads changed"));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), committed);
    assert_eq!(
        snapshot(&a.client, pid, "notebookTools")["payload"],
        tools(sid, remote)
    );
    assert_ne!(committed, marker);
    assert_eq!(
        rpc(
            &a.client,
            "project_data_history",
            json!({"projectId":pid,"kind":"notebookTools"})
        )
        .as_array()
        .unwrap()
        .len(),
        history.as_array().unwrap().len() + 1
    );
    find_button(&widget, "Refresh versions").unwrap().clicked();
    wait(|| contains_label(&widget, "No competing versions."));
    assert!(!find_button(&widget, "Use this retained version")
        .unwrap()
        .is_sensitive());
    // A distinct planning conflict demonstrates saved-project decisions across windows.
    let base = json!({"schemaVersion":1,"records":[],"facts":[]});
    let l = json!({"schemaVersion":1,"records":[],"facts":[],"memo":"Planning local مِ 中文"});
    let r = json!({"schemaVersion":1,"records":[],"facts":[],"memo":"Planning peer 世界"});
    let (planning_left, _) = conflict(&a.client, &peer, pid, "planning", base, l.clone(), r);
    let widget = page(&a, "planning");
    choose(&widget, &planning_left, "Planning local");
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
    edit(&manuscript, "Keep exact unsaved مِ 中文 𠀀 👩🏽‍🚀");
    let expected = manuscript.frame.borrow()["document"].clone();
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    select(&child, SaveChoice::Cancel);
    find_button(&widget, "Use this retained version")
        .unwrap()
        .clicked();
    wait(|| contains_label(&widget, "Resolution cancelled"));
    assert!(manuscript.dirty.get());
    assert_eq!(manuscript.frame.borrow()["document"], expected);
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    // A dirty structured form receives the same cross-window decision.
    save(&child, &manuscript);
    child.open_data("settings", "Review settings");
    let form = child.page("settings");
    wait(|| form.loaded.get() && child.current_widget() == Some(form.widget.clone().upcast()));
    form.value.borrow_mut()["memo"] = json!("Keep pending form مِ 中文");
    form.dirty.set(true);
    form.epoch.set(form.epoch.get() + 1);
    let pending_form = form.value.borrow().clone();
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    select(&child, SaveChoice::Cancel);
    find_button(&widget, "Use this retained version")
        .unwrap()
        .clicked();
    wait(|| contains_label(&widget, "Resolution cancelled"));
    assert!(form.dirty.get());
    assert_eq!(*form.value.borrow(), pending_form);
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    select(&child, SaveChoice::Save);
    find_button(&widget, "Use this retained version")
        .unwrap()
        .clicked();
    wait(|| contains_label(&widget, "Version choice saved."));
    assert!(!form.dirty.get());
    assert_eq!(
        snapshot(&a.client, pid, "settings")["payload"],
        pending_form
    );
    assert!(!manuscript.dirty.get());
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":sid}))["scene"]["document"],
        expected
    );
    assert_eq!(snapshot(&a.client, pid, "planning")["payload"], l);
    assert!(!find_button(&widget, "Use this retained version")
        .unwrap()
        .is_sensitive());
    // Immutable versions from both branches must survive: choosing only one is refused atomically.
    let definition = |id: &str| json!({"schemaVersion":1,"id":id,"mappingVersion":1,"name":id});
    let alphabet =
        |id: &str| json!({"schemaVersion":1,"systems":[definition(id)],"assignments":[]});
    let (alphabet_left, _) = conflict(
        &a.client,
        &peer,
        pid,
        "writingSystems",
        json!({"schemaVersion":1,"systems":[],"assignments":[]}),
        alphabet("Left alphabet"),
        alphabet("Right alphabet"),
    );
    let widget = page(&a, "writingSystems");
    choose(&widget, &alphabet_left, "Left alphabet");
    let heads = rpc(
        &a.client,
        "project_data_heads",
        json!({"projectId":pid,"kind":"writingSystems"}),
    );
    let before = snapshot(&a.client, pid, "writingSystems");
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    find_button(&widget, "Use this retained version")
        .unwrap()
        .clicked();
    wait(|| contains_label(&widget, "immutable"));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    assert_eq!(snapshot(&a.client, pid, "writingSystems"), before);
    assert_eq!(
        rpc(
            &a.client,
            "project_data_heads",
            json!({"projectId":pid,"kind":"writingSystems"})
        ),
        heads
    );
    assert!(contains_preview(&widget, "Left alphabet"));
    // Tool pages keep routing to their new notebook owner, including after the original closes.
    a.detach_current();
    let detached = last_window();
    assert_eq!(detached.current_widget(), Some(widget.clone()));
    child.close();
    wait(|| windows() == 2);
    a.close();
    wait(|| windows() == 1);
    find_button(&widget, "Refresh versions").unwrap().clicked();
    wait(|| {
        find_button(&widget, "Use this retained version")
            .unwrap()
            .is_sensitive()
    });
    choose(&widget, &alphabet_left, "Left alphabet");
    find_button(&widget, "Use this retained version")
        .unwrap()
        .clicked();
    wait(|| contains_label(&widget, "immutable"));
    assert_eq!(
        rpc(
            &detached.client,
            "project_data_heads",
            json!({"projectId":pid,"kind":"writingSystems"})
        ),
        heads
    );
    capture(&detached, "notebook-project-data-conflicts.png");
    // Inspection includes every supported kind; protected branch lineage has no resolution action.
    assert!(combos(&widget)[0].set_active_id(Some("branchLineage")));
    wait(|| contains_label(&widget, "Branch lineage is immutable."));
    assert!(!find_button(&widget, "Use this retained version")
        .unwrap()
        .is_sensitive());
    detached.close();
    wait(|| windows() == 0);
    peer.stop();
}
