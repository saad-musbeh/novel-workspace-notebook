//! One read snapshot verifies every manuscript source of a review proposal.
//! This is read-only evidence; a later author decision still needs source CAS.
use crate::workspace::{err, hash, valid_id, validate_current_source};
use crate::{Scene, Storage};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, Transaction};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewSource {
    pub scene_id: String,
    pub revision_id: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewSourcesCheck {
    pub sources: Vec<ReviewSource>,
}

fn validate_request(pid: &str, sources: &[ReviewSource]) -> Result<(), String> {
    if !valid_id(pid) || sources.is_empty() || sources.len() > 50 {
        return Err("Review requires a project and 1–50 distinct saved scenes.".into());
    }
    let mut seen = HashSet::new();
    for source in sources {
        if !valid_id(&source.scene_id)
            || !valid_id(&source.revision_id)
            || !seen.insert(&source.scene_id)
            || source.hash.len() != 64
            || !source
                .hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(
                "Invalid or duplicate review source. An exact lowercase SHA-256 hash is required."
                    .into(),
            );
        }
    }
    Ok(())
}

async fn check_snapshot(
    tx: &mut Transaction<'_, Sqlite>,
    pid: &str,
    sources: &[ReviewSource],
) -> Result<(), String> {
    for source in sources {
        // Includes undeleted membership, projected head and combined scene /
        // tombstone leaves. An arbitrary retained revision is never current.
        validate_current_source(tx, pid, &source.scene_id, "scene", &source.revision_id).await?;
        let (payload, checksum, version): (String, String, i64) = sqlx::query_as(
            "SELECT payload,checksum,schema_version FROM revision_envelope WHERE id=? AND project_id=? AND entity_id=? AND entity_type='scene'",
        )
        .bind(&source.revision_id)
        .bind(pid)
        .bind(&source.scene_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(err)?
        .ok_or("Review source revision does not belong to this manuscript scene.")?;
        if version != 1 || hash(payload.as_bytes()) != checksum {
            return Err(
                "Review source checksum or schema failed. Stored writing was preserved.".into(),
            );
        }
        let scene: Scene =
            serde_json::from_str(&payload).map_err(|_| "Invalid saved review source scene.")?;
        crate::validate(&scene)?;
        if hash(crate::plain_text(&scene.document).as_bytes()) != source.hash {
            return Err(
                "Conflict: the reviewed source text hash does not match its saved document.".into(),
            );
        }
    }
    Ok(())
}

impl Storage {
    /// Echoes the exact ordered capture only when all sources were current in
    /// one transaction. No manuscript, metadata or marker is written.
    pub async fn review_sources_check(
        &self,
        pid: &str,
        sources: &[ReviewSource],
    ) -> Result<ReviewSourcesCheck, String> {
        validate_request(pid, sources)?;
        let mut tx = self.pool.begin().await.map_err(err)?;
        check_snapshot(&mut tx, pid, sources).await?;
        tx.commit().await.map_err(err)?;
        Ok(ReviewSourcesCheck {
            sources: sources.to_vec(),
        })
    }
}

#[cfg(test)]
#[path = "review_sources_tests.rs"]
mod tests;
