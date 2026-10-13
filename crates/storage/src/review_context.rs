//! One-scene, read-only consistency basis. Root exports this module and wires
//! review_context_snapshot(projectId,sceneId), review_context_check(projectId,
//! sources). Provider canonical_fact_text should delegate to the public shared
//! projection below; its bytes match the retained provider/TypeScript fixture.
//! No schema, planning validator, manuscript or canon write occurs here.
use crate::planning_read::checked_data;
use crate::workspace::{err, hash, valid_id, validate_current_source};
use crate::{ReviewSource, ReviewSourcesCheck, Scene, SceneRecord, Storage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, Sqlite, Transaction};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewContextSource {
    pub scene_id: String,
    pub revision_id: String,
    pub hash: String,
    pub kind: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExcludedReviewFact {
    pub id: String,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewContextScope {
    pub included_scene_id: String,
    pub excluded_scene_count: i64,
    pub included_fact_ids: Vec<String>,
    pub excluded_facts: Vec<ExcludedReviewFact>,
    pub timing_included: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewContextSnapshot {
    pub scene: SceneRecord,
    pub sources: Vec<ReviewContextSource>,
    pub scope: ReviewContextScope,
}
/// Stable ASCII keys/identities and sorted arrays, preserving ALL value/name
/// UTF-8 bytes. Only author-confirmed status=fact enters this projection. Beliefs,
/// rumors, proposals and unknown/unconfirmed rows remain outside canon.
pub fn canonical_review_fact_text(document: &Value) -> Result<String, String> {
    if document["schemaVersion"] != 1 {
        return Err("Unsupported story-fact snapshot.".into());
    }
    let input = document["facts"].as_array().ok_or("Missing saved facts.")?;
    let records = document["records"]
        .as_array()
        .ok_or("Missing saved fact subjects.")?;
    let mut facts = Vec::new();
    let mut seen = HashSet::new();
    let mut subjects = HashSet::new();
    for fact in input
        .iter()
        .filter(|fact| fact["status"] == "fact" && fact["confirmed"] == true)
    {
        let id = fact["id"]
            .as_str()
            .ok_or("Invalid confirmed fact identity.")?;
        let subject = fact["subjectId"]
            .as_str()
            .ok_or("Invalid confirmed fact subject.")?;
        for identity in [id, subject] {
            if identity.is_empty()
                || identity.len() > 200
                || !identity
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
            {
                return Err("Confirmed fact identities must be stable ASCII IDs.".into());
            }
        }
        if !seen.insert(id) {
            return Err("Duplicate confirmed fact identity.".into());
        }
        subjects.insert(subject);
        let mut entry = serde_json::Map::new();
        for key in [
            "id",
            "subjectId",
            "property",
            "value",
            "eventId",
            "untilEventId",
        ] {
            let value = fact.get(key).ok_or("Missing confirmed fact property.")?;
            if !value.is_string() && !(matches!(key, "eventId" | "untilEventId") && value.is_null())
            {
                return Err("Invalid confirmed fact property.".into());
            }
            entry.insert(key.into(), value.clone());
        }
        facts.push(Value::Object(entry));
    }
    if facts.len() > 1000 {
        return Err("Confirmed fact context exceeds its supported record limit.".into());
    }
    facts.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let mut names = Vec::new();
    let mut seen_names = HashSet::new();
    for record in records
        .iter()
        .filter(|r| r["id"].as_str().is_some_and(|id| subjects.contains(id)))
    {
        let id = record["id"]
            .as_str()
            .ok_or("Invalid fact subject identity.")?;
        let name = record["name"]
            .as_str()
            .ok_or("Invalid fact subject name.")?;
        if !seen_names.insert(id) {
            return Err("Duplicate fact subject identity.".into());
        }
        names.push(json!({"id":id,"name":name}));
    }
    names.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    fn ordered(value: &Value) -> Value {
        match value {
            Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort();
                let mut map = serde_json::Map::new();
                for key in keys {
                    map.insert(key.clone(), ordered(&object[key]));
                }
                Value::Object(map)
            }
            Value::Array(array) => Value::Array(array.iter().map(ordered).collect()),
            _ => value.clone(),
        }
    }
    let text = serde_json::to_string(&ordered(
        &json!({"schemaVersion":1,"records":names,"facts":facts}),
    ))
    .map_err(|_| "Invalid canonical fact snapshot.")?;
    if text.len() > 30000 {
        return Err("Confirmed facts exceed the 30 KB context limit.".into());
    }
    Ok(text)
}
async fn capture(
    tx: &mut Transaction<'_, Sqlite>,
    pid: &str,
    sid: &str,
) -> Result<ReviewContextSnapshot, String> {
    let row=sqlx::query("SELECT s.id,s.chapter_id,s.position,s.head,r.payload,r.checksum,r.schema_version FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id JOIN revision_envelope r ON r.id=s.head AND r.project_id=c.project_id AND r.entity_id=s.id AND r.entity_type='scene' WHERE c.project_id=? AND s.id=? AND s.deleted=0")
        .bind(pid).bind(sid).fetch_optional(&mut **tx).await.map_err(err)?.ok_or("Review scene is unavailable in this project.")?;
    let revision: String = row.get("head");
    validate_current_source(tx, pid, sid, "scene", &revision).await?;
    let raw: String = row.get("payload");
    let checksum: String = row.get("checksum");
    let version: i64 = row.get("schema_version");
    if version != 1 || hash(raw.as_bytes()) != checksum {
        return Err("Saved review scene checksum or schema failed.".into());
    }
    let scene: Scene = serde_json::from_str(&raw).map_err(|_| "Invalid saved review scene.")?;
    crate::validate(&scene)?;
    let text = crate::plain_text(&scene.document);
    if text.trim().is_empty() {
        return Err("Write and save a scene before consistency review.".into());
    }
    let record = SceneRecord {
        id: sid.into(),
        chapter_id: row.get("chapter_id"),
        position: row.get("position"),
        revision_id: revision.clone(),
        scene,
    };
    let planning=checked_data(tx,pid,"planning").await?.ok_or("Saved planning is unavailable. Save planning before consistency review; wording remains available.")?;
    validate_current_source(
        tx,
        pid,
        &format!("{pid}:planning"),
        "projectData",
        &planning.revision_id,
    )
    .await?;
    let version: i64 =
        sqlx::query_scalar("SELECT schema_version FROM revision_envelope WHERE id=?")
            .bind(&planning.revision_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(err)?;
    if version != 1 {
        return Err("Saved planning schema is unsupported.".into());
    }
    let facts_text = canonical_review_fact_text(&planning.payload)?;
    let projection: Value =
        serde_json::from_str(&facts_text).map_err(|_| "Invalid canonical fact context.")?;
    let facts = projection["facts"]
        .as_array()
        .ok_or("Invalid canonical facts.")?;
    // An empty projection still has a real saved planning head; final CAS can
    // guard later fact creation. Absence of planning is refused above.
    let subjects = projection["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["id"].as_str())
        .collect::<HashSet<_>>();
    if facts.iter().any(|f| {
        !f["subjectId"]
            .as_str()
            .is_some_and(|id| subjects.contains(id))
    }) {
        return Err("Confirmed fact subject is unavailable. Repair the authored planning records before review.".into());
    }
    if text.encode_utf16().count() + facts_text.encode_utf16().count() > 50000 {
        return Err("Complete scene and confirmed facts exceed the 50,000 UTF-16 unit review limit. No source was truncated; wording remains available.".into());
    }
    let included = facts
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let mut excluded = Vec::new();
    for fact in planning.payload["facts"].as_array().unwrap() {
        if fact["status"] == "fact" && fact["confirmed"] == true {
            continue;
        }
        let id = fact["id"]
            .as_str()
            .ok_or("Invalid excluded fact identity.")?;
        let reason = if fact["status"] == "fact" {
            "Unconfirmed"
        } else {
            "Belief, rumor, proposal or unknown; not canonical"
        };
        excluded.push(ExcludedReviewFact {
            id: id.into(),
            reason: reason.into(),
        });
    }
    excluded.sort_by(|a, b| a.id.cmp(&b.id));
    let others:i64=sqlx::query_scalar("SELECT count(*) FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=? AND s.deleted=0 AND s.id!=?").bind(pid).bind(sid).fetch_one(&mut **tx).await.map_err(err)?;
    Ok(ReviewContextSnapshot {
        scene: record,
        sources: vec![
            ReviewContextSource {
                scene_id: sid.into(),
                revision_id: revision,
                hash: hash(text.as_bytes()),
                kind: "scene".into(),
                text,
            },
            ReviewContextSource {
                scene_id: format!("planning:{pid}"),
                revision_id: planning.revision_id,
                hash: hash(facts_text.as_bytes()),
                kind: "facts".into(),
                text: facts_text,
            },
        ],
        scope: ReviewContextScope {
            included_scene_id: sid.into(),
            excluded_scene_count: others,
            included_fact_ids: included,
            excluded_facts: excluded,
            timing_included: false,
        },
    })
}
impl Storage {
    pub async fn review_context_snapshot(
        &self,
        pid: &str,
        sid: &str,
    ) -> Result<ReviewContextSnapshot, String> {
        if !valid_id(pid) || !valid_id(sid) {
            return Err("Consistency review needs exact project and scene identities.".into());
        }
        let mut tx = self.pool.begin().await.map_err(err)?;
        let snapshot = capture(&mut tx, pid, sid).await?;
        tx.commit().await.map_err(err)?;
        Ok(snapshot)
    }
    /// This slice always checks exactly one editable scene followed by one
    /// supporting planning projection. The full planning head is a dependency,
    /// even when an unrelated planning edit leaves the projection bytes equal.
    pub async fn review_context_check(
        &self,
        pid: &str,
        sources: &[ReviewSource],
    ) -> Result<ReviewSourcesCheck, String> {
        if !valid_id(pid)
            || sources.len() != 2
            || !valid_id(&sources[0].scene_id)
            || sources[1].scene_id != format!("planning:{pid}")
        {
            return Err("Consistency review requires one scene and its project's saved fact context in captured order.".into());
        }
        for source in sources {
            if !valid_id(&source.revision_id)
                || source.hash.len() != 64
                || !source
                    .hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(
                    "Review context requires exact lowercase SHA-256 hashes and saved revisions."
                        .into(),
                );
            }
        }
        let mut tx = self.pool.begin().await.map_err(err)?;
        let snapshot = capture(&mut tx, pid, &sources[0].scene_id).await?;
        for (actual, expected) in snapshot.sources.iter().zip(sources) {
            if actual.scene_id != expected.scene_id
                || actual.revision_id != expected.revision_id
                || actual.hash != expected.hash
            {
                return Err("Conflict: reviewed manuscript or confirmed fact context changed. No decision was accepted.".into());
            }
        }
        tx.commit().await.map_err(err)?;
        Ok(ReviewSourcesCheck {
            sources: sources.to_vec(),
        })
    }
}
#[cfg(test)]
#[path = "review_context_tests.rs"]
mod tests;
