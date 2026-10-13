//! Synthetic GTK checks run inside the existing single-threaded desktop test.
use super::*;

fn request(p:&Rc<EditorPane>,value:Value,editing:bool)->Value{
    let reply=Rc::new(RefCell::new(None));let out=reply.clone();p.request(value,editing,move|r|*out.borrow_mut()=Some(r));wait(||reply.borrow().is_some());let value=reply.borrow_mut().take().unwrap().unwrap();value
}
fn navigation(p:&Rc<EditorPane>)->Value{request(p,json!({"type":"command","command":"captureNavigation"}),false)["navigationCapture"].clone()}
fn select_text(p:&Rc<EditorPane>,anchor:[usize;2],cursor:[usize;2]){
    let mut bookmark=navigation(p);bookmark["anchor"]=json!(anchor);bookmark["cursor"]=json!(cursor);request(p,json!({"type":"command","command":"restoreNavigation","bookmark":bookmark}),false);
}
fn tools(a:&Rc<Workspace>)->Value{rpc(&a.client,"load_project_data",json!({"projectId":a.pid.borrow().clone(),"kind":"notebookTools"}))["payload"].clone()}
fn answer(title:&str,action:&str,text:&str){
    let title=title.to_owned();let action=action.to_owned();let text=text.to_owned();glib::timeout_add_local_once(Duration::from_millis(40),move||{
        let dialog=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Dialog>().ok()).find(|d|d.is_visible() && d.title().as_deref()==Some(title.as_str())).expect("The explicit note action should show its prompt");
        assert!(find_button(&dialog,&action).is_some());find_entry(&dialog).unwrap().set_text(&text);dialog.response(gtk::ResponseType::Accept);
    });
}
fn text_view(widget:&impl IsA<gtk::Widget>)->Option<gtk::TextView>{
    if let Ok(view)=widget.as_ref().clone().downcast::<gtk::TextView>(){return Some(view);}
    if let Ok(container)=widget.as_ref().clone().downcast::<gtk::Container>(){for child in container.children(){if let Some(view)=text_view(&child){return Some(view);}}}None
}
fn reader_select(widget:&impl IsA<gtk::Widget>,quote:&str){
    let view=text_view(widget).expect("The reading page should contain a TextView");assert!(!view.is_editable());let buffer=view.buffer().unwrap();let text=buffer.text(&buffer.start_iter(),&buffer.end_iter(),true).unwrap();let byte=text.find(quote).expect("Reading copy must contain the saved quote");let start=text[..byte].chars().count() as i32;let end=start+quote.chars().count() as i32;buffer.select_range(&buffer.iter_at_offset(start),&buffer.iter_at_offset(end));
}

pub(super) fn run(binaries:&Path){
    assert_eq!(windows(),0);let fixture=tempfile::tempdir().unwrap();let data=fixture.path().join("notebook-tools-data");let cache=fixture.path().join("notebook-tools-cache");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(&cache).unwrap();
    let companion=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};let client=Process::start(&binaries.join(companion),&data,true).unwrap();
    let project=rpc(&client,"create_novel",json!({"title":"Notebook tools fixture"}));let id=project["scene"]["id"].as_str().unwrap().to_owned();let pid=project["project"]["id"].as_str().unwrap().to_owned();
    let saved=rpc(&client,"load_scene",json!({"sceneId":id}));let removed="REMOVED مِ 中文 🌿";let initial=format!("Opening. {removed}Ending.");let mut scene=saved["scene"].clone();scene["document"]=json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":initial}]}]});rpc(&client,"save_scene",json!({"sceneId":id,"scene":scene,"parent":saved["revisionId"]}));
    let a=Workspace::build(client,&cache,binaries,false,false).unwrap();wait(||a.active.borrow().as_ref().is_some_and(|p|p.scene_id==id && p.open.get() && p.pending.get()==0));let p=a.panes.borrow()[&id].clone();
    // The first explicit note removes the empty state and stays above rewrite counts.
    a.attention();wait(||a.current_widget().is_some_and(|w|has_label(&w,"No unresolved reactions or notes.")));let reminders=a.current_widget().unwrap();assert!(has_label(&reminders,"Passages to revisit"));answer("Note for this scene","Save note","First reminder");find_button(&reminders,"Add note").unwrap().clicked();wait(||has_label(&reminders,"Note saved."));assert!(!has_label(&reminders,"No unresolved reactions or notes."));assert!(has_label(&reminders,"First reminder"));find_button(&reminders,"Mark resolved").unwrap().clicked();wait(||has_label(&reminders,"Resolved."));a.open_scene(&id,false);wait(||a.current_widget()==Some(p.widget.clone().upcast()));
    eprintln!("Notebook tools: exact scrap capture");
    // Keeping a selection saves only a scrap, preserving Unicode and manuscript state.
    let start="Opening. ".chars().count();select_text(&p,[0,start],[0,start+removed.chars().count()]);assert_eq!(request(&p,json!({"type":"snapshot"}),false)["selectedText"],removed);
    let document=p.frame.borrow()["document"].clone();a.keep_scrap();wait(||a.status.text()=="Scrap saved. The selected text is still in your draft.");assert_eq!(tools(&a)["scraps"][0]["text"],removed);assert_eq!(p.frame.borrow()["document"],document);assert!(!p.dirty.get());
    eprintln!("Notebook tools: saved deletion recovery and undo");
    // A real saved deletion is also recovered from immutable scene history.
    request(&p,json!({"type":"key","key":"Backspace","ctrl":false,"shift":false}),true);assert!(p.dirty.get());assert_eq!(novel_storage::plain_text(&p.frame.borrow()["document"]),"Opening. Ending.");save(&a,&p);let after_delete=p.frame.borrow()["document"].clone();let saved_revision=p.parent.borrow().clone();
    a.cutting_room();wait(||a.current_widget().is_some_and(|w|has_label(&w,"All recovered passages remain in scene history.")));let cutting=a.current_widget().unwrap();assert!(has_label(&cutting,removed));assert!(has_label(&cutting,"Kept selection"));assert!(has_label(&cutting,"Untitled scene · removed in a saved edit"));capture(&a,"notebook-cutting-room.png");
    find_button(&cutting,"Append to scene").unwrap().clicked();wait(||p.pending.get()==0 && p.dirty.get() && a.current_widget()==Some(p.widget.clone().upcast()));assert!(novel_storage::plain_text(&p.frame.borrow()["document"]).ends_with(removed));assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":id}))["revisionId"],saved_revision);
    request(&p,json!({"type":"command","command":"undo"}),true);assert_eq!(p.frame.borrow()["document"],after_delete);assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":id}))["revisionId"],saved_revision);select(&a,SaveChoice::Discard);
    a.cutting_room();wait(||a.current_widget().is_some_and(|w|find_button(&w,"Append to scene").is_some()));find_button(&a.current_widget().unwrap(),"Append to scene").unwrap().clicked();wait(||p.pending.get()==0 && p.dirty.get());save(&a,&p);
    eprintln!("Notebook tools: return bookmark");
    // Return bookmarks remember a caret and explicit reminder without dirtying text.
    select_text(&p,[0,5],[0,5]);let bookmark=navigation(&p);let note="Next, she discovers the letter. عَ";answer("Next time, start here","Save bookmark",note);a.leave_bookmark();wait(||a.status.text()=="Return bookmark saved. You can close the notebook when ready.");wait(||p.return_note.is_visible());assert_eq!(p.return_note.text(),format!("Next: {note}"));assert_eq!(tools(&a)["bookmarks"][0]["location"],bookmark);assert!(!p.dirty.get());
    select_text(&p,[0,0],[0,0]);a.cutting_room();wait(||a.current_widget()!=Some(p.widget.clone().upcast()));a.return_to_bookmark();wait(||a.current_widget()==Some(p.widget.clone().upcast()) && p.pending.get()==0);pump(Duration::from_millis(80));assert_eq!(navigation(&p),bookmark);assert!(!p.dirty.get());
    eprintln!("Notebook tools: reader reactions");
    // Reader entry respects the normal manual-save decision, including cancellation.
    edit(&p,"Unsaved reader test. ");select(&a,SaveChoice::Cancel);a.reader();assert_eq!(a.current_widget(),Some(p.widget.clone().upcast()));assert!(p.dirty.get());select(&a,SaveChoice::Discard);a.reader();wait(||a.current_widget().is_some_and(|w|find_button(&w,"Curious").is_some()));assert!(!p.dirty.get());let reading=a.current_widget().unwrap();reader_select(&reading,removed);find_button(&reading,"Curious").unwrap().clicked();wait(||has_label(&reading,"Saved: Curious. Find this passage in Passages to revisit."));let reactions=tools(&a);assert_eq!(reactions["reactions"][0]["quote"],removed);assert_eq!(reactions["reactions"][0]["kind"],"curious");assert_eq!(reactions["reactions"][0]["revisionId"],*p.parent.borrow());capture(&a,"notebook-reader.png");
    // Selecting a heading cannot create an invalid passage mark.
    let before=reactions["reactions"].as_array().unwrap().len();let view=text_view(&reading).unwrap();let buffer=view.buffer().unwrap();buffer.select_range(&buffer.iter_at_offset(0),&buffer.iter_at_offset(4));find_button(&reading,"Bored").unwrap().clicked();assert!(has_label(&reading,"Select text within one scene, without its heading."));assert_eq!(tools(&a)["reactions"].as_array().unwrap().len(),before);
    eprintln!("Notebook tools: detached reader and attention");
    // A detached read-only tab saves in its own window; the original is left alone.
    a.detach_current();let child=last_window();assert_eq!(windows(),2);let original=a.current_widget();let moved=child.current_widget().unwrap();assert_eq!(moved,reading);reader_select(&moved,"Opening.");find_button(&moved,"Bored").unwrap().clicked();wait(||has_label(&moved,"Saved: Bored. Find this passage in Passages to revisit."));assert_eq!(tools(&child)["reactions"].as_array().unwrap().len(),2);assert_eq!(a.current_widget(),original);
    child.attention();wait(||child.current_widget().is_some_and(|w|find_button(&w,"Open passage").is_some()));let attention=child.current_widget().unwrap();pump(Duration::from_millis(80));find_button(&attention,"Open passage").unwrap().clicked();wait(||child.active.borrow().as_ref().is_some_and(|p|p.scene_id==id && p.open.get() && p.pending.get()==0));pump(Duration::from_millis(100));let cp=child.active.borrow().as_ref().unwrap().clone();assert_eq!(request(&cp,json!({"type":"snapshot"}),false)["selectedText"],removed);assert!(!cp.dirty.get());
    child.attention();wait(||child.current_widget().is_some_and(|w|find_button(&w,"Mark resolved").is_some()));let attention=child.current_widget().unwrap();find_button(&attention,"Mark resolved").unwrap().clicked();wait(||has_label(&attention,"Resolved."));assert_eq!(tools(&child)["reactions"][0]["resolved"],true);
    let manual="Check why the letter was hidden. 中文";answer("Note for this scene","Save note",manual);find_button(&attention,"Add note").unwrap().clicked();wait(||has_label(&attention,"Note saved."));assert_eq!(tools(&child)["notes"].as_array().unwrap().last().unwrap()["text"],manual);capture(&child,"notebook-attention.png");assert_eq!(a.current_widget(),original);child.close();wait(||windows()==1);a.close();wait(||windows()==0);
    eprintln!("Notebook tools: persisted reminders after reopening");
    // Reopen the actual store and worker: notes, scraps and the return reminder persist.
    let reopened=Workspace::new_with_focus(&data,&cache,binaries,false).unwrap();wait(||reopened.active.borrow().as_ref().is_some_and(|p|p.open.get() && p.pending.get()==0));assert_eq!(*reopened.pid.borrow(),pid);let rp=reopened.active.borrow().as_ref().unwrap().clone();wait(||rp.return_note.is_visible());assert_eq!(rp.return_note.text(),format!("Next: {note}"));assert!(!rp.dirty.get());reopened.return_to_bookmark();pump(Duration::from_millis(100));wait(||rp.pending.get()==0);assert_eq!(navigation(&rp),bookmark);assert!(!rp.dirty.get());let state=tools(&reopened);assert_eq!(state["scraps"][0]["text"],removed);assert_eq!(state["notes"].as_array().unwrap().last().unwrap()["text"],manual);assert_eq!(state["reactions"].as_array().unwrap().len(),2);
    eprintln!("Notebook tools: stale anchors and pending-save close");
    // Cutting-room navigation saves or discards the active draft before reading history.
    edit(&rp,"Changed after the bookmark. ");let changed=rp.frame.borrow()["document"].clone();select(&reopened,SaveChoice::Save);reopened.cutting_room();wait(||reopened.current_widget().is_some_and(|w|has_label(&w,"Cutting-room floor")));assert_eq!(rpc(&reopened.client,"load_scene",json!({"sceneId":id}))["scene"]["document"],changed);assert!(!rp.dirty.get());let before=navigation(&rp);
    // Exact document hashes prevent old bookmarks from moving the current caret.
    reopened.return_to_bookmark();wait(||reopened.status.text().starts_with("Reminder kept; the old location was not applied."));assert_eq!(navigation(&rp),before);assert!(!rp.dirty.get());
    let reaction=&state["reactions"][0];reopened.jump_to_passage(id.clone(),reaction["revisionId"].as_str().unwrap().into(),reaction["quote"].as_str().unwrap().into(),reaction["start"].as_u64().unwrap() as usize);wait(||reopened.status.text().starts_with("This passage has changed since it was marked."));assert_eq!(navigation(&rp),before);assert!(!rp.dirty.get());
    edit(&rp,"Discard this temporary cut. ");select(&reopened,SaveChoice::Discard);reopened.cutting_room();wait(||reopened.current_widget().is_some_and(|w|has_label(&w,"Cutting-room floor")));assert_eq!(rp.frame.borrow()["document"],changed);assert!(!rp.dirty.get());
    // Closing waits for an already-requested notebook mutation to reach storage.
    let complete=Rc::new(Cell::new(false));let done=complete.clone();let close_note=json!({"id":uuid::Uuid::new_v4().to_string(),"sceneId":id,"text":"Saved before closing","resolved":false});
    reopened.update_notebook_tools(move|value|{value["notes"].as_array_mut().unwrap().push(close_note.clone());Ok(())},move|reply|{reply.unwrap();done.set(true);});assert!(reopened.notebook_pending.get()>0);reopened.close();assert!(!reopened.desk.is_closed());wait(||windows()==0);assert!(complete.get());
    let final_open=Workspace::new_with_focus(&data,&cache,binaries,false).unwrap();wait(||final_open.active.borrow().as_ref().is_some_and(|p|p.open.get() && p.pending.get()==0));assert!(tools(&final_open)["notes"].as_array().unwrap().iter().any(|n|n["text"]=="Saved before closing"));
    // A rejected write must cancel a waiting close, preserving the saved notebook.
    eprintln!("Notebook tools: failed save aborts close, capture pending close persists scrap");
    let saved_tools=tools(&final_open);let failure=Rc::new(RefCell::new(None));let reported=failure.clone();
    final_open.update_notebook_tools(|value|{let duplicate=value["notes"][0].clone();value["notes"].as_array_mut().unwrap().push(duplicate);Ok(())},move|reply|*reported.borrow_mut()=Some(reply));
    assert!(final_open.notebook_pending.get()>0);final_open.close();assert!(!final_open.desk.is_closed());wait(||failure.borrow().is_some() && !final_open.desk.busy.get());assert!(failure.borrow().as_ref().unwrap().is_err());assert_eq!(windows(),1);assert!(final_open.window.is_sensitive());assert!(!final_open.desk.is_closed());assert!(final_open.status.text().contains("This window stays open."));assert_eq!(tools(&final_open),saved_tools);
    // keep_scrap first asks the editor to capture selection. Close must wait for
    // that capture AND its subsequent notebook write, even before the latter starts.
    let fp=final_open.active.borrow().as_ref().unwrap().clone();select_text(&fp,[0,0],[0,5]);let quote=request(&fp,json!({"type":"snapshot"}),false)["selectedText"].as_str().unwrap().to_owned();let count=saved_tools["scraps"].as_array().unwrap().len();
    final_open.keep_scrap();assert!(fp.pending.get()>0);assert_eq!(final_open.notebook_pending.get(),0);final_open.close();assert!(!final_open.desk.is_closed());wait(||windows()==0);
    let verified=Workspace::new_with_focus(&data,&cache,binaries,false).unwrap();wait(||verified.active.borrow().as_ref().is_some_and(|p|p.open.get() && p.pending.get()==0));let persisted=tools(&verified);assert_eq!(persisted["scraps"].as_array().unwrap().len(),count+1);assert_eq!(persisted["scraps"].as_array().unwrap().last().unwrap()["text"],quote);assert_eq!(persisted["notes"],saved_tools["notes"]);verified.close();wait(||windows()==0);
}
