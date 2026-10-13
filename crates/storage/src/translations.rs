use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct TranslationDraft {
    pub id:String, pub scene_id:String, pub scene_revision_id:String,
    pub source_language:String, pub target_language:String,
    pub source_text:String, pub text:String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct TranslationsDocument { pub schema_version:u32, pub drafts:Vec<TranslationDraft> }
fn language(s:&str)->bool { ["en","ar","zh-Hans","zh-Hant"].contains(&s) }
pub(crate) fn validate_payload(v:&Value)->Result<(),String>{
    let d:TranslationsDocument=serde_json::from_value(v.clone()).map_err(|_|"Invalid translations document.")?;
    if d.schema_version!=1||d.drafts.len()>1000{return Err("Invalid translations schema or draft count.".into())}
    let mut ids=HashSet::new();
    for x in d.drafts {
        if !crate::workspace::valid_id(&x.id)||!ids.insert(x.id)||!crate::workspace::valid_id(&x.scene_id)||!crate::workspace::valid_id(&x.scene_revision_id)||!language(&x.source_language)||!language(&x.target_language)||x.source_language==x.target_language||x.source_text.is_empty()||x.source_text.encode_utf16().count()>500000||x.text.encode_utf16().count()>500000{return Err("Invalid translation draft fields.".into())}
    }
    if serde_json::to_vec(v).map_err(|_|"Invalid translations document.")?.len()>8_000_000{return Err("Translations exceed 8 MB.".into())}
    Ok(())
}
pub(crate) fn transition(old:&Value,new:&Value)->Result<(),String>{
    let old:TranslationsDocument=serde_json::from_value(old.clone()).map_err(|_|"Invalid previous translations.")?;
    let new:TranslationsDocument=serde_json::from_value(new.clone()).map_err(|_|"Invalid translations.")?;
    let by_id=new.drafts.iter().map(|d|(&d.id,d)).collect::<std::collections::HashMap<_,_>>();
    for prior in old.drafts {
        let next=by_id.get(&prior.id).ok_or("Saved translation drafts are retained; create a new draft for changed source wording.")?;
        if prior.scene_id!=next.scene_id||prior.scene_revision_id!=next.scene_revision_id||prior.source_language!=next.source_language||prior.target_language!=next.target_language||prior.source_text!=next.source_text{return Err("Translation source fields are immutable; create a new draft for changed source wording.".into())}
    }
    Ok(())
}
pub(crate) async fn validate_sources(tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,pid:&str,v:&Value)->Result<(),String>{
    let doc:TranslationsDocument=serde_json::from_value(v.clone()).map_err(|_|"Invalid translations.")?;
    for d in doc.drafts {
        let payload:Option<String>=sqlx::query_scalar("SELECT payload FROM revision_envelope WHERE id=? AND project_id=? AND entity_id=? AND entity_type='scene'").bind(&d.scene_revision_id).bind(pid).bind(&d.scene_id).fetch_optional(&mut **tx).await.map_err(crate::workspace::err)?;
        let payload=payload.ok_or("Translation source revision does not belong to this scene and project.")?;
        let scene:crate::Scene=serde_json::from_str(&payload).map_err(|_|"Invalid translation source scene revision.")?;
        if crate::workspace::plain_text(&scene.document)!=d.source_text{return Err("Translation source text does not match its immutable scene revision.".into())}
    }
    Ok(())
}
