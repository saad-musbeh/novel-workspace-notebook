//! Project-scoped catalogues; novels remain separate manuscripts.
use crate::{Storage,Project,Scene,SceneRecord,ProjectDataSnapshot};
use crate::workspace::{append,id,title,valid_id,hash,validate_current_source};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use sqlx::{Sqlite,Transaction};
use std::collections::HashSet;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct UniverseSeries {pub id:String,pub title:String}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct UniverseNovel {pub project_id:String,pub series_id:Option<String>}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct UniverseCatalogue {pub schema_version:u32,pub series:Vec<UniverseSeries>,pub novels:Vec<UniverseNovel>}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NovelCreated {pub project:Project,pub scene:SceneRecord}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct UniverseNovelCreated {pub project:Project,pub scene:SceneRecord,pub catalogue_revision_id:String}
fn err(e:impl std::fmt::Display)->String{e.to_string()}
pub(crate) fn catalogue(value:&Value)->Result<UniverseCatalogue,String>{
 if value.get("novels").and_then(Value::as_array).is_some_and(|novels|novels.iter().any(|n|n.get("seriesId").is_none())){return Err("Novel seriesId must be explicit or null.".into())}
 let data:UniverseCatalogue=serde_json::from_value(value.clone()).map_err(|_|"Invalid universe catalogue fields.")?;
 if data.schema_version!=1||data.series.len()>1000||data.novels.len()>20000{return Err("Unsupported or oversized universe catalogue.".into())}
 let mut series=HashSet::new();for s in &data.series{if !valid_id(&s.id)||!series.insert(&s.id){return Err("Invalid or duplicate series identity.".into())}title(&s.title)?;}
 let mut novels=HashSet::new();for n in &data.novels{if !valid_id(&n.project_id)||!novels.insert(&n.project_id)||n.series_id.as_ref().is_some_and(|s|!series.contains(s)){return Err("Invalid, duplicate or ungrouped novel identity.".into())}}
 Ok(data)
}
/// External catalogue references may arrive before their novels through sync or
/// portable import. They never create projects or grant access to those projects.
pub(crate) async fn references(tx:&mut Transaction<'_,Sqlite>,pid:&str,value:&Value,allow_missing:bool)->Result<(),String>{
 let data=catalogue(value)?;
 for n in &data.novels{
  if n.project_id==pid{return Err("A universe cannot contain itself as a novel.".into())}
  let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project WHERE id=?)").bind(&n.project_id).fetch_one(&mut **tx).await.map_err(err)?;
  if !allow_missing&&!exists{return Err("A catalogue novel is not available on this computer.".into())}
  let root:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revision_envelope WHERE project_id=? AND entity_type='projectData' AND operation='universeCatalogue')").bind(&n.project_id).fetch_one(&mut **tx).await.map_err(err)?;
  if root{return Err("A universe root cannot be assigned as a novel.".into())}
 }
 // Every competing leaf counts: never silently replace another universe's claim.
 let rows:Vec<(String,String,String)>=sqlx::query_as("SELECT r.project_id,r.payload,r.checksum FROM revision_envelope r WHERE r.entity_type='projectData' AND r.operation='universeCatalogue' AND r.project_id!=? AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.project_id=r.project_id AND c.entity_id=r.entity_id AND p.value=r.id) AND EXISTS(SELECT 1 FROM json_each(r.payload,'$.novels') n WHERE json_extract(n.value,'$.projectId')=? OR EXISTS(SELECT 1 FROM json_each(?,'$.novels') wanted WHERE json_extract(wanted.value,'$.projectId')=json_extract(n.value,'$.projectId')))").bind(pid).bind(pid).bind(value.to_string()).fetch_all(&mut **tx).await.map_err(err)?;
 for (_,payload,checksum) in rows{if hash(payload.as_bytes())!=checksum{return Err("Universe catalogue checksum failed.".into())}catalogue(&serde_json::from_str(&payload).map_err(|_|"Invalid saved universe catalogue.")?)?;return Err("A project is already assigned to another universe. Resolve its membership explicitly first.".into())}
 Ok(())
}
async fn create_project(tx:&mut Transaction<'_,Sqlite>,name:&str)->Result<Project,String>{
 title(name)?;let pid=id();sqlx::query("INSERT INTO project(id,title) VALUES(?,?)").bind(&pid).bind(name).execute(&mut **tx).await.map_err(err)?;
 append(tx,&pid,&pid,"project",vec![],json!({"title":name,"order":[]}),"create",&id()).await?;
 let created_at=sqlx::query_scalar("SELECT created_at FROM project WHERE id=?").bind(&pid).fetch_one(&mut **tx).await.map_err(err)?;
 Ok(Project{id:pid,title:name.into(),created_at})
}
async fn create_manuscript(tx:&mut Transaction<'_,Sqlite>,name:&str)->Result<NovelCreated,String>{
  let project=create_project(tx,name).await?;let cid=id();let sid=id();let rid=id();
  sqlx::query("INSERT INTO chapter(id,project_id,title,position) VALUES(?,?,'Chapter 1',0)").bind(&cid).bind(&project.id).execute(&mut **tx).await.map_err(err)?;
  append(tx,&project.id,&cid,"chapter",vec![],json!({"title":"Chapter 1","position":0,"order":[sid]}),"create",&id()).await?;
  let scene=Scene{schema_version:1,title:"Untitled scene".into(),document:json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"auto","lang":"und"},"content":[]}]})};
  append(tx,&project.id,&sid,"scene",vec![],serde_json::to_value(&scene).unwrap(),"create",&rid).await?;
  sqlx::query("INSERT INTO manuscript_scene(id,chapter_id,position,head) VALUES(?,?,0,?)").bind(&sid).bind(&cid).bind(&rid).execute(&mut **tx).await.map_err(err)?;
  append(tx,&project.id,&sid,"scenePlacement",vec![],json!({"chapterId":cid,"position":0}),"create",&id()).await?;
  // Capture the final chapter order rather than an empty initial manifest.
  let initial:String=sqlx::query_scalar("SELECT id FROM revision_envelope WHERE entity_id=? AND entity_type='project' ORDER BY rowid LIMIT 1").bind(&project.id).fetch_one(&mut **tx).await.map_err(err)?;
  append(tx,&project.id,&project.id,"project",vec![initial],json!({"title":name,"order":[cid]}),"order",&id()).await?;
  Ok(NovelCreated{project,scene:SceneRecord{id:sid,chapter_id:cid,position:0,revision_id:rid,scene}})
}
async fn store_catalogue(tx:&mut Transaction<'_,Sqlite>,pid:&str,payload:Value,parent:Option<String>)->Result<ProjectDataSnapshot,String>{
 crate::workspace::validate_data("universeCatalogue",&payload)?;
 let rid=id();append(tx,pid,&format!("{pid}:universeCatalogue"),"projectData",parent.into_iter().collect(),payload.clone(),"universeCatalogue",&rid).await?;
 sqlx::query("INSERT INTO project_data(project_id,kind,head) VALUES(?,'universeCatalogue',?) ON CONFLICT(project_id,kind) DO UPDATE SET head=excluded.head").bind(pid).bind(&rid).execute(&mut **tx).await.map_err(err)?;
 Ok(ProjectDataSnapshot{revision_id:rid,payload})
}
impl Storage{
 pub async fn create_novel(&self,name:&str)->Result<NovelCreated,String>{
  title(name)?;let mut tx=self.pool.begin().await.map_err(err)?;let created=create_manuscript(&mut tx,name).await?;
  tx.commit().await.map_err(err)?;Ok(created)
 }
 pub async fn create_universe(&self,name:&str)->Result<Project,String>{
  title(name)?;let mut tx=self.pool.begin().await.map_err(err)?;let project=create_project(&mut tx,name).await?;
  store_catalogue(&mut tx,&project.id,json!({"schemaVersion":1,"series":[],"novels":[]}),None).await?;
  tx.commit().await.map_err(err)?;Ok(project)
 }
 pub async fn create_universe_novel(&self,universe_id:&str,series_id:Option<String>,name:&str,parent:&str)->Result<UniverseNovelCreated,String>{
  title(name)?;let mut tx=self.pool.begin().await.map_err(err)?;
  validate_current_source(&mut tx,universe_id,&format!("{universe_id}:universeCatalogue"),"projectData",parent).await?;
  let (payload,checksum):(String,String)=sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=?").bind(parent).fetch_one(&mut *tx).await.map_err(err)?;
  if hash(payload.as_bytes())!=checksum{return Err("Universe catalogue checksum failed.".into())}
  let mut data=catalogue(&serde_json::from_str(&payload).map_err(|_|"Invalid universe catalogue.")?)?;
  if series_id.as_ref().is_some_and(|sid|!data.series.iter().any(|s|s.id==*sid)){return Err("Series is not in this universe.".into())}
  let NovelCreated{project,scene}=create_manuscript(&mut tx,name).await?;
  data.novels.push(UniverseNovel{project_id:project.id.clone(),series_id});let payload=serde_json::to_value(data).unwrap();references(&mut tx,universe_id,&payload,false).await?;
  let saved=store_catalogue(&mut tx,universe_id,payload,Some(parent.into())).await?;
  tx.commit().await.map_err(err)?;Ok(UniverseNovelCreated{project,scene,catalogue_revision_id:saved.revision_id})
 }
}
