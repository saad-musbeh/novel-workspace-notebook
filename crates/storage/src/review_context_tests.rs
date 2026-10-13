use super::*;
fn document() -> Value {
    json!({"schemaVersion":1,"fields":[],"records":[{"id":"c1","name":"Mira"}],"facts":[
    {"id":"f1","subjectId":"c1","property":"role","value":"مِ 中文 😀","status":"fact","confirmed":true,"eventId":null,"untilEventId":null},
    {"id":"belief","subjectId":"c1","property":"rumor","value":"Never canon","status":"rumor","confirmed":false,"eventId":null,"untilEventId":null},
    {"id":"proposed","subjectId":"c1","property":"role","value":"Not approved","status":"fact","confirmed":false,"eventId":null,"untilEventId":null}],
    "boards":[],"lanes":[],"events":[],"constraints":[],"issues":[],"plotPoints":[]})
}
fn scene(text: &str) -> Scene {
    Scene {
        schema_version: 1,
        title: "Synthetic consistency".into(),
        document: json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text,"marks":[{"type":"bold"}]}]}]}),
    }
}
async fn fixture() -> (
    tempfile::TempDir,
    Storage,
    String,
    SceneRecord,
    crate::ProjectDataSnapshot,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::open(&dir.path().join("context.sqlite"))
        .await
        .unwrap();
    let project = store.create_project("Synthetic context").await.unwrap();
    let chapter = store.create_chapter(&project.id, "First").await.unwrap();
    let record = store
        .create_scene(&chapter.id, scene("Exact saved scene مِ e\u{301} 中文 𠀀"))
        .await
        .unwrap();
    let planning = store
        .save_project_data(&project.id, "planning", document(), None)
        .await
        .unwrap();
    (dir, store, project.id, record, planning)
}
fn refs(snapshot: &ReviewContextSnapshot) -> Vec<ReviewSource> {
    snapshot
        .sources
        .iter()
        .map(|s| ReviewSource {
            scene_id: s.scene_id.clone(),
            revision_id: s.revision_id.clone(),
            hash: s.hash.clone(),
        })
        .collect()
}
#[test]
fn canonical_projection_matches_existing_utf8_fixture_and_excludes_noncanon() {
    let text = canonical_review_fact_text(&document()).unwrap();
    assert_eq!(
        text,
        r#"{"facts":[{"eventId":null,"id":"f1","property":"role","subjectId":"c1","untilEventId":null,"value":"مِ 中文 😀"}],"records":[{"id":"c1","name":"Mira"}],"schemaVersion":1}"#
    );
    assert_eq!(
        hash(text.as_bytes()),
        "3ffced9562053b9e0cfc0badb00cc4706a6ef1a60cae689c81fc4850ede6cbaf"
    );
    assert!(!text.contains("Never canon"));
    assert!(!text.contains("Not approved"));
    let mut bad = document();
    bad["facts"][0]["value"] = json!("𠀀".repeat(7501));
    assert!(canonical_review_fact_text(&bad)
        .unwrap_err()
        .contains("30 KB"));
    let mut bad = document();
    let duplicate = bad["facts"][0].clone();
    bad["facts"].as_array_mut().unwrap().push(duplicate);
    assert!(canonical_review_fact_text(&bad).is_err());
}
#[tokio::test]
async fn snapshot_captures_one_scene_and_explicit_confirmed_scope_without_writes() {
    let (_dir, store, pid, record, planning) = fixture().await;
    store
        .create_scene(&record.chapter_id, scene("Excluded other scene"))
        .await
        .unwrap();
    let marker = store.sync_cursor().await.unwrap();
    let snapshot = store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap();
    assert_eq!(snapshot.scene, record);
    assert_eq!(snapshot.sources[0].kind, "scene");
    assert_eq!(
        snapshot.sources[0].text,
        crate::plain_text(&record.scene.document)
    );
    assert_eq!(snapshot.sources[1].kind, "facts");
    assert_eq!(snapshot.sources[1].revision_id, planning.revision_id);
    assert_eq!(snapshot.scope.included_fact_ids, vec!["f1"]);
    assert_eq!(snapshot.scope.excluded_facts.len(), 2);
    assert_eq!(snapshot.scope.excluded_scene_count, 1);
    assert!(!snapshot.scope.timing_included);
    let sources = refs(&snapshot);
    assert_eq!(
        store
            .review_context_check(&pid, &sources)
            .await
            .unwrap()
            .sources,
        sources
    );
    assert_eq!(store.sync_cursor().await.unwrap(), marker);
}
#[tokio::test]
async fn one_transaction_never_mixes_scene_and_planning_around_concurrent_writes() {
    let (dir, store, pid, record, planning) = fixture().await;
    let writer = Storage::open(&dir.path().join("context.sqlite"))
        .await
        .unwrap();
    let before = store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap();
    let mut read = store.pool.begin().await.unwrap();
    let _: String = sqlx::query_scalar("SELECT head FROM manuscript_scene WHERE id=?")
        .bind(&record.id)
        .fetch_one(&mut *read)
        .await
        .unwrap();
    writer
        .save_scene(
            &record.id,
            scene("New scene"),
            Some(record.revision_id.clone()),
        )
        .await
        .unwrap();
    let mut changed = document();
    changed["facts"][0]["value"] = json!("New canonical value");
    writer
        .save_project_data(&pid, "planning", changed, Some(planning.revision_id))
        .await
        .unwrap();
    let coherent = capture(&mut read, &pid, &record.id).await.unwrap();
    assert_eq!(coherent, before);
    read.commit().await.unwrap();
    assert!(store
        .review_context_check(&pid, &refs(&coherent))
        .await
        .is_err());
    let newer = store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap();
    assert_eq!(newer.sources[0].text, "New scene");
    assert!(newer.sources[1].text.contains("New canonical value"));
}
#[tokio::test]
async fn checked_decision_carries_scene_and_full_planning_head_even_if_fact_bytes_stay_equal() {
    let (_dir, store, pid, record, planning) = fixture().await;
    let snapshot = store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap();
    let dependencies = vec![
        crate::SourceDependency {
            entity_id: record.id.clone(),
            entity_type: "scene".into(),
            revision_id: record.revision_id.clone(),
        },
        crate::SourceDependency {
            entity_id: format!("{pid}:planning"),
            entity_type: "projectData".into(),
            revision_id: planning.revision_id.clone(),
        },
    ];
    let settings = store
        .save_project_data_checked(
            &pid,
            "settings",
            json!({"schemaVersion":1}),
            None,
            &dependencies,
        )
        .await
        .unwrap();
    let mut changed = document();
    changed["issues"] = json!([{"id":"unrelated","title":"Authored note"}]);
    let next = store
        .save_project_data(&pid, "planning", changed, Some(planning.revision_id))
        .await
        .unwrap();
    let current = store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap();
    assert_eq!(current.sources[1].hash, snapshot.sources[1].hash);
    assert_ne!(next.revision_id, snapshot.sources[1].revision_id);
    assert!(store
        .review_context_check(&pid, &refs(&snapshot))
        .await
        .is_err());
    let marker = store.sync_cursor().await.unwrap();
    assert!(store
        .save_project_data_checked(
            &pid,
            "settings",
            json!({"schemaVersion":1}),
            Some(settings.revision_id.clone()),
            &dependencies
        )
        .await
        .is_err());
    assert_eq!(store.sync_cursor().await.unwrap(), marker);
    assert_eq!(
        store.load_project_scene(&pid, &record.id).await.unwrap(),
        record
    );
    assert_eq!(
        store
            .load_project_data_unambiguous(&pid, "settings")
            .await
            .unwrap()
            .unwrap()
            .revision_id,
        settings.revision_id
    );
}
#[tokio::test]
async fn unavailable_conflicted_deleted_foreign_or_corrupt_basis_refuses() {
    let (_dir, store, pid, record, _planning) = fixture().await;
    let foreign = store.create_project("Foreign").await.unwrap();
    assert!(store
        .review_context_snapshot(&foreign.id, &record.id)
        .await
        .is_err());
    let mut tx = store.pool.begin().await.unwrap();
    crate::workspace::append(
        &mut tx,
        &pid,
        &format!("{pid}:planning"),
        "projectData",
        vec![],
        document(),
        "planning",
        "competing-planning-context",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap_err()
        .contains("Conflict"));
    let (_dir, store, pid, record, planning) = fixture().await;
    // Insert a corrupt child in this disposable fixture; immutable revision
    // triggers stay enabled and the original envelope is never overwritten.
    sqlx::query("INSERT INTO revision_envelope(id,project_id,entity_id,entity_type,parents,payload,operation,schema_version,checksum,origin_epoch,origin_device) SELECT 'corrupt-context',project_id,entity_id,entity_type,?,payload,operation,schema_version,'bad',origin_epoch,origin_device FROM revision_envelope WHERE id=?")
        .bind(serde_json::to_string(&vec![&planning.revision_id]).unwrap()).bind(&planning.revision_id).execute(&store.pool).await.unwrap();
    sqlx::query(
        "UPDATE project_data SET head='corrupt-context' WHERE project_id=? AND kind='planning'",
    )
    .bind(&pid)
    .execute(&store.pool)
    .await
    .unwrap();
    assert!(store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap_err()
        .contains("checksum"));
    let (_dir, store, pid, record, _planning) = fixture().await;
    store
        .delete_scene(&record.id, &record.revision_id)
        .await
        .unwrap();
    assert!(store
        .review_context_snapshot(&pid, &record.id)
        .await
        .is_err());
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::open(&dir.path().join("missing.sqlite"))
        .await
        .unwrap();
    let project = store.create_project("No planning").await.unwrap();
    let chapter = store.create_chapter(&project.id, "First").await.unwrap();
    let record = store
        .create_scene(&chapter.id, scene("Saved scene"))
        .await
        .unwrap();
    assert!(store
        .review_context_snapshot(&project.id, &record.id)
        .await
        .unwrap_err()
        .contains("unavailable"));
}
#[tokio::test]
async fn context_never_truncates_unknown_rows_or_oversize_canon_and_rejects_forged_manifests() {
    let (_dir, store, pid, record, planning) = fixture().await;
    let snapshot = store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap();
    let sources = refs(&snapshot);
    for altered in [
        vec![sources[0].clone()],
        vec![sources[1].clone(), sources[0].clone()],
    ] {
        assert!(store.review_context_check(&pid, &altered).await.is_err());
    }
    let mut wrong = sources.clone();
    wrong[1].hash = "A".repeat(64);
    assert!(store.review_context_check(&pid, &wrong).await.is_err());
    wrong[1].hash = "0".repeat(64);
    assert!(store.review_context_check(&pid, &wrong).await.is_err());
    let mut no_canon = document();
    no_canon["facts"][0]["confirmed"] = json!(false);
    let next = store
        .save_project_data(&pid, "planning", no_canon, Some(planning.revision_id))
        .await
        .unwrap();
    let empty = store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap();
    assert!(empty.scope.included_fact_ids.is_empty());
    assert_eq!(empty.sources[1].revision_id, next.revision_id);
    assert!(store
        .review_context_check(&pid, &refs(&empty))
        .await
        .is_ok());
    let mut oversized = document();
    oversized["facts"][0]["value"] = json!("x".repeat(30001));
    store
        .save_project_data(&pid, "planning", oversized, Some(next.revision_id))
        .await
        .unwrap();
    assert!(store
        .review_context_check(&pid, &refs(&empty))
        .await
        .is_err());
    assert!(store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap_err()
        .contains("30 KB"));
}

#[tokio::test]
async fn empty_saved_context_remains_revision_bound_when_facts_are_created_later() {
    let (_dir, store, pid, record, planning) = fixture().await;
    let mut none = document();
    none["facts"] = json!([]);
    let empty_head = store
        .save_project_data(&pid, "planning", none, Some(planning.revision_id))
        .await
        .unwrap();
    let empty = store
        .review_context_snapshot(&pid, &record.id)
        .await
        .unwrap();
    assert!(empty.scope.included_fact_ids.is_empty());
    assert_eq!(
        empty.sources[1].text,
        r#"{"facts":[],"records":[],"schemaVersion":1}"#
    );
    assert_eq!(empty.sources[1].revision_id, empty_head.revision_id);
    assert!(store
        .review_context_check(&pid, &refs(&empty))
        .await
        .is_ok());
    let dependencies = vec![
        crate::SourceDependency {
            entity_id: record.id.clone(),
            entity_type: "scene".into(),
            revision_id: record.revision_id,
        },
        crate::SourceDependency {
            entity_id: format!("{pid}:planning"),
            entity_type: "projectData".into(),
            revision_id: empty_head.revision_id.clone(),
        },
    ];
    store
        .save_project_data(&pid, "planning", document(), Some(empty_head.revision_id))
        .await
        .unwrap();
    assert!(store
        .review_context_check(&pid, &refs(&empty))
        .await
        .is_err());
    let marker = store.sync_cursor().await.unwrap();
    assert!(store
        .save_project_data_checked(
            &pid,
            "settings",
            json!({"schemaVersion":1}),
            None,
            &dependencies
        )
        .await
        .is_err());
    assert_eq!(store.sync_cursor().await.unwrap(), marker);
    assert!(store
        .load_project_data_unambiguous(&pid, "settings")
        .await
        .unwrap()
        .is_none());
}
