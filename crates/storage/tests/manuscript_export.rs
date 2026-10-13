use novel_storage::*;
use serde_json::json;

fn scene(text: &str) -> Scene {
    Scene {
        schema_version: 1,
        title: "Synthetic export scene يس".into(),
        document: json!({"type":"doc","content":[{"type":"paragraph","attrs":{"lang":"ar","dir":"rtl"},"content":[{"type":"text","text":text}]}]}),
    }
}

async fn store() -> (tempfile::TempDir, Storage) {
    let directory = tempfile::tempdir().unwrap();
    let storage = Storage::open(&directory.path().join("export.sqlite")).await.unwrap();
    (directory, storage)
}

async fn project(storage: &Storage, title: &str, text: &str) -> (Project, SceneRecord) {
    let project = storage.create_project(title).await.unwrap();
    let chapter = storage.create_chapter(&project.id, "Synthetic chapter").await.unwrap();
    let scene = storage.create_scene(&chapter.id, scene(text)).await.unwrap();
    (project, scene)
}

async fn exchange(from: &Storage, to: &Storage, project_id: &str, after: i64) {
    let remote = from.sync_cursor().await.unwrap();
    let page = from.pull_revisions(
        project_id,
        SyncCursor { epoch: remote.epoch.clone(), sequence: after },
        100,
    ).await.unwrap();
    let local = to.sync_cursor().await.unwrap();
    to.ingest_revisions(
        project_id,
        &local.epoch,
        &remote.epoch,
        from.open_project(project_id).await.unwrap(),
        page.revisions,
    ).await.unwrap();
}

#[tokio::test]
async fn clean_snapshot_matches_open_project_and_is_read_only() {
    let (_directory, storage) = store().await;
    let (project, first) = project(&storage, "Synthetic Arabic 中文", "سلام مِ 👩🏽‍🚀").await;
    let chapter = storage.create_chapter(&project.id, "Second synthetic chapter").await.unwrap();
    storage.create_scene(&chapter.id, scene("Later synthetic text")).await.unwrap();
    let expected = storage.open_project(&project.id).await.unwrap();
    let revisions_before = storage.changes(0, 100).await.unwrap();
    let cursor_before = storage.sync_cursor().await.unwrap();

    let snapshot = storage.manuscript_export_snapshot(&project.id).await.unwrap();
    assert_eq!(snapshot, expected);
    assert_eq!(storage.manuscript_export_snapshot(&project.id).await.unwrap(), expected);
    assert_eq!(storage.changes(0, 100).await.unwrap(), revisions_before);
    assert_eq!(storage.sync_cursor().await.unwrap(), cursor_before);
    assert_eq!(storage.load_scene(&first.id).await.unwrap().scene, first.scene);
}

#[tokio::test]
async fn current_head_and_stale_scene_leaf_conflicts_refuse_export() {
    let (_directory, storage) = store().await;
    let (project, original) = project(&storage, "Conflicted synthetic book", "Original draft").await;
    let current = storage.save_scene(&original.id, scene("Current head"), Some(original.revision_id.clone())).await.unwrap();
    storage.preserve_conflict(&original.id, scene("Stale branch leaf"), &original.revision_id).await.unwrap();
    assert_eq!(storage.load_scene(&original.id).await.unwrap(), current);
    assert!(storage.manuscript_export_snapshot(&project.id).await.is_err());
}

#[tokio::test]
async fn deletion_vs_offline_edit_conflict_refuses_export_until_resolved() {
    let (_dir_a, a) = store().await;
    let (_dir_b, b) = store().await;
    let (project, original) = project(&a, "Delete edit synthetic book", "Shared draft").await;
    exchange(&a, &b, &project.id, 0).await;
    let baseline = a.sync_cursor().await.unwrap().sequence;

    let edited = a.save_scene(&original.id, scene("Offline edited text"), Some(original.revision_id.clone())).await.unwrap();
    let deletion = b.delete_scene(&original.id, &original.revision_id).await.unwrap();
    exchange(&b, &a, &project.id, baseline).await;
    assert!(a.scene_heads(&original.id).await.unwrap().revision_ids.len() > 1);
    assert!(a.manuscript_export_snapshot(&project.id).await.is_err());

    let resolved = a.resolve_scene_deletion(&original.id, &edited.revision_id, vec![deletion]).await.unwrap();
    assert_eq!(a.scene_heads(&original.id).await.unwrap().revision_ids, vec![resolved]);
    let snapshot = a.manuscript_export_snapshot(&project.id).await.unwrap();
    assert_eq!(snapshot, a.open_project(&project.id).await.unwrap());
    assert!(snapshot.scenes.is_empty());
}

#[tokio::test]
async fn resolved_scene_conflict_can_be_exported() {
    let (_directory, storage) = store().await;
    let (project, original) = project(&storage, "Resolved synthetic book", "Original").await;
    let current = storage.save_scene(&original.id, scene("Current branch"), Some(original.revision_id.clone())).await.unwrap();
    let stale = storage.preserve_conflict(&original.id, scene("Stale branch"), &original.revision_id).await.unwrap();
    let merged = storage.resolve_scene(&original.id, scene("Author resolved both branches"), &current.revision_id, vec![stale.revision_id]).await.unwrap();
    assert!(storage.conflicts(&original.id).await.unwrap().is_empty());

    let snapshot = storage.manuscript_export_snapshot(&project.id).await.unwrap();
    assert_eq!(snapshot, storage.open_project(&project.id).await.unwrap());
    assert_eq!(snapshot.scenes[0].revision_id, merged.revision_id);
}

#[tokio::test]
async fn conflicts_in_another_project_do_not_block_selected_project_export() {
    let (_directory, storage) = store().await;
    let (selected, selected_scene) = project(&storage, "Selected clean book", "Clean synthetic text").await;
    let (other, other_scene) = project(&storage, "Other conflicted book", "Other synthetic text").await;
    let newer = storage.save_scene(&other_scene.id, scene("Other current"), Some(other_scene.revision_id.clone())).await.unwrap();
    storage.preserve_conflict(&other_scene.id, scene("Other stale branch"), &other_scene.revision_id).await.unwrap();
    assert_eq!(storage.load_scene(&other_scene.id).await.unwrap(), newer);

    let snapshot = storage.manuscript_export_snapshot(&selected.id).await.unwrap();
    assert_eq!(snapshot, storage.open_project(&selected.id).await.unwrap());
    assert_eq!(snapshot.scenes[0].scene, selected_scene.scene);
    assert!(storage.manuscript_export_snapshot(&other.id).await.is_err());
}

#[tokio::test]
async fn deleted_scene_tombstone_leaf_also_blocks_export() {
    let (_dir_a, a) = store().await;
    let (_dir_b, b) = store().await;
    let (project, scene_record) = project(&a, "Deleted synthetic book", "Will be deleted").await;
    exchange(&a, &b, &project.id, 0).await;
    let baseline = a.sync_cursor().await.unwrap().sequence;
    a.delete_scene(&scene_record.id, &scene_record.revision_id).await.unwrap();
    b.delete_scene(&scene_record.id, &scene_record.revision_id).await.unwrap();
    exchange(&a, &b, &project.id, baseline).await;
    exchange(&b, &a, &project.id, baseline).await;
    assert!(a.open_project(&project.id).await.unwrap().scenes.is_empty());
    assert_eq!(a.scene_heads(&scene_record.id).await.unwrap().revision_ids.len(), 2);
    assert!(a.manuscript_export_snapshot(&project.id).await.is_err());
}
