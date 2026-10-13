//! Single-threaded opt-in GTK/worker coverage; called by desk_tests.
use super::*;

fn combo(widget:&impl IsA<gtk::Widget>,tooltip:&str)->Option<gtk::ComboBoxText>{
    if let Ok(combo)=widget.as_ref().clone().downcast::<gtk::ComboBoxText>(){if combo.tooltip_text().as_deref()==Some(tooltip){return Some(combo);}}
    if let Ok(container)=widget.as_ref().clone().downcast::<gtk::Container>(){for child in container.children(){if let Some(combo)=combo(&child,tooltip){return Some(combo);}}}None
}
fn label_entry(widget:&impl IsA<gtk::Widget>)->Option<gtk::Entry>{
    if let Ok(entry)=widget.as_ref().clone().downcast::<gtk::Entry>(){if entry.placeholder_text().as_deref()==Some("Connection label"){return Some(entry);}}
    if let Ok(container)=widget.as_ref().clone().downcast::<gtk::Container>(){for child in container.children(){if let Some(entry)=label_entry(&child){return Some(entry);}}}None
}
fn canvas(widget:&impl IsA<gtk::Widget>)->Option<gtk::DrawingArea>{
    if let Ok(area)=widget.as_ref().clone().downcast::<gtk::DrawingArea>(){return Some(area);}
    if let Ok(container)=widget.as_ref().clone().downcast::<gtk::Container>(){for child in container.children(){if let Some(area)=canvas(&child){return Some(area);}}}None
}
fn contains_label(widget:&impl IsA<gtk::Widget>,text:&str)->bool{
    if let Ok(label)=widget.as_ref().clone().downcast::<gtk::Label>(){if label.text().contains(text){return true;}}
    widget.as_ref().clone().downcast::<gtk::Container>().ok().is_some_and(|c|c.children().iter().any(|w|contains_label(w,text)))
}
fn rendered_label(area:&gtk::DrawingArea)->Vec<u8>{
    // These fixture endpoints put their label at (335,190), away from cards.
    // Capture only the caption region after the actual native draw callback.
    let mut surface=cairo::ImageSurface::create(cairo::Format::ARgb32,area.allocated_width(),area.allocated_height()).unwrap();let context=cairo::Context::new(&surface).unwrap();area.draw(&context);drop(context);surface.flush();let stride=surface.stride() as usize;let data=surface.data().unwrap();let mut pixels=Vec::new();for y in 145..235{pixels.extend_from_slice(&data[y*stride+200*4..y*stride+470*4]);}pixels
}
fn saved(a:&Rc<Workspace>){
    let reply=Rc::new(RefCell::new(None));let out=reply.clone();a.save_current(move|r|*out.borrow_mut()=Some(r));wait(||reply.borrow().is_some());reply.borrow_mut().take().unwrap().unwrap();
}
pub(super) fn run(binaries:&Path){
    assert_eq!(windows(),0);let fixture=tempfile::tempdir().unwrap();let data=fixture.path().join("board-data");let cache=fixture.path().join("board-cache");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(&cache).unwrap();
    let companion=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};let client=Process::start(&binaries.join(companion),&data,true).unwrap();let novel=rpc(&client,"create_novel",json!({"title":"Authored board fixture"}));let pid=novel["project"]["id"].as_str().unwrap().to_owned();let sid=novel["scene"]["id"].as_str().unwrap().to_owned();let original_scene=rpc(&client,"load_scene",json!({"sceneId":sid}));
    let mut planning=crate::tools::initial("planning");planning["boards"]=json!([{"id":"board","name":"Authored board","groups":[],"cards":[{"id":"card-a","kind":"scene","title":"نور 明","body":"مِ first","reference":{"kind":"scene","id":sid},"x":50,"y":50,"groupId":null,"labels":[]},{"id":"card-b","kind":"note","title":"Garden 中文","body":"Second","reference":null,"x":400,"y":220,"groupId":null,"labels":[]}],"edges":[]}]);let initial=rpc(&client,"save_project_data",json!({"projectId":pid,"kind":"planning","payload":planning,"parent":null}));
    let a=Workspace::build(client,&cache,binaries,false,false).unwrap();wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==sid&&p.open.get()&&p.pending.get()==0));a.desk.area.set_active_id(Some("Planning"));a.open_board();wait(||find_button(&a.desk.board_controls,"Connect cards").is_some());let board=a.current_widget().unwrap();let page=a.current_data("planning").unwrap();assert_eq!(a.desk.board_controls.children().len(),1);
    let source=combo(&a.desk.board_controls,"Connection source").unwrap();let target=combo(&a.desk.board_controls,"Connection target").unwrap();let kind=combo(&a.desk.board_controls,"Connection kind").unwrap();let label=label_entry(&a.desk.board_controls).unwrap();assert_eq!(combo(&a.desk.board_controls,"Board").unwrap().active_id().as_deref(),Some("board"));source.set_active_id(Some("card-a"));target.set_active_id(Some("card-b"));kind.set_active_id(Some("custom"));let multilingual="يَكشف 中文 e\u{301} \u{e000} 👩🏽‍🚀";label.set_text(multilingual);
    find_button(&a.desk.board_controls,"Connect cards").unwrap().clicked();assert!(page.dirty.get());assert_eq!(page.value.borrow()["boards"][0]["edges"].as_array().unwrap().len(),1);let edge=page.value.borrow()["boards"][0]["edges"][0].clone();assert_eq!(edge["label"],multilingual);assert_eq!(edge["kind"],"custom");assert_eq!(edge["confirmed"],false);assert_eq!(page.value.borrow()["boards"][0]["cards"],planning["boards"][0]["cards"]);
    find_button(&a.desk.board_controls,"Connect cards").unwrap().clicked();assert!(contains_label(&a.desk.board_controls,"already exists"));assert_eq!(page.value.borrow()["boards"][0]["edges"].as_array().unwrap().len(),1);pump(Duration::from_millis(250));assert_eq!(rpc(&a.client,"load_project_data",json!({"projectId":pid,"kind":"planning"})),initial);
    select(&a,SaveChoice::Cancel);a.open_scene(&sid,false);assert_eq!(a.current_widget(),Some(board.clone()));assert!(page.dirty.get());
    // Actual Pango/Cairo rendering changes when the exact authored label is
    // present. This does not rely on cairo toy-text or a fake label widget.
    let area=canvas(&board).unwrap();pump(Duration::from_millis(80));let full=rendered_label(&area);page.value.borrow_mut()["boards"][0]["edges"][0]["label"]=json!("");let generic=rendered_label(&area);page.value.borrow_mut()["boards"][0]["edges"][0]["label"]=json!(multilingual);assert_ne!(full,generic);saved(&a);assert!(!page.dirty.get());let committed=rpc(&a.client,"load_project_data",json!({"projectId":pid,"kind":"planning"}));assert_eq!(committed["payload"]["boards"][0]["edges"][0],edge);capture(&a,"native-board-labelled-connection.png");
    // A card removed after choices were captured cannot become a dangling edge
    // or silently attach to a new card occupying the old array position.
    let mut removed=page.value.borrow().clone();removed["boards"][0]["cards"].as_array_mut().unwrap().remove(1);page.update(removed.clone());label.set_text("Another connection");find_button(&a.desk.board_controls,"Connect cards").unwrap().clicked();assert!(contains_label(&a.desk.board_controls,"cards changed"));assert_eq!(*page.value.borrow(),removed);find_button(&a.desk.board_controls,"Refresh card choices").unwrap().clicked();assert_eq!(target.active_id().as_deref(),Some(""));find_button(&a.desk.board_controls,"Connect cards").unwrap().clicked();assert!(contains_label(&a.desk.board_controls,"two different"));assert_eq!(*page.value.borrow(),removed);
    page.discard();find_button(&a.desk.board_controls,"Refresh card choices").unwrap().clicked();assert!(!page.dirty.get());source.set_active_id(Some("card-a"));target.set_active_id(Some("card-a"));find_button(&a.desk.board_controls,"Connect cards").unwrap().clicked();assert!(contains_label(&a.desk.board_controls,"two different"));assert!(!page.dirty.get());
    a.detach_current();let child=last_window();assert_eq!(child.current_widget(),Some(board));assert!(child.current_data("planning").is_some_and(|p|Rc::ptr_eq(&p,&page)));a.close();wait(||windows()==1);
    let source=combo(&child.desk.board_controls,"Connection source").unwrap();let target=combo(&child.desk.board_controls,"Connection target").unwrap();let kind=combo(&child.desk.board_controls,"Connection kind").unwrap();let label=label_entry(&child.desk.board_controls).unwrap();source.set_active_id(Some("card-b"));target.set_active_id(Some("card-a"));kind.set_active_id(Some("depends-on"));label.set_text("ربط آخر 中文");find_button(&child.desk.board_controls,"Connect cards").unwrap().clicked();assert!(page.dirty.get());assert_eq!(page.value.borrow()["boards"][0]["edges"].as_array().unwrap().len(),2);saved(&child);
    let persisted=rpc(&child.client,"load_project_data",json!({"projectId":pid,"kind":"planning"}));assert_eq!(persisted["payload"]["boards"][0]["edges"][0],edge);assert_eq!(persisted["payload"]["boards"][0]["edges"][1]["label"],"ربط آخر 中文");assert_eq!(persisted["payload"]["boards"][0]["cards"],planning["boards"][0]["cards"]);assert_eq!(rpc(&child.client,"load_scene",json!({"sceneId":sid})),original_scene);child.close();wait(||windows()==0);
}
