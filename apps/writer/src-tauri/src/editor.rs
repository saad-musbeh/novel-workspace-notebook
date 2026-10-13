use novel_storage::{validate, Scene};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use tauri::Manager;

#[path = "editor_sessions.rs"]
mod editor_sessions;
use editor_sessions::{SessionRegistry,SessionUpdate,validate_resume_binding};

#[derive(Clone)]
pub struct EditorState(Arc<SessionRegistry<EditorSession>>);
impl Default for EditorState {fn default()->Self {Self(Arc::new(SessionRegistry::new(20_064)))}}
struct EditorSession { process:Option<EditorProcess>, binding:Value, directory:std::path::PathBuf, script:std::path::PathBuf }
impl Drop for EditorSession {fn drop(&mut self){drop(self.process.take());let _=std::fs::remove_dir_all(&self.directory);}}
impl EditorSession {
 fn process(&mut self)->Result<&mut EditorProcess,String>{
  if self.process.is_none(){let mut process=EditorProcess::start(&self.script,&self.directory)?;process.request(json!({"type":"resume"}))?;self.process=Some(process);}
  self.process.as_mut().ok_or_else(||"The native editor could not resume.".into())
 }
}
static ACTIVE_PROCESSES:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
struct ProcessPermit;
impl ProcessPermit {fn acquire()->Result<Self,String>{ACTIVE_PROCESSES.fetch_update(std::sync::atomic::Ordering::AcqRel,std::sync::atomic::Ordering::Acquire,|count|(count<16).then_some(count+1)).map_err(|_|"Too many active writing areas. Close a writing tab before opening another.")?;Ok(Self)}}
impl Drop for ProcessPermit {fn drop(&mut self){ACTIVE_PROCESSES.fetch_sub(1,std::sync::atomic::Ordering::AcqRel);}}

fn window_identity(owner:&str)->Result<(),String>{
    if owner.is_empty()||owner.len()>128||!owner.bytes().all(|b|b.is_ascii_alphanumeric()||b"_-:.".contains(&b)){return Err("Invalid editor window identity.".into())}Ok(())
}
fn session_key(session:Option<&str>,owner:&str)->Result<String,String>{
    window_identity(owner)?;
    if let Some(id)=session {if id.is_empty()||id.len()>128||id.starts_with("legacy:")||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b"_-:".contains(&b)){return Err("Invalid editor session identity.".into())}Ok(id.into())}
    else {Ok(format!("legacy:{owner}"))}
}
impl EditorState {
    /// Native window coordination only: the windows module must authorize the
    /// destination and repair pending transfers before releasing either window.
    pub async fn transfer_session(&self,session_id:&str,from_window:&str,to_window:&str)->Result<(),String>{
        let key=session_key(Some(session_id),from_window)?;window_identity(to_window)?;
        let registry=self.0.clone();let from=from_window.to_owned();let to=to_window.to_owned();
        tauri::async_runtime::spawn_blocking(move||registry.transfer_group(&key,&from,&to)).await.map_err(|_|"Editor ownership transfer stopped.")?
    }
    pub async fn release_session(&self,session_id:&str,owner_window:&str)->Result<(),String>{
        let key=session_key(Some(session_id),owner_window)?;let registry=self.0.clone();let owner=owner_window.to_owned();
        tauri::async_runtime::spawn_blocking(move||registry.release_group(&key,&owner)).await.map_err(|_|"Editor session release stopped.")?
    }
    pub async fn release_window_sessions(&self,owner_window:&str)->Result<(),String>{
        window_identity(owner_window)?;let registry=self.0.clone();let owner=owner_window.to_owned();
        tauri::async_runtime::spawn_blocking(move||registry.release_window(&owner)).await.map_err(|_|"Editor window release stopped.")?
    }
}
struct EditorProcess { child: Child, input: ChildStdin, output: BufReader<ChildStdout>, _permit:ProcessPermit }
impl Drop for EditorProcess {
    fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); }
}
impl EditorProcess {
    fn start(script:&std::path::Path,directory:&std::path::Path)->Result<Self,String>{
        let permit=ProcessPermit::acquire()?;
        let mut child=Command::new(script).arg(directory).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"Could not start the installed native engine.")?;
        let input=child.stdin.take().ok_or("Could not reach native engine.")?;
        let output=BufReader::new(child.stdout.take().ok_or("Could not read native engine.")?);
        Ok(Self{child,input,output,_permit:permit})
    }
    fn request(&mut self, operation: Value) -> Result<Value, String> {
        let mut input = serde_json::to_vec(&operation).map_err(|_| "Could not encode editor operation.")?;
        if input.len() > 2_100_000 { return Err("Editor request exceeds its size limit.".into()); }
        input.push(b'\n');
        self.input.write_all(&input).and_then(|_| self.input.flush()).map_err(|_| "native editor stopped. Your last confirmed scene is preserved.")?;
        let mut line = Vec::new();
        (&mut self.output).take(16_000_001).read_until(b'\n', &mut line).map_err(|_| "Could not read edited scene.")?;
        if line.len() > 16_000_000 || line.last() != Some(&b'\n') { return Err("native editor stopped or returned an unsupported response.".into()); }
        let response: Value = serde_json::from_slice(&line).map_err(|_| "Could not read native editor response.")?;
        if response["ok"] != true { return Err(response["error"].as_str().unwrap_or("native editor failed.").into()); }
        let frame = response.get("frame").ok_or("Missing editor frame.")?.clone();
        if let Some(document) = frame.get("document") { validate(&Scene { schema_version: 1, title: String::new(), document: document.clone() })?; }
        Ok(frame)
    }
}
#[tauri::command]
pub async fn editor_open(app: tauri::AppHandle, window:tauri::Window, state: tauri::State<'_, EditorState>, companion: tauri::State<'_, crate::companion::CompanionState>, scene: Scene, width: Option<u32>, project_id: Option<String>, writing_system: Option<Value>, session_id:Option<String>) -> Result<Value, String> {
    validate(&scene)?;
    if let Some(id)=session_id.as_deref(){crate::windows::authorize_session(&window,id)?;}
    let owner=window.label().to_owned();let key=session_key(session_id.as_deref(),&owner)?;
    let resume=session_id.is_some();
    let book_scene=if let Some(id)=session_id.as_deref(){crate::windows::book_scene_binding(&window,id,project_id.as_deref())?}else{None};
    if let Some(scene_id)=&book_scene {companion.0.request("load_project_scene",json!({"projectId":project_id,"sceneId":scene_id})).await?;}
    let binding=json!({"projectId":project_id,"sceneId":book_scene,"writingSystem":writing_system});
    let mut font_asset: Option<Value> = None;
    if let Some(profile) = &writing_system {
        let pid=project_id.as_deref().ok_or("An alphabet requires an open project.")?;
        let saved=companion.0.request("load_project_data",json!({"projectId":pid,"kind":"writingSystems"})).await?;
        let definitions=saved["payload"]["systems"].as_array().ok_or("The alphabet is not saved in this project.")?;
        let definition=definitions.iter().find(|d|d["id"]==profile["definitionId"]&&d["mappingVersion"]==profile["mappingVersion"]).ok_or("The selected alphabet version is unavailable.")?;
        let font=&definition["font"];
        if !font.is_null() {
            if profile["fontAssetId"]!=font["fontAssetId"]||profile["fontChecksum"]!=font["checksum"]||profile["fontFamily"]!=font["family"] {return Err("Alphabet font configuration changed. Reopen the scene.".into())}
            let assets=companion.0.request("list_assets",json!({"projectId":pid})).await?;
            let asset=assets.as_array().and_then(|a|a.iter().find(|a|a["id"]==font["fontAssetId"]&&a["checksum"]==font["checksum"])).ok_or("The saved alphabet font asset is unavailable.")?;
            if !matches!(asset["mimeType"].as_str(),Some("font/ttf"|"font/otf")){return Err("Unsupported alphabet font format.".into())}
            font_asset=Some(json!({"asset":asset,"family":font["family"]}));
        } else if profile.get("fontAssetId").is_some_and(|v|!v.is_null()) {return Err("The selected alphabet has no saved font.".into())}
    }
    let width = width.unwrap_or(640).clamp(240, 4096);
    if !cfg!(target_os = "linux") { return Err("This native editor currently requires Linux.".into()); }
    let registry=state.0.clone();
    let cache = app.path().app_cache_dir().map_err(|_| "Could not locate the local editor cache.")?;
    tauri::async_runtime::spawn_blocking(move || {
        registry.update_latest(&key,&owner,|state|{
        let incoming_handoff=if resume{crate::windows::incoming_session(&window,&key)?}else{false};
        if resume {
            if let Some(existing)=state.as_mut(){
                // Exact same-document/config mounts preserve undo/caret. Shared
                // refresh or an alphabet change reinitializes only after the
                // caller's acknowledged drain, without exposing stale handles.
                let frame=existing.process()?.request(json!({"type":"snapshot","followCursor":false,"preview":false}))?;
                let exact_match=existing.binding==binding&&frame.get("document")==Some(&scene.document);
                validate_resume_binding(incoming_handoff,exact_match)?;
                if exact_match{return Ok(SessionUpdate::Keep(frame))}
            }
        }
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_| "Could not create an editor session.")?.as_nanos();
        let directory = cache.join("editor-sessions").join(format!("{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&directory).map_err(|_| "Could not create a private editor session.")?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).map_err(|_| "Could not protect the editor session.")?; }
        let script = crate::files::runtime_executable(&app, "novel-editor")?;
        let mut process = EditorProcess::start(&script,&directory)?;
        let mut fonts=Vec::new();
        if let Some(font)=font_asset {
            let asset=&font["asset"];let checksum=asset["checksum"].as_str().ok_or("Missing font checksum.")?;
            if checksum.len()!=64||!checksum.chars().all(|c|c.is_ascii_hexdigit()){return Err("Invalid font checksum.".into())}
            let bytes:Vec<u8>=serde_json::from_value(asset["content"].clone()).map_err(|_|"Invalid font asset.")?;
            let fonts_dir=directory.join("fonts");std::fs::create_dir_all(&fonts_dir).map_err(|_|"Could not prepare project fonts.")?;
            let path=fonts_dir.join(format!("{}.{}",checksum,if asset["mimeType"]=="font/otf"{"otf"}else{"ttf"}));
            std::fs::write(&path,bytes).map_err(|_|"Could not stage the project font.")?;
            fonts.push(json!({"path":path,"checksum":checksum,"family":font["family"]}));
        }
        let frame = process.request(json!({"type":"open","scene":scene,"width":width,"fonts":fonts,"writingSystem":writing_system}))?;
        Ok(SessionUpdate::Replace(EditorSession{process:Some(process),binding,directory,script},frame))
        })
    }).await.map_err(|_| "native editor task stopped.")?
}
#[tauri::command]
pub async fn editor_edit(window:tauri::Window,state: tauri::State<'_, EditorState>, operation: Value,session_id:Option<String>) -> Result<Value, String> {
    if !matches!(operation["type"].as_str(), Some("text" | "key" | "mouse" | "command" | "refresh" | "snapshot")) { return Err("Unsupported editor operation.".into()); }
    if let Some(id)=session_id.as_deref(){crate::windows::authorize_session(&window,id)?;}
    let owner=window.label().to_owned();let key=session_key(session_id.as_deref(),&owner)?;
    let handle = state.0.get(&key)?.ok_or("native editor is not open.")?;
    tauri::async_runtime::spawn_blocking(move || {
        handle.with_owned(&owner,|state|state.as_mut().ok_or("native editor is not open.")?.process()?.request(operation))
    }).await.map_err(|_| "native editor task stopped.")?
}
#[tauri::command]
pub async fn editor_release(window:tauri::Window,state:tauri::State<'_,EditorState>,session_id:Option<String>)->Result<(),String>{
    if let Some(id)=session_id.as_deref(){crate::windows::authorize_session(&window,id)?;}
    let owner=window.label().to_owned();let key=session_key(session_id.as_deref(),&owner)?;let registry=state.0.clone();
    tauri::async_runtime::spawn_blocking(move||registry.release_group(&key,&owner)).await.map_err(|_|"Editor session release stopped.")?
}

/// Called only after a scene's acknowledged save barrier. Retains its Undo and
/// caret in a private data checkpoint, releasing the expensive offscreen helper.
#[tauri::command]
pub async fn editor_park(window:tauri::Window,state:tauri::State<'_,EditorState>,session_id:String)->Result<(),String>{
 crate::windows::authorize_session(&window,&session_id)?;
 if !session_id.contains(":book:"){return Err("Only whole-book scene editors can be parked.".into())}
 let owner=window.label().to_owned();let key=session_key(Some(&session_id),&owner)?;
 let Some(handle)=state.0.get(&key)? else{return Ok(())};
 tauri::async_runtime::spawn_blocking(move||handle.with_owned(&owner,|session|{
  if let Some(session)=session {if let Some(process)=session.process.as_mut(){process.request(json!({"type":"command","command":"park","preview":false}))?;drop(session.process.take());}}
  Ok(())
 })).await.map_err(|_|"Editor checkpoint stopped.")?
}

// The registry source is also imported by standalone integration fixtures.
// Register shared regression scenarios only in the native library test target.
#[cfg(test)]
mod session_tests {
    #[test]
    fn overlapping_open_retries_retired_candidate_once_but_old_edits_stay_retired() { super::editor_sessions::latest_tests::overlapping_open_retries_retired_candidate_once_but_old_edits_stay_retired(); }
    #[test]
    fn retry_preserves_current_owner_and_never_consumes_foreign_open_action() { super::editor_sessions::latest_tests::retry_preserves_current_owner_and_never_consumes_foreign_open_action(); }
    #[test]
    fn handoff_and_release_include_only_exact_book_children() { super::editor_sessions::group_tests::handoff_and_release_include_only_exact_book_children(); }
    #[test]
    fn failed_group_transfer_rolls_back_previous_members() { super::editor_sessions::group_tests::failed_group_transfer_rolls_back_previous_members(); }
}
