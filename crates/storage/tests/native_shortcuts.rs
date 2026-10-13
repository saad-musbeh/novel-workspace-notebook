use novel_storage::{Storage,native_shortcuts::{NativeShortcutSettings,NativeShortcutBinding,parse_native_shortcut_list}};
use serde_json::json;
#[test]
fn shortcut_defaults_and_text_roundtrip_are_exact(){
    let defaults=NativeShortcutSettings::default();defaults.validate().unwrap();
    for bindings in defaults.bindings.values(){let text=bindings.iter().map(NativeShortcutBinding::display).collect::<Vec<_>>().join(", ");assert_eq!(parse_native_shortcut_list(&text).unwrap(),*bindings);}
    assert_eq!(parse_native_shortcut_list("  control+shift+s, f8 ").unwrap().iter().map(NativeShortcutBinding::display).collect::<Vec<_>>(),vec!["Ctrl+Shift+S","F8"]);assert!(parse_native_shortcut_list(" ").unwrap().is_empty());
    for invalid in ["a","Shift+S","4","Ctrl+Alt+Q","Meta+S","Ctrl+Ctrl+S","Ctrl+Control+S","Alt+Tab","Tab","Ctrl+Escape","Enter","Ctrl+Backspace","F1","F6","F7","F11","Alt+F4","F25","F08","Ctrl+S,Ctrl+T,Ctrl+W,F8","Ctrl+"]{assert!(parse_native_shortcut_list(invalid).is_err(),"{invalid}");}
}
#[test]
fn shortcuts_reject_duplicates_across_scopes_and_unknown_or_missing_actions(){
    let defaults=NativeShortcutSettings::default();let mut same=defaults.clone();same.bindings.insert("editor.bold".into(),same.bindings["workspace.save"].clone());assert!(same.validate().unwrap_err().contains("more than once"));
    let mut same=defaults.clone();let duplicate=same.bindings["editor.bold"][0].clone();same.bindings.get_mut("editor.bold").unwrap().push(duplicate);assert!(same.validate().is_err());
    let mut unknown=defaults.clone();unknown.bindings.insert("workspace.remoteCommand".into(),vec![]);assert!(unknown.validate().is_err());let mut absent=defaults.clone();absent.bindings.remove("editor.bold");assert!(absent.validate().is_err());
    let mut disabled=defaults.clone();disabled.bindings.insert("editor.bold".into(),vec![]);disabled.validate().unwrap();let chord=defaults.bindings["editor.bold"][0].clone();assert_eq!(defaults.action("editor.",&chord),Some("editor.bold"));assert_eq!(defaults.action("workspace.",&chord),None);assert_eq!(disabled.action("editor.",&chord),None);
    let mut fields=serde_json::to_value(defaults).unwrap();fields["unknown"]=json!(true);assert!(NativeShortcutSettings::from_project_settings(&json!({"nativeShortcuts":fields})).is_err());
}
#[tokio::test]
async fn shortcut_settings_preserve_other_configuration_and_refuse_stale_concurrent_forms(){
    let d=tempfile::tempdir().unwrap();let path=d.path().join("shortcuts.sqlite");let s=Storage::open(&path).await.unwrap();let p=s.create_project("Shortcut fixture مِ 中文").await.unwrap();
    let prior=json!({"schemaVersion":1,"otherPreference":{"text":"مِ 中文 𠀀 👩🏽‍🚀"}});let saved=s.save_project_data(&p.id,"settings",prior.clone(),None).await.unwrap();
    let mut changed=NativeShortcutSettings::default();changed.bindings.insert("editor.bold".into(),parse_native_shortcut_list("Ctrl+Shift+B").unwrap());let mut payload=prior.clone();payload["nativeShortcuts"]=serde_json::to_value(&changed).unwrap();
    let committed=s.save_project_data_checked(&p.id,"settings",payload.clone(),Some(saved.revision_id.clone()),&[]).await.unwrap();assert_eq!(committed.payload["otherPreference"],prior["otherPreference"]);assert_eq!(NativeShortcutSettings::from_project_settings(&committed.payload).unwrap(),changed);
    let marker=s.sync_cursor().await.unwrap();assert!(s.save_project_data_checked(&p.id,"settings",prior,Some(saved.revision_id),&[]).await.unwrap_err().contains("Conflict"));assert_eq!(s.sync_cursor().await.unwrap(),marker);
    s.close().await;let reopened=Storage::open(&path).await.unwrap();assert_eq!(reopened.load_project_data(&p.id,"settings").await.unwrap().unwrap().payload,payload);
}
#[tokio::test]
async fn invalid_shortcut_payload_cannot_commit_or_advance_history(){
    let d=tempfile::tempdir().unwrap();let s=Storage::open(&d.path().join("invalid.sqlite")).await.unwrap();let p=s.create_project("Shortcuts").await.unwrap();let marker=s.sync_cursor().await.unwrap();let mut shortcuts=NativeShortcutSettings::default();shortcuts.bindings.get_mut("editor.bold").unwrap()[0].alt=true;
    assert!(s.save_project_data(&p.id,"settings",json!({"schemaVersion":1,"nativeShortcuts":shortcuts}),None).await.unwrap_err().contains("AltGr"));assert_eq!(s.sync_cursor().await.unwrap(),marker);assert!(s.load_project_data(&p.id,"settings").await.unwrap().is_none());
}
