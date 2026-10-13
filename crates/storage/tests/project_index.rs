use novel_storage::{Scene,Storage};
use serde_json::json;

fn scene(title:&str,text:&str)->Scene{
    Scene{schema_version:1,title:title.into(),document:json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})}
}
async fn setup()->(tempfile::TempDir,Storage){
    let directory=tempfile::tempdir().unwrap();
    let storage=Storage::open(&directory.path().join("synthetic.sqlite")).await.unwrap();
    (directory,storage)
}

#[tokio::test]
async fn project_index_orders_metadata_without_manuscript_payload(){
    let(_directory,storage)=setup().await;
    let project=storage.create_project("فجر 小说").await.unwrap();
    let first=storage.create_chapter(&project.id,"First").await.unwrap();
    let second=storage.create_chapter(&project.id,"Second").await.unwrap();
    let a=storage.create_scene(&first.id,scene("مِ 中文 😀","PRIVATE_SYNTHETIC_BODY")).await.unwrap();
    let b=storage.create_scene(&first.id,scene("B","another body")).await.unwrap();
    let c=storage.create_scene(&second.id,scene("C","third body")).await.unwrap();
    storage.reorder_chapters(&project.id,vec![second.id.clone(),first.id.clone()]).await.unwrap();
    storage.reorder_scenes(&first.id,vec![b.id.clone(),a.id.clone()]).await.unwrap();
    let index=storage.open_project_index(&project.id).await.unwrap();
    assert_eq!(index.project,project);
    assert_eq!(index.scenes.iter().map(|s|s.id.as_str()).collect::<Vec<_>>(),vec![c.id.as_str(),b.id.as_str(),a.id.as_str()]);
    assert_eq!(index.scenes[2].title,a.scene.title);
    assert_eq!(index.scenes[2].revision_id,a.revision_id);
    let wire=serde_json::to_value(index).unwrap();
    assert_eq!(wire["scenes"][2].as_object().unwrap().len(),5);
    assert!(wire["scenes"][2].get("scene").is_none());
    assert!(wire["scenes"][2].get("document").is_none());
    assert!(!wire.to_string().contains("PRIVATE_SYNTHETIC_BODY"));
    assert!(storage.open_project_index("missing").await.is_err());
    storage.close().await;
}

#[tokio::test]
async fn scoped_scene_load_tracks_current_head_live_membership_and_placement(){
    let(_directory,storage)=setup().await;
    let project=storage.create_project("Project").await.unwrap();
    let other=storage.create_project("Other").await.unwrap();
    let chapter=storage.create_chapter(&project.id,"One").await.unwrap();
    let destination=storage.create_chapter(&project.id,"Second chapter").await.unwrap();
    let created=storage.create_scene(&chapter.id,scene("Title","عَ 世界 𠀀 😀")).await.unwrap();
    let saved=storage.save_scene(&created.id,scene("Current","exact changed متن"),Some(created.revision_id)).await.unwrap();
    let loaded=storage.load_project_scene(&project.id,&created.id).await.unwrap();
    assert_eq!(loaded.scene,saved.scene);
    assert_eq!(loaded.revision_id,saved.revision_id);
    assert!(storage.load_project_scene(&other.id,&created.id).await.is_err());
    storage.move_scene(&created.id,&destination.id,0).await.unwrap();
    assert_eq!(storage.open_project_index(&project.id).await.unwrap().scenes[0].chapter_id,destination.id);
    assert_eq!(storage.load_project_scene(&project.id,&created.id).await.unwrap().chapter_id,destination.id);
    storage.delete_scene(&created.id,&saved.revision_id).await.unwrap();
    assert!(storage.open_project_index(&project.id).await.unwrap().scenes.is_empty());
    assert!(storage.load_project_scene(&project.id,&created.id).await.is_err());
    assert!(storage.load_scene(&created.id).await.is_ok()); // Recovery remains available.
    storage.close().await;
}

#[tokio::test]
async fn metadata_does_not_decode_unrelated_documents_but_active_load_verifies_checksum(){
    let(directory,storage)=setup().await;
    let project=storage.create_project("Project").await.unwrap();
    let chapter=storage.create_chapter(&project.id,"Chapter").await.unwrap();
    let good=storage.create_scene(&chapter.id,scene("Good","exact multilingual عَ 中文")).await.unwrap();
    let broken=storage.create_scene(&chapter.id,scene("Broken","body")).await.unwrap();
    let pool=sqlx::SqlitePool::connect(&format!("sqlite://{}",directory.path().join("synthetic.sqlite").display())).await.unwrap();
    // Deliberately damage only this disposable fixture to simulate disk corruption.
    sqlx::query("DROP TRIGGER immutable_revision_update").execute(&pool).await.unwrap();
    sqlx::query("UPDATE revision_envelope SET payload=? WHERE id=?").bind(r#"{"schemaVersion":1,"title":"Broken","document":null}"#).bind(&broken.revision_id).execute(&pool).await.unwrap();
    assert_eq!(storage.open_project_index(&project.id).await.unwrap().scenes.len(),2);
    assert_eq!(storage.load_project_scene(&project.id,&good.id).await.unwrap(),good);
    assert!(storage.load_project_scene(&project.id,&broken.id).await.unwrap_err().contains("checksum"));
    assert!(storage.open_project(&project.id).await.is_err());
    pool.close().await;storage.close().await;
}

#[tokio::test]
async fn metadata_rejects_malformed_or_nontext_title_without_coercion(){
    let(directory,storage)=setup().await;
    let project=storage.create_project("Project").await.unwrap();
    let chapter=storage.create_chapter(&project.id,"Chapter").await.unwrap();
    let record=storage.create_scene(&chapter.id,scene("Good","body")).await.unwrap();
    let pool=sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect(&format!("sqlite://{}",directory.path().join("synthetic.sqlite").display())).await.unwrap();
    // Bypass storage guards only in this disposable corruption fixture.
    sqlx::query("DROP TRIGGER immutable_revision_update").execute(&pool).await.unwrap();
    sqlx::query("PRAGMA ignore_check_constraints=ON").execute(&pool).await.unwrap();
    for payload in ["not JSON",r#"{"title":null}"#,r#"{"title":42}"#,r#"{"title":{}}"#,r#"{}"#]{
        sqlx::query("UPDATE revision_envelope SET payload=? WHERE id=?").bind(payload).bind(&record.revision_id).execute(&pool).await.unwrap();
        assert!(storage.open_project_index(&project.id).await.unwrap_err().contains("Unsupported saved scene title"));
    }
    pool.close().await;storage.close().await;
}
