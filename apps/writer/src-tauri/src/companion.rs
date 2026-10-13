use serde_json::{json,Value};
use std::{collections::HashMap,io::{BufRead,BufReader,Read,Write},path::Path,process::{Child,ChildStdin,Command,Stdio},sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,Ordering}}};
type Reply=Result<Value,String>;
// Tokio oneshot types are used through a direct cached runtime dependency.
pub struct Companion {
    process:Mutex<Child>,input:Arc<Mutex<ChildStdin>>,pending:Arc<Mutex<HashMap<u64,tokio::sync::oneshot::Sender<Reply>>>>,next:AtomicU64,alive:Arc<AtomicBool>,
}
#[derive(Clone)]
pub struct CompanionState(pub Arc<Companion>);
impl Drop for Companion {fn drop(&mut self){if let Ok(mut child)=self.process.lock(){let _=child.kill();let _=child.wait();}}}
impl Companion {
    pub fn open(directory:&Path,resource_dir:&Path)->Result<CompanionState,String>{
        let sibling=std::env::current_exe().map_err(|_|"Could not locate desktop runtime.")?.parent().ok_or("Missing runtime directory.")?.join("novel-companion");
        let bundled=resource_dir.join("runtime").join("novel-companion");
        let executable=if sibling.is_file(){sibling}else if bundled.is_file(){bundled}else{
            #[cfg(debug_assertions)] {let local=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/novel-companion");if local.is_file(){local}else{return Err("The local storage companion is missing. Rebuild the desktop app.".into());}}
            #[cfg(not(debug_assertions))] {return Err("The bundled local storage companion is missing. Reinstall the desktop package.".into());}
        };
        let mut child=Command::new(executable).arg(directory).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"The local storage companion is missing. Rebuild the desktop app.")?;
        let input=Arc::new(Mutex::new(child.stdin.take().ok_or("Could not reach local storage.")?));
        let mut output=BufReader::new(child.stdout.take().ok_or("Could not read local storage.")?);
        let pending:Arc<Mutex<HashMap<u64,tokio::sync::oneshot::Sender<Reply>>>>=Arc::new(Mutex::new(HashMap::new()));
        let replies=pending.clone();let alive=Arc::new(AtomicBool::new(true));let reader_alive=alive.clone();
        std::thread::spawn(move||{
            loop {
                let mut bytes=Vec::new();
                if (&mut output).take(128_000_001).read_until(b'\n',&mut bytes).is_err()||bytes.len()>128_000_000||bytes.last()!=Some(&b'\n'){break}
                let Ok(response)=serde_json::from_slice::<Value>(&bytes)else{break};
                let Some(id)=response["id"].as_u64()else{break};
                let result=if response["ok"]==true{Ok(response["value"].clone())}else{Err(response["error"].as_str().unwrap_or("Local storage failed.").to_string())};
                if let Ok(mut map)=replies.lock(){if let Some(sender)=map.remove(&id){let _=sender.send(result);}}
            }
            if let Ok(mut map)=replies.lock(){reader_alive.store(false,Ordering::SeqCst);for (_,sender) in map.drain(){let _=sender.send(Err("Local storage stopped. Your acknowledged revisions remain on disk. Reopen the app.".into()));}}
        });
        Ok(CompanionState(Arc::new(Self{process:Mutex::new(child),input,pending,next:AtomicU64::new(1),alive})))
    }
    pub async fn request(&self,op:&str,args:Value)->Reply{
        let id=self.next.fetch_add(1,Ordering::Relaxed);let (sender,receiver)=tokio::sync::oneshot::channel();
        let mut data=serde_json::to_vec(&json!({"id":id,"op":op,"args":args})).map_err(|_|"Could not encode local request.")?;
        if data.len()>128_000_000{return Err("Local request exceeds the workspace limit.".into())}data.push(b'\n');
        {let mut map=self.pending.lock().map_err(|_|"Local request queue is unavailable.")?;if !self.alive.load(Ordering::SeqCst){return Err("The local companion stopped. Reopen the app; stored content is intact.".into())}if map.len()>=64{return Err("Local storage is busy. Retry in a moment.".into())}map.insert(id,sender);}
        let input=self.input.clone();
        let written=tauri::async_runtime::spawn_blocking(move||{ let mut writer=input.lock().map_err(|_|"Local storage is unavailable.")?; writer.write_all(&data).and_then(|_|writer.flush()).map_err(|_|"Local storage stopped.") }).await;
        if !matches!(written,Ok(Ok(()))){if let Ok(mut map)=self.pending.lock(){map.remove(&id);}return Err("Could not send the local request. Your last commit remains intact.".into())}
        receiver.await.map_err(|_|"The local request was interrupted.")?
    }
}
#[tauri::command]
pub async fn workspace_command(state:tauri::State<'_,CompanionState>,operation:String,args:Value)->Reply{
    state.0.request(&operation,args).await
}
