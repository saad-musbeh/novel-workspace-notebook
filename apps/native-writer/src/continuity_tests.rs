use super::*;
fn enabled() -> BTreeSet<String> {
    RULES.iter().map(|(id, _)| id.to_string()).collect()
}
fn fixture() -> Value {
    json!({"schemaVersion":1,"fields":[],"records":[{"id":"hero","kind":"character","name":"نور 明","aliases":[],"notes":"","links":[],"values":{}},{"id":"north","kind":"location","name":"北 مِ","aliases":[],"notes":"","links":[],"values":{}},{"id":"south","kind":"location","name":"南","aliases":[],"notes":"","links":[],"values":{}}],"events":[{"id":"early","title":"Early مِ 中文 \u{e000}","notes":"","sceneIds":["s"],"laneIds":["main","sub"],"time":{"kind":"fixed","day":1,"label":""}},{"id":"late","title":"Late","notes":"","sceneIds":["t"],"laneIds":["main"],"time":{"kind":"fixed","day":2,"label":""}}],"facts":[],"boards":[],"lanes":[{"id":"main","name":"Main مِ","kind":"main"},{"id":"sub","name":"副线","kind":"subplot"}],"constraints":[],"plotPoints":[],"issues":[]})
}
fn fact(id: &str, property: &str, value: &str, event: &str) -> Value {
    json!({"id":id,"subjectId":"hero","property":property,"value":value,"eventId":event,"untilEventId":null,"status":"fact","confirmed":true,"source":"Author مِ 中文","observerId":null})
}
fn rule(report: &Report, id: &str) -> usize {
    report.findings.iter().filter(|f| f.rule == id).count()
}
#[test]
fn prerequisite_uses_only_confirmed_canonical_event_edges_and_established_order() {
    let mut d = fixture();
    d["boards"] = json!([{"id":"b","cards":[{"id":"d","reference":{"kind":"event","id":"early"}},{"id":"p","reference":{"kind":"event","id":"late"}}],"edges":[{"id":"edge","kind":"depends-on","fromId":"d","toId":"p","confirmed":true}]}]);
    let before = d.clone();
    assert_eq!(rule(&check(&d, &enabled()), "missing-prerequisite"), 1);
    assert_eq!(d, before);
    for value in [json!(false), Value::Null] {
        d["boards"][0]["edges"][0]["confirmed"] = value;
        assert_eq!(rule(&check(&d, &enabled()), "missing-prerequisite"), 0);
    }
    d["boards"][0]["edges"][0]["confirmed"] = json!(true);
    d["events"][1]["time"] = json!({"kind":"unknown"});
    let r = check(&d, &enabled());
    assert_eq!(rule(&r, "missing-prerequisite"), 0);
    assert!(r.unresolved > 0);
}
#[test]
fn learn_use_requires_all_recorded_acquisitions_to_be_later_and_never_reads_frozen_knowledge() {
    let mut d = fixture();
    d["facts"] = json!([
        fact("use", "knowledge:use", "key مِ 中文", "early"),
        fact("learn", "knowledge:learn", "key مِ 中文", "late")
    ]);
    assert_eq!(rule(&check(&d, &enabled()), "knowledge-before-learning"), 1);
    d["facts"].as_array_mut().unwrap().push(fact(
        "learn-earlier",
        "knowledge:learn",
        "key مِ 中文",
        "early",
    ));
    assert_eq!(rule(&check(&d, &enabled()), "knowledge-before-learning"), 0);
    d["facts"] = json!([]);
    d["knowledge"] = json!([{"characterId":"hero","stance":"knows","scope":{"kind":"events","fromEventId":"early"},"factSnapshot":{"value":"key مِ 中文"}}]);
    assert_eq!(rule(&check(&d, &enabled()), "knowledge-before-learning"), 0);
}
#[test]
fn no_learning_record_unknown_order_unapproved_and_noncharacter_claims_are_not_proof() {
    let mut d = fixture();
    d["facts"] = json!([fact("use", "knowledge:use", "key", "early")]);
    let r = check(&d, &enabled());
    assert_eq!(rule(&r, "knowledge-before-learning"), 0);
    assert!(r.unresolved > 0);
    d["facts"]
        .as_array_mut()
        .unwrap()
        .push(fact("learn", "knowledge:learn", "key", "late"));
    d["events"][1]["time"] = json!({"kind":"unknown"});
    assert_eq!(rule(&check(&d, &enabled()), "knowledge-before-learning"), 0);
    d["events"][1]["time"] = json!({"kind":"fixed","day":2});
    d["facts"][0]["confirmed"] = json!(false);
    assert_eq!(rule(&check(&d, &enabled()), "knowledge-before-learning"), 0);
    d["facts"][0]["confirmed"] = json!(true);
    d["records"][0]["kind"] = json!("item");
    assert_eq!(rule(&check(&d, &enabled()), "knowledge-before-learning"), 0);
}
#[test]
fn exclusive_presence_requires_definite_overlap_canonical_locations_and_explicit_property() {
    let mut d = fixture();
    d["events"][0]["time"] = json!({"kind":"interval","start":1,"end":3});
    d["facts"] = json!([
        fact("n", "exclusive-location", "north", "early"),
        fact("s", "exclusive-location", "south", "late")
    ]);
    assert_eq!(rule(&check(&d, &enabled()), "exclusive-presence"), 1);
    for time in [
        json!({"kind":"bounds","start":1,"end":3}),
        json!({"kind":"unknown"}),
    ] {
        d["events"][0]["time"] = time;
        assert_eq!(rule(&check(&d, &enabled()), "exclusive-presence"), 0);
    }
    d["events"][0]["time"] = json!({"kind":"fixed","day":2});
    d["facts"][1]["value"] = json!("unretained place");
    assert_eq!(rule(&check(&d, &enabled()), "exclusive-presence"), 0);
    d["facts"][1]["value"] = json!("south");
    d["facts"][0]["property"] = json!("location");
    assert_eq!(rule(&check(&d, &enabled()), "exclusive-presence"), 0);
}
#[test]
fn confirmed_state_uses_bounded_applicability_without_extending_an_unknown_end() {
    let mut d = fixture();
    d["facts"] = json!([
        fact("a", "possession", "A مِ", "early"),
        fact("b", "possession", "B 中文", "late")
    ]);
    assert_eq!(rule(&check(&d, &enabled()), "confirmed-state-conflict"), 0);
    d["facts"][0]["untilEventId"] = json!("late");
    assert_eq!(rule(&check(&d, &enabled()), "confirmed-state-conflict"), 1);
    d["events"][1]["time"] = json!({"kind":"bounds","start":2,"end":4});
    assert_eq!(rule(&check(&d, &enabled()), "confirmed-state-conflict"), 0);
    d["events"][1]["time"] = json!({"kind":"fixed","day":2});
    d["facts"][1]["property"] = json!("different");
    assert_eq!(rule(&check(&d, &enabled()), "confirmed-state-conflict"), 0);
}
#[test]
fn payoff_reminders_respect_authored_closed_scope_and_present_payoff() {
    let mut d = fixture();
    d["plotPoints"] = json!([{"id":"setup","title":"Clue مِ 中文 👩🏽‍🚀","requiresPayoff":true,"scopeClosed":false,"payoffId":null}]);
    assert_eq!(rule(&check(&d, &enabled()), "unresolved-setup"), 0);
    d["plotPoints"][0]["scopeClosed"] = json!(true);
    assert_eq!(rule(&check(&d, &enabled()), "unresolved-setup"), 1);
    d["plotPoints"][0]["payoffId"] = json!("missing");
    assert_eq!(rule(&check(&d, &enabled()), "unresolved-setup"), 1);
    d["plotPoints"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"missing"}));
    assert_eq!(rule(&check(&d, &enabled()), "unresolved-setup"), 0);
}
#[test]
fn exact_issue_annotations_and_page_rule_disabling_preserve_evidence_without_inventing_exceptions()
{
    let mut d = fixture();
    d["plotPoints"] = json!([{"id":"setup","title":"Clue","requiresPayoff":true,"scopeClosed":true,"payoffId":null}]);
    d["issues"] = json!([{"id":"intention","category":"unresolved-setup","status":"intentional","links":[{"kind":"plot-point","id":"setup"}],"resolution":"Deliberate مِ 中文"}]);
    let report = check(&d, &enabled());
    assert_eq!(report.findings.len(), 1);
    assert!(report.findings[0].annotation[0].contains("Deliberate مِ 中文"));
    d["issues"][0]["links"] = json!([]);
    assert!(check(&d, &enabled()).findings[0].annotation.is_empty());
    assert!(check(&d, &BTreeSet::new()).findings.is_empty());
}
#[test]
fn conflicting_component_never_becomes_a_definite_presence_or_state_claim() {
    let mut d = fixture();
    d["events"][1]["time"] = json!({"kind":"fixed","day":1});
    d["facts"] = json!([
        fact("a", "exclusive-location", "north", "early"),
        fact("b", "exclusive-location", "south", "late")
    ]);
    d["constraints"] = json!([{"id":"strict","beforeId":"early","afterId":"late","minimumDays":0,"strict":true,"confirmed":true}]);
    let r = check(&d, &enabled());
    assert_eq!(rule(&r, "chronology-cycle"), 1);
    assert_eq!(rule(&r, "exclusive-presence"), 0);
    assert!(r.unresolved > 0);
}
#[test]
fn lane_projection_retains_one_event_identity_in_each_authored_lane_and_missing_links() {
    let mut d = fixture();
    d["events"][1]["laneIds"] = json!(["missing"]);
    d["events"][1]["sceneIds"] = json!(["gone"]);
    let before = d.clone();
    let r = lanes::project(&d, &HashSet::from(["s".into(), "t".into()]));
    assert_eq!(r.rows.iter().filter(|r| r.event == "early").count(), 2);
    assert!(lanes::render(&r).contains("Early مِ 中文 \u{e000}"));
    assert!(r
        .rows
        .iter()
        .any(|r| r.lane.contains("Missing or ambiguous lane")));
    assert!(r.notes.iter().any(|s| s.contains("gone")));
    assert_eq!(d, before);
    d["events"][1]["laneIds"] = json!([]);
    assert!(lanes::project(&d, &HashSet::new())
        .rows
        .iter()
        .any(|r| r.lane == "Unassigned events"));
}
#[test]
fn coverage_is_bounded_and_source_shape_does_not_accept_foreign_project() {
    let mut d = fixture();
    d["facts"] = (0..2001)
        .map(|i| fact(&format!("f{i}"), "knowledge:use", "key", "early"))
        .collect();
    assert!(check(&d, &enabled()).limited);
    assert!(checked_snapshot(&json!({"index":{"project":{"id":"foreign"},"scenes":[]},"planning":null,"sceneMetadata":null}),"p").is_err());
}

#[test]
fn relative_to_possible_bounds_and_decimal_cancelled_order_are_not_definite_evidence() {
    let mut d = fixture();
    d["events"][0]["time"] = json!({"kind":"bounds","start":1,"end":3});
    d["events"][1]["time"] = json!({"kind":"relative","eventId":"early","offsetDays":0});
    d["facts"] = json!([
        fact("a", "exclusive-location", "north", "early"),
        fact("b", "exclusive-location", "south", "late")
    ]);
    assert_eq!(rule(&check(&d, &enabled()), "exclusive-presence"), 0);
    d["events"] = json!([{"id":"early","time":{"kind":"unknown"}},{"id":"v","time":{"kind":"relative","eventId":"early","offsetDays":0.1}},{"id":"w","time":{"kind":"relative","eventId":"v","offsetDays":0.2}},{"id":"late","time":{"kind":"relative","eventId":"w","offsetDays":-0.3}}]);
    d["facts"] = json!([
        fact("use", "knowledge:use", "key", "early"),
        fact("learn", "knowledge:learn", "key", "late")
    ]);
    let report = check(&d, &enabled());
    assert_eq!(rule(&report, "knowledge-before-learning"), 0);
    assert!(report.unresolved > 0);
}
#[test]
fn duplicate_canonical_fact_ids_never_select_one_claim_for_a_rule() {
    let mut d = fixture();
    d["facts"] = json!([
        fact("use", "knowledge:use", "key", "early"),
        fact("use", "knowledge:use", "key", "early"),
        fact("learn", "knowledge:learn", "key", "late")
    ]);
    assert_eq!(rule(&check(&d, &enabled()), "knowledge-before-learning"), 0);
}

#[test]
fn empty_lanes_and_same_named_subplots_keep_distinct_canonical_ids() {
    let mut d = fixture();
    d["events"] = json!([]);
    d["lanes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"other-sub","name":"副线","kind":"subplot"}));
    let report = lanes::project(&d, &HashSet::new());
    let rendered = lanes::render(&report);
    assert!(rendered.contains("Subplot · 副线 [sub]"));
    assert!(rendered.contains("Subplot · 副线 [other-sub]"));
    assert_eq!(
        rendered.matches("No authored events in this lane.").count(),
        3
    );
}
#[test]
fn malformed_or_extra_issue_links_do_not_acquire_an_intentional_annotation() {
    let mut d = fixture();
    d["plotPoints"] = json!([{"id":"setup","title":"Clue","requiresPayoff":true,"scopeClosed":true,"payoffId":null}]);
    d["issues"] = json!([{"id":"bad","category":"unresolved-setup","status":"intentional","links":[{"kind":"plot-point","id":"setup"},{"bad":"ignored"}]}]);
    assert!(check(&d, &enabled()).findings[0].annotation.is_empty());
    d["issues"][0]["links"][1] = json!({"kind":"scene","id":"unrelated"});
    assert!(check(&d, &enabled()).findings[0].annotation.is_empty());
}

#[test]
fn rounded_relative_overlap_is_unresolved_while_exact_integer_relative_overlap_remains_checked() {
    let mut d = fixture();
    d["events"][0]["time"] = json!({"kind":"fixed","day":0.1});
    d["events"][1]["time"] = json!({"kind":"relative","eventId":"early","offsetDays":0.2});
    d["events"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"same-rounded","time":{"kind":"fixed","day":0.30000000000000004}}));
    d["facts"] = json!([
        fact("a", "exclusive-location", "north", "late"),
        fact("b", "exclusive-location", "south", "same-rounded")
    ]);
    let r = check(&d, &enabled());
    assert_eq!(rule(&r, "exclusive-presence"), 0);
    assert!(r.unresolved > 0);
    d["events"][0]["time"]["day"] = json!(1);
    d["events"][1]["time"]["offsetDays"] = json!(1);
    d["events"][2]["time"]["day"] = json!(2);
    assert_eq!(rule(&check(&d, &enabled()), "exclusive-presence"), 1);
}

fn pump(mut predicate: impl FnMut() -> bool) {
    let start = std::time::Instant::now();
    while !predicate() {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
}
fn rpc(client: &Process, op: &str, args: Value) -> Value {
    let out = Rc::new(RefCell::new(None));
    let next = out.clone();
    client.rpc(op, args, move |r| *next.borrow_mut() = Some(r));
    pump(|| out.borrow().is_some());
    let value = out.borrow_mut().take().unwrap().unwrap();
    value
}
fn find<T: IsA<gtk::Widget> + glib::object::ObjectType>(
    w: &impl IsA<gtk::Widget>,
    predicate: &impl Fn(&T) -> bool,
) -> Option<T> {
    if let Ok(value) = w.as_ref().clone().downcast::<T>() {
        if predicate(&value) {
            return Some(value);
        }
    }
    w.as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .and_then(|c| c.children().iter().find_map(|w| find(w, predicate)))
}
fn click(w: &impl IsA<gtk::Widget>, title: &str) {
    find::<gtk::Button>(w, &|b| b.label().as_deref() == Some(title))
        .unwrap()
        .clicked();
}
fn content(w: &impl IsA<gtk::Widget>) -> String {
    find::<gtk::TextView>(w, &|_| true)
        .map(|v| {
            assert!(!v.is_editable());
            let b = v.buffer().unwrap();
            b.text(&b.start_iter(), &b.end_iter(), true)
                .unwrap()
                .to_string()
        })
        .unwrap_or_default()
}
fn status(w: &impl IsA<gtk::Widget>, needle: &str) -> bool {
    find::<gtk::Label>(w, &|l| l.text().contains(needle)).is_some()
}
/// Root invokes this only inside its single-threaded, opt-in GTK/worker test.
pub(crate) fn run(binaries: &Path) {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let cache = dir.path().join("cache");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let companion = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(companion), &data, true).unwrap();
    let p = rpc(
        &client,
        "create_project",
        json!({"title":"Continuity fixture مِ 中文"}),
    );
    let pid = text(&p, "id").to_owned();
    let chapter = rpc(
        &client,
        "create_chapter",
        json!({"projectId":pid,"title":"Chapter"}),
    );
    let mut saved_scenes = Vec::new();
    for title in ["First مِ 中文", "Second"] {
        saved_scenes.push(rpc(&client,"create_scene",json!({"chapterId":chapter["id"],"scene":{"schemaVersion":1,"title":title,"document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Synthetic prose mentioning secrets and impossible travel. It establishes no rule evidence."}]}]}}})));
    }
    let first = text(&saved_scenes[0], "id").to_owned();
    let second = text(&saved_scenes[1], "id").to_owned();
    let mut plan = fixture();
    plan["events"][0]["sceneIds"] = json!([first]);
    plan["events"][1]["sceneIds"] = json!([second]);
    plan["facts"] = json!([
        fact("use", "knowledge:use", "key", "early"),
        fact("learn", "knowledge:learn", "key", "late")
    ]);
    let saved = rpc(
        &client,
        "save_project_data",
        json!({"projectId":pid,"kind":"planning","payload":plan,"parent":null}),
    );
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    pump(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let pane = a.active.borrow().as_ref().unwrap().clone();
    pane.operation(json!({"type":"text","text":"UNSAVED مِ 中文"}), true);
    pump(|| pane.pending.get() == 0 && pane.dirty.get());
    let draft = pane.frame.borrow()["document"].clone();
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.continuity_checks();
    assert_eq!(a.current_widget(), Some(pane.widget.clone().upcast()));
    assert_eq!(pane.frame.borrow()["document"], draft);
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Discard);
    a.continuity_checks();
    pump(|| {
        a.current_widget()
            .is_some_and(|w| status(&w, "Choose enabled rules"))
    });
    let checks = a.current_widget().unwrap();
    assert!(content(&checks).is_empty());
    click(&checks, "Check saved records");
    pump(|| content(&checks).contains("Knowledge is used before"));
    assert!(status(&checks, text(&saved, "revisionId")));
    let choice = find::<gtk::CheckButton>(&checks, &|b| {
        b.label().as_deref() == Some("Knowledge used too early")
    })
    .unwrap();
    click(&checks, "Check saved records");
    choice.set_active(false);
    assert!(content(&checks).is_empty());
    pump(|| status(&checks, "Rule selection changed while checking"));
    click(&checks, "Check saved records");
    pump(|| content(&checks).contains("No issues found in the checked records"));
    assert_eq!(
        rpc(
            &a.client,
            "load_project_data",
            json!({"projectId":pid,"kind":"planning"})
        ),
        saved
    );
    let mut changed = plan.clone();
    changed["events"][1]["time"]["day"] = json!(0);
    let newer = rpc(
        &a.client,
        "save_project_data",
        json!({"projectId":pid,"kind":"planning","payload":changed,"parent":saved["revisionId"]}),
    );
    click(&checks, "Check saved records");
    pump(|| status(&checks, "Saved sources changed"));
    assert!(content(&checks).is_empty());
    click(&checks, "Refresh saved source");
    pump(|| status(&checks, text(&newer, "revisionId")));
    a.detach_current();
    let detached = super::super::WINDOWS.with(|windows| windows.borrow().last().unwrap().clone());
    a.close();
    pump(|| !a.window.is_visible());
    click(&checks, "Refresh saved source");
    pump(|| status(&checks, text(&newer, "revisionId")));
    click(&checks, "Check saved records");
    pump(|| content(&checks).contains("No issues found in the checked records"));
    detached.occurrence_lanes();
    pump(|| {
        detached
            .current_widget()
            .is_some_and(|w| content(&w).contains("Subplot · 副线"))
    });
    let lanes = detached.current_widget().unwrap();
    assert_eq!(content(&lanes).matches("early · Early مِ 中文").count(), 2);
    assert_eq!(
        rpc(
            &detached.client,
            "load_project_data",
            json!({"projectId":pid,"kind":"planning"})
        ),
        newer
    );
    click(&lanes, "Open linked scene in new window");
    pump(|| {
        super::super::WINDOWS.with(|windows| {
            windows
                .borrow()
                .iter()
                .filter(|w| w.window.is_visible())
                .count()
        }) == 2
    });
    let writer = super::super::WINDOWS.with(|windows| windows.borrow().last().unwrap().clone());
    pump(|| {
        writer
            .active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.scene_id == first && p.open.get())
    });
    writer.close();
    detached.close();
    pump(|| {
        super::super::WINDOWS
            .with(|windows| windows.borrow().iter().all(|w| !w.window.is_visible()))
    });
}
