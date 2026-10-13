//! A recovery name appears only after a complete, private, verified snapshot.
use sqlx::{sqlite::{SqliteConnectOptions,SqlitePoolOptions},SqlitePool};
use std::{fs,path::{Path,PathBuf}};
struct Staging(PathBuf);
impl Drop for Staging{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}

pub(crate) async fn consistent_snapshot(pool:&SqlitePool,destination:&Path)->Result<(),String>{
    if fs::symlink_metadata(destination).is_ok(){return Err("Backup destination already exists; choose a new file.".into())}
    let parent=destination.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or_else(||Path::new("."));
    let staging=parent.join(format!(".novel-snapshot-{}",uuid::Uuid::new_v4()));
    let mut builder=fs::DirBuilder::new();
    #[cfg(unix)]{use std::os::unix::fs::DirBuilderExt;builder.mode(0o700);}
    builder.create(&staging).map_err(|_|"Could not create private backup staging storage.")?;
    let guard=Staging(staging);
    let temporary=guard.0.join("snapshot.sqlite");
    let temporary_name=temporary.to_str().ok_or("The backup destination needs a valid UTF-8 path.")?;
    sqlx::query("VACUUM INTO ?").bind(temporary_name).execute(pool).await.map_err(|_|"Could not complete the consistent backup. Saved writing was preserved.")?;
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;fs::set_permissions(&temporary,fs::Permissions::from_mode(0o600)).map_err(|_|"Could not protect the completed backup.")?;}
    let verify=SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(&temporary).read_only(true)).await.map_err(|_|"Could not verify the completed backup.")?;
    let result=async{
        let integrity:String=sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&verify).await.map_err(|_|"Could not verify backup integrity.")?;
        let foreign_keys=sqlx::query("PRAGMA foreign_key_check").fetch_all(&verify).await.map_err(|_|"Could not verify backup references.")?;
        if integrity!="ok"||!foreign_keys.is_empty(){return Err("Backup integrity or references failed; no completed backup was published.")}
        Ok(())
    }.await;
    verify.close().await;result?;
    fs::File::open(&temporary).and_then(|f|f.sync_all()).map_err(|_|"Could not durably flush the completed backup.")?;
    #[cfg(test)] checkpoint("snapshot");
    // A hard link creates the final name atomically and cannot replace another file.
    fs::hard_link(&temporary,destination).map_err(|_|"Could not publish the backup without replacing existing content.")?;
    if fs::File::open(parent).and_then(|f|f.sync_all()).is_err(){
        let _=fs::remove_file(destination);
        return Err("Could not durably publish the backup. Saved writing was preserved.".into())
    }
    Ok(())
}

// Test binaries alone can stop at an exact unacknowledged durability boundary.
// No environment-controlled hook exists in the production storage/companion.
#[cfg(test)]
pub(crate) fn checkpoint(kind:&str){
    if std::env::var("NOVEL_STORAGE_FAULT_STAGE").ok().as_deref()==Some(kind){
        let ready=std::env::var_os("NOVEL_STORAGE_FAULT_READY").expect("synthetic checkpoint path");
        std::fs::write(ready,kind).expect("synthetic checkpoint ready");
        loop{std::thread::park();}
    }
}
