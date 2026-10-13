use novel_storage::{Storage,UniverseCatalogue,SyncCursor};
use serde_json::json;
async fn fixture()->(tempfile::TempDir,Storage){let d=tempfile::tempdir().unwrap();let s=Storage::open(&d.path().join("universe.sqlite")).await.unwrap();(d,s)}
async fn data(s:&Storage,pid:&str)->novel_storage::ProjectDataSnapshot{s.load_project_data(pid,"universeCatalogue").await.unwrap().unwrap()}
#[tokio::test]
async fn standalone_novel_starts_with_one_editable_manuscript_and_reopens_exactly(){
 let (d,s)=fixture().await;let created=s.create_novel("A new novel مِ 中文 👩🏽‍🚀").await.unwrap();
 let index=s.open_project_index(&created.project.id).await.unwrap();assert_eq!(index.chapters.len(),1);assert_eq!(index.scenes.len(),1);
 assert_eq!(index.chapters[0].title,"Chapter 1");assert_eq!(index.scenes[0].id,created.scene.id);assert_eq!(index.chapters[0].id,created.scene.chapter_id);assert_eq!(created.scene.position,0);
 assert_eq!(novel_storage::plain_text(&created.scene.scene.document),"");assert!(s.load_project_data(&created.project.id,"universeCatalogue").await.unwrap().is_none());
 let mut scene=created.scene.scene.clone();scene.document["content"][0]["content"]=json!([{"type":"text","text":"The story begins. مِ 中文 👩🏽‍🚀"}]);
 let saved=s.save_scene(&created.scene.id,scene.clone(),Some(created.scene.revision_id.clone())).await.unwrap();assert!(s.save_scene(&created.scene.id,scene.clone(),Some(created.scene.revision_id)).await.is_err());
 s.close().await;let reopened=Storage::open(&d.path().join("universe.sqlite")).await.unwrap();assert_eq!(reopened.load_scene(&created.scene.id).await.unwrap().revision_id,saved.revision_id);assert_eq!(reopened.load_scene(&created.scene.id).await.unwrap().scene,scene);
 let imported=reopened.import_archive(&reopened.export_archive(&created.project.id).await.unwrap()).await.unwrap();assert_eq!(reopened.open_project(&imported.project.id).await.unwrap().scenes[0].scene,scene);
}
#[tokio::test]
async fn standalone_novel_failure_rolls_back_every_manuscript_record_and_can_retry(){
 use sqlx::sqlite::SqlitePoolOptions;
 let (d,s)=fixture().await;let before=s.sync_cursor().await.unwrap();assert!(s.create_novel(" ").await.is_err());assert!(s.create_novel(&"x".repeat(501)).await.is_err());
 let pool=SqlitePoolOptions::new().connect(&format!("sqlite:{}",d.path().join("universe.sqlite").display())).await.unwrap();
 sqlx::raw_sql("CREATE TRIGGER reject_initial_placement BEFORE INSERT ON revision_envelope WHEN NEW.entity_type='scenePlacement' BEGIN SELECT RAISE(ABORT,'synthetic initial scene failure'); END;").execute(&pool).await.unwrap();
 assert!(s.create_novel("Must roll back").await.is_err());assert!(s.list_projects().await.unwrap().is_empty());assert_eq!(s.sync_cursor().await.unwrap(),before);
 for table in ["chapter","manuscript_scene","revision_envelope"]{let count:i64=sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}")).fetch_one(&pool).await.unwrap();assert_eq!(count,0);}
 sqlx::raw_sql("DROP TRIGGER reject_initial_placement;").execute(&pool).await.unwrap();let created=s.create_novel("Retry").await.unwrap();assert_eq!(s.list_projects().await.unwrap().len(),1);assert_eq!(s.open_project_index(&created.project.id).await.unwrap().scenes.len(),1);
}
#[tokio::test]
async fn universe_atomic_novels_preserve_independent_manuscripts_and_shared_bible(){
 let (d,s)=fixture().await;let u=s.create_universe("عالم / 世界").await.unwrap();let initial=data(&s,&u.id).await;
 let payload=json!({"schemaVersion":1,"series":[{"id":"series-a","title":"سلسلة 中文"}],"novels":[]});
 let head=s.save_project_data(&u.id,"universeCatalogue",payload,Some(initial.revision_id)).await.unwrap();
 let first=s.create_universe_novel(&u.id,Some("series-a".into()),"Novel مِ 中文",&head.revision_id).await.unwrap();
 let second=s.create_universe_novel(&u.id,None,"Standalone 𠀀",&first.catalogue_revision_id).await.unwrap();
 assert_ne!(first.project.id,second.project.id);assert_ne!(first.scene.id,second.scene.id);assert_eq!(s.open_project_index(&first.project.id).await.unwrap().scenes[0].id,first.scene.id);
 assert!(s.open_project_index(&u.id).await.unwrap().scenes.is_empty());
 let cat:UniverseCatalogue=serde_json::from_value(data(&s,&u.id).await.payload).unwrap();assert_eq!(cat.novels[0].project_id,first.project.id);assert_eq!(cat.novels[1].series_id,None);
 s.save_project_data(&u.id,"planning",json!({"schemaVersion":1,"records":[],"facts":[]}),None).await.unwrap();assert!(s.load_project_data(&first.project.id,"planning").await.unwrap().is_none());
 s.close().await;let reopened=Storage::open(&d.path().join("universe.sqlite")).await.unwrap();assert_eq!(reopened.load_scene(&first.scene.id).await.unwrap().scene,first.scene.scene);assert_eq!(data(&reopened,&u.id).await.revision_id,second.catalogue_revision_id);
}
#[tokio::test]
async fn catalogue_schema_membership_roots_and_stale_heads_fail_without_partial_novels(){
 let (_d,s)=fixture().await;let u=s.create_universe("Universe").await.unwrap();let head=data(&s,&u.id).await;let count=s.list_projects().await.unwrap().len();
 assert!(s.create_universe_novel(&u.id,Some("absent".into()),"Novel",&head.revision_id).await.is_err());assert_eq!(s.list_projects().await.unwrap().len(),count);
 for payload in [json!({"schemaVersion":2,"series":[],"novels":[]}),json!({"schemaVersion":1,"series":[{"id":"a","title":"x"},{"id":"a","title":"y"}],"novels":[]}),json!({"schemaVersion":1,"series":[],"novels":[{"projectId":"missing","seriesId":null}]}),json!({"schemaVersion":1,"series":[],"novels":[{"projectId":u.id,"seriesId":null}]}),json!({"schemaVersion":1,"series":[],"novels":[],"extra":true})]{assert!(s.save_project_data(&u.id,"universeCatalogue",payload,Some(head.revision_id.clone())).await.is_err());}
 let novel=s.create_universe_novel(&u.id,None,"Novel",&head.revision_id).await.unwrap();assert!(s.create_universe_novel(&u.id,None,"Stale",&head.revision_id).await.unwrap_err().starts_with("Conflict"));assert_eq!(s.list_projects().await.unwrap().len(),count+1);
 let other=s.create_universe("Other").await.unwrap();let h=data(&s,&other.id).await;
 for pid in [&novel.project.id,&u.id]{assert!(s.save_project_data(&other.id,"universeCatalogue",json!({"schemaVersion":1,"series":[],"novels":[{"projectId":pid,"seriesId":null}]}),Some(h.revision_id.clone())).await.is_err());}
 assert!(s.save_project_data(&novel.project.id,"universeCatalogue",json!({"schemaVersion":1,"series":[],"novels":[]}),None).await.is_err());
}
#[tokio::test]
async fn marker_failure_rolls_back_catalogue_project_chapter_scene_and_change_feed(){
 use sqlx::sqlite::SqlitePoolOptions;
 let(d,s)=fixture().await;let u=s.create_universe("Universe").await.unwrap();let head=data(&s,&u.id).await;let before=s.sync_cursor().await.unwrap();
 let pool=SqlitePoolOptions::new().connect(&format!("sqlite:{}",d.path().join("universe.sqlite").display())).await.unwrap();
 sqlx::raw_sql("CREATE TRIGGER reject_catalogue BEFORE INSERT ON revision_envelope WHEN NEW.operation='universeCatalogue' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").execute(&pool).await.unwrap();
 assert!(s.create_universe_novel(&u.id,None,"Must roll back",&head.revision_id).await.is_err());assert_eq!(s.list_projects().await.unwrap().len(),1);assert_eq!(s.sync_cursor().await.unwrap(),before);assert_eq!(data(&s,&u.id).await,head);assert!(s.create_universe("Must roll back root").await.is_err());assert_eq!(s.list_projects().await.unwrap().len(),1);
}
#[tokio::test]
async fn catalogue_archives_sync_unresolved_refs_and_exact_head_recovery_preserve_ownership(){
 let (_d,s)=fixture().await;let u=s.create_universe("Universe 世界").await.unwrap();let initial=data(&s,&u.id).await;let novel=s.create_universe_novel(&u.id,None,"Novel مِ",&initial.revision_id).await.unwrap();let saved=data(&s,&u.id).await;
 let archive=s.export_archive(&u.id).await.unwrap();assert!(s.import_archive(&archive).await.is_err()); // Cannot claim an already-owned novel via a new root.
 let(_e,fresh)=fixture().await;let imported=fresh.import_archive(&archive).await.unwrap();assert_eq!(data(&fresh,&imported.project.id).await.payload,saved.payload);assert!(fresh.open_project_index(&novel.project.id).await.is_err());
 let(_r,replica)=fixture().await;let epoch=s.sync_cursor().await.unwrap().epoch;let local=replica.sync_cursor().await.unwrap().epoch;
 let page=s.pull_revisions(&u.id,SyncCursor{epoch:epoch.clone(),sequence:0},100).await.unwrap();replica.ingest_revisions(&u.id,&local,&epoch,s.sync_manifest(&u.id).await.unwrap(),page.revisions).await.unwrap();assert_eq!(data(&replica,&u.id).await.payload,saved.payload);assert!(replica.load_scene(&novel.scene.id).await.is_err());
 let page=s.pull_revisions(&novel.project.id,SyncCursor{epoch:epoch.clone(),sequence:0},100).await.unwrap();replica.ingest_revisions(&novel.project.id,&local,&epoch,s.sync_manifest(&novel.project.id).await.unwrap(),page.revisions).await.unwrap();assert_eq!(replica.load_scene(&novel.scene.id).await.unwrap().scene,novel.scene.scene);
 let points=s.project_time_points(&u.id,None,100).await.unwrap();let point=points.points.iter().find(|p|p.revision_id==initial.revision_id).unwrap();let preview=s.project_time_preview(&u.id,&points.epoch,point.sequence,&point.revision_id).await.unwrap();assert!(preview.objects.iter().any(|o|o.entity_id==format!("{}:universeCatalogue",u.id)));
 let restored=s.recover_time_piece(&u.id,&points.epoch,point.sequence,&point.revision_id,&format!("{}:universeCatalogue",u.id),"projectData",&initial.revision_id,vec![saved.revision_id.clone()]).await.unwrap();assert_ne!(restored.id,initial.revision_id);assert!(data(&s,&u.id).await.payload["novels"].as_array().unwrap().is_empty());assert_eq!(s.load_scene(&novel.scene.id).await.unwrap().scene,novel.scene.scene);
 assert!(s.recover_time_piece(&u.id,&points.epoch,point.sequence,&point.revision_id,&format!("{}:universeCatalogue",u.id),"projectData",&initial.revision_id,vec![saved.revision_id]).await.is_err());
}
#[tokio::test]
async fn concurrent_catalogue_heads_require_explicit_resolution_before_membership_changes(){
 let (_d,s)=fixture().await;let u=s.create_universe("Universe").await.unwrap();let base=data(&s,&u.id).await;let (_e,replica)=fixture().await;let epoch=s.sync_cursor().await.unwrap().epoch;let local=replica.sync_cursor().await.unwrap().epoch;
 let page=s.pull_revisions(&u.id,SyncCursor{epoch:epoch.clone(),sequence:0},100).await.unwrap();replica.ingest_revisions(&u.id,&local,&epoch,s.sync_manifest(&u.id).await.unwrap(),page.revisions).await.unwrap();let cursor=s.sync_cursor().await.unwrap().sequence;
 let a=s.save_project_data(&u.id,"universeCatalogue",json!({"schemaVersion":1,"series":[{"id":"a","title":"Alpha"}],"novels":[]}),Some(base.revision_id.clone())).await.unwrap();
 let b=replica.save_project_data(&u.id,"universeCatalogue",json!({"schemaVersion":1,"series":[{"id":"b","title":"Beta"}],"novels":[]}),Some(base.revision_id)).await.unwrap();
 let page=s.pull_revisions(&u.id,SyncCursor{epoch:epoch.clone(),sequence:cursor},100).await.unwrap();replica.ingest_revisions(&u.id,&local,&epoch,s.sync_manifest(&u.id).await.unwrap(),page.revisions).await.unwrap();
 assert_eq!(data(&replica,&u.id).await.revision_id,b.revision_id);assert!(replica.create_universe_novel(&u.id,None,"No silent winner",&b.revision_id).await.is_err());assert!(replica.save_project_data(&u.id,"universeCatalogue",b.payload.clone(),Some(b.revision_id.clone())).await.is_err());
 let heads=replica.project_data_heads(&u.id,"universeCatalogue").await.unwrap();assert_eq!(heads.revision_ids.len(),2);assert!(heads.revision_ids.contains(&a.revision_id));
 let merged=replica.resolve_project_data(&u.id,"universeCatalogue",json!({"schemaVersion":1,"series":[{"id":"a","title":"Alpha"},{"id":"b","title":"Beta"}],"novels":[]}),heads.revision_ids).await.unwrap();
 let novel=replica.create_universe_novel(&u.id,Some("b".into()),"Approved membership",&merged.revision_id).await.unwrap();assert_eq!(data(&replica,&u.id).await.payload["novels"][0]["projectId"],novel.project.id);
}
