//! Local notebook aids, stored together as revisioned project data.
use crate::workspace::{err,valid_id};
use serde_json::{json,Value};
use sqlx::{Sqlite,Transaction};
use std::collections::HashSet;

pub fn initial()->Value { json!({"schemaVersion":1,"scraps":[],"reactions":[],"bookmarks":[],"notes":[]}) }
fn text(v:&Value,key:&str,max:usize)->bool { v[key].as_str().is_some_and(|s|!s.is_empty()&&s.chars().count()<=max) }
fn identity(v:&Value,key:&str)->bool { v[key].as_str().is_some_and(valid_id) }
pub(crate) fn validate_payload(payload:&Value)->Result<(),String> {
    let mut ids=HashSet::new();
    let mut bookmarks=HashSet::new();
    for key in ["scraps","reactions","bookmarks","notes"] {
        for item in payload[key].as_array().filter(|a|a.len()<=20000).ok_or_else(||format!("Invalid notebook {key} collection."))? {
            if !identity(item,"id")||!identity(item,"sceneId")||!ids.insert(item["id"].as_str().unwrap()) { return Err("Invalid or duplicate notebook entry identity.".into()) }
            match key {
                "scraps"=>if !text(item,"text",2_000_000) {return Err("Invalid notebook scrap text.".into())},
                "notes"=>if !text(item,"text",100000)||!item["resolved"].is_boolean() {return Err("Invalid notebook note.".into())},
                "reactions"=>if !identity(item,"revisionId")||!text(item,"quote",2_000_000)||!item["start"].as_u64().is_some_and(|n|n<=2_000_000)||![Some("bored"),Some("confused"),Some("curious")].contains(&item["kind"].as_str())||!item["resolved"].is_boolean() {return Err("Invalid notebook reading reaction.".into())},
                "bookmarks"=>{
                    if !text(item,"note",100000)||!bookmarks.insert(item["sceneId"].as_str().unwrap()) {return Err("Invalid or duplicate scene return bookmark.".into())}
                    let location=&item["location"];
                    if !location["documentHash"].as_str().is_some_and(|s|s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit())) {return Err("Invalid return bookmark document hash.".into())}
                    for point in ["cursor","anchor"] {if !location[point].as_array().is_some_and(|a|a.len()==2&&a.iter().all(|v|v.as_u64().is_some_and(|n|n<=2_000_000))) {return Err("Invalid return bookmark location.".into())}}
                },
                _=>unreachable!()
            }
        }
    }
    Ok(())
}
pub(crate) fn validate_reaction_source(reaction:&Value,source:&crate::Revision)->Result<(),String> {
    if source.entity_type!="scene"||source.entity_id!=reaction["sceneId"].as_str().unwrap() {return Err("Notebook reaction source does not belong to its scene.".into())}
    let text=crate::plain_text(&source.payload["document"]);
    let start=reaction["start"].as_u64().unwrap() as usize;
    let quote=reaction["quote"].as_str().unwrap();
    if !text.chars().skip(start).take(quote.chars().count()).eq(quote.chars()) {return Err("Notebook reaction quote does not match its saved scene revision.".into())}
    Ok(())
}
pub(crate) async fn references(tx:&mut Transaction<'_,Sqlite>,pid:&str,payload:&Value)->Result<(),String> {
    for reaction in payload["reactions"].as_array().unwrap() {
        let row=sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.id=? AND r.project_id=? AND r.entity_type='scene'").bind(reaction["revisionId"].as_str().unwrap()).bind(pid).fetch_optional(&mut **tx).await.map_err(err)?.ok_or("Notebook reaction source does not belong to this project.")?;
        let source=crate::workspace::revision(row)?;
        if crate::workspace::hash(serde_json::to_string(&source.payload).unwrap().as_bytes())!=source.checksum {return Err("Notebook reaction source checksum failed.".into())}
        validate_reaction_source(reaction,&source)?;
    }
    Ok(())
}

/// Recover text removed between two saved documents, preserving Unicode exactly.
/// Small edited spans use a bounded scalar diff. Very large rewrites retain the
/// complete changed old span (which can include unchanged words), keeping work
/// linear outside a fixed 250,000-cell comparison budget and losing no old text.
pub fn removed_passages(old_doc:&Value,new_doc:&Value)->Vec<String> {
    let old=crate::plain_text(old_doc);let new=crate::plain_text(new_doc);
    let a:Vec<char>=old.chars().collect();let b:Vec<char>=new.chars().collect();
    let prefix=a.iter().zip(&b).take_while(|(a,b)|a==b).count();
    let suffix=a[prefix..].iter().rev().zip(b[prefix..].iter().rev()).take_while(|(a,b)|a==b).count();
    let a=&a[prefix..a.len()-suffix];let b=&b[prefix..b.len()-suffix];
    if a.is_empty(){return vec![]}
    if b.is_empty()||a.len().saturating_mul(b.len())>250000 {return vec![a.iter().collect()]}
    let width=b.len()+1;
    let mut lengths=vec![0u32;(a.len()+1)*width];
    for i in (0..a.len()).rev(){for j in (0..b.len()).rev(){lengths[i*width+j]=if a[i]==b[j]{1+lengths[(i+1)*width+j+1]}else{lengths[(i+1)*width+j].max(lengths[i*width+j+1])};}}
    let(mut i,mut j,mut cuts,mut cut)=(0,0,Vec::new(),String::new());
    while i<a.len(){
        if j<b.len()&&a[i]==b[j]{if !cut.is_empty(){cuts.push(std::mem::take(&mut cut))}i+=1;j+=1;}
        else if j<b.len()&&lengths[i*width+j+1]>lengths[(i+1)*width+j]{j+=1;}
        else{cut.push(a[i]);i+=1;}
    }
    if !cut.is_empty(){cuts.push(cut)}
    cuts
}

#[cfg(test)] mod tests {
    use super::*;
    fn doc(t:&str)->Value{json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":t}]}]})}
    #[test] fn cuts_preserve_unicode_and_distinguish_insertions(){
        assert_eq!(removed_passages(&doc("Hello 世界 😀!"),&doc("Hello !")),vec!["世界 😀"]);
        assert_eq!(removed_passages(&doc("مَرْحَبًا"),&doc("مَرْحَبًا 世界")),Vec::<String>::new());
        assert_eq!(removed_passages(&doc("ab XY cd ZZ ef"),&doc("ab cd ef")),vec!["XY "," ZZ"]);
        assert!(removed_passages(&doc("a b c"),&doc("a NEW b OTHER c")).is_empty());
        let mut marked=doc("same");marked["content"][0]["content"][0]["marks"]=json!([{"type":"bold"}]);assert!(removed_passages(&doc("same"),&marked).is_empty());
    }
    #[test] fn large_rewrite_has_bounded_comparison_and_keeps_old_span(){
        let old="界".repeat(200000);let new="م".repeat(200000);assert_eq!(removed_passages(&doc(&old),&doc(&new)),vec![old]);
    }
}
