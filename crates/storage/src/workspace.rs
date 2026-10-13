use super::*;
use crate::sync::recompute_scene_heads;
use sqlx::{
    Row,
    Sqlite,
    Transaction
};
use sha2::{
    Digest,
    Sha256
};
use unicode_segmentation::UnicodeSegmentation;
use std::collections::{
    HashMap,
    HashSet
};
fn default_chapter_number_start()->i64{1}
const LEGACY_PROJECT: &str = "prototype-project";
const LEGACY_CHAPTER: &str = "prototype-chapter";
const LEGACY_SCENE: &str = "prototype-scene";
const ARCHIVE_LIMIT: usize = 64 * 1024 * 1024;
const RECORD_LIMIT: usize = 20_000;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct Project {
    pub id:String,
    pub title:String,
    pub created_at:String
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct Chapter {
    pub id:String,
    pub project_id:String,
    pub title:String,
    pub position:i64
}
/// One transaction captures the values and revision boundaries shown by the editor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ChapterEditSnapshot {
    pub chapter:Chapter,
    pub expected_chapter_head:Option<String>,
    pub expected_project_head:Option<String>,
    pub expected_order:Vec<String>,
    pub numbering_parent:Option<String>,
    pub chapter_number_start:i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct SceneRecord {
    pub id:String,
    pub chapter_id:String,
    pub position:i64,
    pub revision_id:String,
    pub scene:Scene
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ProjectBundle {
    #[serde(default="default_chapter_number_start")]
    pub chapter_number_start:i64,
    #[serde(default)]
    pub chapter_numbering_revision_id:Option<String>,
    #[serde(default)]
    pub chapter_numbering_conflicted:bool,
    pub project:Project,
    pub chapters:Vec<Chapter>,
    pub scenes:Vec<SceneRecord>
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct WordExportSnapshot {
    pub bundle: ProjectBundle,
    pub profile: ExportProfile,
    pub profile_revision_id: Option<String>,
}
/// Navigation metadata only. Documents/checksums are verified when a scene is loaded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct SceneSummary {
    pub id:String,
    pub chapter_id:String,
    pub position:i64,
    pub revision_id:String,
    pub title:String
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ProjectIndex {
    #[serde(default="default_chapter_number_start")]
    pub chapter_number_start:i64,
    #[serde(default)]
    pub chapter_numbering_revision_id:Option<String>,
    #[serde(default)]
    pub chapter_numbering_conflicted:bool,
    pub project:Project,
    pub chapters:Vec<Chapter>,
    pub scenes:Vec<SceneSummary>
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct Revision {
    pub id:String,
    pub project_id:String,
    pub entity_id:String,
    pub entity_type:String,
    pub parents:Vec<String>,
    pub payload:Value,
    pub operation:String,
    pub schema_version:u32,
    pub checksum:String,
    pub created_at:String,
    pub sequence:i64,
    pub origin_epoch:String,
    pub origin_device:String
}
/// Sequence is local receipt order. created_at is the author's saved timestamp,
/// which can precede receipt for imported/replicated history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ProjectTimePoint {
    pub sequence:i64,
    pub revision_id:String,
    pub created_at:String,
    pub entity_id:String,
    pub entity_type:String,
    pub operation:String
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ProjectTimePoints {
    pub epoch:String,
    pub points:Vec<ProjectTimePoint>,
    pub has_more:bool
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct TimeMachineHead {
    pub revision_id:String,
    pub sequence:i64,
    pub entity_type:String,
    pub title:Option<String>,
    pub created_at:String,
    pub order:Option<Vec<String>>,
    pub chapter_id:Option<String>,
    pub position:Option<i64>
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct TimeMachineObject {
    pub entity_id:String,
    pub entity_type:String,
    pub heads:Vec<TimeMachineHead>,
    pub current_head_ids:Vec<String>,
    pub chapter_id:Option<String>,
    pub position:Option<i64>
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ProjectTimePreview {
    pub epoch:String,
    pub point_revision_id:String,
    pub project_id:String,
    pub sequence:i64,
    pub objects:Vec<TimeMachineObject>,
    pub warnings:Vec<String>
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase")]
pub struct SearchHit {
    pub scene_id:String,
    pub title:String,
    pub revision_id:String,
    pub start_byte:usize,
    pub end_byte:usize,
    pub excerpt:String
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase")]
pub struct Statistics {
    pub scenes:usize,
    pub unicode_scalars:usize,
    pub graphemes:usize,
    pub whitespace_words:usize,
    pub cjk_characters:usize,
    pub method:String
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct SourceDependency {pub entity_id:String,pub entity_type:String,pub revision_id:String}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Archive {
    format:String,
    version:u32,
    entries:Vec<ArchiveEntry>
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct ArchiveEntry {
    path:String,
    size:usize,
    sha256:String,
    content:String
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct ArchiveData {
    bundle:ProjectBundle,
    revisions:Vec<Revision>,
    project_data:Vec<ProjectDataEntry>,
    assets:Vec<ProjectAsset>
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ProjectDataSnapshot {
    pub revision_id:String,
    pub payload:Value
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ProjectDataEntry {
    pub(crate) kind:String,
    pub(crate) revision_id:String
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ProjectAsset {
    pub id:String,
    pub path:String,
    pub mime_type:String,
    pub content:Vec<u8>,
    pub checksum:String
}
pub(crate) fn hash(bytes:&[u8])->String {
    format!("{:x}",Sha256::digest(bytes))
}
fn decode_scene(payload:&str,checksum:&str)->Result<Scene,
String>{
    if hash(payload.as_bytes())!=checksum{
        return Err("Scene checksum failed. Stored data was preserved.".into())
    }
    let scene:Scene=serde_json::from_str(payload).map_err(|_|"Unsupported saved scene.")?;
    validate(&scene)?;
    Ok(scene)
}
pub(crate) fn title(value:&str)->Result<(),
String>{
    if value.trim().is_empty() || value.chars().count()>500 {
        Err("A title must contain 1–500 characters.".into())
    }
    else{
        Ok(())
    }
}
pub(crate) fn id()->String{
    uuid::Uuid::new_v4().to_string()
}
pub(crate) fn err(e:sqlx::Error)->String{
    format!("Local storage operation failed: {e}")
}
pub(crate) fn revision(row:sqlx::sqlite::SqliteRow)->Result<Revision,
String>{
    Ok(Revision{
        id:row.get("id"),project_id:row.get("project_id"),entity_id:row.get("entity_id"),entity_type:row.get("entity_type"),parents:serde_json::from_str(row.get("parents")).map_err(|_|"Invalid revision parents.")?,payload:serde_json::from_str(row.get("payload")).map_err(|_|"Invalid revision payload.")?,operation:row.get("operation"),schema_version:row.get::<i64,_>("schema_version") as u32,checksum:row.get("checksum"),created_at:row.get("created_at"),sequence:row.get("sequence"),origin_epoch:row.get("origin_epoch"),origin_device:row.get("origin_device")
    }
    )
}
pub(crate) async fn append(tx:&mut Transaction<'_,Sqlite>, project:&str,entity:&str,kind:&str,parents:Vec<String>,payload:Value,operation:&str,revision_id:&str)->Result<(),
String>{
    let serialized=serde_json::to_string(&payload).map_err(|_|"Could not encode revision.")?;
    sqlx::query("INSERT INTO revision_envelope(id,project_id,entity_id,entity_type,parents,payload,operation,schema_version,checksum,origin_epoch,origin_device) VALUES(?,?,?,?,?,?,?,1,?,(SELECT epoch FROM storage_identity WHERE singleton=1),(SELECT device FROM storage_identity WHERE singleton=1))").bind(revision_id).bind(project).bind(entity).bind(kind).bind(serde_json::to_string(&parents).unwrap()).bind(&serialized).bind(operation).bind(hash(serialized.as_bytes())).execute(&mut **tx).await.map_err(err)?;
    sqlx::query("INSERT INTO change_marker(revision_id) VALUES(?)").bind(revision_id).execute(&mut **tx).await.map_err(err)?;
    Ok(())
}
pub fn plain_text(document:&Value)->String{
    fn walk(v:&Value,s:&mut String){
        match v.get("type").and_then(Value::as_str){
            Some("text")=>s.push_str(v.get("text").and_then(Value::as_str).unwrap_or("")),
            Some("hardBreak")=>s.push('\n'),
            _=>{
                if let Some(c)=v.get("content").and_then(Value::as_array){
                    for (i,n) in c.iter().enumerate(){
                        if v.get("type").and_then(Value::as_str)==Some("doc") && i>0{
                            s.push_str("\n\n");
                        }
                        walk(n,s)
                    }
                }
            }
        }
    }
    let mut text=String::new();
    walk(document,&mut text);
    text
}
impl Storage {
    pub async fn open(path:&Path)->Result<Self,
    sqlx::Error>{
        let options=SqliteConnectOptions::new().filename(path).create_if_missing(true).foreign_keys(true).journal_mode(SqliteJournalMode::Wal).synchronous(SqliteSynchronous::Full).busy_timeout(std::time::Duration::from_secs(5));
        let pool=SqlitePoolOptions::new().max_connections(1).connect_with(options).await?;
        let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&pool).await?;
        if version>3{
            return Err(sqlx::Error::Protocol("This database needs a newer application. Stored data was preserved.".into()))
        }
        if version<2 {
            if version==1 {
                let backup=path.with_file_name(format!("{}.pre-v2-{}.sqlite",path.file_name().unwrap_or_default().to_string_lossy(),id()));
                crate::recovery::consistent_snapshot(&pool,&backup).await.map_err(sqlx::Error::Protocol)?;
            }
            let mut tx=pool.begin().await?;
            if version==0 {
                sqlx::raw_sql(include_str!("schema.sql")).execute(&mut *tx).await?;
            }
            sqlx::raw_sql(include_str!("migration_v2.sql")).execute(&mut *tx).await?;
            #[cfg(test)] crate::recovery::checkpoint("migration-v2");
            sqlx::query("INSERT INTO storage_identity(singleton,epoch,device) VALUES(1,?,'desktop')").bind(id()).execute(&mut *tx).await?;
            let legacy:Vec<(String,Option<String>,String)>=sqlx::query_as("SELECT id,parent,payload FROM scene_revision ORDER BY rowid").fetch_all(&mut *tx).await?;
            if !legacy.is_empty(){
                sqlx::query("INSERT INTO project(id,title) VALUES(?,'My novel')").bind(LEGACY_PROJECT).execute(&mut *tx).await?;
                sqlx::query("INSERT INTO chapter(id,project_id,title,position) VALUES(?,?,'Chapter 1',0)").bind(LEGACY_CHAPTER).bind(LEGACY_PROJECT).execute(&mut *tx).await?;
                for (rid,parent,payload) in legacy {
                    let parsed:Value=serde_json::from_str(&payload).map_err(|e|sqlx::Error::Protocol(e.to_string()))?;
                    append(&mut tx,LEGACY_PROJECT,LEGACY_SCENE,"scene",parent.into_iter().collect(),parsed,"migrate",&rid).await.map_err(sqlx::Error::Protocol)?;
                }
                let head:Option<String>=sqlx::query_scalar("SELECT revision_id FROM scene_head WHERE singleton=1").fetch_optional(&mut *tx).await?;
                if let Some(head)=head {
                    sqlx::query("INSERT INTO manuscript_scene(id,chapter_id,position,head) VALUES(?,?,0,?)").bind(LEGACY_SCENE).bind(LEGACY_CHAPTER).bind(head).execute(&mut *tx).await?;
                }
            }
            tx.commit().await?;
        }
        if version<3 {
            if version==2 {
                let backup=path.with_file_name(format!("{}.pre-v3-{}.sqlite",path.file_name().unwrap_or_default().to_string_lossy(),id()));
                crate::recovery::consistent_snapshot(&pool,&backup).await.map_err(sqlx::Error::Protocol)?;
            }
            let mut tx=pool.begin().await?;
            sqlx::raw_sql(include_str!("migration_v3.sql")).execute(&mut *tx).await?;
            #[cfg(test)] crate::recovery::checkpoint("migration-v3");
            tx.commit().await?;
        }
        Ok(Self{
            pool
        }
        )
    }
    pub async fn close(&self){
        self.pool.close().await;
    }
    pub async fn load(&self)->Result<Option<Snapshot>,
    String>{
        let row:Option<(String,String,String)>=sqlx::query_as("SELECT r.id,r.payload,r.checksum FROM revision_envelope r JOIN scene_head h ON h.revision_id=r.id WHERE h.singleton=1").fetch_optional(&self.pool).await.map_err(err)?;
        row.map(|(revision_id,payload,checksum)|{
            let scene=decode_scene(&payload,&checksum)?;
            Ok(Snapshot{
                revision_id,scene
            }
            )
        }
        ).transpose()
    }
    pub async fn save(&self,scene:Scene,parent:Option<String>)->Result<Snapshot,
    String>{
        validate(&scene)?;
        if self.load().await?.is_none(){
            let mut tx=self.pool.begin().await.map_err(err)?;
            let current:Option<String>=sqlx::query_scalar("SELECT revision_id FROM scene_head WHERE singleton=1").fetch_optional(&mut *tx).await.map_err(err)?;
            if current!=parent{
                return Err("Conflict: another window saved this scene.".into())
            }
            if current.is_none(){
                sqlx::query("INSERT OR IGNORE INTO project(id,title) VALUES(?,'My novel')").bind(LEGACY_PROJECT).execute(&mut *tx).await.map_err(err)?;
                sqlx::query("INSERT OR IGNORE INTO chapter(id,project_id,title,position) VALUES(?,?,'Chapter 1',0)").bind(LEGACY_CHAPTER).bind(LEGACY_PROJECT).execute(&mut *tx).await.map_err(err)?;
                let rid=id();
                append(&mut tx,LEGACY_PROJECT,LEGACY_SCENE,"scene",vec![],serde_json::to_value(&scene).unwrap(),"create",&rid).await?;
                sqlx::query("INSERT INTO manuscript_scene(id,chapter_id,position,head) VALUES(?,?,0,?)").bind(LEGACY_SCENE).bind(LEGACY_CHAPTER).bind(&rid).execute(&mut *tx).await.map_err(err)?;
                mirror(&mut tx,&rid,None,&scene).await?;
                tx.commit().await.map_err(err)?;
                return Ok(Snapshot{
                    revision_id:rid,scene
                }
                )
            }
        }
        self.save_scene(LEGACY_SCENE,scene,parent).await
    }
    pub async fn backup(&self,path:&Path)->Result<(),
    String>{
        crate::recovery::consistent_snapshot(&self.pool,path).await
    }
    pub async fn changes(&self,after_sequence:i64,limit:u32)->Result<Vec<Revision>,
    String>{
        if after_sequence<0 || limit==0 || limit>1000{
            return Err("Change cursor must be nonnegative and page size 1–1000.".into())
        }
        sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE m.sequence>? ORDER BY m.sequence LIMIT ?").bind(after_sequence).bind(limit).fetch_all(&self.pool).await.map_err(err)?.into_iter().map(revision).collect()
    }
    pub async fn snapshot_scene(&self,sid:&str,expected_head:&str)->Result<Snapshot,
    String>{
        let scene=self.load_scene(sid).await?.scene;
        self.commit_scene(sid,scene,Some(expected_head.into()),"snapshot").await
    }
    pub async fn move_scene(&self,sid:&str,target_chapter_id:&str,position:usize)->Result<(),
    String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let (source,pid):(String,String)=sqlx::query_as("SELECT s.chapter_id,c.project_id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(sid).fetch_one(&mut *tx).await.map_err(err)?;
        let target_pid:String=sqlx::query_scalar("SELECT project_id FROM chapter WHERE id=?").bind(target_chapter_id).fetch_one(&mut *tx).await.map_err(err)?;
        if target_pid!=pid{
            return Err("Scenes can only move between chapters in the same project.".into())
        }
        let mut target:Vec<String>=sqlx::query_scalar("SELECT id FROM manuscript_scene WHERE chapter_id=? ORDER BY position,id").bind(target_chapter_id).fetch_all(&mut *tx).await.map_err(err)?;
        target.retain(|x|x!=sid);
        if position>target.len(){
            return Err("Scene position is outside the chapter.".into())
        }
        target.insert(position,sid.into());
        sqlx::query("UPDATE manuscript_scene SET chapter_id=? WHERE id=?").bind(target_chapter_id).bind(sid).execute(&mut *tx).await.map_err(err)?;
        for (p,id) in target.iter().enumerate(){
            sqlx::query("UPDATE manuscript_scene SET position=? WHERE id=?").bind(p as i64).bind(id).execute(&mut *tx).await.map_err(err)?;
        }
        if source!=target_chapter_id{
            let remaining:Vec<String>=sqlx::query_scalar("SELECT id FROM manuscript_scene WHERE chapter_id=? ORDER BY position,id").bind(&source).fetch_all(&mut *tx).await.map_err(err)?;
            for(p,id)in remaining.iter().enumerate(){
                sqlx::query("UPDATE manuscript_scene SET position=? WHERE id=?").bind(p as i64).bind(id).execute(&mut *tx).await.map_err(err)?;
            }
        }
        let parents=metadata_parent(&mut tx,sid,"scenePlacement").await?;
        append(&mut tx,&pid,sid,"scenePlacement",parents,serde_json::json!({
            "chapterId":target_chapter_id,"position":position,"fromChapterId":source
        }
        ),"move",&id()).await?;
        tx.commit().await.map_err(err)?;
        Ok(())
    }
    pub async fn list_projects(&self)->Result<Vec<Project>,
    String>{
        let rows=sqlx::query("SELECT id,title,created_at FROM project ORDER BY created_at,id").fetch_all(&self.pool).await.map_err(err)?;
        Ok(rows.into_iter().map(|r|Project{
            id:r.get("id"),title:r.get("title"),created_at:r.get("created_at")
        }
        ).collect())
    }
    pub async fn sync_manifest(&self,pid:&str)->Result<ProjectBundle,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;read_bundle(&mut tx,pid).await
    }
    pub async fn deleted_scenes(&self,pid:&str)->Result<Vec<SceneRecord>,
    String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let bundle=read_bundle(&mut tx,pid).await?;
        let ids:Vec<String>=sqlx::query_scalar("SELECT s.id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=? AND s.deleted=1").bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
        let ids=ids.into_iter().collect::<HashSet<_>>();
        Ok(bundle.scenes.into_iter().filter(|s|ids.contains(&s.id)).collect())
    }
    pub async fn create_project(&self,name:&str)->Result<Project,
    String>{
        title(name)?;
        let pid=id();
        let mut tx=self.pool.begin().await.map_err(err)?;
        sqlx::query("INSERT INTO project(id,title) VALUES(?,?)").bind(&pid).bind(name).execute(&mut *tx).await.map_err(err)?;
        append(&mut tx,&pid,&pid,"project",vec![],serde_json::json!({
            "title":name
        }
        ),"create",&id()).await?;
        tx.commit().await.map_err(err)?;
        Ok(self.open_project(&pid).await?.project)
    }
    pub async fn open_project(&self,pid:&str)->Result<ProjectBundle,
    String>{
        self.project_snapshot(pid,false,None).await.map(|snapshot|snapshot.bundle)
    }
    /// A read-only manuscript snapshot cannot silently select one conflicted
    /// scene or deletion branch. Check leaves inside the snapshot transaction.
    pub async fn manuscript_export_snapshot(&self,pid:&str)->Result<ProjectBundle,String>{
        self.project_snapshot(pid,true,None).await.map(|snapshot|snapshot.bundle)
    }
    /// Bind the selected export styles and the manuscript to one transaction.
    /// None means that the caller saw no saved style profile, not an unchecked head.
    pub async fn word_export_snapshot(&self,pid:&str,expected_profile:Option<String>)->Result<WordExportSnapshot,String>{
        self.project_snapshot(pid,true,Some(expected_profile)).await
    }
    async fn project_snapshot(&self,pid:&str,require_resolved:bool,expected_profile:Option<Option<String>>)->Result<WordExportSnapshot,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let r=sqlx::query("SELECT id,title,created_at FROM project WHERE id=?").bind(pid).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Project not found.")?;
        if require_resolved {
            // Deleted projections may retain the last text head. Count DAG
            // leaves directly so a single resolved tombstone is unambiguous.
            let conflicted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revision_envelope r JOIN manuscript_scene s ON s.id=r.entity_id JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=? AND r.entity_type IN('scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.entity_id=r.entity_id AND child.entity_type IN('scene','tombstone') AND p.value=r.id) GROUP BY r.entity_id HAVING count(*)>1)").bind(pid).fetch_one(&mut *tx).await.map_err(err)?;
            if conflicted { return Err("Resolve conflicting scene versions or deletions in Recovery before exporting a Word manuscript.".into()); }
        }
        let project=Project{
            id:r.get("id"),
            title:r.get("title"),
            created_at:r.get("created_at")
        };
        let chapters=sqlx::query("SELECT id,project_id,title,position FROM chapter WHERE project_id=? ORDER BY position,id").bind(pid).fetch_all(&mut *tx).await.map_err(err)?.into_iter().map(|r|Chapter{
            id:r.get("id"),project_id:r.get("project_id"),title:r.get("title"),position:r.get("position")
        }
        ).collect();
        let rows=sqlx::query("SELECT s.id,s.chapter_id,s.position,s.head,r.payload,r.checksum FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id JOIN revision_envelope r ON r.id=s.head WHERE c.project_id=? AND s.deleted=0 ORDER BY c.position,c.id,s.position,s.id").bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
        let scenes=rows.into_iter().map(|r|{
            let scene=decode_scene(r.get("payload"),r.get("checksum"))?;
            Ok(SceneRecord{
                id:r.get("id"),chapter_id:r.get("chapter_id"),position:r.get("position"),revision_id:r.get("head"),scene
            }
            )
        }
        ).collect::<Result<Vec<_>,
        String>>()?;
        let (chapter_number_start,chapter_numbering_revision_id,chapter_numbering_conflicted)=chapter_numbering_state(&mut tx,pid).await?;
        let (profile,profile_revision_id)=if let Some(expected)=expected_profile {
            let (profile,head)=export_profile_state(&mut tx,pid).await?;
            if head!=expected{return Err("Word export styles changed. Reload the saved styles before exporting.".into())}
            (profile,head)
        }else{(ExportProfile::default(),None)};
        tx.commit().await.map_err(err)?;
        Ok(WordExportSnapshot{bundle:ProjectBundle{
            project,chapters,scenes,chapter_number_start,chapter_numbering_revision_id,chapter_numbering_conflicted
        },profile,profile_revision_id})
    }
    /// SQLite extracts titles from JSON, but no full documents cross the wire or
    /// enter Rust scene decoding. This does not validate unrelated checksums.
    pub async fn open_project_index(&self,pid:&str)->Result<ProjectIndex,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let r=sqlx::query("SELECT id,title,created_at FROM project WHERE id=?").bind(pid).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Project not found.")?;
        let project=Project{id:r.get("id"),title:r.get("title"),created_at:r.get("created_at")};
        let chapters=sqlx::query("SELECT id,project_id,title,position FROM chapter WHERE project_id=? ORDER BY position,id").bind(pid).fetch_all(&mut *tx).await.map_err(err)?.into_iter().map(|r|Chapter{id:r.get("id"),project_id:r.get("project_id"),title:r.get("title"),position:r.get("position")}).collect();
        // CASE protects JSON extraction from malformed stored JSON. Never coerce
        // null/numeric/object titles into apparently valid navigation metadata.
        let rows=sqlx::query("SELECT s.id,s.chapter_id,s.position,s.head,CASE WHEN json_valid(r.payload) THEN json_type(r.payload,'$.title') END AS title_type,CASE WHEN json_valid(r.payload) THEN json_extract(r.payload,'$.title') END AS title FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id JOIN revision_envelope r ON r.id=s.head WHERE c.project_id=? AND s.deleted=0 ORDER BY c.position,c.id,s.position,s.id").bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
        let scenes=rows.into_iter().map(|r|{
            if r.get::<Option<String>,_>("title_type").as_deref()!=Some("text"){return Err("Unsupported saved scene title. Stored data was preserved.".into())}
            Ok(SceneSummary{id:r.get("id"),chapter_id:r.get("chapter_id"),position:r.get("position"),revision_id:r.get("head"),title:r.get("title")})
        }).collect::<Result<Vec<_>,String>>()?;
        let (chapter_number_start,chapter_numbering_revision_id,chapter_numbering_conflicted)=chapter_numbering_state(&mut tx,pid).await?;
        tx.commit().await.map_err(err)?;
        Ok(ProjectIndex{project,chapters,scenes,chapter_number_start,chapter_numbering_revision_id,chapter_numbering_conflicted})
    }
    /// Resolve membership, placement and current verified text in one read transaction.
    pub async fn load_project_scene(&self,pid:&str,sid:&str)->Result<SceneRecord,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let r=sqlx::query("SELECT s.id,s.chapter_id,s.position,s.head,r.payload,r.checksum FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id JOIN revision_envelope r ON r.id=s.head WHERE c.project_id=? AND s.id=? AND s.deleted=0").bind(pid).bind(sid).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Scene is no longer in this manuscript.")?;
        let scene=decode_scene(r.get("payload"),r.get("checksum"))?;
        let result=SceneRecord{id:r.get("id"),chapter_id:r.get("chapter_id"),position:r.get("position"),revision_id:r.get("head"),scene};
        tx.commit().await.map_err(err)?;
        Ok(result)
    }
    pub async fn rename_project(&self,pid:&str,name:&str)->Result<(),
    String>{
        self.rename_metadata("project",pid,name).await
    }
    pub async fn rename_chapter(&self,cid:&str,name:&str)->Result<(),
    String>{
        self.rename_metadata("chapter",cid,name).await
    }
    async fn rename_metadata(&self,kind:&str,eid:&str,name:&str)->Result<(),
    String>{
        title(name)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        let pid:String=if kind=="project"{
            sqlx::query_scalar("SELECT id FROM project WHERE id=?").bind(eid).fetch_one(&mut *tx).await.map_err(err)?
        }
        else{
            sqlx::query_scalar("SELECT project_id FROM chapter WHERE id=?").bind(eid).fetch_one(&mut *tx).await.map_err(err)?
        };
        let query=if kind=="project"{
            "UPDATE project SET title=? WHERE id=?"
        }
        else{
            "UPDATE chapter SET title=? WHERE id=?"
        };
        sqlx::query(query).bind(name).bind(eid).execute(&mut *tx).await.map_err(err)?;
        let parents=metadata_parent(&mut tx,eid,kind).await?;
        let payload=metadata_snapshot(&mut tx,eid,kind).await?;
        append(&mut tx,&pid,eid,kind,parents,payload,"rename",&id()).await?;
        tx.commit().await.map_err(err)?;
        Ok(())
    }
    pub async fn chapter_edit_snapshot(&self,pid:&str,cid:&str)->Result<ChapterEditSnapshot,String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let row=sqlx::query("SELECT id,project_id,title,position FROM chapter WHERE project_id=? AND id=?").bind(pid).bind(cid).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Chapter does not belong to this novel.")?;
        let chapter=Chapter{id:row.get("id"),project_id:row.get("project_id"),title:row.get("title"),position:row.get("position")};
        let chapter_heads=metadata_parent(&mut tx,cid,"chapter").await?;
        let project_heads=metadata_parent(&mut tx,pid,"project").await?;
        for head in chapter_heads.iter().chain(project_heads.iter()){
            let (payload,checksum):(String,String)=sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=?").bind(head).fetch_one(&mut *tx).await.map_err(err)?;
            if hash(payload.as_bytes())!=checksum{return Err("Metadata checksum failed. Stored data was preserved.".into())}
        }
        let expected_order=sqlx::query_scalar("SELECT id FROM chapter WHERE project_id=? ORDER BY position,id").bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
        let (chapter_number_start,numbering_parent,conflicted)=chapter_numbering_state(&mut tx,pid).await?;
        if conflicted{return Err("Conflict: resolve competing chapter numbering before editing.".into())}
        tx.commit().await.map_err(err)?;
        Ok(ChapterEditSnapshot{chapter,expected_chapter_head:chapter_heads.first().cloned(),expected_project_head:project_heads.first().cloned(),expected_order,numbering_parent,chapter_number_start})
    }
    /// Rename and renumber with every captured metadata boundary checked together.
    pub async fn edit_chapter(&self,cid:&str,expected_chapter_head:Option<String>,expected_project_head:Option<String>,expected_order:Vec<String>,numbering_parent:Option<String>,name:&str,number:i64)->Result<ProjectIndex,String>{
        title(name)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        let pid:String=sqlx::query_scalar("SELECT project_id FROM chapter WHERE id=?").bind(cid).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Chapter not found.")?;
        let chapter_parents=metadata_parent(&mut tx,cid,"chapter").await?;
        let project_parents=metadata_parent(&mut tx,&pid,"project").await?;
        if chapter_parents.first()!=expected_chapter_head.as_ref()||project_parents.first()!=expected_project_head.as_ref(){return Err("Conflict: chapter or manuscript metadata changed. Reopen the chapter editor.".into())}
        for rid in chapter_parents.iter().chain(project_parents.iter()){
            let (payload,checksum):(String,String)=sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=?").bind(rid).fetch_one(&mut *tx).await.map_err(err)?;
            if hash(payload.as_bytes())!=checksum{return Err("Metadata checksum failed. Stored data was preserved.".into())}
        }
        let mut order:Vec<String>=sqlx::query_scalar("SELECT id FROM chapter WHERE project_id=? ORDER BY position,id").bind(&pid).fetch_all(&mut *tx).await.map_err(err)?;
        if order!=expected_order{return Err("Conflict: manuscript chapter order changed. Reopen the chapter editor.".into())}
        let (start,numbering_head,conflicted)=chapter_numbering_state(&mut tx,&pid).await?;
        if conflicted||numbering_head!=numbering_parent{return Err("Conflict: chapter numbering changed or has competing revisions.".into())}
        let old_index=order.iter().position(|id|id==cid).ok_or("Chapter no longer belongs to this manuscript.")?;
        let new_start=if number==0{0}else if start==0&&old_index==0&&number==1{1}else{start};
        let position=number.checked_sub(new_start).ok_or("Invalid chapter number.")?;
        if position<0||position>=order.len() as i64{return Err("Chapter number is outside this manuscript's range.".into())}
        order.remove(old_index);order.insert(position as usize,cid.into());
        sqlx::query("UPDATE chapter SET title=? WHERE id=?").bind(name).bind(cid).execute(&mut *tx).await.map_err(err)?;
        for (position,id) in order.iter().enumerate(){sqlx::query("UPDATE chapter SET position=? WHERE id=?").bind(position as i64).bind(id).execute(&mut *tx).await.map_err(err)?;}
        let payload=metadata_snapshot(&mut tx,cid,"chapter").await?;
        append(&mut tx,&pid,cid,"chapter",chapter_parents,payload,"editChapter",&id()).await?;
        let payload=metadata_snapshot(&mut tx,&pid,"project").await?;
        append(&mut tx,&pid,&pid,"project",project_parents,payload,"editChapter",&id()).await?;
        if new_start!=start{
            let rid=id();append(&mut tx,&pid,&format!("{pid}:chapterNumbering"),"projectData",numbering_head.into_iter().collect(),serde_json::json!({"schemaVersion":1,"start":new_start}),"chapterNumbering",&rid).await?;
            sqlx::query("INSERT INTO project_data(project_id,kind,head) VALUES(?,'chapterNumbering',?) ON CONFLICT(project_id,kind) DO UPDATE SET head=excluded.head").bind(&pid).bind(rid).execute(&mut *tx).await.map_err(err)?;
        }
        tx.commit().await.map_err(err)?;
        self.open_project_index(&pid).await
    }
    pub async fn create_chapter(&self,pid:&str,name:&str)->Result<Chapter,
    String>{
        title(name)?;
        let cid=id();
        let mut tx=self.pool.begin().await.map_err(err)?;
        let project_parents=metadata_parent(&mut tx,pid,"project").await?;
        let position:i64=sqlx::query_scalar("SELECT coalesce(max(position)+1,0) FROM chapter WHERE project_id=?").bind(pid).fetch_one(&mut *tx).await.map_err(err)?;
        sqlx::query("INSERT INTO chapter(id,project_id,title,position) VALUES(?,?,?,?)").bind(&cid).bind(pid).bind(name).bind(position).execute(&mut *tx).await.map_err(err)?;
        append(&mut tx,pid,&cid,"chapter",vec![],serde_json::json!({
            "title":name,"position":position
        }
        ),"create",&id()).await?;
        let project_payload=metadata_snapshot(&mut tx,pid,"project").await?;
        append(&mut tx,pid,pid,"project",project_parents,project_payload,"createChapter",&id()).await?;
        tx.commit().await.map_err(err)?;
        Ok(Chapter{
            id:cid,project_id:pid.into(),title:name.into(),position
        }
        )
    }
    pub async fn create_scene(&self,cid:&str,scene:Scene)->Result<SceneRecord,
    String>{
        validate(&scene)?;
        let sid=id();
        let rid=id();
        let mut tx=self.pool.begin().await.map_err(err)?;
        let pid:String=sqlx::query_scalar("SELECT project_id FROM chapter WHERE id=?").bind(cid).fetch_one(&mut *tx).await.map_err(err)?;
        let position:i64=sqlx::query_scalar("SELECT coalesce(max(position)+1,0) FROM manuscript_scene WHERE chapter_id=?").bind(cid).fetch_one(&mut *tx).await.map_err(err)?;
        append(&mut tx,&pid,&sid,"scene",vec![],serde_json::to_value(&scene).unwrap(),"create",&rid).await?;
        sqlx::query("INSERT INTO manuscript_scene(id,chapter_id,position,head) VALUES(?,?,?,?)").bind(&sid).bind(cid).bind(position).bind(&rid).execute(&mut *tx).await.map_err(err)?;
        append(&mut tx,&pid,&sid,"scenePlacement",vec![],serde_json::json!({"chapterId":cid,"position":position}),"create",&id()).await?;
        tx.commit().await.map_err(err)?;
        Ok(SceneRecord{
            id:sid,chapter_id:cid.into(),position,revision_id:rid,scene
        }
        )
    }
    pub async fn load_scene(&self,sid:&str)->Result<Snapshot,
    String>{
        let row:(String,String,String)=sqlx::query_as("SELECT r.id,r.payload,r.checksum FROM manuscript_scene s JOIN revision_envelope r ON r.id=s.head WHERE s.id=?").bind(sid).fetch_optional(&self.pool).await.map_err(err)?.ok_or("Scene not found.")?;
        let scene=decode_scene(&row.1,&row.2)?;
        Ok(Snapshot{
            revision_id:row.0,scene
        }
        )
    }
    pub async fn save_scene(&self,sid:&str,scene:Scene,parent:Option<String>)->Result<Snapshot,
    String>{
        self.commit_scene(sid,scene,parent,"edit").await
    }
    async fn commit_scene(&self,sid:&str,scene:Scene,parent:Option<String>,operation:&str)->Result<Snapshot,
    String>{
        self.commit_scene_request(sid,scene,parent,operation,None).await
    }
    pub async fn save_scene_with_id(&self,sid:&str,scene:Scene,parent:Option<String>,request_id:&str)->Result<Snapshot,
    String>{
        if !valid_id(request_id){
            return Err("Invalid save request identity.".into())
        }
        self.commit_scene_request(sid,scene,parent,"edit",Some(request_id)).await
    }
    pub async fn save_scene_checked(&self,sid:&str,scene:Scene,expected_head:&str,dependencies:&[SourceDependency])->Result<Snapshot,String>{
        if dependencies.len()>50{return Err("Review supporting scope exceeds 50 saved sources.".into())}
        self.commit_scene_checked_request(sid,scene,Some(expected_head.into()),"reviewApply",None,Some(dependencies)).await
    }
    async fn commit_scene_request(&self,sid:&str,scene:Scene,parent:Option<String>,operation:&str,request_id:Option<&str>)->Result<Snapshot,String>{
        self.commit_scene_checked_request(sid,scene,parent,operation,request_id,None).await
    }
    async fn commit_scene_checked_request(&self,sid:&str,scene:Scene,parent:Option<String>,operation:&str,request_id:Option<&str>,dependencies:Option<&[SourceDependency]>)->Result<Snapshot,String>{
        validate(&scene)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        if let Some(request)=request_id {
            let existing:Option<(String,String,String,String)>=sqlx::query_as("SELECT entity_id,entity_type,payload,parents FROM revision_envelope WHERE id=?").bind(request).fetch_optional(&mut *tx).await.map_err(err)?;
            if let Some((entity,kind,payload,parents))=existing {
                let wanted=serde_json::to_value(&scene).unwrap();
                let existing_payload:Value=serde_json::from_str(&payload).map_err(|_|"Invalid existing save request.")?;
                let existing_parents:Vec<String>=serde_json::from_str(&parents).map_err(|_|"Invalid existing save request.")?;
                if entity!=sid||kind!="scene"||existing_payload!=wanted||existing_parents!=parent.clone().into_iter().collect::<Vec<_>>(){
                    return Err("Save request identity was already used for different content.".into())
                }
                return Ok(Snapshot{
                    revision_id:request.into(),scene
                }
                )
            }
        }
        let (current,pid,deleted):(String,String,i64)=sqlx::query_as("SELECT s.head,c.project_id,s.deleted FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(sid).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Scene not found.")?;
        if deleted!=0{
            return Err("This scene is deleted. Restore it explicitly before editing.".into())
        }
        if Some(&current)!=parent.as_ref(){
            return Err("Conflict: another window saved this scene. Your edits were preserved in this window.".into())
        }
        if let Some(dependencies)=dependencies{
            validate_current_source(&mut tx,&pid,sid,"scene",&current).await?;
            validate_dependencies(&mut tx,&pid,dependencies).await?;
        }
        let rid=request_id.map(str::to_owned).unwrap_or_else(id);
        append(&mut tx,&pid,sid,"scene",vec![current.clone()],serde_json::to_value(&scene).unwrap(),operation,&rid).await?;
        sqlx::query("UPDATE manuscript_scene SET head=? WHERE id=?").bind(&rid).bind(sid).execute(&mut *tx).await.map_err(err)?;
        if sid==LEGACY_SCENE{
            mirror(&mut tx,&rid,Some(&current),&scene).await?;
        }
        tx.commit().await.map_err(err)?;
        Ok(Snapshot{
            revision_id:rid,scene
        }
        )
    }
    pub async fn rename_scene(&self,sid:&str,name:&str,parent:Option<String>)->Result<Snapshot,
    String>{
        title(name)?;
        let mut scene=self.load_scene(sid).await?.scene;
        scene.title=name.into();
        self.commit_scene(sid,scene,parent,"rename").await
    }
    pub async fn history(&self,sid:&str)->Result<Vec<Revision>,
    String>{
        sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.entity_id=? AND r.entity_type='scene' ORDER BY m.sequence DESC").bind(sid).fetch_all(&self.pool).await.map_err(err)?.into_iter().map(revision).collect()
    }
    pub async fn load_revision(&self,sid:&str,rid:&str)->Result<Snapshot,
    String>{
        let row:(String,String,String)=sqlx::query_as("SELECT r.id,r.payload,r.checksum FROM revision_envelope r JOIN manuscript_scene s ON s.id=r.entity_id JOIN chapter c ON c.id=s.chapter_id WHERE r.id=? AND r.entity_id=? AND r.entity_type='scene' AND r.project_id=c.project_id").bind(rid).bind(sid).fetch_optional(&self.pool).await.map_err(err)?.ok_or("Revision does not belong to this scene.")?;
        if hash(row.1.as_bytes())!=row.2{
            return Err("Revision checksum failed. Stored data was preserved.".into())
        }
        let scene:Scene=serde_json::from_str(&row.1).map_err(|_|"Unsupported saved revision.")?;
        validate(&scene)?;
        Ok(Snapshot{
            revision_id:row.0,scene
        }
        )
    }
    pub async fn restore_scene(&self,sid:&str,rid:&str,expected_head:&str)->Result<Snapshot,
    String>{
        let row=sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.id=? AND r.entity_id=? AND r.entity_type='scene'").bind(rid).bind(sid).fetch_optional(&self.pool).await.map_err(err)?.ok_or("Revision does not belong to this scene.")?;
        let rev=revision(row)?;
        let payload=serde_json::to_string(&rev.payload).unwrap();
        if hash(payload.as_bytes())!=rev.checksum{
            return Err("Revision checksum failed. Stored data was preserved.".into())
        }
        let scene=serde_json::from_value(rev.payload).map_err(|_|"Unsupported saved revision.")?;
        self.commit_scene(sid,scene,Some(expected_head.into()),"restore").await
    }
    pub async fn preserve_conflict(&self,sid:&str,scene:Scene,parent:&str)->Result<Snapshot,
    String>{
        validate(&scene)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        let (pid,current):(String,String)=sqlx::query_as("SELECT c.project_id,s.head FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(sid).fetch_one(&mut *tx).await.map_err(err)?;
        if current==parent{return Err("The supplied parent is current. Save normally; a conflict branch requires a stale ancestor.".into())}
        let valid:i64=sqlx::query_scalar("SELECT count(*) FROM revision_envelope WHERE id=? AND entity_id=? AND entity_type='scene' AND project_id=?").bind(parent).bind(sid).bind(&pid).fetch_one(&mut *tx).await.map_err(err)?;
        if valid!=1{
            return Err("Conflict parent does not belong to this scene.".into())
        }
        let rid=id();
        append(&mut tx,&pid,sid,"scene",vec![parent.into()],serde_json::to_value(&scene).unwrap(),"conflict",&rid).await?;
        tx.commit().await.map_err(err)?;
        Ok(Snapshot{
            revision_id:rid,scene
        }
        )
    }
    pub async fn conflicts(&self,sid:&str)->Result<Vec<Revision>,
    String>{
        sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id JOIN manuscript_scene s ON s.id=r.entity_id WHERE r.entity_id=? AND r.entity_type IN('scene','tombstone') AND r.id!=s.head AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.entity_id=r.entity_id AND child.entity_type IN('scene','tombstone') AND p.value=r.id) ORDER BY m.sequence").bind(sid).fetch_all(&self.pool).await.map_err(err)?.into_iter().map(revision).collect()
    }
    pub async fn resolve_scene(&self,sid:&str,scene:Scene,expected_head:&str,conflict_ids:Vec<String>)->Result<Snapshot,
    String>{
        validate(&scene)?;
        if conflict_ids.is_empty(){
            return Err("Select at least one conflicting revision to acknowledge.".into())
        }
        let mut tx=self.pool.begin().await.map_err(err)?;
        let (head,pid):(String,String)=sqlx::query_as("SELECT s.head,c.project_id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(sid).fetch_one(&mut *tx).await.map_err(err)?;
        if head!=expected_head{
            return Err("Conflict: scene changed before resolution.".into())
        }
        let mut parents=vec![head.clone()];
        for conflict in conflict_ids{
            if parents.contains(&conflict){
                return Err("Duplicate resolution parent.".into())
            }
            let valid:i64=sqlx::query_scalar("SELECT count(*) FROM revision_envelope WHERE id=? AND entity_id=? AND entity_type IN('scene','tombstone')").bind(&conflict).bind(sid).fetch_one(&mut *tx).await.map_err(err)?;
            if valid!=1{
                return Err("Resolution parent is not a conflicting scene revision.".into())
            }
            parents.push(conflict);
        }
        validate_resolution_heads(&mut tx,&pid,sid,&parents).await?;
        let rid=id();
        append(&mut tx,&pid,sid,"scene",parents,serde_json::to_value(&scene).unwrap(),"resolve",&rid).await?;
        sqlx::query("UPDATE manuscript_scene SET head=? WHERE id=?").bind(&rid).bind(sid).execute(&mut *tx).await.map_err(err)?;
        if sid==LEGACY_SCENE{
            mirror(&mut tx,&rid,Some(&head),&scene).await?
        }
        recompute_scene_heads(&mut tx,sid).await?;
        tx.commit().await.map_err(err)?;
        Ok(Snapshot{
            revision_id:rid,scene
        }
        )
    }
    pub async fn resolve_scene_deletion(&self,sid:&str,expected_head:&str,conflict_ids:Vec<String>)->Result<String,String>{
        if conflict_ids.is_empty(){return Err("Select at least one conflicting revision to acknowledge.".into())}
        let mut tx=self.pool.begin().await.map_err(err)?;
        let (head,pid):(String,String)=sqlx::query_as("SELECT s.head,c.project_id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=?").bind(sid).fetch_one(&mut *tx).await.map_err(err)?;
        if head!=expected_head{return Err("Conflict: scene changed before deletion resolution.".into())}
        let mut parents=vec![head];parents.extend(conflict_ids);
        validate_resolution_heads(&mut tx,&pid,sid,&parents).await?;
        let rid=id();append(&mut tx,&pid,sid,"tombstone",parents,serde_json::json!({"deleted":true}),"resolveDelete",&rid).await?;
        recompute_scene_heads(&mut tx,sid).await?;tx.commit().await.map_err(err)?;Ok(rid)
    }
    pub async fn reorder_chapters(&self,pid:&str,ids:Vec<String>)->Result<(),
    String>{
        self.reorder("chapter",pid,ids).await
    }
    pub async fn reorder_scenes(&self,cid:&str,ids:Vec<String>)->Result<(),
    String>{
        self.reorder("scene",cid,ids).await
    }
    async fn reorder(&self,kind:&str,owner:&str,mut ids:Vec<String>)->Result<(),
    String>{
        let mut tx=self.pool.begin().await.map_err(err)?;
        let select=if kind=="chapter"{
            "SELECT id FROM chapter WHERE project_id=?"
        }
        else{
            "SELECT id FROM manuscript_scene WHERE chapter_id=?"
        };
        let existing:Vec<String>=sqlx::query_scalar(select).bind(owner).fetch_all(&mut *tx).await.map_err(err)?;
        if kind=="scene"&&ids.len()!=existing.len(){
            let hidden:Vec<String>=sqlx::query_scalar("SELECT id FROM manuscript_scene WHERE chapter_id=? AND deleted=1 ORDER BY position,id").bind(owner).fetch_all(&mut *tx).await.map_err(err)?;
            let visible=existing.iter().filter(|id|!hidden.contains(id)).collect::<Vec<_>>();
            if ids.len()==visible.len()&&visible.iter().all(|id|ids.contains(id)){ids.extend(hidden);}
        }
        if ids.len()!=existing.len() || ids.iter().collect::<HashSet<_>>().len()!=ids.len() || existing.iter().any(|v|!ids.contains(v)){
            return Err("Reorder must include every existing object exactly once.".into())
        }
        let pid:String=if kind=="chapter"{
            sqlx::query_scalar("SELECT id FROM project WHERE id=?").bind(owner).fetch_one(&mut *tx).await.map_err(err)?
        }
        else{
            sqlx::query_scalar("SELECT project_id FROM chapter WHERE id=?").bind(owner).fetch_one(&mut *tx).await.map_err(err)?
        };
        let update=if kind=="chapter"{
            "UPDATE chapter SET position=? WHERE id=?"
        }
        else{
            "UPDATE manuscript_scene SET position=? WHERE id=?"
        };
        for (position,eid) in ids.iter().enumerate(){
            sqlx::query(update).bind(position as i64).bind(eid).execute(&mut *tx).await.map_err(err)?;
        }
        let entity_type=if kind=="chapter"{"project"}else{"chapter"};
        let parents=metadata_parent(&mut tx,owner,entity_type).await?;
        let payload=metadata_snapshot(&mut tx,owner,entity_type).await?;
        append(&mut tx,&pid,owner,entity_type,parents,payload,"reorder",&id()).await?;
        tx.commit().await.map_err(err)?;
        Ok(())
    }
    pub async fn search(&self,pid:&str,query:&str)->Result<Vec<SearchHit>,
    String>{
        if query.is_empty(){
            return Ok(vec![])
        }
        if query.len()>2000{
            return Err("Search text exceeds 2,000 bytes.".into())
        }
        let bundle=self.open_project(pid).await?;
        let mut hits=vec![];
        for s in bundle.scenes{
            let text=plain_text(&s.scene.document);
            for (start,_) in text.match_indices(query){
                let end=start+query.len();
                let before=text[..start].chars().rev().take(40).collect::<String>().chars().rev().collect::<String>();
                let after=text[end..].chars().take(40).collect::<String>();
                hits.push(SearchHit{
                    scene_id:s.id.clone(),title:s.scene.title.clone(),revision_id:s.revision_id.clone(),start_byte:start,end_byte:end,excerpt:format!("{before}{query}{after}")
                }
                );
                if hits.len()>=10_000{
                    return Ok(hits)
                }
            }
        }
        Ok(hits)
    }
    pub async fn statistics(&self,pid:&str)->Result<Statistics,
    String>{
        let bundle=self.open_project(pid).await?;
        let mut stats=Statistics{
            scenes:bundle.scenes.len(),
            method:"Exact Unicode scalar/grapheme counts; whitespace-delimited tokens and CJK characters are separate measures, not comparable word counts.".into(),
            ..Default::default()
        };
        for s in bundle.scenes{
            let text=plain_text(&s.scene.document);
            stats.unicode_scalars+=text.chars().count();
            stats.graphemes+=text.graphemes(true).count();
            stats.whitespace_words+=text.split_whitespace().count();
            stats.cjk_characters+=text.chars().filter(|c|matches!(*c as u32,0x3400..=0x4dbf|0x4e00..=0x9fff|0xf900..=0xfaff|0x20000..=0x323af)).count();
        }
        Ok(stats)
    }
    pub async fn load_project_data(&self,pid:&str,kind:&str)->Result<Option<ProjectDataSnapshot>,
    String>{
        validate_kind(kind)?;
        let row:Option<(String,String,String)>=sqlx::query_as("SELECT r.id,r.payload,r.checksum FROM project_data d JOIN revision_envelope r ON r.id=d.head WHERE d.project_id=? AND d.kind=?").bind(pid).bind(kind).fetch_optional(&self.pool).await.map_err(err)?;
        row.map(|(revision_id,payload,checksum)|{
            if hash(payload.as_bytes())!=checksum{
                return Err("Project data checksum failed. Stored data was preserved.".into())
            }
            let payload:Value=serde_json::from_str(&payload).map_err(|_|"Invalid saved project data.")?;
            validate_data(kind,&payload)?;
            Ok(ProjectDataSnapshot{
                revision_id,payload
            }
            )
        }
        ).transpose()
    }
    pub async fn save_project_data(&self,pid:&str,kind:&str,payload:Value,parent:Option<String>)->Result<ProjectDataSnapshot,String>{
        self.commit_project_data(pid,kind,payload,parent,None).await
    }
    pub async fn save_project_data_checked(&self,pid:&str,kind:&str,payload:Value,parent:Option<String>,dependencies:&[SourceDependency])->Result<ProjectDataSnapshot,String>{
        self.commit_project_data(pid,kind,payload,parent,Some(dependencies)).await
    }
    async fn commit_project_data(&self,pid:&str,kind:&str,payload:Value,parent:Option<String>,dependencies:Option<&[SourceDependency]>)->Result<ProjectDataSnapshot,String>{
        validate_kind(kind)?;
        if kind=="branchLineage"{return Err("Branch lineage is created only by the native branch operation.".into())}
        validate_data(kind,&payload)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        let current:Option<String>=sqlx::query_scalar("SELECT head FROM project_data WHERE project_id=? AND kind=?").bind(pid).bind(kind).fetch_optional(&mut *tx).await.map_err(err)?;
        if current!=parent{
            return Err("Conflict: another window saved this project data. Pending changes remain local.".into())
        }
        if kind=="exportProfile"{export_profile_state(&mut tx,pid).await?;}
        if kind=="chapterNumbering"&&chapter_numbering_state(&mut tx,pid).await?.2{return Err("Conflict: resolve competing chapter numbering before saving.".into())}
        if kind=="universeCatalogue"{
            if let Some(head)=&current{validate_current_source(&mut tx,pid,&format!("{pid}:{kind}"),"projectData",head).await?;let (old,checksum):(String,String)=sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=?").bind(head).fetch_one(&mut *tx).await.map_err(err)?;if hash(old.as_bytes())!=checksum{return Err("Universe catalogue checksum failed.".into())}crate::universe::catalogue(&serde_json::from_str(&old).map_err(|_|"Invalid saved universe catalogue.")?)?;}else{let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revision_envelope WHERE project_id=? AND entity_id=? AND entity_type='projectData')").bind(pid).bind(format!("{pid}:{kind}")).fetch_one(&mut *tx).await.map_err(err)?;if exists{return Err("Conflict: resolve retained catalogue heads before saving.".into())}}
            crate::universe::references(&mut tx,pid,&payload,false).await?;
        }
        if let Some(dependencies)=dependencies{
            if let Some(head)=&current{validate_current_source(&mut tx,pid,&format!("{pid}:{kind}"),"projectData",head).await?;}
            validate_dependencies(&mut tx,pid,dependencies).await?;
        }
        if let Some(head)=&current{
            let old:String=sqlx::query_scalar("SELECT payload FROM revision_envelope WHERE id=?").bind(head).fetch_one(&mut *tx).await.map_err(err)?;
            let old:Value=serde_json::from_str(&old).map_err(|_|"Invalid saved project data.")?;
            validate_data_transition(kind,&old,&payload)?;
        }
        if kind=="sceneTranslations"{crate::translations::validate_sources(&mut tx,pid,&payload).await?;}
        if ["planning","writingSystems","sceneMetadata","paragraphAlternatives","notebookTools"].contains(&kind){
            let ids:Vec<String>=sqlx::query_scalar("SELECT s.id FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=?").bind(pid).fetch_all(&mut *tx).await.map_err(err)?;
            validate_scene_links(&payload,&ids.into_iter().collect())?;
        }
        if kind=="paragraphAlternatives"{crate::paragraph_alternatives::references(&mut tx,pid,&payload).await?;}
        if kind=="notebookTools"{
            if let Some(head)=&current{validate_current_source(&mut tx,pid,&format!("{pid}:{kind}"),"projectData",head).await?;}
            else{let retained:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revision_envelope WHERE project_id=? AND entity_id=? AND entity_type='projectData')").bind(pid).bind(format!("{pid}:{kind}")).fetch_one(&mut *tx).await.map_err(err)?;if retained{return Err("Conflict: resolve retained notebook tool heads before saving.".into())}}
            crate::notebook_tools::references(&mut tx,pid,&payload).await?;
        }
        let rid=id();
        append(&mut tx,pid,&format!("{pid}:{kind}"),"projectData",current.into_iter().collect(),payload.clone(),kind,&rid).await?;
        sqlx::query("INSERT INTO project_data(project_id,kind,head) VALUES(?,?,?) ON CONFLICT(project_id,kind) DO UPDATE SET head=excluded.head").bind(pid).bind(kind).bind(&rid).execute(&mut *tx).await.map_err(err)?;
        tx.commit().await.map_err(err)?;
        Ok(ProjectDataSnapshot{
            revision_id:rid,payload
        }
        )
    }
    pub async fn load_entity_revision(&self,pid:&str,entity:&str,kind:&str,rid:&str)->Result<Revision,String>{
        if !["scene","tombstone","projectData","project","chapter","scenePlacement","asset"].contains(&kind){return Err("Unsupported revision object type.".into())}
        let alternate=if kind=="scene"{"tombstone"}else{kind};
        let row=sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.id=? AND r.project_id=? AND r.entity_id=? AND r.entity_type IN(?,?)").bind(rid).bind(pid).bind(entity).bind(kind).bind(alternate).fetch_optional(&self.pool).await.map_err(err)?.ok_or("Revision does not belong to this project and object.")?;
        let revision=revision(row)?;if revision.schema_version!=1||hash(serde_json::to_string(&revision.payload).unwrap().as_bytes())!=revision.checksum{return Err("Revision schema or checksum failed. Stored data was preserved.".into())}
        if revision.entity_type=="scene"{let scene:Scene=serde_json::from_value(revision.payload.clone()).map_err(|_|"Invalid scene revision.")?;validate(&scene)?;}else if revision.entity_type=="projectData"{validate_kind(&revision.operation)?;validate_data(&revision.operation,&revision.payload)?;}
        Ok(revision)
    }
    pub async fn load_project_data_revision(&self,pid:&str,kind:&str,rid:&str)->Result<ProjectDataSnapshot,String>{
        validate_kind(kind)?;
        let row:(String,String,String)=sqlx::query_as("SELECT id,payload,checksum FROM revision_envelope WHERE id=? AND project_id=? AND entity_id=? AND entity_type='projectData' AND operation=?").bind(rid).bind(pid).bind(format!("{pid}:{kind}")).bind(kind).fetch_optional(&self.pool).await.map_err(err)?.ok_or("Revision does not belong to this project data.")?;
        if hash(row.1.as_bytes())!=row.2{return Err("Project data revision checksum failed.".into())}
        let payload:Value=serde_json::from_str(&row.1).map_err(|_|"Unsupported project data revision.")?;validate_data(kind,&payload)?;Ok(ProjectDataSnapshot{revision_id:row.0,payload})
    }
    pub async fn project_data_history(&self,pid:&str,kind:&str)->Result<Vec<Revision>,
    String>{
        validate_kind(kind)?;
        sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.project_id=? AND r.entity_id=? ORDER BY m.sequence DESC").bind(pid).bind(format!("{pid}:{kind}")).fetch_all(&self.pool).await.map_err(err)?.into_iter().map(revision).collect()
    }
    pub async fn put_asset(&self,pid:&str,asset:ProjectAsset)->Result<(),
    String>{
        validate_asset(&asset)?;
        let mut tx=self.pool.begin().await.map_err(err)?;
        let existing:Option<String>=sqlx::query_scalar("SELECT checksum FROM project_asset WHERE project_id=? AND id=?").bind(pid).bind(&asset.id).fetch_optional(&mut *tx).await.map_err(err)?;
        if let Some(checksum)=existing{
            if checksum==asset.checksum{
                return Ok(())
            }
            return Err("An asset with this identity already exists. Use a new asset ID.".into())
        }
        sqlx::query("INSERT INTO project_asset(project_id,id,path,mime_type,content,checksum) VALUES(?,?,?,?,?,?)").bind(pid).bind(&asset.id).bind(&asset.path).bind(&asset.mime_type).bind(&asset.content).bind(&asset.checksum).execute(&mut *tx).await.map_err(err)?;
        append(&mut tx,pid,&asset.id,"asset",vec![],serde_json::json!({
            "id":asset.id,"path":asset.path,"mimeType":asset.mime_type,"checksum":asset.checksum,"size":asset.content.len()
        }
        ),"add",&id()).await?;
        tx.commit().await.map_err(err)?;
        Ok(())
    }
    pub async fn list_assets(&self,pid:&str)->Result<Vec<ProjectAsset>,
    String>{
        let rows=sqlx::query("SELECT id,path,mime_type,content,checksum FROM project_asset WHERE project_id=? ORDER BY path").bind(pid).fetch_all(&self.pool).await.map_err(err)?;
        Ok(rows.into_iter().map(|r|ProjectAsset{
            id:r.get("id"),path:r.get("path"),mime_type:r.get("mime_type"),content:r.get("content"),checksum:r.get("checksum")
        }
        ).collect())
    }
    pub async fn export_archive(&self,pid:&str)->Result<String,
    String>{
        // All writers use the one pool connection. Capture a coherent archive in a read transaction.
        let mut tx=self.pool.begin().await.map_err(err)?;
        // Reject definitely oversized exports before allocating every manuscript/history/BLOB.
        let (records,minimum_bytes):(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM chapter WHERE project_id=?1)+(SELECT count(*) FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=?1)+(SELECT count(*) FROM revision_envelope WHERE project_id=?1)+(SELECT count(*) FROM project_asset WHERE project_id=?1),coalesce((SELECT sum(length(CAST(payload AS BLOB))) FROM revision_envelope WHERE project_id=?1),0)+coalesce((SELECT sum(length(CAST(r.payload AS BLOB))) FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id JOIN revision_envelope r ON r.id=s.head WHERE c.project_id=?1),0)+coalesce((SELECT sum(length(content)) FROM project_asset WHERE project_id=?1),0)").bind(pid).fetch_one(&mut *tx).await.map_err(err)?;
        if records>RECORD_LIMIT as i64{return Err("Too many archive records; history was preserved. Use a consistent database backup.".into())}
        if minimum_bytes>ARCHIVE_LIMIT as i64{return Err("Archive exceeds the 64 MiB portable limit; history was preserved. Use a consistent database backup.".into())}
        let bundle=read_bundle(&mut tx,pid).await?;
        let revisions=sqlx::query("SELECT r.*,m.sequence FROM revision_envelope r JOIN change_marker m ON m.revision_id=r.id WHERE r.project_id=? ORDER BY m.sequence").bind(pid).fetch_all(&mut *tx).await.map_err(err)?.into_iter().map(revision).collect::<Result<Vec<_>,
        _>>()?;
        let project_data=sqlx::query("SELECT kind,head FROM project_data WHERE project_id=? ORDER BY kind").bind(pid).fetch_all(&mut *tx).await.map_err(err)?.into_iter().map(|r|ProjectDataEntry{
            kind:r.get("kind"),revision_id:r.get("head")
        }
        ).collect();
        let assets=sqlx::query("SELECT id,path,mime_type,content,checksum FROM project_asset WHERE project_id=? ORDER BY path").bind(pid).fetch_all(&mut *tx).await.map_err(err)?.into_iter().map(|r|ProjectAsset{
            id:r.get("id"),path:r.get("path"),mime_type:r.get("mime_type"),content:r.get("content"),checksum:r.get("checksum")
        }
        ).collect();
        tx.commit().await.map_err(err)?;
        #[cfg(test)] crate::recovery::checkpoint("archive-export");
        let data=ArchiveData{
            bundle,
            revisions,
            project_data,
            assets
        };
        validate_archive_data(&data)?;
        let content=serde_json::to_string(&data).map_err(|_|"Could not encode archive.")?;
        let archive=Archive{
            format:"novel-workspace".into(),
            version:1,
            entries:vec![ArchiveEntry{
                path:"project.json".into(),size:content.len(),sha256:hash(content.as_bytes()),content
            }
            ]
        };
        let encoded=serde_json::to_string(&archive).map_err(|_|"Could not encode archive.")?;
        if encoded.len()>ARCHIVE_LIMIT{
            return Err("Archive exceeds the 64 MiB portable limit.".into())
        }
        Ok(encoded)
    }
    pub async fn import_archive(&self,encoded:&str)->Result<ProjectBundle,String>{self.import_archive_internal(encoded,None).await}
    pub(crate) async fn import_archive_internal(&self,encoded:&str,branch_title:Option<&str>)->Result<ProjectBundle,
    String>{
        if encoded.len()>ARCHIVE_LIMIT{
            return Err("Archive exceeds the 64 MiB portable limit.".into())
        }
        let archive:Archive=serde_json::from_str(encoded).map_err(|_|"Invalid archive manifest.")?;
        if archive.format!="novel-workspace"||archive.version!=1||archive.entries.len()!=1{
            return Err("Unsupported archive format/version or entries.".into())
        }
        let entry=&archive.entries[0];
        if entry.path!="project.json"||entry.size!=entry.content.len()||entry.sha256!=hash(entry.content.as_bytes()){
            return Err("Archive path, size or checksum failed validation.".into())
        }
        let mut data:ArchiveData=serde_json::from_str(&entry.content).map_err(|_|"Invalid archive project schema.")?;
        validate_archive_data(&data)?;
        let old_pid=data.bundle.project.id.clone();
        let pid=id();
        let mut ids=HashMap::from([(old_pid.clone(),pid.clone())]);
        for c in &data.bundle.chapters{
            ids.insert(c.id.clone(),id());
        }
        for s in &data.bundle.scenes{
            ids.insert(s.id.clone(),id());
        }
        for a in &data.assets{
            ids.insert(a.id.clone(),id());
        }
        for r in &data.revisions{
            ids.insert(r.id.clone(),id());
        }
        let mut tx=self.pool.begin().await.map_err(err)?;
        sqlx::query("INSERT INTO project(id,title,created_at) VALUES(?,?,?)").bind(&pid).bind(&data.bundle.project.title).bind(&data.bundle.project.created_at).execute(&mut *tx).await.map_err(err)?;
        #[cfg(test)] crate::recovery::checkpoint("archive-import");
        for c in &data.bundle.chapters{
            sqlx::query("INSERT INTO chapter(id,project_id,title,position) VALUES(?,?,?,?)").bind(&ids[&c.id]).bind(&pid).bind(&c.title).bind(c.position).execute(&mut *tx).await.map_err(err)?;
        }
        // Parents precede children, while imported revision origins remain provenance only.
        data.revisions.sort_by_key(|r|r.sequence);
        for r in &mut data.revisions {
            let entity=if r.entity_type=="projectData"{
                format!("{}:{}",pid,r.operation)
            }
            else{
                ids.get(&r.entity_id).cloned().ok_or("Archive references an unknown entity.")?
            };
            remap_payload(&mut r.payload,&ids);
            let payload=serde_json::to_string(&r.payload).unwrap();
            let parents=r.parents.iter().map(|p|ids[p].clone()).collect::<Vec<_>>();
            sqlx::query("INSERT INTO revision_envelope(id,project_id,entity_id,entity_type,parents,payload,operation,schema_version,checksum,created_at,origin_epoch,origin_device) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)").bind(&ids[&r.id]).bind(&pid).bind(entity).bind(&r.entity_type).bind(serde_json::to_string(&parents).unwrap()).bind(&payload).bind(&r.operation).bind(r.schema_version as i64).bind(hash(payload.as_bytes())).bind(&r.created_at).bind(&r.origin_epoch).bind(&r.origin_device).execute(&mut *tx).await.map_err(err)?;
            sqlx::query("INSERT INTO change_marker(revision_id) VALUES(?)").bind(&ids[&r.id]).execute(&mut *tx).await.map_err(err)?;
        }
        for s in &data.bundle.scenes{
            sqlx::query("INSERT INTO manuscript_scene(id,chapter_id,position,head) VALUES(?,?,?,?)").bind(&ids[&s.id]).bind(&ids[&s.chapter_id]).bind(s.position).bind(&ids[&s.revision_id]).execute(&mut *tx).await.map_err(err)?;
        }
        for d in &data.project_data{
            sqlx::query("INSERT INTO project_data(project_id,kind,head) VALUES(?,?,?)").bind(&pid).bind(&d.kind).bind(&ids[&d.revision_id]).execute(&mut *tx).await.map_err(err)?;
        }
        for d in &data.project_data{if d.kind=="universeCatalogue"{let payload:String=sqlx::query_scalar("SELECT payload FROM revision_envelope WHERE id=?").bind(&ids[&d.revision_id]).fetch_one(&mut *tx).await.map_err(err)?;crate::universe::references(&mut tx,&pid,&serde_json::from_str(&payload).map_err(|_|"Invalid universe catalogue.")?,true).await?;}}
        for a in &data.assets{
            sqlx::query("INSERT INTO project_asset(project_id,id,path,mime_type,content,checksum) VALUES(?,?,?,?,?,?)").bind(&pid).bind(&ids[&a.id]).bind(&a.path).bind(&a.mime_type).bind(&a.content).bind(&a.checksum).execute(&mut *tx).await.map_err(err)?;
        }
        for scene in &data.bundle.scenes{
            recompute_scene_heads(&mut tx,&ids[&scene.id]).await?;
        }
        if let Some(title)=branch_title{crate::branches::record_origin(&mut tx,&pid,&old_pid,title,&data.bundle,&data.project_data,&ids,encoded).await?;}
        tx.commit().await.map_err(err)?;
        self.open_project(&pid).await
    }
}
async fn mirror(tx:&mut Transaction<'_,Sqlite>,rid:&str,parent:Option<&str>,scene:&Scene)->Result<(),
String>{
    sqlx::query("INSERT INTO scene_revision(id,parent,payload) VALUES(?,?,?)").bind(rid).bind(parent).bind(serde_json::to_string(scene).unwrap()).execute(&mut **tx).await.map_err(err)?;
    sqlx::query("INSERT INTO scene_head(singleton,revision_id) VALUES(1,?) ON CONFLICT(singleton) DO UPDATE SET revision_id=excluded.revision_id").bind(rid).execute(&mut **tx).await.map_err(err)?;
    Ok(())
}
async fn chapter_numbering_state(tx:&mut Transaction<'_,Sqlite>,pid:&str)->Result<(i64,Option<String>,bool),String>{
    let head:Option<String>=sqlx::query_scalar("SELECT head FROM project_data WHERE project_id=? AND kind='chapterNumbering'").bind(pid).fetch_optional(&mut **tx).await.map_err(err)?;
    let heads:Vec<String>=sqlx::query_scalar("SELECT r.id FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type='projectData' AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.project_id=r.project_id AND c.entity_id=r.entity_id AND c.entity_type=r.entity_type AND p.value=r.id)").bind(pid).bind(format!("{pid}:chapterNumbering")).fetch_all(&mut **tx).await.map_err(err)?;
    let conflicted=heads.len()>1||heads.first()!=head.as_ref();
    let start=if let Some(rid)=&head{
        let (payload,checksum):(String,String)=sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=? AND project_id=? AND entity_id=? AND entity_type='projectData' AND operation='chapterNumbering'").bind(rid).bind(pid).bind(format!("{pid}:chapterNumbering")).fetch_one(&mut **tx).await.map_err(err)?;
        if hash(payload.as_bytes())!=checksum{return Err("Chapter numbering checksum failed. Stored data was preserved.".into())}
        let payload:Value=serde_json::from_str(&payload).map_err(|_|"Invalid chapter numbering.")?;validate_data("chapterNumbering",&payload)?;payload["start"].as_i64().unwrap()
    }else{1};
    Ok((start,head,conflicted))
}
async fn metadata_parent(tx:&mut Transaction<'_,Sqlite>,eid:&str,kind:&str)->Result<Vec<String>,String>{
    let heads:Vec<String>=sqlx::query_scalar("SELECT r.id FROM revision_envelope r WHERE r.entity_id=? AND r.entity_type=? AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.entity_id=r.entity_id AND c.entity_type=r.entity_type AND p.value=r.id)").bind(eid).bind(kind).fetch_all(&mut **tx).await.map_err(err)?;
    if heads.len()>1{return Err("Conflict: resolve competing metadata revisions before changing this object.".into())}Ok(heads)
}
async fn metadata_snapshot(tx:&mut Transaction<'_,Sqlite>,eid:&str,kind:&str)->Result<Value,String>{
    let title_query=if kind=="project"{"SELECT title FROM project WHERE id=?"}else{"SELECT title FROM chapter WHERE id=?"};
    let order_query=if kind=="project"{"SELECT id FROM chapter WHERE project_id=? ORDER BY position,id"}else{"SELECT id FROM manuscript_scene WHERE chapter_id=? ORDER BY position,id"};
    let title:String=sqlx::query_scalar(title_query).bind(eid).fetch_one(&mut **tx).await.map_err(err)?;
    let order:Vec<String>=sqlx::query_scalar(order_query).bind(eid).fetch_all(&mut **tx).await.map_err(err)?;
    Ok(serde_json::json!({"title":title,"order":order}))
}
async fn read_bundle(tx:&mut Transaction<'_,Sqlite>,pid:&str)->Result<ProjectBundle,
String>{
    let row=sqlx::query("SELECT id,title,created_at FROM project WHERE id=?").bind(pid).fetch_optional(&mut **tx).await.map_err(err)?.ok_or("Project not found.")?;
    let project=Project{
        id:row.get("id"),
        title:row.get("title"),
        created_at:row.get("created_at")
    };
    let chapters=sqlx::query("SELECT id,project_id,title,position FROM chapter WHERE project_id=? ORDER BY position,id").bind(pid).fetch_all(&mut **tx).await.map_err(err)?.into_iter().map(|r|Chapter{
        id:r.get("id"),project_id:r.get("project_id"),title:r.get("title"),position:r.get("position")
    }
    ).collect();
    let rows=sqlx::query("SELECT s.id,s.chapter_id,s.position,s.head,r.payload,r.checksum FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id JOIN revision_envelope r ON r.id=s.head WHERE c.project_id=? ORDER BY c.position,c.id,s.position,s.id").bind(pid).fetch_all(&mut **tx).await.map_err(err)?;
    let scenes=rows.into_iter().map(|r|Ok(SceneRecord{
        id:r.get("id"),chapter_id:r.get("chapter_id"),position:r.get("position"),revision_id:r.get("head"),scene:decode_scene(r.get("payload"),r.get("checksum"))?
    }
    )).collect::<Result<Vec<_>,
    String>>()?;
    let (chapter_number_start,chapter_numbering_revision_id,chapter_numbering_conflicted)=chapter_numbering_state(tx,pid).await?;
    Ok(ProjectBundle{
        project,chapters,scenes,chapter_number_start,chapter_numbering_revision_id,chapter_numbering_conflicted
    }
    )
}
async fn export_profile_state(tx:&mut Transaction<'_,Sqlite>,pid:&str)->Result<(ExportProfile,Option<String>),String>{
    let entity=format!("{pid}:exportProfile");
    let leaves:Vec<String>=sqlx::query_scalar("SELECT r.id FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type='projectData' AND NOT EXISTS(SELECT 1 FROM revision_envelope child,json_each(child.parents) p WHERE child.project_id=r.project_id AND child.entity_id=r.entity_id AND child.entity_type=r.entity_type AND p.value=r.id) ORDER BY r.id").bind(pid).bind(&entity).fetch_all(&mut **tx).await.map_err(err)?;
    if leaves.len()>1{return Err("Conflict: resolve Word export style conflicts in Recovery before saving or exporting.".into())}
    let head:Option<String>=sqlx::query_scalar("SELECT head FROM project_data WHERE project_id=? AND kind='exportProfile'").bind(pid).fetch_optional(&mut **tx).await.map_err(err)?;
    if leaves.first()!=head.as_ref(){return Err("Word export style history does not match its saved version. Use Recovery; stored data was preserved.".into())}
    let Some(head)=head else{return Ok((ExportProfile::default(),None))};
    let (payload,checksum):(String,String)=sqlx::query_as("SELECT payload,checksum FROM revision_envelope WHERE id=? AND project_id=? AND entity_id=? AND entity_type='projectData' AND operation='exportProfile'").bind(&head).bind(pid).bind(&entity).fetch_optional(&mut **tx).await.map_err(err)?.ok_or("Invalid saved Word export style identity.")?;
    if hash(payload.as_bytes())!=checksum{return Err("Word export style checksum failed. Stored data was preserved.".into())}
    let profile:ExportProfile=serde_json::from_str(&payload).map_err(|_|"Invalid saved Word export style fields. Stored data was preserved.")?;
    profile.validate()?;
    Ok((profile,Some(head)))
}
pub(crate) fn validate_kind(kind:&str)->Result<(),
String>{
    if !["planning","writingSystems","languagePacks","settings","assistanceJobs","sceneMetadata","paragraphAlternatives","sceneTranslations","branchLineage","universeCatalogue","chapterNumbering","exportProfile","notebookTools"].contains(&kind){
        return Err("Unsupported project data kind.".into())
    }
    Ok(())
}
pub(crate) fn validate_data(kind:&str,payload:&Value)->Result<(),
String>{
    if !payload.is_object() || serde_json::to_vec(payload).map_err(|_|"Invalid project data.")?.len()>8_000_000{
        return Err("Project data must be an object below 8 MB.".into())
    }
    if (kind!="settings" || payload.get("schemaVersion").is_some()) && payload.get("schemaVersion").and_then(Value::as_u64)!=Some(1){
        return Err("Unsupported project data schema version.".into())
    }
    let bounded_array=|key:&str,
    max:usize,
    required:bool|->Result<(),
    String>{
        match payload.get(key){
            Some(value)=>{
                if !value.as_array().is_some_and(|v|v.len()<=max){
                    return Err(format!("Invalid or oversized {key} collection."))
                }
            },
            None if required=>return Err(format!("Missing {key} collection.")),
            _=>{
            }
        }
        Ok(())
    };
    match kind {
        "exportProfile"=>{let profile:ExportProfile=serde_json::from_value(payload.clone()).map_err(|_|"Invalid Word export style fields.")?;profile.validate()?;},
        "chapterNumbering"=>{if payload.as_object().is_none_or(|v|v.len()!=2)||!matches!(payload["start"].as_i64(),Some(0|1)){return Err("Chapter numbering requires exactly schemaVersion 1 and start 0 or 1.".into())}},
        "universeCatalogue"=>{crate::universe::catalogue(payload)?;},
        "paragraphAlternatives"=>crate::paragraph_alternatives::validate_payload(payload)?,
        "notebookTools"=>crate::notebook_tools::validate_payload(payload)?,
        "sceneTranslations"=>crate::translations::validate_payload(payload)?,
        "branchLineage"=>crate::branches::validate_lineage(payload)?,
        "languagePacks"=>{
            bounded_array("packs",20,true)?;
            let packs=payload["packs"].as_array().unwrap();
            let mut seen=HashSet::new();
            for pack in packs{
                if pack.get("schemaVersion").and_then(Value::as_u64)!=Some(1)||!pack.get("id").and_then(Value::as_str).is_some_and(|id|!id.is_empty()&&id.len()<=100&&id.bytes().next().is_some_and(|c|c.is_ascii_alphanumeric())&&id.bytes().all(|c|c.is_ascii_alphanumeric()||b"._-".contains(&c))&&seen.insert(id))||!pack.get("license").and_then(Value::as_str).is_some_and(|v|!v.trim().is_empty())||!pack.get("checksum").and_then(Value::as_str).is_some_and(|v|v.len()==64&&v.chars().all(|c|c.is_ascii_hexdigit()))||!pack.get("entries").is_some_and(Value::is_array){
                    return Err("Invalid language pack identity, schema, license or checksum metadata.".into())
                }
            }
        },
        "assistanceJobs"=>{
            bounded_array("jobs",1000,true)?;
            for job in payload["jobs"].as_array().unwrap(){
                if !job.is_object()||!job.get("request").is_some_and(Value::is_object)||!job.get("state").is_some_and(Value::is_string){
                    return Err("Invalid assistance job wrapper.".into())
                }
            }
        },
        "writingSystems"=>{
            bounded_array("systems",4096,true)?;
            bounded_array("assignments",10000,true)?;
            for system in payload["systems"].as_array().unwrap(){
                if system.get("schemaVersion").and_then(Value::as_u64)!=Some(1)||!system.get("mappingVersion").and_then(Value::as_u64).is_some_and(|v|v>0)||!system.get("id").is_some_and(Value::is_string){
                    return Err("Invalid writing system version/identity.".into())
                }
                if let Some(font)=system.get("font").filter(|v|!v.is_null()){
                    if font.get("license").and_then(|v|v.get("permissionConfirmed")).and_then(Value::as_bool)!=Some(true){
                        return Err("Font transfer requires the author's recorded permission.".into())
                    }
                }
            }
        },
        "sceneMetadata"=>{
            bounded_array("scenes",20000,true)?;let mut ids=HashSet::new();
            for record in payload["scenes"].as_array().unwrap(){
                let sid=record.get("sceneId").and_then(Value::as_str).ok_or("Invalid scene metadata identity.")?;
                if !valid_id(sid)||!ids.insert(sid)||!record.get("synopsis").and_then(Value::as_str).is_some_and(|v|v.chars().count()<=100000)||!record.get("status").and_then(Value::as_str).is_some_and(|v|["draft","revising","complete"].contains(&v)){return Err("Invalid scene metadata identity, synopsis or status.".into())}
                if !record.get("viewpointId").is_some_and(|v|v.is_null()||v.as_str().is_some_and(|v|!v.is_empty()&&v.len()<=200)){return Err("Invalid scene viewpoint identity.".into())}
                for (key,max_count,max_length) in [("tags",100,100),("eventIds",100,200)]{if !record.get(key).and_then(Value::as_array).is_some_and(|values|values.len()<=max_count&&values.iter().all(|v|v.as_str().is_some_and(|s|!s.is_empty()&&s.chars().count()<=max_length))){return Err(format!("Invalid scene metadata {key} collection."))}}
            }
        },
        "planning"=>{
            for field in ["fields","records","facts","boards","lanes","events","constraints","issues","plotPoints"]{
                bounded_array(field,20_000,false)?;
            }
        },
        "settings"=>{
            if let Some(assistance)=payload.get("assistance"){
                if assistance.get("schemaVersion").and_then(Value::as_u64)!=Some(1){
                    return Err("Unsupported assistance settings version.".into())
                }
                let providers=assistance.get("providers").and_then(Value::as_array).ok_or("Invalid provider settings.")?;
                if providers.len()!=2{
                    return Err("Exactly two local provider configurations are required.".into())
                }
                let mut kinds=HashSet::new();
                for p in providers{
                    let kind=p.get("kind").and_then(Value::as_str).ok_or("Invalid provider kind.")?;
                    if !["languagetool","llm"].contains(&kind)||!kinds.insert(kind)||!p.get("endpoint").and_then(Value::as_str).is_some_and(loopback_endpoint){
                        return Err("Provider configuration must use distinct supported kinds and literal loopback HTTP endpoints.".into())
                    }
                }
                for key in ["exceptions","suggestions"]{
                    if !assistance.get(key).and_then(Value::as_array).is_some_and(|v|v.len()<=10000){
                        return Err(format!("Invalid assistance {key} collection."))
                    }
                }
            }
        },
        _=>return Err("Unsupported project data kind.".into())
    }
    Ok(())
}
pub(crate) fn validate_data_transition(kind:&str,old:&Value,new:&Value)->Result<(),
String>{
    if kind=="paragraphAlternatives"{crate::paragraph_alternatives::transition(old,new)?;}
    if kind=="sceneTranslations"{crate::translations::transition(old,new)?;}
    if kind=="branchLineage"{crate::branches::lineage_transition(old,new)?;}
    if kind=="writingSystems"{
        let previous=old.get("systems").and_then(Value::as_array).ok_or("Invalid prior writing systems.")?;
        let next=new.get("systems").and_then(Value::as_array).ok_or("Invalid writing systems.")?;
        for definition in previous{
            let matched=next.iter().find(|d|d.get("id")==definition.get("id")&&d.get("mappingVersion")==definition.get("mappingVersion"));
            if matched!=Some(definition){
                return Err("Writing system versions are immutable and must be retained. Add a new mapping version instead.".into())
            }
        }
    }
    if kind=="languagePacks"{
        if let (Some(previous),Some(next))=(old.get("packs").and_then(Value::as_array),new.get("packs").and_then(Value::as_array)){
            for pack in previous{
                if let Some(matched)=next.iter().find(|p|p.get("id")==pack.get("id")&&p.get("version")==pack.get("version")){
                    if matched!=pack{
                        return Err("A language pack version cannot be replaced with different content.".into())
                    }
                }
            }
        }
    }
    Ok(())
}
fn loopback_endpoint(endpoint:&str)->bool{
    let e=endpoint.strip_suffix('/').unwrap_or(endpoint);
    let Some(rest)=e.strip_prefix("http://")else{
        return false
    };
    let port=if let Some(rest)=rest.strip_prefix("127.0.0.1"){
        rest
    }
    else if let Some(rest)=rest.strip_prefix("[::1]"){
        rest
    }
    else{
        return false
    };
    if port.is_empty(){
        return true
    }
    port.strip_prefix(':').is_some_and(|p|!p.is_empty()&&p.len()<=5&&p.bytes().all(|c|c.is_ascii_digit())&&p.parse::<u16>().is_ok_and(|p|p>0))
}
pub(crate) fn valid_id(value:&str)->bool{
    !value.is_empty() && value.len()<=128 && value.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_'||c==':')
}
pub(crate) fn safe_path(value:&str)->bool{
    !value.is_empty()&&value.len()<=512&&!value.starts_with('/')&&!value.contains('\\')&&!value.contains(':')&&value.split('/').all(|p|!p.is_empty()&&p!="."&&p!="..")
}
pub(crate) fn validate_asset(a:&ProjectAsset)->Result<(),
String>{
    if !valid_id(&a.id)||!safe_path(&a.path)||!a.path.starts_with("assets/")||a.content.len()>16*1024*1024||hash(&a.content)!=a.checksum||!["font/ttf","font/otf","font/woff","font/woff2","application/json","text/plain"].contains(&a.mime_type.as_str()){
        return Err("Invalid asset path, format, size or checksum. Fonts and language data only are portable assets.".into())
    }
    Ok(())
}
pub(crate) fn validate_scene_links(value:&Value,ids:&HashSet<String>)->Result<(),
String>{
    match value{
        Value::Object(map)=>{
            if map.get("kind").and_then(Value::as_str)==Some("scene"){
                if !map.get("id").and_then(Value::as_str).is_some_and(|id|ids.contains(id)){
                    return Err("Planning data references a missing scene.".into())
                }
            }
            if let Some(sid)=map.get("sceneId"){if !sid.as_str().is_some_and(|sid|ids.contains(sid)){return Err("Project data references a missing scene.".into())}}
            if let Some(scene_ids)=map.get("sceneIds"){
                for id in scene_ids.as_array().ok_or("Invalid planning scene links.")?{
                    if !id.as_str().is_some_and(|id|ids.contains(id)){
                        return Err("Planning event references a missing scene.".into())
                    }
                }
            }
            for v in map.values(){
                validate_scene_links(v,ids)?;
            }
        },
        Value::Array(items)=>{
            for v in items{
                validate_scene_links(v,ids)?;
            }
        },
        _=>{
        }
    }
    Ok(())
}
pub(crate) fn remap_payload(value:&mut Value,ids:&HashMap<String,String>){
    match value{
        Value::Object(map)=>{
            if map.get("kind").and_then(Value::as_str)==Some("scene") || (map.contains_key("mimeType") && map.contains_key("checksum") && map.contains_key("size")){
                if let Some(Value::String(v))=map.get_mut("id"){
                    if let Some(new)=ids.get(v){
                        *v=new.clone();
                    }
                }
            }
            for (key,v) in map.iter_mut(){
                if ["order","sceneIds","eventIds"].contains(&key.as_str()){
                    if let Some(items)=v.as_array_mut(){
                        for item in items{
                            if let Some(old)=item.as_str(){
                                if let Some(new)=ids.get(old){
                                    *item=Value::String(new.clone())
                                }
                            }
                        }
                    }
                }
                else if ["projectId","chapterId","fromChapterId","assetId","fontAssetId","sceneId","revisionId","sceneRevisionId","viewpointId","branchProjectId","branchSceneId","branchBaselineRevisionId","branchPlanningBaselineRevisionId","branchId"].contains(&key.as_str()){
                    if let Some(old)=v.as_str(){
                        if let Some(new)=ids.get(old){
                            *v=Value::String(new.clone())
                        }
                    }
                }
                else{
                    remap_payload(v,ids);
                }
            }
        },
        Value::Array(items)=>{
            for v in items{
                remap_payload(v,ids)
            }
        },
        _=>{
        }
    }
}
fn validate_archive_data(data:&ArchiveData)->Result<(),
String>{
    let b=&data.bundle;
    title(&b.project.title)?;
    if !valid_id(&b.project.id)||b.chapters.len()+b.scenes.len()+data.revisions.len()+data.assets.len()>RECORD_LIMIT{
        return Err("Invalid project identity or too many archive records.".into())
    }
    let mut entities=HashSet::from([b.project.id.clone()]);
    let mut chapters=HashSet::new();
    let mut chapter_positions=HashSet::new();
    for c in &b.chapters{
        title(&c.title)?;
        if !valid_id(&c.id)||c.project_id!=b.project.id||c.position<0||!chapter_positions.insert(c.position)||!entities.insert(c.id.clone()){
            return Err("Invalid chapter identity/order.".into())
        }
        chapters.insert(c.id.clone());
    }
    let mut scenes=HashSet::new();
    let mut positions=HashSet::new();
    for s in &b.scenes{
        validate(&s.scene)?;
        if !valid_id(&s.id)||!chapters.contains(&s.chapter_id)||s.position<0||!positions.insert((s.chapter_id.clone(),s.position))||!entities.insert(s.id.clone()){
            return Err("Invalid scene identity/order.".into())
        }
        scenes.insert(s.id.clone());
    }
    let mut asset_paths=HashSet::new();
    for a in &data.assets{
        validate_asset(a)?;
        if !entities.insert(a.id.clone())||!asset_paths.insert(a.path.clone()){
            return Err("Duplicate asset identity or path.".into())
        }
    }
    let mut kinds=HashSet::new();
    for d in &data.project_data{
        validate_kind(&d.kind)?;
        if !kinds.insert(d.kind.clone()){
            return Err("Duplicate project data kind.".into())
        }
        if !entities.insert(format!("{}:{}",b.project.id,d.kind)){
            return Err("Project data identity collides with another story object.".into())
        }
    }
    let asset_checksums=data.assets.iter().map(|a|(a.id.as_str(),a.checksum.as_str())).collect::<HashMap<_,_>>();
    let mut revs=HashMap::new();
    let mut sequences=HashSet::new();
    for r in &data.revisions{
        if !valid_id(&r.id)||entities.contains(&r.id)||revs.insert(r.id.clone(),r).is_some()||r.schema_version!=1||r.project_id!=b.project.id||!entities.contains(&r.entity_id)||r.sequence<=0||!sequences.insert(r.sequence)||!valid_id(&r.origin_epoch)||!valid_id(&r.origin_device)||r.created_at.is_empty()||r.created_at.len()>64||hash(serde_json::to_string(&r.payload).unwrap().as_bytes())!=r.checksum{
            return Err("Invalid revision identity, schema, origin or checksum.".into())
        }
        match r.entity_type.as_str(){
            "scene"=>{
                let s:Scene=serde_json::from_value(r.payload.clone()).map_err(|_|"Invalid scene revision.")?;
                validate(&s)?;
                if !scenes.contains(&r.entity_id){
                    return Err("Scene revision has wrong entity.".into())
                }
            },
            "projectData"=>{
                validate_kind(&r.operation)?;
                validate_data(&r.operation,&r.payload)?;
                if r.operation=="writingSystems"{for definition in r.payload["systems"].as_array().unwrap(){if let Some(font)=definition.get("font").filter(|v|!v.is_null()){let asset=font.get("fontAssetId").and_then(Value::as_str).ok_or("Invalid font asset reference.")?;let checksum=font.get("checksum").and_then(Value::as_str).ok_or("Invalid font asset checksum.")?;if asset_checksums.get(asset).copied()!=Some(checksum){return Err("A referenced writing-system font is missing or has a different checksum. Finish its verified transfer before exporting a complete archive.".into())}}}}

                if r.entity_id!=format!("{}:{}",b.project.id,r.operation){
                    return Err("Invalid project data revision identity.".into())
                }
                if ["planning","writingSystems","sceneMetadata","paragraphAlternatives","notebookTools"].contains(&r.operation.as_str()){
                    validate_scene_links(&r.payload,&scenes)?;
                }
            },
            "tombstone"=>{
                if !scenes.contains(&r.entity_id)||r.payload!=serde_json::json!({
                    "deleted":true
                }
                ){
                    return Err("Invalid tombstone revision.".into())
                }
            },
            "scenePlacement"=>{
                if !scenes.contains(&r.entity_id)||!r.payload.get("chapterId").and_then(Value::as_str).is_some_and(|c|chapters.contains(c)){
                    return Err("Invalid scene placement revision.".into())
                }
            },
            "project"=>{
                if r.entity_id!=b.project.id{
                    return Err("Project revision has the wrong object identity.".into())
                }
            },
            "chapter"=>{
                if !chapters.contains(&r.entity_id){
                    return Err("Chapter revision has the wrong object identity.".into())
                }
            },
            "asset"=>{
                if !data.assets.iter().any(|a|a.id==r.entity_id){
                    return Err("Asset revision has a missing asset.".into())
                }
            },
            _=>return Err("Unsupported revision entity type.".into())
        }
    }
    for r in &data.revisions{if r.operation=="paragraphAlternatives"{for scene in r.payload["scenes"].as_array().unwrap(){for group in scene["groups"].as_array().unwrap(){let source=revs.get(group["sceneRevisionId"].as_str().unwrap()).ok_or("Missing paragraph source revision.")?;if source.entity_type!="scene"||source.entity_id!=scene["sceneId"].as_str().unwrap(){return Err("Wrong paragraph source revision ownership.".into())}}}}}
    for r in &data.revisions{if r.entity_type=="projectData"&&r.operation=="notebookTools"{for reaction in r.payload["reactions"].as_array().unwrap(){let source=revs.get(reaction["revisionId"].as_str().unwrap()).ok_or("Missing notebook reaction source revision.")?;crate::notebook_tools::validate_reaction_source(reaction,source)?;}}}
    for r in &data.revisions{if r.operation=="sceneTranslations"{for draft in r.payload["drafts"].as_array().unwrap(){let source=revs.get(draft["sceneRevisionId"].as_str().unwrap()).ok_or("Missing translation source revision.")?;if source.entity_type!="scene"||source.entity_id!=draft["sceneId"].as_str().unwrap()||crate::plain_text(&source.payload["document"])!=draft["sourceText"].as_str().unwrap(){return Err("Wrong translation source revision ownership or text.".into())}}}}
    for r in &data.revisions{
        let mut unique=HashSet::new();
        for p in &r.parents{
            let parent=revs.get(p).ok_or("Missing revision parent.")?;
            if parent.entity_id!=r.entity_id||parent.sequence>=r.sequence||!unique.insert(p){
                return Err("Invalid revision ancestry.".into())
            }
            if parent.entity_type!=r.entity_type&&!(["scene","tombstone"].contains(&parent.entity_type.as_str())&&["scene","tombstone"].contains(&r.entity_type.as_str())){
                return Err("Revision parent has an incompatible object type.".into())
            }
            if r.entity_type=="projectData"{
                validate_data_transition(&r.operation,&parent.payload,&r.payload)?;
            }
        }
    }
    for s in &b.scenes{
        let head=revs.get(&s.revision_id).ok_or("Missing scene head.")?;
        if head.entity_id!=s.id||head.entity_type!="scene"||head.payload!=serde_json::to_value(&s.scene).unwrap(){
            return Err("Scene head does not match its content.".into())
        }
    }
    for d in &data.project_data{
        let head=revs.get(&d.revision_id).ok_or("Missing project data head.")?;
        if head.entity_id!=format!("{}:{}",b.project.id,d.kind)||head.operation!=d.kind{
            return Err("Invalid project data head.".into())
        }
    }
    Ok(())
}

pub(crate) async fn validate_current_source(tx:&mut Transaction<'_,Sqlite>,pid:&str,entity:&str,kind:&str,expected:&str)->Result<(),String>{
    let valid:Option<String>=match kind{
        "scene"=>sqlx::query_scalar("SELECT s.head FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE s.id=? AND c.project_id=? AND s.deleted=0").bind(entity).bind(pid).fetch_optional(&mut **tx).await.map_err(err)?,
        "projectData"=>sqlx::query_scalar("SELECT d.head FROM project_data d JOIN revision_envelope r ON r.id=d.head WHERE r.entity_id=? AND d.project_id=?").bind(entity).bind(pid).fetch_optional(&mut **tx).await.map_err(err)?,
        _=>return Err("Unsupported review dependency object type.".into())
    };
    if valid.as_deref()!=Some(expected){return Err("Conflict: a reviewed manuscript or supporting fact revision changed. No suggestion was applied.".into())}
    let query=if kind=="scene"{"SELECT r.id FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type IN('scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.project_id=r.project_id AND c.entity_id=r.entity_id AND c.entity_type IN('scene','tombstone') AND p.value=r.id)"}else{"SELECT r.id FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type='projectData' AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.project_id=r.project_id AND c.entity_id=r.entity_id AND c.entity_type='projectData' AND p.value=r.id)"};
    let heads:Vec<String>=sqlx::query_scalar(query).bind(pid).bind(entity).fetch_all(&mut **tx).await.map_err(err)?;
    if heads.len()!=1||heads[0]!=expected{return Err("Conflict: a reviewed source has competing revisions. Resolve it before applying a suggestion.".into())}Ok(())
}

async fn validate_dependencies(tx:&mut Transaction<'_,Sqlite>,pid:&str,dependencies:&[SourceDependency])->Result<(),String>{
    if dependencies.len()>50{return Err("Supporting scope exceeds 50 saved sources.".into())}
    let mut seen=HashSet::new();
    for source in dependencies{
        if !valid_id(&source.entity_id)||!valid_id(&source.revision_id)||!seen.insert((&source.entity_id,&source.entity_type)){return Err("Invalid or duplicate source dependency.".into())}
        validate_current_source(tx,pid,&source.entity_id,&source.entity_type,&source.revision_id).await?;
    }
    Ok(())
}

async fn validate_resolution_heads(tx:&mut Transaction<'_,Sqlite>,pid:&str,sid:&str,parents:&[String])->Result<(),String>{
    let supplied=parents.iter().collect::<HashSet<_>>();
    if supplied.len()!=parents.len(){return Err("Duplicate resolution parent.".into())}
    let actual:Vec<String>=sqlx::query_scalar("SELECT r.id FROM revision_envelope r WHERE r.project_id=? AND r.entity_id=? AND r.entity_type IN('scene','tombstone') AND NOT EXISTS(SELECT 1 FROM revision_envelope c,json_each(c.parents) p WHERE c.project_id=r.project_id AND c.entity_id=r.entity_id AND c.entity_type IN('scene','tombstone') AND p.value=r.id)").bind(pid).bind(sid).fetch_all(&mut **tx).await.map_err(err)?;
    if actual.len()<2||actual.iter().collect::<HashSet<_>>()!=supplied{return Err("Conflict: acknowledge every current scene and deletion head; the versions changed before resolution.".into())}Ok(())
}
