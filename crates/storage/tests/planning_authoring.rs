//! Synthetic transaction regressions for new planning authorship versus history.
mod manuscript_parts { pub use novel_storage::manuscript_parts::validate_planning_parts; }
#[path = "../src/planning_validation.rs"]
mod guard;
use novel_storage::{Scene, SourceDependency, Storage, SyncCursor};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn fixture(scene: &str) -> Value {
    json!({"schemaVersion":1,
        "fields":[{"id":"links","label":"Links","kind":"links","recordKind":"character","hidden":false,"options":[]}],
        "records":[{"id":"hero","kind":"character","name":"نور 中文","aliases":[],"notes":"exact e\u{301} 𠀀 \u{e000}","values":{"links":[{"kind":"scene","id":scene}]},"links":[]}],
        "facts":[{"id":"fact","subjectId":"hero","property":"role","value":"guard مِ","status":"fact","confirmed":true,"observerId":null,"source":"author","eventId":null,"untilEventId":null}],
        "events":[],"plotPoints":[],"boards":[],"lanes":[],"constraints":[],"issues":[]})
}
fn defects(base: &Value) -> Vec<(&'static str, Value)> {
    let mut duplicate = base.clone();
    duplicate["facts"]
        .as_array_mut()
        .unwrap()
        .push(base["facts"][0].clone());
    let mut status = base.clone();
    let mut fact = base["facts"][0].clone();
    fact["id"] = json!("new-fact");
    fact["status"] = json!("unsupported");
    status["facts"].as_array_mut().unwrap().push(fact);
    let mut links = base.clone();
    links["records"][0]["values"]["links"] = json!(["scene-id"]);
    vec![
        ("duplicate", duplicate),
        ("status", status),
        ("links", links),
    ]
}
async fn store() -> (tempfile::TempDir, Storage, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::open(&dir.path().join("synthetic.sqlite"))
        .await
        .unwrap();
    let project = storage.create_project("Synthetic authoring").await.unwrap();
    let chapter = storage
        .create_chapter(&project.id, "Chapter")
        .await
        .unwrap();
    let scene = storage.create_scene(&chapter.id, Scene { schema_version:1, title:"مِ 中文".into(), document:json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"exact e\u{301} 𠀀 \u{e000}"}]}]}) }).await.unwrap();
    (dir, storage, project.id, scene.id)
}

#[tokio::test]
async fn both_save_apis_refuse_all_three_defects_without_revisions_heads_or_projection_changes() {
    for checked in [false, true] {
        let (_dir, s, pid, sid) = store().await;
        let base = fixture(&sid);
        let scene = s.load_scene(&sid).await.unwrap();
        let deps = [SourceDependency {
            entity_id: sid.clone(),
            entity_type: "scene".into(),
            revision_id: scene.revision_id.clone(),
        }];
        for (name, invalid) in defects(&base) {
            let before = s.sync_cursor().await.unwrap();
            let result = if checked {
                s.save_project_data_checked(&pid, "planning", invalid, None, &deps)
                    .await
            } else {
                s.save_project_data(&pid, "planning", invalid, None).await
            };
            assert!(result.is_err(), "first {checked}/{name}");
            assert_eq!(s.sync_cursor().await.unwrap(), before);
            assert!(s
                .project_data_history(&pid, "planning")
                .await
                .unwrap()
                .is_empty());
            assert!(s
                .load_project_data(&pid, "planning")
                .await
                .unwrap()
                .is_none());
        }
        let saved = s
            .save_project_data(&pid, "planning", base.clone(), None)
            .await
            .unwrap();
        let history = s.project_data_history(&pid, "planning").await.unwrap();
        for (name, invalid) in defects(&base) {
            let before = s.sync_cursor().await.unwrap();
            let result = if checked {
                s.save_project_data_checked(
                    &pid,
                    "planning",
                    invalid,
                    Some(saved.revision_id.clone()),
                    &deps,
                )
                .await
            } else {
                s.save_project_data(&pid, "planning", invalid, Some(saved.revision_id.clone()))
                    .await
            };
            assert!(result.is_err(), "update {checked}/{name}");
            assert_eq!(s.sync_cursor().await.unwrap(), before);
            assert_eq!(
                s.project_data_history(&pid, "planning").await.unwrap(),
                history
            );
            assert_eq!(
                s.project_data_heads(&pid, "planning")
                    .await
                    .unwrap()
                    .revision_ids,
                vec![saved.revision_id.clone()]
            );
            assert_eq!(
                s.load_project_data(&pid, "planning")
                    .await
                    .unwrap()
                    .unwrap(),
                saved
            );
            assert_eq!(s.load_scene(&sid).await.unwrap(), scene);
        }
    }
}
async fn transfer(from: &Storage, to: &Storage, pid: &str) {
    let remote = from.sync_cursor().await.unwrap();
    let local = to.sync_cursor().await.unwrap();
    let page = from
        .pull_revisions(
            pid,
            SyncCursor {
                epoch: remote.epoch.clone(),
                sequence: 0,
            },
            100,
        )
        .await
        .unwrap();
    to.ingest_revisions(
        pid,
        &local.epoch,
        &remote.epoch,
        from.sync_manifest(pid).await.unwrap(),
        page.revisions,
    )
    .await
    .unwrap();
}
#[tokio::test]
async fn actual_conflict_resolution_refuses_defects_atomically_and_keeps_acknowledged_versions() {
    let (_d, s, pid, sid) = store().await;
    let other_dir = tempfile::tempdir().unwrap();
    let r = Storage::open(&other_dir.path().join("replica.sqlite"))
        .await
        .unwrap();
    let base = fixture(&sid);
    let saved = s
        .save_project_data(&pid, "planning", base.clone(), None)
        .await
        .unwrap();
    transfer(&s, &r, &pid).await;
    let mut left = base.clone();
    left["records"][0]["notes"] = json!("Left exact مِ 中文");
    let left = s
        .save_project_data(&pid, "planning", left, Some(saved.revision_id.clone()))
        .await
        .unwrap();
    let mut right = base.clone();
    right["records"][0]["notes"] = json!("Right exact e\u{301}");
    r.save_project_data(&pid, "planning", right, Some(saved.revision_id))
        .await
        .unwrap();
    transfer(&r, &s, &pid).await;
    let heads = s
        .project_data_heads(&pid, "planning")
        .await
        .unwrap()
        .revision_ids;
    assert_eq!(heads.len(), 2);
    let history = s.project_data_history(&pid, "planning").await.unwrap();
    let projection = s.load_project_data(&pid, "planning").await.unwrap();
    for (name, invalid) in defects(&left.payload) {
        let before = s.sync_cursor().await.unwrap();
        assert!(
            s.resolve_project_data(&pid, "planning", invalid, heads.clone())
                .await
                .is_err(),
            "{name}"
        );
        assert_eq!(s.sync_cursor().await.unwrap(), before);
        assert_eq!(
            s.project_data_heads(&pid, "planning")
                .await
                .unwrap()
                .revision_ids,
            heads
        );
        assert_eq!(
            s.project_data_history(&pid, "planning").await.unwrap(),
            history
        );
        assert_eq!(
            s.load_project_data(&pid, "planning").await.unwrap(),
            projection
        );
    }
    let resolved = s
        .resolve_project_data(&pid, "planning", left.payload.clone(), heads.clone())
        .await
        .unwrap();
    assert_eq!(resolved.payload, left.payload);
    assert_eq!(
        s.load_entity_revision(
            &pid,
            &format!("{pid}:planning"),
            "projectData",
            &resolved.revision_id
        )
        .await
        .unwrap()
        .parents,
        heads
    );
    assert_eq!(
        s.project_data_history(&pid, "planning")
            .await
            .unwrap()
            .len(),
        history.len() + 1
    );
}

#[test]
fn typed_fields_enforce_seven_kinds_and_preserve_unavailable_choice_text_and_unicode() {
    let mut doc = fixture("external-scene");
    let kinds = [
        ("text", json!("e\u{301} مِ")),
        ("long-text", json!("𠀀\n中文")),
        ("number", json!(-0.25)),
        ("date", json!("Authored day يس")),
        ("single-choice", json!("Unavailable historical option")),
        ("multiple-choice", json!(["Unavailable", "中文"])),
        ("links", json!([{"kind":"scene","id":"external-scene"}])),
    ];
    for (kind, value) in kinds {
        doc["fields"][0]["kind"] = json!(kind);
        doc["records"][0]["values"]["links"] = value;
        let before = doc.clone();
        guard::validate_transition(None, &doc).unwrap();
        assert_eq!(doc, before);
        let mut invalid = doc.clone();
        invalid["records"][0]["values"]["links"] = json!({"wrong":"shape"});
        assert!(
            guard::validate_transition(None, &invalid).is_err(),
            "{kind}"
        );
    }
}
#[test]
fn field_definition_changes_and_removal_cannot_invalidate_stored_values() {
    let doc = fixture("external-scene");
    for mode in ["kind", "recordKind", "remove"] {
        let mut invalid = doc.clone();
        match mode {
            "kind" => invalid["fields"][0]["kind"] = json!("number"),
            "recordKind" => invalid["fields"][0]["recordKind"] = json!("location"),
            _ => invalid["fields"] = json!([]),
        };
        assert!(
            guard::validate_transition(Some(&doc), &invalid).is_err(),
            "{mode}"
        );
    }
    let mut hidden = doc.clone();
    hidden["fields"][0]["hidden"] = json!(true);
    hidden["fields"][0]["label"] = json!("Renamed يس");
    guard::validate_transition(Some(&doc), &hidden).unwrap();
    assert_eq!(hidden["records"], doc["records"]);
}
#[test]
fn new_owned_references_are_checked_but_unchanged_missing_targets_remain_exact() {
    let doc = fixture("external-scene");
    let mut invalid = doc.clone();
    invalid["facts"][0]["subjectId"] = json!("external-scene");
    assert!(guard::validate_transition(Some(&doc), &invalid).is_err());
    let mut legacy = doc.clone();
    legacy["facts"][0]["subjectId"] = json!("deleted-record");
    legacy["facts"][0]["eventId"] = json!("deleted-event");
    legacy["fields"] = json!([]);
    guard::validate_compatibility(&legacy).unwrap();
    let mut unrelated = legacy.clone();
    unrelated["records"][0]["name"] = json!("Renamed نور");
    unrelated["facts"][0]["source"] = json!("Unrelated exact e\u{301}");
    guard::validate_transition(Some(&legacy), &unrelated).unwrap();
    assert_eq!(
        unrelated["records"][0]["values"],
        legacy["records"][0]["values"]
    );
    assert!(guard::validate_transition(None, &legacy).is_err());
}
#[test]
fn unchanged_duplicate_history_is_not_a_license_to_increase_or_merge_identity_collisions() {
    let mut legacy = fixture("external-scene");
    let duplicate = legacy["facts"][0].clone();
    legacy["facts"].as_array_mut().unwrap().push(duplicate);
    let mut unrelated = legacy.clone();
    unrelated["records"][0]["notes"] = json!("Unrelated edit");
    guard::validate_transition(Some(&legacy), &unrelated).unwrap();
    unrelated["facts"]
        .as_array_mut()
        .unwrap()
        .push(legacy["facts"][0].clone());
    assert!(guard::validate_transition(Some(&legacy), &unrelated).is_err());
    let left = fixture("external-scene");
    let mut right = left.clone();
    right["facts"][0]["value"] = json!("Other value");
    let mut merged = left.clone();
    merged["facts"]
        .as_array_mut()
        .unwrap()
        .push(right["facts"][0].clone());
    assert!(guard::validate_resolution(&[&left, &right], &merged).is_err());
    guard::validate_resolution(&[&legacy], &legacy).unwrap();
}
#[test]
fn anonymous_legacy_rows_are_preserved_but_cannot_be_copied_as_new_anonymous_rows() {
    let legacy = json!({"schemaVersion":1,"issues":[{"links":[],"notes":"exact مِ"}]});
    let mut unrelated = legacy.clone();
    unrelated["notes"] = json!("Unrelated edit");
    guard::validate_transition(Some(&legacy), &unrelated).unwrap();
    let anonymous = legacy["issues"][0].clone();
    unrelated["issues"].as_array_mut().unwrap().push(anonymous);
    assert!(guard::validate_transition(Some(&legacy), &unrelated).is_err());
    assert!(guard::validate_transition(None, &legacy).is_err());
}
#[test]
fn canonical_variants_owned_applicability_and_typed_link_shapes_are_checked() {
    let mut doc = fixture("external-scene");
    doc["lanes"] = json!([{"id":"main","name":"Main","kind":"main"}]);
    doc["events"] = json!([{"id":"event","title":"Event","notes":"","laneIds":["main"],"sceneIds":["external-scene"],"time":{"kind":"fixed","day":-0.25}}]);
    doc["constraints"] = json!([{"id":"constraint","beforeId":"event","afterId":"event","minimumDays":0,"strict":false,"confirmed":false,"notes":"Informal"}]);
    let cases = [
        ("fields", "kind", json!("unsupported")),
        ("fields", "recordKind", json!("scene")),
        ("records", "kind", json!("scene")),
        ("records", "links", json!(["event"])),
        ("facts", "observerId", json!("external-scene")),
        ("facts", "eventId", json!("external-scene")),
        ("facts", "status", json!("belief")),
        ("lanes", "kind", json!("parallel-universe")),
        ("events", "laneIds", json!(["external-scene"])),
        ("events", "laneIds", json!(["main", "main"])),
        ("constraints", "beforeId", json!("external-scene")),
        ("constraints", "minimumDays", json!(-1)),
    ];
    guard::validate_transition(None, &doc).unwrap();
    for (collection, key, value) in cases {
        let mut invalid = doc.clone();
        invalid[collection][0][key] = value;
        assert!(
            guard::validate_transition(Some(&doc), &invalid).is_err(),
            "{collection}.{key}"
        );
    }
    let mut proposal = doc.clone();
    proposal["facts"][0]["status"] = json!("proposal");
    proposal["facts"][0]["confirmed"] = json!(false);
    guard::validate_transition(Some(&doc), &proposal).unwrap();
    let mut hybrid = proposal.clone();
    hybrid["facts"][0]["confirmed"] = json!(true);
    assert!(guard::validate_resolution(&[&proposal, &doc], &hybrid).is_err());
}
#[test]
fn board_and_evidence_identity_namespaces_exclude_external_scene_ids() {
    let mut doc = fixture("external-scene");
    doc["boards"] = json!([{"id":"board","name":"Board","groups":[{"id":"group","name":"Group"}],"cards":[{"id":"external-scene","title":"Card","body":"","kind":"scene","reference":{"kind":"scene","id":"external-scene"},"x":0,"y":1.25,"groupId":"group","labels":[]},{"id":"other-card","kind":"note"}],"edges":[{"id":"edge","fromId":"external-scene","toId":"other-card","kind":"reference","label":"Exact 中文","confirmed":false}]}]);
    doc["issues"] = json!([{"id":"issue","category":"Continuity","description":"Manual","importance":"medium","links":[],"status":"open","resolution":"","evidence":[{"id":"evidence","link":{"kind":"scene","id":"external-scene"},"quote":"مِ","sourceRevision":"source","notes":""}]}]);
    guard::validate_transition(None, &doc).unwrap();
    for mode in [
        "card-core-id",
        "edge-card-id",
        "evidence-core-id",
        "invalid-edge",
        "invalid-card",
        "invalid-evidence",
        "foreign-group",
    ] {
        let mut invalid = doc.clone();
        match mode {
            "card-core-id" => invalid["boards"][0]["cards"][0]["id"] = json!("hero"),
            "edge-card-id" => invalid["boards"][0]["edges"][0]["id"] = json!("other-card"),
            "evidence-core-id" => invalid["issues"][0]["evidence"][0]["id"] = json!("fact"),
            "invalid-edge" => {
                invalid["boards"][0]["edges"][0]["kind"] = json!("hidden-semantic-kind")
            }
            "invalid-card" => invalid["boards"][0]["cards"][0]["kind"] = json!("plotPoint"),
            "invalid-evidence" => {
                invalid["issues"][0]["evidence"][0]["link"] =
                    json!({"kind":"plotPoint","id":"point"})
            }
            _ => invalid["boards"][0]["cards"][0]["groupId"] = json!("external-scene"),
        }
        assert!(
            guard::validate_transition(Some(&doc), &invalid).is_err(),
            "{mode}"
        );
    }
}
fn mutate_archive(encoded: &str, revision_id: &str, payload: Value) -> String {
    let mut archive: Value = serde_json::from_str(encoded).unwrap();
    let mut data: Value =
        serde_json::from_str(archive["entries"][0]["content"].as_str().unwrap()).unwrap();
    let revision = data["revisions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["id"] == revision_id)
        .unwrap();
    revision["payload"] = payload;
    revision["checksum"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&revision["payload"]).unwrap())
    ));
    let content = serde_json::to_string(&data).unwrap();
    archive["entries"][0]["size"] = json!(content.len());
    archive["entries"][0]["sha256"] = json!(format!("{:x}", Sha256::digest(content.as_bytes())));
    archive["entries"][0]["content"] = json!(content);
    serde_json::to_string(&archive).unwrap()
}
#[tokio::test]
async fn checked_historical_archive_import_and_recovery_preserve_legacy_but_reauthorship_is_guarded(
) {
    let (_dir, s, pid, sid) = store().await;
    let base = fixture(&sid);
    let saved = s
        .save_project_data(&pid, "planning", base.clone(), None)
        .await
        .unwrap();
    let mut legacy = base.clone();
    legacy["facts"][0]["status"] = json!("historical-unknown");
    legacy["records"][0]["values"]["links"] = json!(["historical string"]);
    let archive = mutate_archive(
        &s.export_archive(&pid).await.unwrap(),
        &saved.revision_id,
        legacy,
    );
    let before = s.sync_cursor().await.unwrap();
    let projects = s.list_projects().await.unwrap();
    let mut corrupt: Value = serde_json::from_str(&archive).unwrap();
    corrupt["entries"][0]["sha256"] = json!("0".repeat(64));
    assert!(s.import_archive(&corrupt.to_string()).await.is_err());
    assert_eq!(s.sync_cursor().await.unwrap(), before);
    assert_eq!(s.list_projects().await.unwrap(), projects);
    let imported = s.import_archive(&archive).await.unwrap();
    let pid = imported.project.id;
    let retained = s
        .load_project_data(&pid, "planning")
        .await
        .unwrap()
        .unwrap();
    let point = s.sync_cursor().await.unwrap();
    let point_id = s.changes(point.sequence - 1, 1).await.unwrap()[0]
        .id
        .clone();
    let mut unrelated = retained.payload.clone();
    unrelated["records"][0]["notes"] = json!("Unrelated exact مِ e\u{301}");
    let edited = s
        .save_project_data_checked(
            &pid,
            "planning",
            unrelated,
            Some(retained.revision_id.clone()),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(edited.payload["facts"], retained.payload["facts"]);
    assert_eq!(
        edited.payload["records"][0]["values"],
        retained.payload["records"][0]["values"]
    );
    let before = s.sync_cursor().await.unwrap();
    let mut reauthored = edited.payload.clone();
    reauthored["facts"][0]["status"] = json!("new-unsupported");
    assert!(s
        .save_project_data(
            &pid,
            "planning",
            reauthored,
            Some(edited.revision_id.clone())
        )
        .await
        .is_err());
    assert_eq!(s.sync_cursor().await.unwrap(), before);
    let heads = s
        .project_data_heads(&pid, "planning")
        .await
        .unwrap()
        .revision_ids;
    let recovered = s
        .resolve_project_data(&pid, "planning", edited.payload.clone(), heads.clone())
        .await
        .unwrap();
    assert_eq!(recovered.payload, edited.payload);
    let before = s.sync_cursor().await.unwrap();
    assert!(s
        .resolve_project_data(
            &pid,
            "planning",
            retained.payload.clone(),
            vec![retained.revision_id.clone()]
        )
        .await
        .is_err());
    assert_eq!(s.sync_cursor().await.unwrap(), before);
    let mut corrected = recovered.payload;
    corrected["facts"][0]["status"] = json!("fact");
    corrected["records"][0]["values"]["links"] = json!([]);
    let corrected = s
        .save_project_data(&pid, "planning", corrected, Some(recovered.revision_id))
        .await
        .unwrap();
    let history = s.project_data_history(&pid, "planning").await.unwrap();
    let before = s.sync_cursor().await.unwrap();
    assert!(s
        .resolve_project_data(
            &pid,
            "planning",
            edited.payload,
            vec![corrected.revision_id.clone()]
        )
        .await
        .is_err());
    assert_eq!(s.sync_cursor().await.unwrap(), before);
    assert_eq!(
        s.project_data_history(&pid, "planning").await.unwrap(),
        history
    );
    // Explicit time-machine recovery validates a checked historical source;
    // author resolution cannot claim an arbitrary ancestor as a current head.
    let restored = s
        .recover_time_piece(
            &pid,
            &point.epoch,
            point.sequence,
            &point_id,
            &format!("{pid}:planning"),
            "projectData",
            &retained.revision_id,
            vec![corrected.revision_id],
        )
        .await
        .unwrap();
    assert_eq!(restored.payload["facts"], retained.payload["facts"]);
    assert_eq!(
        restored.payload["records"][0]["values"],
        retained.payload["records"][0]["values"]
    );
    assert!(s.export_archive(&pid).await.is_ok());
}
