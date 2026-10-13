use novel_storage::{Storage,native_desk::{NativeDeskPreferences,NativeDeskDirection}};
use serde_json::json;
#[test]
fn writing_view_preferences_are_strict_bounded_and_independent_from_text_direction(){
    assert_eq!(NativeDeskPreferences::from_project_settings(&json!({"other":{"dir":"rtl"}})).unwrap(),NativeDeskPreferences::default());
    for width in [0,360,720,1200]{let saved=json!({"nativeDesk":{"schemaVersion":1,"direction":"rtl","writingWidth":width}});let preferences=NativeDeskPreferences::from_project_settings(&saved).unwrap();assert_eq!(preferences.direction,NativeDeskDirection::Rtl);assert_eq!(preferences.writing_width,width);}
    for desk in [json!({"schemaVersion":1,"direction":"rtl","writingWidth":359}),json!({"schemaVersion":1,"direction":"ltr","writingWidth":1201}),json!({"schemaVersion":1,"direction":"auto","writingWidth":0}),json!({"schemaVersion":2,"direction":"ltr","writingWidth":0}),json!({"schemaVersion":1,"direction":"ltr","writingWidth":720.5}),json!({"schemaVersion":1,"direction":"ltr","writingWidth":-1}),json!({"schemaVersion":1,"direction":"ltr","writingWidth":0,"font":"private"}),json!({"direction":"ltr","writingWidth":0})]{assert!(NativeDeskPreferences::from_project_settings(&json!({"nativeDesk":desk})).is_err());}
}
#[tokio::test]
async fn saved_view_preserves_other_settings_and_refuses_stale_forms(){
    let d=tempfile::tempdir().unwrap();let path=d.path().join("desk.sqlite");let s=Storage::open(&path).await.unwrap();let p=s.create_project("View مِ 中文").await.unwrap();let c=s.create_chapter(&p.id,"A").await.unwrap();let scene=s.create_scene(&c.id,novel_storage::Scene{schema_version:1,title:"Scene".into(),document:json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"مِ 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"}]}]}]})}).await.unwrap();
    let original=json!({"schemaVersion":1,"other":{"text":"مِ 中文 𠀀 👩🏽‍🚀"},"nativeShortcuts":serde_json::to_value(novel_storage::native_shortcuts::NativeShortcutSettings::default()).unwrap()});let initial=s.save_project_data(&p.id,"settings",original.clone(),None).await.unwrap();let mut payload=original.clone();payload["nativeDesk"]=json!({"schemaVersion":1,"direction":"rtl","writingWidth":720});
    let saved=s.save_project_data_checked(&p.id,"settings",payload.clone(),Some(initial.revision_id.clone()),&[]).await.unwrap();assert_eq!(saved.payload["other"],original["other"]);assert_eq!(saved.payload["nativeShortcuts"],original["nativeShortcuts"]);assert_eq!(s.load_scene(&scene.id).await.unwrap().scene,scene.scene);assert_eq!(s.load_scene(&scene.id).await.unwrap().revision_id,scene.revision_id);
    let marker=s.sync_cursor().await.unwrap();assert!(s.save_project_data_checked(&p.id,"settings",original,Some(initial.revision_id),&[]).await.unwrap_err().contains("Conflict"));assert_eq!(s.sync_cursor().await.unwrap(),marker);
    s.close().await;let reopened=Storage::open(&path).await.unwrap();assert_eq!(reopened.load_project_data(&p.id,"settings").await.unwrap().unwrap().payload,payload);assert_eq!(reopened.load_scene(&scene.id).await.unwrap().scene,scene.scene);
}
#[tokio::test]
async fn invalid_view_payload_does_not_create_settings_or_history(){
    let d=tempfile::tempdir().unwrap();let s=Storage::open(&d.path().join("invalid.sqlite")).await.unwrap();let p=s.create_project("View").await.unwrap();let marker=s.sync_cursor().await.unwrap();
    assert!(s.save_project_data(&p.id,"settings",json!({"schemaVersion":1,"nativeDesk":{"schemaVersion":1,"direction":"rtl","writingWidth":1201}}),None).await.is_err());assert_eq!(s.sync_cursor().await.unwrap(),marker);assert!(s.load_project_data(&p.id,"settings").await.unwrap().is_none());
}
