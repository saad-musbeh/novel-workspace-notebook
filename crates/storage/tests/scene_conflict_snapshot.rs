use novel_storage::{Scene, Storage, SyncCursor};
use serde_json::json;

async fn store() -> (tempfile::TempDir, Storage) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::open(&dir.path().join("conflicts.sqlite"))
        .await
        .unwrap();
    (dir, storage)
}
fn scene(title: &str) -> Scene {
    Scene {
        schema_version: 1,
        title: title.into(),
        document: json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"Exact مِ e\u{301} 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"},{"type":"underline"}]}]}]}),
    }
}
async fn exchange(from: &Storage, to: &Storage, pid: &str) {
    let source = from.sync_cursor().await.unwrap();
    let local = to.sync_cursor().await.unwrap();
    let page = from
        .pull_revisions(
            pid,
            SyncCursor {
                epoch: source.epoch.clone(),
                sequence: 0,
            },
            100,
        )
        .await
        .unwrap();
    to.ingest_revisions(
        pid,
        &local.epoch,
        &source.epoch,
        from.sync_manifest(pid).await.unwrap(),
        page.revisions,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn concurrent_deletions_resolve_with_only_actual_leaves_preserving_projected_ancestor() {
    let (_da, a) = store().await;
    let (_db, b) = store().await;
    let p = a.create_project("Both delete").await.unwrap();
    let c = a.create_chapter(&p.id, "Chapter").await.unwrap();
    let original = a
        .create_scene(&c.id, scene("Original retained writing"))
        .await
        .unwrap();
    exchange(&a, &b, &p.id).await;
    let left = a
        .delete_scene(&original.id, &original.revision_id)
        .await
        .unwrap();
    let right = b
        .delete_scene(&original.id, &original.revision_id)
        .await
        .unwrap();
    exchange(&b, &a, &p.id).await;
    let marker = a.sync_cursor().await.unwrap();
    assert!(a
        .resolve_scene_deletion(&original.id, &original.revision_id, vec![left.clone()])
        .await
        .is_err());
    assert!(a
        .resolve_scene_deletion(
            &original.id,
            &original.revision_id,
            vec![left.clone(), left.clone(), right.clone()]
        )
        .await
        .is_err());
    assert!(a
        .resolve_scene_deletion(&original.id, &left, vec![right.clone()])
        .await
        .is_err());
    assert!(a
        .resolve_scene_deletion(
            &original.id,
            &original.revision_id,
            vec![left.clone(), right.clone(), original.revision_id.clone()]
        )
        .await
        .is_err());
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
    let resolved = a
        .resolve_scene_deletion(
            &original.id,
            &original.revision_id,
            vec![left.clone(), right.clone()],
        )
        .await
        .unwrap();
    let revision = a
        .load_entity_revision(&p.id, &original.id, "scene", &resolved)
        .await
        .unwrap();
    assert_eq!(revision.parents, [left, right]);
    assert_eq!(revision.entity_type, "tombstone");
    assert_eq!(
        a.scene_heads(&original.id).await.unwrap().revision_ids,
        [resolved.clone()]
    );
    assert!(a.open_project_index(&p.id).await.unwrap().scenes.is_empty());
    assert_eq!(a.deleted_scenes(&p.id).await.unwrap()[0], original);
    assert_eq!(
        a.load_revision(&original.id, &original.revision_id)
            .await
            .unwrap()
            .scene,
        original.scene
    );
    assert_eq!(a.changes(marker.sequence, 100).await.unwrap().len(), 1);
    exchange(&a, &b, &p.id).await;
    assert_eq!(
        b.scene_heads(&original.id).await.unwrap().revision_ids,
        [resolved]
    );
    assert!(b.open_project_index(&p.id).await.unwrap().scenes.is_empty());
}

#[tokio::test]
async fn snapshots_cover_deleted_and_mixed_choices_preserve_every_immutable_version() {
    let (_da, a) = store().await;
    let (_db, b) = store().await;
    let p = a.create_project("Mixed recover").await.unwrap();
    let c = a.create_chapter(&p.id, "Chapter").await.unwrap();
    let original = a.create_scene(&c.id, scene("Original")).await.unwrap();
    exchange(&a, &b, &p.id).await;
    let edited = a
        .save_scene(
            &original.id,
            scene("Offline writing"),
            Some(original.revision_id.clone()),
        )
        .await
        .unwrap();
    let deletion = b
        .delete_scene(&original.id, &original.revision_id)
        .await
        .unwrap();
    exchange(&b, &a, &p.id).await;
    let marker = a.sync_cursor().await.unwrap();
    let capture = a
        .scene_conflict_snapshot(&p.id, &original.id)
        .await
        .unwrap();
    assert!(!capture.deleted);
    assert_eq!(capture.projected_scene, edited.scene);
    assert_eq!(capture.heads.len(), 2);
    let entries = a.scene_conflict_index(&p.id).await.unwrap();
    assert_eq!(entries[0].head_count, 2);
    assert!(!entries[0].deleted);
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
    let hidden = a
        .resolve_scene_conflict_checked(&capture, None)
        .await
        .unwrap();
    assert!(hidden.deleted);
    assert_eq!(hidden.heads.len(), 1);
    assert_eq!(hidden.projected_revision_id, edited.revision_id);
    assert_eq!(hidden.projected_scene, edited.scene);
    let parents = a
        .load_entity_revision(&p.id, &original.id, "scene", &hidden.heads[0].revision_id)
        .await
        .unwrap()
        .parents;
    assert!(parents.contains(&edited.revision_id) && parents.contains(&deletion));
    let entries = a.scene_conflict_index(&p.id).await.unwrap();
    assert!(entries[0].deleted);
    assert!(a.load_project_scene(&p.id, &original.id).await.is_err());
    let restored = a
        .resolve_scene_conflict_checked(&hidden, Some(original.scene.clone()))
        .await
        .unwrap();
    assert!(!restored.deleted);
    assert_eq!(restored.projected_scene, original.scene);
    assert_eq!(restored.heads.len(), 1);
    assert_eq!(
        a.load_revision(&original.id, &edited.revision_id)
            .await
            .unwrap(),
        edited
    );
    assert_eq!(
        a.load_revision(&original.id, &original.revision_id)
            .await
            .unwrap()
            .scene,
        original.scene
    );
    let copy = a
        .import_archive(&a.export_archive(&p.id).await.unwrap())
        .await
        .unwrap();
    assert_eq!(copy.scenes[0].scene, original.scene);
    assert_eq!(
        a.history(&copy.scenes[0].id).await.unwrap().len(),
        a.history(&original.id).await.unwrap().len()
    );
    let marker = a.sync_cursor().await.unwrap();
    assert!(a
        .resolve_scene_conflict_checked(&restored, None)
        .await
        .is_err());
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
}

#[tokio::test]
async fn original_full_head_and_placement_capture_refuse_stale_forged_or_foreign_choices() {
    let (_dir, a) = store().await;
    let p = a.create_project("Fresh capture").await.unwrap();
    let c = a.create_chapter(&p.id, "Chapter").await.unwrap();
    let other = a.create_chapter(&p.id, "Other").await.unwrap();
    let original = a.create_scene(&c.id, scene("Original")).await.unwrap();
    let current = a
        .save_scene(
            &original.id,
            scene("Current"),
            Some(original.revision_id.clone()),
        )
        .await
        .unwrap();
    a.preserve_conflict(&original.id, scene("Other retained"), &original.revision_id)
        .await
        .unwrap();
    let capture = a
        .scene_conflict_snapshot(&p.id, &original.id)
        .await
        .unwrap();
    let mut duplicate = capture.clone();
    duplicate.heads.push(duplicate.heads[0].clone());
    let mut omitted = capture.clone();
    omitted.heads.pop();
    let mut altered = capture.clone();
    altered.projected_revision_id = original.revision_id.clone();
    let mut placement = capture.clone();
    placement.placement_heads.clear();
    let marker = a.sync_cursor().await.unwrap();
    for invalid in [duplicate, omitted, altered, placement] {
        assert!(a
            .resolve_scene_conflict_checked(&invalid, Some(current.scene.clone()))
            .await
            .is_err());
    }
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
    let mut foreign = capture.clone();
    foreign.project_id = a.create_project("Foreign").await.unwrap().id;
    let marker = a.sync_cursor().await.unwrap();
    assert!(a
        .resolve_scene_conflict_checked(&foreign, None)
        .await
        .is_err());
    assert!(a
        .scene_conflict_snapshot(&foreign.project_id, &original.id)
        .await
        .is_err());
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
    a.preserve_conflict(
        &original.id,
        scene("Unseen new head"),
        &original.revision_id,
    )
    .await
    .unwrap();
    let marker = a.sync_cursor().await.unwrap();
    assert!(a
        .resolve_scene_conflict_checked(&capture, None)
        .await
        .unwrap_err()
        .contains("changed"));
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
    let capture = a
        .scene_conflict_snapshot(&p.id, &original.id)
        .await
        .unwrap();
    a.move_scene(&original.id, &other.id, 0).await.unwrap();
    let marker = a.sync_cursor().await.unwrap();
    assert!(a
        .resolve_scene_conflict_checked(&capture, Some(current.scene.clone()))
        .await
        .unwrap_err()
        .contains("changed"));
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
    let capture = a
        .scene_conflict_snapshot(&p.id, &original.id)
        .await
        .unwrap();
    let mut malformed = current.scene.clone();
    malformed.schema_version = 99;
    assert!(a
        .resolve_scene_conflict_checked(&capture, Some(malformed))
        .await
        .is_err());
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
    let writing = a
        .resolve_scene_conflict_checked(&capture, Some(current.scene.clone()))
        .await
        .unwrap();
    assert!(!writing.deleted);
    assert_eq!(writing.chapter_id, other.id);
    assert_eq!(writing.projected_scene, current.scene);
}

#[tokio::test]
async fn competing_deleted_snapshot_can_keep_deletion_without_resurrecting_ancestor() {
    let (_da, a) = store().await;
    let (_db, b) = store().await;
    let p = a.create_project("Deleted conflict").await.unwrap();
    let c = a.create_chapter(&p.id, "Chapter").await.unwrap();
    let original = a.create_scene(&c.id, scene("Ancestor")).await.unwrap();
    exchange(&a, &b, &p.id).await;
    a.delete_scene(&original.id, &original.revision_id)
        .await
        .unwrap();
    b.delete_scene(&original.id, &original.revision_id)
        .await
        .unwrap();
    exchange(&b, &a, &p.id).await;
    let capture = a
        .scene_conflict_snapshot(&p.id, &original.id)
        .await
        .unwrap();
    assert!(capture.deleted);
    assert_eq!(capture.heads.len(), 2);
    assert!(capture.heads.iter().all(|h| h.scene.is_none()));
    let kept = a
        .resolve_scene_conflict_checked(&capture, None)
        .await
        .unwrap();
    assert!(kept.deleted);
    assert_eq!(kept.projected_scene, original.scene);
    assert_eq!(kept.heads.len(), 1);
    assert!(a.planning_read_snapshot(&p.id).await.is_ok());
    assert!(a.open_project_index(&p.id).await.unwrap().scenes.is_empty());
}

#[tokio::test]
async fn failed_resolution_and_corrupt_checked_previews_preserve_heads_and_data() {
    let (dir, a) = store().await;
    let p = a.create_project("Failed choices").await.unwrap();
    let c = a.create_chapter(&p.id, "Chapter").await.unwrap();
    let original = a.create_scene(&c.id, scene("Original")).await.unwrap();
    let current = a
        .save_scene(
            &original.id,
            scene("Current"),
            Some(original.revision_id.clone()),
        )
        .await
        .unwrap();
    let other = a
        .preserve_conflict(&original.id, scene("Other"), &original.revision_id)
        .await
        .unwrap();
    let capture = a
        .scene_conflict_snapshot(&p.id, &original.id)
        .await
        .unwrap();
    let marker = a.sync_cursor().await.unwrap();
    let raw = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        dir.path().join("conflicts.sqlite").display()
    ))
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER fail_recovery BEFORE INSERT ON revision_envelope WHEN NEW.operation IN('resolve','resolveDelete') BEGIN SELECT RAISE(ABORT,'synthetic resolution fault'); END").execute(&raw).await.unwrap();
    assert!(a
        .resolve_scene_conflict_checked(&capture, None)
        .await
        .is_err());
    assert!(a
        .resolve_scene_conflict_checked(&capture, Some(original.scene.clone()))
        .await
        .is_err());
    assert_eq!(
        a.scene_conflict_snapshot(&p.id, &original.id)
            .await
            .unwrap(),
        capture
    );
    assert_eq!(a.load_scene(&original.id).await.unwrap(), current);
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
    sqlx::query("DROP TRIGGER fail_recovery")
        .execute(&raw)
        .await
        .unwrap();
    // Checksum fault injection only changes this disposable synthetic fixture.
    sqlx::query("DROP TRIGGER immutable_revision_update")
        .execute(&raw)
        .await
        .unwrap();
    sqlx::query("UPDATE revision_envelope SET checksum='bad' WHERE id=?")
        .bind(other.revision_id)
        .execute(&raw)
        .await
        .unwrap();
    assert!(a
        .scene_conflict_snapshot(&p.id, &original.id)
        .await
        .unwrap_err()
        .contains("checksum"));
    assert!(a
        .resolve_scene_conflict_checked(&capture, None)
        .await
        .is_err());
    assert_eq!(a.sync_cursor().await.unwrap(), marker);
    raw.close().await;
}
