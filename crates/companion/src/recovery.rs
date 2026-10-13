//! Independent consistent SQLite snapshots. No automatic pruning or overwrites.
use novel_storage::Storage;
use serde_json::{json,Value};
use std::{path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};
pub struct Recovery{directory:PathBuf,startup_error:std::sync::Mutex<Option<String>>}
impl Recovery{
 pub fn new(root:&Path)->Result<Self,String>{let directory=root.join("recovery");std::fs::create_dir_all(&directory).map_err(|_|"Could not create local recovery storage.")?;#[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&directory,std::fs::Permissions::from_mode(0o700)).map_err(|_|"Could not protect recovery files.")?;}Ok(Self{directory,startup_error:std::sync::Mutex::new(None)})}
 pub async fn initialize(&self,storage:&Storage){if let Err(error)=self.daily(storage).await{if let Ok(mut value)=self.startup_error.lock(){*value=Some(error);}}}
 pub fn status(&self)->Value{json!({"dailyBackupError":self.startup_error.lock().ok().and_then(|v|v.clone()),"automaticPruning":false})}
 pub async fn snapshot(&self,storage:&Storage,kind:&str)->Result<Value,String>{let stamp=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_|"Unavailable recovery clock.")?;let name=format!("{kind}-{}.sqlite3",stamp.as_nanos());let path=self.directory.join(&name);storage.backup(&path).await?;#[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o600)).map_err(|_|"Could not protect the recovery snapshot.")?;}Ok(json!({"name":name,"bytes":std::fs::metadata(path).map_err(|_|"Could not inspect recovery snapshot.")?.len()}))}
 pub fn list(&self)->Result<Value,String>{let mut files=Vec::new();for entry in std::fs::read_dir(&self.directory).map_err(|_|"Could not list recovery snapshots.")?{let entry=entry.map_err(|_|"Could not inspect recovery snapshot.")?;let name=entry.file_name().to_string_lossy().into_owned();let meta=entry.metadata().map_err(|_|"Could not inspect recovery snapshot.")?;if meta.is_file()&&name.ends_with(".sqlite3"){files.push(json!({"name":name,"bytes":meta.len(),"createdAt":meta.modified().ok().and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|t|t.as_secs())}));}}files.sort_by(|a,b|b["name"].as_str().cmp(&a["name"].as_str()));Ok(json!(files))}
 pub async fn daily(&self,storage:&Storage)->Result<(),String>{let today=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_|"Unavailable recovery clock.")?.as_secs()/86400;let files=self.list()?;let exists=files.as_array().is_some_and(|files|files.iter().any(|f|f["createdAt"].as_u64().is_some_and(|t|t/86400==today)));if !exists{self.snapshot(storage,"daily").await?;}Ok(())}
}
#[cfg(test)]mod tests{
 use super::*;
 #[tokio::test]async fn daily_snapshot_is_consistent_private_and_not_pruned(){let dir=tempfile::tempdir().unwrap();let db=Storage::open(&dir.path().join("source.sqlite3")).await.unwrap();db.create_project("Synthetic").await.unwrap();let r=Recovery::new(dir.path()).unwrap();r.daily(&db).await.unwrap();r.daily(&db).await.unwrap();let files=r.list().unwrap();assert_eq!(files.as_array().unwrap().len(),1);let copy=Storage::open(&r.directory.join(files[0]["name"].as_str().unwrap())).await.unwrap();assert_eq!(copy.list_projects().await.unwrap().len(),1);copy.close().await;r.snapshot(&db,"manual").await.unwrap();assert_eq!(r.list().unwrap().as_array().unwrap().len(),2);db.close().await;}
}
