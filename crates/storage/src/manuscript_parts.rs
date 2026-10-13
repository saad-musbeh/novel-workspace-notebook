//! Optional authored chapter groups in revisioned planning metadata. Canonical
//! chapter/scene identities, positions and writing are never changed here.
use crate::planning_read::{checked_chapter_orders,checked_data};
use crate::workspace::{append,err,hash,id,metadata_snapshot,read_project_index,title,valid_id,validate_data,validate_data_transition};
use crate::{Chapter,Project,ProjectDataSnapshot,Storage};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sqlx::{Sqlite,Transaction};
use std::collections::HashSet;

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ManuscriptPart{pub id:String,pub title:String,pub hidden:bool}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ChapterPart{pub chapter_id:String,pub part_id:String}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ManuscriptParts{pub schema_version:u32,pub parts:Vec<ManuscriptPart>,pub chapters:Vec<ChapterPart>}
impl Default for ManuscriptParts{fn default()->Self{Self{schema_version:1,parts:vec![],chapters:vec![]}}}
impl ManuscriptParts{
    pub fn validate(&self)->Result<(),String>{
        if self.schema_version!=1||self.parts.len()>1000||self.chapters.len()>20_000{return Err("Unsupported or oversized manuscript Parts metadata.".into());}
        let mut ids=HashSet::new();for part in &self.parts{if !valid_id(&part.id)||!ids.insert(&part.id){return Err("Part identities must be valid and unique.".into());}title(&part.title)?;}
        let mut chapters=HashSet::new();for assignment in &self.chapters{if !valid_id(&assignment.chapter_id)||!chapters.insert(&assignment.chapter_id)||!ids.contains(&assignment.part_id){return Err("Each chapter needs at most one assignment to an existing Part.".into());}}Ok(())
    }
    pub fn from_planning(payload:&Value)->Result<Self,String>{match payload.get("manuscriptParts"){None=>Ok(Self::default()),Some(value)=>{let parts:Self=serde_json::from_value(value.clone()).map_err(|_|"Invalid manuscript Parts metadata fields.")?;parts.validate()?;Ok(parts)}}}
}
/// The compatibility hook is strict only for this newly introduced explicit
/// schema. Older planning documents have no Parts and retain every other field.
pub fn validate_planning_parts(payload:&Value)->Result<(),String>{ManuscriptParts::from_planning(payload).map(|_|())}
/// Generic planning Save/checked Save/resolution still cannot introduce an
/// assignment to a foreign, missing or unresolved chapter. Only exact pairs in
/// the acknowledged basis may retain an already stale historical assignment.
pub(crate) async fn validate_parts_transition(tx:&mut Transaction<'_,Sqlite>,pid:&str,candidate:&Value,basis:&[&Value])->Result<(),String>{
    let next=ManuscriptParts::from_planning(candidate)?;let old=basis.iter().map(|old|ManuscriptParts::from_planning(old)).collect::<Result<Vec<_>,_>>()?;
    let changed=next.chapters.iter().filter(|assignment|!old.iter().any(|old|old.chapters.contains(assignment))).collect::<Vec<_>>();if changed.is_empty(){return Ok(());}
    capture_bounds(tx,pid).await?;source(tx,pid,pid,"project").await?;for assignment in changed{let owned:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chapter WHERE id=? AND project_id=?)").bind(&assignment.chapter_id).bind(pid).fetch_one(&mut **tx).await.map_err(err)?;if !owned{return Err("A new or changed Part assignment needs a canonical chapter owned by this manuscript.".into());}source(tx,pid,&assignment.chapter_id,"chapter").await?;}Ok(())
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PartsMetadataSource{pub entity_id:String,pub revision_id:String,pub payload:Value}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ManuscriptPartsSnapshot{pub project_id:String,pub project:Project,pub chapters:Vec<Chapter>,pub chapter_number_start:i64,pub project_source:PartsMetadataSource,pub chapter_sources:Vec<PartsMetadataSource>,pub planning:Option<ProjectDataSnapshot>}
async fn capture_bounds(tx:&mut Transaction<'_,Sqlite>,pid:&str)->Result<(),String>{
    let (count,bytes,max_bytes):(i64,i64,i64)=sqlx::query_as("SELECT count(*),coalesce(sum(length(cast(r.payload AS BLOB))+length(cast(r.parents AS BLOB))),0),coalesce(max(length(cast(r.payload AS BLOB))),0) FROM revision_envelope r WHERE r.project_id=? AND ((r.entity_type='project' AND r.entity_id=?) OR (r.entity_type='projectData' AND r.entity_id=?||':planning') OR (r.entity_type='chapter' AND EXISTS(SELECT 1 FROM chapter c WHERE c.id=r.entity_id AND c.project_id=r.project_id))) AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type=r.entity_type AND p.value=r.id)").bind(pid).bind(pid).bind(pid).fetch_one(&mut **tx).await.map_err(err)?;
    if count>20_002||bytes>64*1024*1024||max_bytes>8*1024*1024{return Err("Parts retained metadata exceeds the checked record or byte limit. Stored writing was preserved.".into());}
    let (chapters,scenes):(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM chapter WHERE project_id=?),(SELECT count(*) FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=?)").bind(pid).bind(pid).fetch_one(&mut **tx).await.map_err(err)?;if chapters>20_000||scenes>20_000{return Err("Parts index exceeds the checked chapter/scene read limit. Stored writing was preserved.".into());}Ok(())
}
async fn source(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,kind:&str)->Result<PartsMetadataSource,String>{
    let heads:Vec<(String,String,String,i64)>=sqlx::query_as("SELECT r.id,r.payload,r.checksum,r.schema_version FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type=? AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.project_id=r.project_id AND c.entity_id=r.entity_id AND c.entity_type=r.entity_type AND p.value=r.id) ORDER BY r.id LIMIT 2").bind(pid).bind(entity).bind(kind).fetch_all(&mut **tx).await.map_err(err)?;
    if heads.len()!=1{return Err("Conflict: Parts need one retained project/chapter metadata head. Resolve manuscript metadata before assigning chapters.".into());}
    let (revision_id,raw,checksum,version)=heads.into_iter().next().unwrap();if raw.len()>8*1024*1024{return Err("Parts metadata exceeds the checked byte limit.".into());}if version!=1||hash(raw.as_bytes())!=checksum{return Err("Parts source metadata checksum or schema failed. Stored writing was preserved.".into());}
    let payload:Value=serde_json::from_str(&raw).map_err(|_|"Invalid Parts source metadata.")?;let projection=metadata_snapshot(tx,entity,kind).await?;
    // Legacy creation envelopes may add position, or omit an order. Any
    // explicitly retained title/order must agree with the active projection.
    if !payload.is_object()||payload.get("title")!=projection.get("title")||payload.get("order").is_some_and(|order|Some(order)!=projection.get("order")){return Err("Conflict: retained Parts source metadata differs from its current title or order. Resolve metadata explicitly before assigning chapters.".into());}
    Ok(PartsMetadataSource{entity_id:entity.into(),revision_id,payload})
}
async fn capture(tx:&mut Transaction<'_,Sqlite>,pid:&str)->Result<ManuscriptPartsSnapshot,String>{
    if !valid_id(pid){return Err("Invalid manuscript project identity.".into());}capture_bounds(tx,pid).await?;let index=read_project_index(tx,pid).await?;if index.chapters.len()>20_000{return Err("Parts support at most 20,000 captured chapters.".into());}if index.chapters.iter().enumerate().any(|(i,c)|c.position!=i as i64){return Err("Canonical chapter positions are not contiguous. Resolve chapter order metadata before organizing Parts.".into());}if index.chapter_numbering_conflicted{return Err("Resolve competing chapter numbering before organizing Parts.".into());}checked_chapter_orders(tx,pid).await?;
    let project_source=source(tx,pid,pid,"project").await?;let mut chapter_sources=Vec::new();for chapter in &index.chapters{chapter_sources.push(source(tx,pid,&chapter.id,"chapter").await?);}
    let planning=checked_data(tx,pid,"planning").await?;if let Some(planning)=&planning{validate_planning_parts(&planning.payload)?;}
    Ok(ManuscriptPartsSnapshot{project_id:pid.into(),project:index.project,chapters:index.chapters,chapter_number_start:index.chapter_number_start,project_source,chapter_sources,planning})
}
impl Storage{
    pub async fn parts_edit_snapshot(&self,pid:&str)->Result<ManuscriptPartsSnapshot,String>{let mut tx=self.pool.begin().await.map_err(err)?;let snapshot=capture(&mut tx,pid).await?;tx.commit().await.map_err(err)?;Ok(snapshot)}
    pub async fn save_manuscript_parts_checked(&self,snapshot:ManuscriptPartsSnapshot,parts:ManuscriptParts)->Result<ManuscriptPartsSnapshot,String>{
        parts.validate()?;let mut tx=self.pool.begin().await.map_err(err)?;let current=capture(&mut tx,&snapshot.project_id).await?;if current!=snapshot{return Err("Conflict: planning or canonical chapter metadata changed. Parts draft retained; reopen after Save or Discard to review the current chapters.".into());}
        let old=snapshot.planning.as_ref().map(|p|&p.payload);let previous=old.map(ManuscriptParts::from_planning).transpose()?.unwrap_or_default();
        let known=snapshot.chapters.iter().map(|c|c.id.as_str()).collect::<HashSet<_>>();for assignment in &parts.chapters{let preserved=previous.chapters.iter().any(|old|old==assignment);if !known.contains(assignment.chapter_id.as_str())&&!preserved{return Err("A newly assigned chapter does not belong to this captured manuscript.".into());}}
        let mut payload=old.cloned().unwrap_or_else(||json!({"schemaVersion":1}));payload["manuscriptParts"]=serde_json::to_value(&parts).map_err(|error|error.to_string())?;validate_data("planning",&payload)?;validate_data_transition("planning",old.unwrap_or(&json!({"schemaVersion":1})),&payload)?;crate::planning_validation::validate_transition(old,&payload)?;validate_parts_transition(&mut tx,&snapshot.project_id,&payload,&old.into_iter().collect::<Vec<_>>()).await?;
        // Preserve absent legacy Parts metadata on a no-op rather than creating
        // a new empty group revision simply by opening and saving the page.
        if parts!=previous{let rid=id();append(&mut tx,&snapshot.project_id,&format!("{}:planning",snapshot.project_id),"projectData",snapshot.planning.as_ref().map(|p|p.revision_id.clone()).into_iter().collect(),payload,"planning",&rid).await?;sqlx::query("INSERT INTO project_data(project_id,kind,head) VALUES(?,'planning',?) ON CONFLICT(project_id,kind) DO UPDATE SET head=excluded.head").bind(&snapshot.project_id).bind(&rid).execute(&mut *tx).await.map_err(err)?;}
        let saved=capture(&mut tx,&snapshot.project_id).await?;tx.commit().await.map_err(err)?;Ok(saved)
    }
}
#[cfg(test)]#[path="manuscript_parts_tests.rs"]mod tests;
