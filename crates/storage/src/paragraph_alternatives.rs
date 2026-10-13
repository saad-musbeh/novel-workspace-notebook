use crate::{Scene,validate};
use crate::workspace::err;
use serde_json::{Value,json};
use sqlx::{Sqlite,Transaction};
use std::collections::HashSet;
fn string(value:&Value,key:&str,max:usize)->bool{value.get(key).and_then(Value::as_str).is_some_and(|s|!s.is_empty()&&s.chars().count()<=max)}
fn block(value:&Value)->Result<(),String>{if ![Some("paragraph"),Some("heading")].contains(&value.get("type").and_then(Value::as_str)){return Err("An alternative must contain a paragraph or heading.".into())}validate(&Scene{schema_version:1,title:String::new(),document:json!({"type":"doc","content":[value]})})}
pub(crate) fn validate_payload(payload:&Value)->Result<(),String>{
 let scenes=payload["scenes"].as_array().filter(|s|s.len()<=10000).ok_or("Invalid paragraph alternative scenes.")?;
 let(mut scene_ids,mut groups,mut versions)=(HashSet::new(),HashSet::new(),HashSet::new());
 for scene in scenes{
  if !string(scene,"sceneId",300)||!scene_ids.insert(scene["sceneId"].as_str().unwrap()){return Err("Invalid paragraph alternative scene identity.".into())}
  for group in scene["groups"].as_array().filter(|g|g.len()<=100).ok_or("Invalid paragraph groups.")?{
   if !string(group,"id",300)||!groups.insert(group["id"].as_str().unwrap())||!group["label"].as_str().is_some_and(|s|s.chars().count()<=200)||!string(group,"sceneRevisionId",300)||!group["anchor"]["blockIndex"].as_u64().is_some_and(|n|n<=100000)||!group["anchor"]["documentHash"].as_str().is_some_and(|s|s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))){return Err("Invalid paragraph alternative anchor.".into())}
   block(&group["anchor"]["block"])?;
   let items=group["versions"].as_array().filter(|v|!v.is_empty()&&v.len()<=20).ok_or("Invalid paragraph versions.")?;
   for version in items{if !string(version,"id",300)||!versions.insert(version["id"].as_str().unwrap())||!version["label"].as_str().is_some_and(|s|s.chars().count()<=200){return Err("Invalid paragraph version identity.".into())}block(&version["block"])?;}
   if !items.iter().any(|v|v["id"]==group["originalId"])||!items.iter().any(|v|v["id"]==group["activeId"]){return Err("Paragraph original or active version is missing.".into())}
  }
 }Ok(())
}
pub(crate) fn transition(old:&Value,new:&Value)->Result<(),String>{
 for scene in old["scenes"].as_array().ok_or("Invalid prior alternatives.")?{for group in scene["groups"].as_array().ok_or("Invalid prior groups.")?{
  let next=new["scenes"].as_array().unwrap().iter().flat_map(|s|s["groups"].as_array().unwrap().iter().map(move|g|(s,g))).find(|(_,g)|g["id"]==group["id"]);
  if let Some((s,g))=next{if s["sceneId"]!=scene["sceneId"]||g["originalId"]!=group["originalId"]||group["versions"].as_array().unwrap().iter().any(|v|!g["versions"].as_array().unwrap().contains(v)){return Err("Retained paragraph versions and originals are immutable. Create a new version.".into())}}
 }}Ok(())
}
pub(crate) async fn references(tx:&mut Transaction<'_,Sqlite>,pid:&str,payload:&Value)->Result<(),String>{
 let invalid:i64=sqlx::query_scalar("SELECT count(*) FROM json_each(?,'$.scenes') s,json_each(s.value,'$.groups') g WHERE NOT EXISTS(SELECT 1 FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=json_extract(s.value,'$.sceneId') AND r.id=json_extract(g.value,'$.sceneRevisionId') AND r.entity_type='scene')").bind(serde_json::to_string(payload).unwrap()).bind(pid).fetch_one(&mut **tx).await.map_err(err)?;
 if invalid!=0{return Err("Paragraph source revision does not belong to its saved scene and project.".into())}Ok(())
}
