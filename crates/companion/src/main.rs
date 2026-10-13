//! Local-only database owner. The desktop uses private inherited stdio, not TCP.
mod providers;
mod patch;
mod sync_service;
mod recovery;
mod peer_client;
use novel_storage::{MetadataResolutionSnapshot, SceneArrangementSnapshot, Scene, Storage, ProjectBundle, Revision, SyncCursor, AssetTransferSpec, ProjectAsset, SourceDependency, plain_text};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{fs::OpenOptions, io::{BufRead, Read, Write}, path::PathBuf, sync::Arc};

fn argument<T: DeserializeOwned>(args: &Value, name: &str) -> Result<T, String> {
    serde_json::from_value(args.get(name).cloned().ok_or_else(|| format!("Missing {name}."))?)
        .map_err(|_| format!("Invalid {name}."))
}
fn encode<T: serde::Serialize>(value: T) -> Result<Value, String> { serde_json::to_value(value).map_err(|_| "Could not encode response.".into()) }

async fn execute(storage: &Storage, recovery: &Result<Arc<recovery::Recovery>,String>, sync: &Result<Arc<sync_service::SyncService>,String>, listener: &Arc<tokio::sync::Mutex<Option<sync_service::ListenerHandle>>>, peer: &Result<Arc<peer_client::PeerClient>,String>, op: &str, args: Value) -> Result<Value, String> {
    match op {
        // Validate the frozen saved source before sending it to a local model; translation never writes manuscript data.
        "provider_translate" => {
            let project_id=argument::<String>(&args,"projectId")?;
            let scene_id=argument::<String>(&args,"sceneId")?;
            let revision_id=argument::<String>(&args,"sceneRevisionId")?;
            let text=argument::<String>(&args,"text")?;
            let index=storage.open_project_index(&project_id).await?;
            if !index.scenes.iter().any(|scene|scene.id==scene_id){return Err("Translation source is not in this project.".into())}
            let snapshot=storage.load_revision(&scene_id,&revision_id).await?;
            if plain_text(&snapshot.scene.document)!=text{return Err("Translation source does not match its saved scene revision.".into())}
            providers::translate(args).await
        },
        "peer_identity"|"peer_new_identity"|"peer_list"|"peer_probe"|"peer_pair"|"peer_sync"|"peer_forget" => peer.as_ref().map_err(Clone::clone)?.dispatch(op,args).await,
        "runtime_info" => Ok(json!({"ipcVersion":1,"storageSchema":3,"features":["checked-patches","private-sync","recovery"]})),
        "backup_now" => recovery.as_ref().map_err(Clone::clone)?.snapshot(storage,"manual").await,
        "backup_status" => Ok(match recovery {Ok(recovery)=>recovery.status(),Err(error)=>json!({"dailyBackupError":error,"automaticPruning":false})}),
        "list_backups" => recovery.as_ref().map_err(Clone::clone)?.list(),
        "deleted_scenes" => encode(storage.deleted_scenes(&argument::<String>(&args,"projectId")?).await?),
        "load" => encode(storage.load().await?),
        "save" => encode(storage.save(argument::<Scene>(&args,"scene")?, argument(&args,"parent")?).await?),
        "list_projects" => encode(storage.list_projects().await?),
        "create_universe" => encode(storage.create_universe(&argument::<String>(&args,"title")?).await?),
        "create_universe_novel" => encode(storage.create_universe_novel(&argument::<String>(&args,"universeId")?,argument::<Option<String>>(&args,"seriesId")?,&argument::<String>(&args,"title")?,&argument::<String>(&args,"parent")?).await?),
        "create_novel" => encode(storage.create_novel(&argument::<String>(&args,"title")?).await?),
        "create_project" => encode(storage.create_project(&argument::<String>(&args,"title")?).await?),
        "open_project" => encode(storage.open_project(&argument::<String>(&args,"projectId")?).await?),
        "manuscript_export_snapshot" => encode(storage.manuscript_export_snapshot(&argument::<String>(&args,"projectId")?).await?),
        "word_export_snapshot" => encode(storage.word_export_snapshot(&argument::<String>(&args,"projectId")?,argument::<Option<String>>(&args,"profileRevisionId")?).await?),
        "create_project_branch" => encode(storage.create_project_branch(&argument::<String>(&args,"sourceProjectId")?, &argument::<String>(&args,"title")?).await?),
        "list_project_branches" => encode(storage.list_project_branches(&argument::<String>(&args,"sourceProjectId")?).await?),
        "compare_project_branch" => encode(storage.compare_project_branch(&argument::<String>(&args,"sourceProjectId")?, &argument::<String>(&args,"branchProjectId")?, argument::<u32>(&args,"offset")?, argument::<u32>(&args,"limit")?).await?),
        "bring_back_branch_piece" => encode(storage.bring_back_branch_piece(&argument::<String>(&args,"sourceProjectId")?, &argument::<String>(&args,"branchProjectId")?, &argument::<String>(&args,"lineageRevisionId")?, &argument::<String>(&args,"entityId")?, &argument::<String>(&args,"entityType")?, &argument::<String>(&args,"revisionId")?, argument::<Vec<String>>(&args,"expectedSourceHeads")?, argument::<Vec<String>>(&args,"expectedBranchHeads")?).await?),
        "project_time_points" => encode(storage.project_time_points(&argument::<String>(&args,"projectId")?, argument::<Option<i64>>(&args,"beforeSequence")?, argument::<u32>(&args,"limit")?).await?),
        "project_time_preview" => encode(storage.project_time_preview(&argument::<String>(&args,"projectId")?, &argument::<String>(&args,"epoch")?, argument::<i64>(&args,"sequence")?, &argument::<String>(&args,"pointRevisionId")?).await?),
        "recover_time_piece" => encode(storage.recover_time_piece(&argument::<String>(&args,"projectId")?, &argument::<String>(&args,"epoch")?, argument::<i64>(&args,"sequence")?, &argument::<String>(&args,"pointRevisionId")?, &argument::<String>(&args,"entityId")?, &argument::<String>(&args,"entityType")?, &argument::<String>(&args,"revisionId")?, argument::<Vec<String>>(&args,"expectedHeads")?).await?),
        "open_project_index" => encode(storage.open_project_index(&argument::<String>(&args,"projectId")?).await?),
        "load_project_scene" => encode(storage.load_project_scene(&argument::<String>(&args,"projectId")?, &argument::<String>(&args,"sceneId")?).await?),
        "rename_project" => encode(storage.rename_project(&argument::<String>(&args,"projectId")?, &argument::<String>(&args,"title")?).await?),
        "create_chapter" => encode(storage.create_chapter(&argument::<String>(&args,"projectId")?, &argument::<String>(&args,"title")?).await?),
        "scene_arrangement_snapshot" => encode(storage.scene_arrangement_snapshot(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"sceneId")?).await?),
        "arrange_scene" => encode(storage.arrange_scene(&argument::<SceneArrangementSnapshot>(&args,"snapshot")?,&argument::<String>(&args,"targetChapterId")?,argument::<usize>(&args,"position")?).await?),
        "chapter_edit_snapshot" => encode(storage.chapter_edit_snapshot(&argument::<String>(&args,"projectId")?, &argument::<String>(&args,"chapterId")?).await?),
        "edit_chapter" => encode(storage.edit_chapter(&argument::<String>(&args,"chapterId")?,argument::<Option<String>>(&args,"expectedChapterHead")?,argument::<Option<String>>(&args,"expectedProjectHead")?,argument::<Vec<String>>(&args,"expectedOrder")?,argument::<Option<String>>(&args,"numberingParent")?,&argument::<String>(&args,"title")?,argument::<i64>(&args,"number")?).await?),
        "rename_chapter" => encode(storage.rename_chapter(&argument::<String>(&args,"chapterId")?, &argument::<String>(&args,"title")?).await?),
        "reorder_chapters" => encode(storage.reorder_chapters(&argument::<String>(&args,"projectId")?, argument::<Vec<String>>(&args,"ids")?).await?),
        "create_scene" => encode(storage.create_scene(&argument::<String>(&args,"chapterId")?, argument::<Scene>(&args,"scene")?).await?),
        "load_scene" => encode(storage.load_scene(&argument::<String>(&args,"sceneId")?).await?),
        "save_scene" => {
            let sid=argument::<String>(&args,"sceneId")?;let scene=argument::<Scene>(&args,"scene")?;let parent=argument(&args,"parent")?;
            encode(if let Some(request_id)=args["requestId"].as_str(){storage.save_scene_with_id(&sid,scene,parent,request_id).await?}else{storage.save_scene(&sid,scene,parent).await?})
        },
        "rename_scene" => encode(storage.rename_scene(&argument::<String>(&args,"sceneId")?, &argument::<String>(&args,"title")?, argument(&args,"parent")?).await?),
        "reorder_scenes" => encode(storage.reorder_scenes(&argument::<String>(&args,"chapterId")?, argument::<Vec<String>>(&args,"ids")?).await?),
        "history" => encode(storage.history(&argument::<String>(&args,"sceneId")?).await?),
        "restore_scene" => encode(storage.restore_scene(&argument::<String>(&args,"sceneId")?, &argument::<String>(&args,"revisionId")?, &argument::<String>(&args,"parent")?).await?),
        "search" => encode(storage.search(&argument::<String>(&args,"projectId")?, &argument::<String>(&args,"query")?).await?),
        "statistics" => encode(storage.statistics(&argument::<String>(&args,"projectId")?).await?),
        "export_archive" => encode(storage.export_archive(&argument::<String>(&args,"projectId")?).await?),
        "import_archive" => encode(storage.import_archive(&argument::<String>(&args,"archive")?).await?),
        "load_project_data" => encode(storage.load_project_data(&argument::<String>(&args,"projectId")?, &argument::<String>(&args,"kind")?).await?),
        "load_entity_revision" => encode(storage.load_entity_revision(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"entityId")?,&argument::<String>(&args,"entityType")?,&argument::<String>(&args,"revisionId")?).await?),
        "project_data_history" => encode(storage.project_data_history(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"kind")?).await?),
        "save_project_data_checked" => encode(storage.save_project_data_checked(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"kind")?,argument::<Value>(&args,"payload")?,argument(&args,"parent")?,&argument::<Vec<SourceDependency>>(&args,"dependencies")?).await?),
        "load_project_data_revision" => encode(storage.load_project_data_revision(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"kind")?,&argument::<String>(&args,"revisionId")?).await?),
        "save_project_data" => encode(storage.save_project_data(&argument::<String>(&args,"projectId")?, &argument::<String>(&args,"kind")?, argument::<Value>(&args,"payload")?, argument(&args,"parent")?).await?),
        "move_scene" => encode(storage.move_scene(&argument::<String>(&args,"sceneId")?, &argument::<String>(&args,"chapterId")?, argument::<usize>(&args,"position")?).await?),
        "load_revision" => encode(storage.load_revision(&argument::<String>(&args,"sceneId")?,&argument::<String>(&args,"revisionId")?).await?),
        "snapshot_scene" => encode(storage.snapshot_scene(&argument::<String>(&args,"sceneId")?,&argument::<String>(&args,"parent")?).await?),
        "conflicts" => encode(storage.conflicts(&argument::<String>(&args,"sceneId")?).await?),
        "resolve_scene" => encode(storage.resolve_scene(&argument::<String>(&args,"sceneId")?,argument::<Scene>(&args,"scene")?,&argument::<String>(&args,"parent")?,argument::<Vec<String>>(&args,"conflictIds")?).await?),
        "put_asset" => encode(storage.put_asset(&argument::<String>(&args,"projectId")?,argument::<ProjectAsset>(&args,"asset")?).await?),
        "list_assets" => encode(storage.list_assets(&argument::<String>(&args,"projectId")?).await?),
        "sync_cursor" => encode(storage.sync_cursor().await?),
        "rotate_epoch" => encode(sync.as_ref().map_err(Clone::clone)?.rotate_hub_epoch().await?),
        "pull_revisions" => encode(storage.pull_revisions(&argument::<String>(&args,"projectId")?,argument::<SyncCursor>(&args,"cursor")?,argument::<u32>(&args,"limit")?).await?),
        "ingest_revisions" => encode(storage.ingest_revisions(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"expectedEpoch")?,&argument::<String>(&args,"remoteEpoch")?,argument::<ProjectBundle>(&args,"manifest")?,argument::<Vec<Revision>>(&args,"revisions")?).await?),
        "resolve_scene_deletion" => encode(storage.resolve_scene_deletion(&argument::<String>(&args,"sceneId")?,&argument::<String>(&args,"parent")?,argument::<Vec<String>>(&args,"conflictIds")?).await?),
        "delete_scene" => encode(storage.delete_scene(&argument::<String>(&args,"sceneId")?,&argument::<String>(&args,"parent")?).await?),
        "restore_deleted_scene" => encode(storage.restore_deleted_scene(&argument::<String>(&args,"sceneId")?,argument::<Scene>(&args,"scene")?,argument::<Vec<String>>(&args,"heads")?).await?),
        "metadata_heads" => encode(storage.metadata_heads(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"entityId")?,&argument::<String>(&args,"entityType")?).await?),
        "metadata_resolution_snapshot" => encode(storage.metadata_resolution_snapshot(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"entityId")?,&argument::<String>(&args,"entityType")?).await?),
        "resolve_metadata_checked" => encode(storage.resolve_metadata_checked(&argument::<MetadataResolutionSnapshot>(&args,"snapshot")?,argument::<Value>(&args,"payload")?).await?),
        "resolve_metadata" => encode(storage.resolve_metadata(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"entityId")?,&argument::<String>(&args,"entityType")?,argument::<Value>(&args,"payload")?,argument::<Vec<String>>(&args,"heads")?).await?),
        "scene_heads" => encode(storage.scene_heads(&argument::<String>(&args,"sceneId")?).await?),
        "project_data_heads" => encode(storage.project_data_heads(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"kind")?).await?),
        "resolve_project_data" => {let pid=argument::<String>(&args,"projectId")?;if let Some(jobs)=storage.load_project_data(&pid,"assistanceJobs").await?{if jobs.payload["jobs"].as_array().is_some_and(|jobs|jobs.iter().any(|j|matches!(j["state"].as_str(),Some("pending_local"|"queued"|"running"|"cancelling")))){return Err("Cancel or finish this project’s active review before resolving project settings.".into())}}encode(storage.resolve_project_data(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"kind")?,argument::<Value>(&args,"payload")?,argument::<Vec<String>>(&args,"heads")?).await?)},
        "begin_asset_transfer" => encode(storage.begin_asset_transfer(&argument::<String>(&args,"projectId")?,argument::<AssetTransferSpec>(&args,"spec")?).await?),
        "append_asset_chunk" => encode(storage.append_asset_chunk(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"transferId")?,argument::<usize>(&args,"offset")?,argument::<Vec<u8>>(&args,"bytes")?).await?),
        "finalize_asset_transfer" => encode(storage.finalize_asset_transfer(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"transferId")?).await?),
        "asset_transfer_status" => encode(storage.asset_transfer_status(&argument::<String>(&args,"projectId")?,&argument::<String>(&args,"transferId")?).await?),
        "apply_scene_patch" => {
            let patch_value=argument::<Value>(&args,"patch")?;
            let sid=argument::<String>(&patch_value["source"],"sceneId")?;
            let expected=argument::<String>(&patch_value["source"],"revisionId")?;
            let snapshot=storage.load_revision(&sid,&expected).await?;
            let changed=patch::apply(&snapshot.scene,&patch_value)?;
            let mut dependencies=Vec::new();
            if let Some(sources)=patch_value.get("supportingSources").and_then(Value::as_array){
                if sources.len()>50{return Err("Too many supporting sources.".into())}
                for source in sources {
                    let entity=argument::<String>(source,"sceneId")?;let revision=argument::<String>(source,"revisionId")?;
                    let (entity_id,entity_type,text)=if let Some(pid)=entity.strip_prefix("planning:") {
                        let frozen=storage.load_project_data_revision(pid,"planning",&revision).await?;
                        (format!("{pid}:planning"),"projectData",providers::canonical_fact_text(&frozen.payload)?)
                    }else{let frozen=storage.load_revision(&entity,&revision).await?;(entity.clone(),"scene",plain_text(&frozen.scene.document))};
                    if source["hash"].as_str()!=Some(providers::hash(&text).as_str()){return Err("A supporting source hash does not match.".into())}
                    dependencies.push(SourceDependency{entity_id,entity_type:entity_type.into(),revision_id:revision});
                }
            }
            let saved=storage.save_scene_checked(&sid,changed,&expected,&dependencies).await?;
            Ok(json!({"revisionId":saved.revision_id,"hash":providers::hash(&plain_text(&saved.scene.document))}))
        },
        "provider_status" => providers::status(args).await,
        "provider_run" => {
            let request=&args["request"]; let pid=argument::<String>(request,"projectId")?;
            let bundle=storage.open_project(&pid).await?;
            for source in request["sources"].as_array().ok_or("Missing immutable review sources.")? {
                let sid=argument::<String>(source,"sceneId")?;
                let text=if source["kind"]=="facts" {
                    if request["provider"]!="llm"||sid!=format!("planning:{pid}"){return Err("Invalid canonical fact scope.".into())}
                    let saved=storage.load_project_data_revision(&pid,"planning",&argument::<String>(source,"revisionId")?).await?;
                    providers::canonical_fact_text(&saved.payload)?
                }else{
                    if !bundle.scenes.iter().any(|s|s.id==sid){return Err("Review source is not in this project.".into())}
                    let snapshot=storage.load_revision(&sid,&argument::<String>(source,"revisionId")?).await?;
                    plain_text(&snapshot.scene.document)
                };
                if source["text"].as_str()!=Some(text.as_str())||source["hash"].as_str()!=Some(providers::hash(&text).as_str()){return Err("The saved review source does not match its revision and hash.".into())}
            }
            providers::run(args).await
        },
        "provider_cancel" => providers::cancel(args),
        "credential_storage_status" => Ok(sync_service::credential_storage_status()),
        "sync_devices" => encode(sync.as_ref().map_err(Clone::clone)?.list_devices().await),
        "sync_challenge" => encode(sync.as_ref().map_err(Clone::clone)?.create_pairing_challenge(argument::<sync_service::DeviceIdentity>(&args,"device")?,argument::<Vec<String>>(&args,"projectIds")?,300).await?),
        "sync_revoke" => encode(sync.as_ref().map_err(Clone::clone)?.revoke_device(&argument::<String>(&args,"deviceId")?).await?),
        "sync_start" => {let address=argument::<String>(&args,"address")?.parse::<std::net::SocketAddr>().map_err(|_|"Enter a private IP address and port.")?;let mut guard=listener.lock().await;if guard.is_some(){return Err("Private sync is already enabled. Stop it before changing the address.".into())}let handle=sync.as_ref().map_err(Clone::clone)?.start_listener(address).await?;let value=json!({"enabled":true,"address":handle.address.to_string()});*guard=Some(handle);Ok(value)},
        "sync_stop" => {if let Some(handle)=listener.lock().await.take(){handle.stop().await;}Ok(json!({"enabled":false}))},
        "sync_status" => {let guard=listener.lock().await;Ok(json!({"enabled":guard.is_some(),"address":guard.as_ref().map(|h|h.address.to_string()),"setupError":sync.as_ref().err()}))},
        "text_hash" => Ok(json!(providers::hash(&argument::<String>(&args,"text")?))),
        _ => Err("Unsupported local operation.".into()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(std::env::args_os().nth(1).ok_or("Missing private data directory")?);
    std::fs::create_dir_all(&directory)?;
    let lock = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(directory.join("companion.lock"))?;
    #[cfg(unix)] {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::PermissionsExt;
        lock.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 { return Err("Another companion owns this workspace. Close its window before reopening.".into()); }
    }
    let storage = Arc::new(Storage::open(&directory.join("editor-spike.sqlite3")).await?);
    let recovery=recovery::Recovery::new(&directory).map(Arc::new);
    if let Ok(recovery)=&recovery {recovery.initialize(&storage).await;}
    // Optional recovery/auth metadata failures never remove access to writing.
    let sync=sync_service::SyncService::open(storage.clone(),directory.join("sync-auth.json")).await;
    // Optional connection metadata failure must not block local writing.
    let peer=peer_client::PeerClient::open(storage.clone(),directory.join("peer-clients.json"),Arc::new(peer_client::OsKeyringCredentials));
    let listener=Arc::new(tokio::sync::Mutex::new(None));
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Value>(64);
    std::thread::spawn(move || {
        let stdin = std::io::stdin(); let mut input = stdin.lock();
        loop {
            let mut bytes = Vec::new();
            match (&mut input).take(128_000_001).read_until(b'\n', &mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(_) if bytes.len() > 128_000_000 || bytes.last() != Some(&b'\n') => break,
                _ => if let Ok(value) = serde_json::from_slice(&bytes) { if sender.blocking_send(value).is_err() { break; } },
            }
        }
    });
    let output = Arc::new(std::sync::Mutex::new(std::io::stdout()));
    let mut tasks = tokio::task::JoinSet::new();
    while let Some(request) = receiver.recv().await {
        let storage = storage.clone(); let output = output.clone();let recovery=recovery.clone();let sync=sync.clone();let listener=listener.clone();let peer=peer.clone();
        tasks.spawn(async move {
            let id = request.get("id").cloned().unwrap_or(Value::Null);
            let result = execute(&storage, &recovery, &sync, &listener, &peer, request["op"].as_str().unwrap_or(""), request["args"].clone()).await;
            let response = match result { Ok(value) => json!({"id":id,"ok":true,"value":value}), Err(error) => json!({"id":id,"ok":false,"error":error}) };
            if let Ok(mut writer) = output.lock() { let _ = serde_json::to_writer(&mut *writer, &response); let _ = writer.write_all(b"\n"); let _ = writer.flush(); }
        });
        while tasks.try_join_next().is_some() {}
    }
    while tasks.join_next().await.is_some() {}
    if let Some(handle)=listener.lock().await.take(){handle.stop().await;}
    storage.close().await; drop(lock); Ok(())
}

// The peer source is also compiled by integration fixtures. Register the TLS
// scenarios here only, so workspace tests exercise each certificate case once.
#[cfg(test)]
mod peer_tls_tests {
    #[tokio::test]
    async fn tls_valid_chain_and_hostname_support_real_scoped_pair_sync_and_assets() { super::peer_client::tls_tests::tls_valid_chain_and_hostname_support_real_scoped_pair_sync_and_assets().await; }
    #[tokio::test]
    async fn tls_wrong_hostname_is_rejected_before_http() { super::peer_client::tls_tests::tls_wrong_hostname_is_rejected_before_http().await; }
    #[tokio::test]
    async fn tls_expired_leaf_is_rejected_before_http() { super::peer_client::tls_tests::tls_expired_leaf_is_rejected_before_http().await; }
    #[tokio::test]
    async fn tls_untrusted_chain_is_rejected_before_http() { super::peer_client::tls_tests::tls_untrusted_chain_is_rejected_before_http().await; }
    #[tokio::test]
    async fn tls_redirect_is_refused_after_verified_handshake() { super::peer_client::tls_tests::tls_redirect_is_refused_after_verified_handshake().await; }
}

#[cfg(test)]
mod universe_rpc_tests {
 use super::*;
 #[tokio::test]
 async fn universe_commands_return_atomic_context_and_reject_stale_or_missing_arguments(){
  let d=tempfile::tempdir().unwrap();let storage=Storage::open(&d.path().join("universe-command.sqlite")).await.unwrap();
  let recovery=Err("Synthetic unavailable backup service".to_owned());let sync=Err("Synthetic unavailable sync service".to_owned());let peer=Err("Synthetic unavailable peer service".to_owned());let listener=Arc::new(tokio::sync::Mutex::new(None));
  let universe=execute(&storage,&recovery,&sync,&listener,&peer,"create_universe",json!({"title":"Universe مِ 中文"})).await.unwrap();
  let pid=universe["id"].as_str().unwrap();let head=storage.load_project_data(pid,"universeCatalogue").await.unwrap().unwrap();
  let args=json!({"universeId":pid,"seriesId":null,"title":"Novel 𠀀","parent":head.revision_id});
  let receipt=execute(&storage,&recovery,&sync,&listener,&peer,"create_universe_novel",args.clone()).await.unwrap();
  let created:novel_storage::UniverseNovelCreated=serde_json::from_value(receipt.clone()).unwrap();
  assert_ne!(created.project.id,pid);assert_eq!(created.scene.chapter_id,storage.open_project_index(&created.project.id).await.unwrap().chapters[0].id);assert_eq!(receipt["catalogueRevisionId"],storage.load_project_data(pid,"universeCatalogue").await.unwrap().unwrap().revision_id);
  assert!(execute(&storage,&recovery,&sync,&listener,&peer,"create_universe_novel",args).await.unwrap_err().starts_with("Conflict"));
  assert!(execute(&storage,&recovery,&sync,&listener,&peer,"create_universe_novel",json!({"universeId":pid,"seriesId":null,"title":"Missing parent"})).await.is_err());
  assert_eq!(storage.list_projects().await.unwrap().len(),2);
 }
}

#[cfg(test)]
mod chapter_numbering_rpc_tests {
 use super::*;
 #[tokio::test]
 async fn chapter_command_keeps_numbering_zero_and_rejects_missing_cas_context(){
  let d=tempfile::tempdir().unwrap();let storage=Storage::open(&d.path().join("chapter-command.sqlite")).await.unwrap();
  let recovery=Err("Synthetic unavailable backup service".to_owned());let sync=Err("Synthetic unavailable sync service".to_owned());let peer=Err("Synthetic unavailable peer service".to_owned());let listener=Arc::new(tokio::sync::Mutex::new(None));
  let p=storage.create_project("Novel").await.unwrap();let chapter=storage.create_chapter(&p.id,"First").await.unwrap();
  let captured=execute(&storage,&recovery,&sync,&listener,&peer,"chapter_edit_snapshot",json!({"projectId":p.id,"chapterId":chapter.id})).await.unwrap();
  assert_eq!(captured["chapter"]["title"],"First");assert_eq!(captured["chapterNumberStart"],1);
  let args=json!({"chapterId":chapter.id,"expectedChapterHead":captured["expectedChapterHead"],"expectedProjectHead":captured["expectedProjectHead"],"expectedOrder":captured["expectedOrder"],"numberingParent":captured["numberingParent"],"title":"Prologue مِ 中文","number":0});
  let result=execute(&storage,&recovery,&sync,&listener,&peer,"edit_chapter",args.clone()).await.unwrap();assert_eq!(result["chapterNumberStart"],0);assert_eq!(result["chapters"][0]["title"],"Prologue مِ 中文");assert!(result["chapterNumberingRevisionId"].is_string());
  assert!(execute(&storage,&recovery,&sync,&listener,&peer,"edit_chapter",args).await.is_err());let mut missing=json!({"chapterId":chapter.id,"title":"No overwrite","number":1});assert!(execute(&storage,&recovery,&sync,&listener,&peer,"edit_chapter",missing.take()).await.is_err());assert_eq!(storage.open_project_index(&p.id).await.unwrap().chapter_number_start,0);
 }
}

#[cfg(test)]
mod translation_rpc_tests {
 use super::*;
 #[tokio::test]
 async fn translation_rpc_requires_an_exact_scene_revision_in_the_named_project(){
  let d=tempfile::tempdir().unwrap();let storage=Storage::open(&d.path().join("translation-source.sqlite")).await.unwrap();
  let recovery=Err("Synthetic unavailable backup service".to_owned());let sync=Err("Synthetic unavailable sync service".to_owned());let peer=Err("Synthetic unavailable peer service".to_owned());let listener=Arc::new(tokio::sync::Mutex::new(None));
  let project=storage.create_project("Synthetic translation project").await.unwrap();let chapter=storage.create_chapter(&project.id,"Chapter").await.unwrap();
  let source="Saved synthetic source مِ 中文";
  let scene:Scene=serde_json::from_value(json!({"schemaVersion":1,"title":"Fixture","document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":source}]}]}})).unwrap();
  let saved=storage.create_scene(&chapter.id,scene).await.unwrap();
  let args=json!({"jobId":"translation-rpc-test","runToken":"translation-rpc-test:1","endpoint":"https://example.invalid","model":null,"sourceLanguage":"en","targetLanguage":"ar","text":source,"timeoutMs":120000,"projectId":project.id,"sceneId":saved.id,"sceneRevisionId":saved.revision_id});
  let error=execute(&storage,&recovery,&sync,&listener,&peer,"provider_translate",args.clone()).await.unwrap_err();
  assert!(error.contains("literal loopback"),"the exact saved source should reach provider URL validation: {error}");
  let mut forged=args.clone();forged["text"]=json!("Forged content");
  assert!(execute(&storage,&recovery,&sync,&listener,&peer,"provider_translate",forged).await.unwrap_err().contains("does not match"));
  let other=storage.create_project("Another project").await.unwrap();let foreign_chapter=storage.create_chapter(&other.id,"Chapter").await.unwrap();
  let foreign_scene:Scene=serde_json::from_value(json!({"schemaVersion":1,"title":"Foreign fixture","document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":source}]}]}})).unwrap();
  let foreign=storage.create_scene(&foreign_chapter.id,foreign_scene).await.unwrap();let mut crossed=args;
  crossed["sceneId"]=json!(foreign.id);crossed["sceneRevisionId"]=json!(foreign.revision_id);
  assert!(execute(&storage,&recovery,&sync,&listener,&peer,"provider_translate",crossed).await.unwrap_err().contains("not in this project"));
 }
}
