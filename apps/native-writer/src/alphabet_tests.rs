use super::*;

fn definition() -> Value {
    json!({"schemaVersion":1,"id":"synthetic","mappingVersion":1,"name":"Synthetic alphabet","scriptId":"custom","encoding":"private-use","direction":"ltr","mappings":[{"input":"a","output":"\u{e000}","transliteration":"a"},{"input":"𠀀","output":"\u{f0000}","transliteration":"zh"}],"font":null,"glossary":[{"symbol":"مِ 中文 👩🏽‍🚀","meaning":"exact multilingual fixture"}],"notes":"e\u{301} ≠ é"})
}
fn document() -> Value {
    json!({"schemaVersion":1,"systems":[definition()],"assignments":[]})
}
fn capture() -> Capture {
    let saved = json!({"revisionId":"alphabet-1","payload":document()});
    Capture::checked(
        "novel".into(),
        vec!["alphabet-1".into()],
        Some(saved.clone()),
        &[saved],
        &[("scene".into(), "scene-1".into())],
    )
    .unwrap()
}
fn edited(draft: &mut Draft) {
    draft.choose(Some("synthetic@1")).unwrap();
    let mut d = draft.definition.clone().unwrap();
    d["name"] = json!("Second name");
    d["mappings"][0]["output"] = json!("\u{e001}");
    draft.edit(d).unwrap();
}
#[test]
fn codepoint_parser_is_exact_and_scalar_bounded() {
    for (input, exact) in [
        (" U+E000 ", "\u{e000}"),
        ("u+F0000", "\u{f0000}"),
        ("0X10FFFF", "\u{10ffff}"),
        ("0x0301", "\u{301}"),
        ("20000", "𠀀"),
    ] {
        assert_eq!(parse_codepoint(input).unwrap(), exact);
    }
    for bad in [
        "",
        "U+D800",
        "DFFF",
        "110000",
        "U+0000000",
        "U+E000 U+E001",
        "-1",
        "0xg",
        "a b",
    ] {
        assert!(parse_codepoint(bad).is_err(), "{bad}");
    }
    assert_eq!(
        codepoints("مِ𠀀\u{f0000}e\u{301}"),
        "U+0645 U+0650 U+20000 U+F0000 U+0065 U+0301"
    );
}
#[test]
fn typing_mapping_is_scalar_exact_and_one_pass() {
    let d = definition();
    let original = "a𠀀 مِ 中文 👩🏽‍🚀 e\u{301} é\n";
    assert_eq!(
        map_ordinary_typing(original, Some(&d)).unwrap(),
        "\u{e000}\u{f0000} مِ 中文 👩🏽‍🚀 e\u{301} é\n"
    );
    assert_eq!(map_ordinary_typing(original, None).unwrap(), original);
    let mut d = d;
    d["encoding"] = json!("unicode");
    d["mappings"] = json!([{"input":"a","output":"b","transliteration":""},{"input":"b","output":"c","transliteration":""}]);
    assert_eq!(map_ordinary_typing("ab", Some(&d)).unwrap(), "bc");
    // Paste, drop and palette paths receive the literal string, never this mapper.
    assert_eq!(original, "a𠀀 مِ 中文 👩🏽‍🚀 e\u{301} é\n");
}
#[test]
fn validation_rejects_duplicate_mappings_and_invisible_keys() {
    let valid = definition();
    validate_definition(&valid).unwrap();
    for c in ["", "ab", "\n", "\u{200d}", "\u{202e}", "\u{e0001}"] {
        let mut d = valid.clone();
        d["mappings"][0]["input"] = json!(c);
        assert!(validate_definition(&d).is_err());
    }
    let mut d = valid.clone();
    d["mappings"][1]["input"] = json!("a");
    assert!(validate_definition(&d).is_err());
    let mut d = valid.clone();
    d["mappings"][1]["output"] = json!("\u{e000}");
    assert!(validate_definition(&d).is_err());
    let mut d = valid;
    d["encoding"] = json!("unicode");
    d["mappings"][0]["output"] = json!("\u{301}");
    validate_definition(&d).unwrap();
}
#[test]
fn encoding_font_permission_and_collection_limits_are_explicit() {
    let mut d = definition();
    d["mappings"][0]["output"] = json!("a");
    assert!(validate_definition(&d).is_err());
    d["encoding"] = json!("font-presentation");
    d["mappings"][1]["output"] = json!("𠀀");
    validate_definition(&d).unwrap();
    d["font"] = json!({"fontAssetId":"asset","checksum":"a".repeat(64),"family":"Synthetic family","format":"ttf","license":{"name":"Synthetic test permission","source":"Test fixture only","permissionConfirmed":true}});
    validate_definition(&d).unwrap();
    d["font"]["license"]["source"] = json!("");
    assert!(validate_definition(&d).is_err());
    d["font"]["license"]["source"] = json!("fixture");
    d["font"]["license"]["permissionConfirmed"] = json!(false);
    assert!(validate_definition(&d).is_err());
    d["font"] = Value::Null;
    d["mappings"] = json!(vec![json!({}); 4097]);
    assert!(validate_definition(&d).is_err());
    let mut d = definition();
    d["mappingVersion"] = json!(MAX_VERSION + 1);
    assert!(validate_definition(&d).is_err());
}
#[test]
fn document_rejects_duplicate_versions_and_dangling_assignment() {
    let mut doc = document();
    doc["systems"].as_array_mut().unwrap().push(definition());
    assert!(validate_document(&doc).is_err());
    let mut doc = document();
    doc["assignments"] = json!([{"scene":{"kind":"scene","id":"scene"},"definitionId":"synthetic","mappingVersion":2}]);
    assert!(validate_document(&doc).is_err());
    doc["assignments"][0]["mappingVersion"] = json!(1);
    validate_document(&doc).unwrap();
    let a = doc["assignments"][0].clone();
    doc["assignments"].as_array_mut().unwrap().push(a);
    assert!(validate_document(&doc).is_err());
}
#[test]
fn checked_capture_refuses_competing_heads_missing_history_and_dropped_versions() {
    let saved = json!({"revisionId":"one","payload":document()});
    for heads in [vec![], vec!["two".into()], vec!["one".into(), "two".into()]] {
        assert!(Capture::checked(
            "novel".into(),
            heads,
            Some(saved.clone()),
            &[saved.clone()],
            &[]
        )
        .is_err());
    }
    assert!(Capture::checked(
        "novel".into(),
        vec!["one".into()],
        Some(saved.clone()),
        &[],
        &[]
    )
    .is_err());
    let empty = json!({"revisionId":"two","payload":empty_document()});
    assert!(Capture::checked(
        "novel".into(),
        vec!["two".into()],
        Some(empty.clone()),
        &[saved.clone(), empty],
        &[]
    )
    .is_err());
    assert!(Capture::checked("novel".into(), vec![], None, &[saved], &[]).is_err());
    Capture::checked("novel".into(), vec![], None, &[], &[]).unwrap();
}
#[test]
fn publish_keeps_original_and_exact_glossary_and_assignments() {
    let mut draft = Draft::new(capture());
    edited(&mut draft);
    draft.assign("scene", Some("synthetic@1")).unwrap();
    let request = draft.begin_save().unwrap();
    assert_eq!(request["parent"], "alphabet-1");
    assert_eq!(request["payload"]["systems"][0], definition());
    assert_eq!(request["payload"]["systems"][1]["mappingVersion"], 2);
    assert_eq!(
        request["payload"]["systems"][1]["glossary"],
        definition()["glossary"]
    );
    assert_eq!(
        request["dependencies"],
        json!([{"entityId":"scene","entityType":"scene","revisionId":"scene-1"}])
    );
    assert_eq!(
        selected_definition(&request["payload"], "scene")
            .unwrap()
            .unwrap(),
        &definition()
    );
    assert!(draft.dirty());
    assert!(draft.saving);
    assert!(draft.assign("scene", None).is_err());
    assert!(draft.discard().is_err());
    draft
        .complete_save(Ok(
            json!({"revisionId":"alphabet-2","payload":request["payload"]}),
        ))
        .unwrap();
    assert!(!draft.dirty());
    assert_eq!(draft.capture.heads, vec!["alphabet-2"]);
}
#[test]
fn failed_cas_retains_identical_candidate_without_extra_version() {
    let mut draft = Draft::new(capture());
    edited(&mut draft);
    let first = draft.begin_save().unwrap();
    assert!(draft
        .complete_save(Err("Conflict: scene source changed".into()))
        .is_err());
    assert!(draft.dirty());
    assert!(!draft.saving);
    assert_eq!(draft.saved_document(), &document());
    let retry = draft.begin_save().unwrap();
    assert_eq!(retry, first);
    draft.complete_save(Err("Conflict".into())).unwrap_err();
    draft.discard().unwrap();
    assert_eq!(draft.document, document());
    assert!(!draft.dirty());
}
#[test]
fn acknowledgement_mismatch_never_replaces_draft() {
    let mut draft = Draft::new(capture());
    edited(&mut draft);
    let pending = draft.begin_save().unwrap();
    assert!(draft
        .complete_save(Ok(json!({"revisionId":"other","payload":document()})))
        .is_err());
    assert_eq!(draft.document, document());
    assert_eq!(draft.pending.as_ref().unwrap(), &pending["payload"]);
    assert_eq!(draft.capture.parent.as_deref(), Some("alphabet-1"));
    assert!(draft.dirty());
}
#[test]
fn assignments_require_checked_scene_sources_and_manual_save() {
    let mut draft = Draft::new(capture());
    assert!(draft
        .assign("other-novel-scene", Some("synthetic@1"))
        .is_err());
    assert!(draft.assign("scene", Some("synthetic@9")).is_err());
    draft.assign("scene", Some("synthetic@1")).unwrap();
    assert_eq!(draft.saved_document(), &document());
    let request = draft.begin_save().unwrap();
    assert_eq!(request["payload"]["systems"], document()["systems"]);
    draft
        .complete_save(Ok(
            json!({"revisionId":"assigned","payload":request["payload"]}),
        ))
        .unwrap();
    draft.assign("scene", None).unwrap();
    let request = draft.begin_save().unwrap();
    assert_eq!(request["dependencies"].as_array().unwrap().len(), 1);
    assert_eq!(request["payload"]["assignments"], json!([]));
}
#[test]
fn new_definition_is_a_manual_unpublished_draft() {
    let capture = Capture::checked("novel".into(), vec![], None, &[], &[]).unwrap();
    let mut draft = Draft::new(capture);
    draft.create().unwrap();
    assert!(draft.dirty());
    assert_eq!(draft.saved_document(), &empty_document());
    assert!(draft.choose(None).is_err());
    let request = draft.begin_save().unwrap();
    assert_eq!(request["parent"], Value::Null);
    assert_eq!(request["payload"]["systems"][0]["mappingVersion"], 1);
    assert_eq!(request["dependencies"], json!([]));
}
#[test]
fn invalid_authoring_is_refused_before_staging_or_saving() {
    let mut draft = Draft::new(capture());
    edited(&mut draft);
    let mut d = draft.definition.clone().unwrap();
    d["name"] = json!("");
    draft.edit(d).unwrap();
    assert!(draft.begin_save().is_err());
    assert!(!draft.saving);
    assert_eq!(draft.document, document());
    assert!(draft.dirty());
    let mut d = draft.definition.clone().unwrap();
    d["id"] = json!("new-identity");
    assert!(draft.edit(d).is_err());
}

#[test]
fn literal_input_bypasses_mapping_and_invalid_profile_validation() {
    let exact = "a𠀀\u{e000} مِ e\u{301} 👩🏽‍🚀";
    assert_eq!(
        input_text(exact, Some(&definition()), InputOrigin::Literal).unwrap(),
        exact
    );
    assert_eq!(
        input_text(exact, Some(&json!({})), InputOrigin::Literal).unwrap(),
        exact
    );
    assert!(input_text(exact, Some(&json!({})), InputOrigin::OrdinaryTyping).is_err());
    assert_eq!(
        input_text(exact, Some(&definition()), InputOrigin::OrdinaryTyping).unwrap(),
        "\u{e000}\u{f0000}\u{e000} مِ e\u{301} 👩🏽‍🚀"
    );
}
#[test]
fn session_profile_stays_bound_to_saved_assignment_during_draft_edit() {
    let mut draft = Draft::new(capture());
    draft.assign("scene", Some("synthetic@1")).unwrap();
    let request = draft.begin_save().unwrap();
    draft
        .complete_save(Ok(
            json!({"revisionId":"assigned","payload":request["payload"]}),
        ))
        .unwrap();
    let profile = SessionProfile::checked(&draft.capture, "scene").unwrap();
    assert_eq!(profile.project_id, "novel");
    assert_eq!(profile.scene_id, "scene");
    assert_eq!(profile.scene_revision_id, "scene-1");
    assert_eq!(profile.alphabet_revision_id.as_deref(), Some("assigned"));
    assert_eq!(
        profile.editor_selection(false),
        json!({"definitionId":"synthetic","mappingVersion":1,"direction":"ltr","fontFamily":null})
    );
    draft.assign("scene", None).unwrap();
    assert_eq!(
        SessionProfile::checked(&draft.capture, "scene")
            .unwrap()
            .definition,
        Some(definition())
    );
    assert_eq!(profile.definition, Some(definition()));
    assert!(SessionProfile::checked(&draft.capture, "different-scene").is_err());
}
#[test]
fn assignment_batch_bound_refuses_before_staging() {
    let saved = json!({"revisionId":"alphabet-1","payload":document()});
    let scenes = (0..51)
        .map(|i| (format!("scene-{i}"), format!("revision-{i}")))
        .collect::<Vec<_>>();
    let c = Capture::checked(
        "novel".into(),
        vec!["alphabet-1".into()],
        Some(saved.clone()),
        &[saved],
        &scenes,
    )
    .unwrap();
    let mut draft = Draft::new(c);
    for (scene, _) in &scenes {
        draft.assign(scene, Some("synthetic@1")).unwrap();
    }
    assert!(draft.begin_save().is_err());
    assert!(!draft.saving);
    assert!(draft.dirty());
}
#[test]
fn repeated_parent_acknowledgement_is_refused() {
    let mut draft = Draft::new(capture());
    edited(&mut draft);
    let pending = draft.begin_save().unwrap();
    assert!(draft
        .complete_save(Ok(
            json!({"revisionId":"alphabet-1","payload":pending["payload"]})
        ))
        .is_err());
    assert!(draft.dirty());
    assert_eq!(draft.capture.parent.as_deref(), Some("alphabet-1"));
}

#[test]
fn failed_publication_can_be_corrected_without_phantom_versions() {
    let mut draft = Draft::new(capture());
    edited(&mut draft);
    draft.begin_save().unwrap();
    draft.complete_save(Err("Conflict".into())).unwrap_err();
    let mut d = draft.definition.clone().unwrap();
    d["name"] = json!("Corrected draft");
    draft.edit(d).unwrap();
    let corrected = draft.begin_save().unwrap();
    assert_eq!(corrected["payload"]["systems"].as_array().unwrap().len(), 2);
    assert_eq!(corrected["payload"]["systems"][1]["mappingVersion"], 2);
    assert_eq!(
        corrected["payload"]["systems"][1]["name"],
        "Corrected draft"
    );
    assert_eq!(draft.document, document());
    assert_eq!(draft.session_profile("scene").unwrap().definition, None);
}
