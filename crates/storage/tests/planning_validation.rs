// The validator is deliberately tested before UI/backend wiring as well as by
// transaction tests after integration. It never mutates the supplied values.
#[path = "../src/planning_validation.rs"]
mod planning_validation;
use planning_validation::{validate_compatibility, validate_resolution, validate_transition};
use serde_json::{json, Value};

fn fixture() -> Value {
    json!({"schemaVersion":1,"records":[{"id":"hero","kind":"character","name":"نور 明"}],"facts":[{"id":"fact","subjectId":"hero","property":"Name","value":"مِ 中文 \u{e000} 👩🏽‍🚀","status":"fact","confirmed":true}],"events":[{"id":"event","time":{"kind":"unknown"}},{"id":"end","time":{"kind":"unknown"}}],"readerReveals":[{"id":"reveal","eventId":null,"factId":"fact","scene":{"kind":"scene","id":"scene"},"position":0,"notes":"Authored reveal"}],"knowledge":[{"id":"knowledge","characterId":"hero","factId":"fact","stance":"knows","factSnapshot":{"subjectId":"hero","property":"Name","value":"مِ 中文 \u{e000} 👩🏽‍🚀"},"source":"Authored source","interpretation":"","scope":{"kind":"scenes","sceneIds":["scene"]}}]})
}
#[test] fn legacy_optional_array_absence_and_partial_historical_entries_are_preserved() {
    let legacy = json!({"schemaVersion":1,"records":[],"facts":[]}); let original = legacy.clone(); validate_compatibility(&legacy).unwrap(); validate_transition(None, &legacy).unwrap(); assert_eq!(legacy, original);
    let partial = json!({"knowledge":[{"id":"old","scope":{"kind":"scenes","sceneIds":["scene"]},"factSnapshot":{"subjectId":"hero","value":"exact مِ 中文"}}]}); let original = partial.clone(); validate_compatibility(&partial).unwrap(); let mut edited = partial.clone(); edited["notes"] = json!("Unrelated notes"); validate_transition(Some(&partial), &edited).unwrap(); assert_eq!(partial, original); assert!(validate_transition(None, &partial).is_err());
}
#[test] fn newly_authored_valid_knowledge_freezes_exact_fact_and_unknown_event_times_are_allowed() {
    let doc = fixture(); let original = doc.clone(); validate_transition(None, &doc).unwrap(); assert_eq!(doc, original);
    let mut range = doc; range["knowledge"][0]["scope"] = json!({"kind":"events","fromEventId":"event","untilEventId":"end"}); validate_transition(None, &range).unwrap();
}
#[test] fn canon_change_then_unrelated_edits_keep_legitimate_stale_snapshots_exactly() {
    let approved = fixture(); let frozen = approved["knowledge"][0].clone(); let mut changed = approved.clone(); changed["facts"][0]["value"] = json!("New canon"); changed["facts"][0]["confirmed"] = json!(false); changed["facts"][0]["status"] = json!("proposal"); validate_transition(Some(&approved), &changed).unwrap(); validate_compatibility(&changed).unwrap();
    let mut unrelated = changed.clone(); unrelated["records"][0]["notes"] = json!("Unrelated exact notes مِ 中文"); validate_transition(Some(&changed), &unrelated).unwrap(); assert_eq!(unrelated["knowledge"][0], frozen); assert!(validate_transition(None, &changed).is_err());
}
#[test] fn new_or_reauthored_claims_require_current_approved_fact_source_and_scope() {
    for field in ["proposal", "source", "snapshot", "character", "scope", "stance", "interpretation"] {
        let mut doc = fixture(); match field {
            "proposal" => doc["facts"][0]["confirmed"] = json!(false),
            "source" => doc["knowledge"][0]["source"] = json!("  "),
            "snapshot" => doc["knowledge"][0]["factSnapshot"]["value"] = json!("Invented"),
            "character" => doc["knowledge"][0]["characterId"] = json!("missing"),
            "scope" => doc["knowledge"][0]["scope"]["sceneIds"] = json!([]),
            "stance" => doc["knowledge"][0]["stance"] = json!("guessed"),
            _ => { doc["knowledge"][0]["stance"] = json!("believes"); doc["knowledge"][0]["interpretation"] = json!(""); },
        } assert!(validate_transition(None, &doc).is_err(), "{field}");
    }
    let approved = fixture(); let mut stale = approved.clone(); stale["facts"][0]["value"] = json!("New canon"); validate_transition(Some(&approved), &stale).unwrap(); let mut reauthored = stale.clone(); reauthored["knowledge"][0]["source"] = json!("Reauthored source"); assert!(validate_transition(Some(&stale), &reauthored).is_err()); reauthored["knowledge"][0]["factSnapshot"]["value"] = json!("New canon"); validate_transition(Some(&stale), &reauthored).unwrap();
}
#[test] fn beliefs_do_not_approve_proposals_and_require_their_authored_interpretation() {
    let mut doc = fixture(); doc["facts"][0]["status"] = json!("proposal"); doc["facts"][0]["confirmed"] = json!(false); doc["knowledge"][0]["stance"] = json!("believes"); doc["knowledge"][0]["interpretation"] = json!("An authored mistaken reading"); validate_transition(None, &doc).unwrap(); assert_eq!(doc["facts"][0]["confirmed"], false);
}
#[test] fn reader_reveal_identity_target_position_and_optional_collection_shape_are_checked() {
    for mode in ["both", "missing-target", "negative", "fraction", "unsafe", "duplicate", "array"] {
        let mut doc = fixture(); match mode {
            "both" => doc["readerReveals"][0]["eventId"] = json!("event"),
            "missing-target" => doc["readerReveals"][0]["factId"] = json!("missing"),
            "negative" => doc["readerReveals"][0]["position"] = json!(-1),
            "fraction" => doc["readerReveals"][0]["position"] = json!(0.5),
            "unsafe" => doc["readerReveals"][0]["position"] = json!(9_007_199_254_740_992_u64),
            "duplicate" => doc["readerReveals"][0]["id"] = json!("knowledge"),
            _ => doc["knowledge"] = json!({}),
        } assert!(validate_transition(None, &doc).is_err(), "{mode}");
    }
}
#[test] fn unchanged_missing_historical_targets_are_readable_but_new_deletions_require_resolving_links() {
    let doc = fixture(); let mut deleted = doc.clone(); deleted["facts"] = json!([]); assert!(validate_transition(Some(&doc), &deleted).is_err());
    validate_compatibility(&deleted).unwrap(); let mut unrelated = deleted.clone(); unrelated["notes"] = json!("History remains exact"); validate_transition(Some(&deleted), &unrelated).unwrap();
    deleted["knowledge"] = json!([]); deleted["readerReveals"] = json!([]); validate_transition(Some(&doc), &deleted).unwrap();
}
#[test] fn acknowledged_conflict_heads_can_retain_historical_claims_without_reauthoring() {
    let original = fixture(); let mut withdrawn = original.clone(); withdrawn["facts"][0]["status"] = json!("proposal"); withdrawn["facts"][0]["confirmed"] = json!(false); validate_resolution(&[&original, &withdrawn], &withdrawn).unwrap();
    let mut invention = withdrawn.clone(); invention["knowledge"][0]["source"] = json!("New claim"); assert!(validate_resolution(&[&original, &withdrawn], &invention).is_err());
    let mut missing_historical = original.clone(); missing_historical["facts"] = json!([]);
    validate_resolution(&[&original, &missing_historical], &missing_historical).unwrap();
}

#[tokio::test]
async fn storage_commit_checks_author_claims_and_preserves_stale_history_archive_and_failed_writes() {
    use novel_storage::{Scene, Storage};
    let directory = tempfile::tempdir().unwrap(); let storage = Storage::open(&directory.path().join("planning.sqlite")).await.unwrap();
    let project = storage.create_project("Synthetic planning").await.unwrap(); let chapter = storage.create_chapter(&project.id, "Chapter").await.unwrap();
    let scene = storage.create_scene(&chapter.id, Scene { schema_version: 1, title: "Scene".into(), document: json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"مِ 中文"}]}]}) }).await.unwrap();
    let mut approved = fixture(); approved["readerReveals"][0]["scene"]["id"] = json!(scene.id); approved["knowledge"][0]["scope"]["sceneIds"] = json!([scene.id]);
    let baseline = storage.sync_cursor().await.unwrap(); let mut invalid = approved.clone(); invalid["knowledge"][0]["factSnapshot"]["value"] = json!("Invented claim");
    assert!(storage.save_project_data(&project.id, "planning", invalid, None).await.is_err());
    assert!(storage.load_project_data(&project.id, "planning").await.unwrap().is_none()); assert_eq!(storage.sync_cursor().await.unwrap(), baseline);
    let original = storage.save_project_data(&project.id, "planning", approved.clone(), None).await.unwrap();
    let mut changed = approved.clone(); changed["facts"][0]["value"] = json!("Changed canon"); changed["facts"][0]["status"] = json!("proposal"); changed["facts"][0]["confirmed"] = json!(false);
    let stale = storage.save_project_data(&project.id, "planning", changed.clone(), Some(original.revision_id.clone())).await.unwrap();
    let mut unrelated = changed.clone(); unrelated["records"][0]["notes"] = json!("Unrelated مِ 中文");
    let latest = storage.save_project_data(&project.id, "planning", unrelated.clone(), Some(stale.revision_id)).await.unwrap(); assert_eq!(latest.payload["knowledge"], approved["knowledge"]);
    let mut reauthored = unrelated.clone(); reauthored["knowledge"][0]["source"] = json!("New claim against withdrawn fact"); let before = storage.sync_cursor().await.unwrap();
    assert!(storage.save_project_data(&project.id, "planning", reauthored, Some(latest.revision_id.clone())).await.is_err()); assert_eq!(storage.sync_cursor().await.unwrap(), before); assert_eq!(storage.load_project_data(&project.id, "planning").await.unwrap().unwrap(), latest);
    assert!(storage.save_project_data(&project.id, "planning", approved.clone(), Some(original.revision_id)).await.unwrap_err().starts_with("Conflict"));
    let imported = storage.import_archive(&storage.export_archive(&project.id).await.unwrap()).await.unwrap();
    let copied = storage.load_project_data(&imported.project.id, "planning").await.unwrap().unwrap(); assert_eq!(copied.payload["knowledge"][0]["factSnapshot"], approved["knowledge"][0]["factSnapshot"]); assert_eq!(copied.payload["facts"], unrelated["facts"]); assert_ne!(copied.payload["knowledge"][0]["scope"]["sceneIds"][0], scene.id);
    storage.close().await;
}
