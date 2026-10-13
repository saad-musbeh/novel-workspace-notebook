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

fn context() -> Value {
    let facts=json!({"schemaVersion":1,"records":[{"id":"character","name":"Mira"}],"facts":[{"id":"fact-1","subjectId":"character","property":"role","value":"Confirmed مِ 中文 𠀀","eventId":null,"untilEventId":null}]}).to_string();
    json!({"scene":saved(),"sources":[{"kind":"scene","sceneId":"synthetic-scene","revisionId":"synthetic-revision","hash":"a".repeat(64),"text":novel_storage::plain_text(&saved()["scene"]["document"])},{"kind":"facts","sceneId":"planning:synthetic-project","revisionId":"planning-revision","hash":"b".repeat(64),"text":facts}],"scope":{"includedSceneId":"synthetic-scene","excludedSceneCount":4,"includedFactIds":["fact-1"],"excludedFacts":[{"id":"belief","reason":"Not canonical"}],"timingIncluded":false}})
}
fn consistency_result(request: &Value) -> Value {
    let mut result = result(&saved(), &request["sources"][0]["hash"]);
    result["suggestions"][0]["action"] = json!({"kind":"question"});
    result["suggestions"][0]["sources"] = json!(request["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(source_ref)
        .collect::<Vec<_>>());
    result
}
#[test]
fn consistency_request_is_exact_two_source_question_scope_and_never_whole_book() {
    let context = context();
    let request = consistency_request(
        "synthetic-project",
        "synthetic-scene",
        &context,
        "ar",
        "j",
        "t",
    )
    .unwrap();
    assert_eq!(request["provider"], "llm");
    assert_eq!(request["operation"], "consistency");
    assert_eq!(request["sources"].as_array().unwrap().len(), 2);
    assert_eq!(request["sources"][1]["kind"], "facts");
    assert_eq!(request["sources"][1]["language"], "und");
    assert_eq!(request["sources"][1]["text"], context["sources"][1]["text"]);
    assert_eq!(request["scope"], context["scope"]);
    let mut empty = context.clone();
    empty["scope"]["includedFactIds"] = json!([]);
    empty["sources"][1]["text"] =
        json!(json!({"schemaVersion":1,"records":[],"facts":[]}).to_string());
    assert!(consistency_request(
        "synthetic-project",
        "synthetic-scene",
        &empty,
        "ar",
        "j",
        "t"
    )
    .is_ok());
    let mut bad = context.clone();
    bad["sources"][1]["sceneId"] = json!("planning:foreign");
    assert!(
        consistency_request("synthetic-project", "synthetic-scene", &bad, "ar", "j", "t").is_err()
    );
    let mut bad = context;
    bad["scope"]["includedFactIds"] = json!(["a proposal"]);
    assert!(
        consistency_request("synthetic-project", "synthetic-scene", &bad, "ar", "j", "t").is_err()
    );
}
#[test]
fn consistency_requires_question_manuscript_evidence_and_every_captured_source() {
    let request = consistency_request(
        "synthetic-project",
        "synthetic-scene",
        &context(),
        "ar",
        "j",
        "t",
    )
    .unwrap();
    let result = consistency_result(&request);
    assert!(result_within_scope(&result, &request).is_ok());
    for action in [
        json!({"kind":"replace","replacement":"Auto edit"}),
        json!({"kind":"fact","subject":"Mira","property":"role","value":"Promote claim"}),
    ] {
        let mut wrong = result.clone();
        wrong["suggestions"][0]["action"] = action;
        assert!(result_within_scope(&wrong, &request).is_err());
    }
    let mut incomplete = result.clone();
    incomplete["suggestions"][0]["sources"] = json!([source_ref(&request["sources"][0])]);
    assert!(result_within_scope(&incomplete, &request).is_err());
    let mut facts_only = result;
    facts_only["suggestions"][0]["evidence"] = json!([]);
    assert!(result_within_scope(&facts_only, &request).is_err());
}
#[test]
fn captured_exceptions_suppress_only_valid_exact_proofreading_quote_and_language() {
    let request = request();
    let mut proofreading = result(&saved(), &json!("a".repeat(64)));
    proofreading["suggestions"][0]["origin"] = json!("proofreading");
    let mut settings = crate::tools::initial("settings");
    settings["assistance"]["exceptions"] =
        json!([{"term":"A","language":"ar","reason":"Author spelling"}]);
    assert!(
        checked_provider_result(&proofreading, &request, &settings).unwrap()["suggestions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let mut other_language = request.clone();
    other_language["language"] = json!("en");
    assert_eq!(
        checked_provider_result(&proofreading, &other_language, &settings).unwrap()["suggestions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let llm = result(&saved(), &json!("a".repeat(64)));
    assert_eq!(
        checked_provider_result(&llm, &request, &settings).unwrap(),
        llm
    );
    let mut context_quote = proofreading.clone();
    context_quote["suggestions"][0]["evidence"][0]["quote"] = json!("A ");
    context_quote["suggestions"][0]["evidence"][0]["end"] = json!(2);
    assert_eq!(
        checked_provider_result(&context_quote, &request, &settings).unwrap()["suggestions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // Both malicious results WOULD match an exception; original validation must
    // refuse before filtering can turn either into an apparently empty result.
    let mut malformed = proofreading.clone();
    malformed["suggestions"][0]["evidence"][0]["start"] = json!(1);
    malformed["suggestions"][0]["evidence"][0]["end"] = json!(2);
    assert!(checked_provider_result(&malformed, &request, &settings).is_err());
    let mut foreign = proofreading;
    foreign["suggestions"][0]["sources"][0]["hash"] = json!("c".repeat(64));
    foreign["suggestions"][0]["evidence"][0]["hash"] = json!("c".repeat(64));
    assert!(checked_provider_result(&foreign, &request, &settings).is_err());
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
                "Expected explicit review choice dialog {title}; windows: {:?}",
                gtk::Window::list_toplevels()
                    .into_iter()
                    .filter_map(|w| w.downcast::<gtk::Window>().ok())
                    .map(|w| (w.title(), w.is_visible()))
                    .collect::<Vec<_>>()
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

fn answer_scope(response: gtk::ResponseType) {
    let start = Instant::now();
    glib::timeout_add_local(Duration::from_millis(20), move || {
        let dialog = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Dialog>().ok())
            .find(|w| {
                w.is_visible()
                    && w.title().as_deref() == Some("Confirm one-scene consistency scope")
            });
        if let Some(dialog) = dialog {
            dialog.response(response);
            glib::ControlFlow::Break
        } else {
            assert!(start.elapsed() < Duration::from_secs(20));
            glib::ControlFlow::Continue
        }
    });
}
fn control(widget: &impl IsA<gtk::Widget>, label: &str) -> gtk::Button {
    fn find(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
        if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
            if button.label().as_deref() == Some(label) {
                return Some(button);
            }
        }
        if let Ok(container) = widget.clone().downcast::<gtk::Container>() {
            for child in container.children() {
                if let Some(button) = find(&child, label) {
                    return Some(button);
                }
            }
        }
        None
    }
    find(widget.as_ref(), label).expect("Review decision control exists")
}
fn snapshot_question(snapshot: &Value) -> Value {
    let mut proposal = result(&snapshot["scene"], &snapshot["sources"][0]["hash"]);
    proposal["suggestions"][0]["action"] = json!({"kind":"question"});
    proposal["suggestions"][0]["title"] = json!("Provisional one-scene question");
    proposal["suggestions"][0]["sources"] = json!(snapshot["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(source_ref)
        .collect::<Vec<_>>());
    proposal
}
/// Separate callable synthetic GTK workflow. Root wires both context RPCs and
/// shared canonical projection first. This function never calls a model/server.
pub(crate) fn run_consistency(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("consistency-data");
    let cache = fixture.path().join("consistency-cache");
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
        json!({"title":"Synthetic consistency"}),
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
    rpc(
        &client,
        "create_scene",
        json!({"chapterId":chapter["id"],"scene":{"schemaVersion":1,"title":"Excluded scene","document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"This scene must never enter the request."}]}]}}}),
    );
    let mut planning = crate::tools::initial("planning");
    planning["records"] =
        json!([{"id":"mira","name":"Mira","type":"character","aliases":[],"fields":{},"links":[]}]);
    planning["facts"] = json!([
        {"id":"approved-role","subjectId":"mira","property":"role","value":"Pilot مِ 中文 𠀀","status":"fact","confirmed":true,"eventId":null,"untilEventId":null},
        {"id":"belief","subjectId":"mira","property":"rumor","value":"Unverified rumor must stay outside context","status":"rumor","confirmed":false,"eventId":null,"untilEventId":null},
        {"id":"proposal","subjectId":"mira","property":"role","value":"Proposal not canon","status":"fact","confirmed":false,"eventId":null,"untilEventId":null}]);
    let planning_saved = rpc(
        &client,
        "save_project_data_checked",
        json!({"projectId":project["id"],"kind":"planning","payload":planning,"parent":null,"dependencies":[]}),
    );
    let snapshot = rpc(
        &client,
        "review_context_snapshot",
        json!({"projectId":project["id"],"sceneId":record["id"]}),
    );
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let pane = a.active.borrow().clone().unwrap();
    let history = rpc(&a.client, "history", json!({"sceneId":record["id"]}));
    MOCK_PROVIDER_REPLY.with(|cell| {
        *cell.borrow_mut() = Some((Ok(snapshot_question(&snapshot)), Duration::from_millis(250)))
    });
    answer("Review with", "llm");
    answer("Local review operation", "consistency");
    answer("Review language", "en");
    answer_scope(gtk::ResponseType::Accept);
    a.run_local_review();
    let page = a.review_decision_pages.borrow().last().unwrap().clone();
    let run = REVIEW_RUNS.with(|runs| {
        runs.borrow()
            .get(page.widget.upcast_ref::<gtk::Widget>())
            .unwrap()
            .clone()
    });
    wait(|| !run.request.borrow().is_null() || !run.active.get());
    assert!(run.active.get());
    assert_eq!(run.request.borrow()["operation"], "consistency");
    assert_eq!(run.request.borrow()["sources"].as_array().unwrap().len(), 2);
    assert_eq!(run.request.borrow()["scope"]["excludedSceneCount"], 1);
    assert_eq!(
        run.request.borrow()["scope"]["excludedFacts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(!run.request.borrow()["sources"][1]["text"]
        .as_str()
        .unwrap()
        .contains("Unverified rumor"));
    wait(|| !run.active.get() && !page.saving.get());
    assert!(!page.dirty.get());
    assert!(!pane.dirty.get());
    let metadata = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":project["id"],"kind":"settings"}),
    );
    let question = metadata["payload"]["assistance"]["suggestions"][0].clone();
    assert_eq!(question["reviewOperation"], "consistency");
    assert_eq!(question["sources"][1]["kind"], "facts");
    assert_eq!(question["action"]["kind"], "question");
    control(&page.widget, "Accept question as review note").clicked();
    wait(|| !page.saving.get());
    assert!(!page.dirty.get());
    assert!(page.status.text().contains("Provisional review note"));
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":project["id"],"kind":"planning"})
        ),
        planning_saved
    );
    assert_eq!(
        rpc(&a.client, "history", json!({"sceneId":record["id"]})),
        history
    );
    assert_eq!(pane.frame.borrow()["document"], record["scene"]["document"]);
    // Even an unrelated planning edit with identical canonical projection bytes
    // changes the captured full head and must refuse a consistency decision.
    page.append_result(&a, &json!({"suggestions":[question]}))
        .unwrap();
    wait(|| !page.saving.get());
    page.choices.set_active(Some(1));
    planning["boards"][0]["name"] = json!("Unrelated authored board name");
    rpc(
        &a.client,
        "save_project_data_checked",
        json!({"projectId":project["id"],"kind":"planning","payload":planning,"parent":planning_saved["revisionId"],"dependencies":[]}),
    );
    control(&page.widget, "Reject selected proposal").clicked();
    wait(|| !page.saving.get());
    assert!(!page.dirty.get());
    assert!(page.status.text().contains("changed"));
    let current = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":project["id"],"kind":"settings"}),
    );
    assert_eq!(
        current["payload"]["assistance"]["suggestions"][1]["status"],
        "pending"
    );
    // Explicitly cancelled delayed consistency output cannot append later.
    let snapshot = rpc(
        &a.client,
        "review_context_snapshot",
        json!({"projectId":project["id"],"sceneId":record["id"]}),
    );
    MOCK_PROVIDER_REPLY.with(|cell| {
        *cell.borrow_mut() = Some((Ok(snapshot_question(&snapshot)), Duration::from_secs(2)))
    });
    answer_scope(gtk::ResponseType::Accept);
    let cancelled =
        begin_review_operation(&a, &pane, "llm".into(), "en".into(), "consistency".into()).unwrap();
    wait(|| !cancelled.request.borrow().is_null() || !cancelled.active.get());
    assert!(cancelled.active.get());
    a.release_review_page(&cancelled.widget);
    cancelled.receive(Ok(snapshot_question(&snapshot)));
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":project["id"],"kind":"settings"})
        )["revisionId"],
        current["revisionId"]
    );
    assert_eq!(
        rpc(&a.client, "history", json!({"sceneId":record["id"]})),
        history
    );
    a.cancel_review_generations();
    a.close();
    wait(|| a.desk.is_closed());
}
