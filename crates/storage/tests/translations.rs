use novel_storage::{Storage, Scene};
use serde_json::{json,Value};
fn scene(text:&str)->Scene{Scene{schema_version:1,title:"Scene".into(),document:json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})}}
fn draft(id:&str,sid:&str,rid:&str,source:&str,text:&str)->Value{json!({"id":id,"sceneId":sid,"sceneRevisionId":rid,"sourceLanguage":"ar","targetLanguage":"zh-Hans","sourceText":source,"text":text})}
async fn setup()->(tempfile::TempDir,Storage,String,String,String){let d=tempfile::tempdir().unwrap();let s=Storage::open(&d.path().join("t.sqlite")).await.unwrap();let p=s.create_project("Novel").await.unwrap();let c=s.create_chapter(&p.id,"Chapter").await.unwrap();let x=s.create_scene(&c.id,scene("مَرْحَبًا 世界 😀")).await.unwrap();(d,s,p.id,c.id,x.id)}
#[tokio::test] async fn exact_source_is_bound_to_immutable_revision_and_text_saves_cas(){
 let(_d,s,p,_c,sid)=setup().await;let original=s.load_scene(&sid).await.unwrap();
 let payload=json!({"schemaVersion":1,"drafts":[draft("t1",&sid,&original.revision_id,"مَرْحَبًا 世界 😀","你好 世界")]});
 let saved=s.save_project_data(&p,"sceneTranslations",payload.clone(),None).await.unwrap();
 let mut edited=payload.clone();edited["drafts"][0]["text"]=json!("你好，世界！");
 let changed=s.save_project_data(&p,"sceneTranslations",edited.clone(),Some(saved.revision_id.clone())).await.unwrap();
 assert!(s.save_project_data(&p,"sceneTranslations",payload.clone(),Some(saved.revision_id)).await.unwrap_err().starts_with("Conflict"));
 assert_eq!(s.load_project_data(&p,"sceneTranslations").await.unwrap().unwrap().payload,edited);
 let mut new_source=scene("changed manuscript");new_source.title="Changed".into();s.save_scene(&sid,new_source,Some(original.revision_id.clone())).await.unwrap();
 assert_eq!(s.load_project_data(&p,"sceneTranslations").await.unwrap().unwrap().payload,changed.payload);
}
#[tokio::test] async fn forged_or_foreign_source_is_refused_and_source_fields_cannot_be_rewritten(){
 let(_d,s,p,_c,sid)=setup().await;let src=s.load_scene(&sid).await.unwrap();
 let other=s.create_project("Other").await.unwrap();let c=s.create_chapter(&other.id,"C").await.unwrap();let foreign=s.create_scene(&c.id,scene("forged text")).await.unwrap();
 let mut forged=json!({"schemaVersion":1,"drafts":[draft("t",&sid,&src.revision_id,"not actual","x")]});assert!(s.save_project_data(&p,"sceneTranslations",forged.clone(),None).await.unwrap_err().contains("does not match"));
 forged["drafts"][0]=draft("t",&sid,&foreign.revision_id,"forged text","x");assert!(s.save_project_data(&p,"sceneTranslations",forged.clone(),None).await.unwrap_err().contains("does not belong"));
 forged["drafts"][0]=draft("t",&sid,&src.revision_id,"مَرْحَبًا 世界 😀","x");let saved=s.save_project_data(&p,"sceneTranslations",forged.clone(),None).await.unwrap();
 forged["drafts"][0]["sourceText"]=json!("changed wording");assert!(s.save_project_data(&p,"sceneTranslations",forged,Some(saved.revision_id)).await.unwrap_err().contains("immutable"));
}
#[tokio::test] async fn archive_import_remaps_scene_and_revision_references(){
 let(_d,s,p,_c,sid)=setup().await;let src=s.load_scene(&sid).await.unwrap();let payload=json!({"schemaVersion":1,"drafts":[draft("translation",&sid,&src.revision_id,"مَرْحَبًا 世界 😀","你好 世界")]});s.save_project_data(&p,"sceneTranslations",payload,None).await.unwrap();
 let archive=s.export_archive(&p).await.unwrap();let imported=s.import_archive(&archive).await.unwrap();let new_scene=&imported.scenes[0];assert_ne!(new_scene.id,sid);assert_ne!(new_scene.revision_id,src.revision_id);
 let trans=s.load_project_data(&imported.project.id,"sceneTranslations").await.unwrap().unwrap();assert_eq!(trans.payload["drafts"][0]["sceneId"],new_scene.id);assert_eq!(trans.payload["drafts"][0]["sceneRevisionId"],new_scene.revision_id);assert_eq!(trans.payload["drafts"][0]["text"],"你好 世界");
}
#[tokio::test] async fn old_drafts_must_be_retained_but_text_is_editable(){
 let(_d,s,p,_c,sid)=setup().await;let src=s.load_scene(&sid).await.unwrap();let base=json!({"schemaVersion":1,"drafts":[draft("t",&sid,&src.revision_id,"مَرْحَبًا 世界 😀","draft")]});let saved=s.save_project_data(&p,"sceneTranslations",base.clone(),None).await.unwrap();
 let empty=json!({"schemaVersion":1,"drafts":[]});assert!(s.save_project_data(&p,"sceneTranslations",empty,Some(saved.revision_id.clone())).await.unwrap_err().contains("retained"));
 let mut changed=base;changed["drafts"][0]["text"]=json!("saved edit");assert_eq!(s.save_project_data(&p,"sceneTranslations",changed.clone(),Some(saved.revision_id)).await.unwrap().payload,changed);
}
#[tokio::test] async fn branch_copy_remaps_all_source_links(){
 let(_d,s,p,_c,sid)=setup().await;let src=s.load_scene(&sid).await.unwrap();let payload=json!({"schemaVersion":1,"drafts":[draft("translation",&sid,&src.revision_id,"مَرْحَبًا 世界 😀","你好 世界")]});s.save_project_data(&p,"sceneTranslations",payload,None).await.unwrap();
 let branch=s.create_project_branch(&p,"Branch").await.unwrap();let bundle=s.open_project(&branch.project_id).await.unwrap();let copied=s.load_project_data(&branch.project_id,"sceneTranslations").await.unwrap().unwrap();
 assert_ne!(bundle.scenes[0].id,sid);assert_ne!(bundle.scenes[0].revision_id,src.revision_id);assert_eq!(copied.payload["drafts"][0]["sceneId"],bundle.scenes[0].id);assert_eq!(copied.payload["drafts"][0]["sceneRevisionId"],bundle.scenes[0].revision_id);
}
#[tokio::test] async fn sync_replication_checks_and_preserves_translation_sources(){
 let(_d,source,p,_c,sid)=setup().await;let dst_dir=tempfile::tempdir().unwrap();let target=Storage::open(&dst_dir.path().join("replica.sqlite")).await.unwrap();let src_cursor=source.sync_cursor().await.unwrap();let target_cursor=target.sync_cursor().await.unwrap();
 let initial=source.pull_revisions(&p,novel_storage::SyncCursor{epoch:src_cursor.epoch.clone(),sequence:0},1000).await.unwrap();target.ingest_revisions(&p,&target_cursor.epoch,&src_cursor.epoch,source.open_project(&p).await.unwrap(),initial.revisions).await.unwrap();
 let original=source.load_scene(&sid).await.unwrap();let data=json!({"schemaVersion":1,"drafts":[draft("sync-draft",&sid,&original.revision_id,"مَرْحَبًا 世界 😀","你好 世界")]});source.save_project_data(&p,"sceneTranslations",data.clone(),None).await.unwrap();
 let after=source.sync_cursor().await.unwrap();let page=source.pull_revisions(&p,novel_storage::SyncCursor{epoch:after.epoch.clone(),sequence:src_cursor.sequence},1000).await.unwrap();let target_cursor=target.sync_cursor().await.unwrap();target.ingest_revisions(&p,&target_cursor.epoch,&after.epoch,source.open_project(&p).await.unwrap(),page.revisions).await.unwrap();
 assert_eq!(target.load_project_data(&p,"sceneTranslations").await.unwrap().unwrap().payload,data);
}
#[tokio::test] async fn sync_rejects_forged_source_even_when_scene_revision_arrives_in_same_batch(){
 use sha2::{Digest,Sha256};
 let(_d,source,p,_c,sid)=setup().await;let src_cursor=source.sync_cursor().await.unwrap();let page=source.pull_revisions(&p,novel_storage::SyncCursor{epoch:src_cursor.epoch.clone(),sequence:0},1000).await.unwrap();
 let scene_revision=source.load_scene(&sid).await.unwrap();let forged_payload=json!({"schemaVersion":1,"drafts":[draft("forged-draft",&sid,&scene_revision.revision_id,"text that is not in the scene","x")]});
 let checksum=format!("{:x}",Sha256::digest(serde_json::to_string(&forged_payload).unwrap().as_bytes()));
 let forged=novel_storage::Revision{id:"forged-translation-revision".into(),project_id:p.clone(),entity_id:format!("{p}:sceneTranslations"),entity_type:"projectData".into(),parents:vec![],payload:forged_payload,operation:"sceneTranslations".into(),schema_version:1,checksum,created_at:"2026-10-10T00:00:00Z".into(),sequence:999,origin_epoch:src_cursor.epoch.clone(),origin_device:"forged-device".into()};
 let mut batch=vec![forged];batch.extend(page.revisions);assert!(batch.iter().any(|r|r.id==scene_revision.revision_id));
 let target_dir=tempfile::tempdir().unwrap();let target=Storage::open(&target_dir.path().join("replica.sqlite")).await.unwrap();let local=target.sync_cursor().await.unwrap();
 assert!(target.ingest_revisions(&p,&local.epoch,&src_cursor.epoch,source.open_project(&p).await.unwrap(),batch).await.unwrap_err().contains("does not match"));
 assert!(target.open_project(&p).await.is_err());
}
