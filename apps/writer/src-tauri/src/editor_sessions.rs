//! Native session ownership. The registry lock is never held during editor work.
use std::{collections::HashMap, sync::{Arc, Mutex}};

struct OwnedSession<T> { owner:String, value:Option<T>, released:bool }
struct Entry<T> { owner:String, inner:Arc<Mutex<OwnedSession<T>>> }
pub(crate) struct SessionHandle<T>(Arc<Mutex<OwnedSession<T>>>);
impl<T> Clone for SessionHandle<T> {fn clone(&self)->Self {Self(self.0.clone())}}
pub(crate) struct SessionRegistry<T> { entries:Mutex<HashMap<String,Entry<T>>>, capacity:usize }
pub(crate) enum SessionUpdate<T,R> {Keep(R), Replace(T,R)}
pub(crate) fn validate_resume_binding(incoming_handoff:bool,exact_match:bool)->Result<(),String>{
    if incoming_handoff&&!exact_match{return Err("The scene or alphabet changed during the transfer. The source editor is retained. Cancel the transfer and refresh before trying again.".into())}Ok(())
}
impl<T> SessionRegistry<T> {
    pub(crate) fn new(capacity:usize)->Self {Self{entries:Mutex::new(HashMap::new()),capacity}}
    pub(crate) fn reserve(&self,id:&str,owner:&str)->Result<SessionHandle<T>,String> {
        let mut entries=self.entries.lock().map_err(|_|"Editor session registry is unavailable.")?;
        if let Some(entry)=entries.get(id){return Ok(SessionHandle(entry.inner.clone()))}
        if entries.len()>=self.capacity{return Err("Too many native editor sessions. Close a writing tab before opening another.".into())}
        let inner=Arc::new(Mutex::new(OwnedSession{owner:owner.into(),value:None,released:false}));
        entries.insert(id.into(),Entry{owner:owner.into(),inner:inner.clone()});Ok(SessionHandle(inner))
    }
    pub(crate) fn get(&self,id:&str)->Result<Option<SessionHandle<T>>,String> {
        let entries=self.entries.lock().map_err(|_|"Editor session registry is unavailable.")?;
        Ok(entries.get(id).map(|entry|SessionHandle(entry.inner.clone())))
    }
    /// Initialize/replace under the old session lock, then atomically retire its
    /// handle. Already queued requests cannot accidentally enter the new editor.
    #[cfg(test)]
    pub(crate) fn update<R>(&self,id:&str,owner:&str,handle:&SessionHandle<T>,action:impl FnOnce(&mut Option<T>)->Result<SessionUpdate<T,R>,String>)->Result<R,String>{
        self.update_handle(id,owner,handle,&mut Some(action))?.ok_or_else(||"Editor session was released.".into())
    }
    /// Opens may have waited for an earlier open to replace their candidate.
    /// Retry lookup before invoking the action; never replay editor mutations.
    pub(crate) fn update_latest<R>(&self,id:&str,owner:&str,action:impl FnOnce(&mut Option<T>)->Result<SessionUpdate<T,R>,String>)->Result<R,String>{
        let handle=self.reserve(id,owner)?;
        self.update_latest_from(id,owner,handle,action)
    }
    fn update_latest_from<R>(&self,id:&str,owner:&str,mut handle:SessionHandle<T>,action:impl FnOnce(&mut Option<T>)->Result<SessionUpdate<T,R>,String>)->Result<R,String>{
        let mut action=Some(action);
        loop {
            if let Some(result)=self.update_handle(id,owner,&handle,&mut action)?{return Ok(result)}
            handle=self.reserve(id,owner)?;
        }
    }
    fn update_handle<R,F>(&self,id:&str,owner:&str,handle:&SessionHandle<T>,action:&mut Option<F>)->Result<Option<R>,String>
    where F:FnOnce(&mut Option<T>)->Result<SessionUpdate<T,R>,String>{
        let mut session=handle.0.lock().map_err(|_|"Editor session is unavailable.")?;
        if session.released{return Ok(None)}
        if session.owner!=owner{return Err("This window no longer owns the editor session.".into())}
        match action.take().ok_or("Editor initialization was already consumed.")?(&mut session.value)? {
            SessionUpdate::Keep(result)=>Ok(Some(result)),
            SessionUpdate::Replace(value,result)=>{
                let inner=Arc::new(Mutex::new(OwnedSession{owner:owner.into(),value:Some(value),released:false}));
                {let mut entries=self.entries.lock().map_err(|_|"Editor session registry is unavailable.")?;
                    let entry=entries.get_mut(id).filter(|entry|Arc::ptr_eq(&entry.inner,&handle.0)).ok_or("Editor session was replaced.")?;
                    entry.inner=inner;
                }
                session.released=true;let old=session.value.take();drop(session);drop(old);Ok(Some(result))
            }
        }
    }
    pub(crate) fn transfer(&self,id:&str,from:&str,to:&str)->Result<(),String> {
        loop {
        let Some(handle)=self.get(id)? else{return Ok(())};
        let mut session=handle.0.lock().map_err(|_|"Editor session is unavailable.")?;
        // An in-flight open may have replaced this handle while transfer waited.
        if session.released{continue}
        if session.owner!=from{return Err("This window no longer owns the editor session.".into())}
        let mut entries=self.entries.lock().map_err(|_|"Editor session registry is unavailable.")?;
        let entry=entries.get_mut(id).filter(|entry|Arc::ptr_eq(&entry.inner,&handle.0)).ok_or("Editor session was replaced.")?;
        entry.owner=to.into();session.owner=to.into();return Ok(())
        }
    }
    pub(crate) fn release(&self,id:&str,owner:&str)->Result<(),String> {
        if !self.release_current(id,owner)?{return Err("This window no longer owns the editor session.".into())}Ok(())
    }
    fn release_current(&self,id:&str,owner:&str)->Result<bool,String>{
        loop {let Some(handle)=self.get(id)? else{return Ok(true)};
            if let Some(released)=self.release_handle(id,owner,&handle)?{return Ok(released)}
        }
    }
    fn release_handle(&self,id:&str,owner:&str,handle:&SessionHandle<T>)->Result<Option<bool>,String> {
        let mut session=handle.0.lock().map_err(|_|"Editor session is unavailable.")?;
        if session.released{return Ok(None)}
        if session.owner!=owner{return Ok(Some(false))}
        {let mut entries=self.entries.lock().map_err(|_|"Editor session registry is unavailable.")?;
        if entries.get(id).is_some_and(|entry|Arc::ptr_eq(&entry.inner,&handle.0)){entries.remove(id);}}
        session.released=true;let value=session.value.take();drop(session);
        // Dropping a process may wait. Other sessions and the registry stay free.
        drop(value);Ok(Some(true))
    }
    pub(crate) fn group_ids(&self,tab:&str)->Result<Vec<String>,String>{
        let entries=self.entries.lock().map_err(|_|"Editor session registry is unavailable.")?;
        let prefix=format!("{tab}:book:");
        let mut ids=entries.keys().filter(|id|id.as_str()==tab||id.starts_with(&prefix)).cloned().collect::<Vec<_>>();ids.sort();Ok(ids)
    }
    pub(crate) fn transfer_group(&self,tab:&str,from:&str,to:&str)->Result<(),String>{
        let ids=self.group_ids(tab)?;let mut moved:Vec<String>=Vec::new();
        for id in ids {if let Err(error)=self.transfer(&id,from,to){
            for prior in moved.iter().rev(){self.transfer(prior,to,from)?;}
            return Err(error)
        }moved.push(id);}
        Ok(())
    }
    pub(crate) fn release_group(&self,tab:&str,owner:&str)->Result<(),String>{
        let ids=self.group_ids(tab)?;
        // Refuse a mixed-owner group before dropping any confirmed editor.
        for id in &ids {if let Some(handle)=self.get(id)?{handle.with_owned(owner,|_|Ok(()))?;}}
        for id in ids {self.release(&id,owner)?;}Ok(())
    }
    pub(crate) fn release_window(&self,owner:&str)->Result<(),String> {
        let owned={let entries=self.entries.lock().map_err(|_|"Editor session registry is unavailable.")?;
            entries.iter().filter(|(_,entry)|entry.owner==owner).map(|(id,_)|id.clone()).collect::<Vec<_>>()};
        for id in owned {self.release_current(&id,owner)?;}Ok(())
    }
}
#[cfg(test)]
pub(crate) mod latest_tests {
    use super::*;
    use std::{sync::{mpsc,atomic::{AtomicUsize,Ordering}},thread,time::Duration};
    pub(crate) fn overlapping_open_retries_retired_candidate_once_but_old_edits_stay_retired(){
        let registry=Arc::new(SessionRegistry::new(2));let old=registry.reserve("tab","main").unwrap();
        old.with_owned("main",|value|{*value=Some("initial".to_owned());Ok(())}).unwrap();
        let (started_tx,started_rx)=mpsc::channel();let (release_tx,release_rx)=mpsc::channel();
        let first_registry=registry.clone();let first_handle=old.clone();
        let first=thread::spawn(move||first_registry.update("tab","main",&first_handle,|_|{
            started_tx.send(()).unwrap();release_rx.recv().unwrap();Ok(SessionUpdate::Replace("first acknowledged source".into(),()))
        }));
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        // Force the second open's candidate to be the exact handle held by the
        // first open, independent of thread scheduling before lookup.
        let second_registry=registry.clone();let second_handle=old.clone();let calls=Arc::new(AtomicUsize::new(0));let invoked=calls.clone();
        let (done_tx,done_rx)=mpsc::channel();
        let second=thread::spawn(move||done_tx.send(second_registry.update_latest_from("tab","main",second_handle,|value|{
            invoked.fetch_add(1,Ordering::SeqCst);assert_eq!(value.as_deref(),Some("first acknowledged source"));Ok(SessionUpdate::Replace("second acknowledged source".into(),"opened"))
        })).unwrap());
        assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
        registry.reserve("independent","other").unwrap();
        release_tx.send(()).unwrap();first.join().unwrap().unwrap();
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap(),"opened");second.join().unwrap();assert_eq!(calls.load(Ordering::SeqCst),1);
        assert!(old.with_owned("main",|_|->Result<(),String>{panic!("Old edits cannot enter replacement")}).is_err());
        registry.get("tab").unwrap().unwrap().with_owned("main",|value|{assert_eq!(value.as_deref(),Some("second acknowledged source"));Ok(())}).unwrap();
    }
    pub(crate) fn retry_preserves_current_owner_and_never_consumes_foreign_open_action(){
        let registry=SessionRegistry::<String>::new(1);let old=registry.reserve("tab","main").unwrap();
        registry.update("tab","main",&old,|_|Ok(SessionUpdate::Replace("current".into(),()))).unwrap();
        registry.transfer("tab","main","detached").unwrap();
        assert!(registry.update_latest_from("tab","main",old,|_|->Result<SessionUpdate<String,()>,String>{panic!("Foreign open must never execute")}).is_err());
        registry.update_latest("tab","detached",|value|{assert_eq!(value.as_deref(),Some("current"));Ok(SessionUpdate::Keep(()))}).unwrap();
    }
}
impl<T> SessionHandle<T> {
    pub(crate) fn with_owned<R>(&self,owner:&str,action:impl FnOnce(&mut Option<T>)->Result<R,String>)->Result<R,String> {
        let mut session=self.0.lock().map_err(|_|"Editor session is unavailable.")?;
        if session.released{return Err("Editor session was released.".into())}
        if session.owner!=owner{return Err("This window no longer owns the editor session.".into())}
        action(&mut session.value)
    }
}

#[cfg(test)]pub(crate) mod group_tests {
 use super::*;
 pub(crate) fn handoff_and_release_include_only_exact_book_children(){
  let registry=SessionRegistry::<usize>::new(20);
  for id in ["tab","tab:book:one","tab:book:two","tabs:book:one","other"]{registry.reserve(id,"main").unwrap();}
  registry.transfer_group("tab","main","detached").unwrap();
  for id in ["tab","tab:book:one","tab:book:two"]{assert!(registry.get(id).unwrap().unwrap().with_owned("detached",|_|Ok(())).is_ok());}
  assert!(registry.get("tabs:book:one").unwrap().unwrap().with_owned("main",|_|Ok(())).is_ok());
  registry.release_group("tab","main").unwrap_err();assert!(registry.get("tab").unwrap().is_some());
  registry.release_group("tab","detached").unwrap();assert!(registry.get("tab:book:two").unwrap().is_none());assert!(registry.get("other").unwrap().is_some());
 }
 pub(crate) fn failed_group_transfer_rolls_back_previous_members(){
  let registry=SessionRegistry::<usize>::new(20);registry.reserve("tab:book:a","main").unwrap();registry.reserve("tab:book:b","other").unwrap();
  assert!(registry.transfer_group("tab","main","detached").is_err());
  assert!(registry.get("tab:book:a").unwrap().unwrap().with_owned("main",|_|Ok(())).is_ok());
  assert!(registry.get("tab:book:b").unwrap().unwrap().with_owned("other",|_|Ok(())).is_ok());
 }
}
