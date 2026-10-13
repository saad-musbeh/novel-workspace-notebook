use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous}, SqlitePool};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scene { pub schema_version: u32, pub title: String, pub document: Value }
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot { pub revision_id: String, pub scene: Scene }
pub struct Storage { pool: SqlitePool }

fn validate_node(node: &Value, depth: usize) -> Result<(), String> {
    if depth > 8 { return Err("Unsupported document depth.".into()); }
    let kind = node.get("type").and_then(Value::as_str).ok_or("Invalid document node.")?;
    if !["doc", "paragraph", "heading", "text", "hardBreak"].contains(&kind) { return Err("Unsupported document content.".into()); }
    if kind == "text" && !node.get("text").is_some_and(Value::is_string) { return Err("Invalid text.".into()); }
    if let Some(attrs) = node.get("attrs") {
        if let Some(spacing) = attrs.get("lineSpacing") {
            if !["paragraph","heading"].contains(&kind) || !spacing.as_f64().is_some_and(|value| [1.0,1.15,1.5,2.0].contains(&value)) { return Err("Invalid paragraph line spacing.".into()); }
        }
        if let Some(direction) = attrs.get("dir") {
            if ![Some("auto"), Some("ltr"), Some("rtl")].contains(&direction.as_str()) { return Err("Invalid paragraph direction.".into()); }
        }
        if let Some(language) = attrs.get("lang") {
            if ![Some("en"), Some("ar"), Some("zh-Hans"), Some("zh-Hant"), Some("und")].contains(&language.as_str()) { return Err("Unsupported paragraph language.".into()); }
        }
    }
    if let Some(marks) = node.get("marks") {
        for mark in marks.as_array().ok_or("Invalid formatting.")? {
            if ![Some("bold"), Some("italic"), Some("underline")].contains(&mark.get("type").and_then(Value::as_str)) { return Err("Unsupported formatting.".into()); }
        }
    }
    if let Some(children) = node.get("content") {
        for child in children.as_array().ok_or("Invalid document content.")? { validate_node(child, depth + 1)?; }
    }
    Ok(())
}
pub fn validate(scene: &Scene) -> Result<(), String> {
    if scene.schema_version != 1 { return Err("Unsupported document version. Stored data was preserved.".into()); }
    if scene.title.chars().count() > 500 || scene.document.get("type").and_then(Value::as_str) != Some("doc") { return Err("Invalid scene.".into()); }
    if serde_json::to_vec(scene).map_err(|_| "Invalid scene.")?.len() > 2_000_000 { return Err("Scene exceeds the prototype's 2 MB limit.".into()); }
    validate_node(&scene.document, 0)
}
mod recovery;
pub mod native_pack_versions;
pub mod native_sequences;
#[cfg(test)] mod release_tests;
mod workspace;
mod scene_conflict_snapshot;
pub use scene_conflict_snapshot::*;
mod review_sources;
pub use review_sources::*;
mod planning_read;
pub use planning_read::*;
mod manuscript_read;
mod alphabet_export;
pub use alphabet_export::*;
mod scene_arrangement;
pub mod native_shortcuts;
pub mod native_desk;
pub mod planning_validation;
pub use scene_arrangement::*;
mod export_profile;
pub use export_profile::{ExportProfile, ExportStyle};
mod sync;
mod time_machine;
mod branches;
mod paragraph_alternatives;
pub mod notebook_tools;
mod translations;
mod universe;
mod universe_read;
pub use universe_read::*;
pub use universe::*;
pub use branches::*;
pub use workspace::*;
pub use sync::*;
pub use translations::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn scene() -> Scene {
        Scene { schema_version: 1, title: "الفجر 晨光".into(), document: json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"مَرْحَبًا 世界 𠀀 \u{e000}","marks":[{"type":"bold"}]}]}]}) }
    }
    #[tokio::test]
    async fn underline_marks_persist_exactly_and_unknown_formatting_is_rejected() {
        let directory=tempfile::tempdir().unwrap();let path=directory.path().join("underline.sqlite");let store=Storage::open(&path).await.unwrap();
        let mut draft=scene();draft.document["content"][0]["content"][0]["marks"]=json!([{"type":"bold"},{"type":"italic"},{"type":"underline"}]);
        let saved=store.save(draft.clone(),None).await.unwrap();assert_eq!(saved.scene,draft);
        let mut invalid=draft;invalid.document["content"][0]["content"][0]["marks"]=json!([{"type":"strikethrough"}]);
        assert!(store.save(invalid,Some(saved.revision_id.clone())).await.unwrap_err().contains("Unsupported formatting"));
        assert_eq!(store.load().await.unwrap(),Some(saved.clone()));store.close().await;
        let reopened=Storage::open(&path).await.unwrap();assert_eq!(reopened.load().await.unwrap(),Some(saved));reopened.close().await;
    }
    #[tokio::test]
    async fn line_spacing_is_bounded_and_persists_without_changing_legacy_documents() {
        let directory=tempfile::tempdir().unwrap();let path=directory.path().join("spacing.sqlite");
        let store=Storage::open(&path).await.unwrap();
        for value in [json!(true),json!("1"),json!(0),json!(1.2),Value::Null] {
            let mut invalid=scene();invalid.document["content"][0]["attrs"]["lineSpacing"]=value;
            assert!(store.save(invalid,None).await.unwrap_err().contains("line spacing"));
        }
        let mut invalid=scene();invalid.document["content"][0]["content"][0]["attrs"]=json!({"lineSpacing":1});
        assert!(store.save(invalid,None).await.is_err());
        let original=store.save(scene(),None).await.unwrap();
        assert!(original.scene.document["content"][0]["attrs"].get("lineSpacing").is_none());
        let mut parent=original.revision_id;
        for value in [1.0,1.15,1.5,2.0] {
            let mut changed=scene();changed.document["content"][0]["attrs"]["lineSpacing"]=json!(value);
            let saved=store.save(changed.clone(),Some(parent)).await.unwrap();assert_eq!(saved.scene,changed);parent=saved.revision_id;
        }
        let saved=store.load().await.unwrap();store.close().await;
        let reopened=Storage::open(&path).await.unwrap();assert_eq!(reopened.load().await.unwrap(),saved);reopened.close().await;
    }
    #[tokio::test]
    async fn preserves_multilingual_revision_across_reopen() {
        let directory = tempfile::tempdir().unwrap(); let path = directory.path().join("scene.db");
        let store = Storage::open(&path).await.unwrap();
        assert_eq!(store.load().await.unwrap(), None);
        let saved = store.save(scene(), None).await.unwrap(); store.close().await;
        let reopened = Storage::open(&path).await.unwrap();
        assert_eq!(reopened.load().await.unwrap(), Some(saved));
    }
    #[tokio::test]
    async fn stale_writer_is_rejected_and_history_is_preserved() {
        let directory = tempfile::tempdir().unwrap(); let path = directory.path().join("scene.db");
        let first = Storage::open(&path).await.unwrap(); let second = Storage::open(&path).await.unwrap();
        let original = first.save(scene(), None).await.unwrap();
        assert!(second.save(scene(), None).await.unwrap_err().starts_with("Conflict"));
        assert_eq!(first.load().await.unwrap(), Some(Snapshot { revision_id: original.revision_id.clone(), scene: scene() }));
        let mut changed = scene(); changed.title = "Second draft".into();
        let latest = second.save(changed, Some(original.revision_id)).await.unwrap();
        assert_eq!(first.load().await.unwrap(), Some(latest));
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM scene_revision").fetch_one(&first.pool).await.unwrap();
        assert_eq!(count, 2);
    }
    #[tokio::test]
    async fn failed_head_update_rolls_back_the_new_revision() {
        let directory = tempfile::tempdir().unwrap(); let path = directory.path().join("scene.db");
        let store = Storage::open(&path).await.unwrap();
        let original = store.save(scene(), None).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER reject_head BEFORE UPDATE ON scene_head BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").execute(&store.pool).await.unwrap();
        assert!(store.save(scene(), Some(original.revision_id.clone())).await.is_err());
        assert_eq!(store.load().await.unwrap(), Some(original));
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM scene_revision").fetch_one(&store.pool).await.unwrap();
        assert_eq!(count, 1);
    }
    #[tokio::test]
    async fn rejects_unknown_schema_and_remote_nodes() {
        let directory = tempfile::tempdir().unwrap(); let path = directory.path().join("scene.db");
        let store = Storage::open(&path).await.unwrap(); let mut invalid = scene(); invalid.schema_version = 2;
        assert!(store.save(invalid, None).await.is_err());
        let mut invalid = scene(); invalid.document = json!({"type":"doc","content":[{"type":"image","attrs":{"src":"https://example.com"}}]});
        assert!(store.save(invalid, None).await.is_err()); assert_eq!(store.load().await.unwrap(), None);
        sqlx::query("PRAGMA user_version=4").execute(&store.pool).await.unwrap(); store.close().await;
        assert!(Storage::open(&path).await.is_err());
    }
    #[tokio::test]
    async fn immutable_revision_and_marker_fault_roll_back_all_new_state() {
        let directory=tempfile::tempdir().unwrap(); let store=Storage::open(&directory.path().join("fault.sqlite")).await.unwrap();
        let saved=store.save(scene(),None).await.unwrap();
        assert!(sqlx::query("UPDATE revision_envelope SET payload='{}' WHERE id=?").bind(&saved.revision_id).execute(&store.pool).await.is_err());
        assert!(sqlx::query("DELETE FROM revision_envelope WHERE id=?").bind(&saved.revision_id).execute(&store.pool).await.is_err());
        assert!(sqlx::query("DELETE FROM change_marker").execute(&store.pool).await.is_err());
        sqlx::raw_sql("CREATE TRIGGER fail_marker BEFORE INSERT ON change_marker BEGIN SELECT RAISE(ABORT,'injected marker failure'); END;").execute(&store.pool).await.unwrap();
        assert!(store.save(scene(),Some(saved.revision_id.clone())).await.is_err());
        assert_eq!(store.load().await.unwrap(),Some(saved));
        let envelopes:i64=sqlx::query_scalar("SELECT count(*) FROM revision_envelope").fetch_one(&store.pool).await.unwrap();
        let markers:i64=sqlx::query_scalar("SELECT count(*) FROM change_marker").fetch_one(&store.pool).await.unwrap();
        assert_eq!(envelopes,1); assert_eq!(markers,1);
    }
    #[tokio::test]
    async fn import_transaction_failure_leaves_no_partial_project() {
        let directory=tempfile::tempdir().unwrap(); let store=Storage::open(&directory.path().join("fault.sqlite")).await.unwrap();
        store.save(scene(),None).await.unwrap();let archive=store.export_archive("prototype-project").await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER fail_scene BEFORE INSERT ON manuscript_scene BEGIN SELECT RAISE(ABORT,'injected import failure'); END;").execute(&store.pool).await.unwrap();
        assert!(store.import_archive(&archive).await.is_err());assert_eq!(store.list_projects().await.unwrap().len(),1);
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM revision_envelope").fetch_one(&store.pool).await.unwrap();assert_eq!(count,1);
    }

}
