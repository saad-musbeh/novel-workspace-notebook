use novel_storage::{
    Storage,
    Scene,
    ProjectAsset,
    plain_text
};
use serde_json::{
    json,
    Value
};
use sha2::{
    Digest,
    Sha256
};
fn scene(text:&str)->Scene{
    Scene{
        schema_version:1,
        title:"الفجر 晨光".into(),
        document:json!({
            "type":"doc","content":[{
                "type":"paragraph","attrs":{
                    "dir":"rtl","lang":"ar"
                },
                "content":[{
                    "type":"text","text":text,"marks":[{
                        "type":"bold"
                    }
                    ]
                }
                ]
            }
            ]
        }
        )
    }
}
fn checksum(bytes:&[u8])->String{
    format!("{:x}",Sha256::digest(bytes))
}
async fn setup()->(tempfile::TempDir,Storage){
    let d=tempfile::tempdir().unwrap();
    let s=Storage::open(&d.path().join("test.sqlite")).await.unwrap();
    (d,s)
}
async fn project(s:&Storage)->(String,String,String){
    let p=s.create_project("Novel").await.unwrap();
    let c=s.create_chapter(&p.id,"Chapter 1").await.unwrap();
    let scene=s.create_scene(&c.id,scene("مَرْحَبًا 世界 𠀀 😀 \u{e000}")).await.unwrap();
    (p.id,c.id,scene.id)
}
fn corrupt_archive(archive:&str,mutate:impl FnOnce(&mut Value))->String{
    let mut a:Value=serde_json::from_str(archive).unwrap();
    let mut content:Value=serde_json::from_str(a["entries"][0]["content"].as_str().unwrap()).unwrap();
    mutate(&mut content);
    let content=serde_json::to_string(&content).unwrap();
    a["entries"][0]["size"]=json!(content.len());
    a["entries"][0]["sha256"]=json!(checksum(content.as_bytes()));
    a["entries"][0]["content"]=json!(content);
    serde_json::to_string(&a).unwrap()
}
#[tokio::test]
async fn stable_navigation_rename_and_reorder(){
    let (_d,s)=setup().await;
    let(p,c,sid)=project(&s).await;
    let c2=s.create_chapter(&p,"Second").await.unwrap();
    let x=s.create_scene(&c,scene("other")).await.unwrap();
    s.rename_project(&p,"Renamed").await.unwrap();
    s.rename_chapter(&c,"Opening").await.unwrap();
    let old=s.load_scene(&sid).await.unwrap();
    s.rename_scene(&sid,"Scene renamed",Some(old.revision_id)).await.unwrap();
    s.reorder_chapters(&p,vec![c2.id.clone(),c.clone()]).await.unwrap();
    s.reorder_scenes(&c,vec![x.id.clone(),sid.clone()]).await.unwrap();
    let bundle=s.open_project(&p).await.unwrap();
    assert_eq!(bundle.project.id,p);
    assert_eq!(bundle.chapters[0].id,c2.id);
    assert_eq!(bundle.scenes[0].id,x.id);
    assert_eq!(bundle.scenes[1].id,sid);
    assert_eq!(bundle.scenes[1].scene.title,"Scene renamed");
    assert!(s.reorder_scenes(&c,vec![sid.clone(),sid]).await.is_err());
}
#[tokio::test]
async fn restore_is_new_revision_and_stale_restore_is_rejected(){
    let(_d,s)=setup().await;
    let(_,_,sid)=project(&s).await;
    let first=s.load_scene(&sid).await.unwrap();
    let latest=s.save_scene(&sid,scene("new"),Some(first.revision_id.clone())).await.unwrap();
    let restored=s.restore_scene(&sid,&first.revision_id,&latest.revision_id).await.unwrap();
    assert_eq!(restored.scene,first.scene);
    assert_ne!(restored.revision_id,first.revision_id);
    assert!(s.restore_scene(&sid,&latest.revision_id,&latest.revision_id).await.unwrap_err().starts_with("Conflict"));
    assert_eq!(s.history(&sid).await.unwrap().len(),3);
    let other=s.create_project("Other").await.unwrap();
    let c=s.create_chapter(&other.id,"C").await.unwrap();
    let other=s.create_scene(&c.id,scene("other")).await.unwrap();
    assert!(s.load_revision(&other.id,&first.revision_id).await.is_err());
}
#[tokio::test]
async fn conflicts_preserve_both_heads_and_resolution_acknowledges_parents(){
    let(_d,s)=setup().await;
    let(_,_,sid)=project(&s).await;
    let first=s.load_scene(&sid).await.unwrap();
    let current=s.save_scene(&sid,scene("current"),Some(first.revision_id.clone())).await.unwrap();
    assert!(s.save_scene(&sid,scene("branch"),Some(first.revision_id.clone())).await.is_err());
    let branch=s.preserve_conflict(&sid,scene("branch"),&first.revision_id).await.unwrap();
    assert_eq!(s.load_scene(&sid).await.unwrap(),current);
    assert_eq!(s.conflicts(&sid).await.unwrap().len(),1);
    let merged=s.resolve_scene(&sid,scene("merged"),&current.revision_id,vec![branch.revision_id.clone()]).await.unwrap();
    let revisions=s.history(&sid).await.unwrap();
    assert_eq!(revisions[0].parents,vec![current.revision_id,branch.revision_id]);
    assert_eq!(revisions[0].id,merged.revision_id);
    assert!(s.conflicts(&sid).await.unwrap().is_empty());
}
#[tokio::test]
async fn exact_search_preserves_marks_and_statistics_are_separate(){
    let(_d,s)=setup().await;
    let(p,_,sid)=project(&s).await;
    let before=s.load_scene(&sid).await.unwrap();
    let hits=s.search(&p,"مَرْحَبًا").await.unwrap();
    assert_eq!(hits.len(),1);
    assert_eq!(hits[0].start_byte,0);
    assert!(s.search(&p,"مرحبا").await.unwrap().is_empty());
    assert_eq!(s.search(&p,"𠀀").await.unwrap().len(),1);
    let stats=s.statistics(&p).await.unwrap();
    assert_eq!(stats.cjk_characters,3);
    assert!(stats.graphemes<stats.unicode_scalars);
    assert_eq!(s.load_scene(&sid).await.unwrap(),before);
}
#[test]
fn projection_matches_editor_blank_paragraphs_and_hard_breaks(){
    let doc=json!({
        "type":"doc","content":[{
            "type":"paragraph","content":[{
                "type":"text","text":"😀"
            },
            {
                "type":"hardBreak"
            },
            {
                "type":"text","text":"مَ"
            }
            ]
        },
        {
            "type":"paragraph"
        },
        {
            "type":"paragraph","content":[{
                "type":"text","text":"𠀀"
            }
            ]
        }
        ]
    }
    );
    assert_eq!(plain_text(&doc),"😀\nمَ\n\n\n\n𠀀");
}
#[tokio::test]
async fn archive_roundtrip_keeps_formatting_all_history_assets_and_remaps_planning(){
    let(_d,s)=setup().await;
    let(p,c,sid)=project(&s).await;
    let first=s.load_scene(&sid).await.unwrap();
    s.save_scene(&sid,scene("revised 😀"),Some(first.revision_id)).await.unwrap();
    let planning=json!({
        "schemaVersion":1,"fields":[{
            "id":"f","kind":"links","recordKind":"character"
        }
        ],"records":[{
            "id":"character","kind":"character","links":[{
                "kind":"scene","id":sid
            }
            ],"values":{
                "f":[{
                    "kind":"scene","id":sid
                }
                ]
            }
        }
        ],"facts":[],"boards":[{
            "id":"b","cards":[{
                "id":"card","reference":{
                    "kind":"scene","id":sid
                }
            }
            ]
        }
        ],"lanes":[],"events":[{
            "id":"event","sceneIds":[sid]
        }
        ],"constraints":[],"issues":[{
            "id":"issue",
            "links":[{
                "kind":"scene","id":sid
            }
            ],"evidence":[{
                "id":"evidence",
                "link":{
                    "kind":"scene","id":sid
                }
            }
            ]
        }
        ]
    }
    );
    let data=s.save_project_data(&p,"planning",planning,None).await.unwrap();
    s.save_project_data(&p,"planning",data.payload.clone(),Some(data.revision_id)).await.unwrap();
    let bytes=b"synthetic dictionary".to_vec();
    s.put_asset(&p,ProjectAsset{
        id:"dictionary".into(),path:"assets/dictionary.txt".into(),mime_type:"text/plain".into(),checksum:checksum(&bytes),content:bytes
    }
    ).await.unwrap();
    let archive=s.export_archive(&p).await.unwrap();
    let restored=s.import_archive(&archive).await.unwrap();
    assert_ne!(restored.project.id,p);
    assert_ne!(restored.chapters[0].id,c);
    let new=&restored.scenes[0];
    assert_ne!(new.id,sid);
    assert_eq!(new.scene,s.load_scene(&sid).await.unwrap().scene);
    assert_eq!(s.history(&new.id).await.unwrap().len(),2);
    assert_eq!(s.project_data_history(&restored.project.id,"planning").await.unwrap().len(),2);
    let planning=s.load_project_data(&restored.project.id,"planning").await.unwrap().unwrap().payload;
    assert_eq!(planning["records"][0]["id"],"character");
    assert_eq!(planning["records"][0]["links"][0]["id"],new.id);
    assert_eq!(planning["records"][0]["values"]["f"][0]["id"],new.id);
    assert_eq!(planning["events"][0]["sceneIds"][0],new.id);
    assert_eq!(s.list_assets(&restored.project.id).await.unwrap().len(),1);
    assert!(s.export_archive(&restored.project.id).await.is_ok());
}
#[tokio::test]
async fn archive_rejects_corruption_paths_schema_missing_parents_and_bad_links_atomically(){
    let(_d,s)=setup().await;
    let(p,_,_)=project(&s).await;
    let archive=s.export_archive(&p).await.unwrap();
    let initial=s.list_projects().await.unwrap().len();
    let mut a:Value=serde_json::from_str(&archive).unwrap();
    a["entries"][0]["path"]=json!("../../outside");
    assert!(s.import_archive(&a.to_string()).await.is_err());
    let mut a:Value=serde_json::from_str(&archive).unwrap();
    a["entries"][0]["sha256"]=json!("wrong");
    assert!(s.import_archive(&a.to_string()).await.is_err());
    let bad=corrupt_archive(&archive,|d|d["bundle"]["scenes"][0]["scene"]["schemaVersion"]=json!(88));
    assert!(s.import_archive(&bad).await.is_err());
    let bad=corrupt_archive(&archive,|d|d["revisions"][0]["parents"]=json!(["missing"]));
    assert!(s.import_archive(&bad).await.is_err());
    let bad=corrupt_archive(&archive,|d|d["bundle"]["chapters"][0]["projectId"]=json!("other"));
    assert!(s.import_archive(&bad).await.is_err());
    assert_eq!(s.list_projects().await.unwrap().len(),initial);
}
#[tokio::test]
async fn project_data_cas_invalid_scene_links_and_asset_paths(){
    let(_d,s)=setup().await;
    let(p,_,sid)=project(&s).await;
    let d=s.save_project_data(&p,"settings",json!({
        "theme":"dark"
    }
    ),None).await.unwrap();
    assert!(s.save_project_data(&p,"settings",json!({
        "theme":"light"
    }
    ),None).await.unwrap_err().starts_with("Conflict"));
    assert_eq!(s.load_project_data(&p,"settings").await.unwrap().unwrap(),d);
    assert!(s.save_project_data(&p,"credentials",json!({
    }
    ),None).await.is_err());
    assert!(s.save_project_data(&p,"planning",json!({
        "schemaVersion":1,"events":[{
            "id":"event",
            "sceneIds":["nonexistent"]
        }
        ]
    }
    ),None).await.is_err());
    assert!(s.save_project_data(&p,"planning",json!({
        "schemaVersion":1,"events":[{
            "id":"event",
            "sceneIds":[sid]
        }
        ]
    }
    ),None).await.is_ok());
    let bytes=vec![1,2];
    assert!(s.put_asset(&p,ProjectAsset{
        id:"font".into(),path:"assets/../font".into(),mime_type:"font/ttf".into(),checksum:checksum(&bytes),content:bytes
    }
    ).await.is_err());
}
#[tokio::test]
async fn migration_preserves_prototype_history_and_creates_consistent_snapshot(){
    let d=tempfile::tempdir().unwrap();
    let path=d.path().join("legacy.sqlite");
    let options=sqlx::sqlite::SqliteConnectOptions::new().filename(&path).create_if_missing(true).journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool=sqlx::sqlite::SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::raw_sql(include_str!("../src/schema.sql")).execute(&pool).await.unwrap();
    let first=scene("original مَ");
    let second=scene("latest 世界");
    sqlx::query("INSERT INTO scene_revision(id,parent,payload) VALUES('r1',NULL,?),('r2','r1',?)").bind(serde_json::to_string(&first).unwrap()).bind(serde_json::to_string(&second).unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO scene_head VALUES(1,'r2')").execute(&pool).await.unwrap();
    let s=Storage::open(&path).await.unwrap();
    assert_eq!(s.load().await.unwrap().unwrap().scene,second);
    let b=s.open_project("prototype-project").await.unwrap();
    assert_eq!(b.scenes[0].revision_id,"r2");
    assert_eq!(s.history("prototype-scene").await.unwrap().len(),2);
    assert_eq!(s.load_revision("prototype-scene","r1").await.unwrap().scene,first);
    let backup=std::fs::read_dir(d.path()).unwrap().map(|r|r.unwrap().path()).find(|p|p.file_name().unwrap().to_string_lossy().contains("pre-v2")).unwrap();
    let backup=sqlx::sqlite::SqlitePoolOptions::new().connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(backup).read_only(true)).await.unwrap();
    let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&backup).await.unwrap();
    assert_eq!(version,1);
    let integrity:String=sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&backup).await.unwrap();
    assert_eq!(integrity,"ok");
    assert!(s.export_archive("prototype-project").await.is_ok());
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM scene_revision").fetch_one(&backup).await.unwrap();
    assert_eq!(count,2);
    let foreign_keys:Vec<sqlx::sqlite::SqliteRow>=sqlx::query("PRAGMA foreign_key_check").fetch_all(&pool).await.unwrap();
    assert!(foreign_keys.is_empty());
    pool.close().await;
}
#[tokio::test]
async fn move_snapshot_change_feed_idempotent_commit_and_backup(){
    let(d,s)=setup().await;
    let(p,c,sid)=project(&s).await;
    let c2=s.create_chapter(&p,"Chapter 2").await.unwrap();
    let initial=s.load_scene(&sid).await.unwrap();
    let request="save-request-1";
    let saved=s.save_scene_with_id(&sid,scene("acknowledged"),Some(initial.revision_id.clone()),request).await.unwrap();
    let retried=s.save_scene_with_id(&sid,scene("acknowledged"),Some(initial.revision_id.clone()),request).await.unwrap();
    assert_eq!(saved,retried);
    assert_eq!(s.history(&sid).await.unwrap().len(),2);
    assert!(s.save_scene_with_id(&sid,scene("different"),Some(initial.revision_id),request).await.is_err());
    let snapshot=s.snapshot_scene(&sid,&saved.revision_id).await.unwrap();
    assert_eq!(snapshot.scene,saved.scene);
    s.move_scene(&sid,&c2.id,0).await.unwrap();
    let b=s.open_project(&p).await.unwrap();
    assert_eq!(b.scenes[0].chapter_id,c2.id);
    assert_eq!(b.scenes[0].id,sid);
    assert!(s.reorder_scenes(&c,vec![sid.clone()]).await.is_err());
    let changes=s.changes(0,100).await.unwrap();
    assert!(changes.windows(2).all(|v|v[0].sequence<v[1].sequence));
    assert!(changes.iter().all(|r|!r.origin_epoch.is_empty()));
    assert!(s.changes(changes.last().unwrap().sequence,100).await.unwrap().is_empty());
    let imported=s.import_archive(&s.export_archive(&p).await.unwrap()).await.unwrap();
    assert_eq!(imported.scenes[0].chapter_id,imported.chapters[1].id);
    let backup=d.path().join("backup.sqlite");
    s.backup(&backup).await.unwrap();
    assert!(s.backup(&backup).await.is_err());
    let b=Storage::open(&backup).await.unwrap();
    assert_eq!(b.load_scene(&sid).await.unwrap(),snapshot);
}
#[tokio::test]
async fn two_connections_reject_stale_scene_and_project_data(){
    let(d,s)=setup().await;
    let(p,_,sid)=project(&s).await;
    let second=Storage::open(&d.path().join("test.sqlite")).await.unwrap();
    let first=s.load_scene(&sid).await.unwrap();
    let current=s.save_scene(&sid,scene("writer one"),Some(first.revision_id.clone())).await.unwrap();
    assert!(second.save_scene(&sid,scene("writer two"),Some(first.revision_id)).await.is_err());
    assert_eq!(second.load_scene(&sid).await.unwrap(),current);
    let settings=s.save_project_data(&p,"settings",json!({
        "font":18
    }
    ),None).await.unwrap();
    assert!(second.save_project_data(&p,"settings",json!({
        "font":20
    }
    ),None).await.is_err());
    assert_eq!(second.load_project_data(&p,"settings").await.unwrap().unwrap(),settings);
}
#[tokio::test]
async fn native_data_wrappers_and_immutable_mapping_versions(){
    let(_d,s)=setup().await;
    let(p,_,sid)=project(&s).await;
    assert!(s.save_project_data(&p,"languagePacks",json!({
        "schemaVersion":1,"packs":"wrong"
    }
    ),None).await.is_err());
    assert!(s.save_project_data(&p,"assistanceJobs",json!({
        "schemaVersion":1,"jobs":[{
        }
        ]
    }
    ),None).await.is_err());
    let unsafe_settings=json!({
        "assistance":{
            "schemaVersion":1,"providers":[{
                "kind":"llm","endpoint":"https://cloud.example"
            },
            {
                "kind":"languagetool","endpoint":"http://127.0.0.1:8081"
            }
            ],"exceptions":[],"suggestions":[]
        }
    }
    );
    assert!(s.save_project_data(&p,"settings",unsafe_settings,None).await.is_err());
    let definition=json!({
        "schemaVersion":1,"id":"alphabet","mappingVersion":1,"mappings":[{
            "input":"a","output":"\u{e000}"
        }
        ]
    }
    );
    let d=s.save_project_data(&p,"writingSystems",json!({
        "schemaVersion":1,"systems":[definition],"assignments":[{
            "scene":{
                "kind":"scene","id":sid
            },
            "definitionId":"alphabet","mappingVersion":1
        }
        ]
    }
    ),None).await.unwrap();
    let mut mutated=d.payload.clone();
    mutated["systems"][0]["mappings"][0]["output"]=json!("\u{e001}");
    assert!(s.save_project_data(&p,"writingSystems",mutated,Some(d.revision_id.clone())).await.is_err());
    let deleted=json!({
        "schemaVersion":1,"systems":[],"assignments":[]
    }
    );
    assert!(s.save_project_data(&p,"writingSystems",deleted,Some(d.revision_id.clone())).await.is_err());
    let mut next=d.payload.clone();
    let mut v2=next["systems"][0].clone();
    v2["mappingVersion"]=json!(2);
    v2["mappings"][0]["output"]=json!("\u{e001}");
    next["systems"].as_array_mut().unwrap().push(v2);
    s.save_project_data(&p,"writingSystems",next,Some(d.revision_id)).await.unwrap();
    assert!(s.import_archive(&s.export_archive(&p).await.unwrap()).await.is_ok());
}
#[tokio::test]
async fn additive_v2_upgrade_preserves_commits_and_has_consistent_backup(){
    let(d,s)=setup().await;
    let saved=s.save(scene("v2 acknowledged مَ 𠀀"),None).await.unwrap();
    s.close().await;
    let path=d.path().join("test.sqlite");
    let pool=sqlx::sqlite::SqlitePoolOptions::new().connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path)).await.unwrap();
    sqlx::raw_sql("DROP TABLE completed_asset_transfer; DROP TABLE sync_receipt; DROP TABLE asset_transfer; ALTER TABLE manuscript_scene DROP COLUMN deleted; PRAGMA user_version=2;").execute(&pool).await.unwrap();
    pool.close().await;
    let migrated=Storage::open(&path).await.unwrap();
    assert_eq!(migrated.load().await.unwrap(),Some(saved));
    let backup=std::fs::read_dir(d.path()).unwrap().map(|e|e.unwrap().path()).find(|p|p.file_name().unwrap().to_string_lossy().contains("pre-v3")).unwrap();
    let backup=sqlx::sqlite::SqlitePoolOptions::new().connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(backup).read_only(true)).await.unwrap();
    let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&backup).await.unwrap();
    assert_eq!(version,2);
    let integrity:String=sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&backup).await.unwrap();
    assert_eq!(integrity,"ok");
}
#[tokio::test]
async fn checked_review_commit_rejects_changed_or_conflicted_supporting_sources_atomically(){let(_d,s)=setup().await;let(p,c,target)=project(&s).await;let other=s.create_scene(&c,scene("supporting scene")).await.unwrap();let facts=s.save_project_data(&p,"planning",json!({"schemaVersion":1,"facts":[],"records":[]}),None).await.unwrap();let target_head=s.load_scene(&target).await.unwrap();let deps=vec![novel_storage::SourceDependency{entity_id:other.id.clone(),entity_type:"scene".into(),revision_id:other.revision_id.clone()},novel_storage::SourceDependency{entity_id:format!("{p}:planning"),entity_type:"projectData".into(),revision_id:facts.revision_id.clone()}];let snapshot=s.load_project_data_revision(&p,"planning",&facts.revision_id).await.unwrap();assert_eq!(snapshot,facts);let foreign=s.create_project("Foreign").await.unwrap();assert!(s.load_project_data_revision(&foreign.id,"planning",&facts.revision_id).await.is_err());s.save_project_data(&p,"planning",json!({"schemaVersion":1,"facts":[],"records":[],"issues":[]}),Some(facts.revision_id)).await.unwrap();assert!(s.save_scene_checked(&target,scene("should never apply"),&target_head.revision_id,&deps).await.unwrap_err().starts_with("Conflict"));assert_eq!(s.load_scene(&target).await.unwrap(),target_head);assert_eq!(s.history(&target).await.unwrap().len(),1);let facts=s.load_project_data(&p,"planning").await.unwrap().unwrap();let mut deps=deps;deps[1].revision_id=facts.revision_id;let supporting_latest=s.save_scene(&other.id,scene("current support"),Some(other.revision_id.clone())).await.unwrap();deps[0].revision_id=supporting_latest.revision_id;s.preserve_conflict(&other.id,scene("competing support"),&other.revision_id).await.unwrap();assert!(s.save_scene_checked(&target,scene("still must not apply"),&target_head.revision_id,&deps).await.is_err());assert_eq!(s.history(&target).await.unwrap().len(),1);let support=s.load_scene(&other.id).await.unwrap();let conflict=s.conflicts(&other.id).await.unwrap()[0].id.clone();let resolved=s.resolve_scene(&other.id,scene("resolved support"),&support.revision_id,vec![conflict]).await.unwrap();deps[0].revision_id=resolved.revision_id;let applied=s.save_scene_checked(&target,scene("author approved"),&target_head.revision_id,&deps).await.unwrap();assert_eq!(applied.scene,scene("author approved"));}
#[tokio::test]
async fn scene_metadata_validates_and_remaps_without_changing_scene_contract(){let(_d,s)=setup().await;let(p,_,sid)=project(&s).await;let payload=json!({"schemaVersion":1,"scenes":[{"sceneId":sid,"synopsis":"عَ 世界 😀","status":"revising","viewpointId":"character-stable","tags":["opening"],"eventIds":["event-stable"]}]});let saved=s.save_project_data(&p,"sceneMetadata",payload.clone(),None).await.unwrap();let mut invalid=payload;invalid["scenes"][0]["status"]=json!("unknown");assert!(s.save_project_data(&p,"sceneMetadata",invalid,Some(saved.revision_id)).await.is_err());let imported=s.import_archive(&s.export_archive(&p).await.unwrap()).await.unwrap();let restored=s.load_project_data(&imported.project.id,"sceneMetadata").await.unwrap().unwrap();assert_eq!(restored.payload["scenes"][0]["sceneId"],imported.scenes[0].id);assert_eq!(restored.payload["scenes"][0]["viewpointId"],"character-stable");assert_eq!(restored.payload["scenes"][0]["eventIds"][0],"event-stable");}
#[tokio::test]
async fn generic_revision_preview_requires_exact_project_object_and_type(){let(_d,s)=setup().await;let(p,_,sid)=project(&s).await;let head=s.load_scene(&sid).await.unwrap();let loaded=s.load_entity_revision(&p,&sid,"scene",&head.revision_id).await.unwrap();assert_eq!(loaded.payload,serde_json::to_value(&head.scene).unwrap());assert!(s.load_entity_revision("other-project",&sid,"scene",&head.revision_id).await.is_err());assert!(s.load_entity_revision(&p,"other-scene","scene",&head.revision_id).await.is_err());assert!(s.load_entity_revision(&p,&sid,"chapter",&head.revision_id).await.is_err());let deleted=s.delete_scene(&sid,&head.revision_id).await.unwrap();let tombstone=s.load_entity_revision(&p,&sid,"scene",&deleted).await.unwrap();assert_eq!(tombstone.entity_type,"tombstone");}

#[tokio::test]
async fn confirmed_fact_commit_requires_current_unconflicted_same_project_sources(){
    let(_d,s)=setup().await;
    let(p,c,source)=project(&s).await;
    let original=s.load_scene(&source).await.unwrap();
    let dependency=novel_storage::SourceDependency{entity_id:source.clone(),entity_type:"scene".into(),revision_id:original.revision_id.clone()};
    let approved=json!({"schemaVersion":1,"facts":[{"id":"fact-1","confirmed":true}],"records":[]});
    let current=s.save_scene(&source,scene("Changed evidence"),Some(original.revision_id.clone())).await.unwrap();
    assert!(s.save_project_data_checked(&p,"planning",approved.clone(),None,&[dependency.clone()]).await.unwrap_err().starts_with("Conflict"));
    assert!(s.load_project_data(&p,"planning").await.unwrap().is_none());
    assert!(s.project_data_history(&p,"planning").await.unwrap().is_empty());
    let dependency=novel_storage::SourceDependency{revision_id:current.revision_id.clone(),..dependency};
    s.preserve_conflict(&source,scene("Competing evidence"),&original.revision_id).await.unwrap();
    assert!(s.save_project_data_checked(&p,"planning",approved.clone(),None,&[dependency.clone()]).await.is_err());
    let conflict=s.conflicts(&source).await.unwrap()[0].id.clone();
    let resolved=s.resolve_scene(&source,scene("Resolved evidence"),&current.revision_id,vec![conflict]).await.unwrap();
    let dependency=novel_storage::SourceDependency{revision_id:resolved.revision_id,..dependency};
    let foreign=s.create_project("Foreign").await.unwrap();
    assert!(s.save_project_data_checked(&foreign.id,"planning",approved.clone(),None,&[dependency.clone()]).await.is_err());
    let confirmed=s.save_project_data_checked(&p,"planning",approved.clone(),None,&[dependency.clone()]).await.unwrap();
    assert_eq!(confirmed.payload,approved);
    assert!(s.save_project_data_checked(&p,"planning",json!({"schemaVersion":1,"facts":[]}),None,&[dependency]).await.is_err());
    assert_eq!(s.project_data_history(&p,"planning").await.unwrap().len(),1);
    assert!(s.create_scene(&c,scene("Unaffected scene")).await.is_ok());
}
