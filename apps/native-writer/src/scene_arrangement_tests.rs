use super::*;
fn form(a:&Workspace)->Rc<SceneArrangementPage>{let widget=a.current_widget().unwrap();a.scene_arrangement_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==&widget).unwrap().clone()}
fn open(a:&Rc<Workspace>,sid:&str)->Rc<SceneArrangementPage>{
    let before=a.current_widget();a.tree.selection().select_path(&outline_path(a,sid));find_button(&a.desk.sidebar,"Arrange scene").unwrap().clicked();
    wait(||a.current_widget()!=before&&a.scene_arrangement_pages.borrow().iter().any(|p|p.loaded.get()&&a.current_widget()==Some(p.widget.clone().upcast())));form(a)
}
fn save_form(a:&Rc<Workspace>,p:&Rc<SceneArrangementPage>)->Result<(),String>{let result=Rc::new(RefCell::new(None));let out=result.clone();a.save_current(move|r|*out.borrow_mut()=Some(r));wait(||result.borrow().is_some()&&!p.saving.get());let value=result.borrow_mut().take().unwrap();value}
pub(super) fn run(binaries:&Path){
    let fixture=tempfile::tempdir().unwrap();let data=fixture.path().join("arrangement-data");let cache=fixture.path().join("arrangement-cache");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(&cache).unwrap();
    let companion=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};let client=Process::start(&binaries.join(companion),&data,true).unwrap();
    let project=rpc(&client,"create_novel",json!({"title":"Scene arrangement"}));let pid=project["project"]["id"].as_str().unwrap();let first_id=project["scene"]["id"].as_str().unwrap();let source=project["scene"]["chapterId"].as_str().unwrap();
    let target=rpc(&client,"create_chapter",json!({"projectId":pid,"title":"Target مِ 中文"}))["id"].as_str().unwrap().to_owned();
    let second=rpc(&client,"create_scene",json!({"chapterId":source,"scene":{"schemaVersion":1,"title":"Second مِ 中文","document":{"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"مِ 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"},{"type":"underline"}]}]}]}}}));let second_id=second["id"].as_str().unwrap();
    let a=Workspace::build(client,&cache,binaries,false,false).unwrap();wait(||a.active.borrow().as_ref().is_some_and(|p|p.open.get()&&p.pending.get()==0));
    let index=||rpc(&a.client,"open_project_index",json!({"projectId":pid}));let saved_second=rpc(&a.client,"load_scene",json!({"sceneId":second_id}));
    // A dirty manuscript receives the normal Save / Discard / Cancel decisions.
    let manuscript=a.active.borrow().clone().unwrap();edit(&manuscript,"Kept writing مِ 中文 𠀀 👩🏽‍🚀");let document=manuscript.frame.borrow()["document"].clone();
    a.tree.selection().select_path(&outline_path(&a,second_id));select(&a,SaveChoice::Cancel);find_button(&a.desk.sidebar,"Arrange scene").unwrap().clicked();assert_eq!(a.current_widget(),Some(manuscript.widget.clone().upcast()));assert!(a.scene_arrangement_pages.borrow().is_empty());
    select(&a,SaveChoice::Save);let p=open(&a,second_id);assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":first_id}))["scene"]["document"],document);
    let saved_first=rpc(&a.client,"load_scene",json!({"sceneId":first_id}));assert_eq!(p.position.value_as_int(),2);assert_eq!(p.chapter.active_id().unwrap().as_str(),source);
    let before=index();p.position.set_value(1.);pump(Duration::from_millis(250));assert_eq!(index(),before);select(&a,SaveChoice::Cancel);a.open_book();assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));assert!(p.dirty.get());
    capture(&a,"notebook-arrange-scene.png");find_button(&p.widget,"Save arrangement").unwrap().clicked();wait(||!p.saving.get()&&!p.dirty.get());assert_eq!(index()["scenes"][0]["id"],second_id);assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":second_id})),saved_second);
    p.chapter.set_active_id(Some(&target));assert_eq!(p.position.value_as_int(),1);save_form(&a,&p).unwrap();let moved=index()["scenes"].as_array().unwrap().iter().find(|s|s["id"]==second_id).unwrap().clone();assert_eq!(moved["chapterId"],target);assert_eq!(moved["position"],0);
    let marker=rpc(&a.client,"sync_cursor",json!({}));p.position.set_text("1.5");assert!(save_form(&a,&p).is_err());assert!(p.dirty.get());assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);p.discard();
    p.chapter.set_active_id(Some(source));select(&a,SaveChoice::Discard);a.open_book();assert!(!p.dirty.get());assert!(a.scene_arrangement_pages.borrow().is_empty());assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":first_id})),saved_first);
    // Another window changes the order; the old form retains its unsaved decision.
    let stale=open(&a,first_id);stale.chapter.set_active_id(Some(&target));let child=a.spawn_window();let fresh=open(&child,second_id);fresh.chapter.set_active_id(Some(source));fresh.position.set_value(1.);save_form(&child,&fresh).unwrap();a.load_index();wait(||a.index.borrow()["scenes"][0]["id"]==second_id);
    let before=index();let marker=rpc(&a.client,"sync_cursor",json!({}));assert!(save_form(&a,&stale).unwrap_err().contains("Conflict"));assert!(stale.dirty.get());assert_eq!(stale.chapter.active_id().unwrap().as_str(),target);assert_eq!(index(),before);assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);
    select(&a,SaveChoice::Save);a.close();wait(||!a.closing.get());assert!(a.window.is_visible());assert_eq!(a.current_widget(),Some(stale.widget.clone().upcast()));
    // A form moved to another window resolves its owner at the time of Save.
    select(&a,SaveChoice::Discard);let p=open(&a,second_id);p.chapter.set_active_id(Some(&target));p.composing.set(true);let count=windows();a.detach_current();assert_eq!(windows(),count);p.composing.set(false);a.detach_current();let detached=last_window();assert!(Rc::ptr_eq(&form(&detached),&p));assert!(!a.scene_arrangement_pages.borrow().iter().any(|f|Rc::ptr_eq(f,&p)));
    let old=a.current_widget();find_button(&p.widget,"Save arrangement").unwrap().clicked();wait(||!p.saving.get()&&!p.dirty.get());assert_eq!(a.current_widget(),old);assert_eq!(rpc(&detached.client,"load_scene",json!({"sceneId":second_id})),saved_second);
    a.close();wait(||windows()==2);p.chapter.set_active_id(Some(source));save_form(&detached,&p).unwrap();assert_eq!(rpc(&detached.client,"load_scene",json!({"sceneId":first_id})),saved_first);assert_eq!(rpc(&detached.client,"load_scene",json!({"sceneId":second_id})),saved_second);
    detached.close();wait(||windows()==1);child.close();wait(||windows()==0);
}
