//! Revision-bound scene placement; capture and mutation each use one transaction.
use crate::{Chapter, ProjectIndex, Storage};
use crate::workspace::{append, err, hash, id, metadata_parent, metadata_snapshot, validate_current_source};
use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct SceneArrangementEntry {
    pub id:String,
    pub position:i64,
    pub deleted:bool,
    pub placement_head:Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct SceneArrangementChapter {
    pub chapter:Chapter,
    pub metadata_head:Option<String>,
    pub scenes:Vec<SceneArrangementEntry>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct SceneArrangementSnapshot {
    pub project_id:String,
    pub scene_id:String,
    pub title:String,
    pub source_chapter_id:String,
    pub scene_head:String,
    pub project_head:Option<String>,
    pub chapters:Vec<SceneArrangementChapter>,
}
async fn verified_metadata(tx:&mut Transaction<'_,Sqlite>,entity:&str,kind:&str)->Result<Option<String>,String>{
    let heads=metadata_parent(tx,entity,kind).await?;
    if let Some(head)=heads.first(){
        let (raw,checksum):(String,String)=sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=?").bind(head).fetch_one(&mut **tx).await.map_err(err)?;
        if hash(raw.as_bytes())!=checksum{return Err("Metadata checksum failed. Stored data was preserved.".into())}
    }
    Ok(heads.first().cloned())
}
async fn capture(tx:&mut Transaction<'_,Sqlite>,pid:&str,sid:&str)->Result<SceneArrangementSnapshot,String>{
    let (source,head,payload,checksum):(String,String,String,String)=sqlx::query_as("SELECT s.chapter_id,s.head,r.payload,r.checksum FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id JOIN revision_envelope r ON r.id=s.head WHERE s.id=? AND c.project_id=? AND s.deleted=0").bind(sid).bind(pid).fetch_optional(&mut **tx).await.map_err(err)?.ok_or("Select a saved scene belonging to this novel.")?;
    validate_current_source(tx,pid,sid,"scene",&head).await?;
    if hash(payload.as_bytes())!=checksum{return Err("Scene checksum failed. Stored data was preserved.".into())}
    let scene:crate::Scene=serde_json::from_str(&payload).map_err(|_|"Invalid saved scene.")?;crate::validate(&scene)?;
    let project_head=verified_metadata(tx,pid,"project").await?;
    let rows=sqlx::query("SELECT id,project_id,title,position FROM chapter WHERE project_id=? ORDER BY position,id").bind(pid).fetch_all(&mut **tx).await.map_err(err)?;
    let mut chapters=vec![];
    for row in rows{
        let chapter=Chapter{id:row.get("id"),project_id:row.get("project_id"),title:row.get("title"),position:row.get("position")};
        let metadata_head=verified_metadata(tx,&chapter.id,"chapter").await?;
        let rows=sqlx::query("SELECT id,position,deleted FROM manuscript_scene WHERE chapter_id=? ORDER BY position,id").bind(&chapter.id).fetch_all(&mut **tx).await.map_err(err)?;
        let mut scenes=vec![];
        for row in rows{
            let scene_id:String=row.get("id");
            let placement_head=verified_metadata(tx,&scene_id,"scenePlacement").await?;
            scenes.push(SceneArrangementEntry{id:scene_id,position:row.get("position"),deleted:row.get::<i64,_>("deleted")!=0,placement_head});
        }
        chapters.push(SceneArrangementChapter{chapter,metadata_head,scenes});
    }
    Ok(SceneArrangementSnapshot{project_id:pid.into(),scene_id:sid.into(),title:scene.title,source_chapter_id:source,scene_head:head,project_head,chapters})
}
impl Storage {
    pub async fn scene_arrangement_snapshot(&self,pid:&str,sid:&str)->Result<SceneArrangementSnapshot,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let snapshot=capture(&mut tx,pid,sid).await?;tx.commit().await.map_err(err)?;Ok(snapshot)
    }
    /// `position` is a zero-based insertion slot among the target's visible scenes.
    pub async fn arrange_scene(&self,snapshot:&SceneArrangementSnapshot,target_chapter_id:&str,position:usize)->Result<ProjectIndex,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let current=capture(&mut tx,&snapshot.project_id,&snapshot.scene_id).await?;
        if &current!=snapshot{return Err("Conflict: the scene or manuscript arrangement changed. Reopen Arrange scene; your changes remain in this page.".into())}
        let target=snapshot.chapters.iter().find(|c|c.chapter.id==target_chapter_id).ok_or("Scenes can only move between chapters in the same novel.")?;
        let source=snapshot.chapters.iter().find(|c|c.chapter.id==snapshot.source_chapter_id).ok_or("Source chapter not found.")?;
        let mut target_order:Vec<String>=target.scenes.iter().filter(|s|!s.deleted&&s.id!=snapshot.scene_id).map(|s|s.id.clone()).collect();
        if position>target_order.len(){return Err("Scene position is outside the chapter.".into())}
        target_order.insert(position,snapshot.scene_id.clone());
        // Retain tombstones in their existing relative order after the visible scenes.
        target_order.extend(target.scenes.iter().filter(|s|s.deleted).map(|s|s.id.clone()));
        let source_order:Vec<String>=source.scenes.iter().filter(|s|s.id!=snapshot.scene_id&&!s.deleted).chain(source.scenes.iter().filter(|s|s.deleted)).map(|s|s.id.clone()).collect();
        sqlx::query("UPDATE manuscript_scene SET chapter_id=? WHERE id=?").bind(target_chapter_id).bind(&snapshot.scene_id).execute(&mut *tx).await.map_err(err)?;
        for (p,sid) in target_order.iter().enumerate(){sqlx::query("UPDATE manuscript_scene SET position=? WHERE id=?").bind(p as i64).bind(sid).execute(&mut *tx).await.map_err(err)?;}
        if source.chapter.id!=target_chapter_id{
            for (p,sid) in source_order.iter().enumerate(){sqlx::query("UPDATE manuscript_scene SET position=? WHERE id=?").bind(p as i64).bind(sid).execute(&mut *tx).await.map_err(err)?;}
        }
        let placement=source.scenes.iter().find(|s|s.id==snapshot.scene_id).ok_or("Scene not found in source chapter.")?;
        append(&mut tx,&snapshot.project_id,&snapshot.scene_id,"scenePlacement",placement.placement_head.clone().into_iter().collect(),serde_json::json!({"chapterId":target_chapter_id,"position":position,"fromChapterId":source.chapter.id}),"arrangeScene",&id()).await?;
        let changed_chapters=if source.chapter.id==target.chapter.id{vec![source]}else{vec![source,target]};
        for chapter in changed_chapters{
            let payload=metadata_snapshot(&mut tx,&chapter.chapter.id,"chapter").await?;
            append(&mut tx,&snapshot.project_id,&chapter.chapter.id,"chapter",chapter.metadata_head.clone().into_iter().collect(),payload,"arrangeScene",&id()).await?;
        }
        tx.commit().await.map_err(err)?;
        self.open_project_index(&snapshot.project_id).await
    }
}
