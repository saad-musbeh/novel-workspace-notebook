//! Deliberate file delivery: the renderer never supplies an input or output path.
use serde_json::{json, Value};
use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::{atomic::{AtomicU64, Ordering}, Mutex}};
use tauri::Manager;
use crate::companion::CompanionState;
const FILE_LIMIT: usize = 64 * 1024 * 1024;
static NEXT_FILE: AtomicU64 = AtomicU64::new(1);
static CHOOSER: Mutex<()> = Mutex::new(());

pub fn runtime_executable(app: &tauri::AppHandle, filename: &str) -> Result<PathBuf, String> {
    if filename != "novel-editor" { return Err("Unsupported desktop helper.".into()); }
    let sibling=std::env::current_exe().map_err(|_|"Could not locate desktop runtime.")?.parent().ok_or("Missing runtime directory.")?.join(filename);
    if sibling.is_file(){return Ok(sibling);}
    let bundled=app.path().resource_dir().map_err(|_|"Could not locate desktop resources.")?.join("runtime").join(filename);
    if bundled.is_file(){return Ok(bundled);}
    #[cfg(debug_assertions)] {
        let source=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug").join(filename);
        if source.is_file(){return Ok(source);}
    }
    Err("The Rust editor helper is missing. Rebuild or reinstall this application.".into())
}
fn extension(format: &str) -> Result<&'static str, String> {
    match format { "docx" => Ok(".docx"), "text" => Ok(".txt"), "markdown" => Ok(".md"), "html" => Ok(".html"), "project" => Ok(".novel.json"), "alphabet" => Ok(".alphabet.json"), "dictionary" => Ok(".json"), _ => Err("Unsupported document export or import format.".into()) }
}
fn safe_name(name: Option<&str>, format: &str) -> Result<String, String> {
    let suffix = extension(format)?;
    let name = name.unwrap_or("document");
    if name.is_empty() || name.len() > 200 || matches!(name, "." | "..") || name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\')) { return Err("Choose a filename without folders or control characters.".into()); }
    Ok(if name.ends_with(suffix) { name.to_string() } else { format!("{name}{suffix}") })
}
fn choose(helper: &Path, mode: &str, format: &str, name: &str) -> Result<Option<PathBuf>, String> {
    let _guard = CHOOSER.lock().map_err(|_| "The file chooser is unavailable.")?;
    #[cfg(debug_assertions)] if let Some(directory) = std::env::var_os("NOVEL_FILE_PROBE_DIRECTORY") {
        let directory=PathBuf::from(directory);
        if !directory.is_absolute() || !directory.is_dir() { return Err("Invalid synthetic file-probe directory.".into()); }
        // Fixed synthetic paths only. No renderer-controlled path escapes this directory.
        return Ok(Some(directory.join(format!("synthetic-export{}",extension(format)?))));
    }
    let mut child = Command::new(helper).arg("--choose").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_| "Could not start the local desktop file chooser.")?;
    let request=serde_json::to_vec(&json!({"mode":mode,"format":format,"suggestedName":name})).map_err(|_| "Could not open file selection.")?;
    let mut input=child.stdin.take().ok_or("Could not reach the file chooser.")?;
    input.write_all(&request).and_then(|_|input.write_all(b"\n")).map_err(|_| "Could not reach the file chooser.")?;drop(input);
    let mut bytes=Vec::new();child.stdout.take().ok_or("Could not read file selection.")?.take(8193).read_to_end(&mut bytes).map_err(|_| "Could not read file selection.")?;
    let exit=child.wait().map_err(|_| "File selection stopped.")?;
    if bytes.len()>8192{return Err("The file chooser returned an unsupported response.".into());}
    let reply:Value=serde_json::from_slice(&bytes).map_err(|_| "The local file chooser did not return a selection.")?;
    if !exit.success() || reply["ok"] != true { return Err(reply["error"].as_str().unwrap_or("The local file chooser is unavailable.").into()); }
    if reply["cancelled"] == true { return Ok(None); }
    let path=PathBuf::from(reply["path"].as_str().ok_or("The selected local filename is missing.")?);
    if !path.is_absolute() || path.file_name().is_none() { return Err("The chooser did not select a local file.".into()); }
    Ok(Some(path))
}
fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    if content.len() > FILE_LIMIT { return Err("The export exceeds the 64 MiB file limit.".into()); }
    let parent=path.parent().ok_or("Choose a local destination folder.")?;
    let filename=path.file_name().ok_or("Choose a local filename.")?;
    let stamp=NEXT_FILE.fetch_add(1,Ordering::Relaxed);
    let temporary=parent.join(format!(".{}.novel-{}-{stamp}.tmp",filename.to_string_lossy(),std::process::id()));
    let mut created=false;
    let result=(|| {
        let mut options=OpenOptions::new();options.write(true).create_new(true);
        #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let mut file=options.open(&temporary).map_err(|_| "Could not create the export in the selected folder. Check available space and folder permissions.")?;
        created=true;
        file.write_all(content).and_then(|_|file.sync_all()).map_err(|_| "The export could not be written completely. The previous destination has not been replaced.")?;drop(file);
        fs::rename(&temporary,path).map_err(|_| "Could not finish the export. The previous destination has not been replaced.")?;
        File::open(parent).and_then(|directory|directory.sync_all()).map_err(|_| "The file was written, but durable folder confirmation failed. Keep the project open and verify the exported file.")?;
        Ok(())
    })();
    if created && temporary.exists() { let _=fs::remove_file(&temporary); }
    result
}
fn read_local(path:&Path) -> Result<String,String> {
    let before=fs::metadata(path).map_err(|_| "Could not inspect the selected local file.")?;
    if !before.is_file() || before.len()>FILE_LIMIT as u64 { return Err("Select an ordinary local file no larger than 64 MiB.".into()); }
    let file=File::open(path).map_err(|_| "Could not open the selected local file.")?;
    let metadata=file.metadata().map_err(|_| "Could not inspect the selected local file.")?;
    if !metadata.is_file() || metadata.len()>FILE_LIMIT as u64 { return Err("Select an ordinary local file no larger than 64 MiB.".into()); }
    let mut bytes=Vec::new();file.take(FILE_LIMIT as u64+1).read_to_end(&mut bytes).map_err(|_| "Could not read the selected file completely.")?;
    if bytes.len()>FILE_LIMIT{return Err("The selected file exceeds 64 MiB.".into());}
    String::from_utf8(bytes).map_err(|_| "This import requires UTF-8 text. The original project has not been changed.".into())
}
#[tauri::command]
pub async fn export_document(app:tauri::AppHandle,format:String,content:String,suggested_name:Option<String>)->Result<Value,String> {
    if format=="docx" { return Err("Use the saved-manuscript Word export action.".into()); }
    let name=safe_name(suggested_name.as_deref(),&format)?;
    if content.len()>FILE_LIMIT{return Err("The export exceeds the 64 MiB file limit.".into());}
    let helper=runtime_executable(&app,"novel-editor")?;
    tauri::async_runtime::spawn_blocking(move|| {let Some(path)=choose(&helper,"save",&format,&name)?else{return Ok(json!({"cancelled":true}));};atomic_write(&path,content.as_bytes())?;Ok(json!({"cancelled":false,"name":path.file_name().map(|s|s.to_string_lossy())}))}).await.map_err(|_| "Local export was interrupted.")?
}
#[tauri::command]
pub async fn export_docx(app:tauri::AppHandle,window:tauri::Window,storage:tauri::State<'_,CompanionState>,project_id:String,suggested_name:Option<String>,profile_revision_id:Option<String>)->Result<Value,String> {
    let name=safe_name(suggested_name.as_deref(),"docx")?;
    let helper=runtime_executable(&app,"novel-editor")?;
    crate::project_exports::drain_project(&window,&project_id).await?;
    // The export snapshot reads hierarchy, numbering, conflict leaves and
    // immutable scene revisions together, after open project drafts are saved.
    let snapshot=storage.0.request("word_export_snapshot",json!({"projectId":project_id,"profileRevisionId":profile_revision_id})).await?;
    let snapshot:novel_storage::WordExportSnapshot=serde_json::from_value(snapshot).map_err(|_| "Could not read the saved manuscript for Word export.")?;
    if snapshot.bundle.project.id!=project_id { return Err("The saved manuscript does not match the selected project.".into()); }
    tauri::async_runtime::spawn_blocking(move|| {
        let bytes=novel_docx::export_manuscript_with_profile(&snapshot.bundle,&snapshot.profile)?;
        let Some(path)=choose(&helper,"save","docx",&name)?else{return Ok(json!({"cancelled":true}));};
        atomic_write(&path,&bytes)?;
        Ok(json!({"cancelled":false,"name":path.file_name().map(|s|s.to_string_lossy())}))
    }).await.map_err(|_| "Word export was interrupted. The saved manuscript is intact.")?
}
#[tauri::command]
pub async fn import_document(app:tauri::AppHandle,format:String)->Result<Value,String> {
    if format=="docx" { return Err("Word manuscript import is not available yet. Import a complete project archive instead.".into()); }
    let name=safe_name(None,&format)?;let helper=runtime_executable(&app,"novel-editor")?;
    tauri::async_runtime::spawn_blocking(move||{let Some(path)=choose(&helper,"open",&format,&name)?else{return Ok(json!({"cancelled":true}));};let content=read_local(&path)?;Ok(json!({"cancelled":false,"name":path.file_name().map(|s|s.to_string_lossy()),"content":content}))}).await.map_err(|_| "Local import was interrupted.")?
}
#[cfg(test)] mod tests {
    use super::*;
    fn directory()->PathBuf {let path=std::env::temp_dir().join(format!("novel-file-fixture-{}-{}",std::process::id(),NEXT_FILE.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&path).unwrap();path}
    #[test] fn exact_multilingual_file_delivery_and_atomic_replacement(){let dir=directory();let path=dir.join("synthetic.novel.json");let original="Arabic مَرْحَبًا Chinese 繁體 👩🏽‍🚀 PUA \u{e000} e\u{301}";atomic_write(&path,original.as_bytes()).unwrap();assert_eq!(read_local(&path).unwrap(),original);atomic_write(&path,b"replacement").unwrap();assert_eq!(read_local(&path).unwrap(),"replacement");assert_eq!(fs::read_dir(&dir).unwrap().count(),1);fs::remove_dir_all(dir).unwrap();}
    #[test] fn failed_destination_and_non_utf8_do_not_overwrite_source(){let dir=directory();let source=dir.join("source");atomic_write(&source,b"original").unwrap();assert!(atomic_write(&dir,b"bad").is_err());assert_eq!(read_local(&source).unwrap(),"original");fs::write(dir.join("bytes"),[0xff]).unwrap();assert!(read_local(&dir.join("bytes")).unwrap_err().contains("UTF-8"));fs::remove_dir_all(dir).unwrap();}
    #[test] fn rejects_frontend_paths_and_unknown_formats(){for name in ["../private","/tmp/path","folder\\file","line\nname",""]{assert!(safe_name(Some(name),"text").is_err());}assert!(extension("shell").is_err());assert_eq!(safe_name(Some("نور"),"project").unwrap(),"نور.novel.json");}
    #[test] fn oversized_import_is_rejected_before_loading(){let dir=directory();let path=dir.join("large");File::create(&path).unwrap().set_len(FILE_LIMIT as u64+1).unwrap();assert!(read_local(&path).unwrap_err().contains("64 MiB"));fs::remove_dir_all(dir).unwrap();}
}
