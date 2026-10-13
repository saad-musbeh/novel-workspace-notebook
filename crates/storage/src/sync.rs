//! Bounded replication primitives. Authentication, project authorization and transport
//! belong to the companion; this module never opens a listener or sends manuscript data.
use crate::{
    Storage,
    Scene,
    Snapshot,
    ProjectBundle,
    Revision,
    ProjectAsset,
    validate
};
use crate::workspace::{
    hash,
    id,
    err,
    revision,
    append,
    title,
    validate_kind,
    validate_data,
    validate_data_transition,
    valid_id,
    safe_path,
    validate_asset,
    validate_scene_links
};
use serde::{
    Serialize,
    Deserialize
};
use serde_json::{
    Value,
    json
};
use sqlx::{
    Row,
    Sqlite,
    Transaction
};
use std::collections::HashSet;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct SyncCursor {
    pub epoch:String,
    pub sequence:i64
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct RevisionPage {
    pub epoch:String,
    pub next_sequence:i64,
    pub revisions:Vec<Revision>,
    pub has_more:bool
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct RevisionHeads {
    pub entity_id:String,
    pub entity_type:String,
    pub revision_ids:Vec<String>
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MetadataPlacementState {
    pub scene_id:String,
    pub chapter_id:String,
    pub position:i64,
    pub deleted:bool,
    pub heads:Vec<String>,
}
/// One coherent read binds an author's metadata preview to its ordering dependencies.
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MetadataResolutionSnapshot {
    pub project_id:String,
    pub entity_id:String,
    pub entity_type:String,
    pub heads:Vec<String>,
    pub projection:Value,
    pub chapter_order:Vec<String>,
    pub placement_heads:Vec<MetadataPlacementState>,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct IngestResult {
    pub inserted:usize,
    pub duplicates:usize,
    pub heads:Vec<RevisionHeads>
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct AssetTransferSpec {
    pub asset_id:String,
    pub path:String,
    pub mime_type:String,
    pub total_size:usize,
    pub checksum:String
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct AssetChunk {
    pub asset_id:String,
    pub checksum:String,
    pub offset:usize,
    pub total_size:usize,
    pub bytes:Vec<u8>
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase")]
pub struct AssetTransferStatus {
    pub transfer_id:String,
    pub received:usize,
    pub total_size:usize,
    pub checksum:String
}
async fn epoch(tx:&mut Transaction<'_,Sqlite>)->Result<String,
String>{
    sqlx::query_scalar("SELECT epoch FROM storage_identity WHERE singleton=1").fetch_one(&mut **tx).await.map_err(err)
}
async fn leaves(tx:&mut Transaction<'_,Sqlite>,sid:&str)->Result<Vec<(String,String)>,
String>{
    sqlx::query_as("SELECT r.id,r.entity_type FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.entity_id=? AND r.entity_type IN('scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.entity_id=r.entity_id AND c.entity_type IN('scene','tombstone') AND p.value=r.id) ORDER BY m.sequence").bind(sid).fetch_all(&mut **tx).await.map_err(err)
}
pub(crate) async fn recompute_scene_heads(tx:&mut Transaction<'_,Sqlite>,sid:&str)->Result<(),
String>{
    let heads=leaves(tx,sid).await?;
    let contents=heads.iter().filter(|(_,kind)|kind=="scene").collect::<Vec<_>>();
    if contents.is_empty(){
        sqlx::query("UPDATE manuscript_scene SET deleted=1 WHERE id=?").bind(sid).execute(&mut **tx).await.map_err(err)?;
    }
    else{
        let current:String=sqlx::query_scalar("SELECT head FROM manuscript_scene WHERE id=?").bind(sid).fetch_one(&mut **tx).await.map_err(err)?;
        let selected=if contents.iter().any(|r|r.0==current){
            current
        }
        else{
            contents[0].0.clone()
        };
        sqlx::query("UPDATE manuscript_scene SET head=?,deleted=0 WHERE id=?").bind(&selected).bind(sid).execute(&mut **tx).await.map_err(err)?;
        // Keep the original prototype projection consistent for older close/reopen consumers.
        if sid=="prototype-scene" {
            let (payload,parent):(String,String)=sqlx::query_as("SELECT payload,parents FROM revision_envelope WHERE id=?").bind(&selected).fetch_one(&mut **tx).await.map_err(err)?;
            let parents:Vec<String>=serde_json::from_str(&parent).map_err(|_|"Invalid revision parents.")?;
            let legacy_parent=if let Some(p)=parents.first(){
                sqlx::query_scalar::<_,
                String>("SELECT id FROM scene_revision WHERE id=?").bind(p).fetch_optional(&mut **tx).await.map_err(err)?
            }
            else{
                None
            };
            sqlx::query("INSERT OR IGNORE INTO scene_revision(id,parent,payload) VALUES(?,?,?)").bind(&selected).bind(legacy_parent).bind(payload).execute(&mut **tx).await.map_err(err)?;
            sqlx::query("UPDATE scene_head SET revision_id=? WHERE singleton=1").bind(&selected).execute(&mut **tx).await.map_err(err)?;
        }
    }
    Ok(())
}
impl Storage {
    /// Read and verify one immutable asset, never all project BLOBs.
    pub async fn load_asset(&self,pid:&str,asset_id:&str)->Result<Option<ProjectAsset>,String>{
        if !valid_id(pid)||!valid_id(asset_id){return Err("Invalid project or asset identity.".into())}
        let row=sqlx::query("SELECT id,path,mime_type,checksum,length(content) total_size,CASE WHEN length(content)<=16777216 THEN content END content FROM project_asset WHERE project_id=? AND id=?").bind(pid).bind(asset_id).fetch_optional(&self.pool).await.map_err(err)?;
        let Some(row)=row else{return Ok(None)};
        let size:i64=row.get("total_size");
        if !(0..=16*1024*1024).contains(&size){return Err("Stored asset exceeds its size limit.".into())}
        let asset=ProjectAsset{id:row.get("id"),path:row.get("path"),mime_type:row.get("mime_type"),checksum:row.get("checksum"),content:row.get("content")};
        validate_asset(&asset)?;
        Ok(Some(asset))
    }
    /// Bounded SQL BLOB slice. The completed destination's full checksum remains
    /// authoritative; individual chunks carry the immutable expected checksum.
    pub async fn read_asset_chunk(&self,pid:&str,asset_id:&str,offset:usize,limit:usize)->Result<AssetChunk,String>{
        if !valid_id(pid)||!valid_id(asset_id)||offset>16*1024*1024||limit==0||limit>262144{return Err("Invalid asset identity, offset or chunk limit (1–262144).".into())}
        let row=sqlx::query("SELECT id,path,mime_type,checksum,length(content) total_size,CASE WHEN length(content)<=16777216 AND ?<=length(content) THEN substr(content,?,?) END bytes FROM project_asset WHERE project_id=? AND id=?").bind(offset as i64).bind(offset as i64+1).bind(limit as i64).bind(pid).bind(asset_id).fetch_optional(&self.pool).await.map_err(err)?.ok_or("Asset not found in this project.")?;
        let size:i64=row.get("total_size");
        if !(0..=16*1024*1024).contains(&size){return Err("Stored asset exceeds its size limit.".into())}
        let spec=AssetTransferSpec{asset_id:row.get("id"),path:row.get("path"),mime_type:row.get("mime_type"),checksum:row.get("checksum"),total_size:size as usize};
        validate_asset_spec(&spec)?;
        if offset>spec.total_size{return Err("Asset offset exceeds its verified size.".into())}
        let bytes:Vec<u8>=row.get("bytes");
        if bytes.len()!=limit.min(spec.total_size-offset){return Err("Stored asset chunk has an invalid size.".into())}
        Ok(AssetChunk{asset_id:spec.asset_id,checksum:spec.checksum,offset,total_size:spec.total_size,bytes})
    }
    /// Number of distinct validated envelopes received from this remote epoch
    /// for this project. Receipts commit with ingest, including duplicate
    /// envelopes, and survive consistent backups until the local epoch rotates.
    pub async fn sync_receipt_count(&self,pid:&str,remote_epoch:&str)->Result<i64,String>{
        if !valid_id(pid)||!valid_id(remote_epoch){
            return Err("Invalid project identity or remote sync epoch.".into())
        }
        sqlx::query_scalar("SELECT count(*) FROM sync_receipt s JOIN revision_envelope r ON r.id=s.revision_id WHERE r.project_id=? AND s.remote_epoch=?").bind(pid).bind(remote_epoch).fetch_one(&self.pool).await.map_err(err)
    }
    pub async fn sync_cursor(&self)->Result<SyncCursor,
    String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let epoch=epoch(&mut tx).await?;
        let sequence:i64=sqlx::query_scalar("SELECT coalesce(max(sequence),0) FROM change_marker").fetch_one(&mut *tx).await.map_err(err)?;
        Ok(SyncCursor{
            epoch,sequence
        }
        )
    }
    pub async fn rotate_epoch(&self)->Result<SyncCursor,
    String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let epoch=id();
        sqlx::query("UPDATE storage_identity SET epoch=? WHERE singleton=1").bind(&epoch).execute(&mut *tx).await.map_err(err)?;
        sqlx::query("DELETE FROM sync_receipt").execute(&mut *tx).await.map_err(err)?;
        let sequence=sqlx::query_scalar("SELECT coalesce(max(sequence),0) FROM change_marker").fetch_one(&mut *tx).await.map_err(err)?;
        tx.commit().await.map_err(err)?;
        Ok(SyncCursor{
            epoch,sequence
        }
        )
    }
    pub async fn pull_revisions(&self,pid:&str,cursor:SyncCursor,limit:u32)->Result<RevisionPage,
    String>{
        if cursor.sequence<0||limit==0||limit>1000{
            return Err("Invalid sync cursor or page size (1–1000).".into())
        }
        let mut tx=self.pool.begin().await.map_err(err)?;
        let current_epoch=epoch(&mut tx).await?;
        if cursor.epoch!=current_epoch{
            return Err("Sync epoch changed. Re-pair or explicitly bootstrap; the old cursor is invalid.".into())
        }
        let exists:i64=sqlx::query_scalar("SELECT count(*) FROM project WHERE id=?").bind(pid).fetch_one(&mut *tx).await.map_err(err)?;
        if exists!=1{
            return Err("Project not found.".into())
        }
        let max:i64=sqlx::query_scalar("SELECT coalesce(max(sequence),0) FROM change_marker").fetch_one(&mut *tx).await.map_err(err)?;
        if cursor.sequence>max{
            return Err("Sync cursor is ahead of this hub. Explicit bootstrap is required.".into())
        }
        let rows=sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.project_id=? AND m.sequence>? ORDER BY m.sequence LIMIT ?").bind(pid).bind(cursor.sequence).bind(limit+1).fetch_all(&mut *tx).await.map_err(err)?;
        let mut revisions=rows.into_iter().map(revision).collect::<Result<Vec<_>,
        _>>()?;
        let mut has_more=revisions.len()>limit as usize;
        if has_more{
            revisions.pop();
        }
        let mut bytes=0;
        let mut bounded=vec![];
        for r in revisions{
            let size=serde_json::to_vec(&r).map_err(|_|"Invalid revision.")?.len();
            if bytes+size>8_000_000{
                has_more=true;
                break;
            }
            bytes+=size;
            bounded.push(r);
        }
        if bounded.is_empty()&&has_more{
            return Err("A revision exceeds the 8 MB sync page limit. Use a verified project archive.".into())
        }
        let next_sequence=if !has_more{
            max
        }
        else{
            bounded.last().map(|r|r.sequence).unwrap_or(cursor.sequence)
        };
        Ok(RevisionPage{
            epoch:current_epoch,next_sequence,revisions:bounded,has_more
        }
        )
    }
    pub async fn scene_heads(&self,sid:&str)->Result<RevisionHeads,
    String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let revision_ids=leaves(&mut tx,sid).await?.into_iter().map(|r|r.0).collect();
        Ok(RevisionHeads{
            entity_id:sid.into(),entity_type:"scene".into(),revision_ids
        }
        )
    }
    pub async fn delete_scene(&self,sid:&str,expected_head:&str)->Result<String,
    String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let (head,pid,deleted):(String,String,i64)=sqlx::query_as("SELECT s.head,c.project_id,s.deleted FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(sid).fetch_one(&mut *tx).await.map_err(err)?;
        if head!=expected_head||deleted!=0{
            return Err("Conflict: scene changed or is already deleted.".into())
        }
        let rid=id();
        append(&mut tx,&pid,sid,"tombstone",vec![head],json!({
            "deleted":true
        }
        ),"delete",&rid).await?;
        recompute_scene_heads(&mut tx,sid).await?;
        tx.commit().await.map_err(err)?;
        Ok(rid)
    }
    pub async fn restore_deleted_scene(&self,sid:&str,scene:Scene,expected_heads:Vec<String>)->Result<Snapshot,
    String>{
        validate(&scene)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        let heads=leaves(&mut tx,sid).await?;
        let actual=heads.iter().map(|r|r.0.clone()).collect::<HashSet<_>>();
        if expected_heads.is_empty()||expected_heads.iter().collect::<HashSet<_>>().len()!=expected_heads.len()||actual!=expected_heads.iter().cloned().collect(){
            return Err("Conflict: deleted scene heads changed before restore.".into())
        }
        if !heads.iter().any(|r|r.1=="tombstone"){
            return Err("This scene has no deletion to restore.".into())
        }
        let pid:String=sqlx::query_scalar("SELECT c.project_id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(sid).fetch_one(&mut *tx).await.map_err(err)?;
        let rid=id();
        append(&mut tx,&pid,sid,"scene",expected_heads,serde_json::to_value(&scene).unwrap(),"restoreDeleted",&rid).await?;
        sqlx::query("UPDATE manuscript_scene SET head=?,deleted=0 WHERE id=?").bind(&rid).bind(sid).execute(&mut *tx).await.map_err(err)?;
        recompute_scene_heads(&mut tx,sid).await?;
        tx.commit().await.map_err(err)?;
        Ok(Snapshot{
            revision_id:rid,scene
        }
        )
    }
    pub async fn ingest_revisions(&self,pid:&str,expected_local_epoch:&str,remote_epoch:&str,manifest:ProjectBundle,incoming:Vec<Revision>)->Result<IngestResult,
    String>{
        if !valid_id(pid)||!valid_id(remote_epoch)||incoming.len()>1000||serde_json::to_vec(&incoming).map_err(|_|"Invalid revision batch.")?.len()>8_000_000||serde_json::to_vec(&manifest).map_err(|_|"Invalid project manifest.")?.len()>16_000_000{
            return Err("Invalid or oversized replication batch.".into())
        }
        if manifest.project.id!=pid{
            return Err("Replication manifest belongs to a different project.".into())
        }
        title(&manifest.project.title)?;
        let mut chapter_ids=HashSet::new();
        let mut scene_ids=HashSet::new();
        let mut identities=HashSet::from([pid.to_string()]);
        for c in &manifest.chapters{
            title(&c.title)?;
            if c.project_id!=pid||!valid_id(&c.id)||c.position<0||(!chapter_ids.insert(c.id.clone())||!identities.insert(c.id.clone())){
                return Err("Invalid replica chapter metadata.".into())
            }
        }
        for s in &manifest.scenes{
            validate(&s.scene)?;
            if !valid_id(&s.id)||s.position<0||!chapter_ids.contains(&s.chapter_id)||(!scene_ids.insert(s.id.clone())||!identities.insert(s.id.clone())){
                return Err("Invalid replica scene placement.".into())
            }
        }
        let mut tx=self.pool.begin().await.map_err(err)?;
        if epoch(&mut tx).await?!=expected_local_epoch{
            return Err("Local sync epoch changed. Bootstrap is required.".into())
        }
        sqlx::query("INSERT OR IGNORE INTO project(id,title,created_at) VALUES(?,?,?)").bind(pid).bind(&manifest.project.title).bind(&manifest.project.created_at).execute(&mut *tx).await.map_err(err)?;
        for c in &manifest.chapters{
            let owner:Option<String>=sqlx::query_scalar("SELECT project_id FROM chapter WHERE id=?").bind(&c.id).fetch_optional(&mut *tx).await.map_err(err)?;
            if owner.as_deref().is_some_and(|owner|owner!=pid){
                return Err("Chapter identity is already owned by another project.".into())
            }
            sqlx::query("INSERT OR IGNORE INTO chapter(id,project_id,title,position) VALUES(?,?,?,?)").bind(&c.id).bind(pid).bind(&c.title).bind(c.position).execute(&mut *tx).await.map_err(err)?;
        }
        let local_scenes:Vec<String>=sqlx::query_scalar("SELECT s.id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=?").bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
        scene_ids.extend(local_scenes);
        let local_chapters:Vec<String>=sqlx::query_scalar("SELECT id FROM chapter WHERE project_id=?").bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
        chapter_ids.extend(local_chapters);
        let mut seen=HashSet::new();
        let mut pending=vec![];
        let mut duplicates=0;
        let mut affected=HashSet::new();
        let mut pack_versions=None;
        for r in incoming {
            if !valid_id(&r.id)||r.project_id!=pid||r.schema_version!=1||r.parents.len()>64||r.parents.iter().collect::<HashSet<_>>().len()!=r.parents.len()||r.parents.contains(&r.id)||r.created_at.is_empty()||r.created_at.len()>64||!valid_id(&r.origin_epoch)||!valid_id(&r.origin_device)||r.sequence<1||!seen.insert(r.id.clone())||hash(serde_json::to_string(&r.payload).unwrap().as_bytes())!=r.checksum{
                return Err("Invalid revision envelope, origin, duplicate identity or payload hash.".into())
            }
            match r.entity_type.as_str(){
                "scene"=>{
                    if !scene_ids.contains(&r.entity_id){
                        return Err("Replica scene is not in the project manifest.".into())
                    }
                    let scene:Scene=serde_json::from_value(r.payload.clone()).map_err(|_|"Invalid scene revision.")?;
                    validate(&scene)?;
                },
                "tombstone"=>{
                    if !scene_ids.contains(&r.entity_id)||r.payload!=json!({
                        "deleted":true
                    }
                    ){
                        return Err("Invalid deletion revision.".into())
                    }
                },
                "projectData"=>{
                    validate_kind(&r.operation)?;
                    validate_data(&r.operation,&r.payload)?;
                    if chapter_ids.contains(&r.entity_id)||scene_ids.contains(&r.entity_id)||r.entity_id!=format!("{pid}:{}",r.operation){
                        return Err("Project data identity is outside the authorized project.".into())
                    }
                    if r.operation=="languagePacks"{
                        if pack_versions.is_none(){pack_versions=Some(crate::native_pack_versions::checked_history(&mut tx,pid).await?);}
                        pack_versions.as_mut().unwrap().observe(&r.payload)?;
                    }
                    if ["planning","writingSystems","sceneMetadata","paragraphAlternatives","notebookTools"].contains(&r.operation.as_str()){
                        validate_scene_links(&r.payload,&scene_ids)?;
                    }
                },
                "project"=>{
                    if r.entity_id!=pid{
                        return Err("Wrong project metadata identity.".into())
                    }
                },
                "chapter"=>{
                    if !chapter_ids.contains(&r.entity_id){
                        return Err("Unknown chapter metadata identity.".into())
                    }
                },
                "scenePlacement"=>{
                    if !scene_ids.contains(&r.entity_id)||!r.payload.get("chapterId").and_then(Value::as_str).is_some_and(|c|chapter_ids.contains(c)){
                        return Err("Invalid scene placement revision.".into())
                    }
                },
                "asset"=>{
                    if r.entity_id==pid||chapter_ids.contains(&r.entity_id)||scene_ids.contains(&r.entity_id)||["planning","writingSystems","languagePacks","settings","assistanceJobs","sceneMetadata","paragraphAlternatives","sceneTranslations","branchLineage","notebookTools"].iter().any(|kind|r.entity_id==format!("{pid}:{kind}")){
                        return Err("Asset identity collides with a story object.".into())
                    }
                    let spec=spec_from_payload(&r.payload)?;
                    validate_asset_spec(&spec)?;
                    let old:Option<String>=sqlx::query_scalar("SELECT payload FROM revision_envelope WHERE project_id=? AND entity_id=? AND entity_type='asset' LIMIT 1").bind(pid).bind(&r.entity_id).fetch_optional(&mut *tx).await.map_err(err)?;
                    if let Some(old)=old{
                        let old:Value=serde_json::from_str(&old).map_err(|_|"Invalid stored asset descriptor.")?;
                        if spec_from_payload(&old)?!=spec{
                            return Err("Asset identity was reused for a different immutable descriptor.".into())
                        }
                    }
                    if r.payload.get("id").and_then(Value::as_str)!=Some(&r.entity_id){
                        return Err("Asset revision identity mismatch.".into())
                    }
                },
                _=>return Err("Unsupported replicated revision type.".into())
            }
            affected.insert((r.entity_id.clone(),r.entity_type.clone()));
            let existing=sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.id=?").bind(&r.id).fetch_optional(&mut *tx).await.map_err(err)?;
            if let Some(existing)=existing{
                let existing=revision(existing)?;
                if existing.project_id!=r.project_id||existing.entity_id!=r.entity_id||existing.entity_type!=r.entity_type||existing.parents!=r.parents||existing.payload!=r.payload||existing.operation!=r.operation||existing.schema_version!=r.schema_version||existing.checksum!=r.checksum||existing.created_at!=r.created_at||existing.origin_epoch!=r.origin_epoch||existing.origin_device!=r.origin_device{
                    return Err("A revision identity was reused for different immutable content.".into())
                }
                sqlx::query("INSERT OR IGNORE INTO sync_receipt(remote_epoch,revision_id) VALUES(?,?)").bind(remote_epoch).bind(&r.id).execute(&mut *tx).await.map_err(err)?;
                duplicates+=1;
            }
            else{
                pending.push(r);
            }
        }
        let mut inserted=0;
        let mut alternative_refs=Vec::new();
        let mut translation_refs=Vec::new();
        let mut notebook_refs=Vec::new();
        while !pending.is_empty(){
            let mut ready=None;
            for (i,r) in pending.iter().enumerate(){
                let mut available=true;
                for p in &r.parents{
                    let owner:Option<(String,String)>=sqlx::query_as("SELECT project_id,entity_id FROM revision_envelope WHERE id=?").bind(p).fetch_optional(&mut *tx).await.map_err(err)?;
                    if let Some((project,entity))=owner{
                        if project!=pid||entity!=r.entity_id{
                            return Err("A revision parent belongs to a different project/object.".into())
                        }
                    }
                    else{
                        available=false;
                        break;
                    }
                }
                if available{
                    ready=Some(i);
                    break;
                }
            }
            let r=pending.remove(ready.ok_or("Missing revision parents or cyclic revision graph; fetch ancestors before retrying.")?);
            if r.entity_type=="projectData"{
                for parent in &r.parents{
                    let old:String=sqlx::query_scalar("SELECT payload FROM revision_envelope WHERE id=?").bind(parent).fetch_one(&mut *tx).await.map_err(err)?;
                    let old:Value=serde_json::from_str(&old).map_err(|_|"Invalid prior project data.")?;
                    validate_data_transition(&r.operation,&old,&r.payload)?;
                }
            }
            if r.entity_type=="projectData"&&r.operation=="paragraphAlternatives"{alternative_refs.push(r.payload.clone());}
            if r.entity_type=="projectData"&&r.operation=="sceneTranslations"{translation_refs.push(r.payload.clone());}
            if r.entity_type=="projectData"&&r.operation=="notebookTools"{notebook_refs.push(r.payload.clone());}
            sqlx::query("INSERT INTO revision_envelope(id,project_id,entity_id,entity_type,parents,payload,operation,schema_version,checksum,created_at,origin_epoch,origin_device) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)").bind(&r.id).bind(pid).bind(&r.entity_id).bind(&r.entity_type).bind(serde_json::to_string(&r.parents).unwrap()).bind(serde_json::to_string(&r.payload).unwrap()).bind(&r.operation).bind(r.schema_version as i64).bind(&r.checksum).bind(&r.created_at).bind(&r.origin_epoch).bind(&r.origin_device).execute(&mut *tx).await.map_err(err)?;
            sqlx::query("INSERT INTO change_marker(revision_id) VALUES(?)").bind(&r.id).execute(&mut *tx).await.map_err(err)?;
            sqlx::query("INSERT OR IGNORE INTO sync_receipt(remote_epoch,revision_id) VALUES(?,?)").bind(remote_epoch).bind(&r.id).execute(&mut *tx).await.map_err(err)?;
            inserted+=1;
        }
        for value in alternative_refs{crate::paragraph_alternatives::references(&mut tx,pid,&value).await?;}
        for value in translation_refs{crate::translations::validate_sources(&mut tx,pid,&value).await?;}
        for value in notebook_refs{crate::notebook_tools::references(&mut tx,pid,&value).await?;}
        for s in &manifest.scenes{
            let owner:Option<String>=sqlx::query_scalar("SELECT c.project_id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(&s.id).fetch_optional(&mut *tx).await.map_err(err)?;
            if owner.as_deref().is_some_and(|p|p!=pid){
                return Err("Scene identity is already owned by another project.".into())
            }
            let head:Option<String>=sqlx::query_scalar("SELECT r.id FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.entity_id=? AND r.project_id=? AND r.entity_type='scene' ORDER BY m.sequence LIMIT 1").bind(&s.id).bind(pid).fetch_optional(&mut *tx).await.map_err(err)?;
            if let Some(head)=head{
                sqlx::query("INSERT OR IGNORE INTO manuscript_scene(id,chapter_id,position,head) VALUES(?,?,?,?)").bind(&s.id).bind(&s.chapter_id).bind(s.position).bind(head).execute(&mut *tx).await.map_err(err)?;
            }
        }
        let mut heads=vec![];
        let mut metadata_updates=vec![];
        let mut handled=HashSet::new();
        for(entity,kind)in affected{
            let normalized=if kind=="tombstone"{
                "scene"
            }
            else{
                &kind
            };
            if !handled.insert((entity.clone(),normalized.to_string())){
                continue
            }
            let revision_ids=if normalized=="scene"{
                recompute_scene_heads(&mut tx,&entity).await?;
                leaves(&mut tx,&entity).await?.into_iter().map(|r|r.0).collect::<Vec<_>>()
            }
            else{
                entity_leaves(&mut tx,pid,&entity,normalized).await?
            };
            if normalized=="projectData"&&revision_ids.len()==1{
                let kind:String=sqlx::query_scalar("SELECT operation FROM revision_envelope WHERE id=?").bind(&revision_ids[0]).fetch_one(&mut *tx).await.map_err(err)?;
                if kind=="universeCatalogue"{let payload:String=sqlx::query_scalar("SELECT payload FROM revision_envelope WHERE id=?").bind(&revision_ids[0]).fetch_one(&mut *tx).await.map_err(err)?;crate::universe::references(&mut tx,pid,&serde_json::from_str(&payload).map_err(|_|"Invalid universe catalogue.")?,true).await?;}
                sqlx::query("INSERT INTO project_data(project_id,kind,head) VALUES(?,?,?) ON CONFLICT(project_id,kind) DO UPDATE SET head=excluded.head").bind(pid).bind(kind).bind(&revision_ids[0]).execute(&mut *tx).await.map_err(err)?;
            }
            if revision_ids.len()==1&&["project","chapter","scenePlacement"].contains(&normalized){
                let sequence:i64=sqlx::query_scalar("SELECT sequence FROM change_marker WHERE revision_id=?").bind(&revision_ids[0]).fetch_one(&mut *tx).await.map_err(err)?;
                metadata_updates.push((sequence,entity.clone(),normalized.to_owned(),revision_ids[0].clone()));
            }
            heads.push(RevisionHeads{
                entity_id:entity,entity_type:normalized.into(),revision_ids
            }
            );
        }
        // Cross-chapter moves write placement followed by the two chapter orders.
        // HashSet iteration cannot determine the application order: an order could
        // otherwise refer to a scene whose placement has not been applied yet.
        metadata_updates.sort_by_key(|update|update.0);
        for (_,entity,kind,revision) in metadata_updates{
            // An order depending on a competing placement has no safe membership
            // to apply yet. Retain both histories and the explicit placement heads
            // for Recovery rather than choosing a chapter for the scene.
            if kind=="chapter" && order_has_placement_conflicts(&mut tx,pid,&revision).await?{continue;}
            apply_metadata(&mut tx,pid,&entity,&kind,&revision).await?;
        }
        // Keep order deterministic when concurrent creations use the same insertion position.
        normalize_positions(&mut tx,pid).await?;
        tx.commit().await.map_err(err)?;
        Ok(IngestResult{
            inserted,duplicates,heads
        }
        )
    }
    pub async fn metadata_heads(&self,pid:&str,entity:&str,kind:&str)->Result<RevisionHeads,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;validate_metadata_owner(&mut tx,pid,entity,kind).await?;
        let revision_ids=entity_leaves(&mut tx,pid,entity,kind).await?;Ok(RevisionHeads{entity_id:entity.into(),entity_type:kind.into(),revision_ids})
    }
    pub async fn metadata_resolution_snapshot(&self,pid:&str,entity:&str,kind:&str)->Result<MetadataResolutionSnapshot,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;let snapshot=metadata_resolution_capture(&mut tx,pid,entity,kind).await?;tx.commit().await.map_err(err)?;Ok(snapshot)
    }
    /// Resolve competing heads, or explicitly acknowledge an unapplied retained chapter order.
    /// A single-head acknowledgement can select the immutable payload or the exact
    /// captured current projection; it cannot silently edit either or pick a placement.
    pub async fn resolve_metadata_checked(&self,snapshot:&MetadataResolutionSnapshot,payload:Value)->Result<String,String>{
        validate_metadata_resolution_payload(&payload)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        let current=metadata_resolution_capture(&mut tx,&snapshot.project_id,&snapshot.entity_id,&snapshot.entity_type).await?;
        if &current!=snapshot{return Err("Conflict: metadata, chapter membership or scene placements changed after this preview. Reopen it before choosing a resolution.".into())}
        if snapshot.heads.is_empty(){return Err("This object has no retained metadata to resolve.".into())}
        if snapshot.heads.len()==1{
            if snapshot.entity_type!="chapter"||!payload.get("order").is_some_and(Value::is_array){return Err("Only a retained chapter order can be explicitly replayed with one head.".into())}
            let retained=verified_metadata_payload(&mut tx,&snapshot.project_id,&snapshot.entity_id,&snapshot.entity_type,&snapshot.heads[0]).await?;
            if retained==snapshot.projection{return Err("The retained chapter order is already applied.".into())}
            if payload!=retained&&payload!=snapshot.projection{return Err("Single-head acknowledgement must use the exact retained chapter version or captured current membership and order.".into())}
        }
        guard_metadata_placements(&mut tx,&snapshot.project_id,&snapshot.entity_id,&snapshot.entity_type,&payload).await?;
        let rid=id();append(&mut tx,&snapshot.project_id,&snapshot.entity_id,&snapshot.entity_type,snapshot.heads.clone(),payload,"resolve",&rid).await?;
        apply_metadata(&mut tx,&snapshot.project_id,&snapshot.entity_id,&snapshot.entity_type,&rid).await?;normalize_positions(&mut tx,&snapshot.project_id).await?;tx.commit().await.map_err(err)?;Ok(rid)
    }
    pub async fn resolve_metadata(&self,pid:&str,entity:&str,kind:&str,payload:Value,expected_heads:Vec<String>)->Result<String,String>{
        validate_metadata_resolution_payload(&payload)?;
        let mut tx=self.pool.begin().await.map_err(err)?;validate_metadata_owner(&mut tx,pid,entity,kind).await?;
        let actual=entity_leaves(&mut tx,pid,entity,kind).await?;
        if expected_heads.len()<2||expected_heads.iter().collect::<HashSet<_>>().len()!=expected_heads.len()||actual.iter().cloned().collect::<HashSet<_>>()!=expected_heads.iter().cloned().collect(){return Err("Conflict: metadata heads changed before resolution.".into())}
        guard_metadata_placements(&mut tx,pid,entity,kind,&payload).await?;
        let rid=id();append(&mut tx,pid,entity,kind,expected_heads,payload,"resolve",&rid).await?;apply_metadata(&mut tx,pid,entity,kind,&rid).await?;normalize_positions(&mut tx,pid).await?;tx.commit().await.map_err(err)?;Ok(rid)
    }
    pub async fn project_data_heads(&self,pid:&str,kind:&str)->Result<RevisionHeads,
    String>{
        validate_kind(kind)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        let entity=format!("{pid}:{kind}");
        let revision_ids=entity_leaves(&mut tx,pid,&entity,"projectData").await?;
        Ok(RevisionHeads{
            entity_id:entity,entity_type:"projectData".into(),revision_ids
        }
        )
    }
    pub async fn resolve_project_data(&self,pid:&str,kind:&str,payload:Value,expected_heads:Vec<String>)->Result<crate::ProjectDataSnapshot,
    String>{
        validate_kind(kind)?;
        validate_data(kind,&payload)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        if kind=="branchLineage"{return Err("Branch lineage cannot be changed through generic conflict resolution.".into())}
        let entity=format!("{pid}:{kind}");
        let heads=entity_leaves(&mut tx,pid,&entity,"projectData").await?;
        if expected_heads.is_empty()||heads.iter().cloned().collect::<HashSet<_>>()!=expected_heads.iter().cloned().collect()||expected_heads.iter().collect::<HashSet<_>>().len()!=expected_heads.len(){
            return Err("Conflict: project data heads changed before resolution.".into())
        }
        if ["planning","writingSystems","sceneMetadata","paragraphAlternatives","notebookTools"].contains(&kind){
            let scenes:Vec<String>=sqlx::query_scalar("SELECT s.id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=?").bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
            validate_scene_links(&payload,&scenes.into_iter().collect())?;
        }
        if kind=="languagePacks"{crate::native_pack_versions::validate_candidate(&mut tx,pid,&payload).await?;}
        let mut previous=vec![];
        for head in &expected_heads{
            let (old,checksum):(String,String)=sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=? AND project_id=? AND entity_id=? AND entity_type='projectData'").bind(head).bind(pid).bind(&entity).fetch_one(&mut *tx).await.map_err(err)?;
            if hash(old.as_bytes())!=checksum{return Err("Prior project data checksum failed. Stored data was preserved.".into())}
            let old:Value=serde_json::from_str(&old).map_err(|_|"Invalid prior project data.")?;
            validate_data_transition(kind,&old,&payload)?;
            previous.push(old);
        }
        if kind=="planning"{crate::planning_validation::validate_resolution(&previous.iter().collect::<Vec<_>>(),&payload)?;crate::manuscript_parts::validate_parts_transition(&mut tx,pid,&payload,&previous.iter().collect::<Vec<_>>()).await?;}
        if kind=="sceneTranslations"{crate::translations::validate_sources(&mut tx,pid,&payload).await?;}
        let rid=id();
        if kind=="universeCatalogue"{crate::universe::references(&mut tx,pid,&payload,false).await?;}
        if kind=="paragraphAlternatives"{crate::paragraph_alternatives::references(&mut tx,pid,&payload).await?;}
        if kind=="notebookTools"{crate::notebook_tools::references(&mut tx,pid,&payload).await?;}
        append(&mut tx,pid,&entity,"projectData",expected_heads,payload.clone(),kind,&rid).await?;
        sqlx::query("INSERT INTO project_data(project_id,kind,head) VALUES(?,?,?) ON CONFLICT(project_id,kind) DO UPDATE SET head=excluded.head").bind(pid).bind(kind).bind(&rid).execute(&mut *tx).await.map_err(err)?;
        tx.commit().await.map_err(err)?;
        Ok(crate::ProjectDataSnapshot{
            revision_id:rid,payload
        }
        )
    }
    pub async fn cancel_asset_transfer(&self,pid:&str,transfer_id:&str)->Result<(),
    String>{
        sqlx::query("DELETE FROM asset_transfer WHERE id=? AND project_id=?").bind(transfer_id).bind(pid).execute(&self.pool).await.map_err(err)?;
        Ok(())
    }
    pub async fn begin_asset_transfer(&self,pid:&str,spec:AssetTransferSpec)->Result<AssetTransferStatus,
    String>{
        validate_asset_spec(&spec)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        let existing=sqlx::query("SELECT id,path,mime_type,total_size,checksum,length(content) received FROM asset_transfer WHERE project_id=? AND asset_id=?").bind(pid).bind(&spec.asset_id).fetch_optional(&mut *tx).await.map_err(err)?;
        if let Some(row)=existing{
            if row.get::<String,
            _>("path")!=spec.path||row.get::<String,
            _>("mime_type")!=spec.mime_type||row.get::<i64,
            _>("total_size")!=spec.total_size as i64||row.get::<String,
            _>("checksum")!=spec.checksum{
                return Err("Transfer identity already has a different manifest.".into())
            }
            return Ok(AssetTransferStatus{
                transfer_id:row.get("id"),received:row.get::<i64,_>("received") as usize,total_size:spec.total_size,checksum:spec.checksum
            }
            )
        }
        let completed:Option<String>=sqlx::query_scalar("SELECT checksum FROM project_asset WHERE project_id=? AND id=?").bind(pid).bind(&spec.asset_id).fetch_optional(&mut *tx).await.map_err(err)?;
        if let Some(completed)=completed{
            let row=sqlx::query("SELECT path,mime_type,length(content) size FROM project_asset WHERE project_id=? AND id=?").bind(pid).bind(&spec.asset_id).fetch_one(&mut *tx).await.map_err(err)?;
            if completed!=spec.checksum||row.get::<String,
            _>("path")!=spec.path||row.get::<String,
            _>("mime_type")!=spec.mime_type||row.get::<i64,
            _>("size") as usize!=spec.total_size{
                return Err("Asset already exists with different immutable content.".into())
            }
            let transfer=id();
            sqlx::query("INSERT INTO completed_asset_transfer(id,project_id,asset_id) VALUES(?,?,?)").bind(&transfer).bind(pid).bind(&spec.asset_id).execute(&mut *tx).await.map_err(err)?;
            tx.commit().await.map_err(err)?;
            return Ok(AssetTransferStatus{
                transfer_id:transfer,received:spec.total_size,total_size:spec.total_size,checksum:spec.checksum
            }
            )
        }
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM asset_transfer WHERE project_id=?").bind(pid).fetch_one(&mut *tx).await.map_err(err)?;
        if count>=32{
            return Err("At most 32 incomplete transfers are permitted per project.".into())
        }
        let transfer=id();
        sqlx::query("INSERT INTO asset_transfer(id,project_id,asset_id,path,mime_type,total_size,checksum) VALUES(?,?,?,?,?,?,?)").bind(&transfer).bind(pid).bind(&spec.asset_id).bind(&spec.path).bind(&spec.mime_type).bind(spec.total_size as i64).bind(&spec.checksum).execute(&mut *tx).await.map_err(err)?;
        tx.commit().await.map_err(err)?;
        Ok(AssetTransferStatus{
            transfer_id:transfer,received:0,total_size:spec.total_size,checksum:spec.checksum
        }
        )
    }
    pub async fn append_asset_chunk(&self,pid:&str,transfer_id:&str,offset:usize,bytes:Vec<u8>)->Result<AssetTransferStatus,
    String>{
        if bytes.is_empty()||bytes.len()>262_144{
            return Err("Asset chunks must contain 1–262144 bytes.".into())
        }
        let mut tx=self.pool.begin().await.map_err(err)?;
        let row=sqlx::query("SELECT total_size,checksum,content FROM asset_transfer WHERE id=? AND project_id=?").bind(transfer_id).bind(pid).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Transfer not found for this project.")?;
        let total=row.get::<i64,
        _>("total_size") as usize;
        let mut content:Vec<u8>=row.get("content");
        let end=offset.checked_add(bytes.len()).ok_or("Invalid asset offset.")?;
        if end>total{
            return Err("Chunk exceeds declared asset size.".into())
        }
        if offset<content.len(){
            if end>content.len()||content[offset..end]!=bytes{
                return Err("Retried asset chunk differs from verified received bytes.".into())
            }
        }
        else if offset==content.len(){
            content.extend_from_slice(&bytes);
            sqlx::query("UPDATE asset_transfer SET content=? WHERE id=?").bind(&content).bind(transfer_id).execute(&mut *tx).await.map_err(err)?;
        }
        else{
            return Err("Asset chunk has a gap; resume at the returned received offset.".into())
        }
        let status=AssetTransferStatus{
            transfer_id:transfer_id.into(),
            received:content.len(),
            total_size:total,
            checksum:row.get("checksum")
        };
        tx.commit().await.map_err(err)?;
        Ok(status)
    }
    pub async fn finalize_asset_transfer(&self,pid:&str,transfer_id:&str)->Result<ProjectAsset,
    String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let completed=sqlx::query("SELECT a.id,a.path,a.mime_type,a.content,a.checksum FROM completed_asset_transfer t JOIN project_asset a ON a.project_id=t.project_id AND a.id=t.asset_id WHERE t.id=? AND t.project_id=?").bind(transfer_id).bind(pid).fetch_optional(&mut *tx).await.map_err(err)?;
        if let Some(row)=completed{
            return Ok(ProjectAsset{
                id:row.get("id"),path:row.get("path"),mime_type:row.get("mime_type"),content:row.get("content"),checksum:row.get("checksum")
            }
            )
        }
        let row=sqlx::query("SELECT asset_id,path,mime_type,total_size,checksum,content FROM asset_transfer WHERE id=? AND project_id=?").bind(transfer_id).bind(pid).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Transfer not found for this project.")?;
        let asset=ProjectAsset{
            id:row.get("asset_id"),
            path:row.get("path"),
            mime_type:row.get("mime_type"),
            content:row.get("content"),
            checksum:row.get("checksum")
        };
        if asset.content.len()!=row.get::<i64,
        _>("total_size") as usize{
            return Err("Asset transfer is incomplete.".into())
        }
        validate_asset(&asset)?;
        sqlx::query("INSERT INTO project_asset(project_id,id,path,mime_type,content,checksum) VALUES(?,?,?,?,?,?)").bind(pid).bind(&asset.id).bind(&asset.path).bind(&asset.mime_type).bind(&asset.content).bind(&asset.checksum).execute(&mut *tx).await.map_err(err)?;
        let parents=entity_leaves(&mut tx,pid,&asset.id,"asset").await?;
        append(&mut tx,pid,&asset.id,"asset",parents,json!({
            "id":asset.id,"path":asset.path,"mimeType":asset.mime_type,"checksum":asset.checksum,"size":asset.content.len()
        }
        ),"add",&id()).await?;
        sqlx::query("INSERT INTO completed_asset_transfer(id,project_id,asset_id) VALUES(?,?,?)").bind(transfer_id).bind(pid).bind(&asset.id).execute(&mut *tx).await.map_err(err)?;
        sqlx::query("DELETE FROM asset_transfer WHERE id=?").bind(transfer_id).execute(&mut *tx).await.map_err(err)?;
        tx.commit().await.map_err(err)?;
        Ok(asset)
    }
    pub async fn asset_transfer_status(&self,pid:&str,transfer_id:&str)->Result<AssetTransferStatus,
    String>{
        let row=sqlx::query("SELECT total_size,checksum,length(content) received FROM asset_transfer WHERE id=? AND project_id=? UNION ALL SELECT length(a.content) total_size,a.checksum,length(a.content) received FROM completed_asset_transfer t JOIN project_asset a ON a.project_id=t.project_id AND a.id=t.asset_id WHERE t.id=? AND t.project_id=?").bind(transfer_id).bind(pid).bind(transfer_id).bind(pid).fetch_optional(&self.pool).await.map_err(err)?.ok_or("Transfer not found for this project.")?;
        Ok(AssetTransferStatus{
            transfer_id:transfer_id.into(),received:row.get::<i64,_>("received") as usize,total_size:row.get::<i64,_>("total_size") as usize,checksum:row.get("checksum")
        }
        )
    }
}
async fn entity_leaves(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,kind:&str)->Result<Vec<String>,
String>{
    sqlx::query_scalar("SELECT r.id FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.project_id=? AND r.entity_id=? AND r.entity_type=? AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.entity_id=r.entity_id AND c.project_id=r.project_id AND c.entity_type=r.entity_type AND p.value=r.id) ORDER BY m.sequence").bind(pid).bind(entity).bind(kind).fetch_all(&mut **tx).await.map_err(err)
}
async fn normalize_positions(tx:&mut Transaction<'_,Sqlite>,pid:&str)->Result<(),
String>{
    let chapters:Vec<String>=sqlx::query_scalar("SELECT id FROM chapter WHERE project_id=? ORDER BY position,id").bind(pid).fetch_all(&mut **tx).await.map_err(err)?;
    for (position,cid)in chapters.iter().enumerate(){
        sqlx::query("UPDATE chapter SET position=? WHERE id=?").bind(position as i64).bind(cid).execute(&mut **tx).await.map_err(err)?;
        let scenes:Vec<String>=sqlx::query_scalar("SELECT id FROM manuscript_scene WHERE chapter_id=? ORDER BY position,id").bind(cid).fetch_all(&mut **tx).await.map_err(err)?;
        for(p,sid)in scenes.iter().enumerate(){
            sqlx::query("UPDATE manuscript_scene SET position=? WHERE id=?").bind(p as i64).bind(sid).execute(&mut **tx).await.map_err(err)?;
        }
    }
    Ok(())
}
fn validate_asset_spec(spec:&AssetTransferSpec)->Result<(),
String>{
    if !valid_id(&spec.asset_id)||!safe_path(&spec.path)||!spec.path.starts_with("assets/")||spec.total_size>16*1024*1024||spec.checksum.len()!=64||!spec.checksum.chars().all(|c|c.is_ascii_hexdigit()&&!c.is_ascii_uppercase())||!["font/ttf","font/otf","font/woff","font/woff2","application/json","text/plain"].contains(&spec.mime_type.as_str()){
        return Err("Invalid asset manifest path, format, size or hash.".into())
    }
    Ok(())
}
fn spec_from_payload(payload:&Value)->Result<AssetTransferSpec,
String>{
    Ok(AssetTransferSpec{
        asset_id:payload.get("id").and_then(Value::as_str).ok_or("Invalid asset identity.")?.into(),path:payload.get("path").and_then(Value::as_str).ok_or("Invalid asset path.")?.into(),mime_type:payload.get("mimeType").and_then(Value::as_str).ok_or("Invalid asset type.")?.into(),total_size:payload.get("size").and_then(Value::as_u64).ok_or("Invalid asset size.")? as usize,checksum:payload.get("checksum").and_then(Value::as_str).ok_or("Invalid asset hash.")?.into()
    }
    )
}
async fn order_has_placement_conflicts(tx:&mut Transaction<'_,Sqlite>,pid:&str,rid:&str)->Result<bool,String>{
    let (entity,raw):(String,String)=sqlx::query_as("SELECT entity_id,payload FROM revision_envelope WHERE id=?").bind(rid).fetch_one(&mut **tx).await.map_err(err)?;
    let payload:Value=serde_json::from_str(&raw).map_err(|_|"Invalid metadata payload.")?;
    chapter_order_placement_conflicted(tx,pid,&entity,&payload).await
}
fn validate_metadata_resolution_payload(payload:&Value)->Result<(),String>{
    if !payload.is_object()||serde_json::to_vec(payload).map_err(|_|"Invalid metadata resolution.")?.len()>2_000_000{return Err("Invalid or oversized metadata resolution.".into())}
    if let Some(name)=payload.get("title"){title(name.as_str().ok_or("Invalid resolved title.")?)?;}Ok(())
}
async fn verified_metadata_payload(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,kind:&str,rid:&str)->Result<Value,String>{
    let (raw,checksum):(String,String)=sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=? AND project_id=? AND entity_id=? AND entity_type=?").bind(rid).bind(pid).bind(entity).bind(kind).fetch_optional(&mut **tx).await.map_err(err)?.ok_or("Metadata revision does not belong to this project and object.")?;
    if hash(raw.as_bytes())!=checksum{return Err("Metadata checksum failed. Stored data was preserved.".into())}serde_json::from_str(&raw).map_err(|_|"Invalid retained metadata payload.".into())
}
async fn metadata_resolution_capture(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,kind:&str)->Result<MetadataResolutionSnapshot,String>{
    validate_metadata_owner(tx,pid,entity,kind).await?;
    let heads=entity_leaves(tx,pid,entity,kind).await?;
    for head in &heads{verified_metadata_payload(tx,pid,entity,kind,head).await?;}
    let projection=if kind=="scenePlacement"{
        let (chapter,position):(String,i64)=sqlx::query_as("SELECT chapter_id,position FROM manuscript_scene WHERE id=?").bind(entity).fetch_one(&mut **tx).await.map_err(err)?;json!({"chapterId":chapter,"position":position})
    }else{crate::workspace::metadata_snapshot(tx,entity,kind).await?};
    let chapter_order=sqlx::query_scalar("SELECT id FROM chapter WHERE project_id=? ORDER BY position,id").bind(pid).fetch_all(&mut **tx).await.map_err(err)?;
    let rows=sqlx::query("SELECT s.id,s.chapter_id,s.position,s.deleted FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=? ORDER BY s.id").bind(pid).fetch_all(&mut **tx).await.map_err(err)?;
    if rows.len()>20_000{return Err("Metadata preview exceeds the 20,000-scene bound.".into())}
    let mut placement_heads=vec![];
    for row in rows{
        let scene_id:String=row.get("id");let heads=entity_leaves(tx,pid,&scene_id,"scenePlacement").await?;
        for head in &heads{verified_metadata_payload(tx,pid,&scene_id,"scenePlacement",head).await?;}
        placement_heads.push(MetadataPlacementState{scene_id,chapter_id:row.get("chapter_id"),position:row.get("position"),deleted:row.get::<i64,_>("deleted")!=0,heads});
    }
    Ok(MetadataResolutionSnapshot{project_id:pid.into(),entity_id:entity.into(),entity_type:kind.into(),heads,projection,chapter_order,placement_heads})
}
/// Both currently placed and explicitly listed scenes must have settled membership.
async fn chapter_order_placement_conflicted(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,payload:&Value)->Result<bool,String>{
    if payload.get("order").is_none(){return Ok(false)}
    let mut scenes:HashSet<String>=sqlx::query_scalar::<_,String>("SELECT id FROM manuscript_scene WHERE chapter_id=?").bind(entity).fetch_all(&mut **tx).await.map_err(err)?.into_iter().collect();
    for item in payload["order"].as_array().ok_or("Invalid metadata order.")?{scenes.insert(item.as_str().ok_or("Invalid metadata order ID.")?.into());}
    for scene in scenes{if entity_leaves(tx,pid,&scene,"scenePlacement").await?.len()>1{return Ok(true)}}
    Ok(false)
}
async fn guard_metadata_placements(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,kind:&str,payload:&Value)->Result<(),String>{
    if kind=="chapter"&&chapter_order_placement_conflicted(tx,pid,entity,payload).await?{return Err("Conflict: resolve competing scene placements before applying a chapter order.".into())}Ok(())
}
async fn apply_metadata(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,kind:&str,rid:&str)->Result<(),
String>{
    let raw:String=sqlx::query_scalar("SELECT payload FROM revision_envelope WHERE id=?").bind(rid).fetch_one(&mut **tx).await.map_err(err)?;
    let payload:Value=serde_json::from_str(&raw).map_err(|_|"Invalid metadata payload.")?;
    if kind=="scenePlacement"{
        let chapter=payload.get("chapterId").and_then(Value::as_str).ok_or("Invalid scene placement.")?;
        let owner:Option<String>=sqlx::query_scalar("SELECT project_id FROM chapter WHERE id=?").bind(chapter).fetch_optional(&mut **tx).await.map_err(err)?;if owner.as_deref()!=Some(pid){return Err("Scene placement target is not in this project.".into())}
        let position=payload.get("position").and_then(Value::as_u64).ok_or("Invalid scene placement position.")?;
        if position>20_000{
            return Err("Scene position exceeds the manifest limit.".into())
        }
        sqlx::query("UPDATE manuscript_scene SET chapter_id=?,position=? WHERE id=?").bind(chapter).bind(position as i64).bind(entity).execute(&mut **tx).await.map_err(err)?;
        return Ok(())
    }
    if let Some(name)=payload.get("title").and_then(Value::as_str){
        title(name)?;
        let q=if kind=="project"{
            "UPDATE project SET title=? WHERE id=?"
        }
        else{
            "UPDATE chapter SET title=? WHERE id=?"
        };
        sqlx::query(q).bind(name).bind(entity).execute(&mut **tx).await.map_err(err)?;
    }
    if let Some(order)=payload.get("order"){
        let order=order.as_array().ok_or("Invalid metadata order.")?;
        let mut seen=HashSet::new();
        let mut listed=vec![];
        let query=if kind=="project"{
            "SELECT id FROM chapter WHERE project_id=? ORDER BY position,id"
        }
        else{
            "SELECT id FROM manuscript_scene WHERE chapter_id=? ORDER BY position,id"
        };
        let current:Vec<String>=sqlx::query_scalar(query).bind(entity).fetch_all(&mut **tx).await.map_err(err)?;
        for v in order{
            let item=v.as_str().ok_or("Invalid metadata order ID.")?;
            if !current.iter().any(|id|id==item)||!seen.insert(item.to_string()){
                return Err("Metadata order contains duplicate or unauthorized objects.".into())
            }
            listed.push(item.to_string());
        }
        for item in current{
            if !seen.contains(&item){
                listed.push(item);
            }
        }
        let update=if kind=="project"{
            "UPDATE chapter SET position=? WHERE id=? AND project_id=?"
        }
        else{
            "UPDATE manuscript_scene SET position=? WHERE id=? AND chapter_id=?"
        };
        for(p,eid)in listed.iter().enumerate(){
            sqlx::query(update).bind(p as i64).bind(eid).bind(if kind=="project"{
                pid
            }
            else{
                entity
            }
            ).execute(&mut **tx).await.map_err(err)?;
        }
    }
    Ok(())
}

async fn validate_metadata_owner(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,kind:&str)->Result<(),String>{
    let owner:Option<String>=match kind{
        "project"=>sqlx::query_scalar("SELECT id FROM project WHERE id=?").bind(entity).fetch_optional(&mut **tx).await.map_err(err)?,
        "chapter"=>sqlx::query_scalar("SELECT project_id FROM chapter WHERE id=?").bind(entity).fetch_optional(&mut **tx).await.map_err(err)?,
        "scenePlacement"=>sqlx::query_scalar("SELECT c.project_id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(entity).fetch_optional(&mut **tx).await.map_err(err)?,
        _=>return Err("Unsupported metadata object type.".into())
    };if owner.as_deref()!=Some(pid){return Err("Metadata object is not in this project.".into())}Ok(())
}
