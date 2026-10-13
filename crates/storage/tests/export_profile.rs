use novel_storage::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Connection, SqliteConnection};

fn scene(text: &str) -> Scene {
    Scene {
        schema_version: 1,
        title: "Synthetic profile scene 中文".into(),
        document: json!({"type":"doc","content":[{"type":"paragraph","attrs":{"lang":"zh-Hans","dir":"auto"},"content":[{"type":"text","text":text}]}]}),
    }
}

async fn store() -> (tempfile::TempDir, Storage) {
    let directory = tempfile::tempdir().unwrap();
    let storage = Storage::open(&directory.path().join("export-profile.sqlite")).await.unwrap();
    (directory, storage)
}

async fn raw(path: &std::path::Path) -> SqliteConnection {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false);
    SqliteConnection::connect_with(&options).await.unwrap()
}

async fn project(storage: &Storage, title: &str) -> (Project, SceneRecord) {
    let project = storage.create_project(title).await.unwrap();
    let chapter = storage.create_chapter(&project.id, "Synthetic chapter").await.unwrap();
    let scene = storage.create_scene(&chapter.id, scene("Original scene text 👩🏽‍🚀")).await.unwrap();
    (project, scene)
}

async fn exchange(from: &Storage, to: &Storage, project_id: &str, after: i64) {
    let remote = from.sync_cursor().await.unwrap();
    let page = from.pull_revisions(project_id, SyncCursor { epoch: remote.epoch.clone(), sequence: after }, 100).await.unwrap();
    let local = to.sync_cursor().await.unwrap();
    to.ingest_revisions(project_id, &local.epoch, &remote.epoch, from.open_project(project_id).await.unwrap(), page.revisions).await.unwrap();
}

fn profile_value() -> Value {
    serde_json::to_value(ExportProfile::default()).unwrap()
}

#[tokio::test]
async fn default_profile_snapshot_is_read_only_and_keeps_scene_heads() {
    let (_directory, storage) = store().await;
    let (project, scene_record) = project(&storage, "Default synthetic book").await;
    let before_bundle = storage.open_project(&project.id).await.unwrap();
    let before_history = storage.changes(0, 100).await.unwrap();
    let before_cursor = storage.sync_cursor().await.unwrap();

    let snapshot = storage.word_export_snapshot(&project.id, None).await.unwrap();
    assert_eq!(snapshot.bundle, before_bundle);
    assert_eq!(snapshot.profile, ExportProfile::default());
    assert_eq!(snapshot.profile_revision_id, None);
    assert_eq!(storage.word_export_snapshot(&project.id, None).await.unwrap(), snapshot);
    assert_eq!(storage.changes(0, 100).await.unwrap(), before_history);
    assert_eq!(storage.sync_cursor().await.unwrap(), before_cursor);
    assert_eq!(storage.load_scene(&scene_record.id).await.unwrap().revision_id, scene_record.revision_id);
}

#[tokio::test]
async fn profile_save_uses_cas_and_export_snapshot_rejects_stale_revision() {
    let (_directory, storage) = store().await;
    let (project, source_scene) = project(&storage, "CAS synthetic book").await;
    let value = profile_value();
    let saved = storage.save_project_data(&project.id, "exportProfile", value.clone(), None).await.unwrap();
    let snapshot = storage.word_export_snapshot(&project.id, Some(saved.revision_id.clone())).await.unwrap();
    assert_eq!(snapshot.profile_revision_id.as_deref(), Some(saved.revision_id.as_str()));
    assert_eq!(serde_json::to_value(snapshot.profile).unwrap(), value);

    assert!(storage.save_project_data(&project.id, "exportProfile", value.clone(), None).await.is_err());
    let updated = storage.save_project_data(&project.id, "exportProfile", value, Some(saved.revision_id.clone())).await.unwrap();
    assert!(storage.word_export_snapshot(&project.id, Some(saved.revision_id)).await.is_err());
    assert!(storage.word_export_snapshot(&project.id, None).await.is_err(), "missing expected head must not silently select a saved profile");
    assert_eq!(storage.word_export_snapshot(&project.id, Some(updated.revision_id)).await.unwrap().bundle.scenes[0].revision_id, source_scene.revision_id);
}

#[tokio::test]
async fn conflicting_profile_refuses_export_until_author_resolves_all_heads() {
    let (_dir_a, a) = store().await;
    let (_dir_b, b) = store().await;
    let (project, source_scene) = project(&a, "Conflicted synthetic book").await;
    exchange(&a, &b, &project.id, 0).await;
    let baseline = a.sync_cursor().await.unwrap().sequence;
    let mut left = profile_value();
    left["body"]["font"] = json!("Georgia");
    let mut right = profile_value();
    right["body"]["font"] = json!("Times New Roman");
    a.save_project_data(&project.id, "exportProfile", left.clone(), None).await.unwrap();
    b.save_project_data(&project.id, "exportProfile", right.clone(), None).await.unwrap();
    exchange(&a, &b, &project.id, baseline).await;
    exchange(&b, &a, &project.id, baseline).await;

    assert!(a.word_export_snapshot(&project.id, None).await.is_err());
    let heads = a.project_data_heads(&project.id, "exportProfile").await.unwrap();
    assert_eq!(heads.revision_ids.len(), 2);
    let projected = a.load_project_data(&project.id, "exportProfile").await.unwrap().unwrap();
    assert!(a.save_project_data(&project.id, "exportProfile", left.clone(), Some(projected.revision_id)).await.is_err(), "a projected head must not authorize save while multiple profile leaves exist");
    let resolved = a.resolve_project_data(&project.id, "exportProfile", left.clone(), heads.revision_ids).await.unwrap();
    let snapshot = a.word_export_snapshot(&project.id, Some(resolved.revision_id.clone())).await.unwrap();
    assert_eq!(serde_json::to_value(snapshot.profile).unwrap(), left);
    assert_eq!(snapshot.bundle.scenes[0].revision_id, source_scene.revision_id);
}

#[tokio::test]
async fn profile_roundtrips_through_sync_and_archive_without_changing_manuscript_heads() {
    let (_dir_a, a) = store().await;
    let (_dir_b, b) = store().await;
    let (project, source_scene) = project(&a, "Portable synthetic book").await;
    let profile = profile_value();
    let saved = a.save_project_data(&project.id, "exportProfile", profile.clone(), None).await.unwrap();
    let before_scene = a.load_scene(&source_scene.id).await.unwrap();
    exchange(&a, &b, &project.id, 0).await;
    let remote = b.word_export_snapshot(&project.id, Some(saved.revision_id.clone())).await.unwrap();
    assert_eq!(serde_json::to_value(remote.profile).unwrap(), profile);
    assert_eq!(remote.bundle.scenes[0].revision_id, before_scene.revision_id);

    let archive = a.export_archive(&project.id).await.unwrap();
    let imported = a.import_archive(&archive).await.unwrap();
    let saved_profile = a.load_project_data(&imported.project.id, "exportProfile").await.unwrap().unwrap();
    assert_eq!(saved_profile.payload, profile);
    // The profile is ordinary revisioned project data in the archive. Its head
    // need not preserve a source revision ID across archive identity remapping.
    let restored = a.word_export_snapshot(&imported.project.id, Some(saved_profile.revision_id)).await.unwrap();
    assert_eq!(serde_json::to_value(restored.profile).unwrap(), profile);
    assert_eq!(restored.bundle.scenes[0].scene, before_scene.scene);
}

#[tokio::test]
async fn profile_snapshot_survives_storage_reopen() {
    let (directory, storage) = store().await;
    let (project, _) = project(&storage, "Reopened synthetic book").await;
    let profile = profile_value();
    let saved = storage.save_project_data(&project.id, "exportProfile", profile.clone(), None).await.unwrap();
    drop(storage);
    let reopened = Storage::open(&directory.path().join("export-profile.sqlite")).await.unwrap();
    let snapshot = reopened.word_export_snapshot(&project.id, Some(saved.revision_id.clone())).await.unwrap();
    assert_eq!(serde_json::to_value(snapshot.profile).unwrap(), profile);
    assert_eq!(snapshot.profile_revision_id.as_deref(), Some(saved.revision_id.as_str()));
}

#[tokio::test]
async fn export_snapshot_rejects_corrupt_profile_checksum_and_invalid_hashed_payload() {
    let (directory, storage) = store().await;
    let (checksum_project, _) = project(&storage, "Checksum synthetic book").await;
    let checksum_revision = storage.save_project_data(&checksum_project.id, "exportProfile", profile_value(), None).await.unwrap();
    let mut connection = raw(&directory.path().join("export-profile.sqlite")).await;
    sqlx::query("DROP TRIGGER immutable_revision_update").execute(&mut connection).await.unwrap();
    sqlx::query("UPDATE revision_envelope SET payload='{}' WHERE id=?")
        .bind(&checksum_revision.revision_id).execute(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    assert!(storage.word_export_snapshot(&checksum_project.id, Some(checksum_revision.revision_id)).await.is_err(), "payload changed without checksum must be rejected");
}

#[tokio::test]
async fn export_snapshot_rejects_invalid_profile_payload_even_with_matching_checksum() {
    let (directory, storage) = store().await;
    let (project, _) = project(&storage, "Payload synthetic book").await;
    let saved = storage.save_project_data(&project.id, "exportProfile", profile_value(), None).await.unwrap();
    let payload = "{}";
    let checksum = format!("{:x}", Sha256::digest(payload.as_bytes()));
    let mut connection = raw(&directory.path().join("export-profile.sqlite")).await;
    sqlx::query("DROP TRIGGER immutable_revision_update").execute(&mut connection).await.unwrap();
    sqlx::query("UPDATE revision_envelope SET payload=?,checksum=? WHERE id=?")
        .bind(payload).bind(checksum).bind(&saved.revision_id).execute(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    assert!(storage.word_export_snapshot(&project.id, Some(saved.revision_id)).await.is_err(), "valid hash does not make an invalid export-profile payload acceptable");
}

#[tokio::test]
async fn export_snapshot_rejects_history_projection_that_disagrees_with_profile_leaf() {
    let (directory, storage) = store().await;
    let (project, _) = project(&storage, "Projection synthetic book").await;
    let first = storage.save_project_data(&project.id, "exportProfile", profile_value(), None).await.unwrap();
    let mut changed = profile_value();
    changed["body"]["font"] = json!("Georgia");
    let saved = storage.save_project_data(&project.id, "exportProfile", changed, Some(first.revision_id.clone())).await.unwrap();
    let mut connection = raw(&directory.path().join("export-profile.sqlite")).await;
    sqlx::query("UPDATE project_data SET head=? WHERE project_id=? AND kind='exportProfile'")
        .bind(&first.revision_id).bind(&project.id).execute(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    assert!(storage.word_export_snapshot(&project.id, Some(saved.revision_id)).await.is_err(), "profile history must not be exported when its current projection is corrupt");
}

#[tokio::test]
async fn invalid_profile_keys_ranges_fonts_and_controls_are_refused() {
    let (_directory, storage) = store().await;
    let (project, _) = project(&storage, "Invalid profile synthetic book").await;
    let base = profile_value();
    let invalid = [
        { let mut v=base.clone(); v["unexpected"]=json!(true); v },
        { let mut v=base.clone(); v["schemaVersion"]=json!(2); v },
        { let mut v=base.clone(); v["pageSize"]=json!("legal"); v },
        { let mut v=base.clone(); v["marginMm"]=json!(9.99); v },
        { let mut v=base.clone(); v["marginMm"]=json!(40.01); v },
        { let mut v=base.clone(); v["lineSpacing"]=json!("1.25"); v },
        { let mut v=base.clone(); v["body"]["font"]=json!(""); v },
        { let mut v=base.clone(); v["body"]["font"]=json!(" Bad Font "); v },
        { let mut v=base.clone(); v["body"]["font"]=json!("x".repeat(81)); v },
        { let mut v=base.clone(); v["body"]["font"]=json!("Bad\u{1}Font"); v },
        { let mut v=base.clone(); v["body"]["size"]=json!(7.5); v },
        { let mut v=base.clone(); v["chapter"]["size"]=json!(72.5); v },
        { let mut v=base.clone(); v["scene"]["surprise"]=json!(false); v },
    ];
    for value in invalid {
        assert!(storage.save_project_data(&project.id, "exportProfile", value, None).await.is_err());
    }
    assert!(storage.word_export_snapshot(&project.id, None).await.is_ok());
}
