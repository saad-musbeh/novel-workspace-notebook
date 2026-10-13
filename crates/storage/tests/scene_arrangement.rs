use novel_storage::{Scene, Storage, SyncCursor};
use serde_json::json;
use sqlx::Connection;

fn scene(title:&str)->Scene{Scene{schema_version:1,title:title.into(),document:json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar","lineSpacing":1.5},"content":[{"type":"text","text":"مِ 中文 𠀀 👩🏽‍🚀 e\u{301}","marks":[{"type":"bold"},{"type":"underline"}]}]}]})}}
async fn fixture()->(tempfile::TempDir,Storage){let d=tempfile::tempdir().unwrap();let store=Storage::open(&d.path().join("arrange.sqlite")).await.unwrap();(d,store)}
#[tokio::test]
async fn checked_reorder_and_cross_chapter_move_preserve_content_and_saved_heads(){
    let (d,s)=fixture().await;let p=s.create_project("Arrange مِ 中文").await.unwrap();let a=s.create_chapter(&p.id,"A").await.unwrap();let b=s.create_chapter(&p.id,"B").await.unwrap();
    let first=s.create_scene(&a.id,scene("First مِ 中文")).await.unwrap();let second=s.create_scene(&a.id,scene("Second")).await.unwrap();let third=s.create_scene(&b.id,scene("Third")).await.unwrap();
    let marker=s.sync_cursor().await.unwrap();let capture=s.scene_arrangement_snapshot(&p.id,&second.id).await.unwrap();assert_eq!(s.sync_cursor().await.unwrap(),marker);
    assert_eq!(capture.title,"Second");let index=s.arrange_scene(&capture,&a.id,0).await.unwrap();assert_eq!(index.scenes[0].id,second.id);assert_eq!(index.scenes[1].id,first.id);
    let capture=s.scene_arrangement_snapshot(&p.id,&second.id).await.unwrap();let index=s.arrange_scene(&capture,&b.id,1).await.unwrap();assert_eq!(index.scenes.iter().map(|s|s.id.as_str()).collect::<Vec<_>>(),vec![first.id.as_str(),third.id.as_str(),second.id.as_str()]);
    for original in [&first,&second,&third]{let saved=s.load_scene(&original.id).await.unwrap();assert_eq!(saved.revision_id,original.revision_id);assert_eq!(saved.scene,original.scene);}
    let history=s.changes(marker.sequence,100).await.unwrap();assert!(history.iter().any(|r|r.entity_type=="scenePlacement"&&r.entity_id==second.id&&r.operation=="arrangeScene"));assert!(history.iter().any(|r|r.entity_type=="chapter"&&r.entity_id==a.id&&r.payload["order"]==json!([first.id])));assert!(history.iter().any(|r|r.entity_type=="chapter"&&r.entity_id==b.id&&r.payload["order"]==json!([third.id,second.id])));
    s.close().await;let reopened=Storage::open(&d.path().join("arrange.sqlite")).await.unwrap();assert_eq!(reopened.open_project_index(&p.id).await.unwrap(),index);
}
#[tokio::test]
async fn stale_content_reorder_placement_membership_and_chapter_metadata_are_rejected(){
    let (_d,s)=fixture().await;let p=s.create_project("Arrange").await.unwrap();let a=s.create_chapter(&p.id,"A").await.unwrap();let b=s.create_chapter(&p.id,"B").await.unwrap();let first=s.create_scene(&a.id,scene("First")).await.unwrap();let second=s.create_scene(&a.id,scene("Second")).await.unwrap();
    let stale=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();s.rename_scene(&first.id,"Newer مِ 中文",Some(first.revision_id)).await.unwrap();let marker=s.sync_cursor().await.unwrap();assert!(s.arrange_scene(&stale,&b.id,0).await.unwrap_err().starts_with("Conflict"));assert_eq!(s.sync_cursor().await.unwrap(),marker);
    let stale=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();s.reorder_scenes(&a.id,vec![second.id.clone(),first.id.clone()]).await.unwrap();let before=s.open_project_index(&p.id).await.unwrap();assert!(s.arrange_scene(&stale,&b.id,0).await.unwrap_err().starts_with("Conflict"));assert_eq!(s.open_project_index(&p.id).await.unwrap(),before);
    let stale=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();s.move_scene(&first.id,&b.id,0).await.unwrap();assert!(s.arrange_scene(&stale,&a.id,0).await.unwrap_err().starts_with("Conflict"));
    let stale=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();s.create_scene(&b.id,scene("New scene")).await.unwrap();assert!(s.arrange_scene(&stale,&a.id,0).await.unwrap_err().starts_with("Conflict"));
    let stale=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();s.rename_chapter(&a.id,"Renamed target").await.unwrap();let marker=s.sync_cursor().await.unwrap();assert!(s.arrange_scene(&stale,&a.id,0).await.unwrap_err().starts_with("Conflict"));assert_eq!(s.sync_cursor().await.unwrap(),marker);
}
#[tokio::test]
async fn arrangement_validates_scope_bounds_and_tombstones_without_committing_invalid_input(){
    let (_d,s)=fixture().await;let p=s.create_project("Arrange").await.unwrap();let a=s.create_chapter(&p.id,"A").await.unwrap();let b=s.create_chapter(&p.id,"B").await.unwrap();let first=s.create_scene(&a.id,scene("First")).await.unwrap();let hidden=s.create_scene(&a.id,scene("Deleted")).await.unwrap();let last=s.create_scene(&a.id,scene("Last")).await.unwrap();
    let other=s.create_project("Other").await.unwrap();let foreign=s.create_chapter(&other.id,"Foreign").await.unwrap();assert!(s.scene_arrangement_snapshot(&other.id,&first.id).await.is_err());
    s.delete_scene(&hidden.id,&hidden.revision_id).await.unwrap();assert!(s.scene_arrangement_snapshot(&p.id,&hidden.id).await.is_err());let capture=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();let before=s.open_project_index(&p.id).await.unwrap();let marker=s.sync_cursor().await.unwrap();
    for (target,position) in [(&foreign.id,0),(&b.id,1),(&a.id,2)]{assert!(s.arrange_scene(&capture,target,position).await.is_err());assert_eq!(s.sync_cursor().await.unwrap(),marker);assert_eq!(s.open_project_index(&p.id).await.unwrap(),before);}
    let index=s.arrange_scene(&capture,&a.id,1).await.unwrap();assert_eq!(index.scenes.iter().map(|s|s.id.as_str()).collect::<Vec<_>>(),vec![last.id.as_str(),first.id.as_str()]);assert_eq!(s.deleted_scenes(&p.id).await.unwrap()[0].id,hidden.id);
    let stale=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();s.delete_scene(&last.id,&last.revision_id).await.unwrap();assert!(s.arrange_scene(&stale,&b.id,0).await.unwrap_err().starts_with("Conflict"));
}
#[tokio::test]
async fn interrupted_arrangement_rolls_back_order_placement_and_history(){
    let (d,s)=fixture().await;let p=s.create_project("Arrange").await.unwrap();let a=s.create_chapter(&p.id,"A").await.unwrap();let b=s.create_chapter(&p.id,"B").await.unwrap();let first=s.create_scene(&a.id,scene("First")).await.unwrap();let capture=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();let before=s.open_project_index(&p.id).await.unwrap();let marker=s.sync_cursor().await.unwrap();
    let mut raw=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(d.path().join("arrange.sqlite"))).await.unwrap();sqlx::query("CREATE TRIGGER reject_arrangement BEFORE INSERT ON revision_envelope WHEN NEW.operation='arrangeScene' AND NEW.entity_type='chapter' BEGIN SELECT RAISE(ABORT,'Synthetic interruption'); END").execute(&mut raw).await.unwrap();
    assert!(s.arrange_scene(&capture,&b.id,0).await.is_err());assert_eq!(s.open_project_index(&p.id).await.unwrap(),before);assert_eq!(s.sync_cursor().await.unwrap(),marker);assert_eq!(s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap(),capture);
}
#[tokio::test]
async fn scene_arrangement_sync_replays_both_chapter_orders_and_keeps_scene_content(){
    let (_d,s)=fixture().await;let (_e,r)=fixture().await;let p=s.create_project("Arrange").await.unwrap();let a=s.create_chapter(&p.id,"A").await.unwrap();let b=s.create_chapter(&p.id,"B").await.unwrap();let first=s.create_scene(&a.id,scene("First")).await.unwrap();let second=s.create_scene(&a.id,scene("Second")).await.unwrap();let third=s.create_scene(&b.id,scene("Third")).await.unwrap();
    let epoch=s.sync_cursor().await.unwrap().epoch;let local=r.sync_cursor().await.unwrap().epoch;let initial=s.pull_revisions(&p.id,SyncCursor{epoch:epoch.clone(),sequence:0},100).await.unwrap();r.ingest_revisions(&p.id,&local,&epoch,s.sync_manifest(&p.id).await.unwrap(),initial.revisions).await.unwrap();let after=s.sync_cursor().await.unwrap().sequence;
    let capture=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();let index=s.arrange_scene(&capture,&b.id,1).await.unwrap();let changes=s.pull_revisions(&p.id,SyncCursor{epoch:epoch.clone(),sequence:after},100).await.unwrap();r.ingest_revisions(&p.id,&local,&epoch,s.sync_manifest(&p.id).await.unwrap(),changes.revisions).await.unwrap();assert_eq!(r.open_project_index(&p.id).await.unwrap(),index);
    assert_eq!(index.scenes.iter().map(|s|s.id.as_str()).collect::<Vec<_>>(),vec![second.id.as_str(),third.id.as_str(),first.id.as_str()]);assert_eq!(r.load_scene(&first.id).await.unwrap().scene,first.scene);
}
#[tokio::test]
async fn competing_synced_scene_placements_block_another_arrangement(){
    let (_d,s)=fixture().await;let (_e,r)=fixture().await;let p=s.create_project("Arrange").await.unwrap();let a=s.create_chapter(&p.id,"A").await.unwrap();let b=s.create_chapter(&p.id,"B").await.unwrap();let c=s.create_chapter(&p.id,"C").await.unwrap();let first=s.create_scene(&a.id,scene("First")).await.unwrap();
    let epoch=s.sync_cursor().await.unwrap().epoch;let local=r.sync_cursor().await.unwrap().epoch;let initial=s.pull_revisions(&p.id,SyncCursor{epoch:epoch.clone(),sequence:0},100).await.unwrap();r.ingest_revisions(&p.id,&local,&epoch,s.sync_manifest(&p.id).await.unwrap(),initial.revisions).await.unwrap();let after=s.sync_cursor().await.unwrap().sequence;
    let sc=s.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();let rc=r.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap();s.arrange_scene(&sc,&b.id,0).await.unwrap();r.arrange_scene(&rc,&c.id,0).await.unwrap();
    let changes=s.pull_revisions(&p.id,SyncCursor{epoch:epoch.clone(),sequence:after},100).await.unwrap();r.ingest_revisions(&p.id,&local,&epoch,s.sync_manifest(&p.id).await.unwrap(),changes.revisions).await.unwrap();
    let marker=r.sync_cursor().await.unwrap();assert!(r.scene_arrangement_snapshot(&p.id,&first.id).await.unwrap_err().contains("Conflict"));assert!(r.arrange_scene(&rc,&a.id,0).await.unwrap_err().contains("Conflict"));assert_eq!(r.sync_cursor().await.unwrap(),marker);assert_eq!(r.load_scene(&first.id).await.unwrap().scene,first.scene);
}
