use super::*;

fn form(a:&Workspace)->Rc<ChapterPage>{
    let widget=a.current_widget().unwrap();
    a.chapter_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==&widget).unwrap().clone()
}
fn open(a:&Rc<Workspace>,id:&str)->Rc<ChapterPage>{
    let before=a.current_widget();
    a.tree.selection().select_path(&outline_path(a,id));
    find_button(&a.desk.sidebar,"Edit chapter").unwrap().clicked();
    wait(||a.current_widget()!=before && a.chapter_pages.borrow().iter().any(|p|p.loaded.get() && a.current_widget()==Some(p.widget.clone().upcast())));form(a)
}
fn save_form(a:&Rc<Workspace>,p:&Rc<ChapterPage>)->Result<(),String>{
    let result=Rc::new(RefCell::new(None));let out=result.clone();
    a.save_current(move|r|*out.borrow_mut()=Some(r));wait(||result.borrow().is_some() && !p.saving.get());
    let value=result.borrow_mut().take().unwrap();value
}
pub(super) fn run(binaries:&Path){
    let fixture=tempfile::tempdir().unwrap();let data=fixture.path().join("chapter-data");let cache=fixture.path().join("chapter-cache");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(&cache).unwrap();
    let companion=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};let client=Process::start(&binaries.join(companion),&data,true).unwrap();
    let project=rpc(&client,"create_novel",json!({"title":"Chapter arrangement"}));let pid=project["project"]["id"].as_str().unwrap();let first_id=project["scene"]["id"].as_str().unwrap();
    let first=rpc(&client,"open_project_index",json!({"projectId":pid}))["chapters"][0]["id"].as_str().unwrap().to_owned();
    let second=rpc(&client,"create_chapter",json!({"projectId":pid,"title":"Second chapter"}))["id"].as_str().unwrap().to_owned();
    let third=rpc(&client,"create_chapter",json!({"projectId":pid,"title":"Third chapter"}))["id"].as_str().unwrap().to_owned();
    let a=Workspace::build(client,&cache,binaries,false,false).unwrap();wait(||a.active.borrow().as_ref().is_some_and(|p|p.open.get() && p.pending.get()==0));
    let index=||rpc(&a.client,"open_project_index",json!({"projectId":pid}));
    let scene=||rpc(&a.client,"load_scene",json!({"sceneId":first_id}));
    let manuscript=a.active.borrow().clone().unwrap();
    // Opening the form respects the ordinary unsaved manuscript decisions.
    edit(&manuscript,"Keep this writing. مِ 中文 𠀀 👩🏽‍🚀");let document=manuscript.frame.borrow()["document"].clone();
    a.tree.selection().select_path(&outline_path(&a,&second));select(&a,SaveChoice::Cancel);find_button(&a.desk.sidebar,"Edit chapter").unwrap().clicked();
    assert_eq!(a.current_widget(),Some(manuscript.widget.clone().upcast()));assert!(manuscript.dirty.get());assert!(a.chapter_pages.borrow().is_empty());
    select(&a,SaveChoice::Save);let p=open(&a,&second);assert_eq!(scene()["scene"]["document"],document);let saved_scene=scene();
    assert_eq!(p.number.value_as_int(),2);assert_eq!(p.title.text(),"Second chapter");
    let before=index();p.title.set_text("مقدمة / 序章");p.number.set_value(0.);
    pump(Duration::from_millis(350));assert_eq!(index(),before);assert!(p.dirty.get());
    select(&a,SaveChoice::Cancel);a.open_book();assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));assert_eq!(p.title.text(),"مقدمة / 序章");assert_eq!(index(),before);
    capture(&a,"notebook-edit-chapter.png");find_button(&p.widget,"Save chapter").unwrap().clicked();wait(||!p.saving.get() && !p.dirty.get());
    assert_eq!(index()["chapterNumberStart"],0);assert_eq!(index()["chapters"][0]["id"],second);assert_eq!(index()["chapters"][0]["title"],"مقدمة / 序章");assert_eq!(scene(),saved_scene);
    // The saved form can be edited again with freshly captured heads.
    p.number.set_value(1.);save_form(&a,&p).unwrap();assert_eq!(index()["chapterNumberStart"],1);assert_eq!(index()["chapters"][0]["id"],second);
    p.number.set_value(3.);save_form(&a,&p).unwrap();assert_eq!(index()["chapters"][2]["id"],second);assert_eq!(scene(),saved_scene);
    // Invalid fields never commit, and leaving with Discard restores the saved values.
    let marker=rpc(&a.client,"sync_cursor",json!({}));p.title.set_text(" ");assert!(save_form(&a,&p).unwrap_err().contains("title"));assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);
    p.discard();p.number.set_text("1.5");assert!(save_form(&a,&p).is_err());assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);p.discard();
    p.title.set_text("Cancelled metadata");select(&a,SaveChoice::Discard);a.open_book();assert!(!p.dirty.get());assert_eq!(index()["chapters"][2]["title"],"مقدمة / 序章");assert!(a.chapter_pages.borrow().is_empty());
    // The old Rename action routes chapter edits through the checked form too.
    a.tree.selection().select_path(&outline_path(&a,&second));find_button(&a.desk.sidebar,"Rename selected item").unwrap().clicked();let stale=form(&a);wait(||stale.loaded.get());stale.title.set_text("Old window pending title");
    let child=a.spawn_window();let newer=open(&child,&second);newer.title.set_text("Newer title مِ 中文");save_form(&child,&newer).unwrap();
    let marker=rpc(&a.client,"sync_cursor",json!({}));assert!(save_form(&a,&stale).unwrap_err().contains("Conflict"));assert!(stale.dirty.get());assert_eq!(stale.title.text(),"Old window pending title");assert_eq!(index()["chapters"][2]["title"],"Newer title مِ 中文");assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);
    select(&a,SaveChoice::Save);a.close();wait(||!a.closing.get());assert!(a.window.is_visible());assert_eq!(a.current_widget(),Some(stale.widget.clone().upcast()));
    // A separate window reorders the manuscript while this form remains open.
    select(&a,SaveChoice::Discard);let stale=open(&a,&second);stale.title.set_text("Stale order edit");let reorder=open(&child,&third);reorder.number.set_value(1.);save_form(&child,&reorder).unwrap();
    let before=index();assert!(save_form(&a,&stale).unwrap_err().contains("Conflict"));assert_eq!(index(),before);assert_eq!(stale.title.text(),"Stale order edit");
    // Numbering-only changes are independently checked even if metadata heads agree.
    select(&a,SaveChoice::Discard);let stale=open(&a,&second);stale.title.set_text("Stale numbering edit");let captured=rpc(&a.client,"chapter_edit_snapshot",json!({"projectId":pid,"chapterId":second}));
    rpc(&child.client,"save_project_data",json!({"projectId":pid,"kind":"chapterNumbering","payload":{"schemaVersion":1,"start":0},"parent":captured["numberingParent"]}));
    let before=index();assert!(save_form(&a,&stale).unwrap_err().contains("numbering"));assert_eq!(index(),before);
    // Scene selections choose their containing chapter; dirty forms block navigation/composition.
    select(&a,SaveChoice::Discard);let p=open(&a,first_id);assert_eq!(p.title.text(),"Chapter 1");p.title.set_text("Detached chapter title مِ 中文");
    p.composing.set(true);let count=windows();a.detach_current();assert_eq!(windows(),count);select(&a,SaveChoice::Discard);a.open_book();assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));a.desk.choices.borrow_mut().clear();p.composing.set(false);
    a.detach_current();let detached=last_window();assert!(Rc::ptr_eq(&form(&detached),&p));assert!(!a.chapter_pages.borrow().iter().any(|f|Rc::ptr_eq(f,&p)));
    let old=a.current_widget();find_button(&p.widget,"Save chapter").unwrap().clicked();wait(||!p.saving.get() && !p.dirty.get());assert_eq!(a.current_widget(),old);assert_eq!(index()["chapters"].as_array().unwrap().iter().find(|c|c["id"]==first).unwrap()["title"],"Detached chapter title مِ 中文");
    a.close();wait(||windows()==2);p.title.set_text("Saved after original closes");save_form(&detached,&p).unwrap();assert_eq!(rpc(&detached.client,"load_scene",json!({"sceneId":first_id})),saved_scene);
    detached.close();wait(||windows()==1);child.close();wait(||windows()==0);
}
