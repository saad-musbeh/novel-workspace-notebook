use super::*;
fn labels(widget: &impl IsA<gtk::Widget>, wanted: &str) -> bool {
    if let Ok(label) = widget.as_ref().clone().downcast::<gtk::Label>() {
        if label.text().contains(wanted) { return true; }
    }
    widget.as_ref().clone().downcast::<gtk::Container>().ok()
        .is_some_and(|c| c.children().iter().any(|child| labels(child, wanted)))
}
fn select_word(pane:&Rc<EditorPane>,start:usize,end:usize) {
    let capture=Rc::new(RefCell::new(None));let out=capture.clone();pane.request(json!({"type":"command","command":"captureNavigation"}),false,move|r|*out.borrow_mut()=Some(r.unwrap()["navigationCapture"].clone()));wait(||capture.borrow().is_some());
    let mut bookmark=capture.borrow_mut().take().unwrap();bookmark["cursor"]=json!([0,end]);bookmark["anchor"]=json!([0,start]);let done=Rc::new(Cell::new(false));let out=done.clone();pane.request(json!({"type":"command","command":"restoreNavigation","bookmark":bookmark}),false,move|r|{r.unwrap();out.set(true);});wait(||done.get());
}
fn open_preview(a:&Rc<Workspace>,button:&gtk::Button)->gtk::Widget {
    let previous=a.current_widget();button.clicked();
    wait(||a.current_widget().as_ref().is_some_and(|w|Some(w)!=previous.as_ref()&&labels(w,"Original paragraph:")));
    a.current_widget().unwrap()
}
pub(super) fn synonym_workflow(binaries:&Path) {
    let fixture=tempfile::tempdir().unwrap();let data=fixture.path().join("data");let cache=fixture.path().join("cache");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(&cache).unwrap();
    let executable=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};let client=Process::start(&binaries.join(executable),&data,true).unwrap();
    let novel=rpc(&client,"create_novel",json!({"title":"Captured synonym fixture"}));let pid=novel["project"]["id"].as_str().unwrap();let sid=novel["scene"]["id"].as_str().unwrap();
    let document=json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"مِ ","marks":[{"type":"bold"}]},{"type":"text","text":"bank","marks":[{"type":"italic"},{"type":"underline"}]},{"type":"hardBreak"},{"type":"text","text":"中文 bank 👩🏽‍🚀"}]}]});
    rpc(&client,"save_scene",json!({"sceneId":sid,"scene":{"schemaVersion":1,"title":"Exact passage","document":document},"parent":novel["scene"]["revisionId"]}));
    let pack=json!({"schemaVersion":1,"id":"synthetic","name":"Fixture glossary","language":"en","variant":"test","version":"1.0.0","source":"Synthetic author","license":"CC0-1.0","attribution":"Synthetic author","operations":["dictionary","synonyms"],"entries":[{"term":"bank","senses":[{"id":"finance","definition":"A place for money.","partOfSpeech":"noun","synonyms":["depository"],"related":["deposit"],"translations":[{"language":"ar","text":"مَصْرِف"}]},{"id":"river","definition":"The land beside a river.","partOfSpeech":"noun","synonyms":["shore"],"related":["river"],"translations":[]}],"notes":"Distinct senses."}],"checksum":"e43b256ec9fd243a39ae072c5a1f4cc1896aa23391662b30b900a1b00749cc0a"});
    rpc(&client,"save_project_data",json!({"projectId":pid,"kind":"languagePacks","payload":{"schemaVersion":1,"packs":[pack]},"parent":null}));
    let a=Workspace::build(client,&cache,binaries,false,false).unwrap();wait(||a.active.borrow().as_ref().is_some_and(|p|p.open.get()&&p.pending.get()==0));let pane=a.active.borrow().clone().unwrap();
    pane.operation(json!({"type":"text","text":"Draft "}),true);wait(||pane.pending.get()==0&&pane.dirty.get());let original=pane.frame.borrow()["document"].clone();let parent=pane.parent.borrow().clone();select_word(&pane,9,13);
    let marker=rpc(&a.client,"sync_cursor",json!({}));let saved=rpc(&a.client,"load_scene",json!({"sceneId":sid}));
    a.dictionary();let dictionary=a.current_widget().unwrap();wait(||labels(&dictionary,"1 saved sources"));find_entry(&dictionary).unwrap().set_text("bank");assert!(labels(&dictionary,"Related words: river"));assert!(labels(&dictionary,"Translations: ar:"));
    let preview=find_button(&dictionary,"Preview in captured passage: shore").unwrap();let canceled=open_preview(&a,&preview);assert!(labels(&canceled,"Sense 2 · noun"));assert!(labels(&canceled,"inflection or grammar manually"));find_button(&canceled,"Cancel preview").unwrap().clicked();wait(||a.pages.page_num(&canceled).is_none());assert_eq!(pane.frame.borrow()["document"],original);
    let context=open_preview(&a,&preview);find_button(&context,"Apply to unsaved draft").unwrap().clicked();wait(||labels(&context,"Applied to the unsaved draft"));
    let after=pane.frame.borrow()["document"].clone();assert_eq!(novel_storage::plain_text(&after),"Draft مِ shore\n中文 bank 👩🏽‍🚀");let chosen=after["content"][0]["content"].as_array().unwrap().iter().find(|n|n["text"]=="shore").unwrap();assert_eq!(chosen["marks"],json!([{"type":"italic"},{"type":"underline"}]));assert_eq!(*pane.parent.borrow(),parent);assert!(pane.dirty.get());assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);
    find_button(&context,"Undo this replacement").unwrap().clicked();wait(||labels(&context,"Replacement undone"));assert_eq!(pane.frame.borrow()["document"],original);
    pane.operation(json!({"type":"command","command":"undo"}),true);wait(||pane.pending.get()==0);assert_eq!(pane.frame.borrow()["document"],saved["scene"]["document"]);pane.operation(json!({"type":"command","command":"redo"}),true);wait(||pane.pending.get()==0);assert_eq!(pane.frame.borrow()["document"],original);select_word(&pane,9,13);
    let stale=open_preview(&a,&preview);select_word(&pane,17,21);find_button(&stale,"Apply to unsaved draft").unwrap().clicked();wait(||labels(&stale,"selection or saved parent changed"));assert_eq!(pane.frame.borrow()["document"],original);
    select_word(&pane,9,13);let rich_stale=open_preview(&a,&preview);pane.operation(json!({"type":"command","command":"bold"}),true);wait(||pane.pending.get()==0);assert_eq!(novel_storage::plain_text(&pane.frame.borrow()["document"]),novel_storage::plain_text(&original));find_button(&rich_stale,"Apply to unsaved draft").unwrap().clicked();wait(||labels(&rich_stale,"rich draft, selection or saved parent changed"));pane.operation(json!({"type":"command","command":"undo"}),true);wait(||pane.pending.get()==0);assert_eq!(pane.frame.borrow()["document"],original);
    select_word(&pane,9,13);let cached_page=open_preview(&a,&preview);a.pages.remove_page(a.pages.page_num(&pane.widget));assert!(a.panes.borrow().get(sid).is_some_and(|p|Rc::ptr_eq(p,&pane)));find_button(&cached_page,"Apply to unsaved draft").unwrap().clicked();wait(||labels(&cached_page,"Applied to the unsaved draft"));find_button(&cached_page,"Undo this replacement").unwrap().clicked();wait(||labels(&cached_page,"Replacement undone"));assert_eq!(pane.frame.borrow()["document"],original);
    a.install(&pane.widget,"Captured source",false);select_word(&pane,9,13);let detached_page=open_preview(&a,&preview);a.detach_current();let context_owner=desk::workspace_for_widget(&detached_page).unwrap();
    // Move the original rich pane to another owner, then close the initial window.
    a.install(&pane.widget,"Captured source",false);a.detach_current();let source_owner=desk::workspace_for_widget(pane.widget.upcast_ref()).unwrap();a.close();wait(||a.desk.is_closed());
    find_button(&detached_page,"Apply to unsaved draft").unwrap().clicked();wait(||labels(&detached_page,"Applied to the unsaved draft"));assert_eq!(pane.frame.borrow()["document"],after);find_button(&detached_page,"Undo this replacement").unwrap().clicked();wait(||labels(&detached_page,"Replacement undone"));assert_eq!(pane.frame.borrow()["document"],original);assert_eq!(*pane.parent.borrow(),parent);
    assert_eq!(rpc(&context_owner.client,"sync_cursor",json!({})),marker);assert_eq!(rpc(&context_owner.client,"load_scene",json!({"sceneId":sid})),saved);
    context_owner.close();wait(||context_owner.desk.is_closed());source_owner.desk.choices.borrow_mut().push_back(SaveChoice::Discard);source_owner.close();wait(||source_owner.desk.is_closed());
}
fn case_toggle(widget: &impl IsA<gtk::Widget>) -> Option<gtk::CheckButton> {
    if let Ok(button) = widget.as_ref().clone().downcast::<gtk::CheckButton>() { return Some(button); }
    widget.as_ref().clone().downcast::<gtk::Container>().ok()
        .and_then(|c| c.children().iter().find_map(case_toggle))
}
pub(super) fn run(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap(); let data = fixture.path().join("dictionary-data");
    let cache = fixture.path().join("dictionary-cache"); std::fs::create_dir_all(&data).unwrap(); std::fs::create_dir_all(&cache).unwrap();
    let executable = if binaries.join("novel-companion-rust").is_file() { "novel-companion-rust" } else { "novel-companion" };
    let client = Process::start(&binaries.join(executable), &data, true).unwrap();
    let novel = rpc(&client, "create_novel", json!({"title":"Local lookup"})); let pid = novel["project"]["id"].as_str().unwrap(); let sid = novel["scene"]["id"].as_str().unwrap();
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| a.active.borrow().as_ref().is_some_and(|p| p.open.get() && p.pending.get() == 0));
    let saved_scene = rpc(&a.client, "load_scene", json!({"sceneId":sid}));
    a.dictionary(); let widget = a.current_widget().unwrap(); wait(|| labels(&widget, "0 saved sources"));
    let query = find_entry(&widget).unwrap(); query.set_text("bank"); assert!(labels(&widget, "No saved dictionary"));
    // The fixed checksum is independently reproduced by the parser's pure test.
    let pack = json!({"schemaVersion":1,"id":"synthetic","name":"Fixture glossary","language":"en","variant":"test","version":"1.0.0","source":"Synthetic author","license":"CC0-1.0","attribution":"Synthetic author","operations":["dictionary","synonyms"],"entries":[{"term":"bank","senses":[{"id":"finance","definition":"A place for money.","partOfSpeech":"noun","synonyms":["depository"],"related":["deposit"],"translations":[{"language":"ar","text":"مَصْرِف"}]},{"id":"river","definition":"The land beside a river.","partOfSpeech":"noun","synonyms":["shore"],"related":["river"],"translations":[]}],"notes":"Distinct senses."}],"checksum":"e43b256ec9fd243a39ae072c5a1f4cc1896aa23391662b30b900a1b00749cc0a"});
    let source = rpc(&a.client, "save_project_data", json!({"projectId":pid,"kind":"languagePacks","payload":{"schemaVersion":1,"packs":[pack]},"parent":null}));
    let marker = rpc(&a.client, "sync_cursor", json!({})); find_button(&widget, "Reload saved dictionaries").unwrap().clicked();
    wait(|| labels(&widget, "1 saved sources")); assert!(labels(&widget, "A place for money.")); assert!(labels(&widget, "The land beside a river."));
    assert!(labels(&widget, "Synonyms for this sense: depository")); assert!(labels(&widget, "Synonyms for this sense: shore")); assert!(labels(&widget, "مَصْرِف"));
    assert!(labels(&widget, "Declared license: CC0-1.0")); query.set_text("BANK"); assert!(labels(&widget, "No matching entry"));
    case_toggle(&widget).unwrap().set_active(true); assert!(labels(&widget, "A place for money."));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker); assert_eq!(rpc(&a.client, "load_scene", json!({"sceneId":sid})), saved_scene);
    capture(&a, "notebook-local-dictionary.png");
    a.detach_current(); let detached = last_window(); assert_eq!(detached.current_widget(), Some(widget.clone())); a.close(); wait(|| windows() == 1);
    find_button(&widget, "Reload saved dictionaries").unwrap().clicked(); wait(|| labels(&widget, "1 saved sources"));
    // Corrupt stored pack metadata may be inspected but never supplies meanings.
    let mut broken = source["payload"].clone(); broken["packs"][0]["version"] = json!("1.0.1"); broken["packs"][0]["entries"][0]["senses"][0]["definition"] = json!("Unverified meaning");
    rpc(&detached.client, "save_project_data", json!({"projectId":pid,"kind":"languagePacks","payload":broken,"parent":source["revisionId"]}));
    let marker = rpc(&detached.client, "sync_cursor", json!({})); find_button(&widget, "Reload saved dictionaries").unwrap().clicked();
    wait(|| labels(&widget, "checksum")); assert!(labels(&widget, "0 saved sources")); assert!(!labels(&widget, "Unverified meaning")); assert!(!labels(&widget, "A place for money."));
    assert_eq!(rpc(&detached.client, "sync_cursor", json!({})), marker); assert_eq!(rpc(&detached.client, "load_scene", json!({"sceneId":sid})), saved_scene);
    detached.close(); wait(|| windows() == 0);
}
