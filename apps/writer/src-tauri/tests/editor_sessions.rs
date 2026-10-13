//! Standalone std-only ownership checks: compile with rustc --edition=2021 --test.
//! No native UI, editor process, manuscript, font or dependency build is used.
#[path="../src/editor_sessions.rs"]
mod editor_sessions;
use editor_sessions::{SessionRegistry,SessionUpdate,validate_resume_binding};
use std::{sync::{Arc,mpsc,atomic::{AtomicUsize,Ordering}},thread,time::Duration};

#[derive(Debug,PartialEq)]
struct FakeEditor { text:String, undo:Vec<String>, caret:usize }
fn fake()->FakeEditor {FakeEditor{text:"Synthetic مِ 中文 😀".into(),undo:vec!["Before edit".into()],caret:9}}

#[test]
fn two_visible_sessions_in_one_window_keep_independent_edits_undo_and_release(){
    let registry=SessionRegistry::new(2);
    let left=registry.reserve("left-pane","main").unwrap();let right=registry.reserve("right-pane","main").unwrap();
    for (handle,text) in [(&left,"Left مِ"),(&right,"Right 中文")] {
        handle.with_owned("main",|value|{*value=Some(FakeEditor{text:text.into(),undo:vec![],caret:0});Ok(())}).unwrap();
    }
    // Focus is a frontend concern: either owned session can receive queued work
    // without replacing/resuming the other or erasing its native history.
    for (handle,addition) in [(&left," 😀"),(&right," عَ"),(&left," 中文")] {
        handle.with_owned("main",|value|{let editor=value.as_mut().unwrap();editor.undo.push(editor.text.clone());editor.text.push_str(addition);editor.caret=editor.text.len();Ok(())}).unwrap();
    }
    left.with_owned("main",|value|{let editor=value.as_mut().unwrap();editor.text=editor.undo.pop().unwrap();assert_eq!(editor.text,"Left مِ 😀");Ok(())}).unwrap();
    right.with_owned("main",|value|{let editor=value.as_ref().unwrap();assert_eq!(editor.text,"Right 中文 عَ");assert_eq!(editor.undo,vec!["Right 中文"]);assert_eq!(editor.caret,editor.text.len());Ok(())}).unwrap();
    registry.release("left-pane","main").unwrap();
    assert!(left.with_owned("main",|_|->Result<(),String>{panic!("Released left cannot enter right")}).is_err());
    right.with_owned("main",|value|{let editor=value.as_mut().unwrap();editor.text=editor.undo.pop().unwrap();assert_eq!(editor.text,"Right 中文");Ok(())}).unwrap();
}

#[test]
fn blocked_left_pane_does_not_block_right_pane_in_the_same_window(){
    let registry=Arc::new(SessionRegistry::new(2));
    let left=registry.reserve("left-pane","main").unwrap();let right=registry.reserve("right-pane","main").unwrap();
    let(started_tx,started_rx)=mpsc::channel();let(release_tx,release_rx)=mpsc::channel();
    let worker=thread::spawn(move||left.with_owned("main",|value|{*value=Some(fake());started_tx.send(()).unwrap();release_rx.recv().unwrap();Ok(())}));
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let(done_tx,done_rx)=mpsc::channel();let second=thread::spawn(move||done_tx.send(right.with_owned("main",|value|{*value=Some(fake());value.as_mut().unwrap().text.push_str(" Right acknowledgement");Ok(())})).unwrap());
    let result=done_rx.recv_timeout(Duration::from_secs(2));
    release_tx.send(()).unwrap();worker.join().unwrap().unwrap();second.join().unwrap();
    result.expect("Same-window right pane must work while left is held").unwrap();
    registry.get("right-pane").unwrap().unwrap().with_owned("main",|value|{assert!(value.as_ref().unwrap().text.ends_with("Right acknowledgement"));Ok(())}).unwrap();
}

#[test]
fn changed_handoff_binding_refuses_replacement_and_rollback_keeps_exact_undo_and_caret(){
    let registry=SessionRegistry::new(2);let handle=registry.reserve("tab","main").unwrap();
    handle.with_owned("main",|value|{*value=Some(fake());Ok(())}).unwrap();
    registry.transfer("tab","main","destination").unwrap();
    // Both a changed document and changed font/config are represented by a
    // failed exact match; refusing must happen before constructing replacement.
    for _ in ["document changed","alphabet changed"]{
        let refusal=registry.update_latest("tab","destination",|value|{
            assert_eq!(value.as_ref().unwrap(),&fake());validate_resume_binding(true,false)?;
            panic!("A mismatched incoming handoff cannot replace the editor");
            #[allow(unreachable_code)] Ok(SessionUpdate::Replace(fake(),()))
        });
        assert!(refusal.unwrap_err().contains("source editor is retained"));
        handle.with_owned("destination",|value|{assert_eq!(value.as_ref().unwrap(),&fake());Ok(())}).unwrap();
    }
    registry.update_latest("tab","destination",|value|{validate_resume_binding(true,true)?;assert_eq!(value.as_ref().unwrap(),&fake());Ok(SessionUpdate::Keep(()))}).unwrap();
    registry.transfer("tab","destination","main").unwrap();
    handle.with_owned("main",|value|{assert_eq!(value.as_ref().unwrap(),&fake());Ok(())}).unwrap();
    // An ordinary explicit refresh by the owner may still replace acknowledged
    // content; the guard is specific to an incoming transfer.
    registry.update_latest("tab","main",|_|{validate_resume_binding(false,false)?;Ok(SessionUpdate::Replace(FakeEditor{text:"Approved refresh".into(),undo:vec![],caret:0},()))}).unwrap();
    assert!(handle.with_owned("main",|_|Ok(())).is_err());
}

#[test]
fn transfer_and_rollback_keep_exact_session_state_and_reject_old_owner(){
    let registry=SessionRegistry::new(4);
    let handle=registry.reserve("tab-one","main").unwrap();
    handle.with_owned("main",|value|{*value=Some(fake());Ok(())}).unwrap();
    registry.transfer("tab-one","main","detached-one").unwrap();
    assert!(handle.with_owned("main",|_|->Result<(),String>{panic!("Old owner must not enter editor work")}).is_err());
    assert!(registry.release("tab-one","main").is_err());
    assert!(registry.transfer("tab-one","wrong-window","main").is_err());
    let resumed=registry.reserve("tab-one","detached-one").unwrap();
    resumed.with_owned("detached-one",|value|{assert_eq!(value.as_ref().unwrap(),&fake());Ok(())}).unwrap();
    registry.transfer("tab-one","detached-one","main").unwrap();
    handle.with_owned("main",|value|{assert_eq!(value.as_ref().unwrap(),&fake());Ok(())}).unwrap();
    registry.transfer("unopened-tool-tab","main","detached-two").unwrap();
    assert!(registry.get("unopened-tool-tab").unwrap().is_none());
}

#[test]
fn transfer_waits_for_old_in_flight_work_without_blocking_other_editors(){
    let registry=Arc::new(SessionRegistry::new(4));
    let first=registry.reserve("first","main").unwrap();
    let second=registry.reserve("second","other").unwrap();
    let(started_tx,started_rx)=mpsc::channel();let(release_tx,release_rx)=mpsc::channel();
    let worker=thread::spawn(move||first.with_owned("main",|value|{
        *value=Some(fake());started_tx.send(()).unwrap();release_rx.recv().unwrap();
        value.as_mut().unwrap().text.push_str(" acknowledged before transfer");Ok(())
    }));
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let(done_tx,done_rx)=mpsc::channel();let transferring=registry.clone();
    let transfer=thread::spawn(move||{done_tx.send(transferring.transfer("first","main","detached")).unwrap();});
    assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
    second.with_owned("other",|value|{*value=Some(fake());Ok(())}).unwrap();
    // Registry lookup/reservation is independent from the blocked session too.
    registry.reserve("third","main").unwrap();
    release_tx.send(()).unwrap();worker.join().unwrap().unwrap();
    done_rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();transfer.join().unwrap();
    registry.get("first").unwrap().unwrap().with_owned("detached",|value|{
        assert!(value.as_ref().unwrap().text.ends_with("acknowledged before transfer"));
        assert_eq!(value.as_ref().unwrap().undo,vec!["Before edit"]);Ok(())
    }).unwrap();
}

struct Tracked(Arc<AtomicUsize>);
impl Drop for Tracked {fn drop(&mut self){self.0.fetch_add(1,Ordering::SeqCst);}}
#[test]
fn scoped_release_drops_only_owned_sessions_and_stale_handles_never_reach_reused_ids(){
    let registry=SessionRegistry::new(3);let drops=Arc::new(AtomicUsize::new(0));
    let old=registry.reserve("first","main").unwrap();
    old.with_owned("main",|value|{*value=Some(Tracked(drops.clone()));Ok(())}).unwrap();
    let other=registry.reserve("second","detached").unwrap();
    other.with_owned("detached",|value|{*value=Some(Tracked(drops.clone()));Ok(())}).unwrap();
    registry.release_window("main").unwrap();assert_eq!(drops.load(Ordering::SeqCst),1);
    assert!(registry.get("second").unwrap().is_some());
    assert!(old.with_owned("main",|_|->Result<(),String>{panic!("Released handle must stay retired")}).is_err());
    let replacement=registry.reserve("first","main").unwrap();
    replacement.with_owned("main",|value|{assert!(value.is_none());*value=Some(Tracked(drops.clone()));Ok(())}).unwrap();
    assert!(old.with_owned("main",|_|->Result<(),String>{panic!("Old handle must not edit replacement")}).is_err());
    registry.release_window("main").unwrap();assert_eq!(drops.load(Ordering::SeqCst),2);
    registry.release_window("main").unwrap();assert_eq!(drops.load(Ordering::SeqCst),2);
    registry.release("second","detached").unwrap();assert_eq!(drops.load(Ordering::SeqCst),3);
}

#[test]
fn window_release_never_waits_for_another_windows_blocked_session(){
    let registry=Arc::new(SessionRegistry::<FakeEditor>::new(3));
    let blocked=registry.reserve("blocked","other").unwrap();registry.reserve("own","main").unwrap();
    let(started_tx,started_rx)=mpsc::channel();let(unblock_tx,unblock_rx)=mpsc::channel();
    let worker=thread::spawn(move||blocked.with_owned("other",|_|{started_tx.send(()).unwrap();unblock_rx.recv().unwrap();Ok(())}));
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let(done_tx,done_rx)=mpsc::channel();let closing=registry.clone();
    let release=thread::spawn(move||{done_tx.send(closing.release_window("main")).unwrap();});
    let result=done_rx.recv_timeout(Duration::from_secs(2));
    unblock_tx.send(()).unwrap();worker.join().unwrap().unwrap();release.join().unwrap();
    result.unwrap().unwrap();assert!(registry.get("own").unwrap().is_none());
    assert!(registry.get("blocked").unwrap().is_some());
}

#[test]
fn capacity_and_failed_editor_work_do_not_poison_the_registry(){
    let registry=SessionRegistry::<FakeEditor>::new(2);
    let first=registry.reserve("first","main").unwrap();
    registry.reserve("second","other").unwrap();assert!(registry.reserve("third","main").is_err());
    assert!(first.with_owned("main",|_|Err::<(),_>("Synthetic conversion refusal".into())).is_err());
    first.with_owned("main",|value|{assert!(value.is_none());*value=Some(fake());Ok(())}).unwrap();
    registry.release("first","main").unwrap();registry.reserve("third","main").unwrap();
}

#[test]
fn exact_resume_keeps_handle_but_replacement_retires_delayed_requests_atomically(){
    let registry=SessionRegistry::new(2);let original=registry.reserve("tab","main").unwrap();
    original.with_owned("main",|value|{*value=Some(fake());Ok(())}).unwrap();
    registry.update("tab","main",&original,|value|{assert_eq!(value.as_ref().unwrap(),&fake());Ok(SessionUpdate::Keep(()))}).unwrap();
    original.with_owned("main",|_|Ok(())).unwrap();
    assert!(registry.update("tab","main",&original,|_|Err::<SessionUpdate<FakeEditor,()>,_>("Synthetic replacement failure".into())).is_err());
    original.with_owned("main",|value|{assert_eq!(value.as_ref().unwrap(),&fake());Ok(())}).unwrap();
    let replacement=FakeEditor{text:"Changed acknowledged source 世界".into(),undo:vec![],caret:0};
    registry.update("tab","main",&original,|_|Ok(SessionUpdate::Replace(replacement,()))).unwrap();
    assert!(original.with_owned("main",|_|->Result<(),String>{panic!("Delayed old request must not edit replacement")}).is_err());
    registry.get("tab").unwrap().unwrap().with_owned("main",|value|{assert_eq!(value.as_ref().unwrap().text,"Changed acknowledged source 世界");Ok(())}).unwrap();
}

#[test]
fn transfer_waiting_on_reinitialization_owns_the_replacement_not_a_retired_handle(){
    let registry=Arc::new(SessionRegistry::new(3));let old=registry.reserve("tab","main").unwrap();
    old.with_owned("main",|value|{*value=Some(fake());Ok(())}).unwrap();
    let(started_tx,started_rx)=mpsc::channel();let(release_tx,release_rx)=mpsc::channel();
    let updating=registry.clone();let replacement=thread::spawn(move||updating.update("tab","main",&old,|_|{
        started_tx.send(()).unwrap();release_rx.recv().unwrap();Ok(SessionUpdate::Replace(FakeEditor{text:"Refreshed 中文 source".into(),undo:vec![],caret:0},()))
    }));
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let transferring=registry.clone();let(done_tx,done_rx)=mpsc::channel();
    let transfer=thread::spawn(move||{done_tx.send(transferring.transfer("tab","main","detached")).unwrap();});
    assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
    registry.reserve("independent","another-window").unwrap();
    release_tx.send(()).unwrap();replacement.join().unwrap().unwrap();
    done_rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();transfer.join().unwrap();
    registry.get("tab").unwrap().unwrap().with_owned("detached",|value|{assert_eq!(value.as_ref().unwrap().text,"Refreshed 中文 source");Ok(())}).unwrap();
    registry.release_window("main").unwrap();assert!(registry.get("tab").unwrap().is_some());
    registry.release_window("detached").unwrap();assert!(registry.get("tab").unwrap().is_none());
}
