use novel_storage::{Scene, Storage, SyncCursor};
use serde_json::json;
async fn store() -> (tempfile::TempDir, Storage) {
    let directory = tempfile::tempdir().unwrap();
    let storage = Storage::open(&directory.path().join("read.sqlite")).await.unwrap();
    (directory, storage)
}
fn scene(text: &str) -> Scene { Scene { schema_version: 1, title: "Saved scene مِ 中文".into(), document: json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text,"marks":[{"type":"bold"}]}]}]}) } }
async fn exchange(from: &Storage, to: &Storage, pid: &str) {
    let remote = from.sync_cursor().await.unwrap(); let local = to.sync_cursor().await.unwrap();
    let page = from.pull_revisions(pid, SyncCursor { epoch: remote.epoch.clone(), sequence: 0 }, 100).await.unwrap();
    to.ingest_revisions(pid, &local.epoch, &remote.epoch, from.sync_manifest(pid).await.unwrap(), page.revisions).await.unwrap();
}
#[tokio::test]
async fn saved_planning_views_are_body_free_and_read_only_with_verified_data_heads() {
    let (_dir, storage) = store().await; let p = storage.create_project("Authored view").await.unwrap();
    let chapter = storage.create_chapter(&p.id, "Chapter").await.unwrap();
    let manuscript = storage.create_scene(&chapter.id, scene("PRIVATE SYNTHETIC BODY مِ 中文 𠀀 👩🏽‍🚀")).await.unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    let empty = storage.planning_read_snapshot(&p.id).await.unwrap(); assert!(empty.planning.is_none()); assert!(empty.scene_metadata.is_none());
    assert!(storage.load_project_data_unambiguous("missing-project", "planning").await.is_err());
    assert!(storage.load_project_data_unambiguous(&p.id, "unknown-kind").await.is_err());
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    let planning = storage.save_project_data(&p.id, "planning", json!({"schemaVersion":1,"notes":"Exact مِ 中文 𠀀 👩🏽‍🚀"}), None).await.unwrap();
    let metadata = storage.save_project_data(&p.id, "sceneMetadata", json!({"schemaVersion":1,"scenes":[{"sceneId":manuscript.id,"synopsis":"Explicit synopsis","status":"draft","tags":[],"eventIds":[],"viewpointId":null}]}), None).await.unwrap();
    let marker = storage.sync_cursor().await.unwrap(); let snapshot = storage.planning_read_snapshot(&p.id).await.unwrap();
    assert_eq!(snapshot.index, storage.open_project_index(&p.id).await.unwrap()); assert_eq!(snapshot.planning.unwrap(), planning); assert_eq!(snapshot.scene_metadata.unwrap(), metadata);
    let serialized = serde_json::to_string(&storage.planning_read_snapshot(&p.id).await.unwrap()).unwrap(); assert!(!serialized.contains("PRIVATE SYNTHETIC BODY")); assert!(!serialized.contains("document"));
    assert_eq!(storage.load_project_data_unambiguous(&p.id, "planning").await.unwrap(), Some(planning)); assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    assert_eq!(storage.load_scene(&manuscript.id).await.unwrap().revision_id, manuscript.revision_id);
}
#[tokio::test]
async fn concurrent_roots_and_saved_projection_conflicts_never_choose_a_planning_claim() {
    let (_a, primary) = store().await; let (_b, peer) = store().await; let project = primary.create_project("Conflicting notes").await.unwrap();
    let chapter = primary.create_chapter(&project.id, "Chapter").await.unwrap(); primary.create_scene(&chapter.id, scene("Exact writing")).await.unwrap(); exchange(&primary, &peer, &project.id).await;
    primary.save_project_data(&project.id, "planning", json!({"schemaVersion":1,"notes":"First author's value"}), None).await.unwrap();
    peer.save_project_data(&project.id, "planning", json!({"schemaVersion":1,"notes":"Other author's value"}), None).await.unwrap(); exchange(&peer, &primary, &project.id).await;
    let marker = primary.sync_cursor().await.unwrap(); assert!(primary.load_project_data_unambiguous(&project.id, "planning").await.unwrap_err().contains("competing planning"));
    assert!(primary.planning_read_snapshot(&project.id).await.unwrap_err().contains("competing planning")); assert_eq!(primary.sync_cursor().await.unwrap(), marker);
    let heads = primary.project_data_heads(&project.id, "planning").await.unwrap().revision_ids;
    primary.resolve_project_data(&project.id, "planning", json!({"schemaVersion":1,"notes":"Author's explicit resolution"}), heads).await.unwrap();
    assert_eq!(primary.planning_read_snapshot(&project.id).await.unwrap().planning.unwrap().payload["notes"], "Author's explicit resolution");
    // Competing scene metadata also cannot supply a viewpoint or event link.
    primary.save_project_data(&project.id, "sceneMetadata", json!({"schemaVersion":1,"scenes":[]}), None).await.unwrap();
    peer.save_project_data(&project.id, "sceneMetadata", json!({"schemaVersion":1,"scenes":[],"note":"Other source"}), None).await.unwrap(); exchange(&peer, &primary, &project.id).await;
    let marker = primary.sync_cursor().await.unwrap(); assert!(primary.planning_read_snapshot(&project.id).await.unwrap_err().contains("competing sceneMetadata")); assert_eq!(primary.sync_cursor().await.unwrap(), marker);
}
#[tokio::test]
async fn unresolved_manuscript_versions_and_foreign_projection_heads_refuse_saved_views() {
    let (dir, storage) = store().await; let project = storage.create_project("Scoped views").await.unwrap(); let chapter = storage.create_chapter(&project.id, "Chapter").await.unwrap(); let original = storage.create_scene(&chapter.id, scene("Original")).await.unwrap();
    let saved = storage.save_scene(&original.id, scene("Current"), Some(original.revision_id.clone())).await.unwrap(); storage.preserve_conflict(&original.id, scene("Other retained version"), &original.revision_id).await.unwrap();
    let marker = storage.sync_cursor().await.unwrap(); assert!(storage.planning_read_snapshot(&project.id).await.unwrap_err().contains("manuscript versions")); assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    let heads = storage.conflicts(&original.id).await.unwrap(); storage.resolve_scene(&original.id, saved.scene, &saved.revision_id, heads.into_iter().map(|r|r.id).collect()).await.unwrap(); assert!(storage.planning_read_snapshot(&project.id).await.is_ok());
    let local = storage.save_project_data(&project.id, "planning", json!({"schemaVersion":1}), None).await.unwrap(); let foreign = storage.create_project("Other novel").await.unwrap(); let foreign_data = storage.save_project_data(&foreign.id, "planning", json!({"schemaVersion":1,"notes":"Other novel's notes"}), None).await.unwrap();
    let connection = sqlx::SqlitePool::connect(&format!("sqlite://{}", dir.path().join("read.sqlite").display())).await.unwrap();
    sqlx::query("UPDATE project_data SET head=? WHERE project_id=? AND kind='planning'").bind(&foreign_data.revision_id).bind(&project.id).execute(&connection).await.unwrap();
    let marker = storage.sync_cursor().await.unwrap(); assert!(storage.load_project_data_unambiguous(&project.id, "planning").await.unwrap_err().contains("sole retained head")); assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    assert_eq!(storage.load_project_data_revision(&project.id, "planning", &local.revision_id).await.unwrap(), local);
    connection.close().await;
}
