//! Native ownership and lifecycle for bundled workspace windows.
//! Frontends drain their own saves; only native code can commit a handoff.
use serde_json::{json,Value};
use std::{collections::{BTreeMap,BTreeSet},sync::{Mutex,atomic::{AtomicBool,Ordering}},time::Instant};
use tauri::{Emitter,Manager};
const MAX_WINDOWS:usize=8;
const MAX_TABS:usize=64;
const TRANSFER_TIMEOUT_SECS:u64=30;
fn id(value:&str)->bool{!value.is_empty()&&value.len()<=128&&value.bytes().all(|byte|byte.is_ascii_alphanumeric()||b"_-:".contains(&byte))}
fn keys(value:&Value,allowed:&[&str])->Result<(),String>{let object=value.as_object().ok_or("Expected a workspace object.")?;if object.keys().any(|key|!allowed.contains(&key.as_str())){return Err("Unsupported workspace fields.".into())}Ok(())}
fn string(value:&Value,key:&str)->Result<String,String>{value[key].as_str().map(str::to_owned).ok_or_else(||format!("Missing workspace {key}."))}
#[derive(Clone,Debug,Eq)]
struct Route{project_id:String,scene_id:Option<String>,view:String}
impl PartialEq for Route{fn eq(&self,other:&Self)->bool{self.project_id==other.project_id&&self.view==other.view&&(self.view!="write"||self.scene_id==other.scene_id)}}
impl Route{
 fn parse(value:&Value)->Result<Self,String>{keys(value,&["projectId","sceneId","view"])?;let project_id=string(value,"projectId")?;let view=string(value,"view")?;let scene_id=if value["sceneId"].is_null(){None}else{Some(string(value,"sceneId")?)};if !id(&project_id)||scene_id.as_ref().is_some_and(|value|!id(value))||!["write","plan","review","library","alphabets","book"].contains(&view.as_str()){return Err("Invalid workspace route.".into())}if view=="write"&&scene_id.is_none(){return Err("Writing tabs require a scene.".into())}Ok(Self{project_id,scene_id,view})}
 fn value(&self)->Value{json!({"projectId":self.project_id,"sceneId":self.scene_id,"view":self.view})}
}
#[derive(Clone,Debug)]
struct Tab{id:String,title:String,route:Route,scroll_top:f64,book_ui:Value,owner:String,order:u64}
impl Tab{
 fn parse(value:&Value,owner:&str,order:u64)->Result<Self,String>{keys(value,&["id","title","route","ui"])?;let tab_id=string(value,"id")?;let title=string(value,"title")?;if !id(&tab_id)||title.trim().is_empty()||title.chars().count()>200||title.chars().any(char::is_control){return Err("Invalid workspace tab identity or title.".into())}let scroll_top=if value["ui"].is_null(){0.0}else{keys(&value["ui"],&["scrollTop","bookSceneId","bookSceneOffset","bookCharacterId","bookFoldedSceneIds"])?;value["ui"]["scrollTop"].as_f64().ok_or("Invalid scroll position.")?};if !scroll_top.is_finite()||!(0.0..=100_000_000.0).contains(&scroll_top){return Err("Invalid scroll position.".into())}let route=Route::parse(&value["route"])?;let mut book_ui=json!({});
 for field in ["bookSceneId","bookCharacterId"]{if !value["ui"][field].is_null(){let entry=string(&value["ui"],field)?;if route.view!="book"||!id(&entry){return Err("Invalid whole-book tab position.".into())}book_ui[field]=json!(entry);}}
 if !value["ui"]["bookSceneOffset"].is_null(){let offset=value["ui"]["bookSceneOffset"].as_f64().ok_or("Invalid whole-book scene offset.")?;if route.view!="book"||book_ui["bookSceneId"].is_null()||!offset.is_finite()||!(0.0..=100_000_000.0).contains(&offset){return Err("Invalid whole-book scene offset.".into())}book_ui["bookSceneOffset"]=json!(offset);}
 if !value["ui"]["bookFoldedSceneIds"].is_null(){let entries=value["ui"]["bookFoldedSceneIds"].as_array().ok_or("Invalid folded scenes.")?;let mut unique=BTreeSet::new();if route.view!="book"||entries.len()>20_000||entries.iter().any(|entry|entry.as_str().is_none_or(|entry|!id(entry)||!unique.insert(entry))){return Err("Invalid folded scenes.".into())}book_ui["bookFoldedSceneIds"]=json!(entries);}
 Ok(Self{id:tab_id,title,route,scroll_top,book_ui,owner:owner.into(),order})}
 fn value(&self)->Value{let mut ui=self.book_ui.clone();ui["scrollTop"]=json!(self.scroll_top);json!({"id":self.id,"title":self.title,"route":self.route.value(),"ui":ui})}
 fn located(&self,existing:bool)->Value{json!({"tab":self.value(),"ownerWindow":self.owner,"existing":existing})}
}
#[derive(Clone,Debug)]
struct Transfer{id:String,tab_id:String,source:String,target:String,phase:&'static str,created_window:bool,deadline:Instant,timeout_claimed:bool}
#[derive(Clone,Debug)]
struct CloseRequest{id:String,whole_app:bool}
#[derive(Debug)]
struct WindowState{guard:bool,close:Option<CloseRequest>,allow_close:bool}
impl Default for WindowState{fn default()->Self{Self{guard:true,close:None,allow_close:false}}}
#[derive(Default,Debug)]
struct QuitState{awaiting:BTreeSet<String>,ready:BTreeSet<String>}
#[derive(Default,Debug)]
struct Registry{windows:BTreeMap<String,WindowState>,tabs:BTreeMap<String,Tab>,transfers:BTreeMap<String,Transfer>,serial:u64,quit:Option<QuitState>,closing_tabs:BTreeSet<String>}
impl Registry{
 fn next(&mut self,prefix:&str)->String{self.serial+=1;format!("{prefix}-{}",self.serial)}
 fn add_window(&mut self,label:&str)->Result<(),String>{if self.windows.contains_key(label){return Ok(())}if self.windows.len()>=MAX_WINDOWS{return Err("At most eight workspace windows can be open.".into())}self.windows.insert(label.into(),WindowState::default());Ok(())}
 fn ensure_window(&self,label:&str)->Result<(),String>{if !self.windows.contains_key(label){return Err("Unknown workspace window.".into())}Ok(())}
 fn editable(&self,label:&str)->Result<(),String>{self.ensure_window(label)?;if self.quit.is_some()||self.windows[label].close.is_some()||self.windows[label].allow_close{return Err("Finish or cancel closing before changing tabs.".into())}Ok(())}
 fn tab_busy(&self,tab_id:&str)->bool{self.closing_tabs.contains(tab_id)||self.transfers.values().any(|transfer|transfer.tab_id==tab_id)}
 fn register(&mut self,label:&str,value:&Value)->Result<Value,String>{self.ensure_window(label)?;let tab=Tab::parse(value,label,self.serial+1)?;let presentation_only=self.tabs.get(&tab.id).is_some_and(|current|current.owner==label&&current.route==tab.route&&current.route.scene_id==tab.route.scene_id);if !presentation_only{self.editable(label)?;}if self.tab_busy(&tab.id){return Err("This tab is being transferred.".into())}if let Some(current)=self.tabs.get(&tab.id){if current.owner!=label{return Err("Another window owns this tab.".into())}}
  if let Some(existing)=self.tabs.values().find(|current|current.route==tab.route&&current.id!=tab.id){return Ok(existing.located(true))}
  if !self.tabs.contains_key(&tab.id)&&self.tabs.len()>=MAX_TABS{return Err("At most 64 workspace tabs can be open.".into())}let mut tab=tab;if let Some(current)=self.tabs.get(&tab.id){tab.order=current.order}else{self.serial+=1;}let result=tab.located(false);self.tabs.insert(tab.id.clone(),tab);Ok(result)
 }
 fn owned(&self,label:&str,tab_id:&str)->Result<&Tab,String>{self.ensure_window(label)?;let tab=self.tabs.get(tab_id).ok_or("Workspace tab not found.")?;if tab.owner!=label{return Err("Another window owns this tab.".into())}Ok(tab)}
 fn session_tab<'a>(&self,session:&'a str)->Result<&'a str,String>{
  if let Some((parent,scene))=session.split_once(":book:"){
   if !id(scene)||scene.contains(":book:")||self.tabs.get(parent).is_none_or(|tab|tab.route.view!="book"){return Err("This editor does not belong to an open whole-book tab.".into())}Ok(parent)
  }else{Ok(session)}
 }
 fn authorize_session(&self,label:&str,session:&str)->Result<(),String>{self.ensure_window(label)?;let tab_id=self.session_tab(session)?;let Some(tab)=self.tabs.get(tab_id)else{self.editable(label)?;return Ok(())};
  if self.closing_tabs.contains(tab_id){return Err("This writing tab is closing.".into())}
  if let Some(transfer)=self.transfers.values().find(|transfer|transfer.tab_id==tab_id){
   if transfer.target==label&&["awaiting_ready","committing"].contains(&transfer.phase){return Ok(())}
   return Err("This editor is being transferred. Finish or cancel the transfer first.".into())
  }
  if tab.owner==label{return Ok(())}Err("Another window owns this writing tab.".into())}

 fn state(&self,label:&str)->Result<Value,String>{self.ensure_window(label)?;let mut tabs=self.tabs.values().filter(|tab|tab.owner==label).collect::<Vec<_>>();tabs.sort_by_key(|tab|tab.order);let incoming=self.transfers.values().filter(|transfer|transfer.target==label).filter_map(|transfer|self.tabs.get(&transfer.tab_id).map(|tab|json!({"transferId":transfer.id,"tab":tab.value(),"phase":transfer.phase,"sourceWindow":transfer.source}))).collect::<Vec<_>>();let close=self.windows[label].close.as_ref().map(|request|json!({"requestId":request.id,"wholeApp":request.whole_app}));Ok(json!({"windowLabel":label,"tabs":tabs.iter().map(|tab|tab.value()).collect::<Vec<_>>(),"incoming":incoming,"closeRequest":close}))}
 fn move_source(&self,target:&str,tab_id:&str)->Result<String,String>{
  self.editable(target)?;
  let tab=self.tabs.get(tab_id).ok_or("Workspace tab not found.")?;
  self.editable(&tab.owner)?;
  if tab.owner==target{return Err("This tab already belongs to this window.".into())}
  if self.tab_busy(tab_id){return Err("This tab is already being transferred.".into())}
  Ok(tab.owner.clone())
 }
 fn can_close_empty(&self,label:&str)->Result<(),String>{
  self.editable(label)?;
  if self.tabs.values().any(|tab|tab.owner==label)||self.transfers.values().any(|transfer|transfer.source==label||transfer.target==label){return Err("This window still has open or incoming tabs.".into())}
  if !self.windows.iter().any(|(other,state)|other!=label&&!state.allow_close&&state.close.is_none()){return Err("Keep the last workspace window open.".into())}
  Ok(())
 }
 fn reserve_empty_close(&mut self,label:&str)->Result<(),String>{
  self.can_close_empty(label)?;self.windows.get_mut(label).unwrap().allow_close=true;Ok(())
 }
 fn finish_empty_close(&self,label:&str)->Result<(),String>{
  self.ensure_window(label)?;
  if !self.windows[label].allow_close||self.windows[label].close.is_some()||self.quit.is_some(){return Err("The empty-window close changed. Try again.".into())}
  if self.tabs.values().any(|tab|tab.owner==label)||self.transfers.values().any(|transfer|transfer.source==label||transfer.target==label){return Err("This window still has open or incoming tabs.".into())}
  if !self.windows.iter().any(|(other,state)|other!=label&&!state.allow_close&&state.close.is_none()){return Err("Keep the last workspace window open.".into())}
  Ok(())
 }
 fn prepare(&mut self,source:&str,tab_id:&str,destination:&str)->Result<Transfer,String>{
  self.editable(source)?;self.owned(source,tab_id)?;
  if self.tab_busy(tab_id){return Err("This tab is already being transferred.".into())}
  let existing=match destination{
   "new"=>None,
   "main"=>self.windows.keys().find(|label|label.as_str()=="main"&&label.as_str()!=source&&!self.windows[*label].allow_close&&self.windows[*label].close.is_none()).cloned().or_else(||self.windows.keys().find(|label|label.as_str()!=source&&!self.windows[*label].allow_close&&self.windows[*label].close.is_none()).cloned()),
   destination=>{let target=destination.strip_prefix("window:").unwrap_or(destination);self.editable(target)?;if target==source{return Err("This tab already belongs to this window.".into())}Some(target.to_owned())}
  };
  let created_window=existing.is_none();
  let target=if let Some(target)=existing{self.editable(&target)?;target}else{let target=self.next("workspace");self.add_window(&target)?;target};
  let transfer=Transfer{id:self.next("transfer"),tab_id:tab_id.into(),source:source.into(),target,phase:"preparing",created_window,deadline:Instant::now()+std::time::Duration::from_secs(TRANSFER_TIMEOUT_SECS),timeout_claimed:false};
  self.transfers.insert(transfer.id.clone(),transfer.clone());Ok(transfer)
 }
 fn expired_transfers(&mut self,now:Instant)->Vec<String>{self.transfers.values_mut().filter(|transfer|!transfer.timeout_claimed&&transfer.deadline<=now&&["preparing","awaiting_ready"].contains(&transfer.phase)).map(|transfer|{transfer.timeout_claimed=true;transfer.id.clone()}).collect()}
 fn awaiting_ready(&mut self,transfer_id:&str)->Result<Transfer,String>{let transfer=self.transfers.get_mut(transfer_id).ok_or("The transfer was cancelled.")?;if transfer.phase!="preparing"{return Err("Unexpected transfer phase.".into())}transfer.phase="awaiting_ready";Ok(transfer.clone())}
 fn claim_ready(&mut self,label:&str,transfer_id:&str)->Result<Transfer,String>{self.editable(label)?;let transfer=self.transfers.get_mut(transfer_id).ok_or("Transfer not found.")?;if transfer.target!=label||transfer.phase!="awaiting_ready"{return Err("Only the mounted destination can complete this transfer.".into())}transfer.phase="committing";Ok(transfer.clone())}
 fn commit(&mut self,transfer_id:&str)->Result<Transfer,String>{let transfer=self.transfers.get(transfer_id).ok_or("Transfer not found.")?.clone();if transfer.phase!="committing"{return Err("The destination has not acknowledged readiness.".into())}let tab=self.tabs.get_mut(&transfer.tab_id).ok_or("Workspace tab not found.")?;if tab.owner!=transfer.source{return Err("The source ownership changed.".into())}tab.owner=transfer.target.clone();self.serial+=1;tab.order=self.serial;self.transfers.remove(transfer_id);Ok(transfer)}
 fn claim_abort(&mut self,label:Option<&str>,transfer_id:&str)->Result<Transfer,String>{let transfer=self.transfers.get_mut(transfer_id).ok_or("Transfer not found.")?;if label.is_some_and(|label|label!=transfer.source&&label!=transfer.target)||transfer.phase=="committing"||transfer.phase=="aborting"{return Err("This transfer cannot be cancelled by that window or is completing.".into())}transfer.phase="aborting";Ok(transfer.clone())}
 fn abort(&mut self,transfer_id:&str){self.transfers.remove(transfer_id);}
 fn enable_guard(&mut self,label:&str)->Result<(),String>{self.ensure_window(label)?;self.windows.get_mut(label).unwrap().guard=true;Ok(())}
 fn request_close(&mut self,label:&str)->Result<Option<CloseRequest>,String>{self.ensure_window(label)?;if self.windows[label].allow_close{return Ok(None)}if let Some(request)=&self.windows[label].close{return Ok(Some(request.clone()))}if !self.windows[label].guard{return Ok(None)}let request=CloseRequest{id:self.next("close"),whole_app:false};self.windows.get_mut(label).unwrap().close=Some(request.clone());Ok(Some(request))}
 fn begin_quit(&mut self)->Result<Vec<(String,CloseRequest)>,String>{if !self.transfers.is_empty(){return Err("Finish or cancel the tab transfer before quitting.".into())}if self.quit.is_some(){return Ok(vec![])}let labels=self.windows.keys().cloned().collect::<Vec<_>>();let mut requests=vec![];let mut quit=QuitState::default();for label in labels{if self.windows[&label].allow_close{continue}let request=CloseRequest{id:self.next("quit"),whole_app:true};self.windows.get_mut(&label).unwrap().close=Some(request.clone());quit.awaiting.insert(label.clone());requests.push((label,request));}self.quit=Some(quit);Ok(requests)}
 fn can_complete_close(&self,label:&str)->Result<CloseRequest,String>{self.ensure_window(label)?;if self.transfers.values().any(|transfer|transfer.source==label||transfer.target==label){return Err("Wait for this window's tab transfer to finish or cancel it.".into())}self.windows[label].close.clone().ok_or("No guarded close was requested.".into())}
 fn complete_close(&mut self,label:&str)->Result<bool,String>{let request=self.can_complete_close(label)?;if request.whole_app{let quit=self.quit.as_mut().ok_or("No app quit was requested.")?;quit.ready.insert(label.into());Ok(quit.awaiting==quit.ready)}else{let window=self.windows.get_mut(label).unwrap();window.allow_close=true;Ok(false)}}
 fn exit_allowed(&self)->bool{if !self.transfers.is_empty(){return false}self.quit.as_ref().is_some_and(|quit|quit.awaiting==quit.ready)||self.windows.values().all(|window|window.allow_close||!window.guard&&window.close.is_none())}
 fn cancel_close(&mut self,label:&str)->Result<(),String>{self.ensure_window(label)?;if self.windows[label].close.as_ref().is_some_and(|request|request.whole_app){self.quit=None;for window in self.windows.values_mut(){window.close=None;window.allow_close=false;}}else{let window=self.windows.get_mut(label).unwrap();window.close=None;window.allow_close=false;}Ok(())}
 fn remove_window(&mut self,label:&str){self.windows.remove(label);self.tabs.retain(|_,tab|tab.owner!=label);if let Some(quit)=&mut self.quit{quit.awaiting.remove(label);quit.ready.remove(label);}}
}
#[derive(Default)]pub struct WindowManager{registry:Mutex<Registry>,transition:tokio::sync::Mutex<()>,stopped:AtomicBool}
fn locked(state:&WindowManager)->Result<std::sync::MutexGuard<'_,Registry>,String>{state.registry.lock().map_err(|_|"The window registry is unavailable.".into())}
fn emit_change(app:&tauri::AppHandle,label:&str){let _=app.emit_to(label,"workspace-tabs-changed",json!({}));}
/// Every window holding this project must acknowledge its pending saves before export.
pub fn project_export_targets(window:&tauri::Window,project_id:&str)->Result<BTreeSet<String>,String>{
 let state=window.state::<WindowManager>();let registry=locked(&state)?;registry.editable(window.label())?;
 let mut targets=BTreeSet::new();
 for tab in registry.tabs.values().filter(|tab|tab.route.project_id==project_id){
  if registry.tab_busy(&tab.id){return Err("Finish the manuscript tab transfer before exporting Word.".into())}
  registry.editable(&tab.owner)?;targets.insert(tab.owner.clone());
 }
 if !targets.contains(window.label()){return Err("Open this project in the exporting window first.".into())}
 Ok(targets)
}
pub fn initialize(app:&tauri::AppHandle)->Result<(),String>{let state=app.state::<WindowManager>();{let mut registry=locked(&state)?;for label in app.webview_windows().keys(){registry.add_window(label)?;}}let monitor_app=app.clone();std::thread::Builder::new().name("workspace-handoffs".into()).spawn(move||loop{let state=monitor_app.state::<WindowManager>();if state.stopped.load(Ordering::Acquire){break}let expired=match locked(&state){Ok(mut registry)=>registry.expired_transfers(Instant::now()),Err(_)=>break};for transfer_id in expired{let app=monitor_app.clone();tauri::async_runtime::spawn(async move{let _=rollback(app,transfer_id,"The other window did not finish opening. The source tab was retained.".into()).await;});}std::thread::sleep(std::time::Duration::from_millis(500));}).map_err(|_|"Could not start safe window handoff monitoring.")?;Ok(())}
#[tauri::command]pub fn workspace_window_state(window:tauri::Window,state:tauri::State<'_,WindowManager>)->Result<Value,String>{locked(&state)?.state(window.label())}
#[tauri::command]pub fn register_tab(window:tauri::Window,state:tauri::State<'_,WindowManager>,tab:Value)->Result<Value,String>{let result=locked(&state)?.register(window.label(),&tab)?;emit_change(window.app_handle(),window.label());Ok(result)}
#[tauri::command]pub fn workspace_find_tab(window:tauri::Window,state:tauri::State<'_,WindowManager>,route:Value)->Result<Value,String>{let route=Route::parse(&route)?;let registry=locked(&state)?;registry.ensure_window(window.label())?;Ok(registry.tabs.values().find(|tab|tab.route==route).map(|tab|tab.located(true)).unwrap_or(Value::Null))}
#[tauri::command]pub fn workspace_focus_tab(window:tauri::Window,state:tauri::State<'_,WindowManager>,tab_id:String)->Result<(),String>{let owner={let registry=locked(&state)?;registry.ensure_window(window.label())?;let tab=registry.tabs.get(&tab_id).ok_or("Workspace tab not found.")?;if registry.tab_busy(&tab_id){return Err("This tab is being transferred.".into())}tab.owner.clone()};let target=window.app_handle().get_webview_window(&owner).ok_or("The owning window is unavailable.")?;window.app_handle().emit_to(owner.as_str(),"workspace-focus-tab",json!({"tabId":tab_id})).map_err(|_|"Could not focus the workspace tab.")?;target.show().and_then(|_|target.set_focus()).map_err(|_|"Could not focus the owning window.".into())}
#[tauri::command]pub fn workspace_notify_change(window:tauri::Window,state:tauri::State<'_,WindowManager>,project_id:String,tab_id:String)->Result<(),String>{{let registry=locked(&state)?;let tab=registry.owned(window.label(),&tab_id)?;if tab.route.project_id!=project_id{return Err("This tab does not own that project's change notification.".into())}}window.app_handle().emit("workspace-content-changed",json!({"projectId":project_id,"tabId":tab_id})).map_err(|_|"Could not notify the other windows.".into())}
#[tauri::command]pub async fn unregister_tab(window:tauri::Window,state:tauri::State<'_,WindowManager>,lo:tauri::State<'_,crate::editor::EditorState>,tab_id:String)->Result<(),String>{let _transition=state.transition.lock().await;{let mut registry=locked(&state)?;registry.editable(window.label())?;registry.owned(window.label(),&tab_id)?;if registry.tab_busy(&tab_id){return Err("This tab is being transferred.".into())}registry.closing_tabs.insert(tab_id.clone());}if let Err(error)=lo.release_session(&tab_id,window.label()).await{locked(&state)?.closing_tabs.remove(&tab_id);return Err(error)}{let mut registry=locked(&state)?;registry.owned(window.label(),&tab_id)?;registry.tabs.remove(&tab_id);registry.closing_tabs.remove(&tab_id);}emit_change(window.app_handle(),window.label());Ok(())}
async fn rollback(app:tauri::AppHandle,transfer_id:String,error:String)->Result<(),String>{let state=app.state::<WindowManager>();let _transition=state.transition.lock().await;rollback_inner(&app,transfer_id,error).await}
async fn rollback_inner(app:&tauri::AppHandle,transfer_id:String,error:String)->Result<(),String>{let transfer={let state=app.state::<WindowManager>();let mut registry=locked(&state)?;registry.claim_abort(None,&transfer_id)?};let lo=app.state::<crate::editor::EditorState>();let restored=lo.transfer_session(&transfer.tab_id,&transfer.target,&transfer.source).await;if let Err(restore_error)=restored{let state=app.state::<WindowManager>();if let Some(current)=locked(&state)?.transfers.get_mut(&transfer_id){current.phase="restore_failed";}let detail=format!("{error} Editor handoff could not be restored: {restore_error}. Retry cancelling the transfer.");let _=app.emit_to(transfer.source.as_str(),"workspace-transfer-failed",json!({"tabId":transfer.tab_id,"transferId":transfer.id,"error":detail}));return Err(detail)}let state=app.state::<WindowManager>();let mut registry=locked(&state)?;registry.abort(&transfer_id);let created_empty=transfer.created_window&&!registry.tabs.values().any(|tab|tab.owner==transfer.target);if created_empty{if let Some(window)=registry.windows.get_mut(&transfer.target){window.allow_close=true;}}drop(registry);let detail=error;let _=app.emit_to(transfer.source.as_str(),"workspace-transfer-failed",json!({"tabId":transfer.tab_id,"transferId":transfer.id,"error":detail}));emit_change(&app,&transfer.source);emit_change(&app,&transfer.target);if created_empty{if let Some(window)=app.get_webview_window(&transfer.target){let _=window.close();}}Ok(())}
#[tauri::command]
pub fn workspace_toggle_fullscreen(window:tauri::Window)->Result<(),String>{
 let fullscreen=window.is_fullscreen().map_err(|e|e.to_string())?;
 window.set_fullscreen(!fullscreen).map_err(|e|e.to_string())
}
/// The receiving window requests a move; only the current owner can drain and start it.
#[tauri::command]
pub fn workspace_request_tab_move(window:tauri::Window,state:tauri::State<'_,WindowManager>,tab_id:String)->Result<(),String>{
 let source=locked(&state)?.move_source(window.label(),&tab_id)?;
 window.app_handle().emit_to(source.as_str(),"workspace-tab-move-request",json!({"tabId":tab_id,"destinationWindow":window.label()})).map_err(|_|"Could not request the tab move.".into())
}
/// Called only after the source UI removed its acknowledged last tab.
#[tauri::command]
pub async fn workspace_close_empty_window(window:tauri::Window,state:tauri::State<'_,WindowManager>,lo:tauri::State<'_,crate::editor::EditorState>)->Result<(),String>{
 let _transition=state.transition.lock().await;
 locked(&state)?.reserve_empty_close(window.label())?;
 if let Err(error)=lo.release_window_sessions(window.label()).await{locked(&state)?.cancel_close(window.label())?;return Err(error)}
 let completion={locked(&state)?.finish_empty_close(window.label())};
 if let Err(error)=completion{locked(&state)?.cancel_close(window.label())?;return Err(error)}
 if let Err(error)=window.close(){locked(&state)?.cancel_close(window.label())?;return Err(error.to_string())}
 Ok(())
}
#[tauri::command]pub async fn workspace_transfer_tab(window:tauri::Window,state:tauri::State<'_,WindowManager>,lo:tauri::State<'_,crate::editor::EditorState>,tab_id:String,destination:String)->Result<Value,String>{let _transition=state.transition.lock().await;let app=window.app_handle().clone();let transfer=locked(&state)?.prepare(window.label(),&tab_id,&destination)?;
 if transfer.created_window{if let Err(error)=tauri::WebviewWindowBuilder::new(&app,&transfer.target,tauri::WebviewUrl::App("index.html".into())).title("Novel Workspace").disable_drag_drop_handler().enable_clipboard_access().visible(false).inner_size(1200.0,860.0).min_inner_size(360.0,580.0).build(){let mut registry=locked(&state)?;registry.abort(&transfer.id);registry.remove_window(&transfer.target);return Err(format!("Could not prepare another window: {error}"))}}
 if let Err(error)=lo.transfer_session(&tab_id,&transfer.source,&transfer.target).await{locked(&state)?.abort(&transfer.id);if transfer.created_window{if let Some(target)=app.get_webview_window(&transfer.target){locked(&state)?.windows.get_mut(&transfer.target).unwrap().allow_close=true;let _=target.close();}}emit_change(&app,&transfer.source);return Err(error)}
 let phase_result={locked(&state)?.awaiting_ready(&transfer.id)};let ready=match phase_result{Ok(ready)=>ready,Err(error)=>{let _=lo.transfer_session(&tab_id,&transfer.target,&transfer.source).await;return Err(error)}};let result={let registry=locked(&state)?;json!({"transferId":ready.id,"destinationWindow":ready.target,"tab":registry.tabs[&tab_id].value(),"phase":ready.phase})};emit_change(&app,&ready.target);
 Ok(result)
}
#[tauri::command]pub async fn workspace_transfer_ready(window:tauri::Window,state:tauri::State<'_,WindowManager>,transfer_id:String)->Result<(),String>{let _transition=state.transition.lock().await;let transfer=locked(&state)?.claim_ready(window.label(),&transfer_id)?;let target=window.app_handle().get_webview_window(&transfer.target);if target.as_ref().is_none_or(|target|target.show().is_err()){locked(&state)?.transfers.get_mut(&transfer_id).unwrap().phase="awaiting_ready";let _=rollback_inner(window.app_handle(),transfer_id,"The destination could not be shown.".into()).await;return Err("The destination could not be shown. Source tab retained.".into())}let transfer=locked(&state)?.commit(&transfer.id)?;let _=window.app_handle().emit_to(transfer.source.as_str(),"workspace-tab-transferred",json!({"tabId":transfer.tab_id,"transferId":transfer.id,"destinationWindow":transfer.target}));emit_change(window.app_handle(),&transfer.source);emit_change(window.app_handle(),&transfer.target);let _=target.unwrap().set_focus();Ok(())}
#[tauri::command]pub async fn workspace_transfer_abort(window:tauri::Window,state:tauri::State<'_,WindowManager>,transfer_id:String)->Result<(),String>{{let registry=locked(&state)?;let transfer=registry.transfers.get(&transfer_id).ok_or("Transfer not found.")?;if window.label()!=transfer.source&&window.label()!=transfer.target{return Err("Another window owns this transfer.".into())}}rollback(window.app_handle().clone(),transfer_id,"Tab transfer cancelled. Source tab retained.".into()).await}
pub fn enable_guard(window:&tauri::Window)->Result<(),String>{let state=window.state::<WindowManager>();let pending={let mut registry=locked(&state)?;registry.enable_guard(window.label())?;registry.windows[window.label()].close.clone()};if let Some(request)=pending{window.app_handle().emit_to(window.label(),"save-before-close",json!({"requestId":request.id,"wholeApp":request.whole_app})).map_err(|_|"Could not resume the pending save guard.")?;}Ok(())}
/// Explicit editor sessions inherit native tab authority, including unopened tabs.
pub fn authorize_session(window:&tauri::Window,tab_id:&str)->Result<(),String>{let state=window.state::<WindowManager>();let result=locked(&state)?.authorize_session(window.label(),tab_id);result}
/// A destination mount must preserve the drained source editor until native
/// readiness commits the transfer; changed canonical data requires rollback.
pub fn incoming_session(window:&tauri::Window,tab_id:&str)->Result<bool,String>{let state=window.state::<WindowManager>();let registry=locked(&state)?;registry.authorize_session(window.label(),tab_id)?;let parent=registry.session_tab(tab_id)?;Ok(registry.transfers.values().any(|transfer|transfer.tab_id==parent&&transfer.target==window.label()&&["awaiting_ready","committing"].contains(&transfer.phase)))}
pub fn book_scene_binding(window:&tauri::Window,session:&str,project_id:Option<&str>)->Result<Option<String>,String>{
 let Some((parent,scene))=session.split_once(":book:")else{return Ok(None)};
 let state=window.state::<WindowManager>();let registry=locked(&state)?;registry.authorize_session(window.label(),session)?;
 let tab=registry.tabs.get(parent).ok_or("The whole-book tab is no longer open.")?;
 if Some(tab.route.project_id.as_str())!=project_id{return Err("This scene does not belong to the whole-book project.".into())}Ok(Some(scene.into()))
}
pub fn close_requested(window:&tauri::Window)->bool{window.state::<WindowManager>().registry.lock().is_ok_and(|registry|registry.windows.get(window.label()).is_some_and(|state|state.close.is_some()))}
pub fn guard_enabled(window:&tauri::Window)->bool{window.state::<WindowManager>().registry.lock().is_ok_and(|registry|registry.windows.get(window.label()).is_some_and(|state|state.guard))}
#[tauri::command]pub fn workspace_cancel_close(window:tauri::Window,state:tauri::State<'_,WindowManager>)->Result<(),String>{let whole_app={let mut registry=locked(&state)?;let whole=registry.windows.get(window.label()).and_then(|state|state.close.as_ref()).is_some_and(|request|request.whole_app);registry.cancel_close(window.label())?;whole};if whole_app{let _=window.app_handle().emit("workspace-close-cancelled",json!({}));}else{let _=window.app_handle().emit_to(window.label(),"workspace-close-cancelled",json!({}));}Ok(())}
fn emit_closes(app:&tauri::AppHandle,requests:Vec<(String,CloseRequest)>)->Result<(),String>{for(label,request)in requests{if app.emit_to(label.as_str(),"save-before-close",json!({"requestId":request.id,"wholeApp":request.whole_app})).is_err(){let state=app.state::<WindowManager>();{let mut registry=locked(&state)?;registry.quit=None;for window in registry.windows.values_mut(){window.close=None;window.allow_close=false;}}let _=app.emit("workspace-close-cancelled",json!({}));return Err("Could not request a window's save guard. The app remains open.".into())}}Ok(())}
#[tauri::command]pub fn workspace_request_quit(window:tauri::Window,state:tauri::State<'_,WindowManager>)->Result<(),String>{let requests={let mut registry=locked(&state)?;registry.ensure_window(window.label())?;registry.begin_quit()?};emit_closes(window.app_handle(),requests)}
pub async fn complete_window_close(window:tauri::Window)->Result<(),String>{let state=window.state::<WindowManager>();let _transition=state.transition.lock().await;let request=locked(&state)?.can_complete_close(window.label())?;if request.whole_app{let quit_complete=locked(&state)?.complete_close(window.label())?;if quit_complete{window.app_handle().exit(0);}return Ok(())}window.state::<crate::editor::EditorState>().release_window_sessions(window.label()).await?;locked(&state)?.complete_close(window.label())?;if let Err(error)=window.close(){let _=locked(&state)?.cancel_close(window.label());return Err(error.to_string())}Ok(())}

pub fn on_window_event(window:&tauri::Window,event:&tauri::WindowEvent){match event{
 tauri::WindowEvent::CloseRequested{api,..}=>{let state=window.state::<WindowManager>();let request=locked(&state).and_then(|mut registry|registry.request_close(window.label()));match request{Ok(Some(request))=>{api.prevent_close();if window.app_handle().emit_to(window.label(),"save-before-close",json!({"requestId":request.id,"wholeApp":request.whole_app})).is_err(){let _=locked(&state).and_then(|mut registry|registry.cancel_close(window.label()));let _=window.app_handle().emit_to(window.label(),"workspace-lifecycle-error",json!({"error":"Could not request the save guard. The window remains open."}));}},Err(error)=>{api.prevent_close();let _=window.app_handle().emit_to(window.label(),"workspace-lifecycle-error",json!({"error":error}));},Ok(None)=>{}}},
 tauri::WindowEvent::Destroyed=>{if let Ok(mut registry)=window.state::<WindowManager>().registry.lock(){if let Some(state)=registry.windows.get_mut(window.label()){state.allow_close=true;}}let app=window.app_handle().clone();let label=window.label().to_owned();tauri::async_runtime::spawn(async move{let transfer_ids={let state=app.state::<WindowManager>();let registry=match locked(&state){Ok(registry)=>registry,Err(_)=>return};registry.transfers.values().filter(|transfer|transfer.source==label||transfer.target==label).map(|transfer|transfer.id.clone()).collect::<Vec<_>>()};for transfer in transfer_ids{let _=rollback(app.clone(),transfer,"A transfer window closed before readiness. Source tab retained.".into()).await;}let _=app.state::<crate::editor::EditorState>().release_window_sessions(&label).await;let state=app.state::<WindowManager>();if let Ok(mut registry)=locked(&state){registry.remove_window(&label);};});},_=>{}
}}
pub fn on_run_event(app:&tauri::AppHandle,event:&tauri::RunEvent){if matches!(event,tauri::RunEvent::Exit){app.state::<WindowManager>().stopped.store(true,Ordering::Release);return}if let tauri::RunEvent::ExitRequested{api,..}=event{let state=app.state::<WindowManager>();let requests={let Ok(mut registry)=locked(&state)else{api.prevent_exit();return};if registry.exit_allowed(){return}api.prevent_exit();registry.begin_quit()};match requests{Ok(requests)=>{if let Err(error)=emit_closes(app,requests){let _=app.emit("workspace-lifecycle-error",json!({"error":error}));}},Err(error)=>{let _=app.emit("workspace-lifecycle-error",json!({"error":error}));}}}}

#[cfg(test)]
mod tests {
    use super::*;
    fn registry() -> Registry {
        let mut registry = Registry::default();
        registry.add_window("main").unwrap();
        registry.add_window("workspace-existing").unwrap();
        registry
    }
    fn tab(tab_id: &str, scene_id: &str) -> Value {
        json!({"id":tab_id,"title":"الفصل الأول — Chapter 1","route":{"projectId":"project-1","sceneId":scene_id,"view":"write"},"ui":{"scrollTop":1234.5}})
    }
    #[test]
    fn explicit_destination_requests_resolve_current_owner_and_reject_busy_or_foreign_windows() {
        let mut r=registry();r.add_window("workspace-third").unwrap();r.register("main",&tab("tab-1","scene-1")).unwrap();
        assert_eq!(r.move_source("workspace-third","tab-1").unwrap(),"main");
        assert!(r.move_source("main","tab-1").is_err());assert!(r.move_source("unknown","tab-1").is_err());
        assert!(r.prepare("main","tab-1","unknown").is_err());assert!(r.prepare("main","tab-1","main").unwrap().target!="main");
        let transfer=r.transfers.values().next().unwrap().id.clone();r.abort(&transfer);
        let t=r.prepare("main","tab-1","workspace-third").unwrap();assert!(!t.created_window);assert_eq!(t.target,"workspace-third");
        assert!(r.move_source("workspace-existing","tab-1").is_err());
        r.awaiting_ready(&t.id).unwrap();r.claim_ready("workspace-third",&t.id).unwrap();r.commit(&t.id).unwrap();
        assert_eq!(r.move_source("main","tab-1").unwrap(),"workspace-third");
    }
    #[test]
    fn empty_source_closes_only_after_commit_and_never_with_tabs_transfers_or_as_last_window() {
        let mut r=registry();r.register("main",&tab("tab-1","scene-1")).unwrap();assert!(r.can_close_empty("main").is_err());
        let t=r.prepare("main","tab-1","workspace-existing").unwrap();assert!(r.can_close_empty("main").is_err());assert!(r.can_close_empty("workspace-existing").is_err());
        r.awaiting_ready(&t.id).unwrap();r.claim_ready("workspace-existing",&t.id).unwrap();r.commit(&t.id).unwrap();assert!(r.can_close_empty("main").is_ok());assert!(r.can_close_empty("workspace-existing").is_err());
        r.register("main",&tab("tab-2","scene-2")).unwrap();assert!(r.can_close_empty("main").is_err());r.tabs.remove("tab-2");
        r.windows.get_mut("workspace-existing").unwrap().allow_close=true;assert!(r.can_close_empty("main").is_err());
    }
    #[test]
    fn reserved_empty_close_blocks_provisional_editors_and_routes_and_rolls_back_on_failure() {
        let mut r=registry();r.register("workspace-existing",&tab("tab-1","scene-1")).unwrap();
        r.reserve_empty_close("main").unwrap();assert!(r.finish_empty_close("main").is_ok());
        assert!(r.authorize_session("main","provisional").is_err());assert!(r.register("main",&tab("tab-2","scene-2")).is_err());assert!(r.prepare("workspace-existing","tab-1","window:main").is_err());
        r.cancel_close("main").unwrap();assert!(r.authorize_session("main","provisional").is_ok());assert!(r.register("main",&tab("tab-2","scene-2")).is_ok());
    }
    #[test]
    fn routes_require_a_scene_for_writing_and_preserve_tool_context() {
        assert!(Route::parse(&json!({"projectId":"project-1","view":"write"})).is_err());
        let a = Route::parse(&json!({"projectId":"project-1","sceneId":"scene-1","view":"plan"})).unwrap();
        let b = Route::parse(&json!({"projectId":"project-1","view":"plan"})).unwrap();
        assert_eq!(a,b);
        assert_eq!(a.scene_id.as_deref(),Some("scene-1"));
        assert_eq!(a.value()["sceneId"],"scene-1");
        assert!(Route::parse(&json!({"projectId":"../escape","view":"plan"})).is_err());
        assert!(Route::parse(&json!({"projectId":"project-1","view":"plan","url":"https://example.org"})).is_err());
    }
    #[test]
    fn presentation_is_bounded_and_multilingual_state_survives() {
        let mut registry = registry();
        let value = registry.register("main", &tab("tab-1","scene-1")).unwrap();
        assert_eq!(value["tab"]["ui"]["scrollTop"],1234.5);
        assert_eq!(value["tab"]["title"],"الفصل الأول — Chapter 1");
        for (key,value) in [("title",json!("bad\nlabel")),("ui",json!({"scrollTop":-1})),("ui",json!({"scrollTop":100000001})),("ui",json!({"scrollTop":0,"token":"secret"}))] {
            let mut invalid=tab("tab-2","scene-2");invalid[key]=value;
            assert!(registry.register("main",&invalid).is_err());
        }
    }
    #[test]
    fn acknowledged_title_can_update_during_close_without_allowing_new_routes() {
        let mut registry=registry();registry.register("main",&tab("tab-1","scene-1")).unwrap();
        registry.begin_quit().unwrap();
        let mut label=tab("tab-1","scene-1");label["title"]=json!("Acknowledged title مِ 中文");
        assert!(registry.register("main",&label).is_ok());
        assert!(registry.register("main",&tab("tab-1","scene-2")).is_err());
        assert!(registry.register("main",&tab("tab-2","scene-2")).is_err());
        assert_eq!(registry.tabs["tab-1"].route.scene_id.as_deref(),Some("scene-1"));
    }
    #[test]
    fn a_duplicate_route_returns_its_owner_without_stealing() {
        let mut registry=registry();registry.register("main",&tab("tab-1","scene-1")).unwrap();
        let duplicate=registry.register("workspace-existing",&tab("tab-2","scene-1")).unwrap();
        assert_eq!(duplicate["existing"],true);assert_eq!(duplicate["ownerWindow"],"main");
        assert_eq!(registry.tabs.len(),1);
        assert!(registry.register("workspace-existing",&tab("tab-1","scene-2")).is_err());
    }
    #[test]
    fn ownership_changes_only_after_exact_destination_readiness() {
        let mut registry=registry();registry.register("main",&tab("tab-1","scene-1")).unwrap();
        let transfer=registry.prepare("main","tab-1","new").unwrap();
        assert_eq!(registry.tabs["tab-1"].owner,"main");
        assert!(registry.commit(&transfer.id).is_err());
        assert!(registry.claim_ready(&transfer.target,&transfer.id).is_err());
        registry.awaiting_ready(&transfer.id).unwrap();
        assert!(registry.claim_ready("workspace-existing",&transfer.id).is_err());
        assert!(registry.claim_ready("main",&transfer.id).is_err());
        registry.claim_ready(&transfer.target,&transfer.id).unwrap();registry.commit(&transfer.id).unwrap();
        assert_eq!(registry.tabs["tab-1"].owner,transfer.target);
        assert!(registry.claim_ready(&transfer.target,&transfer.id).is_err());
        assert!(registry.owned("main","tab-1").is_err());
    }
    #[test]
    fn cancellation_retains_source_and_blocks_foreign_abort() {
        let mut registry=registry();registry.register("main",&tab("tab-1","scene-1")).unwrap();
        let transfer=registry.prepare("main","tab-1","new").unwrap();registry.awaiting_ready(&transfer.id).unwrap();
        assert!(registry.claim_abort(Some("workspace-existing"),&transfer.id).is_err());
        registry.claim_abort(Some("main"),&transfer.id).unwrap();registry.abort(&transfer.id);
        assert_eq!(registry.tabs["tab-1"].owner,"main");
        registry.register("main",&tab("tab-1","scene-1")).unwrap();
    }
    #[test]
    fn transfer_reserves_tab_and_disallows_quit_and_source_close() {
        let mut registry=registry();registry.register("main",&tab("tab-1","scene-1")).unwrap();
        registry.prepare("main","tab-1","new").unwrap();
        assert!(registry.prepare("main","tab-1","new").is_err());
        assert!(registry.register("main",&tab("tab-1","scene-2")).is_err());
        assert!(registry.begin_quit().is_err());
        registry.request_close("main").unwrap();assert!(registry.can_complete_close("main").is_err());
        assert!(!registry.exit_allowed());
    }
    #[test]
    fn return_uses_a_surviving_window_when_main_is_gone() {
        let mut registry=registry();registry.add_window("workspace-source").unwrap();registry.remove_window("main");
        registry.register("workspace-source",&tab("tab-1","scene-1")).unwrap();
        let transfer=registry.prepare("workspace-source","tab-1","main").unwrap();
        assert_eq!(transfer.target,"workspace-existing");assert!(!transfer.created_window);
    }
    #[test]
    fn hidden_startup_is_guarded_and_single_close_does_not_exit_other_windows() {
        let mut registry=registry();assert!(registry.windows["main"].guard);
        let request=registry.request_close("main").unwrap().unwrap();assert!(!request.whole_app);
        assert!(!registry.complete_close("main").unwrap());assert!(!registry.exit_allowed());
        registry.remove_window("main");assert_eq!(registry.windows.len(),1);assert!(!registry.exit_allowed());
        assert!(registry.complete_close("workspace-existing").is_err());
    }
    #[test]
    fn quit_waits_for_every_guard_and_keeps_completed_windows_inert() {
        let mut registry=registry();let requests=registry.begin_quit().unwrap();assert_eq!(requests.len(),2);
        assert!(requests.iter().all(|(_,request)|request.whole_app));
        assert!(!registry.complete_close("main").unwrap());assert!(!registry.exit_allowed());
        assert!(registry.register("main",&tab("tab-1","scene-1")).is_err());
        assert!(!registry.windows["main"].allow_close);
        assert!(registry.complete_close("workspace-existing").unwrap());assert!(registry.exit_allowed());
    }
    #[test]
    fn a_failed_quit_cancels_every_guard_without_releasing_tabs() {
        let mut registry=registry();registry.register("main",&tab("tab-1","scene-1")).unwrap();
        registry.begin_quit().unwrap();registry.complete_close("main").unwrap();
        registry.cancel_close("workspace-existing").unwrap();assert!(registry.quit.is_none());
        assert!(registry.windows.values().all(|window|window.close.is_none()&&!window.allow_close));
        assert!(!registry.exit_allowed());registry.register("main",&tab("tab-1","scene-1")).unwrap();
        assert_eq!(registry.tabs["tab-1"].owner,"main");
    }
    #[test]
    fn bounded_windows_tabs_and_unknown_destinations_fail_closed() {
        let mut registry=registry();for n in 0..MAX_WINDOWS-2{registry.add_window(&format!("other-{n}")).unwrap();}
        assert!(registry.add_window("extra").is_err());
        for n in 0..MAX_TABS{registry.register("main",&tab(&format!("tab-{n}"),&format!("scene-{n}"))).unwrap();}
        assert!(registry.register("main",&tab("extra","scene-extra")).is_err());
        assert!(registry.prepare("main","tab-0","https://arbitrary.invalid").is_err());
        assert!(registry.prepare("main","tab-0","new").is_err());
        assert!(registry.transfers.is_empty());
    }
    #[test]
    fn one_watchdog_claims_each_timeout_once_and_never_retries_failed_restores() {
        let mut registry=registry();registry.register("main",&tab("tab-1","scene-1")).unwrap();
        let transfer=registry.prepare("main","tab-1","new").unwrap();
        assert!(registry.expired_transfers(transfer.deadline-std::time::Duration::from_secs(1)).is_empty());
        registry.awaiting_ready(&transfer.id).unwrap();
        assert_eq!(registry.expired_transfers(transfer.deadline),vec![transfer.id.clone()]);
        assert!(registry.expired_transfers(transfer.deadline+std::time::Duration::from_secs(1)).is_empty());
        registry.transfers.get_mut(&transfer.id).unwrap().phase="restore_failed";
        registry.transfers.get_mut(&transfer.id).unwrap().timeout_claimed=false;
        assert!(registry.expired_transfers(transfer.deadline+std::time::Duration::from_secs(1)).is_empty());
        assert_eq!(registry.tabs["tab-1"].owner,"main");
    }
    #[test]
    fn startup_close_remains_visible_when_guard_listener_is_installed_later() {
        let mut registry=registry();let request=registry.request_close("main").unwrap().unwrap();
        registry.enable_guard("main").unwrap();
        let state=registry.state("main").unwrap();
        assert_eq!(state["closeRequest"]["requestId"],request.id);
        assert_eq!(state["closeRequest"]["wholeApp"],false);
        assert!(registry.register("main",&tab("tab-1","scene-1")).is_err());
        registry.cancel_close("main").unwrap();
        assert!(registry.state("main").unwrap()["closeRequest"].is_null());
    }
    #[test]
    fn book_child_sessions_inherit_parent_route_owner_and_transfer_barriers(){
        let mut registry=registry();let book=json!({"id":"book-tab","title":"Book","route":{"projectId":"project-1","view":"book"}});registry.register("main",&book).unwrap();
        assert!(registry.authorize_session("main","book-tab:book:scene-1").is_ok());
        assert!(registry.authorize_session("workspace-existing","book-tab:book:scene-1").is_err());
        assert!(registry.authorize_session("main","missing:book:scene-1").is_err());
        assert!(registry.authorize_session("main","book-tab:book:scene-1:book:other").is_err());
        registry.register("main",&tab("writing-tab","scene-1")).unwrap();assert!(registry.authorize_session("main","writing-tab:book:scene-1").is_err());
        let transfer=registry.prepare("main","book-tab","new").unwrap();
        assert!(registry.authorize_session("main","book-tab:book:scene-2").is_err());
        assert!(registry.authorize_session(&transfer.target,"book-tab:book:scene-2").is_err());
        registry.awaiting_ready(&transfer.id).unwrap();assert!(registry.authorize_session(&transfer.target,"book-tab:book:scene-2").is_ok());
        registry.claim_abort(Some("main"),&transfer.id).unwrap();assert!(registry.authorize_session(&transfer.target,"book-tab:book:scene-2").is_err());registry.abort(&transfer.id);
        assert!(registry.authorize_session("main","book-tab:book:scene-2").is_ok());registry.closing_tabs.insert("book-tab".into());assert!(registry.authorize_session("main","book-tab:book:scene-2").is_err());
    }
    #[test]
    fn unopened_editor_ids_are_bound_to_registered_native_tab_owners() {
        let mut registry=registry();registry.register("main",&tab("tab-1","scene-1")).unwrap();
        assert!(registry.authorize_session("main","tab-1").is_ok());
        assert!(registry.authorize_session("workspace-existing","tab-1").is_err());
        assert!(registry.authorize_session("main","unregistered").is_ok());
        assert!(registry.authorize_session("unknown-window","unregistered").is_err());
        let transfer=registry.prepare("main","tab-1","new").unwrap();
        assert!(registry.authorize_session(&transfer.target,"tab-1").is_err());
        registry.awaiting_ready(&transfer.id).unwrap();
        assert!(registry.authorize_session(&transfer.target,"tab-1").is_ok());
        registry.claim_abort(Some("main"),&transfer.id).unwrap();
        assert!(registry.authorize_session(&transfer.target,"tab-1").is_err());
    }
    #[test]
    fn pending_release_cannot_be_transferred_or_replaced() {
        let mut registry=registry();registry.register("main",&tab("tab-1","scene-1")).unwrap();
        registry.closing_tabs.insert("tab-1".into());
        assert!(registry.prepare("main","tab-1","new").is_err());
        assert!(registry.register("main",&tab("tab-1","scene-1")).is_err());
        registry.closing_tabs.remove("tab-1");assert!(registry.prepare("main","tab-1","main").is_ok());
    }
    #[test]
    fn folded_book_ui_is_bounded_unique_and_preserved(){
        let mut book=json!({"id":"book-tab","title":"Book","route":{"projectId":"project-1","view":"book"},"ui":{"scrollTop":20,"bookFoldedSceneIds":["scene-1","scene-2"]}});
        let parsed=Tab::parse(&book,"main",0).unwrap();assert_eq!(parsed.value()["ui"]["bookFoldedSceneIds"],book["ui"]["bookFoldedSceneIds"]);
        for bad in [json!(["scene-1","scene-1"]),json!(["../escape"]),json!([1]),json!({}),json!((0..20_001).map(|i|format!("scene-{i}")).collect::<Vec<_>>())]{book["ui"]["bookFoldedSceneIds"]=bad;assert!(Tab::parse(&book,"main",0).is_err());}
        book["ui"]["bookFoldedSceneIds"]=json!([]);book["route"]["view"]=json!("plan");assert!(Tab::parse(&book,"main",0).is_err());
    }

}
