//! A bounded, conflict-free saved manuscript captured in the caller's transaction.
use crate::planning_read::checked_chapter_orders;
use crate::workspace::{decode_scene, err, hash, read_project_index, valid_id};
use crate::{ProjectBundle, SceneRecord};
use serde::Serialize;
use sqlx::{Row,Sqlite,Transaction};
use std::collections::{HashMap,HashSet};
const LIMIT:usize = 64 * 1024 * 1024;
struct SizeBound(usize);
impl std::io::Write for SizeBound {
    fn write(&mut self, bytes:&[u8])->std::io::Result<usize> {
        self.0 = self.0.checked_add(bytes.len()).filter(|n| *n<=LIMIT).ok_or_else(||std::io::Error::other("Saved manuscript exceeds 64 MiB."))?;
        Ok(bytes.len())
    }
    fn flush(&mut self)->std::io::Result<()> { Ok(()) }
}
pub(crate) fn checked_size<T:Serialize>(value:&T)->Result<usize,String> {
    let mut writer=SizeBound(0);serde_json::to_writer(&mut writer,value).map_err(|e|e.to_string())?;Ok(writer.0)
}
async fn hierarchy_sources(tx:&mut Transaction<'_,Sqlite>,index:&crate::ProjectIndex)->Result<(),String> {
    let pid=&index.project.id;
    let scenes:Vec<(String,String)>=sqlx::query_as("SELECT s.id,s.chapter_id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=?").bind(pid).fetch_all(&mut **tx).await.map_err(err)?;
    let scenes:HashMap<String,String>=scenes.into_iter().collect();
    let chapters:HashSet<&str>=index.chapters.iter().map(|c|c.id.as_str()).collect();
    let mut after:Option<i64>=None;
    loop {
        let row:Option<(i64,String,String,Option<String>,String,i64)>=sqlx::query_as("SELECT r.rowid,r.entity_id,r.entity_type,CASE WHEN length(CAST(r.payload AS BLOB))<=8000000 THEN r.payload END,r.checksum,r.schema_version FROM revision_envelope r WHERE r.project_id=? AND r.entity_type IN('project','chapter','scenePlacement') AND (? IS NULL OR r.rowid>?) AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type=r.entity_type AND p.value=r.id) ORDER BY r.rowid LIMIT 1")
            .bind(pid).bind(after).bind(after).fetch_optional(&mut **tx).await.map_err(err)?;
        let Some((rowid,entity,kind,raw,checksum,schema))=row else{break;};after=Some(rowid);
        let raw=raw.ok_or("Saved hierarchy metadata exceeds 8 MB.")?;
        if schema!=1 || hash(raw.as_bytes())!=checksum {return Err("Saved hierarchy metadata checksum or schema failed. Stored writing was preserved.".into());}
        let payload:serde_json::Value=serde_json::from_str(&raw).map_err(|_|"Invalid saved hierarchy metadata.")?;
        if !payload.is_object(){return Err("Invalid saved hierarchy metadata.".into());}
        match kind.as_str() {
            "project"=>{
                if entity!=*pid{return Err("Foreign project metadata identity.".into());}
                if let Some(order)=payload.get("order") {
                    let order=order.as_array().filter(|o|o.len()<=20_000).ok_or("Invalid saved chapter order.")?;
                    let mut listed=HashSet::new();let mut expected=Vec::new();
                    for id in order {let id=id.as_str().filter(|id|valid_id(id)&&listed.insert(*id)).ok_or("Invalid or duplicate saved chapter identity.")?;expected.push(id);}
                    let actual:Vec<&str>=index.chapters.iter().map(|c|c.id.as_str()).filter(|id|listed.contains(id)).collect();
                    if expected!=actual {return Err("Conflict: retained chapter order differs from current chapter membership or order. Use Scene and chapter conflicts before reading saved manuscript text.".into());}
                }
            }
            "chapter"=>{if !chapters.contains(entity.as_str()){return Err("Foreign chapter metadata identity.".into());}}
            "scenePlacement"=>{
                let chapter=payload["chapterId"].as_str().ok_or("Invalid saved scene placement.")?;
                if !chapters.contains(chapter)||!payload["position"].as_u64().is_some_and(|n|n<=20_000)||scenes.get(&entity).map(String::as_str)!=Some(chapter){return Err("Conflict: saved placement differs from scene ownership. Use Scene and chapter conflicts before reading saved manuscript text.".into());}
            }
            _=>unreachable!(),
        }
    }
    Ok(())
}

/// The caller adds its own metadata checks before committing the same read transaction.
pub(crate) async fn read_project_manuscript(tx:&mut Transaction<'_,Sqlite>,pid:&str)->Result<ProjectBundle,String>{
        let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM chapter WHERE project_id=?),(SELECT count(*) FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=?)")
            .bind(pid).bind(pid).fetch_one(&mut **tx).await.map_err(err)?;
        if counts.0>20_000 || counts.1>20_000 {return Err("Saved manuscript exceeds the 20,000-chapter or scene capture bound. Stored writing was preserved.".into());}
        let index = read_project_index(tx, pid).await?;
        let conflict: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revision_envelope r WHERE r.project_id=? AND r.entity_type IN('project','chapter','scenePlacement','scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND (child.entity_type=r.entity_type OR child.entity_type IN('scene','tombstone') AND r.entity_type IN('scene','tombstone')) AND p.value=r.id) GROUP BY r.entity_id,CASE WHEN r.entity_type IN('scene','tombstone') THEN 'scene' ELSE r.entity_type END HAVING count(*)>1)")
            .bind(pid).fetch_one(&mut **tx).await.map_err(err)?;
        if conflict || index.chapter_numbering_conflicted {
            return Err("Conflict: resolve manuscript, deletion, placement and chapter conflicts before reading saved manuscript text.".into());
        }
        let bad_projection:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=? AND NOT EXISTS(SELECT 1 FROM revision_envelope r WHERE r.project_id=c.project_id AND r.entity_id=s.id AND (s.deleted=1 AND r.entity_type='tombstone' OR s.deleted=0 AND r.entity_type='scene' AND r.id=s.head) AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type IN('scene','tombstone') AND p.value=r.id)))")
            .bind(pid).fetch_one(&mut **tx).await.map_err(err)?;
        if bad_projection { return Err("Conflict: manuscript deletion or scene projection differs from its retained head. Use Recovery before reading saved manuscript text.".into()); }
        checked_chapter_orders(tx, pid).await?;
        hierarchy_sources(tx,&index).await?;
        let mut captured_bytes=checked_size(&index)?;
        let mut scenes = Vec::new();
        for scene in &index.scenes {
            let row = sqlx::query("SELECT CASE WHEN length(CAST(payload AS BLOB))<=2000000 THEN payload END AS payload,checksum,schema_version,entity_id,entity_type,project_id FROM revision_envelope WHERE id=?")
                .bind(&scene.revision_id).fetch_one(&mut **tx).await.map_err(err)?;
            let raw:String=row.get::<Option<String>,_>("payload").ok_or("Saved scene exceeds 2 MB.")?;
            captured_bytes=captured_bytes.saturating_add(raw.len());
            if captured_bytes>LIMIT { return Err("Saved manuscript exceeds 64 MiB.".into()); }
            if row.get::<String,_>("project_id") != pid || row.get::<String,_>("entity_id") != scene.id || row.get::<String,_>("entity_type") != "scene" || row.get::<i64,_>("schema_version") != 1 {
                return Err("Unsupported manuscript source identity or schema.".into());
            }
            let leaves: Vec<String> = sqlx::query_scalar("SELECT r.id FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type IN('scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type IN('scene','tombstone') AND p.value=r.id) ORDER BY r.id")
                .bind(pid).bind(&scene.id).fetch_all(&mut **tx).await.map_err(err)?;
            if leaves != vec![scene.revision_id.clone()] {
                return Err("Conflict: manuscript projection differs from its retained scene head. Use Recovery before reading saved manuscript text.".into());
            }
            scenes.push(SceneRecord { id:scene.id.clone(), chapter_id:scene.chapter_id.clone(), position:scene.position, revision_id:scene.revision_id.clone(), scene:decode_scene(&raw,row.get("checksum"))? });
        }
        let bundle=ProjectBundle { project:index.project, chapters:index.chapters, scenes, chapter_number_start:index.chapter_number_start, chapter_numbering_revision_id:index.chapter_numbering_revision_id, chapter_numbering_conflicted:false };
        checked_size(&bundle)?;
        Ok(bundle)
}
