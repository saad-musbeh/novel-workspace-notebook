//! Checked read-only catalogue membership. Shared notes retain their source
//! project/revision; no novel planning data is written or merged here.
use crate::planning_read::checked_data;
use crate::universe::catalogue;
use crate::workspace::{err,hash,title,valid_id,validate_data};
use crate::{Project,ProjectDataSnapshot,Storage,UniverseCatalogue};
use serde::{Deserialize,Serialize};
use serde_json::Value;
use sqlx::{Row,Sqlite,Transaction};
use std::collections::{BTreeMap,HashMap,HashSet,VecDeque};

const LIMIT:usize=20_000;
const BYTE_LIMIT:usize=64*1024*1024;
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct UniverseCatalogueView{pub project_id:String,pub title:String,pub catalogue_revision_id:Option<String>,pub catalogue:Option<UniverseCatalogue>,pub unavailable:Option<String>}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct UniverseMembership{pub project_id:String,pub universe_id:Option<String>,pub series_id:Option<String>,pub status:String,pub reason:Option<String>}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct UniverseCatalogueSnapshot{pub projects:Vec<Project>,pub universes:Vec<UniverseCatalogueView>,pub memberships:Vec<UniverseMembership>}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct UniverseNotesSnapshot{pub project_id:String,pub membership:UniverseMembership,pub universe:Option<UniverseCatalogueView>,pub planning:Option<ProjectDataSnapshot>,pub unavailable:Option<String>}

struct Revision{id:String,pid:String,entity:String,operation:String,parents:String,raw:String,checksum:String,schema:i64}
/// Check every retained ancestor and the complete scoped parent graph. A broken
/// ancestor cannot be hidden behind a plausible current catalogue/planning head.
fn history(rows:&[Revision],pid:&str,kind:&str)->Result<Vec<usize>,String>{
    if !valid_id(pid){return Err("Invalid shared-source project identity.".into());}
    if rows.len()>LIMIT{return Err("Shared-source history exceeds the checked read limit.".into());}
    let metadata=kind=="project";let entity=if metadata{pid.to_owned()}else{format!("{pid}:{kind}")};let mut bytes=0usize;let mut index=HashMap::new();
    for (i,row) in rows.iter().enumerate(){bytes=bytes.saturating_add(row.raw.len()).saturating_add(row.parents.len());if bytes>BYTE_LIMIT{return Err("Shared-source history exceeds the checked byte limit.".into());}
        if !valid_id(&row.id)||row.pid!=pid||row.entity!=entity||(!metadata&&row.operation!=kind)||row.schema!=1||hash(row.raw.as_bytes())!=row.checksum||index.insert(row.id.as_str(),i).is_some(){return Err("Shared-source history identity, schema or checksum failed.".into());}
        let value:Value=serde_json::from_str(&row.raw).map_err(|_|"Invalid shared-source history payload.")?;if metadata{if !value.is_object(){return Err("Invalid universe title metadata history.".into());}if let Some(name)=value.get("title"){title(name.as_str().ok_or("Invalid universe title history.")?)?;}if let Some(order)=value.get("order"){let order=order.as_array().filter(|a|a.len()<=LIMIT).ok_or("Invalid universe project order history.")?;let mut unique=HashSet::new();for id in order{let id=id.as_str().filter(|id|valid_id(id)).ok_or("Invalid universe project order identity.")?;if !unique.insert(id){return Err("Duplicate universe project order identity.".into());}}}}else{validate_data(kind,&value)?;}
    }
    let mut children=vec![Vec::new();rows.len()];let mut indegree=vec![0usize;rows.len()];let mut work=0usize;
    for (i,row) in rows.iter().enumerate(){let parents:Vec<String>=serde_json::from_str(&row.parents).map_err(|_|"Invalid shared-source history parents.")?;let mut seen=HashSet::new();for parent in parents{work+=1;if work>LIMIT*4{return Err("Shared-source history exceeds the checked parent limit.".into());}if !seen.insert(parent.clone()){return Err("Duplicate shared-source history parent.".into());}let Some(&p)=index.get(parent.as_str())else{return Err("Missing or foreign shared-source history parent.".into());};children[p].push(i);indegree[i]+=1;}}
    let mut queue:VecDeque<usize>=indegree.iter().enumerate().filter_map(|(i,n)|(*n==0).then_some(i)).collect();let mut visited=0;
    while let Some(i)=queue.pop_front(){visited+=1;for child in &children[i]{indegree[*child]-=1;if indegree[*child]==0{queue.push_back(*child);}}}
    if visited!=rows.len(){return Err("Cyclic shared-source history was preserved; no membership or notes were chosen.".into());}
    Ok(children.iter().enumerate().filter_map(|(i,c)|c.is_empty().then_some(i)).collect())
}
fn revision(row:sqlx::sqlite::SqliteRow)->Revision{Revision{id:row.get("id"),pid:row.get("project_id"),entity:row.get("entity_id"),operation:row.get("operation"),parents:row.get("parents"),raw:row.get("payload"),checksum:row.get("checksum"),schema:row.get("schema_version")}}
async fn source_history(tx:&mut Transaction<'_,Sqlite>,pid:&str,kind:&str)->Result<Vec<Revision>,String>{
    Ok(sqlx::query("SELECT id,project_id,entity_id,operation,parents,payload,checksum,schema_version FROM revision_envelope WHERE project_id=? AND entity_type='projectData' AND entity_id=? ORDER BY id LIMIT 20001").bind(pid).bind(format!("{pid}:{kind}")).fetch_all(&mut **tx).await.map_err(err)?.into_iter().map(revision).collect())
}
async fn checked_universe_title(tx:&mut Transaction<'_,Sqlite>,project:&Project,work:&mut usize,bytes:&mut usize)->Result<(),String>{
    title(&project.title)?;if *work>=LIMIT||*bytes>=BYTE_LIMIT{return Err("Universe title history exceeds the checked read limit.".into());}
    let rows=sqlx::query("SELECT id,project_id,entity_id,operation,parents,payload,checksum,schema_version FROM revision_envelope WHERE project_id=? AND entity_id=? AND entity_type='project' ORDER BY id LIMIT 20001").bind(&project.id).bind(&project.id).fetch_all(&mut **tx).await.map_err(err)?.into_iter().map(revision).collect::<Vec<_>>();
    *work=work.saturating_add(rows.len());*bytes=bytes.saturating_add(rows.iter().map(|r|r.raw.len().saturating_add(r.parents.len())).sum::<usize>());if *work>LIMIT||*bytes>BYTE_LIMIT{return Err("Universe title history exceeds the checked read limit.".into());}
    let heads=history(&rows,&project.id,"project")?;if heads.len()!=1{return Err("Universe title has missing or competing retained versions.".into());}
    let value:Value=serde_json::from_str(&rows[heads[0]].raw).map_err(|_|"Invalid universe title history.")?;if value["title"].as_str()!=Some(project.title.as_str()){return Err("Universe title projection does not match its retained head.".into());}Ok(())
}
async fn capture(tx:&mut Transaction<'_,Sqlite>)->Result<UniverseCatalogueSnapshot,String>{
    let projects:Vec<Project>=sqlx::query("SELECT id,title,created_at FROM project ORDER BY created_at,id LIMIT 20001").fetch_all(&mut **tx).await.map_err(err)?.into_iter().map(|r|Project{id:r.get("id"),title:r.get("title"),created_at:r.get("created_at")}).collect();
    if projects.len()>LIMIT{return Err("Universe catalogue exceeds the checked project limit. Local projects remain available.".into());}
    let by_id=projects.iter().map(|p|(p.id.as_str(),p)).collect::<HashMap<_,_>>();
    let raw=sqlx::query("SELECT id,project_id,entity_id,operation,parents,payload,checksum,schema_version FROM revision_envelope WHERE entity_type='projectData' AND (operation='universeCatalogue' OR entity_id=project_id||':universeCatalogue') ORDER BY project_id,id LIMIT 20001").fetch_all(&mut **tx).await.map_err(err)?;
    if raw.len()>LIMIT{return Err("Universe catalogue exceeds the checked history limit. Local projects remain available.".into());}
    let mut histories:BTreeMap<String,Vec<Revision>>=BTreeMap::new();let mut bytes=0usize;for row in raw{let r=revision(row);bytes=bytes.saturating_add(r.raw.len()).saturating_add(r.parents.len());if bytes>BYTE_LIMIT{return Err("Universe catalogue exceeds the checked byte limit.".into());}histories.entry(r.pid.clone()).or_default().push(r);}
    let projections:HashMap<String,String>=sqlx::query_as::<_,(String,String)>("SELECT project_id,head FROM project_data WHERE kind='universeCatalogue' LIMIT 20001").fetch_all(&mut **tx).await.map_err(err)?.into_iter().collect();
    if projections.len()>LIMIT{return Err("Universe catalogue exceeds the checked projection limit.".into());}for pid in projections.keys(){histories.entry(pid.clone()).or_default();}
    let roots=histories.keys().cloned().collect::<HashSet<_>>();let mut views=Vec::new();let mut claims:HashMap<String,Vec<(String,Option<String>,bool)>>=HashMap::new();let mut unknown_history=None;let mut membership_work=0usize;let mut title_work=histories.values().map(Vec::len).sum::<usize>();
    for (pid,rows) in histories{
        let mut view=UniverseCatalogueView{project_id:pid.clone(),title:by_id.get(pid.as_str()).map(|p|p.title.clone()).unwrap_or_else(||"Unavailable universe".into()),catalogue_revision_id:None,catalogue:None,unavailable:None};
        match history(&rows,&pid,"universeCatalogue"){
            Err(error)=>{unknown_history=Some(error.clone());view.unavailable=Some(error);},
            Ok(heads)=>{
                if heads.is_empty(){let error="Universe catalogue has no retained history.".to_owned();unknown_history=Some(error.clone());view.unavailable=Some(error);}
                else{
                    if heads.len()!=1{view.unavailable=Some("Conflict: resolve competing universe catalogue versions before grouping novels or reading shared notes.".into());}
                    else if projections.get(&pid)!=Some(&rows[heads[0]].id){view.unavailable=Some("Universe catalogue projection does not match its sole retained head.".into());}
                    else if let Some(project)=by_id.get(pid.as_str()){if let Err(error)=checked_universe_title(tx,project,&mut title_work,&mut bytes).await{view.unavailable=Some(error);}}
                    else{view.unavailable=Some("This universe project is not available on this computer.".into());}
                    let catalogues=heads.iter().map(|head|catalogue(&serde_json::from_str(&rows[*head].raw).map_err(|_|"Invalid universe catalogue.")?)).collect::<Result<Vec<_>,String>>()?;
                    if catalogues.iter().any(|cat|cat.novels.iter().any(|n|roots.contains(&n.project_id))){view.unavailable=Some("A universe root is claimed as a novel; no shared source was chosen.".into());}
                    let available=view.unavailable.is_none();
                    for (head,cat) in heads.iter().zip(catalogues){membership_work=membership_work.saturating_add(cat.novels.len()).saturating_add(cat.series.len());if membership_work>LIMIT{return Err("Universe catalogue exceeds the checked membership limit.".into());}for novel in &cat.novels{let rows=claims.entry(novel.project_id.clone()).or_default();if !rows.iter().any(|(root,_,_)|root==&pid){rows.push((pid.clone(),novel.series_id.clone(),available));}}
                        if available{view.catalogue_revision_id=Some(rows[*head].id.clone());view.catalogue=Some(cat);}
                    }
                }
            }
        }views.push(view);
    }
    let memberships=projects.iter().map(|project|{
        let mut entry=UniverseMembership{project_id:project.id.clone(),universe_id:None,series_id:None,status:"unassigned".into(),reason:None};
        if roots.contains(&project.id){entry.status="universe".into();if claims.contains_key(&project.id){entry.status="conflicted".into();entry.reason=Some("A universe root is also claimed as a novel. No membership was chosen.".into());}return entry;}
        if let Some(reason)=&unknown_history{entry.status="unavailable".into();entry.reason=Some(format!("Catalogue history could not be verified; no membership was chosen. {reason}"));return entry;}
        match claims.get(&project.id).map(Vec::as_slice).unwrap_or(&[]){[]=>{},[(root,series,true)]=>{entry.status="member".into();entry.universe_id=Some(root.clone());entry.series_id=series.clone();},[_]=>{entry.status="conflicted".into();entry.reason=Some("The claimed universe catalogue is conflicted or unavailable. No membership was chosen.".into());},_=>{entry.status="conflicted".into();entry.reason=Some("Competing universes claim this novel. Resolve membership explicitly before using shared notes.".into());}}entry
    }).collect();Ok(UniverseCatalogueSnapshot{projects,universes:views,memberships})
}
impl Storage{
    pub async fn universe_catalogue_snapshot(&self)->Result<UniverseCatalogueSnapshot,String>{let mut tx=self.pool.begin().await.map_err(err)?;let snapshot=capture(&mut tx).await?;tx.commit().await.map_err(err)?;Ok(snapshot)}
    pub async fn universe_notes_snapshot(&self,pid:&str)->Result<UniverseNotesSnapshot,String>{
        if !valid_id(pid){return Err("Invalid project identity.".into());}let mut tx=self.pool.begin().await.map_err(err)?;let catalogue=capture(&mut tx).await?;
        let membership=catalogue.memberships.iter().find(|m|m.project_id==pid).cloned().ok_or("Project not available on this computer.")?;
        let root=if membership.status=="member"{membership.universe_id.as_deref()}else if membership.status=="universe"{Some(pid)}else{None};
        let mut snapshot=UniverseNotesSnapshot{project_id:pid.into(),membership:membership.clone(),universe:root.and_then(|id|catalogue.universes.iter().find(|v|v.project_id==id).cloned()),planning:None,unavailable:membership.reason.clone()};
        if let Some(root)=root{
            if let Some(view)=&snapshot.universe{if let Some(reason)=&view.unavailable{snapshot.unavailable=Some(reason.clone());}else{
                let rows=source_history(&mut tx,root,"planning").await?;
                match history(&rows,root,"planning"){Err(error)=>snapshot.unavailable=Some(error),Ok(_)=>match checked_data(&mut tx,root,"planning").await{Ok(data)=>snapshot.planning=data,Err(error)=>snapshot.unavailable=Some(error)}}
            }}else{snapshot.unavailable=Some("The shared universe is unavailable.".into());}
        }else if snapshot.unavailable.is_none(){snapshot.unavailable=Some("This novel has no assigned shared universe.".into());}
        tx.commit().await.map_err(err)?;Ok(snapshot)
    }
}
#[cfg(test)]#[path="universe_read_tests.rs"]mod tests;
