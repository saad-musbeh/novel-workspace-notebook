//! Explicit desktop loopback peer client. Bearer tokens never cross the WebView
//! boundary or enter project data; only the OS keyring owns stored credentials.
use crate::sync_service::{self, DeviceIdentity, PairingChallenge, PairingCompletion, PairingGrant, RemoteRequest};
use novel_storage::{AssetTransferSpec,AssetTransferStatus,ProjectBundle, Revision, RevisionPage, Storage, SyncCursor};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs::{File, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}, sync::Arc};
use tokio::sync::{Mutex, Semaphore};

const MAX_PEERS:usize=32;
const MAX_RESPONSE:usize=25_000_000;
const PAGE_SIZE:u32=32;
const MAX_PAGES:usize=64;
const ASSET_BUDGET:usize=4*1024*1024;
const ASSET_CHUNK:usize=262144;
fn digest(text:&str)->String{format!("{:x}",Sha256::digest(text.as_bytes()))}
fn identity(value:&str)->bool{!value.is_empty()&&value.len()<=128&&value.bytes().all(|byte|byte.is_ascii_alphanumeric()||b"_-:".contains(&byte))}
fn now()->u64{std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()}
fn random_id()->Result<String,String>{let mut bytes=[0u8;32];File::open("/dev/urandom").and_then(|mut file|file.read_exact(&mut bytes)).map_err(|_|"Operating-system entropy is unavailable.")?;Ok(bytes.iter().map(|byte|format!("{byte:02x}")).collect())}
pub trait CredentialStore:Send+Sync+'static {
    fn store(&self,peer:&str,token:&str)->Result<(),String>;
    fn load(&self,peer:&str)->Result<String,String>;
    fn clear(&self,peer:&str)->Result<(),String>;
}
pub struct OsKeyringCredentials;
impl CredentialStore for OsKeyringCredentials {
    fn store(&self,peer:&str,token:&str)->Result<(),String>{sync_service::store_client_token_in_keyring(peer,token)}
    fn load(&self,peer:&str)->Result<String,String>{sync_service::load_client_token_from_keyring(peer)}
    fn clear(&self,peer:&str)->Result<(),String>{sync_service::clear_client_token_from_keyring(peer)}
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct PullAnchor{id:String,entity_id:String,entity_type:String,checksum:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Peer {
    id:String,endpoint:String,project_id:String,device:DeviceIdentity,
    hub_epoch:String,local_epoch:String,pull_sequence:i64,push_sequence:i64,expires_at:u64,
    #[serde(default)]pull_anchor:Option<PullAnchor>,
    #[serde(default)]min_receipt_count:i64,
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct PeerFile{schema_version:u32,device_id:String,peers:Vec<Peer>}
pub struct PeerClient{storage:Arc<Storage>,path:PathBuf,data:Mutex<PeerFile>,credentials:Arc<dyn CredentialStore>,keyring_gate:Arc<Semaphore>,#[cfg(test)]tls_test_client:Option<reqwest::Client>}
/// Restrict remote endpoints to exact Tailscale Serve DNS names and HTTPS443.
/// Certificate chain and hostname verification remain the native TLS backend's responsibility.
pub fn validate_tailscale_serve_endpoint(endpoint:&str)->Result<reqwest::Url,String>{
    let authority=endpoint.strip_prefix("https://").ok_or("Remote peers require Tailscale Serve HTTPS.")?;
    let authority=authority.strip_suffix('/').unwrap_or(authority);
    let host=authority.strip_suffix(":443").unwrap_or(authority);
    if host.len()>253||!host.ends_with(".ts.net")||host.contains(['/',':','@','?','#','%','\\']){return Err("Use only an HTTPS Tailscale Serve hostname with no credentials, path, query or nonstandard port.".into())}
    let labels=host.split('.').collect::<Vec<_>>();
    if labels.len()!=4||labels.iter().any(|label|label.is_empty()||label.len()>63||label.starts_with('-')||label.ends_with('-')||!label.bytes().all(|byte|byte.is_ascii_lowercase()||byte.is_ascii_digit()||byte==b'-')){return Err("Use the exact lowercase machine.tailnet.ts.net Tailscale Serve hostname.".into())}
    reqwest::Url::parse(&format!("https://{host}/")).map_err(|_|"Invalid Tailscale Serve hostname.".into())
}
fn transport_name(endpoint:&str)->&'static str{if endpoint.starts_with("https://"){"tailscale-https"}else{"loopback-only"}}
fn bounded_http_builder()->reqwest::ClientBuilder{
    // Platform trust and verified hostname/SNI are mandatory. No RPC can alter
    // roots, DNS, certificate verification or proxy configuration.
    reqwest::Client::builder().tls_backend_rustls().no_proxy().redirect(reqwest::redirect::Policy::none()).connect_timeout(std::time::Duration::from_secs(3)).timeout(std::time::Duration::from_secs(30))
}
fn endpoint_url(endpoint:&str)->Result<reqwest::Url,String>{
    if endpoint.starts_with("https://"){return validate_tailscale_serve_endpoint(endpoint)}
    let authority=endpoint.strip_prefix("http://").ok_or("Use literal loopback HTTP or verified Tailscale Serve HTTPS.")?.trim_end_matches('/');
    let port=if let Some(rest)=authority.strip_prefix("127.0.0.1:"){rest}else if let Some(rest)=authority.strip_prefix("[::1]:"){rest}else{return Err("Plaintext remote peers are refused. Use a literal loopback address and explicit port, or Tailscale Serve HTTPS.".into())};
    if port.is_empty()||!port.bytes().all(|byte|byte.is_ascii_digit())||port.parse::<u16>().ok().is_none_or(|port|port==0){return Err("Use a valid explicit loopback port.".into())}
    reqwest::Url::parse(&format!("http://{authority}/")).map_err(|_|"Invalid loopback peer address.".into())
}
pub fn validate_tailscale_addresses(addresses:&[std::net::SocketAddr])->Result<(),String>{
    if addresses.is_empty()||addresses.len()>64||addresses.iter().any(|address|address.port()!=443||match address.ip(){std::net::IpAddr::V4(ip)=>{let bytes=ip.octets();bytes[0]!=100||!(64..=127).contains(&bytes[1])},std::net::IpAddr::V6(ip)=>ip.segments()[..3]!=[0xfd7a,0x115c,0xa1e0]}){return Err("Tailscale Serve DNS must resolve only to Tailscale addresses at port443. No connection was attempted.".into())}Ok(())
}
async fn transport(endpoint:&str)->Result<(reqwest::Client,reqwest::Url),String>{
    let url=endpoint_url(endpoint)?;let mut builder=bounded_http_builder().https_only(url.scheme()=="https");
    if url.scheme()=="https"{
        let host=url.host_str().ok_or("Missing Tailscale Serve hostname.")?;
        let resolved=tokio::time::timeout(std::time::Duration::from_secs(3),tokio::net::lookup_host((host,443))).await.map_err(|_|"Tailscale Serve DNS lookup timed out.")?.map_err(|_|"Tailscale Serve DNS could not resolve this hostname.")?;
        let mut addresses=Vec::new();for address in resolved{if !addresses.contains(&address){addresses.push(address)}if addresses.len()>64{break}}
        validate_tailscale_addresses(&addresses)?;
        // Pin this validated answer for the entire request/pass. TLS retains
        // the original hostname for SNI and verification; no second DNS lookup.
        builder=builder.resolve_to_addrs(host,&addresses);
    }
    let client=builder.build().map_err(|_|"Could not configure the verified private peer client.")?;Ok((client,url))
}

async fn response(response:reqwest::Response)->Result<Value,String>{
    if response.status().is_redirection(){return Err("Peer redirects are refused.".into())}
    if response.content_length().is_some_and(|length|length>MAX_RESPONSE as u64){return Err("Peer response exceeds its size limit.".into())}
    let mut response=response;let mut bytes=Vec::new();while let Some(chunk)=response.chunk().await.map_err(|_|"Could not read the bounded peer response.")?{if bytes.len().saturating_add(chunk.len())>MAX_RESPONSE{return Err("Peer response exceeds its size limit.".into())}bytes.extend_from_slice(&chunk);}
    let body:Value=serde_json::from_slice(&bytes).map_err(|_|"Unsupported peer response.")?;
    if body["ok"]!=true {return Err(match body["error"]["code"].as_str(){Some("authentication")=>"Peer authorization expired or was revoked. Revoke the old device on the hub and pair again.",Some("authorization")=>"This connection is not approved for that project.",Some("stale_cursor")=>"The peer epoch or cursor changed. Forget this connection and pair again explicitly.",Some("rate_limit")=>"The peer request limit was reached. Retry later.",Some("missing_ancestors")=>"The peer rejected missing or invalid immutable ancestors. Saved local revisions were preserved.",Some("conflict")=>"Concurrent revisions were preserved. Refresh Recovery and compare them.",_=>"The peer rejected this bounded request. No acknowledgement was recorded."}.into())}
    if !response.status().is_success(){return Err("The peer returned an inconsistent HTTP status.".into())}
    body.get("value").cloned().ok_or_else(||"Peer acknowledgement is missing.".into())
}
fn persist(path:&Path,data:&PeerFile)->Result<(),String>{
    let parent=path.parent().ok_or("Peer metadata has no private directory.")?;std::fs::create_dir_all(parent).map_err(|_|"Could not create peer metadata directory.")?;
    let temporary=parent.join(format!(".peer-clients-{}.tmp",random_id()?));
    let result=(||{let mut options=OpenOptions::new();options.write(true).create_new(true);#[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}let mut file=options.open(&temporary).map_err(|_|"Could not begin a peer metadata transaction.")?;let bytes=serde_json::to_vec(data).map_err(|_|"Invalid peer metadata.")?;file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|_|"Could not commit peer metadata.")?;std::fs::rename(&temporary,path).map_err(|_|"Could not advance peer metadata.")?;File::open(parent).and_then(|file|file.sync_all()).map_err(|_|"Could not sync the peer metadata directory.")?;Ok(())})();if result.is_err(){let _=std::fs::remove_file(temporary);}result
}
pub fn validate_revision_page(page:&RevisionPage,epoch:&str,old_sequence:i64)->Result<(),String>{
    let mut sequence=old_sequence;for revision in &page.revisions{if revision.sequence<=sequence||revision.sequence>page.next_sequence{return Err("The peer revision range is not strictly increasing within its acknowledged cursor.".into())}sequence=revision.sequence;}
    if page.epoch!=epoch||old_sequence<0||page.next_sequence<old_sequence||page.revisions.len()>PAGE_SIZE as usize||page.has_more&&(page.next_sequence!=sequence||page.revisions.is_empty()){return Err("The peer revision page has an invalid epoch, cursor or bound.".into())}Ok(())
}
impl PeerClient {
    pub fn open(storage:Arc<Storage>,path:PathBuf,credentials:Arc<dyn CredentialStore>)->Result<Arc<Self>,String>{
        let data=if path.exists(){let metadata=std::fs::symlink_metadata(&path).map_err(|_|"Could not inspect peer metadata.")?;if !metadata.is_file()||metadata.file_type().is_symlink()||metadata.len()>128_000{return Err("Unsafe peer metadata type or size.".into())}#[cfg(unix)]{use std::os::unix::fs::PermissionsExt;if metadata.permissions().mode()&0o077!=0{return Err("Peer metadata must have private mode 0600.".into())}}serde_json::from_slice::<PeerFile>(&std::fs::read(&path).map_err(|_|"Could not read peer metadata.")?).map_err(|_|"Unsupported peer metadata. Stored content was preserved.")?}else{PeerFile{schema_version:1,device_id:random_id()?,peers:vec![]}};
        if data.schema_version!=1||!identity(&data.device_id)||data.peers.len()>MAX_PEERS{return Err("Unsupported peer metadata schema or capacity.".into())}let mut ids=std::collections::HashSet::new();for peer in &data.peers{endpoint_url(&peer.endpoint)?;if peer.id!=digest(&(peer.endpoint.clone()+"\0"+&peer.device.device_id+"\0"+&peer.local_epoch+"\0"+&peer.project_id))||!ids.insert(&peer.id)||!identity(&peer.project_id)||!identity(&peer.hub_epoch)||!identity(&peer.local_epoch)||peer.device.device_id!=data.device_id||peer.device.device_epoch!=peer.local_epoch||peer.min_receipt_count<0||peer.pull_sequence<0||peer.push_sequence<0||peer.device.name.is_empty()||peer.device.name.len()>800||peer.pull_anchor.as_ref().is_some_and(|anchor|!identity(&anchor.id)||!identity(&anchor.entity_id)||!["scene","tombstone","projectData","project","chapter","scenePlacement","asset"].contains(&anchor.entity_type.as_str())||anchor.checksum.len()!=64||!anchor.checksum.bytes().all(|byte|byte.is_ascii_hexdigit())){return Err("Invalid scoped peer metadata. Stored content was preserved.".into())}}
        persist(&path,&data)?;Ok(Arc::new(Self{storage,path,data:Mutex::new(data),credentials,keyring_gate:Arc::new(Semaphore::new(1)),#[cfg(test)]tls_test_client:None}))
    }
    async fn http(&self,endpoint:&str)->Result<(reqwest::Client,reqwest::Url),String>{
        #[cfg(test)]if endpoint.starts_with("https://"){if let Some(client)=&self.tls_test_client{return Ok((client.clone(),validate_tailscale_serve_endpoint(endpoint)?))}}
        transport(endpoint).await
    }
    async fn credential(&self,operation:&str,peer:&str,token:Option<String>)->Result<String,String>{
        let permit=self.keyring_gate.clone().try_acquire_owned().map_err(|_|"The OS keyring is busy. Retry explicitly.")?;let credentials=self.credentials.clone();let peer=peer.to_owned();let operation=operation.to_owned();
        tokio::task::spawn_blocking(move||{let _permit=permit;match operation.as_str(){"store"=>credentials.store(&peer,token.as_deref().unwrap_or("")).map(|_|String::new()),"load"=>credentials.load(&peer),"clear"=>credentials.clear(&peer).map(|_|String::new()),_=>Err("Unsupported credential operation.".into())}}).await.map_err(|_|"The credential worker stopped.")?.map_err(|_|"The OS keyring operation failed or remained locked. No plaintext fallback was used.".into())
    }
    pub async fn identity(&self)->Result<Value,String>{let data=self.data.lock().await;let cursor=self.storage.sync_cursor().await?;Ok(json!({"deviceId":data.device_id,"deviceEpoch":cursor.epoch,"transport":"loopback-or-tailscale-https","remoteTailscaleTlsAvailable":true}))}
    pub async fn list(&self)->Value{let data=self.data.lock().await;json!({"transport":"loopback-or-tailscale-https","remoteTailscaleTlsAvailable":true,"peers":data.peers})}
    pub async fn new_identity(&self)->Result<Value,String>{let mut data=self.data.try_lock().map_err(|_|"A peer operation is already in progress.")?;if !data.peers.is_empty(){return Err("Forget all saved peer connections before creating a new pairing identity. Revoke their authorizations on their hubs separately.".into())}let mut next=data.clone();next.device_id=random_id()?;persist(&self.path,&next)?;*data=next;let cursor=self.storage.sync_cursor().await?;Ok(json!({"deviceId":data.device_id,"deviceEpoch":cursor.epoch,"manuscriptEpochChanged":false}))}
    pub async fn probe(&self,endpoint:&str)->Result<Value,String>{let (http,url)=self.http(endpoint).await?;let value=response(http.get(url.join("v1/status").map_err(|_|"Invalid peer endpoint.")?).send().await.map_err(|_|"The private peer could not be reached with its required transport and certificate verification.")?).await?;if value["protocolVersion"]!=1||value["service"]!="private-project-sync"{return Err("This service is not a compatible private sync peer.".into())}Ok(json!({"state":"ready","transport":transport_name(endpoint),"remoteTailscaleTlsAvailable":true}))}
    pub async fn pair(&self,endpoint:&str,challenge:PairingChallenge,project_id:&str)->Result<Value,String>{
        let mut data=self.data.try_lock().map_err(|_|"A peer operation is already in progress.")?;let cursor=self.storage.sync_cursor().await?;
        if challenge.identity.device_id!=data.device_id||challenge.identity.device_epoch!=cursor.epoch||!challenge.project_ids.contains(&project_id.to_owned())||!identity(project_id)||challenge.expires_at<=now()||challenge.expires_at>now()+600||challenge.challenge.len()!=64||!challenge.challenge.bytes().all(|byte|byte.is_ascii_hexdigit())||challenge.challenge_id.len()!=64||challenge.identity.name.is_empty()||challenge.identity.name.chars().count()>200{return Err("Use an unexpired hub challenge for this desktop identity, epoch and selected project.".into())}
        let (http,url)=self.http(endpoint).await?;let endpoint=url.as_str().trim_end_matches('/').to_owned();let id=digest(&(endpoint.clone()+"\0"+&data.device_id+"\0"+&cursor.epoch+"\0"+project_id));if data.peers.iter().any(|peer|peer.id==id)||data.peers.len()>=MAX_PEERS{return Err("This peer is already connected or the 32-connection limit was reached. Forget it before pairing again.".into())}
        let completion=PairingCompletion{challenge_id:challenge.challenge_id,challenge:challenge.challenge,device_id:data.device_id.clone(),device_epoch:cursor.epoch.clone()};
        let value=response(http.post(url.join("v1/pair/complete").map_err(|_|"Invalid peer endpoint.")?).json(&completion).send().await.map_err(|_|"Pairing could not reach the verified private hub. A lost acknowledgement may require a new challenge.")?).await?;
        let grant:PairingGrant=serde_json::from_value(value).map_err(|_|"Invalid pairing acknowledgement. No credential was saved.")?;
        if grant.device.identity.device_id!=data.device_id||grant.device.identity.device_epoch!=cursor.epoch||grant.device.identity.name!=challenge.identity.name||!grant.device.project_ids.contains(&project_id.to_owned())||grant.device.revoked||grant.device.expires_at<=now()||!identity(&grant.hub_epoch)||grant.token.len()!=64||!grant.token.bytes().all(|byte|byte.is_ascii_hexdigit()){return Err("The hub grant did not match the approved identity and scope. No credential was saved.".into())}
        if self.credential("store",&id,Some(grant.token)).await.is_err(){let cleaned=self.credential("clear",&id,None).await.is_ok();return Err(if cleaned{"Pairing credential storage failed and was cleared. Revoke the device on the hub, unlock the OS keyring and pair again.".into()}else{format!("Pairing credential storage and cleanup failed. No file fallback was used. Forget credential {id} explicitly and revoke this device on the hub.")})}
        let peer=Peer{id:id.clone(),endpoint,project_id:project_id.into(),device:grant.device.identity,hub_epoch:grant.hub_epoch,local_epoch:cursor.epoch,pull_sequence:0,push_sequence:0,expires_at:grant.device.expires_at,pull_anchor:None,min_receipt_count:0};let mut next=data.clone();next.peers.push(peer.clone());if persist(&self.path,&next).is_err(){let cleaned=self.credential("clear",&id,None).await.is_ok();return Err(if cleaned{"Peer metadata could not commit; its credential was cleared. Revoke the device on the hub and pair again.".into()}else{format!("Peer metadata commit and keyring cleanup failed. Forget credential {id} explicitly and revoke this device on the hub.")})}*data=next;Ok(json!({"peer":peer,"credentialStorage":"OS keyring","transport":transport_name(&peer.endpoint)}))
    }
    async fn remote(&self,transport:&(reqwest::Client,reqwest::Url),peer:&Peer,token:&str,operation:&str,payload:Value)->Result<Value,String>{let (http,url)=transport;let request=RemoteRequest{protocol_version:1,device_id:peer.device.device_id.clone(),device_epoch:peer.local_epoch.clone(),project_id:peer.project_id.clone(),operation:operation.into(),payload};if serde_json::to_vec(&request).map_err(|_|"Invalid bounded peer request.")?.len()>MAX_RESPONSE{return Err("Peer request exceeds its size limit.".into())}response(http.post(url.join("v1/sync").map_err(|_|"Invalid peer endpoint.")?).bearer_auth(token).json(&request).send().await.map_err(|_|"The verified private sync request failed or timed out. Retrying retains acknowledged cursors.")?).await}
    fn asset_specs(revisions:&[Revision])->Result<Vec<AssetTransferSpec>,String>{revisions.iter().filter(|revision|revision.entity_type=="asset").map(|revision|{let p=&revision.payload;serde_json::from_value(json!({"assetId":p["id"],"path":p["path"],"mimeType":p["mimeType"],"checksum":p["checksum"],"totalSize":p["size"]})).map_err(|_|"Invalid immutable asset descriptor.".into())}).collect()}
    async fn download_asset(&self,transport:&(reqwest::Client,reqwest::Url),peer:&Peer,token:&str,spec:&AssetTransferSpec,budget:&mut usize)->Result<bool,String>{
        if let Some(asset)=self.storage.load_asset(&peer.project_id,&spec.asset_id).await?{if asset.checksum!=spec.checksum||asset.path!=spec.path||asset.mime_type!=spec.mime_type||asset.content.len()!=spec.total_size{return Err("Existing local asset bytes do not match their immutable descriptor.".into())}return Ok(true)}
        let mut transfer=self.storage.begin_asset_transfer(&peer.project_id,spec.clone()).await?;
        while transfer.received<spec.total_size {if *budget==0{return Ok(false)}let limit=ASSET_CHUNK.min(*budget).min(spec.total_size-transfer.received);let value=self.remote(transport,peer,token,"asset_download",json!({"assetId":spec.asset_id,"offset":transfer.received,"limit":limit})).await?;let bytes:Vec<u8>=serde_json::from_value(value["bytes"].clone()).map_err(|_|"Invalid peer asset bytes.")?;if value["assetId"]!=spec.asset_id||value["checksum"]!=spec.checksum||value["offset"].as_u64()!=Some(transfer.received as u64)||value["totalSize"].as_u64()!=Some(spec.total_size as u64)||bytes.is_empty()||bytes.len()>limit{return Err("Peer asset chunk does not match its verified descriptor and offset.".into())}*budget-=bytes.len();transfer=self.storage.append_asset_chunk(&peer.project_id,&transfer.transfer_id,transfer.received,bytes).await?;}
        let asset=self.storage.finalize_asset_transfer(&peer.project_id,&transfer.transfer_id).await?;if asset.id!=spec.asset_id||asset.checksum!=spec.checksum{return Err("Finalized asset did not match its immutable descriptor.".into())}Ok(true)
    }
    async fn upload_asset(&self,transport:&(reqwest::Client,reqwest::Url),peer:&Peer,token:&str,spec:&AssetTransferSpec,budget:&mut usize)->Result<bool,String>{
        let asset=self.storage.load_asset(&peer.project_id,&spec.asset_id).await?.ok_or("Asset bytes referenced by local history are missing. Revision sync remains incomplete; restore the verified bytes before retrying.")?;
        if asset.checksum!=spec.checksum||asset.path!=spec.path||asset.mime_type!=spec.mime_type||asset.content.len()!=spec.total_size{return Err("Local asset bytes do not match their immutable descriptor.".into())}
        drop(asset); // Validate one scoped BLOB once; transfer reads only bounded SQL chunks.
        let mut transfer:AssetTransferStatus=serde_json::from_value(self.remote(transport,peer,token,"asset_begin",json!({"spec":spec})).await?).map_err(|_|"Invalid peer asset transfer acknowledgement.")?;
        if !identity(&transfer.transfer_id)||transfer.received>spec.total_size||transfer.total_size!=spec.total_size||transfer.checksum!=spec.checksum{return Err("Peer asset transfer has an invalid immutable manifest.".into())}
        while transfer.received<spec.total_size {if *budget==0{return Ok(false)}let start=transfer.received;let end=start+ASSET_CHUNK.min(*budget).min(spec.total_size-start);let chunk=self.storage.read_asset_chunk(&peer.project_id,&spec.asset_id,start,end-start).await?;if chunk.asset_id!=spec.asset_id||chunk.checksum!=spec.checksum||chunk.offset!=start||chunk.total_size!=spec.total_size||chunk.bytes.len()!=end-start{return Err("Local asset chunk no longer matches its immutable descriptor.".into())}let next:AssetTransferStatus=serde_json::from_value(self.remote(transport,peer,token,"asset_append",json!({"transferId":transfer.transfer_id,"offset":start,"bytes":chunk.bytes})).await?).map_err(|_|"Invalid peer chunk acknowledgement.")?;if next.transfer_id!=transfer.transfer_id||next.received!=end||next.total_size!=spec.total_size||next.checksum!=spec.checksum{return Err("Peer did not acknowledge the exact asset chunk.".into())}*budget-=end-start;transfer=next;}
        let finalized=self.remote(transport,peer,token,"asset_finalize",json!({"transferId":transfer.transfer_id})).await?;if finalized["id"]!=spec.asset_id||finalized["checksum"]!=spec.checksum||finalized["size"].as_u64()!=Some(spec.total_size as u64){return Err("Peer did not acknowledge the finalized verified asset.".into())}Ok(true)
    }
    fn acknowledge(&self,data:&mut PeerFile,index:usize,peer:&Peer)->Result<(),String>{let mut next=data.clone();next.peers[index]=peer.clone();persist(&self.path,&next)?;*data=next;Ok(())}
    pub async fn forget(&self,peer_id:&str)->Result<Value,String>{if peer_id.len()!=64||!peer_id.bytes().all(|byte|byte.is_ascii_hexdigit()){return Err("Invalid peer credential identity.".into())}let mut data=self.data.try_lock().map_err(|_|"A peer operation is already in progress.")?;self.credential("clear",peer_id,None).await?;let mut next=data.clone();next.peers.retain(|peer|peer.id!=peer_id);persist(&self.path,&next)?;*data=next;Ok(json!({"forgotten":true,"remoteAuthorizationRevoked":false}))}
    async fn verify_pull_anchor(&self,peer:&Peer)->Result<(),String>{
        if peer.pull_sequence==0{return Ok(())}
        if peer.min_receipt_count==0||self.storage.sync_receipt_count(&peer.project_id,&peer.hub_epoch).await?<peer.min_receipt_count{return Err("The database is missing acknowledged peer receipts or was restored from an older backup. Forget and pair again explicitly; no cursor was reset.".into())}
        if let Some(anchor)=&peer.pull_anchor {let revision=self.storage.load_entity_revision(&peer.project_id,&anchor.entity_id,&anchor.entity_type,&anchor.id).await.map_err(|_|"The database no longer contains its asset/revision receipt. It may have been restored from an older backup. Forget and pair again explicitly; no cursor was reset.")?;if revision.checksum!=anchor.checksum{return Err("The immutable peer receipt checksum changed. Stored writing was preserved; explicit repair is required.".into())}}
        Ok(())
    }
    async fn local_anchor_after(&self,project:&str,start:SyncCursor)->Result<Option<PullAnchor>,String>{
        let mut cursor=start;let mut anchor=None;
        for _ in 0..MAX_PAGES{let page=self.storage.pull_revisions(project,cursor.clone(),PAGE_SIZE).await?;if let Some(revision)=page.revisions.last(){anchor=Some(PullAnchor{id:revision.id.clone(),entity_id:revision.entity_id.clone(),entity_type:revision.entity_type.clone(),checksum:revision.checksum.clone()})}if !page.has_more{return Ok(anchor)}cursor.sequence=page.next_sequence;}
        Err("Local receipt verification exceeded its bounded pass. No peer cursor was acknowledged.".into())
    }
    pub async fn sync(&self,peer_id:&str)->Result<Value,String>{tokio::time::timeout(std::time::Duration::from_secs(120),self.sync_bounded(peer_id)).await.map_err(|_|"This sync pass reached its two-minute limit. Acknowledged pages were retained; retry explicitly.")?}
    async fn sync_bounded(&self,peer_id:&str)->Result<Value,String>{
        let mut data=self.data.try_lock().map_err(|_|"A peer operation is already in progress.")?;let index=data.peers.iter().position(|peer|peer.id==peer_id).ok_or("Peer connection not found.")?;let mut peer=data.peers[index].clone();let local=self.storage.sync_cursor().await?;if local.epoch!=peer.local_epoch||peer.expires_at<=now(){return Err("The local epoch changed or this authorization expired. Revoke the old device and pair again explicitly.".into())}self.verify_pull_anchor(&peer).await?;let token=self.credential("load",&peer.id,None).await?;if token.len()!=64||!token.bytes().all(|byte|byte.is_ascii_hexdigit()){return Err("No usable OS keyring credential was found.".into())}
        let transport=&self.http(&peer.endpoint).await?;
        let manifest:ProjectBundle=serde_json::from_value(self.remote(transport,&peer,&token,"manifest",json!({})).await?).map_err(|_|"Invalid scoped peer manifest.")?;if manifest.project.id!=peer.project_id{return Err("The peer manifest belongs to another project.".into())}
        let mut pulled=0usize;let mut pushed=0usize;let mut more_pull=false;let mut more_push=false;let mut budget=ASSET_BUDGET;let mut pending_assets=Vec::new();let mut asset_count=0usize;let mut received_assets=HashSet::new();let mut sent_assets=HashSet::new();
        for _ in 0..MAX_PAGES {
            let page:RevisionPage=serde_json::from_value(self.remote(transport,&peer,&token,"pull",json!({"cursor":{"epoch":peer.hub_epoch,"sequence":peer.pull_sequence},"limit":PAGE_SIZE})).await?).map_err(|_|"Invalid revision page acknowledgement.")?;
            validate_revision_page(&page,&peer.hub_epoch,peer.pull_sequence)?;
            let count=page.revisions.len();let local_before=self.storage.sync_cursor().await?;let specs=Self::asset_specs(&page.revisions)?;self.storage.ingest_revisions(&peer.project_id,&local.epoch,&peer.hub_epoch,manifest.clone(),page.revisions).await?;pulled+=count;
            for spec in specs{let key=serde_json::to_string(&spec).map_err(|_|"Invalid asset identity.")?;if received_assets.contains(&key){continue}if !self.download_asset(transport,&peer,&token,&spec,&mut budget).await?{pending_assets.push(json!({"assetId":spec.asset_id,"direction":"receive"}));break}received_assets.insert(key);asset_count+=1;}
            if !pending_assets.is_empty(){more_pull=true;break}
            peer.pull_sequence=page.next_sequence;peer.min_receipt_count=self.storage.sync_receipt_count(&peer.project_id,&peer.hub_epoch).await?;if let Some(anchor)=self.local_anchor_after(&peer.project_id,local_before).await?{peer.pull_anchor=Some(anchor)}self.acknowledge(&mut data,index,&peer)?;more_pull=page.has_more;if !page.has_more{break}
        }
        // Complete ancestor and asset receipt before pushing. Partial transfers
        // resume from the native verified offset on the next explicit pass.
        if !more_pull {
            let manifest=self.storage.sync_manifest(&peer.project_id).await?;
            for _ in 0..MAX_PAGES{
                let page=self.storage.pull_revisions(&peer.project_id,SyncCursor{epoch:local.epoch.clone(),sequence:peer.push_sequence},PAGE_SIZE).await?;let count=page.revisions.len();let specs=Self::asset_specs(&page.revisions)?;
                for spec in specs{let key=serde_json::to_string(&spec).map_err(|_|"Invalid asset identity.")?;if sent_assets.contains(&key){continue}if !self.upload_asset(transport,&peer,&token,&spec,&mut budget).await?{pending_assets.push(json!({"assetId":spec.asset_id,"direction":"send"}));break}sent_assets.insert(key);asset_count+=1;}
                if !pending_assets.is_empty(){more_push=true;break}
                if count>0{let receipt=self.remote(transport,&peer,&token,"push",json!({"expectedEpoch":peer.hub_epoch,"remoteEpoch":peer.local_epoch,"manifest":manifest,"revisions":page.revisions})).await?;if receipt["inserted"].as_u64().zip(receipt["duplicates"].as_u64()).is_none_or(|(inserted,duplicates)|inserted.saturating_add(duplicates)!=count as u64){return Err("The peer did not acknowledge the complete revision batch.".into())}}
                peer.push_sequence=page.next_sequence;self.acknowledge(&mut data,index,&peer)?;pushed+=count;more_push=page.has_more;if !page.has_more{break}
            }
        }
        Ok(json!({"projectId":peer.project_id,"pulledRevisions":pulled,"pushedRevisions":pushed,"morePending":more_pull||more_push,"pendingAssets":pending_assets,"assetReceipts":asset_count,"assetBytesTransferred":ASSET_BUDGET-budget,"assetsComplete":!more_pull&&!more_push,"transport":transport_name(&peer.endpoint)}))
    }

    pub async fn dispatch(&self,operation:&str,args:Value)->Result<Value,String>{fn string<'a>(args:&'a Value,key:&str)->Result<&'a str,String>{args[key].as_str().ok_or_else(||format!("Missing peer {key}."))}match operation{"peer_identity"=>self.identity().await,"peer_new_identity"=>self.new_identity().await,"peer_list"=>Ok(self.list().await),"peer_probe"=>self.probe(string(&args,"endpoint")?).await,"peer_pair"=>self.pair(string(&args,"endpoint")?,serde_json::from_value(args["challenge"].clone()).map_err(|_|"Unsupported pairing challenge.")?,string(&args,"projectId")?).await,"peer_forget"=>self.forget(string(&args,"peerId")?).await,"peer_sync"=>self.sync(string(&args,"peerId")?).await,_=>Err("Unsupported desktop peer operation.".into())}}
}

#[cfg(test)]
pub(crate) mod tls_tests {
    use super::*;
    use rcgen::{BasicConstraints,CertificateParams,CertifiedIssuer,ExtendedKeyUsagePurpose,IsCa,KeyPair,KeyUsagePurpose};
    use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::{TcpListener,TcpStream},task::{JoinHandle,JoinSet}};
    use tokio_rustls::{rustls::{self,pki_types::PrivatePkcs8KeyDer},TlsAcceptor};
    use std::{net::SocketAddr,sync::{atomic::{AtomicUsize,Ordering},Mutex as StdMutex}};
    const HOST:&str="writer.tailfixture.ts.net";
    const ENDPOINT:&str="https://writer.tailfixture.ts.net";
    struct TlsFixture{address:SocketAddr,root:reqwest::Certificate,requests:Arc<AtomicUsize>,task:JoinHandle<()>}
    impl TlsFixture{async fn stop(self){self.task.abort();let _=self.task.await;}}
    async fn tls_fixture(host:&str,expired:bool,backend:Option<SocketAddr>,redirect:bool)->TlsFixture{
        let mut root=CertificateParams::new(vec![]).unwrap();root.is_ca=IsCa::Ca(BasicConstraints::Unconstrained);root.key_usages=vec![KeyUsagePurpose::KeyCertSign,KeyUsagePurpose::CrlSign];
        let ca=CertifiedIssuer::self_signed(root,KeyPair::generate().unwrap()).unwrap();
        let mut leaf=CertificateParams::new(vec![host.into()]).unwrap();leaf.extended_key_usages=vec![ExtendedKeyUsagePurpose::ServerAuth];leaf.key_usages=vec![KeyUsagePurpose::DigitalSignature];
        if expired{leaf.not_before=rcgen::date_time_ymd(2000,1,1);leaf.not_after=rcgen::date_time_ymd(2001,1,1);}
        let key=KeyPair::generate().unwrap();let cert=leaf.signed_by(&key,&ca).unwrap();
        let config=rustls::ServerConfig::builder().with_no_client_auth().with_single_cert(vec![cert.der().clone()],PrivatePkcs8KeyDer::from(key.serialize_der()).into()).unwrap();
        let acceptor=TlsAcceptor::from(Arc::new(config));let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();let address=listener.local_addr().unwrap();let requests=Arc::new(AtomicUsize::new(0));let count=requests.clone();
        let task=tokio::spawn(async move{let mut connections=JoinSet::new();loop{tokio::select!{
            accepted=listener.accept()=>{let Ok((stream,_))=accepted else{break};let acceptor=acceptor.clone();let count=count.clone();connections.spawn(async move{
                let Ok(Ok(mut tls))=tokio::time::timeout(std::time::Duration::from_secs(3),acceptor.accept(stream)).await else{return};
                // TLS verification succeeds before a single application request.
                if let Some(address)=backend{let Ok(mut plain)=TcpStream::connect(address).await else{return};count.fetch_add(1,Ordering::Relaxed);let _=tokio::io::copy_bidirectional(&mut tls,&mut plain).await;}
                else{let mut header=[0u8;4096];if tls.read(&mut header).await.unwrap_or(0)==0{return}count.fetch_add(1,Ordering::Relaxed);let body="{\"ok\":true,\"value\":{\"protocolVersion\":1,\"service\":\"private-project-sync\"}}";let status=if redirect{"302 Found"}else{"200 OK"};let extra=if redirect{"Location: https://example.invalid/\r\n"}else{""};let response=format!("HTTP/1.1 {status}\r\n{extra}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());let _=tls.write_all(response.as_bytes()).await;let _=tls.shutdown().await;}
            });},
            _=connections.join_next(),if !connections.is_empty()=>{}
        }}});
        TlsFixture{address,root:reqwest::Certificate::from_der(ca.der()).unwrap(),requests,task}
    }
    // This private helper and the override field exist only under cfg(test).
    // No production RPC, settings document or environment variable can set roots/DNS.
    fn fixture_client(fixture:&TlsFixture,trust:bool)->reqwest::Client{
        let builder=bounded_http_builder().https_only(true).resolve(HOST,fixture.address);
        let builder=if trust{builder.tls_certs_only([fixture.root.clone()])}else{builder};builder.build().unwrap()
    }
    fn override_tls(client:&mut Arc<PeerClient>,fixture:&TlsFixture,trust:bool){Arc::get_mut(client).unwrap().tls_test_client=Some(fixture_client(fixture,trust));}
    #[derive(Default)]struct FakeCredentials(StdMutex<std::collections::HashMap<String,String>>);
    impl CredentialStore for FakeCredentials{
        fn store(&self,id:&str,token:&str)->Result<(),String>{self.0.lock().unwrap().insert(id.into(),token.into());Ok(())}
        fn load(&self,id:&str)->Result<String,String>{self.0.lock().unwrap().get(id).cloned().ok_or("Missing test credential".into())}
        fn clear(&self,id:&str)->Result<(),String>{self.0.lock().unwrap().remove(id);Ok(())}
    }
    async fn local_client(directory:&Path)->Arc<PeerClient>{let storage=Arc::new(Storage::open(&directory.join("peer.sqlite")).await.unwrap());PeerClient::open(storage,directory.join("peers.json"),Arc::new(FakeCredentials::default())).unwrap()}
    pub(crate) async fn tls_valid_chain_and_hostname_support_real_scoped_pair_sync_and_assets(){
        let directory=tempfile::tempdir().unwrap();let hub=Arc::new(Storage::open(&directory.path().join("hub.sqlite")).await.unwrap());let project=hub.create_project("TLS synthetic").await.unwrap();let chapter=hub.create_chapter(&project.id,"Chapter").await.unwrap();
        let scene=hub.create_scene(&chapter.id,novel_storage::Scene{schema_version:1,title:"TLS مِ 中文 😀".into(),document:json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Synthetic TLS writing"}]}]})}).await.unwrap();
        let content=vec![42u8;500_000];hub.put_asset(&project.id,novel_storage::ProjectAsset{id:"tls-fixture-asset".into(),path:"assets/tls.txt".into(),mime_type:"text/plain".into(),checksum:format!("{:x}",Sha256::digest(&content)),content}).await.unwrap();
        let hub_service=sync_service::SyncService::open(hub.clone(),directory.path().join("auth.json")).await.unwrap();let hub_listener=hub_service.start_listener("127.0.0.1:0".parse().unwrap()).await.unwrap();let fixture=tls_fixture(HOST,false,Some(hub_listener.address),false).await;
        let mut client=local_client(directory.path()).await;override_tls(&mut client,&fixture,true);assert_eq!(client.probe(ENDPOINT).await.unwrap()["transport"],"tailscale-https");
        let identity=client.identity().await.unwrap();let challenge=hub_service.create_pairing_challenge(DeviceIdentity{device_id:identity["deviceId"].as_str().unwrap().into(),device_epoch:identity["deviceEpoch"].as_str().unwrap().into(),name:"TLS peer".into()},vec![project.id.clone()],60).await.unwrap();
        let paired=client.pair(&format!("{ENDPOINT}:443/"),challenge,&project.id).await.unwrap();assert_eq!(paired["peer"]["endpoint"],ENDPOINT);assert_eq!(paired["transport"],"tailscale-https");let id=paired["peer"]["id"].as_str().unwrap();let token=client.credentials.load(id).unwrap();assert!(!paired.to_string().contains(&token));assert!(!client.list().await.to_string().contains(&token));assert!(!std::fs::read_to_string(directory.path().join("peers.json")).unwrap().contains(&token));
        let synced=client.sync(id).await.unwrap();assert_eq!(synced["assetsComplete"],true);assert_eq!(synced["transport"],"tailscale-https");assert_eq!(client.storage.load_scene(&scene.id).await.unwrap(),hub.load_scene(&scene.id).await.unwrap());assert_eq!(client.storage.load_asset(&project.id,"tls-fixture-asset").await.unwrap(),hub.load_asset(&project.id,"tls-fixture-asset").await.unwrap());
        let before=client.storage.sync_cursor().await.unwrap();client.sync(id).await.unwrap();assert_eq!(client.storage.sync_cursor().await.unwrap(),before);assert!(fixture.requests.load(Ordering::Relaxed)>5);fixture.stop().await;hub_listener.stop().await;
    }
    async fn rejects_certificate(host:&str,expired:bool,trust:bool){
        let directory=tempfile::tempdir().unwrap();let fixture=tls_fixture(host,expired,None,false).await;let mut client=local_client(directory.path()).await;override_tls(&mut client,&fixture,trust);
        assert!(client.probe(ENDPOINT).await.unwrap_err().contains("certificate verification"));assert_eq!(fixture.requests.load(Ordering::Relaxed),0);assert!(client.list().await["peers"].as_array().unwrap().is_empty());fixture.stop().await;
    }
    pub(crate) async fn tls_wrong_hostname_is_rejected_before_http(){rejects_certificate("wrong.tailfixture.ts.net",false,true).await;}
    pub(crate) async fn tls_expired_leaf_is_rejected_before_http(){rejects_certificate(HOST,true,true).await;}
    pub(crate) async fn tls_untrusted_chain_is_rejected_before_http(){rejects_certificate(HOST,false,false).await;}
    pub(crate) async fn tls_redirect_is_refused_after_verified_handshake(){let directory=tempfile::tempdir().unwrap();let fixture=tls_fixture(HOST,false,None,true).await;let mut client=local_client(directory.path()).await;override_tls(&mut client,&fixture,true);assert!(client.probe(ENDPOINT).await.unwrap_err().contains("redirect"));assert_eq!(fixture.requests.load(Ordering::Relaxed),1);fixture.stop().await;}
}
