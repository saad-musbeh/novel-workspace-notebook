use novel_storage::{native_sequences::*, Storage};
use serde_json::{json, Value};

fn settings(actions: Value) -> Value {
    json!({"schemaVersion":1,"nativeCommandSequences":{"schemaVersion":1,"sequences":[{"slot":1,"name":"مِ 中文 👩🏽‍🚀","actions":actions}]}})
}
fn bold() -> Value {
    json!({"kind":"format","command":"bold"})
}

#[test]
fn strict_schema_accepts_only_retained_formatting_commands() {
    let actions = json!([
        {"kind":"format","command":"bold"},{"kind":"format","command":"italic"},
        {"kind":"format","command":"underline"},{"kind":"format","command":"clearFormatting"},
        {"kind":"format","command":"ltr"},{"kind":"format","command":"rtl"},
        {"kind":"spacing","value":1},{"kind":"spacing","value":1.15},
        {"kind":"spacing","value":1.5},{"kind":"spacing","value":2},
        {"kind":"paragraph","style":"heading","language":"ar","direction":"rtl"},
        {"kind":"paragraph","style":"paragraph","language":"zh-Hant","direction":"auto"}
    ]);
    let parsed = NativeSequenceSettings::from_project_settings(&settings(actions)).unwrap();
    assert_eq!(parsed.sequences[0].actions.len(), 12);
    for command in [
        "text",
        "copy",
        "cut",
        "paste",
        "delete",
        "undo",
        "redo",
        "selectAll",
        "unicodeToggle",
        "ArrowLeft",
        "navigate",
    ] {
        assert!(
            NativeSequenceSettings::from_project_settings(&settings(
                json!([{"kind":"format","command":command}])
            ))
            .is_err(),
            "{command}"
        );
    }
    for action in [
        json!({"kind":"text","text":"private"}),
        json!({"kind":"format","command":"bold","text":"private"}),
        json!({"kind":"spacing","value":1.2}),
        json!({"kind":"spacing","value":"1"}),
        json!({"kind":"paragraph","style":"quote","language":"ar","direction":"rtl"}),
        json!({"kind":"paragraph","style":"heading","language":"xx","direction":"rtl"}),
        json!({"kind":"paragraph","style":"heading","language":"ar","direction":"vertical"}),
        json!({"kind":"paragraph","style":"heading","language":"ar"}),
    ] {
        assert!(
            NativeSequenceSettings::from_project_settings(&settings(json!([action]))).is_err(),
            "{action}"
        );
    }
    for value in [f64::NAN, f64::INFINITY, 0., -1., 3.] {
        assert!(NativeSequenceAction::Spacing { value }.validate().is_err());
    }
}
#[test]
fn eight_slots_and_thirty_two_commands_are_hard_bounds() {
    let mut payload = settings(json!([bold()]));
    assert!(NativeSequenceSettings::from_project_settings(&json!({}))
        .unwrap()
        .sequences
        .is_empty());
    for count in [0, 33] {
        payload["nativeCommandSequences"]["sequences"][0]["actions"] = json!(vec![bold(); count]);
        assert!(NativeSequenceSettings::from_project_settings(&payload).is_err());
    }
    payload["nativeCommandSequences"]["sequences"][0]["actions"] = json!(vec![bold(); 32]);
    NativeSequenceSettings::from_project_settings(&payload).unwrap();
    let template = payload["nativeCommandSequences"]["sequences"][0].clone();
    payload["nativeCommandSequences"]["sequences"] = json!((1..=8)
        .map(|slot| {
            let mut s = template.clone();
            s["slot"] = json!(slot);
            s
        })
        .collect::<Vec<_>>());
    NativeSequenceSettings::from_project_settings(&payload).unwrap();
    for slot in [0, 9, 1] {
        let mut bad = payload.clone();
        bad["nativeCommandSequences"]["sequences"][7]["slot"] = json!(slot);
        assert!(NativeSequenceSettings::from_project_settings(&bad).is_err());
    }
    for name in ["   ".to_owned(), "x".repeat(81), "𠀀".repeat(41)] {
        let mut bad = payload.clone();
        bad["nativeCommandSequences"]["sequences"][0]["name"] = json!(name);
        assert!(NativeSequenceSettings::from_project_settings(&bad).is_err());
    }
    for path in ["settings", "sequence"] {
        let mut bad = payload.clone();
        if path == "settings" {
            bad["nativeCommandSequences"]["extra"] = json!(true);
        } else {
            bad["nativeCommandSequences"]["sequences"][0]["extra"] = json!(true);
        }
        assert!(NativeSequenceSettings::from_project_settings(&bad).is_err());
    }
    payload["nativeCommandSequences"]["schemaVersion"] = json!(2);
    assert!(NativeSequenceSettings::from_project_settings(&payload).is_err());
}
#[tokio::test]
async fn settings_cas_preserves_unrelated_data_and_history_on_stale_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sequences.sqlite");
    let storage = Storage::open(&path).await.unwrap();
    let project = storage.create_project("Synthetic مِ 中文").await.unwrap();
    let prior = json!({"schemaVersion":1,"unrelated":{"text":"مِ 中文 𠀀 👩🏽‍🚀"}});
    let first = storage
        .save_project_data_checked(&project.id, "settings", prior.clone(), None, &[])
        .await
        .unwrap();
    let parsed = NativeSequenceSettings::from_project_settings(&settings(json!([bold()]))).unwrap();
    let payload = parsed.merge_into(&prior).unwrap();
    assert_eq!(payload["unrelated"], prior["unrelated"]);
    let saved = storage
        .save_project_data_checked(
            &project.id,
            "settings",
            payload.clone(),
            Some(first.revision_id.clone()),
            &[],
        )
        .await
        .unwrap();
    let cursor = storage.sync_cursor().await.unwrap();
    assert!(storage
        .save_project_data_checked(&project.id, "settings", prior, Some(first.revision_id), &[])
        .await
        .unwrap_err()
        .contains("Conflict"));
    assert_eq!(storage.sync_cursor().await.unwrap(), cursor);
    assert_eq!(
        storage
            .load_project_data(&project.id, "settings")
            .await
            .unwrap()
            .unwrap()
            .revision_id,
        saved.revision_id
    );
    storage.close().await;
    let storage = Storage::open(&path).await.unwrap();
    assert_eq!(
        storage
            .load_project_data(&project.id, "settings")
            .await
            .unwrap()
            .unwrap()
            .payload,
        payload
    );
}
#[tokio::test]
async fn invalid_sequences_cannot_advance_storage_history() {
    // Requires the documented root validator hook, deliberately exercising the
    // real backend rather than validating just before this test's save call.
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::open(&dir.path().join("invalid.sqlite"))
        .await
        .unwrap();
    let project = storage.create_project("Synthetic").await.unwrap();
    let cursor = storage.sync_cursor().await.unwrap();
    assert!(storage
        .save_project_data_checked(
            &project.id,
            "settings",
            settings(json!([{"kind":"format","command":"undo"}])),
            None,
            &[]
        )
        .await
        .is_err());
    assert_eq!(storage.sync_cursor().await.unwrap(), cursor);
    assert!(storage
        .load_project_data(&project.id, "settings")
        .await
        .unwrap()
        .is_none());
}
