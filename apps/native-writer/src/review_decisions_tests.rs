use super::*;
fn saved() -> Value {
    json!({"id":"scene-fixture","revisionId":"revision-fixture","scene":{"schemaVersion":1,"title":"Synthetic","document":{"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"مِ 𠀀 ","marks":[{"type":"bold"}]},{"type":"text","text":"old","marks":[{"type":"italic"}]},{"type":"hardBreak"},{"type":"text","text":"中文"}]},{"type":"paragraph","content":[{"type":"text","text":"tail"}]}]}}})
}
fn proposal(saved: &Value, hash: &Value) -> Value {
    let source = json!({"sceneId":saved["id"],"revisionId":saved["revisionId"],"hash":hash});
    json!({"id":"fixture-proposal","origin":"llm","title":"Author proposal","explanation":"Synthetic review","status":"pending","sources":[source],"evidence":[{"sceneId":saved["id"],"revisionId":saved["revisionId"],"hash":hash,"offsetUnit":"utf16","start":6,"end":9,"quote":"old","prefix":"𠀀 ","suffix":"\n中文"}],"action":{"kind":"replace","replacement":"new\nword"}})
}
#[test]
fn utf16_scalar_boundaries_are_exact() {
    let text = "مِ𠀀👩🏽‍🚀";
    assert_eq!(scalar_at(text, 0).unwrap(), 0);
    assert_eq!(scalar_at(text, 2).unwrap(), 2);
    assert!(scalar_at(text, 3).is_err());
    assert_eq!(scalar_at(text, 4).unwrap(), 3);
    assert!(scalar_at(text, 5).is_err());
    assert!(scalar_at(text, usize::MAX).is_err());
}
#[test]
fn rich_replacement_preserves_exact_runs_and_paragraphs() {
    let saved = saved();
    let hash = json!("a".repeat(64));
    let s = proposal(&saved, &hash);
    let (index, block) = checked_replacement(&s, &saved, &hash).unwrap();
    assert_eq!(index, 0);
    assert_eq!(
        block["attrs"],
        saved["scene"]["document"]["content"][0]["attrs"]
    );
    assert_eq!(
        block["content"][0],
        saved["scene"]["document"]["content"][0]["content"][0]
    );
    assert_eq!(block["content"][1]["marks"], json!([{"type":"italic"}]));
    assert_eq!(novel_storage::plain_text(&block), "مِ 𠀀 new\nword\n中文");
}
#[test]
fn refuses_stale_identity_hash_quote_context_and_offset_units() {
    let saved = saved();
    let hash = json!("b".repeat(64));
    let s = proposal(&saved, &hash);
    for (key, value) in [
        ("sceneId", json!("other")),
        ("revisionId", json!("older")),
        ("hash", json!("0".repeat(64))),
        ("quote", json!("bad")),
        ("prefix", json!("other")),
        ("suffix", json!("other")),
        ("offsetUnit", json!("scalar")),
        ("start", json!(-1)),
    ] {
        let mut bad = s.clone();
        bad["evidence"][0][key] = value;
        assert!(checked_replacement(&bad, &saved, &hash).is_err(), "{key}");
    }
    let mut changed = saved.clone();
    changed["revisionId"] = json!("changed");
    assert!(checked_replacement(&s, &changed, &hash).is_err());
    assert!(checked_replacement(&s, &saved, &json!("c".repeat(64))).is_err());
}
#[test]
fn refuses_astral_splits_and_cross_paragraph_changes() {
    let saved = saved();
    let hash = json!("a".repeat(64));
    let mut s = proposal(&saved, &hash);
    s["evidence"][0]["start"] = json!(4);
    s["evidence"][0]["end"] = json!(5);
    s["evidence"][0]["quote"] = json!("x");
    s["evidence"][0]["prefix"] = json!("");
    s["evidence"][0]["suffix"] = json!("");
    assert!(checked_replacement(&s, &saved, &hash)
        .unwrap_err()
        .contains("scalar"));
    let mut s = proposal(&saved, &hash);
    s["evidence"][0]["end"] = json!(18);
    s["evidence"][0]["quote"] = json!("old\n中文\n\ntail");
    s["evidence"][0]["suffix"] = json!("");
    assert!(checked_replacement(&s, &saved, &hash)
        .unwrap_err()
        .contains("paragraph"));
}
#[test]
fn rejected_decisions_keep_full_proposals_and_settings() {
    let hash = json!("a".repeat(64));
    let s = proposal(&saved(), &hash);
    let mut value = crate::tools::initial("settings");
    value["customAuthorSetting"] = json!("kept");
    value["assistance"]["suggestions"] = json!([s]);
    let next = decided(&value, "fixture-proposal", "rejected").unwrap();
    assert_eq!(next["customAuthorSetting"], "kept");
    assert_eq!(
        next["assistance"]["providers"],
        value["assistance"]["providers"]
    );
    let mut expected = value["assistance"]["suggestions"][0].clone();
    expected["status"] = json!("rejected");
    assert_eq!(next["assistance"]["suggestions"][0], expected);
    assert!(decided(&next, "fixture-proposal", "accepted").is_err());
    assert_eq!(value["assistance"]["suggestions"][0]["status"], "pending");
}
#[test]
fn fact_and_question_apply_never_changes_manuscript_or_canon() {
    let saved = saved();
    let hash = json!("a".repeat(64));
    for action in [
        json!({"kind":"question"}),
        json!({"kind":"fact","subject":"Mira","property":"knows","value":"a secret"}),
    ] {
        let mut s = proposal(&saved, &hash);
        s["action"] = action;
        assert!(validate_suggestion(&s).is_ok());
        assert!(checked_replacement(&s, &saved, &hash).is_err());
    }
}
#[test]
fn swap_keeps_one_undo_and_exact_original() {
    let saved = saved();
    let hash = json!("a".repeat(64));
    let (index, block) = checked_replacement(&proposal(&saved, &hash), &saved, &hash).unwrap();
    let document = saved["scene"]["document"].clone();
    let mut model = novel_editor::model::TextModel::new(document.clone()).unwrap();
    model.swap_paragraph(index, block).unwrap();
    assert_ne!(model.document(), document);
    model
        .edit(&json!({"type":"command","command":"undo"}))
        .unwrap();
    assert_eq!(model.document(), document);
}

#[test]
fn apply_manifest_is_bounded_unique_lowercase_hash_and_ordered() {
    let sources=(0..50).map(|i|json!({"sceneId":format!("scene-{i}"),"revisionId":format!("rev-{i}"),"hash":"a".repeat(64)})).collect::<Vec<_>>();
    let manifest = apply_sources(&json!({"sources":sources})).unwrap();
    assert_eq!(manifest, json!(sources));
    let mut too_many = sources.clone();
    too_many.push(json!({"sceneId":"extra","revisionId":"extra","hash":"b".repeat(64)}));
    assert!(apply_sources(&json!({"sources":too_many})).is_err());
    let mut duplicate = sources.clone();
    duplicate[1]["sceneId"] = duplicate[0]["sceneId"].clone();
    assert!(apply_sources(&json!({"sources":duplicate})).is_err());
    for hash in ["A".repeat(64), "g".repeat(64), "a".repeat(63)] {
        let mut bad = sources[0].clone();
        bad["hash"] = json!(hash);
        assert!(apply_sources(&json!({"sources":[bad]})).is_err());
    }
    assert!(apply_sources(&json!({"sources":[]})).is_err());
    let mut extra = sources[0].clone();
    extra["modelClaim"] = json!("not an authorization");
    assert_eq!(
        apply_sources(&json!({"sources":[extra]})).unwrap(),
        json!([sources[0]])
    );
}
#[test]
fn atomic_gate_requires_every_source_and_exact_ordered_echo() {
    let sources = json!([
        {"sceneId":"a","revisionId":"ra","hash":"a".repeat(64)},
        {"sceneId":"b","revisionId":"rb","hash":"b".repeat(64)}
    ]);
    let anchor = json!({"blockIndex":0,"documentHash":"captured"});
    let replacement = json!({"type":"paragraph"});
    let good = checked_swap(
        &sources,
        Ok(json!({"sources":sources})),
        &anchor,
        &replacement,
    )
    .unwrap();
    assert_eq!(good["command"], "swapParagraph");
    assert_eq!(good["anchor"], anchor);
    let mut changed = sources.clone();
    changed[1]["revisionId"] = json!("newer");
    let mut changed_hash = sources.clone();
    changed_hash[1]["hash"] = json!("c".repeat(64));
    for reply in [
        Err("Supporting scene changed between individual reads.".into()),
        Ok(json!({})),
        Ok(json!({"sources":[sources[0]]})),
        Ok(json!({"sources":[sources[1],sources[0]]})),
        Ok(json!({"sources":changed})),
        Ok(json!({"sources":changed_hash})),
    ] {
        assert!(checked_swap(&sources, reply, &anchor, &replacement).is_err());
    }
}
#[test]
fn denied_atomic_gate_preserves_document_and_undo_then_success_is_one_edit() {
    let saved = saved();
    let hash = json!("a".repeat(64));
    let s = proposal(&saved, &hash);
    let sources = apply_sources(&s).unwrap();
    let (index, replacement) = checked_replacement(&s, &saved, &hash).unwrap();
    let document = saved["scene"]["document"].clone();
    let mut model = novel_editor::model::TextModel::new(document.clone()).unwrap();
    let anchor = json!({"blockIndex":index,"documentHash":"mock-capture","block":document["content"][index]});
    if let Ok(command) = checked_swap(
        &sources,
        Err("Synthetic concurrent head change".into()),
        &anchor,
        &replacement,
    ) {
        model
            .swap_paragraph(index, command["replacement"].clone())
            .unwrap();
    }
    assert_eq!(model.document(), document);
    assert!(model.undo.is_empty());
    let command = checked_swap(
        &sources,
        Ok(json!({"sources":sources})),
        &anchor,
        &replacement,
    )
    .unwrap();
    model
        .swap_paragraph(index, command["replacement"].clone())
        .unwrap();
    assert_eq!(model.undo.len(), 1);
    model
        .edit(&json!({"type":"command","command":"undo"}))
        .unwrap();
    assert_eq!(model.document(), document);
}
#[test]
fn post_apply_failure_explicitly_denies_acceptance_and_retains_undo_guidance() {
    let message = save_failure(true, "Synthetic metadata CAS conflict");
    assert!(message.contains("decision was not accepted"));
    assert!(message.contains("unsaved scene draft"));
    assert!(message.contains("Undo"));
    assert!(message.contains("manual scene Save"));
    assert!(message.contains("Synthetic metadata CAS conflict"));
    assert!(!save_failure(false, "conflict").contains("Applied to"));
}

fn question_basis() -> (Value, Value) {
    let scene = saved();
    let scene_source = json!({"sceneId":scene["id"],"revisionId":scene["revisionId"],"hash":"a".repeat(64),"kind":"scene","text":novel_storage::plain_text(&scene["scene"]["document"])});
    let facts_source = json!({"sceneId":"planning:project-fixture","revisionId":"planning-head","hash":"b".repeat(64),"kind":"facts","text":"Confirmed مِ 中文 𠀀"});
    let mut question = proposal(&scene, &scene_source["hash"]);
    question["reviewOperation"] = json!("consistency");
    question["sources"] = json!([scene_source, facts_source]);
    question["action"] = json!({"kind":"question"});
    let context = json!({"scene":scene,"sources":[scene_source,facts_source]});
    (question, context)
}
#[test]
fn consistency_note_has_exact_scene_and_planning_dependencies_without_a_text_edit() {
    let (question, context) = question_basis();
    let deps = checked_consistency_basis(&question, "project-fixture", &context).unwrap();
    assert_eq!(deps,json!([{"entityId":"scene-fixture","entityType":"scene","revisionId":"revision-fixture"},{"entityId":"project-fixture:planning","entityType":"projectData","revisionId":"planning-head"}]).as_array().unwrap().clone());
    assert!(
        checked_replacement(&question, &context["scene"], &context["sources"][0]["hash"]).is_err()
    );
    let mut settings = crate::tools::initial("settings");
    settings["assistance"]["suggestions"] = json!([question]);
    let accepted = decided(&settings, "fixture-proposal", "accepted").unwrap();
    assert_eq!(
        accepted["assistance"]["suggestions"][0]["action"]["kind"],
        "question"
    );
    assert!(accepted.get("planning").is_none());
}
#[test]
fn stale_planning_equal_text_new_head_wrong_quote_and_fact_only_evidence_refuse_notes() {
    let (question, context) = question_basis();
    for (field, value) in [
        ("revisionId", json!("new-head-same-text")),
        ("hash", json!("c".repeat(64))),
    ] {
        let mut newer = context.clone();
        newer["sources"][1][field] = value;
        assert!(checked_consistency_basis(&question, "project-fixture", &newer).is_err());
    }
    assert!(checked_consistency_basis(&question, "foreign-project", &context).is_err());
    let mut bad = question.clone();
    bad["evidence"][0]["quote"] = json!("BAD");
    assert!(checked_consistency_basis(&bad, "project-fixture", &context).is_err());
    let mut fact_only = question;
    let mut anchor = context["sources"][1].clone();
    anchor["offsetUnit"] = json!("utf16");
    anchor["start"] = json!(0);
    anchor["end"] = json!(9);
    anchor["quote"] = json!("Confirmed");
    anchor["prefix"] = json!("");
    anchor["suffix"] = json!("");
    fact_only["evidence"] = json!([anchor]);
    assert!(checked_consistency_basis(&fact_only, "project-fixture", &context).is_err());
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
/// Callable from the parent's existing single GTK fixture. It uses only the
/// fixture's local workers and synthetic proposals; it never invokes a provider.
pub(crate) fn run(a: &Rc<Workspace>, pane: &Rc<EditorPane>) {
    let saved = rpc(
        &a.client,
        "load_project_scene",
        json!({"projectId":a.pid.borrow().clone(),"sceneId":pane.scene_id}),
    );
    let text = novel_storage::plain_text(&saved["scene"]["document"]);
    assert!(!text.is_empty(), "Use a synthetic nonempty scene fixture");
    let hash = rpc(&a.client, "text_hash", json!({"text":text}));
    let quote = text.chars().next().unwrap().to_string();
    let source = json!({"sceneId":saved["id"],"revisionId":saved["revisionId"],"hash":hash});
    let s = json!({"id":"gtk-mock","title":"Synthetic author choice","explanation":"Mock provider data only","origin":"llm","status":"pending","sources":[source],"evidence":[{"sceneId":saved["id"],"revisionId":saved["revisionId"],"hash":hash,"offsetUnit":"utf16","start":0,"end":quote.encode_utf16().count(),"quote":quote,"prefix":"","suffix":""}],"action":{"kind":"replace","replacement":"中文 مِ 𠀀"}});
    let p = ReviewDecisionsPage::new(a.pid.borrow().clone());
    p.bind_scene(pane);
    a.install(&p.widget, "Local review", false);
    p.load(a);
    wait(|| p.loaded.get());
    p.append_result(a, &json!({"suggestions":[s]})).unwrap();
    wait(|| !p.saving.get() && !p.dirty.get());
    let before = rpc(&a.client, "history", json!({"sceneId":pane.scene_id}));
    let original = pane.frame.borrow()["document"].clone();
    p.apply(a);
    wait(|| !p.saving.get());
    assert!(pane.dirty.get());
    assert!(!p.dirty.get());
    assert_eq!(p.selected().unwrap()["status"], "accepted");
    assert_eq!(
        rpc(&a.client, "history", json!({"sceneId":pane.scene_id})),
        before
    );
    pane.operation(json!({"type":"command","command":"undo"}), true);
    wait(|| pane.pending.get() == 0);
    assert_eq!(pane.frame.borrow()["document"], original);
    let done = Rc::new(Cell::new(false));
    let out = done.clone();
    pane.discard(move |r| {
        r.unwrap();
        out.set(true);
    });
    wait(|| done.get());
    let mut stale = p.selected().unwrap();
    stale["status"] = json!("pending");
    stale["sources"][0]["revisionId"] = json!("unavailable-old-revision");
    stale["evidence"][0]["revisionId"] = json!("unavailable-old-revision");
    p.append_result(a, &json!({"suggestions":[stale]})).unwrap();
    wait(|| !p.saving.get());
    p.choices.set_active(Some(1));
    p.apply(a);
    wait(|| !p.saving.get());
    assert!(!pane.dirty.get());
    assert_eq!(p.selected().unwrap()["status"], "pending");
    p.reject(a);
    wait(|| !p.saving.get());
    assert_eq!(p.selected().unwrap()["status"], "rejected");
    assert_eq!(
        rpc(&a.client, "history", json!({"sceneId":pane.scene_id})),
        before
    );
    // A competing settings save must retain the pending local decision, never
    // overwrite the other window's head or erase the original proposal.
    let mut candidate = p.selected().unwrap();
    candidate["status"] = json!("pending");
    p.append_result(a, &json!({"suggestions":[candidate]}))
        .unwrap();
    wait(|| !p.saving.get());
    p.choices.set_active(Some(2));
    let snapshot = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":a.pid.borrow().clone(),"kind":"settings"}),
    );
    let competing = rpc(
        &a.client,
        "save_project_data_checked",
        json!({"projectId":a.pid.borrow().clone(),"kind":"settings","parent":snapshot["revisionId"],"payload":snapshot["payload"],"dependencies":[]}),
    );
    p.reject(a);
    wait(|| !p.saving.get());
    assert!(p.dirty.get());
    assert_eq!(p.selected().unwrap()["status"], "rejected");
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":a.pid.borrow().clone(),"kind":"settings"})
        ),
        competing
    );
    p.discard();
    assert!(!p.dirty.get());
    assert_eq!(p.selected().unwrap()["status"], "pending");
    // Let independent source reads finish, then change a supporting source
    // immediately before the atomic gate. The target editor must stay untouched.
    p.loaded.set(false);
    p.load(a);
    wait(|| p.loaded.get());
    let support_text = "Supporting exact UTF-8 مِ 中文 𠀀";
    let support = rpc(
        &a.client,
        "create_scene",
        json!({"chapterId":saved["chapterId"],"scene":{"schemaVersion":1,"title":"Synthetic support","document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":support_text}]}]}}}),
    );
    let support_hash = rpc(&a.client, "text_hash", json!({"text":support_text}));
    let mut fresh = suggestions(&p.value.borrow())[0].clone();
    fresh["status"] = json!("pending");
    let mut scoped = fresh.clone();
    scoped["sources"].as_array_mut().unwrap().push(
        json!({"sceneId":support["id"],"revisionId":support["revisionId"],"hash":support_hash}),
    );
    p.append_result(a, &json!({"suggestions":[scoped]}))
        .unwrap();
    wait(|| !p.saving.get());
    p.choices
        .set_active(Some((suggestions(&p.value.borrow()).len() - 1) as u32));
    let client = a.client.clone();
    let support_ = support.clone();
    *p.before_sources_check.borrow_mut() = Some(Box::new(move || {
        let mut changed = support_["scene"].clone();
        changed["document"]["content"][0]["content"][0]["text"] =
            json!("Concurrent support revision مِ 中文 𠀀");
        rpc(
            &client,
            "save_scene",
            json!({"sceneId":support_["id"],"parent":support_["revisionId"],"scene":changed,"requestId":uuid::Uuid::new_v4().to_string()}),
        );
    }));
    p.apply(a);
    wait(|| !p.saving.get());
    assert!(!pane.dirty.get());
    assert_eq!(pane.frame.borrow()["document"], original);
    assert_eq!(p.selected().unwrap()["status"], "pending");
    assert!(!p.dirty.get());
    assert!(p.status.text().contains("no edit applied"));
    assert_eq!(
        rpc(&a.client, "history", json!({"sceneId":pane.scene_id})),
        before
    );

    // A later metadata conflict is different: the authorized editor mutation
    // remains unsaved and undoable, but its decision was never accepted on disk.
    p.append_result(a, &json!({"suggestions":[fresh]})).unwrap();
    wait(|| !p.saving.get());
    p.choices
        .set_active(Some((suggestions(&p.value.borrow()).len() - 1) as u32));
    let proposal_id = p.selected().unwrap()["id"].clone();
    let client = a.client.clone();
    let project = a.pid.borrow().clone();
    *p.before_decision_save.borrow_mut() = Some(Box::new(move || {
        let current = rpc(
            &client,
            "load_project_data_unambiguous",
            json!({"projectId":project,"kind":"settings"}),
        );
        rpc(
            &client,
            "save_project_data_checked",
            json!({"projectId":project,"kind":"settings","parent":current["revisionId"],"payload":current["payload"],"dependencies":[]}),
        );
    }));
    p.apply(a);
    wait(|| !p.saving.get());
    assert!(pane.dirty.get());
    assert!(p.dirty.get());
    assert!(p.applied_pending.get());
    assert!(p.status.text().contains("decision was not accepted"));
    assert!(p.status.text().contains("Undo"));
    let current = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":a.pid.borrow().clone(),"kind":"settings"}),
    );
    assert_eq!(
        suggestions(&current["payload"])
            .into_iter()
            .find(|s| s["id"] == proposal_id)
            .unwrap()["status"],
        "pending"
    );
    assert_eq!(
        rpc(&a.client, "history", json!({"sceneId":pane.scene_id})),
        before
    );
    pane.operation(json!({"type":"command","command":"undo"}), true);
    wait(|| pane.pending.get() == 0);
    assert_eq!(pane.frame.borrow()["document"], original);
    let done = Rc::new(Cell::new(false));
    let out = done.clone();
    pane.discard(move |r| {
        r.unwrap();
        out.set(true);
    });
    wait(|| done.get());
    p.discard();
}
