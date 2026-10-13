use super::*;
fn fixture()->Value {json!({"scenes":1,"unicodeScalars":15,"graphemes":10,"whitespaceWords":5,"cjkCharacters":3,"method":METHOD})}
#[test]
fn response_keeps_unicode_units_distinct_and_definitions_explain_saved_scope() {
    let s=checked(fixture()).unwrap();assert_eq!(measures(&s),"Saved scenes: 1\nUnicode scalars: 15\nGrapheme clusters: 10\nWhitespace tokens: 5\nCJK characters: 3");
    assert!(DEFINITIONS.contains("Unsaved editor and form drafts are excluded"));assert!(DEFINITIONS.contains("U+20000–323AF"));assert!(DEFINITIONS.contains("not a language-aware word count"));
}
#[test]
fn malformed_or_incoherent_counts_are_not_rendered_as_current_statistics() {
    for (key,value) in [("scenes",json!(-1)),("unicodeScalars",json!(true)),("graphemes",json!(16)),("whitespaceWords",json!("five")),("cjkCharacters",json!(16)),("method",json!("Guessed words")),("scenes",json!(0)),("unicodeScalars",json!(64*1024*1024+1))] {let mut v=fixture();v[key]=value;assert!(checked(v).is_err());}
    let mut v=fixture();v.as_object_mut().unwrap().remove("graphemes");assert!(checked(v).is_err());
}
fn wait(mut predicate:impl FnMut()->bool) {let start=std::time::Instant::now();while !predicate(){while glib::MainContext::default().pending(){glib::MainContext::default().iteration(false);}assert!(start.elapsed()<std::time::Duration::from_secs(20),"Statistics fixture timed out");std::thread::sleep(std::time::Duration::from_millis(3));}}
fn rpc(client:&Process,op:&str,args:Value)->Value {let result=Rc::new(RefCell::new(None));let out=result.clone();client.rpc(op,args,move|r|*out.borrow_mut()=Some(r));wait(||result.borrow().is_some());let r=result.borrow_mut().take().unwrap().unwrap();r}
fn descendants<T:glib::types::StaticType+IsA<gtk::Widget>+Clone+glib::object::ObjectType>(widget:&impl IsA<gtk::Widget>)->Vec<T> {
    let mut found=Vec::new();if let Ok(w)=widget.as_ref().clone().downcast::<T>(){found.push(w);}
    if let Ok(c)=widget.as_ref().clone().downcast::<gtk::Container>(){for child in c.children(){found.extend(descendants::<T>(&child));}}found
}
fn refresh(widget:&gtk::Widget)->gtk::Button {descendants::<gtk::Button>(widget).into_iter().find(|b|b.label().as_deref()==Some("Refresh saved statistics")).unwrap()}
fn label_contains(widget:&gtk::Widget,text:&str)->bool {descendants::<gtk::Label>(widget).into_iter().any(|l|l.text().contains(text))}
fn count_label(widget:&gtk::Widget)->gtk::Label {descendants::<gtk::Label>(widget).into_iter().find(|l|l.text().starts_with("Saved scenes:")).unwrap()}
fn scene(text:&str)->Value {json!({"schemaVersion":1,"title":"Title excluded 中文","document":{"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":text,"marks":[{"type":"bold"},{"type":"underline"}]}]}]}})}
fn transfer(from:&Process,to:&Process,pid:&str) {let remote=rpc(from,"sync_cursor",json!({}));let local=rpc(to,"sync_cursor",json!({}));let revisions=rpc(from,"pull_revisions",json!({"projectId":pid,"cursor":{"epoch":remote["epoch"],"sequence":0},"limit":100}));rpc(to,"ingest_revisions",json!({"projectId":pid,"expectedEpoch":local["epoch"],"remoteEpoch":remote["epoch"],"manifest":rpc(from,"open_project",json!({"projectId":pid})),"revisions":revisions["revisions"]}));}

/// Parent runs this on its sole initialized opt-in GTK thread.
pub(crate) fn run(binaries:&Path) {
    let d=tempfile::tempdir().unwrap();let data=d.path().join("data");let cache=d.path().join("cache");let peer_data=d.path().join("peer");for path in [&data,&cache,&peer_data]{std::fs::create_dir_all(path).unwrap();}
    let name=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};let client=Process::start(&binaries.join(name),&data,true).unwrap();
    let novel=rpc(&client,"create_novel",json!({"title":"Statistics fixture"}));let pid=novel["project"]["id"].as_str().unwrap();let sid=novel["scene"]["id"].as_str().unwrap();
    let saved=rpc(&client,"save_scene",json!({"sceneId":sid,"scene":scene("e\u{301} مِ 中文 𠀀 👩🏽‍🚀"),"parent":novel["scene"]["revisionId"]}));
    let peer=Process::start(&binaries.join(name),&peer_data,true).unwrap();transfer(&client,&peer,pid);
    let a=Workspace::build(client,&cache,binaries,false,false).unwrap();wait(||a.active.borrow().as_ref().is_some_and(|p|p.open.get()&&p.pending.get()==0));
    let pane=a.active.borrow().clone().unwrap();pane.operation(json!({"type":"text","text":"Unsaved draft 中文"}),true);wait(||pane.pending.get()==0&&pane.dirty.get());
    let draft=pane.frame.borrow()["document"].clone();let parent=pane.parent.borrow().clone();let marker=rpc(&a.client,"sync_cursor",json!({}));
    a.desk.choices.borrow_mut().push_back(desk::SaveChoice::Cancel);
    a.manuscript_statistics();let widget=a.current_widget().unwrap();wait(||label_contains(&widget,"Unicode scalars: 15"));
    assert_eq!(a.desk.choices.borrow().len(),1);a.desk.choices.borrow_mut().clear();assert!(a.pages.page_num(&pane.widget).is_some());assert_eq!(pane.frame.borrow()["document"],draft);assert_eq!(*pane.parent.borrow(),parent);assert!(pane.dirty.get());
    assert!(label_contains(&widget,"Grapheme clusters: 10"));assert!(label_contains(&widget,"Whitespace tokens: 5"));assert!(label_contains(&widget,"CJK characters: 3"));assert!(label_contains(&widget,"Unsaved editor and form drafts are excluded"));
    let counts=count_label(&widget);let button=refresh(&widget);button.clicked();assert!(counts.text().is_empty());wait(||label_contains(&widget,"Unicode scalars: 15"));
    assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":sid})),saved);
    // Another request supersedes an already-delivered snapshot before rendering.
    let retried=Rc::new(Cell::new(false));let out=retried.clone();let b=button.clone();let c=counts.clone();
    BEFORE_REPLY.with(|h|*h.borrow_mut()=Some(Box::new(move||{b.clicked();assert!(c.text().is_empty());out.set(true);})));button.clicked();wait(||retried.get()&&label_contains(&widget,"Unicode scalars: 15"));
    let newer=rpc(&a.client,"save_scene",json!({"sceneId":sid,"scene":scene("A 中"),"parent":saved["revisionId"]}));
    button.clicked();wait(||label_contains(&widget,"Unicode scalars: 3"));assert_eq!(pane.frame.borrow()["document"],draft);assert_eq!(*pane.parent.borrow(),parent);assert!(pane.dirty.get());
    rpc(&peer,"save_scene",json!({"sceneId":sid,"scene":scene("Other branch 中文"),"parent":saved["revisionId"]}));transfer(&peer,&a.client,pid);let marker=rpc(&a.client,"sync_cursor",json!({}));
    button.clicked();wait(||label_contains(&widget,"Saved statistics unavailable"));assert!(counts.text().is_empty());assert!(label_contains(&widget,"Conflict"));assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);
    let current=rpc(&a.client,"load_scene",json!({"sceneId":sid}));let heads=rpc(&a.client,"scene_heads",json!({"sceneId":sid}));let conflicts:Vec<Value>=heads["revisionIds"].as_array().unwrap().iter().filter(|id|**id!=current["revisionId"]).cloned().collect();
    rpc(&a.client,"resolve_scene",json!({"sceneId":sid,"scene":newer["scene"],"parent":current["revisionId"],"conflictIds":conflicts}));button.clicked();wait(||label_contains(&widget,"Unicode scalars: 3"));
    let marker=rpc(&a.client,"sync_cursor",json!({}));let stored=rpc(&a.client,"load_scene",json!({"sceneId":sid}));
    a.detach_current();let detached=desk::workspace_for_widget(&widget).unwrap();assert!(!Rc::ptr_eq(&a,&detached));
    a.desk.choices.borrow_mut().push_back(desk::SaveChoice::Discard);a.close();wait(||a.desk.is_closed());
    refresh(&widget).clicked();wait(||label_contains(&widget,"Unicode scalars: 3"));assert_eq!(rpc(&detached.client,"sync_cursor",json!({})),marker);
    // Closing the current owner while a response is in flight leaves no page
    // remount or stale count render, and does not write a manuscript revision.
    let closed=detached.clone();BEFORE_REPLY.with(|h|*h.borrow_mut()=Some(Box::new(move||closed.close())));refresh(&widget).clicked();wait(||detached.desk.is_closed());assert!(counts.text().is_empty());
    let probe=Process::start(&binaries.join(name),&data,true).unwrap();assert_eq!(rpc(&probe,"sync_cursor",json!({})),marker);assert_eq!(rpc(&probe,"load_scene",json!({"sceneId":sid})),stored);probe.stop();peer.stop();
}
