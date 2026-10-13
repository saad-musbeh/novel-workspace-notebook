//! Read-only authored views bind manuscript order and unambiguous data together.
use crate::workspace::{err, hash, read_project_index, valid_id, validate_data, validate_kind};
use crate::{ProjectDataSnapshot, ProjectIndex, Storage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Sqlite, Transaction};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanningReadSnapshot {
    pub index: ProjectIndex,
    pub planning: Option<ProjectDataSnapshot>,
    pub scene_metadata: Option<ProjectDataSnapshot>,
}

async fn checked_data(
    tx: &mut Transaction<'_, Sqlite>,
    pid: &str,
    kind: &str,
) -> Result<Option<ProjectDataSnapshot>, String> {
    validate_kind(kind)?;
    let entity = format!("{pid}:{kind}");
    let heads: Vec<(String, String, String)> = sqlx::query_as("SELECT r.id,r.payload,r.checksum FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type='projectData' AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type='projectData' AND p.value=r.id) ORDER BY r.id LIMIT 2")
        .bind(pid).bind(&entity).fetch_all(&mut **tx).await.map_err(err)?;
    if heads.len() > 1 {
        return Err(format!("Conflict: resolve competing {kind} versions in Project data conflicts before using this saved view."));
    }
    let projection: Option<String> =
        sqlx::query_scalar("SELECT head FROM project_data WHERE project_id=? AND kind=?")
            .bind(pid)
            .bind(kind)
            .fetch_optional(&mut **tx)
            .await
            .map_err(err)?;
    let Some((revision_id, raw, checksum)) = heads.into_iter().next() else {
        if projection.is_some() {
            return Err(
                "Saved data has no verified retained head. Stored data was preserved.".into(),
            );
        }
        return Ok(None);
    };
    if projection.as_deref() != Some(&revision_id) {
        return Err("Saved data projection does not match its sole retained head. Stored data was preserved.".into());
    }
    if hash(raw.as_bytes()) != checksum {
        return Err("Project data checksum failed. Stored data was preserved.".into());
    }
    let payload: Value = serde_json::from_str(&raw).map_err(|_| "Invalid saved project data.")?;
    validate_data(kind, &payload)?;
    Ok(Some(ProjectDataSnapshot {
        revision_id,
        payload,
    }))
}

/// A sole retained chapter head may still await explicit replay after competing
/// placements settle. Do not build reader cutoffs from that interim projection.
async fn checked_chapter_orders(tx: &mut Transaction<'_, Sqlite>, pid: &str) -> Result<(), String> {
    // Include deleted scenes: deletion hides text without changing the retained
    // order or membership used by explicit metadata recovery.
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT s.chapter_id,s.id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=? ORDER BY c.position,c.id,s.position,s.id")
        .bind(pid).fetch_all(&mut **tx).await.map_err(err)?;
    let mut members: HashMap<String, Vec<String>> = HashMap::new();
    for (chapter, scene) in rows {
        members.entry(chapter).or_default().push(scene);
    }
    let heads: Vec<(String, String, String, i64)> = sqlx::query_as("SELECT r.entity_id,r.payload,r.checksum,r.schema_version FROM revision_envelope r JOIN chapter c ON c.id=r.entity_id AND c.project_id=r.project_id WHERE r.project_id=? AND r.entity_type='chapter' AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type='chapter' AND p.value=r.id) ORDER BY r.entity_id,r.id")
        .bind(pid).fetch_all(&mut **tx).await.map_err(err)?;
    for (chapter, raw, checksum, version) in heads {
        if version != 1 || hash(raw.as_bytes()) != checksum {
            return Err(
                "Chapter metadata checksum or schema failed. Stored writing was preserved.".into(),
            );
        }
        let payload: Value =
            serde_json::from_str(&raw).map_err(|_| "Invalid saved chapter metadata.")?;
        if !payload.is_object() {
            return Err("Invalid saved chapter metadata.".into());
        }
        let Some(order) = payload.get("order") else {
            continue;
        };
        let order = order
            .as_array()
            .filter(|a| a.len() <= 20_000)
            .ok_or("Invalid saved chapter order.")?;
        let mut listed = HashSet::new();
        let expected = order
            .iter()
            .map(|id| {
                let id = id
                    .as_str()
                    .filter(|id| valid_id(id))
                    .ok_or("Invalid saved chapter scene identity.")?;
                if !listed.insert(id) {
                    return Err("Duplicate saved chapter scene identity.");
                }
                Ok(id)
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let current = members.get(&chapter).map(Vec::as_slice).unwrap_or_default();
        let actual: Vec<&str> = current
            .iter()
            .map(String::as_str)
            .filter(|id| listed.contains(id))
            .collect();
        // New scenes absent from an older order are legitimate additions. The
        // listed members themselves must all remain here and in retained order.
        if actual != expected {
            return Err("Conflict: a retained chapter order still differs from settled scene membership or order. Use Scene and chapter conflicts to explicitly apply the retained order or keep the current chapter membership and order before using this saved planning view.".into());
        }
    }
    Ok(())
}
impl Storage {
    /// Optional preferences and read-only tools must not choose a competing data head.
    pub async fn load_project_data_unambiguous(
        &self,
        pid: &str,
        kind: &str,
    ) -> Result<Option<ProjectDataSnapshot>, String> {
        let mut tx = self.pool.begin().await.map_err(err)?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project WHERE id=?)")
            .bind(pid)
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
        if !exists {
            return Err("Project not found.".into());
        }
        let data = checked_data(&mut tx, pid, kind).await?;
        tx.commit().await.map_err(err)?;
        Ok(data)
    }
    /// Body-free snapshot: reveal progress, links and scene details share one order.
    pub async fn planning_read_snapshot(&self, pid: &str) -> Result<PlanningReadSnapshot, String> {
        let mut tx = self.pool.begin().await.map_err(err)?;
        let index = read_project_index(&mut tx, pid).await?;
        let conflict: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revision_envelope r WHERE r.project_id=? AND r.entity_type IN('project','chapter','scenePlacement','scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND (child.entity_type=r.entity_type OR child.entity_type IN('scene','tombstone') AND r.entity_type IN('scene','tombstone')) AND p.value=r.id) GROUP BY r.entity_id,CASE WHEN r.entity_type IN('scene','tombstone') THEN 'scene' ELSE r.entity_type END HAVING count(*)>1)")
            .bind(pid).fetch_one(&mut *tx).await.map_err(err)?;
        if conflict || index.chapter_numbering_conflicted {
            return Err("Conflict: resolve competing manuscript versions, placements or chapter metadata before using this saved planning view.".into());
        }
        checked_chapter_orders(&mut tx, pid).await?;
        let planning = checked_data(&mut tx, pid, "planning").await?;
        let scene_metadata = checked_data(&mut tx, pid, "sceneMetadata").await?;
        tx.commit().await.map_err(err)?;
        Ok(PlanningReadSnapshot {
            index,
            planning,
            scene_metadata,
        })
    }
}
