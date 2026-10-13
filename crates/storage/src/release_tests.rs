//! Capacity and killed-process checks operate only on synthetic temporary stores.
use super::*;
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions,SqlitePoolOptions};
use std::{path::Path,process::{Command,Stdio},time::{Duration,Instant}};
fn scene(text:&str)->Scene{Scene{schema_version:1,title:"Recovery fixture".into(),document:json!({"type":"doc","content":[{"type":"paragraph","attrs":{"lang":"ar","dir":"rtl"},"content":[{"type":"text","text":text,"marks":[{"type":"bold"}]}]}]})}}
async fn assert_integrity(pool:&SqlitePool){
    assert_eq!(sqlx::query_scalar::<_,String>("PRAGMA integrity_check").fetch_one(pool).await.unwrap(),"ok");
    assert!(sqlx::query("PRAGMA foreign_key_check").fetch_all(pool).await.unwrap().is_empty());
}
#[tokio::test]
async fn sqlite_capacity_failure_never_acknowledges_scene_or_partial_archive_import(){
    let d=tempfile::tempdir().unwrap();let path=d.path().join("capacity.sqlite");let s=Storage::open(&path).await.unwrap();
    let original=s.save(scene("Acknowledged مَ 世界 😀"),None).await.unwrap();
    let archive=s.export_archive("prototype-project").await.unwrap();
    let pages:i64=sqlx::query_scalar("PRAGMA page_count").fetch_one(&s.pool).await.unwrap();
    sqlx::query(&format!("PRAGMA max_page_count={pages}")).execute(&s.pool).await.unwrap();
    let mut large=scene(&"大".repeat(100_000));large.title="This write must fail".into();
    let error=s.save(large,Some(original.revision_id.clone())).await.unwrap_err();assert!(error.contains("full"),"Expected actual SQLITE_FULL, got {error}");
    assert_eq!(s.load().await.unwrap(),Some(original.clone()));
    assert_eq!(s.history("prototype-scene").await.unwrap().len(),1);
    // A bigger source archive forces growth instead of relying on incidental free pages.
    let source=Storage::open(&d.path().join("source.sqlite")).await.unwrap();source.save(scene(&"中".repeat(100_000)),None).await.unwrap();
    let large_archive=source.export_archive("prototype-project").await.unwrap();
    assert!(s.import_archive(&large_archive).await.unwrap_err().contains("full"));
    assert_eq!(s.list_projects().await.unwrap().len(),1);assert_eq!(s.changes(0,100).await.unwrap().len(),1);assert_eq!(s.export_archive("prototype-project").await.unwrap(),archive);
    assert_integrity(&s.pool).await;s.close().await;
    let reopened=Storage::open(&path).await.unwrap();assert_eq!(reopened.load().await.unwrap(),Some(original));assert_integrity(&reopened.pool).await;
}
#[tokio::test]
async fn snapshot_is_private_no_overwrite_and_failed_destination_keeps_source_usable(){
    let d=tempfile::tempdir().unwrap();let s=Storage::open(&d.path().join("source.sqlite")).await.unwrap();let original=s.save(scene("Safe مَ 世界"),None).await.unwrap();
    let destination=d.path().join("backup.sqlite");s.backup(&destination).await.unwrap();
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;assert_eq!(std::fs::metadata(&destination).unwrap().permissions().mode()&0o777,0o600);}
    let bytes=std::fs::read(&destination).unwrap();assert!(s.backup(&destination).await.is_err());assert_eq!(std::fs::read(&destination).unwrap(),bytes);
    assert!(s.backup(&d.path().join("missing/backup.sqlite")).await.is_err());
    assert_eq!(s.load().await.unwrap(),Some(original));
    assert!(!std::fs::read_dir(d.path()).unwrap().any(|entry|entry.unwrap().file_name().to_string_lossy().starts_with(".novel-snapshot-")));
    let restored=Storage::open(&destination).await.unwrap();assert_integrity(&restored.pool).await;assert_eq!(restored.load().await.unwrap(),s.load().await.unwrap());
}
#[test]
fn recovery_fault_worker(){
    let Some(path)=std::env::var_os("NOVEL_STORAGE_FAULT_DATABASE")else{return};
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async{
        let s=Storage::open(Path::new(&path)).await.unwrap();
        match std::env::var("NOVEL_STORAGE_FAULT_STAGE").unwrap().as_str(){
            "snapshot"=>s.backup(&Path::new(&path).with_file_name("published.sqlite")).await.unwrap(),
            "archive-export"=>{s.export_archive("prototype-project").await.unwrap();},
            "archive-import"=>{let archive=std::fs::read_to_string(Path::new(&path).with_file_name("fixture-archive.json")).unwrap();s.import_archive(&archive).await.unwrap();},
            "migration-v2"|"migration-v3"=>{},
            _=>panic!("Unknown synthetic fault fixture")
        }
    });
    panic!("Expected fixture to pause before acknowledgement");
}
fn kill_at_checkpoint(path:&Path,kind:&str){
    let ready=path.with_file_name("fault-ready");let _=std::fs::remove_file(&ready);
    let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","release_tests::recovery_fault_worker","--nocapture"])
        .env("NOVEL_STORAGE_FAULT_DATABASE",path).env("NOVEL_STORAGE_FAULT_STAGE",kind).env("NOVEL_STORAGE_FAULT_READY",&ready)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let deadline=Instant::now()+Duration::from_secs(10);
    while !ready.exists(){
        if let Some(status)=child.try_wait().unwrap(){let mut message=String::new();use std::io::Read;child.stderr.take().unwrap().read_to_string(&mut message).unwrap();panic!("Fixture exited before {kind}: {status}, {message}")}
        if Instant::now()>deadline{let _=child.kill();let _=child.wait();panic!("Fixture did not reach {kind}")}
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill().unwrap();child.wait().unwrap();assert_eq!(std::fs::read_to_string(ready).unwrap(),kind);
}
#[tokio::test]
async fn killed_snapshot_export_and_import_never_publish_or_commit_partial_recovery(){
    for stage in ["snapshot","archive-export","archive-import"]{
        let d=tempfile::tempdir().unwrap();let path=d.path().join("fixture.sqlite");let s=Storage::open(&path).await.unwrap();let original=s.save(scene("Last acknowledged مَ 世界 😀"),None).await.unwrap();
        let archive=s.export_archive("prototype-project").await.unwrap();std::fs::write(d.path().join("fixture-archive.json"),&archive).unwrap();s.close().await;
        kill_at_checkpoint(&path,stage);
        assert!(!d.path().join("published.sqlite").exists());
        for entry in std::fs::read_dir(d.path()).unwrap(){let entry=entry.unwrap();if entry.file_name().to_string_lossy().starts_with(".novel-snapshot-"){
            #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;assert_eq!(entry.metadata().unwrap().permissions().mode()&0o777,0o700);}
        }}
        let recovered=Storage::open(&path).await.unwrap();assert_eq!(recovered.load().await.unwrap(),Some(original));assert_eq!(recovered.list_projects().await.unwrap().len(),1);assert_eq!(recovered.changes(0,100).await.unwrap().len(),1);assert_eq!(recovered.export_archive("prototype-project").await.unwrap(),archive);assert_integrity(&recovered.pool).await;
        recovered.save(scene("Next committed edit"),recovered.load().await.unwrap().map(|s|s.revision_id)).await.unwrap();
    }
}
#[tokio::test]
async fn killed_schema_upgrade_rolls_back_ddl_and_retains_private_pre_upgrade_snapshot(){
    for (version,stage) in [(1,"migration-v2"),(2,"migration-v3")]{
        let d=tempfile::tempdir().unwrap();let path=d.path().join("upgrade.sqlite");
        let original=if version==1{
            let pool=SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(&path).create_if_missing(true)).await.unwrap();sqlx::raw_sql(include_str!("schema.sql")).execute(&pool).await.unwrap();let scene=scene("Legacy acknowledged مَ 世界");sqlx::query("INSERT INTO scene_revision VALUES('original',NULL,?)").bind(serde_json::to_string(&scene).unwrap()).execute(&pool).await.unwrap();sqlx::query("INSERT INTO scene_head VALUES(1,'original')").execute(&pool).await.unwrap();pool.close().await;Snapshot{revision_id:"original".into(),scene}
        }else{
            let s=Storage::open(&path).await.unwrap();let original=s.save(scene("Schema2 acknowledged مَ 世界"),None).await.unwrap();sqlx::raw_sql("DROP TABLE completed_asset_transfer; DROP TABLE sync_receipt; DROP TABLE asset_transfer; ALTER TABLE manuscript_scene DROP COLUMN deleted; PRAGMA user_version=2;").execute(&s.pool).await.unwrap();s.close().await;original
        };
        kill_at_checkpoint(&path,stage);
        let pool=SqlitePoolOptions::new().connect_with(SqliteConnectOptions::new().filename(&path)).await.unwrap();assert_eq!(sqlx::query_scalar::<_,i64>("PRAGMA user_version").fetch_one(&pool).await.unwrap(),version);
        if version==1{assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM sqlite_master WHERE name='project'").fetch_one(&pool).await.unwrap(),0);}else{assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM pragma_table_info('manuscript_scene') WHERE name='deleted'").fetch_one(&pool).await.unwrap(),0);}
        assert_integrity(&pool).await;pool.close().await;
        let backup=std::fs::read_dir(d.path()).unwrap().map(|entry|entry.unwrap().path()).find(|p|p.file_name().unwrap().to_string_lossy().contains(".pre-v")).unwrap();
        #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;assert_eq!(std::fs::metadata(&backup).unwrap().permissions().mode()&0o777,0o600);}
        let snapshot=SqlitePoolOptions::new().connect_with(SqliteConnectOptions::new().filename(&backup).read_only(true)).await.unwrap();assert_eq!(sqlx::query_scalar::<_,i64>("PRAGMA user_version").fetch_one(&snapshot).await.unwrap(),version);assert_integrity(&snapshot).await;snapshot.close().await;
        let resumed=Storage::open(&path).await.unwrap();assert_eq!(resumed.load().await.unwrap(),Some(original));assert_integrity(&resumed.pool).await;
    }
}

#[tokio::test]
async fn archive_record_preflight_rejects_large_history_without_mutating_it(){
    let d=tempfile::tempdir().unwrap();let s=Storage::open(&d.path().join("large-history.sqlite")).await.unwrap();let original=s.save(scene("Retained acknowledged text مَ 世界"),None).await.unwrap();
    // Build a synthetic immutable graph directly, avoiding 20,000 RPC round trips.
    // Every envelope has valid original payload/hash/provenance and a durable marker.
    let mut tx=s.pool.begin().await.unwrap();
    sqlx::query("WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<20001) INSERT INTO revision_envelope(id,project_id,entity_id,entity_type,parents,payload,operation,schema_version,checksum,origin_epoch,origin_device) SELECT 'synthetic-revision-'||n.x,r.project_id,r.entity_id,r.entity_type,'[]',r.payload,'synthetic',r.schema_version,r.checksum,r.origin_epoch,r.origin_device FROM n,revision_envelope r WHERE r.id=?").bind(&original.revision_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO change_marker(revision_id) SELECT id FROM revision_envelope WHERE operation='synthetic'").execute(&mut *tx).await.unwrap();tx.commit().await.unwrap();
    let error=s.export_archive("prototype-project").await.unwrap_err();assert!(error.starts_with("Too many archive records"),"{error}");
    assert_eq!(s.load().await.unwrap(),Some(original));assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM revision_envelope").fetch_one(&s.pool).await.unwrap(),20002);
    let backup=d.path().join("large-history-backup.sqlite");s.backup(&backup).await.unwrap();
    let restored=Storage::open(&backup).await.unwrap();assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM revision_envelope").fetch_one(&restored.pool).await.unwrap(),20002);assert_integrity(&restored.pool).await;
}
