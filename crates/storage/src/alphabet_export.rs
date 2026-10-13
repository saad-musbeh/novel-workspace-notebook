//! A saved manuscript and its immutable alphabet versions share one read snapshot.
use crate::planning_read::{checked_chapter_orders, checked_data};
use crate::workspace::{decode_scene, err, hash, read_project_index, validate_data, validate_data_transition};
use crate::{ProjectBundle, ProjectDataSnapshot, SceneRecord, Storage};
use serde::{Deserialize, Serialize};
use sqlx::Row;
const LIMIT:usize = 64 * 1024 * 1024;
struct SizeBound(usize);
impl std::io::Write for SizeBound {
    fn write(&mut self, bytes:&[u8])->std::io::Result<usize> {
        self.0 = self.0.checked_add(bytes.len()).filter(|n| *n<=LIMIT).ok_or_else(||std::io::Error::other("Alphabet export exceeds 64 MiB."))?;
        Ok(bytes.len())
    }
    fn flush(&mut self)->std::io::Result<()> { Ok(()) }
}
fn size<T:Serialize>(value:&T)->Result<usize,String> {
    let mut writer=SizeBound(0);serde_json::to_writer(&mut writer,value).map_err(|e|e.to_string())?;Ok(writer.0)
}

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
        let index = read_project_index(&mut tx, pid).await?;
        let conflict: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revision_envelope r WHERE r.project_id=? AND r.entity_type IN('project','chapter','scenePlacement','scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND (child.entity_type=r.entity_type OR child.entity_type IN('scene','tombstone') AND r.entity_type IN('scene','tombstone')) AND p.value=r.id) GROUP BY r.entity_id,CASE WHEN r.entity_type IN('scene','tombstone') THEN 'scene' ELSE r.entity_type END HAVING count(*)>1)")
            .bind(pid).fetch_one(&mut *tx).await.map_err(err)?;
        if conflict || index.chapter_numbering_conflicted {
            return Err("Conflict: resolve manuscript, deletion, placement and chapter conflicts before exporting alphabets.".into());
        }
        let bad_projection:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=? AND NOT EXISTS(SELECT 1 FROM revision_envelope r WHERE r.project_id=c.project_id AND r.entity_id=s.id AND (s.deleted=1 AND r.entity_type='tombstone' OR s.deleted=0 AND r.entity_type='scene' AND r.id=s.head) AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type IN('scene','tombstone') AND p.value=r.id)))")
            .bind(pid).fetch_one(&mut *tx).await.map_err(err)?;
        if bad_projection { return Err("Conflict: manuscript deletion or scene projection differs from its retained head. Use Recovery before exporting.".into()); }
        checked_chapter_orders(&mut tx, pid).await?;
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
        let mut captured_bytes=size(&index)?.saturating_add(size(&writing_systems)?);
        let mut scenes = Vec::new();
        for scene in &index.scenes {
            let row = sqlx::query("SELECT CASE WHEN length(CAST(payload AS BLOB))<=2000000 THEN payload END AS payload,checksum,schema_version,entity_id,entity_type,project_id FROM revision_envelope WHERE id=?")
                .bind(&scene.revision_id).fetch_one(&mut *tx).await.map_err(err)?;
            let raw:String=row.get::<Option<String>,_>("payload").ok_or("Saved scene exceeds 2 MB; no file was written.")?;
            captured_bytes=captured_bytes.saturating_add(raw.len());
            if captured_bytes>LIMIT { return Err("Alphabet export exceeds 64 MiB; no file was written.".into()); }
            if row.get::<String,_>("project_id") != pid || row.get::<String,_>("entity_id") != scene.id || row.get::<String,_>("entity_type") != "scene" || row.get::<i64,_>("schema_version") != 1 {
                return Err("Unsupported manuscript source identity or schema.".into());
            }
            let leaves: Vec<String> = sqlx::query_scalar("SELECT r.id FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type IN('scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type IN('scene','tombstone') AND p.value=r.id) ORDER BY r.id")
                .bind(pid).bind(&scene.id).fetch_all(&mut *tx).await.map_err(err)?;
            if leaves != vec![scene.revision_id.clone()] {
                return Err("Conflict: manuscript projection differs from its retained scene head. Use Recovery before exporting.".into());
            }
            scenes.push(SceneRecord { id:scene.id.clone(), chapter_id:scene.chapter_id.clone(), position:scene.position, revision_id:scene.revision_id.clone(), scene:decode_scene(&raw,row.get("checksum"))? });
        }
        let snapshot = AlphabetExportSnapshot { bundle:ProjectBundle { project:index.project, chapters:index.chapters, scenes, chapter_number_start:index.chapter_number_start, chapter_numbering_revision_id:index.chapter_numbering_revision_id, chapter_numbering_conflicted:false }, writing_systems };
        size(&snapshot)?;
        tx.commit().await.map_err(err)?;
        Ok(snapshot)
    }
}
