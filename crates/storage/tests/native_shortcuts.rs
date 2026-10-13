use novel_storage::{Storage,native_shortcuts::{NativeShortcutSettings,NativeShortcutBinding,parse_native_shortcut_list}};
use serde_json::json;
#[test]
fn shortcut_defaults_and_text_roundtrip_are_exact(){
    let defaults=NativeShortcutSettings::default();defaults.validate().unwrap();
    for bindings in defaults.bindings.values(){let text=bindings.iter().map(NativeShortcutBinding::display).collect::<Vec<_>>().join(", ");assert_eq!(parse_native_shortcut_list(&text).unwrap(),*bindings);}
    assert_eq!(parse_native_shortcut_list("  control+shift+s, f8 ").unwrap().iter().map(NativeShortcutBinding::display).collect::<Vec<_>>(),vec!["Ctrl+Shift+S","F8"]);assert!(parse_native_shortcut_list(" ").unwrap().is_empty());
    for invalid in ["a","Shift+S","4","Ctrl+Alt+Q","Meta+S","Ctrl+Ctrl+S","Ctrl+Control+S","Alt+Tab","Tab","Ctrl+Escape","Enter","Ctrl+Backspace","F1","F6","F7","Alt+F4","F25","F08","Ctrl+S,Ctrl+T,Ctrl+W,F8","Ctrl+"]{assert!(parse_native_shortcut_list(invalid).is_err(),"{invalid}");}
}
#[test]
fn shortcuts_reject_duplicates_across_scopes_and_unknown_or_missing_actions(){
    let defaults=NativeShortcutSettings::default();let mut same=defaults.clone();same.bindings.insert("editor.bold".into(),same.bindings["workspace.save"].clone());assert!(same.validate().unwrap_err().contains("more than once"));
    let mut same=defaults.clone();let duplicate=same.bindings["editor.bold"][0].clone();same.bindings.get_mut("editor.bold").unwrap().push(duplicate);assert!(same.validate().is_err());
    let mut unknown=defaults.clone();unknown.bindings.insert("workspace.remoteCommand".into(),vec![]);assert!(unknown.validate().is_err());let mut absent=defaults.clone();absent.bindings.remove("editor.bold");assert!(absent.validate().is_err());
    let mut disabled=defaults.clone();disabled.bindings.insert("editor.bold".into(),vec![]);disabled.validate().unwrap();let chord=defaults.bindings["editor.bold"][0].clone();assert_eq!(defaults.action("editor.",&chord),Some("editor.bold"));assert_eq!(defaults.action("workspace.",&chord),None);assert_eq!(disabled.action("editor.",&chord),None);
    let mut fields=serde_json::to_value(defaults).unwrap();fields["unknown"]=json!(true);assert!(NativeShortcutSettings::from_project_settings(&json!({"nativeShortcuts":fields})).is_err());
}
#[test]
fn added_navigation_and_fullscreen_actions_preserve_schema_one_preferences(){
    let mut legacy=NativeShortcutSettings::default();for id in ["workspace.back","workspace.forward","workspace.fullscreen"]{legacy.bindings.remove(id);}legacy.bindings.insert("editor.bold".into(),vec![]);let original=serde_json::to_value(&legacy).unwrap();
    let normalized=NativeShortcutSettings::from_project_settings(&json!({"nativeShortcuts":original})).unwrap();assert!(normalized.bindings["editor.bold"].is_empty());assert_eq!(normalized.bindings["workspace.back"],parse_native_shortcut_list("Alt+ArrowLeft").unwrap());assert_eq!(normalized.bindings["workspace.forward"],parse_native_shortcut_list("Alt+Right").unwrap());assert_eq!(normalized.bindings["workspace.fullscreen"],parse_native_shortcut_list("F11").unwrap());assert_eq!(serde_json::to_value(&legacy).unwrap(),original,"Reading adds defaults without mutating saved preferences");
    legacy.bindings.insert("workspace.back".into(),vec![]);let normalized=NativeShortcutSettings::from_project_settings(&json!({"nativeShortcuts":legacy})).unwrap();assert!(normalized.bindings["workspace.back"].is_empty(),"Explicitly disabled actions remain disabled");
    let mut missing=normalized.clone();missing.bindings.remove("workspace.save");assert!(NativeShortcutSettings::from_project_settings(&json!({"nativeShortcuts":missing})).is_err());
    let mut unknown=normalized.clone();unknown.bindings.insert("workspace.sequence1".into(),vec![]);assert!(NativeShortcutSettings::from_project_settings(&json!({"nativeShortcuts":unknown})).is_err());
    let mut remapped=normalized;remapped.bindings.remove("workspace.back");remapped.bindings.insert("editor.bold".into(),parse_native_shortcut_list("Alt+Left").unwrap());let projected=NativeShortcutSettings::from_project_settings(&json!({"nativeShortcuts":remapped})).unwrap();assert!(projected.bindings["workspace.back"].is_empty());assert_eq!(projected.bindings["editor.bold"],parse_native_shortcut_list("Alt+ArrowLeft").unwrap());
}
#[test]
fn navigation_chords_protect_editing_and_fullscreen_key_is_action_scoped(){
    for invalid in ["Left","ArrowRight","Shift+Left","Ctrl+ArrowLeft","Ctrl+Shift+ArrowRight","Ctrl+Alt+Left","Alt+Up","Ctrl+F11","Shift+F11"]{assert!(parse_native_shortcut_list(invalid).is_err(),"{invalid}");}
    let default=NativeShortcutSettings::default();assert_eq!(default.action("workspace.",&parse_native_shortcut_list("Alt+Left").unwrap()[0]),Some("workspace.back"));assert_eq!(default.action("workspace.",&parse_native_shortcut_list("Alt+Right").unwrap()[0]),Some("workspace.forward"));
    let mut reserved=default.clone();reserved.bindings.insert("workspace.fullscreen".into(),vec![]);reserved.bindings.insert("workspace.save".into(),parse_native_shortcut_list("F11").unwrap());assert!(reserved.validate().unwrap_err().contains("reserved for the fullscreen"));
    let mut remapped=default;remapped.bindings.insert("workspace.fullscreen".into(),parse_native_shortcut_list("F8").unwrap());remapped.validate().unwrap();assert_eq!(remapped.action("workspace.",&parse_native_shortcut_list("F8").unwrap()[0]),Some("workspace.fullscreen"));assert!(remapped.action("workspace.",&parse_native_shortcut_list("F11").unwrap()[0]).is_none());
}
#[tokio::test]
async fn shortcut_settings_preserve_other_configuration_and_refuse_stale_concurrent_forms(){
    let d=tempfile::tempdir().unwrap();let path=d.path().join("shortcuts.sqlite");let s=Storage::open(&path).await.unwrap();let p=s.create_project("Shortcut fixture مِ 中文").await.unwrap();
    let prior=json!({"schemaVersion":1,"otherPreference":{"text":"مِ 中文 𠀀 👩🏽‍🚀"}});let saved=s.save_project_data(&p.id,"settings",prior.clone(),None).await.unwrap();
    let mut changed=NativeShortcutSettings::default();changed.bindings.insert("editor.bold".into(),parse_native_shortcut_list("Ctrl+Shift+B").unwrap());changed.bindings.insert("editor.sequence3".into(),parse_native_shortcut_list("Ctrl+Shift+M").unwrap());let mut payload=prior.clone();payload["nativeShortcuts"]=serde_json::to_value(&changed).unwrap();
    let committed=s.save_project_data_checked(&p.id,"settings",payload.clone(),Some(saved.revision_id.clone()),&[]).await.unwrap();assert_eq!(committed.payload["otherPreference"],prior["otherPreference"]);assert_eq!(NativeShortcutSettings::from_project_settings(&committed.payload).unwrap(),changed);
    let marker=s.sync_cursor().await.unwrap();assert!(s.save_project_data_checked(&p.id,"settings",prior,Some(saved.revision_id),&[]).await.unwrap_err().contains("Conflict"));assert_eq!(s.sync_cursor().await.unwrap(),marker);
    s.close().await;let reopened=Storage::open(&path).await.unwrap();assert_eq!(reopened.load_project_data(&p.id,"settings").await.unwrap().unwrap().payload,payload);
}
#[tokio::test]
async fn invalid_shortcut_payload_cannot_commit_or_advance_history(){
    let d=tempfile::tempdir().unwrap();let s=Storage::open(&d.path().join("invalid.sqlite")).await.unwrap();let p=s.create_project("Shortcuts").await.unwrap();let marker=s.sync_cursor().await.unwrap();let mut shortcuts=NativeShortcutSettings::default();shortcuts.bindings.get_mut("editor.bold").unwrap()[0].alt=true;
    assert!(s.save_project_data(&p.id,"settings",json!({"schemaVersion":1,"nativeShortcuts":shortcuts}),None).await.unwrap_err().contains("AltGr"));assert_eq!(s.sync_cursor().await.unwrap(),marker);assert!(s.load_project_data(&p.id,"settings").await.unwrap().is_none());
}

#[test]
fn sequence_slots_are_optional_disabled_and_preserve_explicit_profile_choices(){
    let defaults=NativeShortcutSettings::default();
    for slot in 1..=8{assert!(defaults.bindings[&format!("editor.sequence{slot}")].is_empty());}
    let mut legacy=defaults.clone();for slot in 1..=8{legacy.bindings.remove(&format!("editor.sequence{slot}"));}
    legacy.bindings.insert("editor.bold".into(),vec![]);let captured=serde_json::to_value(&legacy).unwrap();
    let normalized=NativeShortcutSettings::from_project_settings(&json!({"nativeShortcuts":captured})).unwrap();
    assert!(normalized.bindings["editor.bold"].is_empty());
    for slot in 1..=8{assert!(normalized.bindings[&format!("editor.sequence{slot}")].is_empty());}
    assert_eq!(serde_json::to_value(&legacy).unwrap(),captured,"Normalization must not rewrite stored profiles");
    legacy.bindings.insert("editor.sequence1".into(),vec![]);
    legacy.bindings.insert("editor.sequence3".into(),parse_native_shortcut_list("Ctrl+Shift+M").unwrap());
    let normalized=NativeShortcutSettings::from_project_settings(&json!({"nativeShortcuts":legacy})).unwrap();
    assert!(normalized.bindings["editor.sequence1"].is_empty());
    assert_eq!(normalized.action("editor.",&parse_native_shortcut_list("Ctrl+Shift+M").unwrap()[0]),Some("editor.sequence3"));
    assert!(normalized.action("workspace.",&parse_native_shortcut_list("Ctrl+Shift+M").unwrap()[0]).is_none());
    let mut invalid=normalized.clone();invalid.bindings.remove("editor.paste");assert!(invalid.validate().is_err());
    for action in ["editor.sequence0","editor.sequence9","editor.sequence01","workspace.sequence1"]{let mut invalid=normalized.clone();invalid.bindings.insert(action.into(),vec![]);assert!(invalid.validate().is_err(),"{action}");}
}
#[test]
fn sequence_chords_keep_duplicate_and_reserved_input_protections(){
    let mut profile=NativeShortcutSettings::default();profile.bindings.insert("editor.sequence1".into(),parse_native_shortcut_list("F8").unwrap());profile.validate().unwrap();
    let mut duplicate=profile.clone();duplicate.bindings.insert("editor.sequence2".into(),parse_native_shortcut_list("F8").unwrap());assert!(duplicate.validate().unwrap_err().contains("more than once"));
    let mut duplicate=profile.clone();duplicate.bindings.insert("editor.sequence2".into(),profile.bindings["workspace.save"].clone());assert!(duplicate.validate().unwrap_err().contains("more than once"));
    for key in ["F6","F7","Ctrl+Alt+M","M","Shift+M","ArrowLeft","Ctrl+ArrowRight"]{assert!(parse_native_shortcut_list(key).is_err(),"{key}");}
    profile.bindings.insert("editor.sequence1".into(),parse_native_shortcut_list("F11").unwrap());assert!(profile.validate().unwrap_err().contains("reserved for the fullscreen"));
}
