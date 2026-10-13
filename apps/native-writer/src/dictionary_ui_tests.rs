use super::*;
fn labels(widget: &impl IsA<gtk::Widget>, wanted: &str) -> bool {
    if let Ok(label) = widget.as_ref().clone().downcast::<gtk::Label>() {
        if label.text().contains(wanted) { return true; }
    }
    widget.as_ref().clone().downcast::<gtk::Container>().ok()
        .is_some_and(|c| c.children().iter().any(|child| labels(child, wanted)))
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
