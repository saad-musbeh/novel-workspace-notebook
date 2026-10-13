use super::*;
fn saved() -> Value {
    json!({"id":"synthetic-scene","revisionId":"synthetic-revision","scene":{"schemaVersion":1,"title":"Synthetic","document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"A saved source مِ 中文 𠀀"}]}]}}})
}
fn request() -> Value {
    frozen_request(
        "synthetic-project",
        "synthetic-scene",
        &saved(),
        json!("a".repeat(64)),
        "llm",
        "ar",
        "synthetic-job",
        "2026-10-13T00:00:00Z",
    )
    .unwrap()
}
fn result(saved: &Value, hash: &Value) -> Value {
    json!({"suggestions":[{"id":"mock-host-result","origin":"llm","title":"Synthetic local result","explanation":"Mock backend data; no inference","status":"pending","sources":[{"sceneId":saved["id"],"revisionId":saved["revisionId"],"hash":hash}],"evidence":[{"sceneId":saved["id"],"revisionId":saved["revisionId"],"hash":hash,"offsetUnit":"utf16","start":0,"end":1,"quote":"A","prefix":"","suffix":""}],"action":{"kind":"replace","replacement":"Author preview مِ 中文 𠀀"}}]})
}
#[test]
fn provider_configuration_refuses_external_or_credentialed_endpoints() {
    let mut snapshot = json!({"payload":crate::tools::initial("settings")});
    assert_eq!(
        configuration(&snapshot, "llm").unwrap()["endpoint"],
        "http://127.0.0.1:8080"
    );
    for endpoint in [
        "https://example.invalid",
        "http://localhost:8080",
        "http://127.0.0.1.evil",
        "http://user@127.0.0.1:8080",
        "http://127.0.0.1:8080/v1",
        "http://127.0.0.1:0",
        "http://127.0.0.1:70000",
    ] {
        snapshot["payload"]["assistance"]["providers"][1]["endpoint"] = json!(endpoint);
        assert!(configuration(&snapshot, "llm").is_err());
    }
    assert!(endpoint_is_local("http://[::1]:8080/"));
    assert!(configuration(&Value::Null, "unsupported").is_err());
}
#[test]
fn request_preserves_exact_saved_utf8_text_and_explicit_choices() {
    let request = request();
    assert_eq!(request["provider"], "llm");
    assert_eq!(request["language"], "ar");
    assert_eq!(request["operation"], "wording");
    assert_eq!(
        request["sources"][0]["text"],
        novel_storage::plain_text(&saved()["scene"]["document"])
    );
    assert_eq!(request["sources"][0]["revisionId"], "synthetic-revision");
    assert!(frozen_request(
        "synthetic-project",
        "another-scene",
        &saved(),
        json!("a".repeat(64)),
        "llm",
        "en",
        "j",
        "t"
    )
    .is_err());
    assert!(frozen_request(
        "synthetic-project",
        "synthetic-scene",
        &saved(),
        json!("A".repeat(64)),
        "llm",
        "en",
        "j",
        "t"
    )
    .is_err());
    let mut huge = saved();
    huge["scene"]["document"]["content"][0]["content"][0]["text"] = json!("𠀀".repeat(25001));
    assert!(frozen_request(
        "synthetic-project",
        "synthetic-scene",
        &huge,
        json!("a".repeat(64)),
        "llm",
        "en",
        "j",
        "t"
    )
    .is_err());
}
#[test]
fn provider_results_remain_pending_and_within_captured_scope() {
    let request = request();
    let result = result(&saved(), &json!("a".repeat(64)));
    assert!(result_within_scope(&result, &request).is_ok());
    for (key, value) in [
        ("sceneId", json!("another-scene")),
        ("revisionId", json!("different-revision")),
        ("hash", json!("b".repeat(64))),
    ] {
        let mut forged = result.clone();
        for collection in ["sources", "evidence"] {
            forged["suggestions"][0][collection][0][key] = value.clone();
        }
        assert!(result_within_scope(&forged, &request).is_err());
    }
    let mut accepted = result;
    accepted["suggestions"][0]["status"] = json!("accepted");
    assert!(result_within_scope(&accepted, &request).is_err());
}

fn wait(condition: impl Fn() -> bool) {
    let context = glib::MainContext::default();
    let start = Instant::now();
    while !condition() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "Native review host fixture timed out"
        );
        std::thread::sleep(Duration::from_millis(8));
    }
    while context.pending() {
        context.iteration(false);
    }
}
fn rpc(client: &Process, op: &str, args: Value) -> Value {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    client.rpc(op, args, move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some());
    let value = result.borrow_mut().take().unwrap().unwrap();
    value
}
fn answer(title: &str, id: &str) {
    let title = title.to_owned();
    let id = id.to_owned();
    let start = Instant::now();
    glib::timeout_add_local(Duration::from_millis(20), move || {
        let dialog = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Dialog>().ok())
            .find(|w| w.is_visible() && w.title().as_deref() == Some(title.as_str()));
        if let Some(dialog) = dialog {
            eprintln!("Answering review fixture dialog: {title} -> {id}");
            let combo = dialog
                .content_area()
                .children()
                .into_iter()
                .find_map(|w| w.downcast::<gtk::ComboBoxText>().ok())
                .unwrap();
            combo.set_active_id(Some(&id));
            dialog.response(gtk::ResponseType::Accept);
            glib::ControlFlow::Break
        } else {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "Expected explicit review choice dialog {title}; windows: {:?}", gtk::Window::list_toplevels().into_iter().filter_map(|w| w.downcast::<gtk::Window>().ok()).map(|w|(w.title(),w.is_visible())).collect::<Vec<_>>()
            );
            glib::ControlFlow::Continue
        }
    });
}
fn choices() {
    answer("Review with", "languagetool");
    answer("Review language", "ar");
}
/// Call from the parent's already-initialized, single-thread GTK suite. This
/// creates its own synthetic store and actual stdio workers. provider_run is
/// replaced by test-only delayed mock replies, never a model or network request.
/// Root must first register both page Draft types and ownership/release hooks.
pub(crate) fn run(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("review-host-data");
    let cache = fixture.path().join("review-host-cache");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let companion = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(companion), &data, true).unwrap();
    let project = rpc(
        &client,
        "create_project",
        json!({"title":"Synthetic review host مِ 中文"}),
    );
    let chapter = rpc(
        &client,
        "create_chapter",
        json!({"projectId":project["id"],"title":"First"}),
    );
    let record = rpc(
        &client,
        "create_scene",
        json!({"chapterId":chapter["id"],"scene":saved()["scene"]}),
    );
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let p = a.active.borrow().clone().unwrap();
    assert_eq!(p.scene_id, record["id"].as_str().unwrap());
    let before = rpc(&a.client, "history", json!({"sceneId":p.scene_id}));
    a.review_decisions();
    wait(|| {
        a.review_decision_pages
            .borrow()
            .iter()
            .any(|p| p.loaded.get())
    });
    assert_eq!(a.review_decision_pages.borrow().len(), 1);
    let review = a.review_decision_pages.borrow()[0].clone();
    assert!(a.pages.page_num(&review.widget).is_some());
    wait(|| p.pending.get() == 0 && !p.saving.get());
    a.new_translation();
    wait(|| {
        a.translation_preview_pages
            .borrow()
            .iter()
            .any(|p| p.loaded.get())
    });
    assert_eq!(a.translation_preview_pages.borrow().len(), 1);
    let translator = a.translation_preview_pages.borrow()[0].clone();
    assert!(a.pages.page_num(&translator.widget).is_some());
    let counts = (
        a.review_decision_pages.borrow().len(),
        a.translation_preview_pages.borrow().len(),
    );
    p.composing.set(true);
    a.review_decisions();
    a.translation_preview();
    p.composing.set(false);
    p.pending.set(1);
    a.translation_preview();
    p.pending.set(0);
    assert_eq!(
        (
            a.review_decision_pages.borrow().len(),
            a.translation_preview_pages.borrow().len()
        ),
        counts
    );
    assert_eq!(
        rpc(&a.client, "history", json!({"sceneId":p.scene_id})),
        before
    );
    // The already-delivered module fixtures use this store and writing session.
    super::super::review_decisions::tests::run(&a, &p);
    super::super::translation_preview::tests::run(&a, &p);
    // Cancelling ordinary manual drain must not create a generation or new page.
    p.operation(json!({"type":"text","text":"unsaved writing 𠀀"}), true);
    wait(|| p.pending.get() == 0 && p.dirty.get());
    let count = a.review_decision_pages.borrow().len();
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    choices();
    a.run_local_review();
    assert_eq!(a.review_decision_pages.borrow().len(), count);
    assert!(!pending_review(project["id"].as_str().unwrap()));
    assert_eq!(
        rpc(&a.client, "history", json!({"sceneId":p.scene_id})),
        before
    );
    let done = Rc::new(Cell::new(false));
    let out = done.clone();
    p.discard(move |r| {
        r.unwrap();
        out.set(true);
    });
    wait(|| done.get());
    let current = rpc(
        &a.client,
        "load_project_scene",
        json!({"projectId":project["id"],"sceneId":p.scene_id}),
    );
    let hash = rpc(
        &a.client,
        "text_hash",
        json!({"text":novel_storage::plain_text(&current["scene"]["document"])}),
    );
    // Delayed optional inference is not a manual-drain barrier. It follows its
    // mounted widget into a detached window, even after its origin closes.
    MOCK_PROVIDER_REPLY.with(|cell| {
        *cell.borrow_mut() = Some((Ok(result(&current, &hash)), Duration::from_secs(2)))
    });
    choices();
    a.run_local_review();
    let page = a.review_decision_pages.borrow().last().unwrap().clone();
    let widget = page.widget.clone().upcast::<gtk::Widget>();
    let run = REVIEW_RUNS.with(|runs| runs.borrow().get(&widget).unwrap().clone());
    wait(|| !run.request.borrow().is_null() || !run.active.get());
    assert!(run.active.get());
    assert_eq!(run.request.borrow()["provider"], "languagetool");
    assert_eq!(run.request.borrow()["language"], "ar");
    assert!(!page.saving.get());
    let drained = Rc::new(RefCell::new(None));
    let out = drained.clone();
    a.drain(move |r| *out.borrow_mut() = Some(r));
    wait(|| drained.borrow().is_some());
    drained.borrow_mut().take().unwrap().unwrap();
    assert!(run.active.get());
    a.detach_current();
    let child = desk::workspace_for_widget(&widget).unwrap();
    assert!(!Rc::ptr_eq(&a, &child));
    assert!(child
        .review_decision_pages
        .borrow()
        .iter()
        .any(|p| Rc::ptr_eq(p, &page)));
    a.close();
    wait(|| a.desk.is_closed());
    wait(|| !run.active.get() && !page.saving.get());
    assert!(!page.dirty.get());
    assert!(child.pages.page_num(&widget).is_some());
    assert_eq!(
        rpc(&child.client, "history", json!({"sceneId":p.scene_id})),
        before
    );
    let metadata = rpc(
        &child.client,
        "load_project_data_unambiguous",
        json!({"projectId":project["id"],"kind":"settings"}),
    );
    assert!(metadata["payload"]["assistance"]["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["title"] == "Synthetic local result"));
    // An accepted release invalidates its token synchronously. A late mock result
    // cannot append into a new project or remount an orphaned/closed page.
    let fresh = child.panes.borrow().get(&p.scene_id).unwrap().clone();
    wait(|| fresh.open.get() && fresh.pending.get() == 0);
    let prior = metadata["revisionId"].clone();
    let cancelled = begin_review(&child, &fresh, "llm".into(), "en".into()).unwrap();
    child.release_review_page(&cancelled.widget);
    assert!(!cancelled.active.get());
    cancelled.receive(Ok(result(&current, &hash)));
    assert_eq!(
        rpc(
            &child.client,
            "load_project_data_unambiguous",
            json!({"projectId":project["id"],"kind":"settings"})
        )["revisionId"],
        prior
    );
    child.cancel_review_generations();
    child.close();
    wait(|| child.desk.is_closed());
}
