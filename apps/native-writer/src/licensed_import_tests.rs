use super::*;
use crate::local_resources::tests::{license_fixture, pack_fixture, synthetic_sfnt};
fn alphabet_doc() -> Value {
    json!({"schemaVersion":1,"systems":[{"schemaVersion":1,"id":"synthetic","mappingVersion":1,"name":"Test alphabet","scriptId":"custom","encoding":"private-use","direction":"ltr","mappings":[{"input":"a","output":"\u{e000}","transliteration":"a"}],"font":null,"glossary":[{"symbol":"مِ 𠀀","meaning":"exact"}],"notes":"synthetic only"}],"assignments":[{"scene":{"kind":"scene","id":"scene"},"definitionId":"synthetic","mappingVersion":1}]})
}
fn draft() -> ImportDraft {
    ImportDraft::checked(
        "novel".into(),
        Value::Null,
        json!({"revisionId":"alph-1","payload":alphabet_doc()}),
        &[],
    )
    .unwrap()
}
#[test]
fn pack_preview_requires_explicit_license_permission_before_real_save_request() {
    let mut d = draft();
    let p = pack_fixture();
    d.preview_pack(&serde_json::to_vec(&p).unwrap()).unwrap();
    assert!(d.begin_save(false).is_err());
    assert!(!d.saving);
    assert_eq!(d.packs, json!({"schemaVersion":1,"packs":[]}));
    let req = d.begin_save(true).unwrap();
    assert_eq!(req.len(), 1);
    assert_eq!(req[0].0, "save_project_data_checked");
    assert_eq!(
        req[0].1,
        json!({"projectId":"novel","kind":"languagePacks","payload":{"schemaVersion":1,"packs":[p]},"parent":null,"dependencies":[]})
    );
    assert!(d.preview_text().contains("Preview only"));
    assert!(d.discard().is_err());
    d.complete_save(Ok(
        json!({"revisionId":"pack-1","payload":req[0].1["payload"]}),
    ))
    .unwrap();
    assert!(d.pending.is_none());
    assert_eq!(d.packs_parent, "pack-1");
}
#[test]
fn font_save_stores_asset_before_immutable_definition_and_preserves_assignment() {
    let mut d = draft();
    d.preview_font(&synthetic_sfnt(false), &license_fixture(), "synthetic@1")
        .unwrap();
    let req = d.begin_save(false).unwrap();
    assert_eq!(req.len(), 2);
    assert_eq!(req[0].0, "put_asset");
    assert_eq!(req[0].1["projectId"], "novel");
    assert_eq!(req[1].0, "save_project_data_checked");
    assert_eq!(req[1].1["parent"], "alph-1");
    assert_eq!(req[1].1["kind"], "writingSystems");
    let candidate = &req[1].1["payload"];
    assert_eq!(candidate["systems"][0], alphabet_doc()["systems"][0]);
    assert_eq!(candidate["assignments"], alphabet_doc()["assignments"]);
    assert_eq!(candidate["systems"][1]["mappingVersion"], 2);
    assert_eq!(
        candidate["systems"][1]["font"]["fontAssetId"],
        req[0].1["asset"]["id"]
    );
    assert_eq!(
        candidate["systems"][1]["glossary"],
        alphabet_doc()["systems"][0]["glossary"]
    );
    d.complete_save(Ok(json!({"revisionId":"alph-2","payload":candidate})))
        .unwrap();
    assert_eq!(d.alphabets_parent, "alph-2");
}
#[test]
fn font_cas_failure_preserves_same_asset_and_exact_candidate_for_retry() {
    let mut d = draft();
    d.preview_font(&synthetic_sfnt(true), &license_fixture(), "synthetic@1")
        .unwrap();
    let first = d.begin_save(false).unwrap();
    d.complete_save(Err("Conflict".into())).unwrap_err();
    assert!(d.pending.is_some());
    assert_eq!(d.alphabets, alphabet_doc());
    assert_eq!(d.begin_save(false).unwrap(), first);
    d.complete_save(Err("Conflict".into())).unwrap_err();
    d.discard().unwrap();
    assert!(d.pending.is_none());
}
#[test]
fn acknowledgement_mismatch_retains_manual_preview() {
    let mut d = draft();
    d.preview_pack(&serde_json::to_vec(&pack_fixture()).unwrap())
        .unwrap();
    d.begin_save(true).unwrap();
    assert!(d
        .complete_save(Ok(
            json!({"revisionId":"wrong","payload":{"schemaVersion":1,"packs":[]}})
        ))
        .is_err());
    assert!(d.pending.is_some());
    assert!(d.packs_parent.is_null());
}
#[test]
fn checked_pack_history_prevents_reusing_an_older_version_with_changed_data() {
    let first = pack_fixture();
    let mut second = first.clone();
    second["version"] = json!("2");
    second["checksum"] = json!(resources::pack_checksum(&second).unwrap());
    let s1 = json!({"revisionId":"p1","payload":{"schemaVersion":1,"packs":[first.clone()]}});
    let s2 = json!({"revisionId":"p2","payload":{"schemaVersion":1,"packs":[second]}});
    let mut d = ImportDraft::checked("novel".into(), s2.clone(), Value::Null, &[s1, s2]).unwrap();
    let mut changed = first;
    changed["attribution"] = json!("different");
    changed["checksum"] = json!(resources::pack_checksum(&changed).unwrap());
    assert!(d
        .preview_pack(&serde_json::to_vec(&changed).unwrap())
        .is_err());
}
#[test]
fn loading_and_preview_refuse_missing_history_invalid_license_or_wrong_definition() {
    let saved = json!({"revisionId":"p1","payload":{"schemaVersion":1,"packs":[pack_fixture()]}});
    assert!(ImportDraft::checked("novel".into(), saved, Value::Null, &[]).is_err());
    let mut d = draft();
    assert!(d
        .preview_font(&synthetic_sfnt(false), &license_fixture(), "missing@1")
        .is_err());
    let mut license = license_fixture();
    license["permissionConfirmed"] = json!(false);
    assert!(d
        .preview_font(&synthetic_sfnt(false), &license, "synthetic@1")
        .is_err());
    assert!(d.pending.is_none());
    assert!(d.begin_save(true).is_err());
}
#[test]
fn history_id_capture_is_complete_unique_bounded_and_sorted() {
    assert_eq!(
        history_ids(&json!([{"id":"b"},{"id":"a"}])).unwrap(),
        vec!["a", "b"]
    );
    assert!(history_ids(&json!([{"id":"a"},{"id":"a"}])).is_err());
    assert!(history_ids(&json!(vec![json!({"id":"a"}); HISTORY_LIMIT + 1])).is_err());
}

#[test]
fn older_alphabet_font_edit_publishes_next_latest_version_without_reinterpretation() {
    let mut d = draft();
    d.preview_font(&synthetic_sfnt(false), &license_fixture(), "synthetic@1")
        .unwrap();
    let first = d.begin_save(false).unwrap();
    d.complete_save(Ok(
        json!({"revisionId":"a2","payload":first[1].1["payload"]}),
    ))
    .unwrap();
    d.preview_font(&synthetic_sfnt(true), &license_fixture(), "synthetic@1")
        .unwrap();
    let next = d.begin_save(false).unwrap();
    assert_eq!(next[1].1["payload"]["systems"][2]["mappingVersion"], 3);
    assert_eq!(
        next[1].1["payload"]["assignments"],
        alphabet_doc()["assignments"]
    );
}

fn wait(condition: impl Fn() -> bool) {
    let start = std::time::Instant::now();
    let context = glib::MainContext::default();
    while !condition() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "Licensed resource UI timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn rpc(client: &Process, op: &str, args: Value) -> Value {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    client.rpc(op, args, move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some());
    let result = result.borrow_mut().take().unwrap().unwrap();
    result
}
fn open(a: &Rc<Workspace>) -> Rc<LicensedImportsPage> {
    a.licensed_imports();
    wait(|| {
        a.licensed_import_pages.borrow().last().is_some_and(|p| {
            p.loaded.get() && a.current_widget() == Some(p.widget.clone().upcast())
        })
    });
    a.licensed_import_pages.borrow().last().unwrap().clone()
}
fn save(a: &Rc<Workspace>) -> Result<()> {
    let done = Rc::new(RefCell::new(None));
    let out = done.clone();
    a.save_current(move |r| *out.borrow_mut() = Some(r));
    wait(|| done.borrow().is_some());
    let result = done.borrow_mut().take().unwrap();
    result
}
fn preview_pack(p: &Rc<LicensedImportsPage>, pack: &Value) {
    p.ready().unwrap();
    p.draft
        .borrow_mut()
        .as_mut()
        .unwrap()
        .preview_pack(&serde_json::to_vec(pack).unwrap())
        .unwrap();
    p.dirty.set(true);
    p.sync();
}
fn preview_font(p: &Rc<LicensedImportsPage>, otf: bool) {
    p.ready().unwrap();
    let mut license = license_fixture();
    license["name"] = json!("CC0-1.0 · synthetic test font");
    license["source"] = json!("Generated synthetic fixture, no external font bytes");
    p.license_name.set_text(license["name"].as_str().unwrap());
    p.license_source
        .set_text(license["source"].as_str().unwrap());
    p.font_permission.set_active(true);
    p.draft
        .borrow_mut()
        .as_mut()
        .unwrap()
        .preview_font(&synthetic_sfnt(otf), &license, "synthetic@1")
        .unwrap();
    p.dirty.set(true);
    p.sync();
}
// Root's initialized opt-in GTK thread invokes this against temporary stores.
pub(crate) fn run(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("imports-data");
    let cache = fixture.path().join("imports-cache");
    for p in [&data, &cache] {
        std::fs::create_dir_all(p).unwrap();
    }
    let worker = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(worker), &data, true).unwrap();
    let created = rpc(
        &client,
        "create_novel",
        json!({"title":"Synthetic licensed imports"}),
    );
    let pid = created["project"]["id"].as_str().unwrap();
    let sid = created["scene"]["id"].as_str().unwrap();
    let mut base = alphabet_doc();
    base["assignments"][0]["scene"]["id"] = json!(sid);
    rpc(
        &client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":"writingSystems","payload":base,"parent":null,"dependencies":[]}),
    );
    let history = rpc(&client, "history", json!({"sceneId":sid}));
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let p = open(&a);
    let mut pack = pack_fixture();
    pack["license"] = json!("CC0-1.0 · synthetic test dictionary");
    pack["checksum"] = json!(resources::pack_checksum(&pack).unwrap());
    preview_pack(&p, &pack);
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    assert!(save(&a).unwrap_err().contains("license"));
    assert!(p.dirty.get());
    assert!(p.draft.borrow().as_ref().unwrap().pending.is_some());
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.close_tab(p.widget.upcast_ref());
    assert!(a.pages.page_num(&p.widget).is_some());
    assert!(p.dirty.get());
    p.pack_permission.set_active(true);
    assert!(save(&a).is_ok());
    assert!(!p.dirty.get());
    let packs = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":pid,"kind":"languagePacks"}),
    );
    assert_eq!(packs["payload"]["packs"][0], pack);
    assert_eq!(rpc(&a.client, "history", json!({"sceneId":sid})), history);
    let mut second = pack.clone();
    second["version"] = json!("2");
    second["name"] = json!("Second synthetic multilingual pack");
    second["checksum"] = json!(resources::pack_checksum(&second).unwrap());
    preview_pack(&p, &second);
    p.pack_permission.set_active(true);
    let prepared = p.draft.borrow().as_ref().unwrap().preview_text();
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.close_tab(p.widget.upcast_ref());
    assert!(p.dirty.get());
    assert_eq!(p.draft.borrow().as_ref().unwrap().preview_text(), prepared);
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Discard);
    a.close_tab(p.widget.upcast_ref());
    wait(|| a.pages.page_num(&p.widget).is_none());
    assert!(!a
        .licensed_import_pages
        .borrow()
        .iter()
        .any(|v| Rc::ptr_eq(v, &p)));
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":pid,"kind":"languagePacks"})
        ),
        packs
    );
    let p = open(&a);
    preview_font(&p, false);
    crate::app::desk::tests::capture(&a,"notebook-licensed-font-preview.png");
    let (asset, candidate) = match p.draft.borrow().as_ref().unwrap().pending.as_ref().unwrap() {
        PendingImport::Font {
            asset, candidate, ..
        } => (asset.clone(), candidate.clone()),
        _ => panic!("Expected font preview"),
    };
    assert!(save(&a).is_ok());
    let font_saved = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":pid,"kind":"writingSystems"}),
    );
    assert_eq!(font_saved["payload"], candidate);
    assert_eq!(font_saved["payload"]["systems"][0], base["systems"][0]);
    assert_eq!(font_saved["payload"]["assignments"], base["assignments"]);
    let stored = rpc(
        &a.client,
        "get_asset",
        json!({"projectId":pid,"assetId":asset.id}),
    );
    assert_eq!(
        serde_json::from_value::<novel_storage::ProjectAsset>(stored).unwrap(),
        asset
    );
    // A font asset may be stored before a competing alphabet CAS refuses its
    // metadata. The complete preview and immutable prior version stay intact.
    preview_font(&p, true);
    let pending = p.draft.borrow().as_ref().unwrap().preview_text();
    let failed_asset = match p.draft.borrow().as_ref().unwrap().pending.as_ref().unwrap() {
        PendingImport::Font { asset, .. } => asset.clone(),
        _ => panic!("Font preview"),
    };
    let competing = rpc(
        &a.client,
        "save_project_data_checked",
        json!({"projectId":pid,"kind":"writingSystems","parent":font_saved["revisionId"],"payload":font_saved["payload"],"dependencies":[]}),
    );
    assert!(save(&a).is_err());
    assert!(p.dirty.get());
    assert_eq!(p.draft.borrow().as_ref().unwrap().preview_text(), pending);
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":pid,"kind":"writingSystems"})
        ),
        competing
    );
    assert_eq!(
        serde_json::from_value::<novel_storage::ProjectAsset>(rpc(
            &a.client,
            "get_asset",
            json!({"projectId":pid,"assetId":failed_asset.id})
        ))
        .unwrap(),
        failed_asset
    );
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Discard);
    a.close_tab(p.widget.upcast_ref());
    wait(|| a.pages.page_num(&p.widget).is_none());
    let p = open(&a);
    preview_font(&p, true);
    let expected = p.draft.borrow().as_ref().unwrap().preview_text();
    a.detach_current();
    let child = desk::workspace_for_widget(p.widget.upcast_ref()).unwrap();
    assert!(!a
        .licensed_import_pages
        .borrow()
        .iter()
        .any(|v| Rc::ptr_eq(v, &p)));
    assert!(child
        .licensed_import_pages
        .borrow()
        .iter()
        .any(|v| Rc::ptr_eq(v, &p)));
    a.close();
    wait(|| a.desk.is_closed());
    assert_eq!(p.draft.borrow().as_ref().unwrap().preview_text(), expected);
    assert!(save(&child).is_ok());
    let final_saved = rpc(
        &child.client,
        "load_project_data_unambiguous",
        json!({"projectId":pid,"kind":"writingSystems"}),
    );
    assert_eq!(
        final_saved["payload"]["systems"].as_array().unwrap().len(),
        3
    );
    assert_eq!(
        final_saved["payload"]["systems"][1],
        font_saved["payload"]["systems"][1]
    );
    assert_eq!(final_saved["payload"]["assignments"], base["assignments"]);
    assert_eq!(
        rpc(&child.client, "history", json!({"sceneId":sid})),
        history
    );
    child.close();
    wait(|| child.desk.is_closed());
}
