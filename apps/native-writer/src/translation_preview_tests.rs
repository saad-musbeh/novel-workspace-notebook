use super::*;
fn saved() -> Value {
    json!({"id":"synthetic-scene","revisionId":"captured-revision","scene":{"document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"مِ 𠀀 中文"}]}]}}})
}
#[test]
fn captures_exact_multilingual_immutable_source() {
    let draft = source_draft(&saved(), "ar", "zh-Hant", "draft-1").unwrap();
    assert_eq!(draft["sourceText"], "مِ 𠀀 中文");
    assert_eq!(draft["sceneRevisionId"], "captured-revision");
    assert_eq!(draft["text"], "");
    let mut doc = crate::tools::initial("sceneTranslations");
    doc = append_draft(&doc, draft.clone()).unwrap();
    let next = edit_draft(&doc, "draft-1", "Author 中文 𠀀").unwrap();
    for k in [
        "sceneId",
        "sceneRevisionId",
        "sourceText",
        "sourceLanguage",
        "targetLanguage",
    ] {
        assert_eq!(next["drafts"][0][k], draft[k]);
    }
}
#[test]
fn new_translation_keeps_older_drafts_and_writing() {
    let saved = saved();
    let initial = crate::tools::initial("sceneTranslations");
    let first = append_draft(
        &initial,
        source_draft(&saved, "ar", "en", "draft-1").unwrap(),
    )
    .unwrap();
    let edited = edit_draft(&first, "draft-1", "My exact earlier wording").unwrap();
    let mut newer = saved.clone();
    newer["revisionId"] = json!("newer-revision");
    newer["scene"]["document"]["content"][0]["content"][0]["text"] =
        json!("Different saved original");
    let next = append_draft(
        &edited,
        source_draft(&newer, "en", "ar", "draft-2").unwrap(),
    )
    .unwrap();
    assert_eq!(next["drafts"][0], edited["drafts"][0]);
    assert_eq!(next["drafts"].as_array().unwrap().len(), 2);
    assert_eq!(saved, super::tests::saved());
    assert!(append_draft(&next, next["drafts"][0].clone()).is_err());
}
#[test]
fn cancel_and_late_generation_cannot_create_draft() {
    let mut generation = Generation::default();
    generation.start("first".into()).unwrap();
    assert!(generation.start("duplicate".into()).is_err());
    assert_eq!(generation.cancel().as_deref(), Some("first"));
    generation.start("second".into()).unwrap();
    assert_eq!(
        generation.finish("first", json!({"text":"late"})).unwrap(),
        None
    );
    assert!(generation.current("second"));
    assert_eq!(
        generation
            .finish("second", json!({"text":"中文 مِ"}))
            .unwrap(),
        Some("中文 مِ".into())
    );
    assert_eq!(
        generation
            .finish("second", json!({"text":"duplicate"}))
            .unwrap(),
        None
    );
}
#[test]
fn invalid_generation_results_release_attempt_without_overwriting() {
    for result in [
        json!({}),
        json!({"text":"  "}),
        json!({"text":"𠀀".repeat(25001)}),
    ] {
        let mut g = Generation::default();
        g.start("token".into()).unwrap();
        assert!(g.finish("token", result).is_err());
        assert!(!g.current("token"));
    }
}
#[test]
fn source_validation_and_manual_limits_use_utf16() {
    let saved = saved();
    assert!(source_draft(&saved, "en", "en", "draft").is_err());
    assert!(source_draft(&saved, "und", "ar", "draft").is_err());
    let mut empty = saved.clone();
    empty["scene"]["document"]["content"] = json!([]);
    assert!(source_draft(&empty, "en", "ar", "draft").is_err());
    let document = append_draft(
        &crate::tools::initial("sceneTranslations"),
        source_draft(&saved, "ar", "en", "draft").unwrap(),
    )
    .unwrap();
    assert!(edit_draft(&document, "draft", &"𠀀".repeat(250001)).is_err());
    assert!(edit_draft(&document, "unknown", "x").is_err());
    assert!(edit_draft(&document, "draft", "").is_ok());
}

fn wait(condition: impl Fn() -> bool) {
    let start = std::time::Instant::now();
    let context = glib::MainContext::default();
    while !condition() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(start.elapsed() < Duration::from_secs(20));
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
/// Parent invokes this in its initialized GTK fixture. Inference completion is
/// synthetic; this helper calls no provider or network endpoint.
pub(crate) fn run(a: &Rc<Workspace>, pane: &Rc<EditorPane>) {
    let p = TranslationPreviewPage::new(
        a.pid.borrow().clone(),
        pane.scene_id.clone(),
        a.client.clone(),
    );
    a.install(&p.widget, "Translations", false);
    p.load(a);
    wait(|| p.loaded.get());
    let saved = rpc(
        &a.client,
        "load_project_scene",
        json!({"projectId":a.pid.borrow().clone(),"sceneId":pane.scene_id}),
    );
    let history = rpc(&a.client, "history", json!({"sceneId":pane.scene_id}));
    p.create(a, false);
    wait(|| p.generation.borrow().active.is_none());
    assert!(p.dirty.get());
    assert_eq!(
        p.original
            .buffer()
            .unwrap()
            .text(
                &p.original.buffer().unwrap().start_iter(),
                &p.original.buffer().unwrap().end_iter(),
                true
            )
            .as_deref(),
        Some(novel_storage::plain_text(&saved["scene"]["document"]).as_str())
    );
    p.text
        .buffer()
        .unwrap()
        .set_text("Author translation مِ 中文 𠀀");
    let done = Rc::new(RefCell::new(None));
    let out = done.clone();
    p.save(a, move |r| *out.borrow_mut() = Some(r));
    wait(|| done.borrow().is_some());
    done.borrow_mut().take().unwrap().unwrap();
    assert!(!p.dirty.get());
    let first = p.selected().unwrap();
    assert_eq!(first["text"], "Author translation مِ 中文 𠀀");
    // Inject a mock generation reply through the same token gate, retaining both drafts.
    p.generation
        .borrow_mut()
        .start("mock-generation".into())
        .unwrap();
    let text = p
        .generation
        .borrow_mut()
        .finish("mock-generation", json!({"text":"Generated 中文 مِ"}))
        .unwrap()
        .unwrap();
    let mut draft = source_draft(&saved, "en", "ar", &uuid::Uuid::new_v4().to_string()).unwrap();
    draft["text"] = json!(text);
    p.add(draft);
    assert_eq!(p.value.borrow()["drafts"].as_array().unwrap().len(), 2);
    assert_eq!(p.value.borrow()["drafts"][0], first);
    assert!(p.dirty.get());
    p.composing.set(true);
    let failed = Rc::new(RefCell::new(None));
    let out = failed.clone();
    p.save(a, move |r| *out.borrow_mut() = Some(r));
    assert!(failed.borrow().as_ref().unwrap().is_err());
    assert!(p.dirty.get());
    p.composing.set(false);
    p.discard();
    assert_eq!(p.value.borrow()["drafts"].as_array().unwrap().len(), 1);
    p.generation
        .borrow_mut()
        .start("mock-cancel".into())
        .unwrap();
    p.cancel_generation();
    assert_eq!(
        p.generation
            .borrow_mut()
            .finish("mock-cancel", json!({"text":"late"}))
            .unwrap(),
        None
    );
    assert_eq!(p.value.borrow()["drafts"][0], first);
    assert_eq!(
        rpc(&a.client, "history", json!({"sceneId":pane.scene_id})),
        history
    );
    assert_eq!(pane.frame.borrow()["document"], saved["scene"]["document"]);
    // Current translation metadata is guarded even though its immutable source
    // is allowed to be an older historical scene revision.
    let snapshot = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":a.pid.borrow().clone(),"kind":"sceneTranslations"}),
    );
    let competing = rpc(
        &a.client,
        "save_project_data_checked",
        json!({"projectId":a.pid.borrow().clone(),"kind":"sceneTranslations","parent":snapshot["revisionId"],"payload":snapshot["payload"],"dependencies":[]}),
    );
    p.text
        .buffer()
        .unwrap()
        .set_text("Retained failed save مِ 中文 𠀀");
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    p.save(a, move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some());
    assert!(result.borrow().as_ref().unwrap().is_err());
    assert!(p.dirty.get());
    assert_eq!(
        p.selected().unwrap()["text"],
        "Retained failed save مِ 中文 𠀀"
    );
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":a.pid.borrow().clone(),"kind":"sceneTranslations"})
        ),
        competing
    );
    p.discard();
    assert_eq!(p.selected().unwrap(), first);
}
