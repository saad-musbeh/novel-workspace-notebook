//! Callable from desk_tests after the parent adds ordinary draft lifecycle glue.
use super::*;
use crate::app::knowledge::{AuthoringKind, KnowledgePage};

fn form(a: &Workspace) -> Rc<KnowledgePage> {
    let widget = a.current_widget().unwrap();
    a.knowledge_pages
        .borrow()
        .iter()
        .find(|p| p.widget.upcast_ref::<gtk::Widget>() == &widget)
        .unwrap()
        .clone()
}
fn open(a: &Rc<Workspace>, kind: AuthoringKind) -> Rc<KnowledgePage> {
    let before = a.current_widget();
    match kind {
        AuthoringKind::Knowledge => a.character_knowledge(),
        AuthoringKind::ReaderTimeline => a.reader_timeline(),
    }
    wait(|| {
        a.current_widget() != before
            && a.knowledge_pages
                .borrow()
                .iter()
                .any(|p| p.loaded.get() && a.current_widget() == Some(p.widget.clone().upcast()))
    });
    form(a)
}
fn save_form(a: &Rc<Workspace>, p: &Rc<KnowledgePage>) -> Result<(), String> {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    a.save_current(move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some() && !p.saving.get());
    let value = result.borrow_mut().take().unwrap();
    value
}
fn fill(p: &KnowledgePage, scene: &str, stance: &str, fact: &str) {
    p.character.set_active_id(Some("hero"));
    p.stance.set_active_id(Some(stance));
    p.fact.set_active_id(Some(fact));
    p.capture_fact();
    p.source
        .buffer()
        .unwrap()
        .set_text("Author source مِ 中文 𠀀 👩🏽‍🚀");
    p.scene.set_active_id(Some(scene));
    find_button(&p.widget, "Add chosen scene")
        .unwrap()
        .clicked();
}
fn planning(a: &Workspace, pid: &str) -> Value {
    rpc(
        &a.client,
        "planning_read_snapshot",
        json!({"projectId":pid}),
    )["planning"]
        .clone()
}

pub(super) fn run(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("knowledge-data");
    let cache = fixture.path().join("knowledge-cache");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let companion = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(companion), &data, true).unwrap();
    let novel = rpc(
        &client,
        "create_novel",
        json!({"title":"Authored knowledge / reader order"}),
    );
    let pid = novel["project"]["id"].as_str().unwrap();
    let first = novel["scene"]["id"].as_str().unwrap();
    let chapter = novel["scene"]["chapterId"].as_str().unwrap();
    let second = rpc(
        &client,
        "create_scene",
        json!({"chapterId":chapter,"scene":{"schemaVersion":1,"title":"Second scene 中文","document":{"type":"doc","content":[{"type":"paragraph","content":[]}]}}}),
    );
    let second = second["id"].as_str().unwrap();
    let exact = "مِ 中文 𠀀 \u{e000} 👩🏽‍🚀";
    let mut seed = crate::tools::initial("planning");
    seed["records"] = json!([{"id":"hero","kind":"character","name":"نور 明","aliases":[],"notes":"","values":{},"links":[]}]);
    seed["facts"] = json!([{"id":"approved","subjectId":"hero","property":"Secret","value":exact,"status":"fact","confirmed":true,"source":"Author","observerId":null,"eventId":null,"untilEventId":null},{"id":"proposal","subjectId":"hero","property":"Guess","value":"Unapproved","status":"proposal","confirmed":false,"source":"Author question","observerId":null,"eventId":null,"untilEventId":null}]);
    seed["events"] = json!([{"id":"early","title":"Earlier occurrence","notes":"","laneIds":[],"sceneIds":[second],"time":{"kind":"fixed","day":1,"label":""}},{"id":"late","title":"Later occurrence","notes":"","laneIds":[],"sceneIds":[first],"time":{"kind":"fixed","day":20,"label":""}}]);
    rpc(
        &client,
        "save_project_data",
        json!({"projectId":pid,"kind":"planning","payload":seed,"parent":null}),
    );
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let manuscript = a.active.borrow().clone().unwrap();
    edit(&manuscript, exact);
    let document = manuscript.frame.borrow()["document"].clone();
    select(&a, SaveChoice::Cancel);
    a.character_knowledge();
    assert_eq!(a.current_widget(), Some(manuscript.widget.clone().upcast()));
    assert!(a.knowledge_pages.borrow().is_empty());
    select(&a, SaveChoice::Save);
    let p = open(&a, AuthoringKind::Knowledge);
    let saved_scene = rpc(&a.client, "load_scene", json!({"sceneId":first}));
    assert_eq!(saved_scene["scene"]["document"], document);
    let before = planning(&a, pid);
    fill(&p, first, "knows", "approved");
    assert!(!p.snapshot_view.is_editable());
    assert!(p
        .snapshot_view
        .buffer()
        .unwrap()
        .text(
            &p.snapshot_view.buffer().unwrap().start_iter(),
            &p.snapshot_view.buffer().unwrap().end_iter(),
            true
        )
        .unwrap()
        .contains(exact));
    pump(Duration::from_millis(300));
    assert_eq!(planning(&a, pid), before);
    assert!(p.dirty.get());
    capture(&a, "notebook-character-knowledge.png");
    select(&a, SaveChoice::Cancel);
    a.reader_timeline();
    assert_eq!(a.current_widget(), Some(p.widget.clone().upcast()));
    save_form(&a, &p).unwrap();
    let known = planning(&a, pid);
    let known_id = known["payload"]["knowledge"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        known["payload"]["knowledge"][0]["factSnapshot"]["value"],
        exact
    );
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":first})),
        saved_scene
    );
    // Invalid knowledge and composition never commit; discarded fields restore.
    find_button(&p.widget, "New entry").unwrap().clicked();
    fill(&p, first, "knows", "proposal");
    let marker = rpc(&a.client, "sync_cursor", json!({}));
    assert!(save_form(&a, &p).unwrap_err().contains("approved"));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})), marker);
    p.stance.set_active_id(Some("believes"));
    assert!(save_form(&a, &p).unwrap_err().contains("interpretation"));
    p.interpretation
        .buffer()
        .unwrap()
        .set_text("An explicit wrong interpretation 中文 مِ");
    p.scope.set_active_id(Some("events"));
    p.from.set_active_id(Some("early"));
    p.until.set_active_id(Some("early"));
    assert!(save_form(&a, &p).unwrap_err().contains("distinct"));
    p.until.set_active_id(Some("late"));
    p.composing.set(true);
    assert!(save_form(&a, &p).unwrap_err().contains("composition"));
    let count = windows();
    a.detach_current();
    assert_eq!(windows(), count);
    p.composing.set(false);
    save_form(&a, &p).unwrap();
    assert_eq!(
        planning(&a, pid)["payload"]["knowledge"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let first_version = known["revisionId"].clone();
    let historical = rpc(
        &a.client,
        "load_project_data_revision",
        json!({"projectId":pid,"kind":"planning","revisionId":first_version}),
    );
    assert_eq!(historical["payload"], known["payload"]);
    // Canon changes leave historical frozen claims untouched. Editing a stale
    // claim needs an explicit recapture after reloading the current planning.
    let current = planning(&a, pid);
    let mut changed = current["payload"].clone();
    changed["facts"][0]["value"] = json!("New canon مِ 中文");
    rpc(
        &a.client,
        "save_project_data",
        json!({"projectId":pid,"kind":"planning","payload":changed,"parent":current["revisionId"]}),
    );
    find_button(&p.widget, "Reload saved planning")
        .unwrap()
        .clicked();
    wait(|| p.loaded.get());
    p.entries.set_active_id(Some(&known_id));
    p.source.buffer().unwrap().set_text("Changed source");
    assert!(save_form(&a, &p).unwrap_err().contains("frozen"));
    assert_eq!(
        planning(&a, pid)["payload"]["knowledge"][0]["factSnapshot"]["value"],
        exact
    );
    p.capture_fact();
    save_form(&a, &p).unwrap();
    assert_eq!(
        planning(&a, pid)["payload"]["knowledge"][0]["factSnapshot"]["value"],
        "New canon مِ 中文"
    );
    // The reader page saves fact/event placements without editing occurrence time.
    let timeline = open(&a, AuthoringKind::ReaderTimeline);
    let events = planning(&a, pid)["payload"]["events"].clone();
    timeline.target_kind.set_active_id(Some("event"));
    timeline.event.set_active_id(Some("late"));
    timeline.scene.set_active_id(Some(first));
    timeline.position.set_text("1");
    timeline
        .notes
        .buffer()
        .unwrap()
        .set_text("Reader is told here مِ 中文");
    let before = planning(&a, pid);
    pump(Duration::from_millis(250));
    assert_eq!(planning(&a, pid), before);
    save_form(&a, &timeline).unwrap();
    find_button(&timeline.widget, "New entry")
        .unwrap()
        .clicked();
    timeline.event.set_active_id(Some("early"));
    timeline.target_kind.set_active_id(Some("event"));
    timeline.scene.set_active_id(Some(second));
    timeline.position.set_text("0");
    assert!(save_form(&a, &timeline).is_err());
    timeline.position.set_text("1.5");
    assert!(save_form(&a, &timeline).is_err());
    timeline.position.set_text("1");
    save_form(&a, &timeline).unwrap();
    let saved = planning(&a, pid);
    capture(&a, "notebook-reader-learns-timeline.png");
    assert_eq!(saved["payload"]["events"], events);
    assert_eq!(
        saved["payload"]["readerReveals"].as_array().unwrap().len(),
        2
    );
    find_button(&timeline.widget, "New entry")
        .unwrap()
        .clicked();
    timeline.target_kind.set_active_id(Some("fact"));
    timeline.fact.set_active_id(Some("approved"));
    timeline.scene.set_active_id(Some(first));
    timeline.position.set_text("2");
    save_form(&a, &timeline).unwrap();
    assert_eq!(
        planning(&a, pid)["payload"]["readerReveals"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let index = rpc(&a.client, "open_project_index", json!({"projectId":pid}));
    rpc(
        &a.client,
        "reorder_scenes",
        json!({"chapterId":chapter,"ids":[second,first]}),
    );
    find_button(&timeline.widget, "Reload saved planning")
        .unwrap()
        .clicked();
    wait(|| timeline.loaded.get());
    assert_eq!(planning(&a, pid)["payload"]["events"], events);
    let reordered = rpc(&a.client, "open_project_index", json!({"projectId":pid}));
    assert_ne!(index["scenes"][0]["id"], reordered["scenes"][0]["id"]);
    // Concurrent planning refuses stale CAS and retains all entered fields.
    timeline
        .notes
        .buffer()
        .unwrap()
        .set_text("Stale timeline draft مِ 中文");
    let child = a.spawn_window();
    let other = open(&child, AuthoringKind::ReaderTimeline);
    other.target_kind.set_active_id(Some("fact"));
    other.fact.set_active_id(Some("approved"));
    other.scene.set_active_id(Some(second));
    other
        .notes
        .buffer()
        .unwrap()
        .set_text("Competing explicit reveal");
    save_form(&child, &other).unwrap();
    let newer = planning(&a, pid);
    assert!(save_form(&a, &timeline).is_err());
    assert!(timeline.dirty.get());
    assert_eq!(planning(&a, pid), newer);
    select(&a, SaveChoice::Cancel);
    a.close();
    wait(|| !a.closing.get());
    assert!(a.window.is_visible());
    // Discard/Reload is explicit; detachment transfers the same page draft.
    timeline.discard();
    find_button(&timeline.widget, "Reload saved planning")
        .unwrap()
        .clicked();
    wait(|| timeline.loaded.get());
    timeline
        .notes
        .buffer()
        .unwrap()
        .set_text("Detached reveal мِ 中文");
    a.detach_current();
    let detached = last_window();
    assert!(Rc::ptr_eq(&form(&detached), &timeline));
    assert!(!a
        .knowledge_pages
        .borrow()
        .iter()
        .any(|p| Rc::ptr_eq(p, &timeline)));
    let original = a.current_widget();
    find_button(&timeline.widget, "Save entry")
        .unwrap()
        .clicked();
    wait(|| !timeline.saving.get() && !timeline.dirty.get());
    assert_eq!(a.current_widget(), original);
    a.close();
    wait(|| windows() == 2);
    timeline
        .notes
        .buffer()
        .unwrap()
        .set_text("Saved after original owner closes");
    save_form(&detached, &timeline).unwrap();
    assert_eq!(
        rpc(&detached.client, "load_scene", json!({"sceneId":first})),
        saved_scene
    );
    // Removal is pending until manual Save and older versions remain readable.
    let before = planning(&detached, pid);
    find_button(&timeline.widget, "Remove entry on Save")
        .unwrap()
        .clicked();
    assert!(timeline.dirty.get());
    assert_eq!(planning(&detached, pid), before);
    timeline.discard();
    assert_eq!(planning(&detached, pid), before);
    find_button(&timeline.widget, "Remove entry on Save")
        .unwrap()
        .clicked();
    save_form(&detached, &timeline).unwrap();
    assert_eq!(
        planning(&detached, pid)["payload"]["readerReveals"]
            .as_array()
            .unwrap()
            .len()
            + 1,
        before["payload"]["readerReveals"].as_array().unwrap().len()
    );
    let retained = rpc(
        &detached.client,
        "load_project_data_revision",
        json!({"projectId":pid,"kind":"planning","revisionId":before["revisionId"]}),
    );
    assert_eq!(retained["payload"], before["payload"]);
    // A scene deleted after the checked form capture must not receive a new
    // knowledge scope or reveal. Planning remains unchanged and each draft stays.
    let third = rpc(&detached.client, "create_scene", json!({"chapterId":chapter,"scene":{"schemaVersion":1,"title":"Scope deletion race مِ 中文","document":{"type":"doc","content":[{"type":"paragraph","content":[]}]}}}));
    let knowledge = open(&detached, AuthoringKind::Knowledge);
    fill(&knowledge, third["id"].as_str().unwrap(), "knows", "approved");
    let before = planning(&detached, pid);
    rpc(&detached.client, "delete_scene", json!({"sceneId":third["id"],"parent":third["revisionId"]}));
    assert!(save_form(&detached, &knowledge).is_err());
    assert!(knowledge.dirty.get());
    assert_eq!(planning(&detached, pid), before);
    knowledge.discard();
    let fourth = rpc(&detached.client, "create_scene", json!({"chapterId":chapter,"scene":{"schemaVersion":1,"title":"Reveal deletion race مِ 中文","document":{"type":"doc","content":[{"type":"paragraph","content":[]}]}}}));
    let reveal = open(&detached, AuthoringKind::ReaderTimeline);
    reveal.target_kind.set_active_id(Some("fact"));
    reveal.fact.set_active_id(Some("approved"));
    reveal.scene.set_active_id(fourth["id"].as_str());
    reveal.position.set_text("1");
    let before = planning(&detached, pid);
    rpc(&detached.client, "delete_scene", json!({"sceneId":fourth["id"],"parent":fourth["revisionId"]}));
    assert!(save_form(&detached, &reveal).is_err());
    assert!(reveal.dirty.get());
    assert_eq!(planning(&detached, pid), before);
    assert_eq!(rpc(&detached.client, "load_scene", json!({"sceneId":first})), saved_scene);
    reveal.discard();
    detached.close();
    wait(|| windows() == 1);
    child.close();
    wait(|| windows() == 0);
}
