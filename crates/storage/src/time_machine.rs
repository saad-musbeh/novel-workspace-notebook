//! Read-only projections of locally recorded history, and bounded selective recovery.
//! Author timestamps never define a cutoff: imported history appears when received.
use crate::*;
use crate::workspace::{append,err,hash,id,revision,validate_data,validate_scene_links};
use sqlx::{Row,Sqlite,Transaction};
use std::collections::{BTreeMap,HashSet};
const OBJECT_LIMIT:usize=20_000;
const SUMMARY_LIMIT:i64=64*1024*1024;
const RELEVANT:&str="(r.entity_type IN('scene','tombstone','project','chapter','scenePlacement') OR (r.entity_type='projectData' AND r.operation IN('planning','universeCatalogue')))";

async fn check_cutoff(tx:&mut Transaction<'_,Sqlite>,pid:&str,wanted_epoch:&str,sequence:i64,point_revision_id:&str)->Result<(),String>{
    let epoch:String=sqlx::query_scalar("SELECT epoch FROM storage_identity WHERE singleton=1").fetch_one(&mut **tx).await.map_err(err)?;
    if wanted_epoch!=epoch{return Err("This history belongs to a different workspace epoch. Reload the time machine.".into())}
    let point:i64=sqlx::query_scalar("SELECT count(*) FROM change_marker m JOIN revision_envelope r ON r.id=m.revision_id WHERE r.project_id=? AND m.sequence=? AND r.id=?").bind(pid).bind(sequence).bind(point_revision_id).fetch_one(&mut **tx).await.map_err(err)?;
    if sequence<1||point!=1{return Err("This recorded point is unavailable in this project. Reload the time machine.".into())}Ok(())
}
fn normalized(kind:&str)->&str{if kind=="tombstone"{"scene"}else{kind}}
/// Only summary JSON is read. Immutable documents are verified on selected-body load.
async fn projected_heads(tx:&mut Transaction<'_,Sqlite>,pid:&str,cutoff:i64)->Result<BTreeMap<(String,String),Vec<TimeMachineHead>>,String>{
    let clause=format!("r.project_id=? AND {RELEVANT} AND m.sequence<=? AND NOT EXISTS(SELECT 1 FROM revision_envelope c JOIN change_marker cm ON cm.revision_id=c.id,json_each(c.parents) p WHERE c.project_id=r.project_id AND c.entity_id=r.entity_id AND (c.entity_type=r.entity_type OR (c.entity_type IN('scene','tombstone') AND r.entity_type IN('scene','tombstone'))) AND cm.sequence<=? AND p.value=r.id)");
    let (count,bytes):(i64,i64)=sqlx::query_as(&format!("SELECT count(*),coalesce(sum(length(coalesce(json_extract(r.payload,'$.order'),''))),0) FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE {clause}")).bind(pid).bind(cutoff).bind(cutoff).fetch_one(&mut **tx).await.map_err(err)?;
    if count>OBJECT_LIMIT as i64||bytes>SUMMARY_LIMIT{return Err("History preview exceeds the bounded 20,000-head/64 MiB summary limit. Export this project to inspect its complete history.".into())}
    let rows=sqlx::query(&format!("SELECT r.id,m.sequence,r.entity_id,r.entity_type,r.schema_version,r.created_at,json_extract(r.payload,'$.title') AS title,json_extract(r.payload,'$.order') AS object_order,json_extract(r.payload,'$.chapterId') AS chapter_id,json_extract(r.payload,'$.position') AS position FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE {clause} ORDER BY m.sequence,r.id")).bind(pid).bind(cutoff).bind(cutoff).fetch_all(&mut **tx).await.map_err(err)?;
    let mut result:BTreeMap<(String,String),Vec<TimeMachineHead>>=BTreeMap::new();
    for row in rows{
        if row.get::<i64,_>("schema_version")!=1{return Err("Unsupported history schema. Stored data was preserved.".into())}
        let kind:String=row.get("entity_type");
        let order:Option<String>=row.try_get("object_order").map_err(err)?;
        result.entry((row.get("entity_id"),normalized(&kind).into())).or_default().push(TimeMachineHead{revision_id:row.get("id"),sequence:row.get("sequence"),entity_type:kind,title:row.try_get("title").map_err(err)?,created_at:row.get("created_at"),order:order.map(|value|serde_json::from_str(&value).map_err(|_|"Invalid historical order.".to_string())).transpose()?,chapter_id:row.try_get("chapter_id").map_err(err)?,position:row.try_get("position").map_err(err)?});
    }Ok(result)
}
/// Replays recorded hierarchy events, never today's mutable SQL positions.
/// Competing metadata heads remain explicitly unprojected rather than last-write-wins.
async fn project_order(tx:&mut Transaction<'_,Sqlite>,pid:&str,cutoff:i64,objects:&mut [TimeMachineObject])->Result<(),String>{
    let conflicts=objects.iter().filter(|object|object.heads.len()>1).map(|object|(object.entity_id.clone(),object.entity_type.clone())).collect::<HashSet<_>>();
    let clause="r.project_id=? AND m.sequence<=? AND r.entity_type IN('project','chapter','scenePlacement')";
    let (count,bytes):(i64,i64)=sqlx::query_as(&format!("SELECT count(*),coalesce(sum(length(CAST(r.payload AS BLOB))),0) FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE {clause}")).bind(pid).bind(cutoff).fetch_one(&mut **tx).await.map_err(err)?;
    if count>OBJECT_LIMIT as i64||bytes>SUMMARY_LIMIT{return Err("Historical hierarchy exceeds the bounded 20,000-event/64 MiB preview limit.".into())}
    let rows=sqlx::query(&format!("SELECT r.* FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE {clause} ORDER BY m.sequence")).bind(pid).bind(cutoff).fetch_all(&mut **tx).await.map_err(err)?;
    let mut chapters:Vec<String>=vec![];let mut contents:BTreeMap<String,Vec<String>>=BTreeMap::new();
    for row in rows{
        let entity:String=row.get("entity_id");let kind:String=row.get("entity_type");
        if conflicts.contains(&(entity.clone(),kind.clone())){continue;}
        let raw:String=row.get("payload");if row.get::<i64,_>("schema_version")!=1||hash(raw.as_bytes())!=row.get::<String,_>("checksum"){return Err("Historical hierarchy checksum/schema failed.".into())}
        let payload:Value=serde_json::from_str(&raw).map_err(|_|"Invalid historical hierarchy.")?;
        let order=payload.get("order").map(|value|serde_json::from_value::<Vec<String>>(value.clone()).map_err(|_|"Invalid historical order.")).transpose()?;
        match kind.as_str(){
            "project"=>if let Some(order)=order{chapters=order},
            "chapter"=>{if !chapters.contains(&entity){let pos=payload.get("position").and_then(Value::as_u64).unwrap_or(chapters.len() as u64) as usize;chapters.insert(pos.min(chapters.len()),entity.clone());}
                if let Some(order)=order{for existing in contents.values_mut(){existing.retain(|id|!order.contains(id));}contents.insert(entity,order);}else{contents.entry(entity).or_default();}}
            "scenePlacement"=>{let chapter=payload.get("chapterId").and_then(Value::as_str).ok_or("Invalid historical placement.")?;let position=payload.get("position").and_then(Value::as_u64).ok_or("Invalid historical placement position.")? as usize;
                for order in contents.values_mut(){order.retain(|id|id!=&entity);}let order=contents.entry(chapter.into()).or_default();order.insert(position.min(order.len()),entity);}
            _=>{}
        }
    }
    for object in objects{
        if object.entity_type=="chapter"&&!conflicts.contains(&(pid.into(),"project".into())){object.position=chapters.iter().position(|id|id==&object.entity_id).map(|position|position as i64);}
        if object.entity_type=="scene"&&!conflicts.contains(&(object.entity_id.clone(),"scenePlacement".into())){
            for (chapter,order) in &contents{if !conflicts.contains(&(chapter.clone(),"chapter".into())){if let Some(position)=order.iter().position(|id|id==&object.entity_id){object.chapter_id=Some(chapter.clone());object.position=Some(position as i64);}}}
        }
    }Ok(())
}
pub(crate) async fn exact_heads(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,kind:&str)->Result<Vec<String>,String>{
    let predicate=if kind=="scene"{"r.entity_type IN('scene','tombstone')"}else{"r.entity_type='projectData'"};
    sqlx::query_scalar(&format!("SELECT r.id FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND {predicate} AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.project_id=r.project_id AND c.entity_id=r.entity_id AND (c.entity_type=r.entity_type OR(c.entity_type IN('scene','tombstone') AND r.entity_type IN('scene','tombstone'))) AND p.value=r.id) ORDER BY r.id")).bind(pid).bind(entity).fetch_all(&mut **tx).await.map_err(err)
}
impl Storage{
    pub async fn project_time_points(&self,pid:&str,before_sequence:Option<i64>,limit:u32)->Result<ProjectTimePoints,String>{
        if !(1..=100).contains(&limit)||before_sequence.is_some_and(|value|value<1){return Err("History page requires 1–100 points and a positive cursor.".into())}
        let mut tx=self.pool.begin().await.map_err(err)?;
        let exists:i64=sqlx::query_scalar("SELECT count(*) FROM project WHERE id=?").bind(pid).fetch_one(&mut *tx).await.map_err(err)?;
        if exists!=1{return Err("Project not found.".into())}
        let epoch=sqlx::query_scalar("SELECT epoch FROM storage_identity WHERE singleton=1").fetch_one(&mut *tx).await.map_err(err)?;
        let rows=sqlx::query(&format!("SELECT m.sequence,r.id,r.created_at,r.entity_id,r.entity_type,r.operation FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.project_id=? AND {RELEVANT} AND m.sequence<? ORDER BY m.sequence DESC LIMIT ?")).bind(pid).bind(before_sequence.unwrap_or(i64::MAX)).bind(limit+1).fetch_all(&mut *tx).await.map_err(err)?;
        let has_more=rows.len()>limit as usize;
        let points=rows.into_iter().take(limit as usize).map(|row|ProjectTimePoint{sequence:row.get("sequence"),revision_id:row.get("id"),created_at:row.get("created_at"),entity_id:row.get("entity_id"),entity_type:row.get("entity_type"),operation:row.get("operation")}).collect();
        tx.commit().await.map_err(err)?;Ok(ProjectTimePoints{epoch,points,has_more})
    }
    pub async fn project_time_preview(&self,pid:&str,wanted_epoch:&str,sequence:i64,point_revision_id:&str)->Result<ProjectTimePreview,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;check_cutoff(&mut tx,pid,wanted_epoch,sequence,point_revision_id).await?;
        let old=projected_heads(&mut tx,pid,sequence).await?;
        let current=projected_heads(&mut tx,pid,i64::MAX).await?;
        let mut objects=old.into_iter().map(|((entity_id,entity_type),heads)|{let current_head_ids=current.get(&(entity_id.clone(),entity_type.clone())).into_iter().flatten().map(|head|head.revision_id.clone()).collect();TimeMachineObject{entity_id,entity_type,heads,current_head_ids,chapter_id:None,position:None}}).collect::<Vec<_>>();
        project_order(&mut tx,pid,sequence,&mut objects).await?;
        let mut warnings=vec!["Points follow changes recorded on this computer. Imported author timestamps do not change that order.".into(),"Historical titles, order and placements are shown only where revisions recorded them; missing early metadata is not reconstructed from today's layout.".into()];
        if objects.iter().any(|object|object.heads.len()>1){warnings.push("This point contains competing revisions. Every branch is retained; choose a preview explicitly.".into());}
        tx.commit().await.map_err(err)?;Ok(ProjectTimePreview{epoch:wanted_epoch.into(),point_revision_id:point_revision_id.into(),project_id:pid.into(),sequence,objects,warnings})
    }
    /// Copy an explicitly reviewed historical scene or whole planning document.
    /// Current head validation, deletion revival, envelope and marker are atomic.
    pub async fn recover_time_piece(&self,pid:&str,wanted_epoch:&str,sequence:i64,point_revision_id:&str,entity:&str,kind:&str,rid:&str,expected_heads:Vec<String>)->Result<Revision,String>{
        if kind!="scene"&&(kind!="projectData"||![format!("{pid}:planning"),format!("{pid}:universeCatalogue")].contains(&entity.to_string())){return Err("Recover a scene or the whole story bible explicitly. Other metadata is preview-only.".into())}
        let mut tx=self.pool.begin().await.map_err(err)?;check_cutoff(&mut tx,pid,wanted_epoch,sequence,point_revision_id).await?;
        let actual=exact_heads(&mut tx,pid,entity,kind).await?;
        if expected_heads.len()!=1||actual.len()!=1||actual!=expected_heads{return Err("Conflict: this piece changed or has competing heads. Refresh and resolve conflicts before recovering; nothing was replaced.".into())}
        let historic:Option<sqlx::sqlite::SqliteRow>=sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.project_id=? AND r.entity_id=? AND r.entity_type=? AND r.id=? AND m.sequence<=? AND NOT EXISTS(SELECT 1 FROM revision_envelope c JOIN change_marker cm ON cm.revision_id=c.id,json_each(c.parents) p WHERE c.project_id=r.project_id AND c.entity_id=r.entity_id AND(c.entity_type=r.entity_type OR(c.entity_type IN('scene','tombstone') AND r.entity_type IN('scene','tombstone'))) AND cm.sequence<=? AND p.value=r.id)").bind(pid).bind(entity).bind(kind).bind(rid).bind(sequence).bind(sequence).fetch_optional(&mut *tx).await.map_err(err)?;
        let historic=revision(historic.ok_or("This version is not a head at the selected recorded point.")?)?;
        if historic.schema_version!=1||hash(serde_json::to_string(&historic.payload).unwrap().as_bytes())!=historic.checksum{return Err("Historical revision schema or checksum failed. Nothing was recovered.".into())}
        let fresh=id();
        if kind=="scene"{
            let scene:Scene=serde_json::from_value(historic.payload.clone()).map_err(|_|"Unsupported historical scene.")?;validate(&scene)?;
            let owner:Option<String>=sqlx::query_scalar("SELECT c.project_id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(entity).fetch_optional(&mut *tx).await.map_err(err)?;
            if owner.as_deref()!=Some(pid){return Err("This scene is not in the selected project.".into())}
            append(&mut tx,pid,entity,"scene",actual,historic.payload,"timeMachineRestore",&fresh).await?;
            sqlx::query("UPDATE manuscript_scene SET head=?,deleted=0 WHERE id=?").bind(&fresh).bind(entity).execute(&mut *tx).await.map_err(err)?;
            crate::sync::recompute_scene_heads(&mut tx,entity).await?;
        }else{
            let data_kind=historic.operation.as_str();if !["planning","universeCatalogue"].contains(&data_kind)||entity!=format!("{pid}:{data_kind}"){return Err("This version is not the selected story bible or universe catalogue.".into())}
            validate_data(data_kind,&historic.payload)?;
            let ids:Vec<String>=sqlx::query_scalar("SELECT s.id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=?").bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
            if data_kind=="planning"{validate_scene_links(&historic.payload,&ids.into_iter().collect::<HashSet<_>>())?;}else{crate::universe::references(&mut tx,pid,&historic.payload,false).await?;}
            let current:Option<String>=sqlx::query_scalar("SELECT head FROM project_data WHERE project_id=? AND kind=?").bind(pid).bind(data_kind).fetch_optional(&mut *tx).await.map_err(err)?;
            if current.as_ref()!=actual.first(){return Err("Conflict: the current story bible projection changed. Nothing was recovered.".into())}
            append(&mut tx,pid,entity,"projectData",actual,historic.payload,data_kind,&fresh).await?;
            sqlx::query("UPDATE project_data SET head=? WHERE project_id=? AND kind=?").bind(&fresh).bind(pid).bind(data_kind).execute(&mut *tx).await.map_err(err)?;
        }
        let result=revision(sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.id=?").bind(&fresh).fetch_one(&mut *tx).await.map_err(err)?)?;
        tx.commit().await.map_err(err)?;Ok(result)
    }
}
