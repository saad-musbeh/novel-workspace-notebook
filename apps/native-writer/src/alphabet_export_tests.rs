use super::*;
use novel_storage::{Chapter, Project, ProjectBundle, ProjectDataSnapshot, Scene, SceneRecord};

fn definition(version:u64) -> Value {
    json!({"schemaVersion":1,"id":"symbols","mappingVersion":version,"name":"Synthetic مِ 中文","scriptId":"synthetic","encoding":"private-use","direction":"rtl","mappings":[{"input":"a","output":"\u{e000}","transliteration":if version==1 {"\u{e001}a"} else {"LATEST"}},{"input":"b","output":"\u{e001}","transliteration":"B"},{"input":"c","output":"\u{f0000}","transliteration":""}],"font":null,"glossary":[],"notes":"Exact e\u{301}"})
}
fn snapshot() -> AlphabetExportSnapshot {
    AlphabetExportSnapshot { bundle:ProjectBundle { project:Project{id:"p".into(),title:"Book مِ".into(),created_at:"2026-01-01".into()}, chapters:vec![Chapter{id:"c".into(),project_id:"p".into(),title:"Chapter".into(),position:0}], scenes:vec![SceneRecord{id:"s".into(),chapter_id:"c".into(),position:0,revision_id:"r".into(),scene:Scene{schema_version:1,title:"Title \u{e000}".into(),document:json!({"type":"doc","content":[{"type":"paragraph","attrs":{"lang":"ar","dir":"rtl","lineSpacing":1.5},"content":[{"type":"text","text":"\u{e000}\u{e001}\u{f0000} a e\u{301} مِ 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"},{"type":"underline"}]},{"type":"hardBreak"},{"type":"text","text":"\u{e000}","marks":[{"type":"italic"}]}]}]})}}],chapter_number_start:1,chapter_numbering_revision_id:None,chapter_numbering_conflicted:false }, writing_systems:Some(ProjectDataSnapshot{revision_id:"maps".into(),payload:json!({"schemaVersion":1,"systems":[definition(1),definition(2)],"assignments":[{"scene":{"kind":"scene","id":"s"},"definitionId":"symbols","mappingVersion":1}]})}) }
}
#[test]
fn exact_original_default_and_one_pass_historical_rules_preserve_rich_metadata() {
    let snapshot=snapshot();let before=snapshot.clone();
    let original:Value=serde_json::from_slice(&encode(&snapshot,false).unwrap()).unwrap();
    assert_eq!(original["original"],serde_json::to_value(&snapshot.bundle).unwrap());
    assert_eq!(original["writingSystems"],serde_json::to_value(&snapshot.writing_systems).unwrap());
    assert!(original["transliterated"].is_null());
    let derived:Value=serde_json::from_slice(&encode(&snapshot,true).unwrap()).unwrap();
    let scene=&derived["transliterated"]["scenes"][0];
    assert_eq!(scene["mappingVersion"],1);assert_eq!(scene["sceneRevisionId"],"r");
    assert_eq!(scene["scene"]["document"]["content"][0]["content"][0]["text"],"\u{e001}aB\u{f0000} a e\u{301} مِ 中文 𠀀 👩🏽‍🚀");
    let mut expected=snapshot.bundle.scenes[0].scene.clone();
    expected.document["content"][0]["content"][0]["text"]=json!("\u{e001}aB\u{f0000} a e\u{301} مِ 中文 𠀀 👩🏽‍🚀");
    expected.document["content"][0]["content"][2]["text"]=json!("\u{e001}a");
    assert_eq!(scene["scene"],serde_json::to_value(expected).unwrap());
    assert_eq!(snapshot,before);
}
#[test]
fn unassigned_text_is_literal_and_unavailable_or_malformed_versions_refuse() {
    let mut s=snapshot();s.writing_systems.as_mut().unwrap().payload["assignments"]=json!([]);
    let value:Value=serde_json::from_slice(&encode(&s,true).unwrap()).unwrap();
    assert!(value["transliterated"]["scenes"][0]["definitionId"].is_null());
    assert_eq!(value["transliterated"]["scenes"][0]["scene"],serde_json::to_value(&s.bundle.scenes[0].scene).unwrap());
    s.writing_systems=None;assert!(encode(&s,true).is_ok());
    for mutation in [json!(99),json!("1"),Value::Null] {
        let mut s=snapshot();s.writing_systems.as_mut().unwrap().payload["assignments"][0]["mappingVersion"]=mutation;
        assert!(encode(&s,true).is_err());assert!(encode(&s,false).is_err());
    }
    let mut s=snapshot();s.writing_systems.as_mut().unwrap().payload["systems"][0]["mappings"][0]["output"]=json!("two");
    assert!(encode(&s,true).is_err());
}
#[test]
fn font_license_metadata_is_retained_but_no_bytes_or_font_activation_are_exported() {
    let mut s=snapshot();
    s.writing_systems.as_mut().unwrap().payload["systems"][0]["font"]=json!({"fontAssetId":"fixture-font","family":"Synthetic family","checksum":"0".repeat(64),"format":"ttf","license":{"name":"CC0-1.0 synthetic fixture","source":"Synthetic author","permissionConfirmed":true}});
    let bytes=encode(&s,true).unwrap();let value:Value=serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["writingSystems"]["payload"]["systems"][0]["font"],s.writing_systems.as_ref().unwrap().payload["systems"][0]["font"]);
    assert!(!String::from_utf8(bytes).unwrap().contains("base64"));
    let mut foreign=s.clone();foreign.writing_systems.as_mut().unwrap().payload["systems"][0]["font"]["content"]=json!([0,1,2,3]);
    assert!(encode(&foreign,false).unwrap_err().contains("without resource bytes"));
    s.writing_systems.as_mut().unwrap().payload["systems"][0]["font"]["license"]["permissionConfirmed"]=json!(false);
    assert!(encode(&s,false).is_err());
}
#[test]
fn valid_oversized_transliteration_refuses_incrementally_without_mutating_source_or_file() {
    let mut s=snapshot();
    s.bundle.scenes[0].scene.document["content"][0]["content"][0]["text"]=json!("\u{e000}".repeat(350_000));
    s.writing_systems.as_mut().unwrap().payload["systems"][0]["mappings"][0]["transliteration"]=json!("x".repeat(200));
    let before=s.clone();novel_storage::validate(&s.bundle.scenes[0].scene).unwrap();
    let d=tempfile::tempdir().unwrap();let path=d.path().join("original.json");files::write(&path,b"Existing local file").unwrap();
    let result=encode(&s,true).and_then(|bytes|files::write(&path,&bytes));
    assert!(result.unwrap_err().contains("64 MiB"));assert_eq!(s,before);
    assert_eq!(files::read(&path).unwrap(),"Existing local file");
}

fn pump_until(mut predicate: impl FnMut()->bool) {
    let start=std::time::Instant::now();while !predicate(){while glib::MainContext::default().pending(){glib::MainContext::default().iteration(false);}assert!(start.elapsed()<std::time::Duration::from_secs(20));std::thread::sleep(std::time::Duration::from_millis(3));}
}
fn rpc(client:&Process,op:&str,args:Value)->Value {
    let result=Rc::new(RefCell::new(None));let out=result.clone();client.rpc(op,args,move|r|*out.borrow_mut()=Some(r));pump_until(||result.borrow().is_some());let r=result.borrow_mut().take().unwrap().unwrap();r
}
fn combo(widget:&impl IsA<gtk::Widget>)->Option<gtk::ComboBoxText> {
    if let Ok(c)=widget.as_ref().clone().downcast::<gtk::ComboBoxText>(){return Some(c);}
    widget.as_ref().clone().downcast::<gtk::Container>().ok().and_then(|c|c.children().iter().find_map(combo))
}
fn answer(include:bool,response:gtk::ResponseType) {
    glib::timeout_add_local_once(std::time::Duration::from_millis(30),move||{
        let dialog=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Dialog>().ok()).find(|d|d.title().as_deref()==Some("Alphabet manuscript export")&&d.is_visible()).unwrap();
        let combo=combo(&dialog).unwrap();assert_eq!(combo.active_id().as_deref(),Some("original"));
        if include {combo.set_active_id(Some("transliterated"));}dialog.response(response);
    });
}
/// Called only by the parent's serialized, opt-in GTK harness.
pub(crate) fn run(binaries:&Path) {
    let d=tempfile::tempdir().unwrap();let data=d.path().join("data");let cache=d.path().join("cache");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(&cache).unwrap();
    let name=if binaries.join("novel-companion-rust").is_file(){"novel-companion-rust"}else{"novel-companion"};
    let client=Process::start(&binaries.join(name),&data,true).unwrap();let novel=rpc(&client,"create_novel",json!({"title":"Alphabet export fixture"}));
    let pid=novel["project"]["id"].as_str().unwrap();let sid=novel["scene"]["id"].as_str().unwrap();
    let mut scene=novel["scene"]["scene"].clone();scene["document"]=snapshot().bundle.scenes[0].scene.document.clone();
    let saved=rpc(&client,"save_scene",json!({"sceneId":sid,"scene":scene,"parent":novel["scene"]["revisionId"]}));
    let mut doc=snapshot().writing_systems.unwrap().payload;doc["assignments"][0]["scene"]["id"]=json!(sid);
    rpc(&client,"save_project_data",json!({"projectId":pid,"kind":"writingSystems","payload":doc,"parent":null}));
    let a=Workspace::build(client,&cache,binaries,false,false).unwrap();pump_until(||a.active.borrow().as_ref().is_some_and(|p|p.open.get()&&p.pending.get()==0));
    let marker=rpc(&a.client,"sync_cursor",json!({}));
    glib::timeout_add_local_once(std::time::Duration::from_millis(30),||{
        let dialog=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Dialog>().ok()).find(|d|d.title().as_deref()==Some("Alphabet manuscript export")&&d.is_visible()).unwrap();
        assert_eq!(combo(&dialog).unwrap().active_id().as_deref(),Some("original"));dialog.response(gtk::ResponseType::Cancel);
    });
    a.alphabet_export();
    // Accepted real action: dialog -> project decisions -> RPC -> worker -> atomic write.
    for include in [false,true] {
        let path=d.path().join(if include{"derived.alphabet-manuscript.json"}else{"original.alphabet-manuscript.json"});
        DESTINATIONS.with(|q|q.borrow_mut().push_back(Some(path.clone())));
        answer(include,gtk::ResponseType::Accept);a.alphabet_export();pump_until(||path.is_file());
        let exported:Value=serde_json::from_str(&files::read(&path).unwrap()).unwrap();assert_eq!(exported["original"]["scenes"][0]["scene"],saved["scene"]);
        assert_eq!(exported["transliterated"].is_null(),!include);
        if include {assert_eq!(exported["transliterated"]["scenes"][0]["mappingVersion"],1);}
    }
    assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);
    let pane=a.active.borrow().clone().unwrap();let parent=pane.parent.borrow().clone();
    let before=pane.frame.borrow()["document"].clone();
    pane.operation(json!({"type":"text","text":"Unsaved export choice مِ 中文"}),true);pump_until(||pane.pending.get()==0&&pane.dirty.get());
    let draft=pane.frame.borrow()["document"].clone();let canceled=d.path().join("canceled.json");
    DESTINATIONS.with(|q|q.borrow_mut().push_back(Some(canceled.clone())));a.desk.choices.borrow_mut().push_back(desk::SaveChoice::Cancel);
    answer(false,gtk::ResponseType::Accept);a.alphabet_export();
    pump_until(||a.desk.choices.borrow().is_empty());
    assert!(!canceled.exists());assert_eq!(*pane.parent.borrow(),parent);assert_eq!(pane.frame.borrow()["document"],draft);assert!(pane.dirty.get());
    assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);DESTINATIONS.with(|q|q.borrow_mut().clear());
    pane.operation(json!({"type":"command","command":"undo"}),true);pump_until(||pane.pending.get()==0);assert_eq!(pane.frame.borrow()["document"],before);
    let approved=d.path().join("manual-save.json");DESTINATIONS.with(|q|q.borrow_mut().push_back(Some(approved.clone())));
    a.desk.choices.borrow_mut().push_back(desk::SaveChoice::Save);answer(false,gtk::ResponseType::Accept);a.alphabet_export();pump_until(||approved.is_file());
    let saved=rpc(&a.client,"load_scene",json!({"sceneId":sid}));let marker=rpc(&a.client,"sync_cursor",json!({}));
    let value:Value=serde_json::from_str(&files::read(&approved).unwrap()).unwrap();assert_eq!(value["original"]["scenes"][0]["revisionId"],saved["revisionId"]);assert_eq!(value["original"]["scenes"][0]["scene"],saved["scene"]);assert!(!pane.dirty.get());
    let failed=d.path().join("missing-folder/export.json");DESTINATIONS.with(|q|q.borrow_mut().push_back(Some(failed.clone())));
    answer(false,gtk::ResponseType::Accept);a.alphabet_export();pump_until(||DESTINATIONS.with(|q|q.borrow().is_empty())&&!a.status.text().starts_with("Preparing"));
    assert!(!failed.exists());assert!(!a.status.text().starts_with("Exported"));assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);
    // Finished worker result cannot target a switched generation or discard input
    // that arrived after the author's project approval.
    let late=d.path().join("late.json");DESTINATIONS.with(|q|q.borrow_mut().push_back(Some(late.clone())));
    let delivered=Rc::new(Cell::new(false));let out=delivered.clone();let owner=a.clone();let typed=pane.clone();let generation=a.desk.generation.get();let parent=pane.parent.borrow().clone();let before=pane.frame.borrow()["document"].clone();
    BEFORE_DELIVERY.with(|hook|*hook.borrow_mut()=Some(Box::new(move||{typed.operation(json!({"type":"text","text":"After export approval مِ 中文"}),true);owner.desk.generation.set(generation+1);out.set(true);})));
    answer(true,gtk::ResponseType::Accept);a.alphabet_export();pump_until(||delivered.get()&&pane.pending.get()==0);
    assert!(!late.exists());assert!(pane.dirty.get());assert_eq!(*pane.parent.borrow(),parent);assert!(novel_storage::plain_text(&pane.frame.borrow()["document"]).contains("After export approval"));
    assert_eq!(rpc(&a.client,"sync_cursor",json!({})),marker);assert_eq!(rpc(&a.client,"load_scene",json!({"sceneId":sid})),saved);
    pane.operation(json!({"type":"command","command":"undo"}),true);pump_until(||pane.pending.get()==0);assert_eq!(pane.frame.borrow()["document"],before);
    a.desk.generation.set(generation);DESTINATIONS.with(|q|q.borrow_mut().clear());
    // A closed window also rejects an already-computed worker result.
    let closed=d.path().join("closed.json");DESTINATIONS.with(|q|q.borrow_mut().push_back(Some(closed.clone())));
    let owner=a.clone();BEFORE_DELIVERY.with(|hook|*hook.borrow_mut()=Some(Box::new(move||owner.close())));
    a.desk.choices.borrow_mut().push_back(desk::SaveChoice::Discard);answer(false,gtk::ResponseType::Accept);a.alphabet_export();pump_until(||a.desk.is_closed());
    assert!(!closed.exists());DESTINATIONS.with(|q|q.borrow_mut().clear());
    let probe=Process::start(&binaries.join(name),&data,true).unwrap();
    assert_eq!(rpc(&probe,"sync_cursor",json!({})),marker);assert_eq!(rpc(&probe,"load_scene",json!({"sceneId":sid})),saved);probe.stop();

}
