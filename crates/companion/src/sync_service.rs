//! Explicitly enabled private sync endpoint. Device credentials authorize project
//! replication only; this module has no editor, inference, shell or filesystem RPC.
use novel_storage::{AssetTransferSpec, ProjectBundle, Revision, Storage, SyncCursor};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::{HashMap, HashSet}, fs::{File, OpenOptions}, io::{Read, Write}, net::{IpAddr, SocketAddr}, path::{Path, PathBuf}, process::Command, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::{Mutex, Semaphore, oneshot}, task::{JoinHandle, JoinSet}, time::{Duration, timeout}};
const MAX_BODY: usize = 25_000_000;
const MAX_HEADER: usize = 8192;
const MAX_DEVICES: usize = 32;
const TOKEN_SECONDS: u64 = 90 * 24 * 60 * 60;
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn digest(text: &str) -> String { format!("{:x}", Sha256::digest(text.as_bytes())) }
fn random_secret() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut bytes)).map_err(|_| "Operating-system entropy is unavailable.")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn identity(value: &str) -> bool { !value.is_empty() && value.len() <= 128 && value.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-:".contains(&b)) }
fn equal_hash(a: &str, b: &str) -> bool { a.len() == 64 && b.len() == 64 && a.bytes().zip(b.bytes()).fold(0u8, |difference, (a,b)| difference | (a ^ b)) == 0 }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct DeviceIdentity { pub device_id: String, pub device_epoch: String, pub name: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct DeviceRecord {
    pub identity: DeviceIdentity, pub project_ids: Vec<String>, pub issued_at: u64,
    pub expires_at: u64, pub revoked: bool, token_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PairedDevice { pub identity:DeviceIdentity,pub project_ids:Vec<String>,pub issued_at:u64,pub expires_at:u64,pub revoked:bool }
impl DeviceRecord {fn public(&self)->PairedDevice{PairedDevice{identity:self.identity.clone(),project_ids:self.project_ids.clone(),issued_at:self.issued_at,expires_at:self.expires_at,revoked:self.revoked}}}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Challenge { id: String, identity: DeviceIdentity, project_ids: Vec<String>, expires_at: u64, challenge_hash: String }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct AuthFile { schema_version: u32, hub_epoch: String, devices: Vec<DeviceRecord>, challenges: Vec<Challenge> }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PairingChallenge { pub challenge_id: String, pub challenge: String, pub identity: DeviceIdentity, pub project_ids: Vec<String>, pub expires_at: u64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PairingCompletion { pub challenge_id: String, pub challenge: String, pub device_id: String, pub device_epoch: String }
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct PairingGrant { pub token: String, pub device: PairedDevice, pub hub_epoch: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct RemoteRequest {
    pub protocol_version: u32, pub device_id: String, pub device_epoch: String,
    pub project_id: String, pub operation: String, pub payload: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ProtocolError { pub code: String, pub message: String }
impl ProtocolError { fn new(code: &str, message: &str) -> Self { Self { code: code.into(), message: message.into() } } }
fn validation(message: &str) -> ProtocolError { ProtocolError::new("validation", message) }
fn storage_error(error: String) -> ProtocolError {
    if error.contains("epoch") || error.contains("cursor") { ProtocolError::new("stale_cursor", "The hub epoch or cursor changed. Explicit bootstrap or pairing is required.") }
    else if error.contains("parent") || error.contains("ancestor") { ProtocolError::new("missing_ancestors", "Required immutable ancestors are missing or invalid. Fetch them before retrying.") }
    else if error.starts_with("Conflict") { ProtocolError::new("conflict", "Concurrent revisions were preserved. Inspect and explicitly resolve the competing heads.") }
    else { ProtocolError::new("validation", "The supplied replication data or asset failed validation. No partial acknowledgement was issued.") }
}
fn argument<T: serde::de::DeserializeOwned>(payload: &Value, field: &str) -> Result<T, ProtocolError> {
    serde_json::from_value(payload.get(field).cloned().ok_or_else(|| validation("A required request field is missing."))?).map_err(|_| validation("A request field has an unsupported shape."))
}
fn encode<T: Serialize>(value: T) -> Result<Value, ProtocolError> { serde_json::to_value(value).map_err(|_| ProtocolError::new("internal", "Could not encode the private protocol response.")) }
struct RateWindow { start: u64, count: u32 }
pub struct SyncService { storage: Arc<Storage>, auth_path: PathBuf, auth: Mutex<AuthFile>, rates: Mutex<HashMap<String, RateWindow>> }
impl SyncService {
    pub async fn open(storage: Arc<Storage>, auth_path: PathBuf) -> Result<Arc<Self>, String> {
        let hub_epoch = storage.sync_cursor().await?.epoch;
        let mut auth = if auth_path.exists() {
            let metadata = std::fs::symlink_metadata(&auth_path).map_err(|_| "Could not inspect the private credential file.")?;
            if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 256_000 { return Err("Unsafe private credential-file type or size.".into()) }
            #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; if metadata.permissions().mode() & 0o077 != 0 { return Err("Private credential file must have mode 0600 before it can be used.".into()) } }
            let auth: AuthFile = serde_json::from_slice(&std::fs::read(&auth_path).map_err(|_| "Could not read the private credential file.")?).map_err(|_| "Unsupported or corrupt private credential file.")?;
            if auth.schema_version != 1 || auth.devices.len() > MAX_DEVICES || auth.challenges.len() > MAX_DEVICES { return Err("Unsupported private authentication schema or capacity.".into()) }
            let mut ids = HashSet::new();
            for device in &auth.devices { validate_device(&device.identity)?; if !ids.insert(&device.identity.device_id) || device.token_hash.len()!=64 || device.project_ids.len()>128 || device.project_ids.iter().any(|p| !identity(p)) { return Err("Invalid saved device authorization.".into()) } }
            auth
        } else { AuthFile { schema_version: 1, hub_epoch: hub_epoch.clone(), devices: vec![], challenges: vec![] } };
        // Restored/rotated hubs never keep authorizing old peer tokens or cursors.
        if auth.hub_epoch != hub_epoch { auth.hub_epoch = hub_epoch; for device in &mut auth.devices { device.revoked=true; } auth.challenges.clear(); }
        auth.challenges.retain(|c| c.expires_at > now());
        persist(&auth_path, &auth)?;
        Ok(Arc::new(Self { storage, auth_path, auth: Mutex::new(auth), rates: Mutex::new(HashMap::new()) }))
    }
    /// Local PC command only: issuing this scoped challenge is the PC approval.
    /// It is never an unauthenticated network operation or automatic discovery.
    pub async fn create_pairing_challenge(&self, device: DeviceIdentity, project_ids: Vec<String>, ttl_seconds: u64) -> Result<PairingChallenge, String> {
        self.refresh_epoch().await?;
        validate_device(&device)?;
        if ttl_seconds==0 || ttl_seconds>600 || project_ids.is_empty() || project_ids.len()>128 || project_ids.iter().collect::<HashSet<_>>().len()!=project_ids.len() { return Err("Approve 1–128 distinct projects and a challenge lasting 1–600 seconds.".into()) }
        for pid in &project_ids { if !identity(pid) { return Err("Invalid approved project identity.".into()) } self.storage.open_project(pid).await?; }
        let mut auth=self.auth.lock().await;
        if auth.devices.iter().any(|d|d.identity.device_id==device.device_id && !d.revoked) { return Err("This device identity is already paired. Revoke it before pairing a new device epoch.".into()) }
        if auth.devices.iter().any(|d|d.identity.device_id==device.device_id && d.identity.device_epoch==device.device_epoch) { return Err("Re-pairing requires a fresh device epoch.".into()) }
        auth.challenges.retain(|c|c.expires_at>now());
        if auth.challenges.len()>=MAX_DEVICES || auth.devices.len()>=MAX_DEVICES && !auth.devices.iter().any(|d|d.identity.device_id==device.device_id) { return Err("Pairing capacity has been reached.".into()) }
        if auth.challenges.iter().any(|c|c.identity.device_id==device.device_id) { return Err("A challenge for this identity is already pending.".into()) }
        let challenge=random_secret()?; let challenge_id=random_secret()?; let expires_at=now()+ttl_seconds;
        let next=Challenge {id:challenge_id.clone(),identity:device.clone(),project_ids:project_ids.clone(),expires_at,challenge_hash:digest(&challenge)};
        let mut updated=auth.clone();updated.challenges.push(next);persist(&self.auth_path,&updated)?;*auth=updated;
        Ok(PairingChallenge{challenge_id,challenge,identity:device,project_ids,expires_at})
    }
    pub async fn complete_pairing(&self, completion: PairingCompletion) -> Result<PairingGrant, ProtocolError> {
        if !identity(&completion.device_id)||!identity(&completion.device_epoch)||completion.challenge.len()!=64 { return Err(ProtocolError::new("authentication", "Invalid, expired or already consumed pairing challenge.")) }
        let mut auth=self.auth.lock().await;
        let Some(index)=auth.challenges.iter().position(|c|c.id==completion.challenge_id && c.identity.device_id==completion.device_id && c.identity.device_epoch==completion.device_epoch && c.expires_at>now() && equal_hash(&c.challenge_hash,&digest(&completion.challenge))) else { return Err(ProtocolError::new("authentication", "Invalid, expired or already consumed pairing challenge.")) };
        let token=random_secret().map_err(|_|ProtocolError::new("internal","Operating-system entropy is unavailable."))?;
        let mut updated=auth.clone();let challenge=updated.challenges.remove(index);
        let device=DeviceRecord{identity:challenge.identity,project_ids:challenge.project_ids,issued_at:now(),expires_at:now()+TOKEN_SECONDS,revoked:false,token_hash:digest(&token)};
        updated.devices.retain(|d|d.identity.device_id!=device.identity.device_id);updated.devices.push(device.clone());
        persist(&self.auth_path,&updated).map_err(|_|ProtocolError::new("storage","Pairing could not be committed. Retry the same challenge."))?;*auth=updated;
        Ok(PairingGrant{token,device:device.public(),hub_epoch:auth.hub_epoch.clone()})
    }
    pub async fn rotate_hub_epoch(&self)->Result<SyncCursor,String>{
        let mut auth=self.auth.lock().await;let cursor=self.storage.rotate_epoch().await?;
        let mut updated=auth.clone();updated.hub_epoch=cursor.epoch.clone();updated.challenges.clear();for device in &mut updated.devices{device.revoked=true;}
        persist(&self.auth_path,&updated)?;*auth=updated;Ok(cursor)
    }
    pub async fn refresh_epoch(&self)->Result<(),String>{
        let mut auth=self.auth.lock().await;let hub=self.storage.sync_cursor().await?;
        if auth.hub_epoch!=hub.epoch{let mut updated=auth.clone();updated.hub_epoch=hub.epoch;updated.challenges.clear();for device in &mut updated.devices{device.revoked=true;}persist(&self.auth_path,&updated)?;*auth=updated;}Ok(())
    }
    pub async fn list_devices(&self)->Vec<PairedDevice>{ self.auth.lock().await.devices.iter().map(DeviceRecord::public).collect() }
    pub async fn revoke_device(&self, device_id:&str)->Result<(),String>{
        let mut auth=self.auth.lock().await;let mut updated=auth.clone();let device=updated.devices.iter_mut().find(|d|d.identity.device_id==device_id).ok_or("Paired device not found.")?;device.revoked=true;updated.challenges.retain(|c|c.identity.device_id!=device_id);persist(&self.auth_path,&updated)?;*auth=updated;Ok(())
    }
    async fn rate(&self,key:String,limit:u32)->Result<(),ProtocolError>{
        let mut rates=self.rates.lock().await;let current=now();rates.retain(|_,w|current.saturating_sub(w.start)<60);
        if rates.len()>=1024 && !rates.contains_key(&key){return Err(ProtocolError::new("rate_limit","Private service capacity is temporarily full."))}
        let window=rates.entry(key).or_insert(RateWindow{start:current,count:0});if window.count>=limit{return Err(ProtocolError::new("rate_limit","Private request limit reached. Retry after one minute."))}window.count+=1;Ok(())
    }
    pub async fn handle(&self, token:&str, request:RemoteRequest)->Result<Value,ProtocolError>{
        if request.protocol_version!=1{return Err(ProtocolError::new("unsupported_protocol","This companion supports private sync protocol 1."))}
        if serde_json::to_vec(&request).map_err(|_|validation("Invalid request."))?.len()>MAX_BODY{return Err(ProtocolError::new("too_large","Request exceeds the private protocol size limit."))}
        if token.len()!=64||!token.bytes().all(|c|c.is_ascii_hexdigit()){return Err(ProtocolError::new("authentication","A valid paired-device credential is required."))}
        // Keep authorization locked through the transaction. A completed revocation
        // therefore cannot race with a later acknowledged request.
        let auth=self.auth.lock().await;
        let hub=self.storage.sync_cursor().await.map_err(storage_error)?;
        if auth.hub_epoch!=hub.epoch{return Err(ProtocolError::new("stale_cursor","The restored hub requires new pairing credentials."))}
        let token_hash=digest(token);
        let device=auth.devices.iter().find(|d|d.identity.device_id==request.device_id&&d.identity.device_epoch==request.device_epoch&&!d.revoked&&d.expires_at>now()&&equal_hash(&d.token_hash,&token_hash)).ok_or_else(||ProtocolError::new("authentication","A valid paired-device credential is required."))?;
        if !device.project_ids.contains(&request.project_id){return Err(ProtocolError::new("authorization","This device is not approved for that project."))}
        self.rate(format!("device:{}",request.device_id),120).await?;
        let pid=&request.project_id;let p=&request.payload;
        let result=match request.operation.as_str(){
            "pull"=>encode(self.storage.pull_revisions(pid,argument::<SyncCursor>(p,"cursor")?,argument::<u32>(p,"limit")?).await.map_err(storage_error)?),
            "push"=>{let epoch:String=argument(p,"expectedEpoch")?;let remote:String=argument(p,"remoteEpoch")?;if remote!=device.identity.device_epoch{return Err(validation("Revision sender epoch does not match the paired device epoch."))}encode(self.storage.ingest_revisions(pid,&epoch,&remote,argument::<ProjectBundle>(p,"manifest")?,argument::<Vec<Revision>>(p,"revisions")?).await.map_err(storage_error)?)},
            "manifest"=>encode(self.storage.sync_manifest(pid).await.map_err(storage_error)?),
            "asset_begin"=>encode(self.storage.begin_asset_transfer(pid,argument::<AssetTransferSpec>(p,"spec")?).await.map_err(storage_error)?),
            "asset_append"=>encode(self.storage.append_asset_chunk(pid,&argument::<String>(p,"transferId")?,argument::<usize>(p,"offset")?,argument::<Vec<u8>>(p,"bytes")?).await.map_err(storage_error)?),
            "asset_finalize"=>{let asset=self.storage.finalize_asset_transfer(pid,&argument::<String>(p,"transferId")?).await.map_err(storage_error)?;Ok(json!({"id":asset.id,"path":asset.path,"mimeType":asset.mime_type,"checksum":asset.checksum,"size":asset.content.len()}))},
            "asset_status"=>encode(self.storage.asset_transfer_status(pid,&argument::<String>(p,"transferId")?).await.map_err(storage_error)?),
            "asset_cancel"=>encode(self.storage.cancel_asset_transfer(pid,&argument::<String>(p,"transferId")?).await.map_err(storage_error)?),
            "asset_download"=>{let asset_id:String=argument(p,"assetId")?;let offset:usize=argument(p,"offset")?;let limit:usize=argument(p,"limit")?;let chunk=self.storage.read_asset_chunk(pid,&asset_id,offset,limit).await.map_err(storage_error)?;serde_json::to_value(chunk).map_err(|_|validation("Could not encode the bounded asset chunk."))},
            _=>Err(ProtocolError::new("unsupported_operation","Only scoped revision and verified asset operations are available on this endpoint."))
        };
        drop(auth);result
    }
    /// Explicit local command only. No listener is started by `open`.
    pub async fn start_listener(self:&Arc<Self>,address:SocketAddr)->Result<ListenerHandle,String>{
        verify_bind_address(address)?;
        let listener=TcpListener::bind(address).await.map_err(|_|"Could not bind the requested private endpoint.")?;let address=listener.local_addr().map_err(|_|"Could not inspect private listener address.")?;
        let (stop_tx,mut stop_rx)=oneshot::channel();let service=self.clone();
        let task=tokio::spawn(async move{let semaphore=Arc::new(Semaphore::new(4));let mut connections=JoinSet::new();loop{tokio::select!{
            _=&mut stop_rx=>break,
            accepted=listener.accept()=>{let Ok((stream,peer))=accepted else{break};let Ok(permit)=semaphore.clone().try_acquire_owned()else{drop(stream);continue};let service=service.clone();connections.spawn(async move{let _permit=permit;let _=timeout(Duration::from_secs(30),serve_connection(service,stream,peer)).await;});},
            _=connections.join_next(),if !connections.is_empty()=>{}
        }}connections.abort_all();while connections.join_next().await.is_some(){} });
        Ok(ListenerHandle{address,stop:Some(stop_tx),task})
    }
}
pub struct ListenerHandle{pub address:SocketAddr,stop:Option<oneshot::Sender<()>>,task:JoinHandle<()>}
impl ListenerHandle{pub async fn stop(mut self){if let Some(stop)=self.stop.take(){let _=stop.send(());}let _=(&mut self.task).await;}}
impl Drop for ListenerHandle{fn drop(&mut self){if let Some(stop)=self.stop.take(){let _=stop.send(());}}}
fn validate_device(d:&DeviceIdentity)->Result<(),String>{if !identity(&d.device_id)||!identity(&d.device_epoch)||d.name.trim().is_empty()||d.name.chars().count()>200{return Err("Device identity, epoch and display name are required.".into())}Ok(())}
fn persist(path:&Path,auth:&AuthFile)->Result<(),String>{
    let parent=path.parent().ok_or("Credential destination has no private directory.")?;std::fs::create_dir_all(parent).map_err(|_|"Could not create the private credential directory.")?;
    let temporary=parent.join(format!(".sync-auth-{}.tmp",random_secret()?));
    let result=(||{let mut options=OpenOptions::new();options.write(true).create_new(true);#[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let mut file=options.open(&temporary).map_err(|_|"Could not create a private credential transaction.")?;let bytes=serde_json::to_vec(auth).map_err(|_|"Could not encode credentials.")?;file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|_|"Could not commit private credentials.")?;
        std::fs::rename(&temporary,path).map_err(|_|"Could not advance the private credential transaction.")?;File::open(parent).and_then(|f|f.sync_all()).map_err(|_|"Could not sync the credential directory.")?;Ok(())})();if result.is_err(){let _=std::fs::remove_file(temporary);}result
}
fn verify_bind_address(address:SocketAddr)->Result<(),String>{
    if address.ip().is_loopback(){return Ok(())}if address.ip().is_unspecified(){return Err("Wildcard listeners are forbidden. Use loopback or this device's verified Tailscale address.".into())}
    let plausible=match address.ip(){IpAddr::V4(ip)=>{let n=u32::from(ip);n&0xffc00000==0x64400000},IpAddr::V6(ip)=>{let s=ip.segments();s[0]==0xfd7a&&s[1]==0x115c&&s[2]==0xa1e0}};
    if !plausible{return Err("Public and ordinary LAN listeners are forbidden. Use loopback or a verified Tailscale address.".into())}
    let output=Command::new("tailscale").args(["status","--json"]).output().map_err(|_|"Tailscale address verification is unavailable. Use loopback for private Serve.")?;
    if !output.status.success()||output.stdout.len()>1_000_000{return Err("Could not verify the running Tailscale interface.".into())}let status:Value=serde_json::from_slice(&output.stdout).map_err(|_|"Invalid Tailscale interface status.")?;
    if status["BackendState"].as_str()!=Some("Running")||!status["Self"]["TailscaleIPs"].as_array().is_some_and(|ips|ips.iter().any(|ip|ip.as_str().and_then(|s|s.parse::<IpAddr>().ok())==Some(address.ip()))){return Err("That address is not a verified running Tailscale interface on this PC.".into())}Ok(())
}
async fn serve_connection(service:Arc<SyncService>,mut stream:TcpStream,peer:SocketAddr)->Result<(),String>{
    if let Err(error)=service.rate(format!("ip:{}",peer.ip()),240).await{write_http(&mut stream,429,Err(error)).await?;return Ok(())}
    let parsed=timeout(Duration::from_secs(10),read_http(&mut stream)).await;
    let response=match parsed{
        Ok(Ok((method,path,token,body)))=>match (method.as_str(),path.as_str()){
            ("GET","/v1/status")=>Ok(json!({"protocolVersion":1,"service":"private-project-sync"})),
            ("POST","/v1/pair/complete")=>{if let Err(error)=service.rate(format!("pair:{}",peer.ip()),10).await{Err(error)}else{let request=serde_json::from_slice::<PairingCompletion>(&body).map_err(|_|validation("Invalid pairing request."));match request{Ok(request)=>service.complete_pairing(request).await.and_then(encode),Err(error)=>Err(error)}}},
            ("POST","/v1/sync")=>match serde_json::from_slice::<RemoteRequest>(&body){Ok(request)=>service.handle(token.as_deref().unwrap_or(""),request).await,Err(_)=>Err(validation("Invalid private sync request."))},
            _=>Err(ProtocolError::new("unsupported_operation","Unknown private protocol endpoint."))
        },
        _=>Err(ProtocolError::new("invalid_http","Invalid, oversized or timed-out private HTTP request."))
    };
    let status=match &response{Ok(_)=>200,Err(e)=>match e.code.as_str(){"authentication"=>401,"authorization"=>403,"rate_limit"=>429,"too_large"=>413,"stale_cursor"|"conflict"=>409,_=>400}};write_http(&mut stream,status,response).await
}
async fn read_http(stream:&mut TcpStream)->Result<(String,String,Option<String>,Vec<u8>),String>{
    let mut bytes=vec![];let end=loop{let mut chunk=[0u8;1024];let n=stream.read(&mut chunk).await.map_err(|_|"Read failed.")?;if n==0{return Err("Incomplete HTTP headers.".into())}bytes.extend_from_slice(&chunk[..n]);if let Some(i)=bytes.windows(4).position(|w|w==b"\r\n\r\n"){if i+4>MAX_HEADER{return Err("Headers too large.".into())}break i+4}if bytes.len()>MAX_HEADER{return Err("Headers too large.".into())}};
    let header=std::str::from_utf8(&bytes[..end]).map_err(|_|"Invalid HTTP headers.")?;let mut lines=header.split("\r\n");let parts=lines.next().ok_or("Missing request line.")?.split(' ').collect::<Vec<_>>();if parts.len()!=3||parts[2]!="HTTP/1.1"||!["GET","POST"].contains(&parts[0]){return Err("Unsupported HTTP request.".into())}let method=parts[0].to_owned();let path=parts[1].to_owned();let mut length=None;let mut token=None;let mut host=false;let mut seen=HashSet::new();
    for line in lines.filter(|l|!l.is_empty()){let(key,value)=line.split_once(':').ok_or("Invalid HTTP header.")?;if key.is_empty()||!key.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'){return Err("Invalid HTTP header name.".into())}let key=key.to_ascii_lowercase();if !seen.insert(key.clone()){return Err("Duplicate HTTP headers are not supported.".into())}let value=value.trim();match key.as_str(){"content-length"=>{if value.is_empty()||!value.bytes().all(|b|b.is_ascii_digit()){return Err("Invalid content length.".into())}length=Some(value.parse::<usize>().map_err(|_|"Invalid content length.")?)},"authorization"=>{token=Some(value.strip_prefix("Bearer ").ok_or("Invalid bearer header.")?.to_owned())},"host"=>host=true,"transfer-encoding"|"origin"=>return Err("Transfer encodings and browser-origin requests are not supported.".into()),_=>{}}}
    if !host{return Err("Host header is required.".into())}let length=length.unwrap_or(0);if length>MAX_BODY||method=="GET"&&length!=0||method=="POST"&&!seen.contains("content-length"){return Err("Invalid or oversized request body.".into())}
    let mut body=bytes[end..].to_vec();if body.len()>length{return Err("Pipelining or extra body bytes are not supported.".into())}while body.len()<length{let mut chunk=[0u8;8192];let remaining=(length-body.len()).min(chunk.len());let n=stream.read(&mut chunk[..remaining]).await.map_err(|_|"Body read failed.")?;if n==0{return Err("Incomplete request body.".into())}body.extend_from_slice(&chunk[..n]);}Ok((method,path,token,body))
}
async fn write_http(stream:&mut TcpStream,status:u16,response:Result<Value,ProtocolError>)->Result<(),String>{let body=serde_json::to_vec(&match response{Ok(value)=>json!({"ok":true,"value":value}),Err(error)=>json!({"ok":false,"error":error})}).map_err(|_|"Encoding failed.")?;if body.len()>MAX_BODY{return Err("Response exceeds the private protocol limit.".into())}let status_text=match status{200=>"OK",401=>"Unauthorized",403=>"Forbidden",409=>"Conflict",413=>"Payload Too Large",429=>"Too Many Requests",_=>"Bad Request"};let header=format!("HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n",body.len());stream.write_all(header.as_bytes()).await.map_err(|_|"Response write failed.")?;stream.write_all(&body).await.map_err(|_|"Response write failed.")?;stream.shutdown().await.map_err(|_|"Response close failed.".into())}
/// Presence is reported separately from a successfully unlocked keyring.
pub fn credential_storage_status()->Value{let available=Command::new("secret-tool").arg("--help").output().is_ok_and(|o|o.status.success()||o.status.code()==Some(2)&&String::from_utf8_lossy(&o.stderr).contains("usage: secret-tool"));json!({"serverStorage":"hashed tokens in atomic mode-0600 file","osKeyringAdapterAvailable":available,"osKeyringVerified":false,"note":"No bearer tokens are stored in the server file. Client OS keyring availability/unlock must be verified separately."})}
/// Explicit native client operations. Pairing stores, syncing loads and forgetting clears. These never run during startup and
/// never expose tokens to a WebView or the unauthenticated network protocol.
pub fn store_client_token_in_keyring(peer_id:&str,token:&str)->Result<(),String>{
    if !identity(peer_id)||token.len()!=64||!token.bytes().all(|b|b.is_ascii_hexdigit()){return Err("Invalid native peer credential.".into())}
    keyring_command(&["store","--label=Novel Workspace private sync","application","novel-workspace","peer-id",peer_id],Some(token)).map(|_|())
}
pub fn load_client_token_from_keyring(peer_id:&str)->Result<String,String>{
    if !identity(peer_id){return Err("Invalid native peer identity.".into())}
    let output=keyring_command(&["lookup","application","novel-workspace","peer-id",peer_id],None)?;
    let token=String::from_utf8(output).map_err(|_|"Unsupported keyring credential encoding.")?;let token=token.trim();
    if token.len()!=64||!token.bytes().all(|b|b.is_ascii_hexdigit()){return Err("No usable native peer credential was found in the OS keyring.".into())}Ok(token.into())
}
pub fn clear_client_token_from_keyring(peer_id:&str)->Result<(),String>{if !identity(peer_id){return Err("Invalid native peer identity.".into())}keyring_command(&["clear","application","novel-workspace","peer-id",peer_id],None).map(|_|())}
fn keyring_command(args:&[&str],input:Option<&str>)->Result<Vec<u8>,String>{
    use std::process::Stdio;let mut child=Command::new("secret-tool").args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"The OS keyring adapter is unavailable; no credential was saved.")?;
    if let Some(input)=input{let mut stdin=child.stdin.take().ok_or("The keyring input channel is unavailable.")?;stdin.write_all(input.as_bytes()).and_then(|_|stdin.write_all(b"\n")).map_err(|_|"Could not send a credential to the OS keyring.")?;}else{drop(child.stdin.take());}
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(15);
    loop{match child.try_wait(){Ok(Some(_))=>break,Ok(None) if std::time::Instant::now()<deadline=>std::thread::sleep(std::time::Duration::from_millis(20)),_=>{let _=child.kill();let _=child.wait();return Err("The OS keyring was unavailable or remained locked. Unlock it and retry explicitly.".into())}}}
    let output=child.wait_with_output().map_err(|_|"Could not read the OS keyring response.")?;if !output.status.success()||output.stdout.len()>1024{return Err("The OS keyring did not accept this operation. No file fallback was used.".into())}Ok(output.stdout)
}
