//! Runs inside the existing opt-in, single-threaded GTK/worker workflow.
use super::*;

fn text_view(widget: &impl IsA<gtk::Widget>) -> Option<gtk::TextView> {
    if let Ok(view) = widget.as_ref().clone().downcast::<gtk::TextView>() { return Some(view); }
    if let Ok(container) = widget.as_ref().clone().downcast::<gtk::Container>() { for child in container.children() { if let Some(view) = text_view(&child) { return Some(view); } } } None
}
fn content(widget: &impl IsA<gtk::Widget>) -> String {
    let Some(view) = text_view(widget) else { return String::new(); }; assert!(!view.is_editable()); let buffer = view.buffer().unwrap(); buffer.text(&buffer.start_iter(), &buffer.end_iter(), true).unwrap_or_default().to_string()
}
fn combo(widget: &impl IsA<gtk::Widget>, tooltip: &str) -> Option<gtk::ComboBoxText> {
    if let Ok(combo) = widget.as_ref().clone().downcast::<gtk::ComboBoxText>() { if combo.tooltip_text().as_deref() == Some(tooltip) { return Some(combo); } }
    if let Ok(container) = widget.as_ref().clone().downcast::<gtk::Container>() { for child in container.children() { if let Some(combo) = combo(&child, tooltip) { return Some(combo); } } } None
}
fn check(widget: &impl IsA<gtk::Widget>, caption: &str) -> Option<gtk::CheckButton> {
    if let Ok(button) = widget.as_ref().clone().downcast::<gtk::CheckButton>() { if button.label().as_deref() == Some(caption) { return Some(button); } }
    if let Ok(container) = widget.as_ref().clone().downcast::<gtk::Container>() { for child in container.children() { if let Some(button) = check(&child, caption) { return Some(button); } } } None
}
fn label_contains(widget:&impl IsA<gtk::Widget>,text:&str)->bool{
    if let Ok(label)=widget.as_ref().clone().downcast::<gtk::Label>(){if label.text().contains(text){return true;}}
    widget.as_ref().clone().downcast::<gtk::Container>().ok().is_some_and(|c|c.children().iter().any(|w|label_contains(w,text)))
}
fn opened(a: &Rc<Workspace>, scene: &str) {
    wait(|| a.active.borrow().as_ref().is_some_and(|p| p.scene_id == scene && p.open.get() && p.pending.get() == 0));
}
fn planning(first: &str, second: &str, third: &str) -> Value {
    json!({"schemaVersion":1,"fields":[],"records":[{"id":"hero","kind":"character","name":"نور 明","aliases":[],"notes":"Authored hero notes","links":[{"kind":"scene","id":first}],"values":{}},{"id":"future-person","kind":"character","name":"Future person","aliases":[],"notes":"","links":[],"values":{}}],"facts":[{"id":"known","subjectId":"hero","property":"Name","value":"EXACT مِ 中文 \u{e000} 👩🏽‍🚀","status":"fact","confirmed":true,"observerId":null,"source":"Author source","eventId":"old-event","untilEventId":null},{"id":"future-fact","subjectId":"future-person","property":"SECRET FUTURE PROPERTY","value":"SECRET FUTURE VALUE","status":"proposal","confirmed":false,"observerId":null,"source":"","eventId":null,"untilEventId":null}],"events":[{"id":"old-event","title":"Old event","notes":"Authored occurrence note","sceneIds":[second],"laneIds":["main"],"time":{"kind":"fixed","day":1,"label":""}}],"readerReveals":[{"id":"first-reveal","factId":"known","eventId":null,"scene":{"kind":"scene","id":first},"position":2,"notes":"Explicit visible reveal"},{"id":"event-reveal","eventId":"old-event","factId":null,"scene":{"kind":"scene","id":second},"position":1,"notes":"Flashback told later"},{"id":"future-reveal","factId":"future-fact","eventId":null,"scene":{"kind":"scene","id":third},"position":0,"notes":"SECRET FUTURE REVEAL"}],"knowledge":[{"id":"knowledge","characterId":"hero","factId":"known","stance":"knows","factSnapshot":{"subjectId":"hero","property":"Name","value":"EXACT مِ 中文 \u{e000} 👩🏽‍🚀"},"interpretation":"","source":"Authored knowledge source","scope":{"kind":"scenes","sceneIds":[first]}}],"plotPoints":[{"id":"setup","title":"Visible setup","notes":"An authored clue","links":[{"kind":"scene","id":first}],"requiresPayoff":true,"scopeClosed":false,"payoffId":"payoff"},{"id":"payoff","title":"SECRET FUTURE PAYOFF","notes":"SECRET FUTURE PLOT NOTE","links":[{"kind":"scene","id":third}],"requiresPayoff":false,"scopeClosed":false,"payoffId":null}],"boards":[],"lanes":[{"id":"main","name":"Main","kind":"main"}],"constraints":[],"issues":[]})
}

pub(super) fn run(binaries: &Path) {
    assert_eq!(windows(), 0); let fixture = tempfile::tempdir().unwrap(); let data = fixture.path().join("projection-data"); let cache = fixture.path().join("projection-cache"); std::fs::create_dir_all(&data).unwrap(); std::fs::create_dir_all(&cache).unwrap();
    let companion = if binaries.join("novel-companion-rust").is_file() { "novel-companion-rust" } else { "novel-companion" }; let client = Process::start(&binaries.join(companion), &data, true).unwrap();
    let project = rpc(&client, "create_project", json!({"title":"Projection fixture"})); let pid = project["id"].as_str().unwrap().to_owned(); let chapter = rpc(&client, "create_chapter", json!({"projectId":pid,"title":"Chapter"})); let cid = chapter["id"].as_str().unwrap();
    let mut records = Vec::new();
    for title in ["First · مِ 中文", "Second", "Third"] {
        records.push(rpc(&client, "create_scene", json!({"chapterId":cid,"scene":{"schemaVersion":1,"title":title,"document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"نور 明 appears in this synthetic prose. Body names do not establish authored presence."}]}]}}})));
    }
    let first = records[0]["id"].as_str().unwrap().to_owned(); let second = records[1]["id"].as_str().unwrap().to_owned(); let third = records[2]["id"].as_str().unwrap().to_owned();
    let plan = planning(&first, &second, &third); let saved_plan = rpc(&client, "save_project_data", json!({"projectId":pid,"kind":"planning","payload":plan,"parent":null}));
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap(); opened(&a, &first); let pane = a.active.borrow().as_ref().unwrap().clone();
    eprintln!("Projection pages: guarded navigation and exact disclosure");
    edit(&pane, "UNSAVED draft مِ "); let unsaved = pane.frame.borrow()["document"].clone();
    for open in [Workspace::reader_rehearsal, Workspace::character_cut, Workspace::scene_references] {
        select(&a, SaveChoice::Cancel); open(&a); assert_eq!(a.current_widget(), Some(pane.widget.clone().upcast())); assert!(pane.dirty.get()); assert_eq!(pane.frame.borrow()["document"], unsaved);
    }
    select(&a, SaveChoice::Discard); a.reader_rehearsal(); wait(|| a.current_widget().is_some_and(|w| content(&w).contains("Explicit visible reveal"))); assert!(!pane.dirty.get()); let reading = a.current_widget().unwrap();
    assert!(!content(&reading).contains("SECRET FUTURE")); assert!(content(&reading).contains("EXACT مِ 中文 \u{e000} 👩🏽‍🚀"));
    let progress = combo(&reading, "Progress scene").unwrap(); let position = find_entry(&reading).unwrap(); position.set_text("1"); assert!(!content(&reading).contains("EXACT مِ 中文")); position.set_text("3"); assert!(content(&reading).contains("EXACT مِ 中文"));
    find_button(&reading, "Next scene").unwrap().clicked(); assert_eq!(progress.active_id().as_deref(), Some(second.as_str())); assert_eq!(position.text(), ""); assert!(content(&reading).contains("Old event")); assert!(!content(&reading).contains("SECRET FUTURE"));
    find_button(&reading, "Next scene").unwrap().clicked(); assert_eq!(progress.active_id().as_deref(), Some(third.as_str())); assert!(content(&reading).contains("SECRET FUTURE VALUE")); assert!(!content(&reading).contains("Payoff not yet placed"));
    find_button(&reading, "Previous scene").unwrap().clicked(); position.set_text("1"); assert!(!content(&reading).contains("Old event")); find_button(&reading, "Refresh saved planning").unwrap().clicked(); wait(|| content(&reading).contains("Facts\n\n")); assert_eq!(progress.active_id().as_deref(), Some(second.as_str())); assert_eq!(position.text(), "1"); assert!(!content(&reading).contains("Old event")); capture(&a, "native-reader-rehearsal.png");
    a.detach_current(); let reader = last_window(); assert_eq!(reader.current_widget(), Some(reading.clone())); a.close(); wait(|| windows() == 1); find_button(&reading, "Refresh saved planning").unwrap().clicked(); wait(|| content(&reading).contains("Facts\n\n")); assert_eq!(progress.active_id().as_deref(), Some(second.as_str()));
    find_button(&reading, "Open scene in new window").unwrap().clicked(); wait(|| windows() == 2); let writer = last_window(); opened(&writer, &second); assert_eq!(reader.current_widget(), Some(reading)); reader.close(); wait(|| windows() == 1);

    eprintln!("Projection pages: canonical character cuts without prose matching");
    let live_before = writer.panes.borrow().len(); writer.character_cut(); wait(|| writer.current_widget().is_some_and(|w| find_button(&w, "Refresh saved links").is_some() && has_label(&w, "1 / 2 · First · مِ 中文 · Saved authored links"))); let cut = writer.current_widget().unwrap();
    assert_eq!(writer.panes.borrow().len(), live_before); let character = combo(&cut, "Character").unwrap(); let linked = combo(&cut, "Linked scene").unwrap(); assert_eq!(character.active_id().as_deref(), Some("hero")); assert_eq!(linked.model().unwrap().iter_n_children(None), 2);
    find_button(&cut, "Next linked scene").unwrap().clicked(); assert_eq!(linked.active_id().as_deref(), Some(second.as_str())); assert!(!find_button(&cut, "Next linked scene").unwrap().is_sensitive()); find_button(&cut, "Refresh saved links").unwrap().clicked(); wait(|| has_label(&cut, "2 / 2 · Second · Saved authored links")); assert_eq!(linked.active_id().as_deref(), Some(second.as_str()));
    character.set_active_id(Some("future-person")); assert_eq!(linked.active_id().as_deref(), Some(third.as_str())); assert_eq!(linked.model().unwrap().iter_n_children(None), 1); character.set_active_id(Some("hero")); assert_eq!(linked.model().unwrap().iter_n_children(None), 2); capture(&writer, "native-character-cut.png");
    find_button(&cut, "Open scene in new window").unwrap().clicked(); wait(|| windows() == 2); let opened_scene = last_window(); opened(&opened_scene, &first); assert_eq!(writer.current_widget(), Some(cut)); opened_scene.close(); wait(|| windows() == 1);

    eprintln!("Projection pages: pinned/followed references and historical knowledge");
    writer.scene_references(); wait(|| writer.current_widget().is_some_and(|w| content(&w).contains("Story occurrence events") && content(&w).contains("Old event · Day 1"))); let reference = writer.current_widget().unwrap(); let reference_scene = combo(&reference, "Reference scene").unwrap(); let pin = check(&reference, "Pin scene").unwrap(); assert!(!pin.is_active()); assert_eq!(reference_scene.active_id().as_deref(), Some(second.as_str()));
    reference_scene.set_active_id(Some(&first)); assert!(pin.is_active()); assert!(content(&reference).contains("knows · Applies")); assert!(content(&reference).contains("Reveal position 3")); find_button(&reference, "Open scene in new window").unwrap().clicked(); wait(|| windows() == 2); let writing = last_window(); opened(&writing, &first);
    writing.open_scene(&second, false); opened(&writing, &second); assert_eq!(reference_scene.active_id().as_deref(), Some(first.as_str())); find_button(&reference, "Follow current scene").unwrap().clicked(); assert!(!pin.is_active()); assert_eq!(reference_scene.active_id().as_deref(), Some(second.as_str()));
    writing.open_scene(&third, false); opened(&writing, &third); assert_eq!(reference_scene.active_id().as_deref(), Some(third.as_str())); assert!(!content(&reference).contains("نور 明"));
    reference_scene.set_active_id(Some(&first)); writing.open_scene(&second, false); opened(&writing, &second); assert_eq!(reference_scene.active_id().as_deref(), Some(first.as_str())); assert!(pin.is_active());
    let before_read = rpc(&writer.client, "load_project_data", json!({"projectId":pid,"kind":"planning"})); assert_eq!(before_read, saved_plan);
    let mut changed = plan.clone(); changed["facts"][0]["value"] = json!("New canon value"); let current = rpc(&writer.client, "save_project_data", json!({"projectId":pid,"kind":"planning","payload":changed,"parent":saved_plan["revisionId"]}));
    find_button(&reference, "Refresh saved references").unwrap().clicked(); wait(|| content(&reference).contains("Referenced fact changed")); assert!(content(&reference).contains("Name: New canon value")); assert!(content(&reference).contains("Recorded fact: Name: EXACT مِ 中文")); assert_eq!(reference_scene.active_id().as_deref(), Some(first.as_str())); assert!(pin.is_active()); capture(&writer, "native-scene-references.png");
    writer.detach_current(); let detached = last_window(); assert_eq!(detached.current_widget(), Some(reference.clone())); writer.close(); wait(|| windows() == 2); find_button(&reference, "Refresh saved references").unwrap().clicked(); wait(|| content(&reference).contains("Referenced fact changed"));
    find_button(&reference, "Follow current scene").unwrap().clicked(); assert_eq!(reference_scene.active_id().as_deref(), Some(second.as_str())); writing.open_scene(&first, false); opened(&writing, &first); assert_eq!(reference_scene.active_id().as_deref(), Some(first.as_str())); pin.set_active(true); writing.open_scene(&third, false); opened(&writing, &third); assert_eq!(reference_scene.active_id().as_deref(), Some(first.as_str()));
    assert_eq!(rpc(&detached.client, "load_project_data", json!({"projectId":pid,"kind":"planning"})), current);
    for record in records { let id = record["id"].as_str().unwrap(); let saved = rpc(&detached.client, "load_scene", json!({"sceneId":id})); assert_eq!(saved["revisionId"], record["revisionId"]); assert_eq!(saved["scene"], record["scene"]); }
    // A saved view cannot choose one competing canon head or disclose its values.
    let peer_data=fixture.path().join("peer-data"); std::fs::create_dir_all(&peer_data).unwrap();
    let peer=Process::start(&binaries.join(companion),&peer_data,true).unwrap();
    let transfer=|from:&Process,to:&Process|{let cursor=rpc(from,"sync_cursor",json!({}));let local=rpc(to,"sync_cursor",json!({}));let revisions=rpc(from,"pull_revisions",json!({"projectId":pid,"cursor":{"epoch":cursor["epoch"],"sequence":0},"limit":1000}));rpc(to,"ingest_revisions",json!({"projectId":pid,"expectedEpoch":local["epoch"],"remoteEpoch":cursor["epoch"],"manifest":rpc(from,"open_project",json!({"projectId":pid})),"revisions":revisions["revisions"]}));};
    transfer(&detached.client,&peer);
    let mut local=current["payload"].clone();local["facts"][0]["source"]=json!("Local disputed claim");
    let local_saved=rpc(&detached.client,"save_project_data",json!({"projectId":pid,"kind":"planning","payload":local,"parent":current["revisionId"]}));
    let mut remote=current["payload"].clone();remote["facts"][0]["value"]=json!("COMPETING SECRET VALUE");
    rpc(&peer,"save_project_data",json!({"projectId":pid,"kind":"planning","payload":remote,"parent":current["revisionId"]}));transfer(&peer,&detached.client);
    let marker=rpc(&detached.client,"sync_cursor",json!({}));
    find_button(&reference,"Refresh saved references").unwrap().clicked();wait(||content(&reference).contains("competing planning"));let withheld=content(&reference);assert!(withheld.contains("Shared World notes"));for claim in ["Name: New canon value","Recorded fact:","COMPETING SECRET VALUE"]{assert!(!withheld.contains(claim),"Ambiguous local canon disclosed: {claim}");}
    detached.reader_rehearsal();wait(||detached.current_widget().is_some_and(|w|label_contains(&w,"competing planning")));assert!(content(&detached.current_widget().unwrap()).is_empty());
    detached.character_cut();wait(||detached.current_widget().is_some_and(|w|label_contains(&w,"competing planning")));assert_eq!(rpc(&detached.client,"sync_cursor",json!({})),marker);
    let heads=rpc(&detached.client,"project_data_heads",json!({"projectId":pid,"kind":"planning"}));
    rpc(&detached.client,"resolve_project_data",json!({"projectId":pid,"kind":"planning","payload":local_saved["payload"],"heads":heads["revisionIds"]}));
    detached.reader_rehearsal();wait(||detached.current_widget().is_some_and(|w|content(&w).contains("New canon value")));assert!(!content(&detached.current_widget().unwrap()).contains("COMPETING SECRET VALUE"));peer.stop();
    writing.close(); wait(|| windows() == 1); detached.close(); wait(|| windows() == 0);
}
