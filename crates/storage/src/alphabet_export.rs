//! A saved manuscript and its immutable alphabet versions share one read snapshot.
use crate::planning_read::checked_data;
use crate::manuscript_read::{checked_size, read_project_manuscript};
use crate::workspace::{err, hash, validate_data, validate_data_transition};
use crate::{ProjectBundle, ProjectDataSnapshot, Storage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlphabetExportSnapshot {
    pub bundle: ProjectBundle,
    pub writing_systems: Option<ProjectDataSnapshot>,
}

impl Storage {
    /// Does not save drafts, create revisions, export assets or choose a conflict.
    pub async fn alphabet_export_snapshot(&self, pid: &str) -> Result<AlphabetExportSnapshot, String> {
        let mut tx = self.pool.begin().await.map_err(err)?;
        let bundle = read_project_manuscript(&mut tx, pid).await?;
        let writing_systems = checked_data(&mut tx, pid, "writingSystems").await?;
        // A saved export must not conceal an unresolved authored translation.
        checked_data(&mut tx, pid, "sceneTranslations").await?;
        if let Some(data) = &writing_systems {
            // Interpretation must agree with EVERY retained immutable version,
            // not merely with whichever payload is currently projected.
            let mut after: Option<i64> = None;
            let mut count = 0usize;
            let mut bytes = 0usize;
            loop {
                let retained: Option<(i64,Option<String>,String,i64,String)> = sqlx::query_as("SELECT rowid,CASE WHEN length(CAST(payload AS BLOB))<=8000000 THEN payload END,checksum,schema_version,operation FROM revision_envelope WHERE project_id=? AND entity_id=? AND entity_type='projectData' AND (? IS NULL OR rowid>?) ORDER BY rowid LIMIT 1")
                    .bind(pid).bind(format!("{pid}:writingSystems")).bind(after).bind(after).fetch_optional(&mut *tx).await.map_err(err)?;
                let Some((rowid,raw,checksum,version,operation)) = retained else { break; };
                after = Some(rowid);
                count += 1;
                if count > 4096 { return Err("Alphabet history exceeds the 4,096-revision verification bound. No history was truncated.".into()); }
                let raw = raw.ok_or("Retained alphabet payload exceeds 8 MB. No history was truncated.")?;
                bytes = bytes.saturating_add(raw.len());
                if bytes > 64 * 1024 * 1024 { return Err("Alphabet history exceeds the 64 MiB verification bound. No history was truncated.".into()); }
                if version != 1 || operation != "writingSystems" || hash(raw.as_bytes()) != checksum { return Err("Retained alphabet history checksum or schema failed. Stored data was preserved.".into()); }
                let previous = serde_json::from_str(&raw).map_err(|_| "Invalid retained alphabet history.")?;
                validate_data("writingSystems", &previous)?;
                validate_data_transition("writingSystems", &previous, &data.payload)?;
            }
            for assignment in data.payload["assignments"].as_array().ok_or("Invalid alphabet assignments.")? {
                let sid = assignment["scene"]["id"].as_str().ok_or("Invalid alphabet scene assignment.")?;
                if assignment["scene"]["kind"] != "scene" {
                    return Err("Invalid alphabet scene assignment.".into());
                }
                let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=? AND s.id=?)")
                    .bind(pid).bind(sid).fetch_one(&mut *tx).await.map_err(err)?;
                if !owned { return Err("An alphabet assignment refers to a missing or foreign scene. Stored writing was preserved.".into()); }
            }
        }
        let snapshot = AlphabetExportSnapshot { bundle, writing_systems };
        checked_size(&snapshot)?;
        tx.commit().await.map_err(err)?;
        Ok(snapshot)
    }
}
