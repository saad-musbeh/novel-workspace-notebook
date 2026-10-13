use novel_storage::{Scene,Storage,SyncCursor};
use serde_json::json;
#[tokio::test]
async fn checked_first_save_refuses_unprojected_concurrent_roots(){
    let directory=tempfile::tempdir().unwrap();let left=Storage::open(&directory.path().join("left.sqlite")).await.unwrap();let right=Storage::open(&directory.path().join("right.sqlite")).await.unwrap();let receiver=Storage::open(&directory.path().join("receiver.sqlite")).await.unwrap();
    let project=left.create_project("Concurrent metadata").await.unwrap();let chapter=left.create_chapter(&project.id,"First").await.unwrap();left.create_scene(&chapter.id,Scene{schema_version:1,title:"مِ 中文".into(),document:json!({"type":"doc","content":[{"type":"paragraph"}]})}).await.unwrap();
    let left_epoch=left.sync_cursor().await.unwrap().epoch;let right_epoch=right.sync_cursor().await.unwrap().epoch;let receiver_epoch=receiver.sync_cursor().await.unwrap().epoch;
    let initial=left.pull_revisions(&project.id,SyncCursor{epoch:left_epoch.clone(),sequence:0},100).await.unwrap();right.ingest_revisions(&project.id,&right_epoch,&left_epoch,left.sync_manifest(&project.id).await.unwrap(),initial.revisions).await.unwrap();
    let payload=json!({"schemaVersion":1,"scenes":[]});let l=left.save_project_data(&project.id,"sceneMetadata",payload.clone(),None).await.unwrap();let r=right.save_project_data(&project.id,"sceneMetadata",payload.clone(),None).await.unwrap();
    let mut page=left.pull_revisions(&project.id,SyncCursor{epoch:left_epoch.clone(),sequence:0},100).await.unwrap().revisions;
    let right_page=right.pull_revisions(&project.id,SyncCursor{epoch:right_epoch.clone(),sequence:0},100).await.unwrap();page.extend(right_page.revisions.into_iter().filter(|rev|rev.id==r.revision_id));
    receiver.ingest_revisions(&project.id,&receiver_epoch,&left_epoch,left.sync_manifest(&project.id).await.unwrap(),page).await.unwrap();
    assert!(receiver.load_project_data(&project.id,"sceneMetadata").await.unwrap().is_none());let heads=receiver.project_data_heads(&project.id,"sceneMetadata").await.unwrap().revision_ids;assert_eq!(heads.len(),2);assert!(heads.contains(&l.revision_id));assert!(heads.contains(&r.revision_id));
    let before=receiver.sync_cursor().await.unwrap();assert!(receiver.save_project_data_checked(&project.id,"sceneMetadata",payload.clone(),None,&[]).await.unwrap_err().starts_with("Conflict"));assert_eq!(receiver.sync_cursor().await.unwrap(),before);assert_eq!(receiver.project_data_heads(&project.id,"sceneMetadata").await.unwrap().revision_ids,heads);
    receiver.resolve_project_data(&project.id,"sceneMetadata",payload,heads).await.unwrap();assert!(receiver.load_project_data(&project.id,"sceneMetadata").await.unwrap().is_some());
}
