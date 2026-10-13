use super::*;
use serde_json::json;

fn scene(text: &str) -> Scene {
    Scene {
        schema_version: 1,
        title: "Synthetic review مِ 中文".into(),
        document: json!({"type":"doc","content":[
            {"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":text,"marks":[{"type":"bold"},{"type":"underline"}]}]},
            {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"後 \u{200f} 𠀀 👩🏽‍🚀"},{"type":"hardBreak"},{"type":"text","text":"Final"}]}
        ]}),
    }
}
async fn store() -> (tempfile::TempDir, Storage, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::open(&dir.path().join("review.sqlite"))
        .await
        .unwrap();
    let project = storage.create_project("Review synthetic").await.unwrap();
    let chapter = storage
        .create_chapter(&project.id, "Chapter")
        .await
        .unwrap();
    (dir, storage, project.id, chapter.id)
}
fn source(record: &crate::SceneRecord) -> ReviewSource {
    ReviewSource {
        scene_id: record.id.clone(),
        revision_id: record.revision_id.clone(),
        hash: hash(crate::plain_text(&record.scene.document).as_bytes()),
    }
}

#[tokio::test]
async fn exact_unicode_multi_source_capture_echoes_order_without_writes() {
    let (_dir, storage, pid, chapter) = store().await;
    let first = storage
        .create_scene(&chapter, scene("مِ e\u{301} 中文 𠀀 👩🏽‍🚀"))
        .await
        .unwrap();
    let second = storage
        .create_scene(&chapter, scene("Another exact source"))
        .await
        .unwrap();
    let sources = vec![source(&second), source(&first)];
    let marker = storage.sync_cursor().await.unwrap();
    let result = storage.review_sources_check(&pid, &sources).await.unwrap();
    assert_eq!(result.sources, sources);
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    assert_eq!(
        storage.load_project_scene(&pid, &first.id).await.unwrap(),
        first
    );
    assert_eq!(
        storage.load_project_scene(&pid, &second.id).await.unwrap(),
        second
    );
    let mut wrong = sources.clone();
    // Normalizing combining marks would change evidence; hashes use exact bytes.
    wrong[1].hash = hash(crate::plain_text(&scene("مِ é 中文 𠀀 👩🏽‍🚀").document).as_bytes());
    assert!(storage
        .review_sources_check(&pid, &wrong)
        .await
        .unwrap_err()
        .contains("hash"));
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
}

#[tokio::test]
async fn stale_foreign_and_deleted_sources_refuse_atomically() {
    let (_dir, storage, pid, chapter) = store().await;
    let first = storage
        .create_scene(&chapter, scene("First"))
        .await
        .unwrap();
    let second = storage
        .create_scene(&chapter, scene("Second"))
        .await
        .unwrap();
    let sources = vec![source(&first), source(&second)];
    let new = storage
        .save_scene(
            &second.id,
            scene("Changed"),
            Some(second.revision_id.clone()),
        )
        .await
        .unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    assert!(storage
        .review_sources_check(&pid, &sources)
        .await
        .unwrap_err()
        .contains("changed"));
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    let foreign = storage.create_project("Foreign").await.unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    assert!(storage
        .review_sources_check(&foreign.id, &[source(&first)])
        .await
        .is_err());
    assert!(storage
        .review_sources_check("missing", &[source(&first)])
        .await
        .is_err());
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    storage
        .delete_scene(&second.id, &new.revision_id)
        .await
        .unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    let mut deleted = source(&second);
    deleted.revision_id = new.revision_id;
    deleted.hash = hash(crate::plain_text(&new.scene.document).as_bytes());
    assert!(storage
        .review_sources_check(&pid, &[source(&first), deleted])
        .await
        .is_err());
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
}

#[tokio::test]
async fn retained_competing_versions_are_never_reviewed_as_current() {
    let (_dir, storage, pid, chapter) = store().await;
    let original = storage
        .create_scene(&chapter, scene("Original"))
        .await
        .unwrap();
    storage
        .save_scene(
            &original.id,
            scene("Current"),
            Some(original.revision_id.clone()),
        )
        .await
        .unwrap();
    let current = storage
        .load_project_scene(&pid, &original.id)
        .await
        .unwrap();
    storage
        .preserve_conflict(&original.id, scene("Other retained"), &original.revision_id)
        .await
        .unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    assert!(storage
        .review_sources_check(&pid, &[source(&current)])
        .await
        .unwrap_err()
        .contains("competing"));
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
}

#[tokio::test]
async fn competing_scene_and_tombstone_leaves_refuse_even_with_live_projection() {
    let (_dir, storage, pid, chapter) = store().await;
    let original = storage
        .create_scene(&chapter, scene("Live writing"))
        .await
        .unwrap();
    storage
        .save_scene(
            &original.id,
            scene("Current live writing"),
            Some(original.revision_id.clone()),
        )
        .await
        .unwrap();
    let current = storage
        .load_project_scene(&pid, &original.id)
        .await
        .unwrap();
    let mut tx = storage.pool.begin().await.unwrap();
    crate::workspace::append(
        &mut tx,
        &pid,
        &original.id,
        "tombstone",
        vec![original.revision_id.clone()],
        json!({"deleted":true}),
        "delete",
        "synthetic-deletion",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    assert!(storage
        .review_sources_check(&pid, &[source(&current)])
        .await
        .unwrap_err()
        .contains("competing"));
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
}

#[tokio::test]
async fn invalid_shapes_hashes_bounds_and_duplicate_scenes_refuse_without_writes() {
    let (_dir, storage, pid, chapter) = store().await;
    let record = storage
        .create_scene(&chapter, scene("Exact"))
        .await
        .unwrap();
    let valid = source(&record);
    let marker = storage.sync_cursor().await.unwrap();
    assert!(storage.review_sources_check(&pid, &[]).await.is_err());
    assert!(storage
        .review_sources_check(&pid, &[valid.clone(), valid.clone()])
        .await
        .is_err());
    assert!(storage
        .review_sources_check(&pid, &vec![valid.clone(); 51])
        .await
        .is_err());
    assert!(storage
        .review_sources_check("bad/project", &[valid.clone()])
        .await
        .is_err());
    for (field, value) in [
        ("sceneId", json!(null)),
        ("revisionId", json!(7)),
        ("hash", json!(true)),
        ("extra", json!("ignored?")),
    ] {
        let mut raw = serde_json::to_value(&valid).unwrap();
        raw[field] = value;
        assert!(serde_json::from_value::<ReviewSource>(raw).is_err());
    }
    for bad_hash in [
        "f".repeat(63),
        "A".repeat(64),
        "g".repeat(64),
        "0".repeat(65),
    ] {
        let mut invalid = valid.clone();
        invalid.hash = bad_hash;
        assert!(storage
            .review_sources_check(&pid, &[invalid])
            .await
            .is_err());
    }
    for bad_id in ["", "foreign/id", &"a".repeat(129)] {
        let mut invalid = valid.clone();
        invalid.scene_id = bad_id.into();
        assert!(storage
            .review_sources_check(&pid, &[invalid])
            .await
            .is_err());
    }
    let mut wrong_revision = valid;
    wrong_revision.revision_id = "retained-but-unrelated".into();
    assert!(storage
        .review_sources_check(&pid, &[wrong_revision])
        .await
        .is_err());
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
}

#[tokio::test]
async fn corrupted_checksum_schema_payload_and_envelope_ownership_refuse() {
    let (_dir, storage, pid, chapter) = store().await;
    let record = storage
        .create_scene(&chapter, scene("Exact"))
        .await
        .unwrap();
    let capture = source(&record);
    let original: (String, String) =
        sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=?")
            .bind(&record.revision_id)
            .fetch_one(&storage.pool)
            .await
            .unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    // Fault injection is restricted to this disposable synthetic database.
    sqlx::query("DROP TRIGGER immutable_revision_update")
        .execute(&storage.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE revision_envelope SET checksum='bad' WHERE id=?")
        .bind(&record.revision_id)
        .execute(&storage.pool)
        .await
        .unwrap();
    assert!(storage
        .review_sources_check(&pid, &[capture.clone()])
        .await
        .unwrap_err()
        .contains("checksum"));
    sqlx::query("UPDATE revision_envelope SET checksum=?,schema_version=99 WHERE id=?")
        .bind(&original.1)
        .bind(&record.revision_id)
        .execute(&storage.pool)
        .await
        .unwrap();
    assert!(storage
        .review_sources_check(&pid, &[capture.clone()])
        .await
        .unwrap_err()
        .contains("schema"));
    let invalid = "{\"schemaVersion\":1,\"title\":\"Broken\",\"document\":{\"type\":\"doc\",\"content\":[{\"type\":\"foreign-node\"}]}}";
    sqlx::query("UPDATE revision_envelope SET schema_version=1,payload=?,checksum=? WHERE id=?")
        .bind(invalid)
        .bind(hash(invalid.as_bytes()))
        .bind(&record.revision_id)
        .execute(&storage.pool)
        .await
        .unwrap();
    assert!(storage
        .review_sources_check(&pid, &[capture.clone()])
        .await
        .unwrap_err()
        .contains("content"));
    sqlx::query(
        "UPDATE revision_envelope SET payload=?,checksum=?,entity_id='foreign-scene' WHERE id=?",
    )
    .bind(&original.0)
    .bind(&original.1)
    .bind(&record.revision_id)
    .execute(&storage.pool)
    .await
    .unwrap();
    assert!(storage
        .review_sources_check(&pid, &[capture])
        .await
        .is_err());
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
}

#[tokio::test]
async fn multiple_sources_share_one_read_snapshot_when_one_changes() {
    let (dir, storage, pid, chapter) = store().await;
    let writer = Storage::open(&dir.path().join("review.sqlite"))
        .await
        .unwrap();
    let first = storage
        .create_scene(&chapter, scene("First"))
        .await
        .unwrap();
    let second = storage
        .create_scene(&chapter, scene("Second"))
        .await
        .unwrap();
    let sources = vec![source(&first), source(&second)];
    let mut tx = storage.pool.begin().await.unwrap();
    // Establish the same read snapshot the first source query establishes.
    validate_current_source(&mut tx, &pid, &first.id, "scene", &first.revision_id)
        .await
        .unwrap();
    let changed = writer
        .save_scene(
            &second.id,
            scene("Writer committed during read"),
            Some(second.revision_id.clone()),
        )
        .await
        .unwrap();
    let marker = writer.sync_cursor().await.unwrap();
    check_snapshot(&mut tx, &pid, &sources).await.unwrap();
    tx.commit().await.unwrap();
    // A new snapshot refuses; checking evidence never reserves future state.
    assert!(storage.review_sources_check(&pid, &sources).await.is_err());
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    assert_eq!(storage.load_scene(&second.id).await.unwrap(), changed);
}
