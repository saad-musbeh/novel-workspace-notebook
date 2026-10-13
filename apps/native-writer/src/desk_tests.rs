#[path="peek_tests.rs"]
mod peek_forms;
#[path="knowledge_tests.rs"]
mod knowledge_forms;
#[path="navigation_tests.rs"]
mod exact_navigation;
#[path="dictionary_ui_tests.rs"]
mod dictionary_forms;
#[path="projection_tests.rs"]
mod projection_forms;
#[path="board_tests.rs"]
mod board_forms;
#[path="shortcut_tests.rs"]
mod shortcut_forms;
#[path="desktop_preferences_tests.rs"]
mod desktop_preference_forms;
#[path="scene_arrangement_tests.rs"]
mod scene_arrangement_forms;
#[path="notebook_tests.rs"]
mod notebook_features;
#[path="chapter_tests.rs"]
mod chapter_forms;
#[path="alternatives_tests.rs"]
mod paragraph_forms;
#[path="data_conflicts_tests.rs"]
mod project_data_conflicts;
use super::*;
use std::time::Instant;
fn wait(condition:impl Fn()->bool) {
    let start=Instant::now();let context=glib::MainContext::default();
    while !condition(){while context.pending(){context.iteration(false);}if start.elapsed()>=Duration::from_secs(20){WINDOWS.with(|windows|for a in windows.borrow().iter(){eprintln!("UI timeout: status={} busy={} panes={}",a.status.text(),a.desk.busy.get(),a.panes.borrow().len());for p in a.panes.borrow().values(){eprintln!("pane pending={} open={} saving={} dirty={} allocated={}x{} frame={}x{}",p.pending.get(),p.open.get(),p.saving.get(),p.dirty.get(),p.area.allocated_width(),p.area.allocated_height(),p.frame.borrow()["width"],p.frame.borrow()["height"]);}});panic!("Native UI operation timed out");}std::thread::sleep(Duration::from_millis(8));}
    while context.pending(){context.iteration(false);}
}
fn pump(duration:Duration) {let start=Instant::now();wait(||start.elapsed()>=duration);}
fn rpc(process:&Process,op:&str,args:Value)->Value {
    let result=Rc::new(RefCell::new(None));let out=result.clone();process.rpc(op,args,move|r|*out.borrow_mut()=Some(r));wait(||result.borrow().is_some());let value=result.borrow_mut().take().unwrap().unwrap();value
}
fn find_button(widget:&impl IsA<gtk::Widget>,name:&str)->Option<gtk::Button>{
    if let Ok(button)=widget.as_ref().clone().downcast::<gtk::Button>(){if button.label().as_deref()==Some(name){return Some(button);}}
    if let Ok(container)=widget.as_ref().clone().downcast::<gtk::Container>(){for child in container.children(){if let Some(button)=find_button(&child,name){return Some(button);}}}None
}
fn has_label(widget:&impl IsA<gtk::Widget>,text:&str)->bool{
    if let Ok(label)=widget.as_ref().clone().downcast::<gtk::Label>(){if label.text()==text{return true;}}
    widget.as_ref().clone().downcast::<gtk::Container>().ok().is_some_and(|c|c.children().iter().any(|w|has_label(w,text)))
}
fn find_entry(widget:&impl IsA<gtk::Widget>)->Option<gtk::Entry>{
    if let Ok(entry)=widget.as_ref().clone().downcast::<gtk::Entry>(){return Some(entry);}
    if let Ok(container)=widget.as_ref().clone().downcast::<gtk::Container>(){for child in container.children(){if let Some(entry)=find_entry(&child){return Some(entry);}}}None
}
fn answer_new_novel(title:&str,response:gtk::ResponseType){
    let title=title.to_owned();glib::timeout_add_local_once(Duration::from_millis(40),move||{
        let dialog=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Dialog>().ok()).find(|d|d.is_visible() && d.title().as_deref()==Some("New novel")).expect("New novel should request its name");
        assert!(find_button(&dialog,"Create").is_some());find_entry(&dialog).unwrap().set_text(&title);dialog.response(response);
    });
}
fn save(a:&Rc<Workspace>,p:&Rc<EditorPane>) {
    let done=Rc::new(Cell::new(false));let out=done.clone();a.save_pane(p.clone(),move|r|{r.unwrap();out.set(true);});wait(||done.get());
}
fn edit(p:&Rc<EditorPane>,text:&str) {p.operation(json!({"type":"text","text":text}),true);wait(||p.pending.get()==0 && p.dirty.get());}
fn select(a:&Workspace,choice:SaveChoice) {a.desk.choices.borrow_mut().push_back(choice);}
fn last_window()->Rc<Workspace> {WINDOWS.with(|windows|windows.borrow().last().unwrap().clone())}
fn windows()->usize {WINDOWS.with(|windows|windows.borrow().len())}
fn capture(a:&Workspace,name:&str) {
    eprintln!("Capturing synthetic UI: {name}");
    pump(Duration::from_millis(120));a.window.check_resize();let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../work").join(name);
    let surface=cairo::ImageSurface::create(cairo::Format::ARgb32,a.window.allocated_width(),a.window.allocated_height()).unwrap();let context=cairo::Context::new(&surface).unwrap();a.window.draw(&context);surface.write_to_png(&mut std::fs::File::create(path).unwrap()).unwrap();
}
fn assert_paper_contrast(name:&str,context:&gtk::StyleContext,state:gtk::StateFlags) {
    let background=context.style_property_for_state("background-color",state).get::<gdk::RGBA>().unwrap();
    let foreground=context.color(state);
    let channel=|x:f64|if x<=0.04045{x/12.92}else{((x+0.055)/1.055).powf(2.4)};
    let luminance=|c:&gdk::RGBA|0.2126*channel(c.red())+0.7152*channel(c.green())+0.0722*channel(c.blue());
    let bg=luminance(&background);let fg=luminance(&foreground);
    assert!(background.alpha()>0.99 && bg>0.75,"{name} must have a light opaque background, got {background:?}");
    assert!((bg.max(fg)+0.05)/(bg.min(fg)+0.05)>=4.5,"{name} text must remain readable: {foreground:?} on {background:?}");
}
fn child_style(parent:&gtk::StyleContext,name:&str)->gtk::StyleContext {
    let path=parent.path().unwrap();let position=path.append_type(gtk::Widget::static_type());path.iter_set_object_name(position,Some(name));
    let context=gtk::StyleContext::new();context.set_path(&path);context.set_parent(Some(parent));context
}
fn notebook_surfaces_remain_light() {
    let settings=gtk::Settings::default().unwrap();let theme=settings.gtk_theme_name();let dark=settings.is_gtk_application_prefer_dark_theme();
    apply_style();
    for (theme_name,prefer_dark) in [("Breeze-Dark",false),("Adwaita",true),("Breeze",false)]{
        settings.set_gtk_theme_name(Some(theme_name));settings.set_gtk_application_prefer_dark_theme(prefer_dark);
        let menu=gtk::Menu::new();let item=gtk::MenuItem::with_label("Planning");menu.append(&item);
        let notebook=gtk::Notebook::new();let text=gtk::TextView::new();let spin=gtk::SpinButton::with_range(0.,10.,1.);let popover=gtk::Popover::new(None::<&gtk::Widget>);
        let list=gtk::ListBox::new();let row=gtk::ListBoxRow::new();list.add(&row);
        let text_style=child_style(&text.style_context(),"text");let stack=child_style(&notebook.style_context(),"stack");
        for (name,context) in [("dropdown",menu.style_context()),("dropdown item",item.style_context()),("tab content",stack),("text area",text_style.clone()),("number field",spin.style_context()),("popover",popover.style_context()),("list row",row.style_context())]{
            for state in [gtk::StateFlags::NORMAL,gtk::StateFlags::BACKDROP]{assert_paper_contrast(&format!("{theme_name}: {name}"),&context,state);}
        }
        assert_paper_contrast("highlighted dropdown item",&item.style_context(),gtk::StateFlags::PRELIGHT);
        assert_paper_contrast("selected list row",&row.style_context(),gtk::StateFlags::SELECTED);
        let selection=child_style(&text_style,"selection");assert_paper_contrast("selected text",&selection,gtk::StateFlags::SELECTED);
    }
    settings.set_gtk_theme_name(theme.as_deref());settings.set_gtk_application_prefer_dark_theme(dark);
}
fn outline_path(a:&Workspace,id:&str)->gtk::TreePath{
    let mut found=None;a.outline.foreach(|model,path,iter|{
        if model.value(iter,1).get::<String>().ok().as_deref()==Some(id){found=Some(path.clone());true}else{false}
    });found.expect("Chapter or scene should be visible in the outline")
}
fn chapter_writing(a:&Rc<Workspace>,project:&Value,first:&Value,chapter:&Value){
    let prologue=rpc(&a.client,"create_chapter",json!({"projectId":project["id"],"title":"Prologue"}));
    let appendix=rpc(&a.client,"create_chapter",json!({"projectId":project["id"],"title":"Appendix"}));
    let afterword=rpc(&a.client,"create_chapter",json!({"projectId":project["id"],"title":"Afterword"}));
    a.load_index();wait(||a.index.borrow()["chapters"].as_array().unwrap().len()==4);
    let first_id=first["id"].as_str().unwrap();let prologue_id=prologue["id"].as_str().unwrap();let appendix_id=appendix["id"].as_str().unwrap();let afterword_id=afterword["id"].as_str().unwrap();
    let index=||rpc(&a.client,"open_project_index",json!({"projectId":project["id"]}));
    let count=|chapter:&str|index()["scenes"].as_array().unwrap().iter().filter(|s|s["chapterId"]==chapter).count();
    // Empty title matches remain searchable, and opening a chapter does not create text.
    a.search.set_text("PROLOGUE");a.populate_outline();assert_eq!(a.outline.iter_n_children(None),1);outline_path(a,prologue_id);
    a.search.set_text("");a.populate_outline();let column=a.tree.column(0).unwrap();
    a.tree.row_activated(&outline_path(a,prologue_id),&column);
    let page=a.current_widget().unwrap();assert!(has_label(&page,"Prologue"));assert_eq!(count(prologue_id),0);assert_eq!(a.pages.n_pages(),1);assert_eq!(windows(),1);
    capture(a,"notebook-empty-chapter.png");
    // The action owns the opened chapter, regardless of the later sidebar selection.
    a.tree.selection().select_path(&outline_path(a,chapter["id"].as_str().unwrap()));
    let start=find_button(&page,"Start writing").unwrap();start.clicked();start.clicked();
    wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id!=first_id && p.open.get() && p.pending.get()==0));
    let p=a.active.borrow().as_ref().unwrap().clone();let scene_id=p.scene_id.clone();
    assert_eq!(count(prologue_id),1);assert_eq!(count(chapter["id"].as_str().unwrap()),2);assert!(!p.dirty.get());
    edit(&p,"A prologue begins. مِ 中文 👩🏽‍🚀\n");assert_ne!(rpc(&a.client,"load_scene",json!({"sceneId":scene_id}))["scene"]["document"],p.frame.borrow()["document"]);
    let document=p.frame.borrow()["document"].clone();save(a,&p);assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":scene_id}))["scene"]["document"],document);
    capture(a,"notebook-prologue-writing.png");
    edit(&p,"Keep until saved. ");select(a,SaveChoice::Cancel);a.tree.row_activated(&outline_path(a,chapter["id"].as_str().unwrap()),&column);assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));assert!(p.dirty.get());
    select(a,SaveChoice::Save);a.tree.row_activated(&outline_path(a,chapter["id"].as_str().unwrap()),&column);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==first_id));
    assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":scene_id}))["scene"]["document"],p.frame.borrow()["document"]);
    a.open_chapter(prologue_id,false);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==scene_id));assert_eq!(count(prologue_id),1);
    // The manuscript overview also gives an empty chapter a usable route.
    a.open_book();let group=a.book.children().into_iter().filter_map(|w|w.downcast::<gtk::Expander>().ok()).find(|w|w.label().as_deref()==Some("Appendix")).unwrap();find_button(&group,"Start writing").unwrap().clicked();
    let page=a.current_widget().unwrap();assert!(has_label(&page,"Appendix"));let start=find_button(&page,"Start writing").unwrap();
    a.open_scene(first_id,false);start.clicked();assert_eq!(count(appendix_id),0);
    // Delayed metadata must not create a scene or replace a page opened afterwards.
    a.open_chapter(appendix_id,false);find_button(&a.current_widget().unwrap(),"Start writing").unwrap().clicked();a.open_scene(first_id,false);
    assert_eq!(count(appendix_id),0);assert_eq!(a.current_widget(),Some(a.panes.borrow()[first_id].widget.clone().upcast()));
    // A stale empty-page view opens an existing first scene rather than adding another.
    a.open_chapter(appendix_id,false);let start=find_button(&a.current_widget().unwrap(),"Start writing").unwrap();
    let existing=rpc(&a.client,"create_scene",json!({"chapterId":appendix["id"],"scene":{"schemaVersion":1,"title":"Already started","document":{"type":"doc","content":[{"type":"paragraph"}]}}}));start.clicked();
    wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==existing["id"].as_str().unwrap() && p.open.get()));assert_eq!(count(appendix_id),1);
    // Middle click uses the actual outline event and leaves the original editor alone.
    pump(Duration::from_millis(80));let path=outline_path(a,prologue_id);let area=a.tree.cell_area(Some(&path),Some(&column));
    let mut event=gdk::Event::new(gdk::EventType::ButtonPress);let e=event.downcast_mut::<gdk::EventButton>().unwrap().as_mut();e.button=2;e.x=(area.x()+3) as f64;e.y=(area.y()+area.height()/2) as f64;
    assert!(a.tree.path_at_pos(e.x as i32,e.y as i32).is_some());let old=a.current_widget();assert!(a.tree.emit_by_name::<bool>("button-press-event",&[&event]));wait(||windows()==2 && last_window().active.borrow().as_ref().is_some_and(|p|p.scene_id==scene_id && p.open.get()));assert_eq!(a.current_widget(),old);last_window().close();wait(||windows()==1);
    // Empty pages still work after drag-out; their action follows their current window.
    a.open_chapter(afterword_id,false);a.detach_current();let child=last_window();let old=a.current_widget();find_button(&child.current_widget().unwrap(),"Start writing").unwrap().clicked();
    wait(||child.active.borrow().as_ref().is_some_and(|p|p.open.get()));assert_eq!(count(afterword_id),1);assert_eq!(a.current_widget(),old);child.close();wait(||windows()==1);
    // Closing while metadata loads must not create a first scene in the background.
    let empty=rpc(&a.client,"create_chapter",json!({"projectId":project["id"],"title":"Unstarted"}));a.load_index();wait(||a.index.borrow()["chapters"].as_array().unwrap().len()==5);
    a.open_chapter(empty["id"].as_str().unwrap(),true);let child=last_window();find_button(&child.current_widget().unwrap(),"Start writing").unwrap().clicked();child.close();wait(||windows()==1);assert_eq!(count(empty["id"].as_str().unwrap()),0);
    a.open_scene(first_id,false);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==first_id && p.open.get() && p.pending.get()==0));
    // Cancelling ordinary New scene navigation must not leave a saved empty scene.
    let p=a.active.borrow().as_ref().unwrap().clone();let total=index()["scenes"].as_array().unwrap().len();edit(&p,"Cancelled new scene. ");select(a,SaveChoice::Cancel);
    glib::timeout_add_local_once(Duration::from_millis(40),||{for w in gtk::Window::list_toplevels(){if let Ok(d)=w.downcast::<gtk::Dialog>(){if d.title().as_deref()==Some("New scene"){d.response(gtk::ResponseType::Accept);}}}});
    a.new_scene();assert_eq!(index()["scenes"].as_array().unwrap().len(),total);assert!(p.dirty.get());assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));
    select(a,SaveChoice::Discard);a.open_chapter(prologue_id,false);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==scene_id));a.open_scene(first_id,false);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==first_id));
}
fn new_novel_workflow(binaries:&Path){
    let fixture=tempfile::tempdir().unwrap();let data=fixture.path().join("new-novel-data");let cache=fixture.path().join("new-novel-cache");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(&cache).unwrap();
    let companion=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};let client=Process::start(&binaries.join(companion),&data,true).unwrap();
    let a=Workspace::build(client,&cache,binaries,false,false).unwrap();wait(||a.status.text()=="Choose New novel to start writing.");
    assert_eq!(windows(),1);assert!(!has_label(&a.desk.sidebar,"Middle click a page for a new window.\nDrag a top tab out to move it."));capture(&a,"notebook-new-novel.png");
    // The first-launch page creates and opens writing through its actual button.
    answer_new_novel("A fresh novel مِ 中文",gtk::ResponseType::Accept);find_button(&a.current_widget().unwrap(),"New novel").unwrap().clicked();
    assert!(!gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Dialog>().ok()).any(|d|d.is_visible() && d.title().as_deref()==Some("New novel")));
    wait(||a.active.borrow().as_ref().is_some_and(|p|p.open.get() && p.pending.get()==0) && a.projects.active_id().as_deref()==Some(a.pid.borrow().as_str()));
    assert_eq!(a.index.borrow()["chapters"].as_array().unwrap().len(),1);assert_eq!(a.index.borrow()["scenes"].as_array().unwrap().len(),1);assert_eq!(a.index.borrow()["chapters"][0]["title"],"Chapter 1");assert_eq!(a.pages.n_pages(),1);assert_eq!(windows(),1);assert_eq!(a.desk.area.active_id().as_deref(),Some("Write"));
    let first_pid=a.pid.borrow().clone();let first=a.active.borrow().as_ref().unwrap().clone();assert_eq!(novel_storage::plain_text(&first.frame.borrow()["document"]),"");edit(&first,"The story begins. مِ 中文 👩🏽‍🚀\n");let document=first.frame.borrow()["document"].clone();
    a.desk.area.set_active_id(Some("Project"));let create=find_button(&a.desk.sidebar,"New novel").unwrap();
    answer_new_novel("Cancelled name",gtk::ResponseType::Cancel);create.clicked();assert!(!a.loading.get());assert_eq!(rpc(&a.client,"list_projects",json!({})).as_array().unwrap().len(),1);
    answer_new_novel(" ",gtk::ResponseType::Accept);create.clicked();assert!(a.status.text().contains("Enter a name"));assert!(!a.loading.get());assert_eq!(a.current_widget(),Some(first.widget.clone().upcast()));
    // Cancelling the manual-save decision creates no novel and preserves the draft.
    select(&a,SaveChoice::Cancel);answer_new_novel("Do not create",gtk::ResponseType::Accept);create.clicked();assert_eq!(rpc(&a.client,"list_projects",json!({})).as_array().unwrap().len(),1);assert_eq!(*a.pid.borrow(),first_pid);assert_eq!(first.frame.borrow()["document"],document);assert!(first.dirty.get());
    // Save the old draft before creating; switch this window to a ready blank scene.
    a.search.set_text("unmatched title");select(&a,SaveChoice::Save);answer_new_novel("Second novel",gtk::ResponseType::Accept);create.clicked();
    wait(||*a.pid.borrow()!=first_pid && a.active.borrow().as_ref().is_some_and(|p|p.open.get() && p.pending.get()==0));
    assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":first.scene_id}))["scene"]["document"],document);assert_eq!(rpc(&a.client,"list_projects",json!({})).as_array().unwrap().len(),2);assert_eq!(a.pages.n_pages(),1);assert_eq!(windows(),1);assert_eq!(a.desk.area.active_id().as_deref(),Some("Write"));assert_eq!(a.search.text(),"");
    let second=a.active.borrow().as_ref().unwrap().clone();edit(&second,"A second beginning. عَ 繁體 👩🏽‍🚀\n");let document=second.frame.borrow()["document"].clone();
    second.operation(json!({"type":"refresh","top":0}),false);wait(||second.pending.get()==0);assert_eq!(second.frame.borrow()["document"],document);assert!(second.dirty.get());
    save(&a,&second);second.operation(json!({"type":"refresh","top":0}),false);wait(||second.pending.get()==0);assert!(!second.dirty.get());assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":second.scene_id}))["scene"]["document"],second.frame.borrow()["document"]);capture(&a,"notebook-created-novel.png");
    // The library exposes the same standalone creation action.
    a.desk.area.set_active_id(Some("Project"));find_button(&a.desk.sidebar,"Library").unwrap().clicked();wait(||a.current_widget().is_some_and(|w|has_label(&w,"Novel and universe library")));answer_new_novel("Library novel",gtk::ResponseType::Accept);find_button(&a.current_widget().unwrap(),"New novel").unwrap().clicked();
    wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id!=second.scene_id && p.open.get()));assert_eq!(rpc(&a.client,"list_projects",json!({})).as_array().unwrap().len(),3);
    // A backend validation failure leaves the existing page usable and no partial novel.
    let old=a.current_widget();answer_new_novel(&"x".repeat(501),gtk::ResponseType::Accept);a.new_project();wait(||!a.loading.get());assert!(a.window.is_sensitive());assert_eq!(a.current_widget(),old);assert_eq!(rpc(&a.client,"list_projects",json!({})).as_array().unwrap().len(),3);assert!(a.status.text().contains("1–500"));
    a.close();wait(||windows()==0);
}
fn review_translation_workflows(binaries:&Path){
    let fixture=tempfile::tempdir().unwrap();let data=fixture.path().join("review-data");let cache=fixture.path().join("review-cache");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(&cache).unwrap();
    let worker=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};let client=Process::start(&binaries.join(worker),&data,true).unwrap();
    let created=rpc(&client,"create_novel",json!({"title":"Synthetic review and translations"}));let pid=created["project"]["id"].as_str().unwrap();let sid=created["scene"]["id"].as_str().unwrap();
    let rich=json!({"schemaVersion":1,"title":"Saved source مِ 中文","document":{"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"مِ e\u{301} 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"},{"type":"underline"}]}]}]}});
    rpc(&client,"save_scene",json!({"sceneId":sid,"scene":rich,"parent":created["scene"]["revisionId"],"requestId":"review-lifecycle-source"}));
    let a=Workspace::build(client,&cache,binaries,false,false).unwrap();wait(||a.active.borrow().as_ref().is_some_and(|p|p.open.get()&&p.pending.get()==0));
    // Register an actual translation draft so Ctrl+S and tab close use Desk's
    // shared save decision instead of calling the page's save method directly.
    let source_history=rpc(&a.client,"history",json!({"sceneId":sid}));
    let p=TranslationPreviewPage::new(pid.into(),sid.into(),a.client.clone());a.translation_preview_pages.borrow_mut().push(p.clone());a.install(&p.widget,"Translation lifecycle",false);p.load(&a);wait(||p.loaded.get());
    find_button(&p.widget,"Add translation myself").unwrap().clicked();wait(||p.dirty.get());p.text.buffer().unwrap().set_text("Manual lifecycle translation مِ 中文 𠀀");
    select(&a,SaveChoice::Cancel);a.close_tab(p.widget.upcast_ref());assert!(a.pages.page_num(&p.widget).is_some());assert!(p.dirty.get());assert!(a.translation_preview_pages.borrow().iter().any(|v|Rc::ptr_eq(v,&p)));
    select(&a,SaveChoice::Save);a.close_tab(p.widget.upcast_ref());wait(||a.pages.page_num(&p.widget).is_none());assert!(!p.dirty.get());assert!(!a.translation_preview_pages.borrow().iter().any(|v|Rc::ptr_eq(v,&p)));
    let data=rpc(&a.client,"load_project_data_unambiguous",json!({"projectId":pid,"kind":"sceneTranslations"}));assert!(data["payload"]["drafts"].as_array().unwrap().iter().any(|d|d["text"]=="Manual lifecycle translation مِ 中文 𠀀"));assert_eq!(rpc(&a.client,"history",json!({"sceneId":sid})),source_history);
    // A detached dirty translation keeps the same form and saves using its new
    // owner after the source window closes; the manuscript stays untouched.
    let moved=TranslationPreviewPage::new(pid.into(),sid.into(),a.client.clone());a.translation_preview_pages.borrow_mut().push(moved.clone());a.install(&moved.widget,"Detached translation lifecycle",false);moved.load(&a);wait(||moved.loaded.get());moved.text.buffer().unwrap().set_text("Detached lifecycle draft 中文 مِ 𠀀");assert!(moved.dirty.get());
    a.detach_current();let child=workspace_for_widget(moved.widget.upcast_ref()).unwrap();assert!(!a.translation_preview_pages.borrow().iter().any(|v|Rc::ptr_eq(v,&moved)));assert!(child.translation_preview_pages.borrow().iter().any(|v|Rc::ptr_eq(v,&moved)));a.close();wait(||a.desk.is_closed());
    let saved=Rc::new(RefCell::new(None));let out=saved.clone();child.save_current(move|r|*out.borrow_mut()=Some(r));wait(||saved.borrow().is_some());saved.borrow_mut().take().unwrap().unwrap();assert!(!moved.dirty.get());let data=rpc(&child.client,"load_project_data_unambiguous",json!({"projectId":pid,"kind":"sceneTranslations"}));assert!(data["payload"]["drafts"].as_array().unwrap().iter().any(|d|d["text"]=="Detached lifecycle draft 中文 مِ 𠀀"));assert_eq!(rpc(&child.client,"history",json!({"sceneId":sid})),source_history);
    // Source loading for optional generation never makes a translation a saving
    // draft. Accepted release cancels its token before settings/provider launch.
    find_button(&moved.widget,"Generate translation").unwrap().clicked();assert!(!moved.saving.get());assert!(!child.draft(moved.widget.upcast_ref()).unwrap().busy());child.close_tab(moved.widget.upcast_ref());wait(||child.pages.page_num(&moved.widget).is_none());assert!(moved.status.text().contains("generation cancelled"));assert!(!child.translation_preview_pages.borrow().iter().any(|v|Rc::ptr_eq(v,&moved)));child.close();wait(||windows()==0);
}
#[test]
#[ignore="Requires a graphical desktop; creates only synthetic temporary stores and does not take focus."]
fn notebook_navigation_manual_save_and_multi_window() {
    gtk::init().unwrap();notebook_surfaces_remain_light();let fixture=tempfile::tempdir().unwrap();let data=fixture.path().join("data");let cache=fixture.path().join("cache");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(&cache).unwrap();
    let binaries=std::env::var_os("NOVEL_RUST_TEST_BINARIES").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug"));
    if let Ok(section)=std::env::var("NOVEL_NATIVE_TEST_SECTION") {
        match section.as_str() {
            "navigation"=>exact_navigation::run(&binaries),
            "shortcuts"=>shortcut_forms::run(&binaries),
            "view"=>desktop_preference_forms::run(&binaries),
            "metadata"=>super::super::metadata_conflicts::tests::run(&binaries),
            "private-sync"=>super::super::private_sync::tests::run(&binaries),
            "board"=>board_forms::run(&binaries),
            "dictionary"=>dictionary_forms::run(&binaries),
            "projections"=>projection_forms::run(&binaries),
            "conflicts"=>project_data_conflicts::run(&binaries),
            "ripple"=>super::super::ripple::tests::run(&binaries),
            "knowledge"=>knowledge_forms::run(&binaries),
            "peek"=>peek_forms::run(&binaries),
            "scene-recovery"=>super::super::scene_conflicts::tests::run(&binaries),
            "review-decisions"|"translations"|"review-translations"=>review_translation_workflows(&binaries),
            "review-host"=>super::super::review_host::tests::run(&binaries),
            _=>panic!("Unknown native test section: {section}"),
        }
        assert_eq!(windows(),0); return;
    }
    new_novel_workflow(&binaries);
    notebook_features::run(&binaries);
    chapter_forms::run(&binaries);
    paragraph_forms::run(&binaries);
    project_data_conflicts::run(&binaries);
    scene_arrangement_forms::run(&binaries);
    projection_forms::run(&binaries);
    dictionary_forms::run(&binaries);
    board_forms::run(&binaries);
    exact_navigation::run(&binaries);
    shortcut_forms::run(&binaries);
    desktop_preference_forms::run(&binaries);
    super::super::metadata_conflicts::tests::run(&binaries);
    super::super::private_sync::tests::run(&binaries);
    super::super::ripple::tests::run(&binaries);
    knowledge_forms::run(&binaries);
    peek_forms::run(&binaries);
    super::super::scene_conflicts::tests::run(&binaries);
    review_translation_workflows(&binaries);
    super::super::review_host::tests::run(&binaries);
    let companion=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};let client=Process::start(&binaries.join(companion),&data,true).unwrap();
    let project=rpc(&client,"create_project",json!({"title":"The quiet notebook"}));let chapter=rpc(&client,"create_chapter",json!({"projectId":project["id"],"title":"Chapter one"}));
    let first=rpc(&client,"create_scene",json!({"chapterId":chapter["id"],"scene":{"schemaVersion":1,"title":"The first morning","document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"The room was quiet. مِ 中文 👩🏽‍🚀"}]}]}}}));
    let second=rpc(&client,"create_scene",json!({"chapterId":chapter["id"],"scene":{"schemaVersion":1,"title":"A letter arrives","document":{"type":"doc","content":[{"type":"paragraph"}]}}}));
    let a=Workspace::build(client,&cache,&binaries,false,false).unwrap();wait(||a.active.borrow().as_ref().is_some_and(|p|p.open.get() && p.pending.get()==0));let id=first["id"].as_str().unwrap();let second_id=second["id"].as_str().unwrap();let p=a.panes.borrow()[id].clone();
    assert_eq!(a.pages.n_pages(),1);assert_eq!(windows(),1);assert!(a.desk.groups.borrow().iter().find(|(name,_)|name=="Write").unwrap().1.is_visible());
    chapter_writing(&a,&project,&first,&chapter);
    // Reproduce the synchronous focus-in callback without taking desktop focus.
    // A normal no-focus test otherwise misses the user's active-scene crash.
    let focus_events=Rc::new(Cell::new(0));let events=focus_events.clone();
    let focus_handler=p.area.connect_grab_focus(move|area|{
        events.set(events.get()+1);
        let mut event=gdk::Event::new(gdk::EventType::FocusChange);
        event.downcast_mut::<gdk::EventFocus>().unwrap().as_mut().in_=1;
        area.send_focus_change(&event);
    });
    a.open_scene(id,false);assert!(focus_events.get()>0);
    let mut scene_path=None;
    a.outline.foreach(|model,path,iter|{
        if model.value(iter,1).get::<String>().ok().as_deref()==Some(id){scene_path=Some(path.clone());true}else{false}
    });
    let scene_path=scene_path.unwrap();let column=a.tree.column(0).unwrap();
    a.tree.row_activated(&scene_path,&column);assert!(focus_events.get()>1);
    // Manual means an idle draft does not create a saved revision.
    edit(&p,"A new beginning. عَ 繁體 ");pump(Duration::from_millis(1000));assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":id}))["revisionId"],first["revisionId"]);
    let draft=p.frame.borrow()["document"].clone();let focused=focus_events.get();
    for _ in 0..3{a.tree.row_activated(&scene_path,&column);}
    assert!(focus_events.get()>=focused+3);assert_eq!(p.frame.borrow()["document"],draft);
    assert!(p.dirty.get());assert_eq!(a.pages.n_pages(),1);assert!(Rc::ptr_eq(a.active.borrow().as_ref().unwrap(),&p));
    p.area.disconnect(focus_handler);
    let mut event=gdk::Event::new(gdk::EventType::FocusChange);
    event.downcast_mut::<gdk::EventFocus>().unwrap().as_mut().in_=0;
    p.area.send_focus_change(&event);
    select(&a,SaveChoice::Cancel);a.open_scene(second_id,false);pump(Duration::from_millis(60));assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));assert!(p.dirty.get());assert!(a.window.is_sensitive());
    // Exercise the actual modal Cancel response as well as queued decisions.
    glib::timeout_add_local_once(Duration::from_millis(40),||{for w in gtk::Window::list_toplevels(){if let Ok(d)=w.downcast::<gtk::Dialog>(){if d.title().as_deref()==Some("Unsaved changes"){d.response(gtk::ResponseType::Cancel);}}}});
    a.open_scene(second_id,false);assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));
    p.composing.set(true);a.open_scene(second_id,false);assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));assert!(p.dirty.get());p.composing.set(false);
    p.operation(json!({"type":"text","text":"Queued exact input. "}),true);
    select(&a,SaveChoice::Save);a.open_scene(second_id,false);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==second_id && p.open.get()));assert_eq!(a.pages.n_pages(),1);assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":id}))["scene"]["document"],p.frame.borrow()["document"]);
    a.open_scene(id,false);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==id));let saved=p.frame.borrow()["document"].clone();edit(&p,"Discard this. ");select(&a,SaveChoice::Discard);a.open_scene(second_id,false);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==second_id));assert_eq!(p.frame.borrow()["document"],saved);assert!(!p.dirty.get());
    // Top tabs: ordinary links replace the active tab; explicit New tab adds one.
    a.new_tab();assert_eq!(a.pages.n_pages(),2);a.open_scene(id,false);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==id));edit(&p,"A kept tab draft. ");select(&a,SaveChoice::Cancel);a.pages.set_current_page(Some(0));pump(Duration::from_millis(80));assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));
    select(&a,SaveChoice::Save);a.pages.set_current_page(Some(0));wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==second_id));assert!(!p.dirty.get());assert_eq!(a.pages.n_pages(),2);
    a.pages.set_current_page(Some(1));wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==id));capture(&a,"notebook-writing.png");
    a.details();let metadata=a.page("sceneMetadata");wait(||metadata.loaded.get() && a.current_widget()==Some(metadata.widget.clone().upcast()));assert!(metadata.dirty.get());select(&a,SaveChoice::Save);a.open_scene(id,false);wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==id));assert_eq!(rpc(&a.client,"load_project_data",json!({"projectId":project["id"],"kind":"sceneMetadata"}))["payload"]["scenes"][0]["sceneId"],first["id"]);
    // A middle button event on an actual navigation button opens a full second window.
    let group=gtk::Box::new(gtk::Orientation::Vertical,0);let sid=second_id.to_owned();let b=a.link(&group,"Another scene",move|a|a.open_scene(&sid,false));
    let mut event=gdk::Event::new(gdk::EventType::ButtonPress);event.downcast_mut::<gdk::EventButton>().unwrap().as_mut().button=2;
    let stopped=b.emit_by_name::<bool>("button-press-event",&[&event]);assert!(stopped);wait(||windows()==2 && last_window().active.borrow().as_ref().is_some_and(|p|p.scene_id==second_id && p.open.get()));let child=last_window();assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));assert!(child.desk.sidebar.is_visible());assert_eq!(child.pages.n_pages(),1);
    let child_p=child.active.borrow().as_ref().unwrap().clone();edit(&child_p,"Independent window. ");assert_ne!(child_p.frame.borrow()["document"],p.frame.borrow()["document"]);select(&child,SaveChoice::Discard);child.close();wait(||windows()==1);assert!(a.window.is_visible());
    // Drag-out uses GTK's create-window signal, moves the same editor and retains Undo.
    edit(&p,"Move this draft. ");let before=p.frame.borrow()["document"].clone();let widget=p.widget.clone().upcast::<gtk::Widget>();let target=a.pages.emit_by_name::<gtk::Notebook>("create-window",&[&widget,&0i32,&0i32]);let moved=last_window();assert_eq!(target,moved.pages);let tab=a.pages.tab_label(&widget).unwrap();a.desk.suppress.set(true);a.pages.remove_page(a.pages.page_num(&widget));a.desk.suppress.set(false);target.append_page(&widget,Some(&tab));widget.show_all();wait(||moved.active.borrow().is_some());assert!(Rc::ptr_eq(&moved.panes.borrow()[id],&p));assert_eq!(p.frame.borrow()["document"],before);assert!(p.dirty.get());assert!(!a.panes.borrow().contains_key(id));
    p.operation(json!({"type":"command","command":"undo"}),true);wait(||p.pending.get()==0);assert_ne!(p.frame.borrow()["document"],before);save(&moved,&p);capture(&moved,"notebook-detached.png");
    // Forms have the same Save/Discard/Cancel guard, and are detachable.
    moved.desk.area.set_active_id(Some("Planning"));moved.open_data("planning","Story notes");let planning=moved.page("planning");wait(||planning.loaded.get());let original=planning.value.borrow().clone();let mut value=original.clone();value["records"].as_array_mut().unwrap().push(json!({"id":uuid::Uuid::new_v4().to_string(),"kind":"character","name":"Mariam","aliases":[],"notes":"Keeps old letters. مِ 中文","values":{},"links":[]}));value["boards"][0]["cards"]=json!([{"id":uuid::Uuid::new_v4().to_string(),"title":"An unexpected letter","body":"Mariam finds the first clue.","kind":"note","reference":null,"x":30,"y":30,"groupId":null,"labels":[]}]);planning.update(value.clone());
    select(&moved,SaveChoice::Cancel);moved.open_book();assert_eq!(moved.current_widget(),Some(planning.widget.clone().upcast()));select(&moved,SaveChoice::Discard);moved.open_book();assert_eq!(*planning.value.borrow(),original);
    moved.open_data("planning","Story notes");planning.update(value);select(&moved,SaveChoice::Save);moved.open_board();wait(||moved.current_widget().is_some_and(|w|w!=planning.widget.clone().upcast::<gtk::Widget>()));assert_eq!(rpc(&moved.client,"load_project_data",json!({"projectId":project["id"],"kind":"planning"}))["payload"]["records"][0]["name"],"Mariam");assert!(has_label(&moved.pages.tab_label(&moved.current_widget().unwrap()).unwrap(),"Plot board"));capture(&moved,"notebook-planning.png");
    moved.open_data("planning","Story notes");moved.detach_current();let form_window=last_window();assert!(Rc::ptr_eq(&form_window.page("planning"),&planning));assert_eq!(form_window.current_widget(),Some(planning.widget.clone().upcast()));assert_eq!(windows(),3);
    // Tool pages occupy the current tab, not another toplevel window.
    let count=windows();a.desk.area.set_active_id(Some("Project"));a.library();wait(||a.current_widget().is_some_and(|w|has_label(&w,"Novel and universe library")));assert_eq!(windows(),count);capture(&a,"notebook-project.png");
    // A detached history page routes its buttons to the new window.
    moved.open_scene(id,false);wait(||moved.active.borrow().is_some());moved.desk.area.set_active_id(Some("Review"));moved.history();wait(||moved.current_widget().is_some_and(|w|has_label(&w,"Scene history")));capture(&moved,"notebook-review.png");moved.detach_current();let history_window=last_window();let old=moved.current_widget();
    find_button(&history_window.window,"Preview").unwrap().clicked();wait(||history_window.current_widget().is_some_and(|w|has_label(&w,"Saved scene")));assert_eq!(moved.current_widget(),old);history_window.close();wait(||windows()==count);
    // A competing saved revision refuses Save and leaves the local draft in its window.
    moved.open_scene(id,false);wait(||moved.active.borrow().as_ref().is_some_and(|p|p.scene_id==id));let saved=rpc(&moved.client,"load_scene",json!({"sceneId":id}));let mut other=saved["scene"].clone();other["document"]["content"][0]["content"]=json!([{"type":"text","text":"Competing saved text"}]);let competing=rpc(&moved.client,"save_scene",json!({"sceneId":id,"scene":other,"parent":saved["revisionId"]}));edit(&p,"Retained conflict. ");let local=p.frame.borrow()["document"].clone();select(&moved,SaveChoice::Save);moved.open_scene(second_id,false);wait(||!moved.desk.busy.get());assert_eq!(moved.current_widget(),Some(p.widget.clone().upcast()));assert_eq!(p.frame.borrow()["document"],local);assert!(p.dirty.get());assert_eq!(rpc(&moved.client,"load_scene",json!({"sceneId":id}))["revisionId"],competing["revisionId"]);
    select(&moved,SaveChoice::Cancel);moved.close();assert!(moved.window.is_visible());
    // A separate conflict preview can save the retained local draft as a new revision.
    moved.open_conflicts_window();let conflicts=last_window();wait(||find_button(&conflicts.window,"Preview and choose").is_some());find_button(&conflicts.window,"Preview and choose").unwrap().clicked();wait(||find_button(&conflicts.window,"Use this version; keep all history").is_some());find_button(&conflicts.window,"Use this version; keep all history").unwrap().clicked();
    wait(||!p.dirty.get());
    wait(||conflicts.panes.borrow().get(id).is_some_and(|p|p.open.get()));
    assert_eq!(rpc(&moved.client,"load_scene",json!({"sceneId":id}))["scene"]["document"],local);conflicts.close();wait(||windows()==3);
    // Project switching and tab closing are guarded as well.
    edit(&p,"A final unsaved change. ");select(&moved,SaveChoice::Cancel);moved.close_tab(&p.widget.clone().upcast());assert_eq!(moved.current_widget(),Some(p.widget.clone().upcast()));
    let other_project=rpc(&moved.client,"create_project",json!({"title":"Another notebook"}));select(&moved,SaveChoice::Cancel);moved.switch_project(other_project["id"].as_str().unwrap().into());assert_eq!(*moved.pid.borrow(),project["id"].as_str().unwrap());assert!(p.dirty.get());
    select(&moved,SaveChoice::Discard);moved.close();wait(||windows()==2);
    // Closing the original leaves the shared companion alive for another window.
    a.close();wait(||windows()==1);assert!(form_window.window.is_visible());assert_eq!(rpc(&form_window.client,"list_projects",json!({}))[0]["id"],project["id"]);
    form_window.open_data("exportProfile","Word styles");let profile=form_window.page("exportProfile");wait(||profile.loaded.get());let mut value=profile.value.borrow().clone();value["pageSize"]=json!("a4");profile.update(value);let done=Rc::new(Cell::new(false));let out=done.clone();form_window.save_current(move|r|{r.unwrap();out.set(true);});wait(||done.get());
    let snapshot:novel_storage::WordExportSnapshot=serde_json::from_value(rpc(&form_window.client,"word_export_snapshot",json!({"projectId":project["id"],"profileRevisionId":profile.parent.borrow().clone()}))).unwrap();assert_eq!(snapshot.profile.page_size,"a4");let docx=novel_docx::export_manuscript_with_profile(&snapshot.bundle,&snapshot.profile).unwrap();assert_eq!(&docx[..2],b"PK");
    form_window.close();wait(||windows()==0);assert!(!form_window.window.is_visible());
}
