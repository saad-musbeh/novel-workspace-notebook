//! Transaction, archive and local synthetic-replication regressions. No network.
use novel_storage::{ProjectBundle, Revision, Scene, Storage, SyncCursor};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Connection;
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn seal(mut pack: Value) -> Value {
    pack.as_object_mut().unwrap().remove("checksum");
    pack["checksum"] = json!(hash(&serde_json::to_vec(&pack).unwrap()));
    pack
}
fn payload(version: &str, text: &str) -> Value {
    json!({"schemaVersion":1,"packs":[seal(json!({"schemaVersion":1,"id":"synthetic.lexicon","name":"Synthetic test pack","language":"ar","variant":"test","version":version,"source":"Original synthetic fixture only","license":"Synthetic fixture permission","attribution":"Test generator","operations":["dictionary"],"entries":[{"term":text,"senses":[{"id":"sense","definition":"Synthetic meaning","partOfSpeech":"noun","synonyms":[],"related":[],"translations":[]}]}]}))]})
}
async fn store() -> (tempfile::TempDir, Storage) {
    let d = tempfile::tempdir().unwrap();
    let s = Storage::open(&d.path().join("test.sqlite")).await.unwrap();
    (d, s)
}
async fn novel(s: &Storage) -> (String, String) {
    let p = s.create_project("Pack history test").await.unwrap();
    let c = s.create_chapter(&p.id, "First").await.unwrap();
    let scene=s.create_scene(&c.id,Scene{schema_version:1,title:"مِ 中文".into(),document:json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Original e\u{301} 𠀀 \u{e000}"}]}]})}).await.unwrap();
    (p.id, scene.id)
}
async fn transfer(from: &Storage, to: &Storage, pid: &str, after: i64) {
    let remote = from.sync_cursor().await.unwrap();
    let local = to.sync_cursor().await.unwrap();
    let page = from
        .pull_revisions(
            pid,
            SyncCursor {
                epoch: remote.epoch.clone(),
                sequence: after,
            },
            1000,
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
async fn versions(s: &Storage, pid: &str) -> Vec<Revision> {
    s.project_data_history(pid, "languagePacks").await.unwrap()
}
fn mutate_archive(encoded: &str, revision_id: &str, changed: Value) -> String {
    let mut archive: Value = serde_json::from_str(encoded).unwrap();
    let mut data: Value =
        serde_json::from_str(archive["entries"][0]["content"].as_str().unwrap()).unwrap();
    let r = data["revisions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["id"] == revision_id)
        .unwrap();
    r["payload"] = changed;
    r["checksum"] = json!(hash(&serde_json::to_vec(&r["payload"]).unwrap()));
    let content = serde_json::to_string(&data).unwrap();
    archive["entries"][0]["size"] = json!(content.len());
    archive["entries"][0]["sha256"] = json!(hash(content.as_bytes()));
    archive["entries"][0]["content"] = json!(content);
    serde_json::to_string(&archive).unwrap()
}

#[tokio::test]
async fn both_save_apis_refuse_older_version_reuse_and_keep_exact_history() {
    for checked in [false, true] {
        let (_d, s) = store().await;
        let (pid, sid) = novel(&s).await;
        let original_scene = s.load_scene(&sid).await.unwrap();
        let one = payload("1", "مِ 𠀀 e\u{301}");
        let first = s
            .save_project_data(&pid, "languagePacks", one.clone(), None)
            .await
            .unwrap();
        let second = s
            .save_project_data(
                &pid,
                "languagePacks",
                payload("2", "new"),
                Some(first.revision_id.clone()),
            )
            .await
            .unwrap();
        let empty = s
            .save_project_data(
                &pid,
                "languagePacks",
                json!({"schemaVersion":1,"packs":[]}),
                Some(second.revision_id),
            )
            .await
            .unwrap();
        let before = s.sync_cursor().await.unwrap();
        let retained = versions(&s, &pid).await;
        for reused_version in ["1", "2"] {
            let bad = payload(reused_version, "different data");
            let result = if checked {
                s.save_project_data_checked(
                    &pid,
                    "languagePacks",
                    bad,
                    Some(empty.revision_id.clone()),
                    &[],
                )
                .await
            } else {
                s.save_project_data(&pid, "languagePacks", bad, Some(empty.revision_id.clone()))
                    .await
            };
            assert!(result.unwrap_err().contains("immutable"));
            assert_eq!(s.sync_cursor().await.unwrap(), before);
            assert_eq!(versions(&s, &pid).await, retained);
            assert_eq!(
                s.load_project_data(&pid, "languagePacks").await.unwrap(),
                Some(empty.clone())
            );
            assert_eq!(s.load_scene(&sid).await.unwrap(), original_scene);
        }
        let restored = s
            .save_project_data_checked(
                &pid,
                "languagePacks",
                one.clone(),
                Some(empty.revision_id),
                &[],
            )
            .await
            .unwrap();
        assert_eq!(restored.payload, one);
        assert_eq!(
            s.load_project_data_revision(&pid, "languagePacks", &first.revision_id)
                .await
                .unwrap(),
            first
        );
        let (other, _) = novel(&s).await;
        s.save_project_data(
            &other,
            "languagePacks",
            payload("1", "independent project data"),
            None,
        )
        .await
        .unwrap();
    }
}
#[tokio::test]
async fn resolution_checks_older_non_head_versions_and_plain_save_refuses_conflicts() {
    let (_a, a) = store().await;
    let (_b, b) = store().await;
    let (pid, _) = novel(&a).await;
    let one = payload("1", "original");
    let first = a
        .save_project_data(&pid, "languagePacks", one.clone(), None)
        .await
        .unwrap();
    transfer(&a, &b, &pid, 0).await;
    a.save_project_data(
        &pid,
        "languagePacks",
        payload("2", "left"),
        Some(first.revision_id.clone()),
    )
    .await
    .unwrap();
    b.save_project_data(
        &pid,
        "languagePacks",
        payload("3", "right"),
        Some(first.revision_id),
    )
    .await
    .unwrap();
    transfer(&b, &a, &pid, 0).await;
    let heads = a
        .project_data_heads(&pid, "languagePacks")
        .await
        .unwrap()
        .revision_ids;
    assert_eq!(heads.len(), 2);
    let before = a.sync_cursor().await.unwrap();
    let history = versions(&a, &pid).await;
    let parent = a
        .load_project_data(&pid, "languagePacks")
        .await
        .unwrap()
        .map(|s| s.revision_id);
    assert!(a
        .save_project_data(
            &pid,
            "languagePacks",
            payload("4", "no implicit resolution"),
            parent
        )
        .await
        .unwrap_err()
        .starts_with("Conflict"));
    assert!(a
        .resolve_project_data(
            &pid,
            "languagePacks",
            payload("1", "changed"),
            heads.clone()
        )
        .await
        .unwrap_err()
        .contains("immutable"));
    assert_eq!(a.sync_cursor().await.unwrap(), before);
    assert_eq!(versions(&a, &pid).await, history);
    assert_eq!(
        a.project_data_heads(&pid, "languagePacks")
            .await
            .unwrap()
            .revision_ids,
        heads
    );
    // A fresh receiver has the same competing roots without a projection. Neither
    // save API may pretend that parent=None means no retained pack history.
    let (_c, c) = store().await;
    transfer(&a, &c, &pid, 0).await;
    assert!(c
        .load_project_data(&pid, "languagePacks")
        .await
        .unwrap()
        .is_none());
    let unprojected_before = c.sync_cursor().await.unwrap();
    assert!(c
        .save_project_data(&pid, "languagePacks", payload("4", "new"), None)
        .await
        .unwrap_err()
        .starts_with("Conflict"));
    assert!(c
        .save_project_data_checked(&pid, "languagePacks", payload("4", "new"), None, &[])
        .await
        .unwrap_err()
        .starts_with("Conflict"));
    assert_eq!(c.sync_cursor().await.unwrap(), unprojected_before);
    assert_eq!(
        c.project_data_heads(&pid, "languagePacks")
            .await
            .unwrap()
            .revision_ids
            .len(),
        2
    );
    let resolved = a
        .resolve_project_data(&pid, "languagePacks", one.clone(), heads)
        .await
        .unwrap();
    assert_eq!(resolved.payload, one);
    assert_eq!(versions(&a, &pid).await.len(), history.len() + 1);
}
#[tokio::test]
async fn identical_concurrent_versions_and_duplicate_receipts_are_compatible() {
    let (_a, a) = store().await;
    let (_b, b) = store().await;
    let (pid, _) = novel(&a).await;
    let first = a
        .save_project_data(&pid, "languagePacks", payload("1", "original"), None)
        .await
        .unwrap();
    transfer(&a, &b, &pid, 0).await;
    let same = payload("2", "مِ 中文 𠀀");
    a.save_project_data(
        &pid,
        "languagePacks",
        same.clone(),
        Some(first.revision_id.clone()),
    )
    .await
    .unwrap();
    b.save_project_data(&pid, "languagePacks", same.clone(), Some(first.revision_id))
        .await
        .unwrap();
    transfer(&b, &a, &pid, 0).await;
    let before = a.sync_cursor().await.unwrap();
    transfer(&b, &a, &pid, 0).await;
    assert_eq!(a.sync_cursor().await.unwrap(), before);
    let heads = a
        .project_data_heads(&pid, "languagePacks")
        .await
        .unwrap()
        .revision_ids;
    assert_eq!(heads.len(), 2);
    a.resolve_project_data(&pid, "languagePacks", same, heads)
        .await
        .unwrap();
    assert_eq!(versions(&a, &pid).await.len(), 4);
}
#[tokio::test]
async fn archive_import_rejects_internal_old_version_collision_and_preserves_target() {
    let (_a, a) = store().await;
    let (_b, b) = store().await;
    let (pid, _) = novel(&a).await;
    let first = a
        .save_project_data(&pid, "languagePacks", payload("1", "original"), None)
        .await
        .unwrap();
    let second = a
        .save_project_data(
            &pid,
            "languagePacks",
            payload("2", "next"),
            Some(first.revision_id),
        )
        .await
        .unwrap();
    let third = a
        .save_project_data(
            &pid,
            "languagePacks",
            payload("3", "latest"),
            Some(second.revision_id),
        )
        .await
        .unwrap();
    let encoded = a.export_archive(&pid).await.unwrap();
    let imported = b.import_archive(&encoded).await.unwrap();
    assert_eq!(versions(&b, &imported.project.id).await.len(), 3);
    assert_eq!(
        b.load_project_data(&imported.project.id, "languagePacks")
            .await
            .unwrap()
            .unwrap()
            .payload,
        third.payload
    );
    let before = b.sync_cursor().await.unwrap();
    let projects = b.list_projects().await.unwrap();
    let bad = mutate_archive(
        &encoded,
        &third.revision_id,
        payload("1", "altered old version"),
    );
    assert!(b
        .import_archive(&bad)
        .await
        .unwrap_err()
        .contains("immutable"));
    assert_eq!(b.sync_cursor().await.unwrap(), before);
    assert_eq!(b.list_projects().await.unwrap(), projects);
    assert_eq!(versions(&b, &imported.project.id).await.len(), 3);
}
async fn forged_update(a: &Storage, pid: &str) -> (String, ProjectBundle, Vec<Revision>) {
    let epoch = a.sync_cursor().await.unwrap().epoch;
    let mut revisions = a
        .pull_revisions(
            pid,
            SyncCursor {
                epoch: epoch.clone(),
                sequence: 0,
            },
            1000,
        )
        .await
        .unwrap()
        .revisions;
    let last = revisions
        .iter_mut()
        .rev()
        .find(|r| r.operation == "languagePacks")
        .unwrap();
    last.payload = payload("1", "changed old version");
    last.checksum = hash(&serde_json::to_vec(&last.payload).unwrap());
    (epoch, a.sync_manifest(pid).await.unwrap(), revisions)
}
#[tokio::test]
async fn sync_refuses_collisions_against_local_history_and_within_new_batch_atomically() {
    let (_a, a) = store().await;
    let (_b, b) = store().await;
    let (_c, c) = store().await;
    let (pid, _) = novel(&a).await;
    let first = a
        .save_project_data(&pid, "languagePacks", payload("1", "original"), None)
        .await
        .unwrap();
    let second = a
        .save_project_data(
            &pid,
            "languagePacks",
            payload("2", "next"),
            Some(first.revision_id),
        )
        .await
        .unwrap();
    transfer(&a, &b, &pid, 0).await;
    let after = a.sync_cursor().await.unwrap().sequence;
    a.create_chapter(&pid, "Must roll back in receiver")
        .await
        .unwrap();
    a.save_project_data(
        &pid,
        "settings",
        json!({"schemaVersion":1,"caption":"benign earlier batch entry"}),
        None,
    )
    .await
    .unwrap();
    a.save_project_data(
        &pid,
        "languagePacks",
        payload("3", "latest"),
        Some(second.revision_id),
    )
    .await
    .unwrap();
    let (epoch, manifest, all) = forged_update(&a, &pid).await;
    for (receiver, only_new) in [(&b, true), (&c, false)] {
        let before = receiver.sync_cursor().await.unwrap();
        let projects = receiver.list_projects().await.unwrap();
        let receipts = receiver.sync_receipt_count(&pid, &epoch).await.unwrap();
        let history = versions(receiver, &pid).await;
        let batch = all
            .iter()
            .filter(|r| !only_new || r.sequence > after)
            .cloned()
            .collect();
        assert!(receiver
            .ingest_revisions(&pid, &before.epoch, &epoch, manifest.clone(), batch)
            .await
            .unwrap_err()
            .contains("immutable"));
        assert_eq!(receiver.sync_cursor().await.unwrap(), before);
        assert_eq!(receiver.list_projects().await.unwrap(), projects);
        assert_eq!(
            receiver.sync_receipt_count(&pid, &epoch).await.unwrap(),
            receipts
        );
        assert_eq!(versions(receiver, &pid).await, history);
        assert!(receiver
            .load_project_data(&pid, "settings")
            .await
            .unwrap()
            .is_none());
    }
    assert_eq!(b.open_project(&pid).await.unwrap().chapters.len(), 1);
}
#[tokio::test]
async fn corrupt_non_head_history_checksum_refuses_new_version_without_mutation() {
    for corrupt_rowid in [-1i64, 1000] {
        let (d, s) = store().await;
        let (pid, _) = novel(&s).await;
        let first = s
            .save_project_data(&pid, "languagePacks", payload("1", "original"), None)
            .await
            .unwrap();
        let second = s
            .save_project_data(
                &pid,
                "languagePacks",
                payload("2", "next"),
                Some(first.revision_id.clone()),
            )
            .await
            .unwrap();
        // Insert a synthetic corrupt retained envelope with a negative SQLite rowid;
        // every retained record must be checked, and no original record is edited.
        let mut raw = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new().filename(d.path().join("test.sqlite")),
        )
        .await
        .unwrap();
        sqlx::query("INSERT INTO revision_envelope(rowid,id,project_id,entity_id,entity_type,parents,payload,operation,schema_version,checksum,created_at,origin_epoch,origin_device) SELECT ?,'synthetic-corrupt-pack',project_id,entity_id,entity_type,parents,payload,operation,schema_version,?,created_at,origin_epoch,origin_device FROM revision_envelope WHERE id=?").bind(corrupt_rowid).bind("0".repeat(64)).bind(first.revision_id).execute(&mut raw).await.unwrap();
        sqlx::query("INSERT INTO change_marker(revision_id) VALUES('synthetic-corrupt-pack')")
            .execute(&mut raw)
            .await
            .unwrap();
        let before = s.sync_cursor().await.unwrap();
        assert!(s
            .save_project_data_checked(
                &pid,
                "languagePacks",
                payload("3", "fresh"),
                Some(second.revision_id.clone()),
                &[]
            )
            .await
            .unwrap_err()
            .contains("checksum"));
        assert_eq!(s.sync_cursor().await.unwrap(), before);
        assert_eq!(
            s.load_project_data(&pid, "languagePacks").await.unwrap(),
            Some(second)
        );
    }
}
#[tokio::test]
async fn unchanged_legacy_unversioned_packs_roundtrip_archive_and_sync() {
    let (_a, a) = store().await;
    let (_b, b) = store().await;
    let (_c, c) = store().await;
    let (pid, _) = novel(&a).await;
    let mut legacy = payload("1", "مِ legacy 中文");
    let pack = &mut legacy["packs"][0];
    pack.as_object_mut().unwrap().remove("version");
    *pack = seal(pack.clone());
    let first = a
        .save_project_data(&pid, "languagePacks", legacy.clone(), None)
        .await
        .unwrap();
    a.save_project_data_checked(
        &pid,
        "languagePacks",
        legacy.clone(),
        Some(first.revision_id),
        &[],
    )
    .await
    .unwrap();
    transfer(&a, &b, &pid, 0).await;
    assert_eq!(
        b.load_project_data(&pid, "languagePacks")
            .await
            .unwrap()
            .unwrap()
            .payload,
        legacy
    );
    let imported = c
        .import_archive(&a.export_archive(&pid).await.unwrap())
        .await
        .unwrap();
    assert_eq!(
        c.load_project_data(&imported.project.id, "languagePacks")
            .await
            .unwrap()
            .unwrap()
            .payload,
        legacy
    );
    assert_eq!(versions(&c, &imported.project.id).await.len(), 2);
}
