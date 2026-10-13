use novel_storage::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn definition(version: u64) -> Value {
    json!({"schemaVersion":1,"id":"symbols","mappingVersion":version,"name":"Synthetic مِ 中文","scriptId":"synthetic","encoding":"private-use","direction":"rtl","mappings":[{"input":"a","output":"\u{e000}","transliteration":if version==1 {"old"} else {"new"}}],"font":null,"glossary":[],"notes":"e\u{301}"})
}
fn doc(sid: &str, version: u64) -> Value {
    json!({"schemaVersion":1,"systems":(1..=version).map(definition).collect::<Vec<_>>(),"assignments":[{"scene":{"kind":"scene","id":sid},"definitionId":"symbols","mappingVersion":1}]})
}
async fn fixture() -> (tempfile::TempDir, Storage, Project, SceneRecord) {
    let d = tempfile::tempdir().unwrap();
    let s = Storage::open(&d.path().join("alphabet.sqlite")).await.unwrap();
    let p = s.create_project("Exact alphabet export مِ 中文").await.unwrap();
    let c = s.create_chapter(&p.id, "Chapter").await.unwrap();
    let r = s.create_scene(&c.id, Scene { schema_version:1, title:"Original title".into(), document:json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar","lineSpacing":1.5},"content":[{"type":"text","text":"\u{e000} مِ e\u{301} 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"},{"type":"underline"}]},{"type":"hardBreak"}]}]}) }).await.unwrap();
    (d,s,p,r)
}
async fn exchange(from:&Storage,to:&Storage,pid:&str) {
    let remote=from.sync_cursor().await.unwrap(); let local=to.sync_cursor().await.unwrap();
    let page=from.pull_revisions(pid,SyncCursor{epoch:remote.epoch.clone(),sequence:0},100).await.unwrap();
    to.ingest_revisions(pid,&local.epoch,&remote.epoch,from.sync_manifest(pid).await.unwrap(),page.revisions).await.unwrap();
}
#[tokio::test]
async fn snapshot_pins_historical_assignment_and_exact_rich_saved_versions_without_writes() {
    let (_d,s,p,r)=fixture().await;
    let first=s.save_project_data(&p.id,"writingSystems",doc(&r.id,1),None).await.unwrap();
    let second=s.save_project_data(&p.id,"writingSystems",doc(&r.id,2),Some(first.revision_id)).await.unwrap();
    let marker=s.sync_cursor().await.unwrap();
    let snapshot=s.alphabet_export_snapshot(&p.id).await.unwrap();
    assert_eq!(snapshot.bundle,s.open_project(&p.id).await.unwrap());
    assert_eq!(snapshot.writing_systems,Some(second));
    assert_eq!(snapshot.bundle.scenes[0].scene,r.scene);
    assert_eq!(s.sync_cursor().await.unwrap(),marker);
    let mut changed=r.scene.clone();changed.document["content"][0]["content"][0]["text"]=json!("Changed after capture");
    s.save_scene(&r.id,changed,Some(r.revision_id.clone())).await.unwrap();
    assert_eq!(snapshot.bundle.scenes[0].revision_id,r.revision_id);
    assert_eq!(snapshot.bundle.scenes[0].scene,r.scene);
}
#[tokio::test]
async fn scene_and_alphabet_conflicts_refuse_without_markers_or_projection_changes() {
    let (_d,s,p,r)=fixture().await;
    s.save_scene(&r.id,r.scene.clone(),Some(r.revision_id.clone())).await.unwrap();
    s.preserve_conflict(&r.id,r.scene.clone(),&r.revision_id).await.unwrap();
    let marker=s.sync_cursor().await.unwrap();
    assert!(s.alphabet_export_snapshot(&p.id).await.unwrap_err().contains("Conflict"));
    assert_eq!(s.sync_cursor().await.unwrap(),marker);
    let (_d,a,p,r)=fixture().await;let d=tempfile::tempdir().unwrap();let b=Storage::open(&d.path().join("peer.sqlite")).await.unwrap();
    exchange(&a,&b,&p.id).await;
    a.save_project_data(&p.id,"writingSystems",doc(&r.id,1),None).await.unwrap();
    let mut other=doc(&r.id,1);other["systems"][0]["id"]=json!("peer");other["assignments"][0]["definitionId"]=json!("peer");
    b.save_project_data(&p.id,"writingSystems",other,None).await.unwrap();exchange(&b,&a,&p.id).await;
    let marker=a.sync_cursor().await.unwrap();
    assert!(a.alphabet_export_snapshot(&p.id).await.unwrap_err().contains("competing writingSystems"));
    assert_eq!(a.sync_cursor().await.unwrap(),marker);
}
async fn corrupt(d:&tempfile::TempDir,id:&str,payload:Value) {
    let pool=sqlx::SqlitePool::connect(&format!("sqlite://{}",d.path().join("alphabet.sqlite").display())).await.unwrap();
    sqlx::query("DROP TRIGGER IF EXISTS immutable_revision_update").execute(&pool).await.unwrap();
    let raw=serde_json::to_string(&payload).unwrap();let checksum=format!("{:x}",Sha256::digest(raw.as_bytes()));
    sqlx::query("UPDATE revision_envelope SET payload=?,checksum=? WHERE id=?").bind(raw).bind(checksum).bind(id).execute(&pool).await.unwrap();pool.close().await;
}
#[tokio::test]
async fn historically_reinterpreted_version_and_foreign_assignment_refuse_even_with_valid_checksums() {
    let (d,s,p,r)=fixture().await;
    let first=s.save_project_data(&p.id,"writingSystems",doc(&r.id,1),None).await.unwrap();
    let second=s.save_project_data(&p.id,"writingSystems",doc(&r.id,2),Some(first.revision_id.clone())).await.unwrap();
    let mut altered=first.payload.clone();altered["systems"][0]["mappings"][0]["transliteration"]=json!("Reinterpreted");
    corrupt(&d,&first.revision_id,altered).await;
    let marker=s.sync_cursor().await.unwrap();
    assert!(s.alphabet_export_snapshot(&p.id).await.unwrap_err().contains("immutable"));
    assert_eq!(s.sync_cursor().await.unwrap(),marker);
    corrupt(&d,&first.revision_id,first.payload).await;
    let foreign=s.create_project("Foreign").await.unwrap();let c=s.create_chapter(&foreign.id,"Foreign").await.unwrap();let f=s.create_scene(&c.id,r.scene.clone()).await.unwrap();
    let mut altered=second.payload.clone();altered["assignments"][0]["scene"]["id"]=json!(f.id);
    corrupt(&d,&second.revision_id,altered).await;
    let marker=s.sync_cursor().await.unwrap();
    assert!(s.alphabet_export_snapshot(&p.id).await.unwrap_err().contains("foreign scene"));
    assert_eq!(s.sync_cursor().await.unwrap(),marker);
}
#[tokio::test]
async fn scene_checksum_failure_refuses_and_deleted_scene_is_not_exported() {
    let (d,s,p,r)=fixture().await;
    s.delete_scene(&r.id,&r.revision_id).await.unwrap();
    assert!(s.alphabet_export_snapshot(&p.id).await.unwrap().bundle.scenes.is_empty());
    let c=s.create_chapter(&p.id,"New").await.unwrap();let new=s.create_scene(&c.id,r.scene.clone()).await.unwrap();
    let pool=sqlx::SqlitePool::connect(&format!("sqlite://{}",d.path().join("alphabet.sqlite").display())).await.unwrap();
    sqlx::query("DROP TRIGGER immutable_revision_update").execute(&pool).await.unwrap();
    sqlx::query("UPDATE revision_envelope SET checksum=? WHERE id=?").bind("0".repeat(64)).bind(&new.revision_id).execute(&pool).await.unwrap();pool.close().await;
    let marker=s.sync_cursor().await.unwrap();
    assert!(s.alphabet_export_snapshot(&p.id).await.unwrap_err().contains("checksum"));
    assert_eq!(s.sync_cursor().await.unwrap(),marker);
}
#[tokio::test]
async fn hidden_competing_deletions_and_translations_refuse_read_only_export() {
    for deletion in [false,true] {
        let (_d,a,p,r)=fixture().await;let d=tempfile::tempdir().unwrap();let b=Storage::open(&d.path().join("peer.sqlite")).await.unwrap();exchange(&a,&b,&p.id).await;
        if deletion {
            a.delete_scene(&r.id,&r.revision_id).await.unwrap();b.delete_scene(&r.id,&r.revision_id).await.unwrap();
        } else {
            for store in [&a,&b] {store.save_project_data(&p.id,"sceneTranslations",json!({"schemaVersion":1,"drafts":[]}),None).await.unwrap();}
        }
        exchange(&b,&a,&p.id).await;let marker=a.sync_cursor().await.unwrap();
        assert!(a.alphabet_export_snapshot(&p.id).await.is_err());assert_eq!(a.sync_cursor().await.unwrap(),marker);
    }
}
#[tokio::test]
async fn deferred_retained_chapter_order_refuses_until_explicit_replay_without_text_changes() {
    let (_d,a,p,moved)=fixture().await;let d=tempfile::tempdir().unwrap();let b=Storage::open(&d.path().join("peer.sqlite")).await.unwrap();
    let destination=a.create_chapter(&p.id,"B").await.unwrap();let alternative=a.create_chapter(&p.id,"C").await.unwrap();
    let first=a.create_scene(&alternative.id,moved.scene.clone()).await.unwrap();let second=a.create_scene(&alternative.id,moved.scene.clone()).await.unwrap();
    exchange(&a,&b,&p.id).await;
    let capture=a.scene_arrangement_snapshot(&p.id,&moved.id).await.unwrap();a.arrange_scene(&capture,&destination.id,0).await.unwrap();
    let capture=b.scene_arrangement_snapshot(&p.id,&moved.id).await.unwrap();b.arrange_scene(&capture,&alternative.id,0).await.unwrap();
    b.reorder_scenes(&alternative.id,vec![second.id.clone(),first.id.clone(),moved.id.clone()]).await.unwrap();exchange(&b,&a,&p.id).await;
    let capture=a.metadata_resolution_snapshot(&p.id,&moved.id,"scenePlacement").await.unwrap();
    a.resolve_metadata_checked(&capture,json!({"chapterId":alternative.id,"position":0})).await.unwrap();
    for cid in [&moved.chapter_id,&destination.id] {
        let capture=a.metadata_resolution_snapshot(&p.id,cid,"chapter").await.unwrap();
        a.resolve_metadata_checked(&capture,capture.projection.clone()).await.unwrap();
    }
    let capture=a.metadata_resolution_snapshot(&p.id,&alternative.id,"chapter").await.unwrap();
    let retained=a.load_entity_revision(&p.id,&alternative.id,"chapter",&capture.heads[0]).await.unwrap().payload;
    assert_ne!(retained["order"],capture.projection["order"]);
    let marker=a.sync_cursor().await.unwrap();assert!(a.alphabet_export_snapshot(&p.id).await.unwrap_err().contains("Scene and chapter conflicts"));assert_eq!(a.sync_cursor().await.unwrap(),marker);
    a.resolve_metadata_checked(&capture,retained).await.unwrap();
    let snapshot=a.alphabet_export_snapshot(&p.id).await.unwrap();
    let order:Vec<_>=snapshot.bundle.scenes.iter().filter(|s|s.chapter_id==alternative.id).map(|s|s.id.clone()).collect();
    assert_eq!(order,vec![second.id,first.id,moved.id.clone()]);
    assert_eq!(a.load_scene(&moved.id).await.unwrap().revision_id,moved.revision_id);
    assert_eq!(a.load_scene(&moved.id).await.unwrap().scene,moved.scene);
}
