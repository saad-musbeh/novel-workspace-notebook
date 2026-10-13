//! Explicit scene/deletion recovery with a complete retained-head capture.
use crate::sync::recompute_scene_heads;
use crate::workspace::{append, err, hash, id, valid_id};
use crate::{Scene, Storage};
use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneConflictRevision {
    pub revision_id: String,
    pub entity_type: String,
    pub operation: String,
    pub created_at: String,
    pub scene: Option<Scene>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneConflictSnapshot {
    pub project_id: String,
    pub scene_id: String,
    pub chapter_id: String,
    pub chapter_title: String,
    pub position: i64,
    pub deleted: bool,
    pub projected_revision_id: String,
    pub projected_scene: Scene,
    pub heads: Vec<SceneConflictRevision>,
    pub placement_heads: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneConflictEntry {
    pub scene_id: String,
    pub title: String,
    pub chapter_title: String,
    pub deleted: bool,
    pub head_count: i64,
}

fn decode(payload: &str, checksum: &str, schema: i64) -> Result<Scene, String> {
    if schema != 1 || hash(payload.as_bytes()) != checksum {
        return Err(
            "Scene recovery checksum or schema failed. Stored writing was preserved.".into(),
        );
    }
    let scene: Scene =
        serde_json::from_str(payload).map_err(|_| "Invalid retained scene recovery document.")?;
    crate::validate(&scene)?;
    Ok(scene)
}
async fn capture(
    tx: &mut Transaction<'_, Sqlite>,
    pid: &str,
    sid: &str,
) -> Result<SceneConflictSnapshot, String> {
    if !valid_id(pid) || !valid_id(sid) {
        return Err("Invalid scene recovery identity.".into());
    }
    let row = sqlx::query("SELECT s.chapter_id,c.title,s.position,s.deleted,s.head,r.payload,r.checksum,r.schema_version,r.project_id,r.entity_id,r.entity_type FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id JOIN revision_envelope r ON r.id=s.head WHERE s.id=? AND c.project_id=?")
        .bind(sid).bind(pid).fetch_optional(&mut **tx).await.map_err(err)?.ok_or("Scene does not belong to this novel.")?;
    if row.get::<String, _>("project_id") != pid
        || row.get::<String, _>("entity_id") != sid
        || row.get::<String, _>("entity_type") != "scene"
    {
        return Err("Scene recovery projection does not belong to this manuscript scene.".into());
    }
    let projected_scene = decode(
        row.get("payload"),
        row.get("checksum"),
        row.get("schema_version"),
    )?;
    let rows = sqlx::query("SELECT r.id,r.entity_type,r.operation,r.created_at,r.payload,r.checksum,r.schema_version FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type IN('scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type IN('scene','tombstone') AND p.value=r.id) ORDER BY r.id LIMIT 65")
        .bind(pid).bind(sid).fetch_all(&mut **tx).await.map_err(err)?;
    if rows.is_empty() || rows.len() > 64 {
        return Err("Scene recovery requires 1–64 retained heads.".into());
    }
    let mut heads = Vec::new();
    for r in rows {
        let kind: String = r.get("entity_type");
        let raw: &str = r.get("payload");
        let checksum: &str = r.get("checksum");
        let version = r.get("schema_version");
        let scene = if kind == "scene" {
            Some(decode(raw, checksum, version)?)
        } else {
            if version != 1
                || hash(raw.as_bytes()) != checksum
                || serde_json::from_str::<serde_json::Value>(raw)
                    .map_err(|_| "Invalid retained deletion.")?
                    != serde_json::json!({"deleted":true})
            {
                return Err(
                    "Scene deletion checksum or schema failed. Stored history was preserved."
                        .into(),
                );
            }
            None
        };
        heads.push(SceneConflictRevision {
            revision_id: r.get("id"),
            entity_type: kind,
            operation: r.get("operation"),
            created_at: r.get("created_at"),
            scene,
        });
    }
    let placements = sqlx::query("SELECT r.id,r.payload,r.checksum,r.schema_version FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type='scenePlacement' AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type='scenePlacement' AND p.value=r.id) ORDER BY r.id LIMIT 65")
        .bind(pid).bind(sid).fetch_all(&mut **tx).await.map_err(err)?;
    if placements.len() > 64 {
        return Err("Scene recovery placement scope exceeds 64 heads.".into());
    }
    let mut placement_heads = Vec::new();
    for r in placements {
        if r.get::<i64, _>("schema_version") != 1
            || hash(r.get::<String, _>("payload").as_bytes()) != r.get::<String, _>("checksum")
        {
            return Err(
                "Scene placement checksum or schema failed. Stored writing was preserved.".into(),
            );
        }
        placement_heads.push(r.get("id"));
    }
    let deleted = row.get::<i64, _>("deleted") != 0;
    let current: String = row.get("head");
    let content_heads = heads
        .iter()
        .filter(|h| h.entity_type == "scene")
        .collect::<Vec<_>>();
    if deleted != content_heads.is_empty()
        || (!deleted && !content_heads.iter().any(|h| h.revision_id == current))
    {
        return Err(
            "Scene recovery projection and retained heads disagree. Stored writing was preserved."
                .into(),
        );
    }
    let snapshot = SceneConflictSnapshot {
        project_id: pid.into(),
        scene_id: sid.into(),
        chapter_id: row.get("chapter_id"),
        chapter_title: row.get("title"),
        position: row.get("position"),
        deleted,
        projected_revision_id: current,
        projected_scene,
        heads,
        placement_heads,
    };
    if serde_json::to_vec(&snapshot)
        .map_err(|_| "Invalid scene recovery capture.")?
        .len()
        > 8_000_000
    {
        return Err("The complete scene recovery capture exceeds 8 MB. All retained writing remains in storage.".into());
    }
    Ok(snapshot)
}

impl Storage {
    /// Body-free project catalogue includes deleted scenes with competing leaves.
    pub async fn scene_conflict_index(&self, pid: &str) -> Result<Vec<SceneConflictEntry>, String> {
        if !valid_id(pid) {
            return Err("Invalid scene recovery project identity.".into());
        }
        let mut tx = self.pool.begin().await.map_err(err)?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project WHERE id=?)")
            .bind(pid)
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
        if !exists {
            return Err("Project not found.".into());
        }
        let rows = sqlx::query("SELECT s.id,c.title chapter_title,s.deleted,CASE WHEN json_valid(r.payload) THEN json_type(r.payload,'$.title') END title_type,CASE WHEN json_valid(r.payload) THEN json_extract(r.payload,'$.title') END scene_title,(SELECT count(*) FROM revision_envelope h WHERE h.project_id=c.project_id AND h.entity_id=s.id AND h.entity_type IN('scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=h.project_id AND child.entity_id=h.entity_id AND child.entity_type IN('scene','tombstone') AND p.value=h.id)) head_count FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id JOIN revision_envelope r ON r.id=s.head WHERE c.project_id=? ORDER BY c.position,c.id,s.position,s.id LIMIT 20001")
            .bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
        if rows.len() > 20_000 {
            return Err("Scene recovery catalogue exceeds 20,000 scenes.".into());
        }
        let mut entries = Vec::new();
        for r in rows {
            if r.get::<Option<String>, _>("title_type").as_deref() != Some("text") {
                return Err("Invalid saved scene recovery title.".into());
            }
            let title: String = r.get("scene_title");
            if title.chars().count() > 500 {
                return Err("Invalid saved scene recovery title.".into());
            }
            entries.push(SceneConflictEntry {
                scene_id: r.get("id"),
                title,
                chapter_title: r.get("chapter_title"),
                deleted: r.get::<i64, _>("deleted") != 0,
                head_count: r.get("head_count"),
            });
        }
        tx.commit().await.map_err(err)?;
        Ok(entries)
    }
    pub async fn scene_conflict_snapshot(
        &self,
        pid: &str,
        sid: &str,
    ) -> Result<SceneConflictSnapshot, String> {
        let mut tx = self.pool.begin().await.map_err(err)?;
        let snapshot = capture(&mut tx, pid, sid).await?;
        tx.commit().await.map_err(err)?;
        Ok(snapshot)
    }
    /// `Some(scene)` is an explicit author writing candidate; `None` explicitly
    /// keeps deletion. All original leaves and captured placement must match.
    pub async fn resolve_scene_conflict_checked(
        &self,
        snapshot: &SceneConflictSnapshot,
        scene: Option<Scene>,
    ) -> Result<SceneConflictSnapshot, String> {
        if let Some(scene) = &scene {
            crate::validate(scene)?;
        }
        let mut tx = self.pool.begin().await.map_err(err)?;
        let current = capture(&mut tx, &snapshot.project_id, &snapshot.scene_id).await?;
        if current != *snapshot {
            return Err("Conflict: the scene versions, deletion or placement changed after this preview. Refresh scene recovery; stored writing was preserved.".into());
        }
        if snapshot.heads.len() < 2 && (!snapshot.deleted || scene.is_none()) {
            return Err("This scene has no competing versions to resolve. A single deleted version may only be explicitly restored.".into());
        }
        let parents = snapshot
            .heads
            .iter()
            .map(|h| h.revision_id.clone())
            .collect();
        let revision_id = id();
        if let Some(scene) = scene {
            append(
                &mut tx,
                &snapshot.project_id,
                &snapshot.scene_id,
                "scene",
                parents,
                serde_json::to_value(&scene).map_err(|_| "Invalid writing candidate.")?,
                "resolve",
                &revision_id,
            )
            .await?;
            sqlx::query("UPDATE manuscript_scene SET head=?,deleted=0 WHERE id=?")
                .bind(&revision_id)
                .bind(&snapshot.scene_id)
                .execute(&mut *tx)
                .await
                .map_err(err)?;
        } else {
            append(
                &mut tx,
                &snapshot.project_id,
                &snapshot.scene_id,
                "tombstone",
                parents,
                serde_json::json!({"deleted":true}),
                "resolveDelete",
                &revision_id,
            )
            .await?;
        }
        recompute_scene_heads(&mut tx, &snapshot.scene_id).await?;
        let result = capture(&mut tx, &snapshot.project_id, &snapshot.scene_id).await?;
        tx.commit().await.map_err(err)?;
        Ok(result)
    }
}
