use super::*;
fn page(a: &Workspace) -> Rc<AlternativesPage> {
    let widget = a.current_widget().unwrap();
    a.alternative_pages
        .borrow()
        .iter()
        .find(|p| p.widget.upcast_ref::<gtk::Widget>() == &widget)
        .unwrap()
        .clone()
}
fn open(a: &Rc<Workspace>) -> Rc<AlternativesPage> {
    let before = a.current_widget();
    a.alternatives();
    wait(|| {
        a.current_widget() != before
            && a.alternative_pages
                .borrow()
                .iter()
                .any(|p| p.loaded.get() && a.current_widget() == Some(p.widget.clone().upcast()))
    });
    page(a)
}
fn saved(p: &Rc<AlternativesPage>) {
    wait(|| !p.saving.get() && !p.dirty.get());
}
fn save_form(a: &Rc<Workspace>, p: &Rc<AlternativesPage>) -> Result<(), String> {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    a.save_current(move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some() && !p.saving.get());
    let value = result.borrow_mut().take().unwrap();
    value
}
fn discard_scene(p: &Rc<EditorPane>) {
    let done = Rc::new(Cell::new(false));
    let out = done.clone();
    p.discard(move |r| {
        r.unwrap();
        out.set(true);
    });
    wait(|| done.get());
}
pub(super) fn run(binaries: &Path) {
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("alternative-data");
    let cache = fixture.path().join("alternative-cache");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let companion = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(companion), &data, true).unwrap();
    let project = rpc(
        &client,
        "create_project",
        json!({"title":"Paragraph versions"}),
    );
    let pid = project["id"].as_str().unwrap();
    let chapter = rpc(
        &client,
        "create_chapter",
        json!({"projectId":pid,"title":"First"}),
    );
    let rich = json!({"type":"heading","attrs":{"dir":"rtl","lang":"ar","lineSpacing":1.5},"content":[{"type":"text","text":"مِ start ","marks":[{"type":"bold"}]},{"type":"text","text":"old","marks":[{"type":"italic"}]},{"type":"hardBreak"},{"type":"text","text":"中文 👩🏽‍🚀 tail","marks":[{"type":"underline"}]}]});
    let record = rpc(
        &client,
        "create_scene",
        json!({"chapterId":chapter["id"],"scene":{"schemaVersion":1,"title":"A revised line","document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"First paragraph 𠀀"}]},rich]}}}),
    );
    let sid = record["id"].as_str().unwrap();
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let source = a.active.borrow().clone().unwrap();
    source.operation(json!({"type":"key","key":"End","ctrl":true}), false);
    wait(|| source.pending.get() == 0);
    // Explicit paragraph targeting stays read-only and saves its exact rich original.
    let p = open(&a);
    p.paragraph.set_value(2.);
    let metadata = || {
        rpc(
            &a.client,
            "load_project_data",
            json!({"projectId":pid,"kind":"paragraphAlternatives"}),
        )
    };
    let scene = || rpc(&a.client, "load_scene", json!({"sceneId":sid}));
    let initial = scene();
    p.paragraph.set_text("1.5");
    find_button(&p.widget, "Capture paragraph")
        .unwrap()
        .clicked();
    wait(|| !p.saving.get());
    assert!(p.status.text().contains("whole paragraph"));
    assert!(metadata().is_null());
    p.paragraph.set_value(2.);
    find_button(&p.widget, "Capture paragraph")
        .unwrap()
        .clicked();
    saved(&p);
    let original = metadata()["payload"]["scenes"][0]["groups"][0].clone();
    assert_eq!(original["anchor"]["blockIndex"], 1);
    assert_eq!(original["versions"][0]["block"], rich);
    assert_eq!(scene(), initial);
    let original_id = original["originalId"].as_str().unwrap().to_owned();
    p.text
        .buffer()
        .unwrap()
        .set_text("مِ start revised\n中文 👩🏽‍🚀 tail");
    p.name.set_text("Revision A");
    pump(Duration::from_millis(350));
    assert_eq!(
        metadata()["payload"]["scenes"][0]["groups"][0]["versions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    select(&a, SaveChoice::Cancel);
    a.open_book();
    assert_eq!(a.current_widget(), Some(p.widget.clone().upcast()));
    assert!(p.dirty.get());
    save_form(&a, &p).unwrap();
    let group = metadata()["payload"]["scenes"][0]["groups"][0].clone();
    let alternative = group["versions"][1].clone();
    assert_eq!(group["versions"][0], original["versions"][0]);
    assert_eq!(alternative["block"]["attrs"], rich["attrs"]);
    assert_eq!(alternative["block"]["content"][0], rich["content"][0]);
    assert_eq!(
        alternative["block"]["content"].as_array().unwrap().last(),
        rich["content"].as_array().unwrap().last()
    );
    assert_eq!(scene(), initial);
    capture(&a, "notebook-paragraph-alternatives.png");
    find_button(&p.widget, "Apply saved version to draft")
        .unwrap()
        .clicked();
    saved(&p);
    wait(|| source.pending.get() == 0);
    assert!(source.dirty.get());
    assert_eq!(scene(), initial);
    assert_eq!(
        source.frame.borrow()["document"]["content"][1],
        alternative["block"]
    );
    source.operation(json!({"type":"command","command":"undo"}), true);
    wait(|| source.pending.get() == 0);
    assert_eq!(
        source.frame.borrow()["document"],
        initial["scene"]["document"]
    );
    // Undo invalidates the prior anchor; no stale replacement is applied.
    find_button(&p.widget, "Apply saved version to draft")
        .unwrap()
        .clicked();
    wait(|| !p.saving.get());
    assert!(p.status.text().contains("changed"));
    assert_eq!(
        source.frame.borrow()["document"],
        initial["scene"]["document"]
    );
    discard_scene(&source);
    find_button(&p.widget, "Rebind saved group")
        .unwrap()
        .clicked();
    saved(&p);
    find_button(&p.widget, "Apply saved version to draft")
        .unwrap()
        .clicked();
    saved(&p);
    save(&a, &source);
    let rewritten = scene();
    assert_ne!(rewritten["revisionId"], initial["revisionId"]);
    p.versions.set_active_id(Some(&original_id));
    find_button(&p.widget, "Apply saved version to draft")
        .unwrap()
        .clicked();
    wait(|| !p.saving.get());
    assert!(p.status.text().contains("since capture"));
    assert_eq!(scene(), rewritten);
    find_button(&p.widget, "Rebind saved group")
        .unwrap()
        .clicked();
    saved(&p);
    find_button(&p.widget, "Apply saved version to draft")
        .unwrap()
        .clicked();
    saved(&p);
    assert_eq!(
        source.frame.borrow()["document"],
        initial["scene"]["document"]
    );
    assert_eq!(scene(), rewritten);
    source.operation(json!({"type":"command","command":"undo"}), true);
    wait(|| source.pending.get() == 0);
    assert_eq!(
        source.frame.borrow()["document"],
        rewritten["scene"]["document"]
    );
    discard_scene(&source);
    // Two independent metadata drafts cannot overwrite each other.
    find_button(&p.widget, "Rebind saved group")
        .unwrap()
        .clicked();
    saved(&p);
    let child = a.spawn_window();
    child.open_scene(sid, false);
    wait(|| child.active.borrow().as_ref().is_some_and(|p| p.open.get()));
    let other = open(&child);
    other
        .text
        .buffer()
        .unwrap()
        .set_text("Other version مِ 中文");
    other.name.set_text("Other window");
    save_form(&child, &other).unwrap();
    let before = metadata();
    find_button(&p.widget, "Apply saved version to draft")
        .unwrap()
        .clicked();
    wait(|| !p.saving.get());
    assert!(p.dirty.get());
    assert!(p.status.text().contains("scene draft"));
    assert_eq!(metadata(), before);
    assert_eq!(scene(), rewritten);
    assert_eq!(
        source.frame.borrow()["document"],
        initial["scene"]["document"]
    );
    source.operation(json!({"type":"command","command":"undo"}), true);
    wait(|| source.pending.get() == 0);
    assert_eq!(
        source.frame.borrow()["document"],
        rewritten["scene"]["document"]
    );
    discard_scene(&source);
    p.discard();
    p.text
        .buffer()
        .unwrap()
        .set_text("Retained failed version 𠀀");
    p.name.set_text("Retained version");
    let before = metadata();
    assert!(save_form(&a, &p).unwrap_err().contains("Conflict"));
    assert!(p.dirty.get());
    assert_eq!(metadata(), before);
    assert_eq!(p.name.text(), "Retained version");
    select(&a, SaveChoice::Save);
    a.close();
    wait(|| !a.closing.get());
    assert!(a.window.is_visible());
    assert!(p.dirty.get());
    // Composition blocks leaving and moving the form. Discard preserves saved originals.
    p.discard();
    p.composing.set(true);
    let count = windows();
    a.detach_current();
    assert_eq!(windows(), count);
    a.open_book();
    assert_eq!(a.current_widget(), Some(p.widget.clone().upcast()));
    p.composing.set(false);
    select(&a, SaveChoice::Discard);
    a.close();
    wait(|| windows() == 1);
    child.detach_current();
    let detached = last_window();
    assert!(Rc::ptr_eq(&page(&detached), &other));
    child.close();
    wait(|| windows() == 1);
    other.name.set_text("After original close");
    other
        .text
        .buffer()
        .unwrap()
        .set_text("Detached alternative مِ 中文 👩🏽‍🚀");
    save_form(&detached, &other).unwrap();
    let final_data = rpc(
        &detached.client,
        "load_project_data",
        json!({"projectId":pid,"kind":"paragraphAlternatives"}),
    );
    assert_eq!(
        final_data["payload"]["scenes"][0]["groups"][0]["versions"][0],
        original["versions"][0]
    );
    assert_eq!(
        rpc(&detached.client, "load_scene", json!({"sceneId":sid})),
        rewritten
    );
    detached.close();
    wait(|| windows() == 0);
}
