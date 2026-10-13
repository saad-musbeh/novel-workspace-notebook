//! Pure host checks plus callable, serialized worker-backed GTK scenarios.
//! None of the callable helpers initialize GTK or launch a desktop by themselves.
use super::*;
use novel_storage::native_sequences::{NativeSequenceAction, SequenceFormat};
fn settings() -> NativeSequenceSettings {
    NativeSequenceSettings {
        schema_version: 1,
        sequences: vec![NativeCommandSequence {
            slot: 1,
            name: "مِ 中文".into(),
            actions: vec![
                NativeSequenceAction::Format {
                    command: SequenceFormat::Bold,
                },
                NativeSequenceAction::Spacing { value: 1.5 },
                NativeSequenceAction::Format {
                    command: SequenceFormat::Italic,
                },
            ],
        }],
    }
}
#[test]
fn host_checked_rpc_arguments_preserve_settings_and_captured_parent() {
    let payload = settings()
        .merge_into(&json!({"schemaVersion":1,"authorPreference":"مِ 中文 𠀀 👩🏽‍🚀"}))
        .unwrap();
    let snapshot = SettingsSnapshot {
        payload: payload.clone(),
        parent: Some("captured-head".into()),
    };
    let args = settings_save_args("project", &snapshot).unwrap();
    assert_eq!(
        args,
        json!({"projectId":"project","kind":"settings","payload":payload,"parent":"captured-head","dependencies":[]})
    );
    assert!(settings_save_args("", &snapshot).is_err());
    let snapshot = SettingsSnapshot {
        payload: json!({"nativeCommandSequences":{"schemaVersion":1,"sequences":[{"slot":1,"name":"bad","actions":[{"kind":"format","command":"undo"}]}]}}),
        parent: None,
    };
    assert!(settings_save_args("project", &snapshot).is_err());
}
#[test]
fn name_drafts_cannot_save_empty_sequences_or_change_commands() {
    let initial = settings();
    let mut names = vec![String::new(); 8];
    names[0] = "New مِ 中文".into();
    let renamed = renamed_settings(&initial, &names).unwrap();
    assert_eq!(renamed.sequences[0].name, names[0]);
    assert_eq!(renamed.sequences[0].actions, initial.sequences[0].actions);
    names[7] = "Unrecorded".into();
    assert!(renamed_settings(&initial, &names)
        .unwrap_err()
        .contains("needs recorded"));
    names[7].clear();
    names[0] = " ".into();
    assert!(renamed_settings(&initial, &names).is_err());
    names[0] = "𠀀".repeat(41);
    assert!(renamed_settings(&initial, &names).is_err());
    assert!(renamed_settings(&initial, &names[..7]).is_err());
}
#[test]
fn synchronous_internal_capability_restores_nested_scope_and_unwind() {
    assert!(INTERNAL_REQUEST.with(|s| s.borrow().is_none()));
    with_internal_request("first", || {
        assert_eq!(
            INTERNAL_REQUEST.with(|s| s.borrow().clone()).as_deref(),
            Some("first")
        );
        with_internal_request("second", || {
            assert_eq!(
                INTERNAL_REQUEST.with(|s| s.borrow().clone()).as_deref(),
                Some("second")
            )
        });
        assert_eq!(
            INTERNAL_REQUEST.with(|s| s.borrow().clone()).as_deref(),
            Some("first")
        );
    });
    assert!(INTERNAL_REQUEST.with(|s| s.borrow().is_none()));
    let result =
        std::panic::catch_unwind(|| with_internal_request("panic", || panic!("synthetic")));
    assert!(result.is_err());
    assert!(INTERNAL_REQUEST.with(|s| s.borrow().is_none()));
}
#[test]
fn existing_composition_survives_ordinary_busy_drain_but_clipboard_does_not() {
    assert!(input_state_allows(false, false, false, false));
    assert!(!input_state_allows(true, false, false, false));
    assert!(input_state_allows(true, false, false, true));
    // Draining a formatting playback is different from ordinary draft drain.
    assert!(!input_state_allows(true, false, true, true));
    assert!(!input_state_allows(false, false, true, true));
    assert!(!input_state_allows(true, true, false, true));
    assert!(!input_state_allows(false, true, false, true));
}
#[test]
fn tool_action_rejects_completed_playback_even_with_unchanged_generation() {
    assert!(captured_context_matches(12, 40, 12, 40));
    assert!(!captured_context_matches(12, 40, 13, 40));
    assert!(!captured_context_matches(12, 40, 12, 42));
    assert!(!captured_context_matches(12, 40, 13, 42));
    let gate = SequenceInputGate::default();
    let original = gate.user_permit().unwrap();
    assert!(gate.allows_user(original));
    let lease = gate.acquire().unwrap();
    assert!(!gate.allows_user(original));
    gate.release(lease);
    assert!(!gate.allows_user(original));
    assert!(gate.allows_user(gate.user_permit().unwrap()));
}
#[test]
fn tool_input_never_inherits_composition_busy_exception() {
    for busy in [false, true] {
        for saving in [false, true] {
            for playback in [false, true] {
                assert_eq!(
                    input_state_allows(busy, saving, playback, false),
                    !busy && !saving && !playback
                );
            }
        }
    }
}
#[test]
fn draft_authority_grants_only_explicit_drain_or_saved_step_during_playback() {
    for kind in [DraftMutation::Save, DraftMutation::Discard] {
        assert!(mutation_authority_allows(
            MutationAuthority::Ordinary,
            kind,
            false,
            false
        ));
        assert!(!mutation_authority_allows(
            MutationAuthority::Ordinary,
            kind,
            true,
            true
        ));
        assert!(!mutation_authority_allows(
            MutationAuthority::Ordinary,
            kind,
            true,
            false
        ));
        assert!(mutation_authority_allows(
            MutationAuthority::Drain,
            kind,
            true,
            true
        ));
        assert!(!mutation_authority_allows(
            MutationAuthority::Drain,
            kind,
            true,
            false
        ));
        assert!(!mutation_authority_allows(
            MutationAuthority::Drain,
            kind,
            false,
            false
        ));
    }
    assert!(mutation_authority_allows(
        MutationAuthority::Step,
        DraftMutation::Save,
        true,
        false
    ));
    assert!(!mutation_authority_allows(
        MutationAuthority::Step,
        DraftMutation::Save,
        true,
        true
    ));
    assert!(!mutation_authority_allows(
        MutationAuthority::Step,
        DraftMutation::Discard,
        true,
        false
    ));
    assert!(!mutation_authority_allows(
        MutationAuthority::Step,
        DraftMutation::Save,
        false,
        false
    ));
}
fn inert_mutation(kind: DraftMutation, epoch: u64) -> DraftMutationPermit {
    DraftMutationPermit {
        pane: Weak::new(),
        owner: Weak::new(),
        project_id: "synthetic".into(),
        generation: 0,
        epoch,
        kind,
        authority: MutationAuthority::Ordinary,
        lease: None,
    }
}
#[test]
fn mutation_scope_is_temporary_nested_and_unwind_safe() {
    assert!(DRAFT_MUTATION.with(|scope| scope.borrow().is_none()));
    with_mutation_capability(inert_mutation(DraftMutation::Save, 1), || {
        assert_eq!(
            DRAFT_MUTATION.with(|s| s.borrow().as_ref().unwrap().kind),
            DraftMutation::Save
        );
        with_mutation_capability(inert_mutation(DraftMutation::Discard, 2), || {
            assert_eq!(
                DRAFT_MUTATION.with(|s| s.borrow().as_ref().unwrap().epoch),
                2
            );
        });
        assert_eq!(
            DRAFT_MUTATION.with(|s| s.borrow().as_ref().unwrap().epoch),
            1
        );
    });
    assert!(DRAFT_MUTATION.with(|scope| scope.borrow().is_none()));
    let result = std::panic::catch_unwind(|| {
        with_mutation_capability(inert_mutation(DraftMutation::Save, 3), || {
            panic!("synthetic")
        })
    });
    assert!(result.is_err());
    assert!(DRAFT_MUTATION.with(|scope| scope.borrow().is_none()));
}

#[test]
fn saved_shortcut_load_or_context_refusal_never_starts_commands() {
    for error in [
        "Unambiguous settings refused",
        "Original input epoch changed",
        "Visible owner changed",
    ] {
        let called = Rc::new(Cell::new(0));
        let count = called.clone();
        let result = continue_saved_play::<()>(
            Err(error.into()),
            Box::new(move |outcome| {
                count.set(count.get() + 1);
                assert_eq!((outcome.accepted_steps, outcome.saved_steps), (0, 0));
                assert_eq!(outcome.error.as_deref(), Some(error));
            }),
            |_, _| panic!("Refused load/context must not begin playback"),
        );
        assert!(result.is_err());
        assert_eq!(called.get(), 1);
    }
}
#[test]
fn saved_shortcut_missing_slot_reports_start_refusal_once() {
    let called = Rc::new(Cell::new(0));
    let count = called.clone();
    let result = continue_saved_play(
        Ok(()),
        Box::new(move |outcome| {
            count.set(count.get() + 1);
            assert_eq!(outcome.saved_steps, 0);
            assert!(outcome.error.unwrap().contains("No sequence"));
        }),
        |_, _| Err("No sequence is saved in this slot.".into()),
    );
    assert!(result.is_err());
    assert_eq!(called.get(), 1);
}
#[test]
fn saved_shortcut_success_retains_completion_until_playback_acknowledges() {
    let called = Rc::new(Cell::new(0));
    let count = called.clone();
    let pending = Rc::new(RefCell::new(None));
    let after = pending.clone();
    continue_saved_play(
        Ok(()),
        Box::new(move |outcome| {
            count.set(count.get() + 1);
            assert_eq!(outcome.saved_steps, 3);
        }),
        move |_, done| {
            *after.borrow_mut() = Some(done);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(called.get(), 0);
    pending.borrow_mut().take().unwrap()(PlaybackOutcome {
        accepted_steps: 3,
        saved_steps: 3,
        error: None,
    });
    assert_eq!(called.get(), 1);
}

fn wait(condition: impl Fn() -> bool) {
    let started = Instant::now();
    let context = glib::MainContext::default();
    while !condition() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "Sequence host operation timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    while context.pending() {
        context.iteration(false);
    }
}
fn rpc(client: &Process, op: &str, args: Value) -> Result<Value, String> {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    client.rpc(op, args, move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some());
    let value = result.borrow_mut().take().unwrap();
    value
}
fn save_settings(
    client: &Process,
    pid: &str,
    snapshot: SettingsSnapshot,
) -> Result<SettingsSnapshot, String> {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    rpc_settings_save(
        client,
        pid,
        snapshot,
        Box::new(move |r| *out.borrow_mut() = Some(r)),
    );
    wait(|| result.borrow().is_some());
    let value = result.borrow_mut().take().unwrap();
    value
}
/// Headless actual companion RPC test. Explicitly opt in; it never initializes GTK.
#[test]
#[ignore = "Requires NOVEL_SEQUENCE_TEST_COMPANION pointing to an already-built companion; synthetic temporary store only"]
fn real_settings_rpc_unambiguous_load_and_checked_cas() {
    let executable = PathBuf::from(
        std::env::var_os("NOVEL_SEQUENCE_TEST_COMPANION").expect("Already-built companion path"),
    );
    let fixture = tempfile::tempdir().unwrap();
    let client = Process::start(&executable, fixture.path(), true).unwrap();
    let project = rpc(
        &client,
        "create_novel",
        json!({"title":"Synthetic sequences"}),
    )
    .unwrap();
    let pid = project["project"]["id"].as_str().unwrap();
    let first = SettingsSnapshot::from_rpc(
        &rpc(
            &client,
            "load_project_data_unambiguous",
            json!({"projectId":pid,"kind":"settings"}),
        )
        .unwrap(),
    )
    .unwrap();
    let first = save_settings(
        &client,
        pid,
        SettingsSnapshot {
            payload: json!({"schemaVersion":1,"unrelated":"مِ 中文 𠀀 👩🏽‍🚀"}),
            parent: first.parent,
        },
    )
    .unwrap();
    let payload = settings().merge_into(&first.payload).unwrap();
    let saved = save_settings(
        &client,
        pid,
        SettingsSnapshot {
            payload: payload.clone(),
            parent: first.parent.clone(),
        },
    )
    .unwrap();
    let cursor = rpc(&client, "sync_cursor", json!({})).unwrap();
    let error = save_settings(&client, pid, first).unwrap_err();
    assert!(error.contains("Conflict"));
    assert_eq!(rpc(&client, "sync_cursor", json!({})).unwrap(), cursor);
    let read = SettingsSnapshot::from_rpc(
        &rpc(
            &client,
            "load_project_data_unambiguous",
            json!({"projectId":pid,"kind":"settings"}),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(read, saved);
    assert_eq!(read.payload, payload);
    assert_eq!(read.payload["unrelated"], "مِ 中文 𠀀 👩🏽‍🚀");
    client.stop();
}

fn request(p: &Rc<EditorPane>, value: Value, editing: bool) -> Result<Value, String> {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    p.request(value, editing, move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some() && p.pending.get() == 0);
    let value = result.borrow_mut().take().unwrap();
    value
}
fn find_buttons(widget: &impl IsA<gtk::Widget>, name: &str) -> Vec<gtk::Button> {
    let mut buttons = vec![];
    if let Ok(b) = widget.as_ref().clone().downcast::<gtk::Button>() {
        if b.label().as_deref() == Some(name) {
            buttons.push(b);
        }
    }
    if let Ok(c) = widget.as_ref().clone().downcast::<gtk::Container>() {
        for w in c.children() {
            buttons.extend(find_buttons(&w, name));
        }
    }
    buttons
}
fn finish_recording(binding: &Rc<PaneBinding>) -> Result<(), String> {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    binding.save_recording(Box::new(move |r| *out.borrow_mut() = Some(r)));
    wait(|| result.borrow().is_some());
    let value = result.borrow_mut().take().unwrap();
    value
}
fn play(a: &Rc<Workspace>, p: &Rc<EditorPane>) -> PlaybackOutcome {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    a.sequence_play(1, p, Box::new(move |r| *out.borrow_mut() = Some(r)))
        .unwrap();
    wait(|| result.borrow().is_some());
    let value = result.borrow_mut().take().unwrap();
    value
}
fn save_page(a: &Rc<Workspace>, p: &Rc<SequencePage>) -> Result<(), String> {
    assert_eq!(a.current_widget(), Some(p.widget.clone().upcast()));
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    // Exercise root's actual Draft registration/Save routing, not a direct call.
    a.save_current(move |r| *out.borrow_mut() = Some(r));
    wait(|| result.borrow().is_some());
    let value = result.borrow_mut().take().unwrap();
    value
}
fn rich() -> Value {
    json!({"schemaVersion":1,"title":"Sequence draft مِ 中文","document":{"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"مِ 中文 𠀀 👩🏽‍🚀 e\u{301}"}]}]}})
}
fn fixture(
    binaries: &Path,
) -> (
    tempfile::TempDir,
    Rc<Workspace>,
    Rc<EditorPane>,
    String,
    String,
) {
    assert!(
        gtk::is_initialized_main_thread(),
        "Parent initializes and serializes GTK"
    );
    let fixture = tempfile::tempdir().unwrap();
    let data = fixture.path().join("sequence-data");
    let cache = fixture.path().join("sequence-cache");
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
        "create_novel",
        json!({"title":"Synthetic sequence host"}),
    )
    .unwrap();
    let pid = project["project"]["id"].as_str().unwrap().to_owned();
    let sid = project["scene"]["id"].as_str().unwrap().to_owned();
    let original = rpc(&client, "load_scene", json!({"sceneId":sid})).unwrap();
    rpc(
        &client,
        "save_scene",
        json!({"sceneId":sid,"parent":original["revisionId"],"scene":rich()}),
    )
    .unwrap();
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    let p = a.active.borrow().clone().unwrap();
    a.sequence_bind_pane(&p);
    (fixture, a, p, pid, sid)
}
fn open_page(a: &Rc<Workspace>, p: &Rc<EditorPane>) -> Rc<SequencePage> {
    a.install(&p.widget, &p.title.borrow(), false);
    a.command_sequences();
    wait(|| {
        a.sequence_host
            .pages
            .borrow()
            .iter()
            .any(|p| p.loaded.get() && a.current_widget() == Some(p.widget.clone().upcast()))
    });
    a.sequence_page(&a.current_widget().unwrap()).unwrap()
}
fn confirm_leave(a: &Rc<Workspace>, choice: RecordingChoice) -> bool {
    TEST_RECORDING_CHOICES.with(|q| q.borrow_mut().push_back(choice));
    let result = Rc::new(Cell::new(None));
    let out = result.clone();
    a.sequence_confirm_leave(None, move |ok| out.set(Some(ok)));
    wait(|| result.get().is_some());
    result.get().unwrap()
}
fn discard_scene(p: &Rc<EditorPane>) {
    let result = Rc::new(Cell::new(false));
    let out = result.clone();
    p.discard(move |r| {
        r.unwrap();
        out.set(true);
    });
    wait(|| result.get());
}

/// Parent calls this from its existing opt-in GTK worker workflow after applying
/// the reported Draft/editor/lifecycle hooks. It uses actual buttons, editor worker,
/// settings CAS, scene saves, Undo and live-window ownership. It is NOT #[test].
pub(crate) fn run(binaries: &Path) {
    let (_fixture, a, p, pid, sid) = fixture(binaries);
    let base = rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap();
    let binding = binding_for(&p).unwrap();
    let page = open_page(&a, &p);
    page.controls.names[0].set_text("Format مِ 中文");
    assert!(page.dirty.get());
    let marker = rpc(&a.client, "sync_cursor", json!({})).unwrap();
    assert!(save_page(&a, &page).unwrap_err().contains("needs recorded"));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    wait(|| p.pending.get() == 0 && !p.saving.get() && !a.desk.busy.get());
    find_buttons(&page.widget, "Record")[0].clicked();
    assert!(binding.runtime.recording().is_some(), "{}", binding.runtime.message.borrow());
    assert_eq!(a.current_widget(), Some(p.widget.clone().upcast()));
    request(&p, json!({"type":"command","command":"selectAll"}), false).unwrap();
    for operation in [
        json!({"type":"command","command":"bold"}),
        json!({"type":"command","command":"lineSpacing","lineSpacing":1.5}),
        json!({"type":"command","command":"italic"}),
    ] {
        request(&p, operation, true).unwrap();
    }
    // This assertion deliberately requires the root request acknowledgement hook.
    assert_eq!(
        binding.runtime.recording().unwrap().sequence.actions.len(),
        3
    );
    let edited = p.frame.borrow()["document"].clone();
    super::super::desk::tests::capture(&a, "notebook-sequence-recording");
    assert!(p.dirty.get());
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap(),
        base
    );
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    assert!(!confirm_leave(&a, RecordingChoice::Cancel));
    assert!(binding.runtime.recording().is_some());
    assert_eq!(p.frame.borrow()["document"], edited);
    // These queues prevent an interactive fallback dialog if a parent omitted
    // its lifecycle hook; the consumed recording queue proves that hook ran.
    TEST_RECORDING_CHOICES.with(|q| q.borrow_mut().push_back(RecordingChoice::Cancel));
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.close();
    wait(|| !a.closing.get());
    assert!(!a.desk.is_closed());
    assert!(
        TEST_RECORDING_CHOICES.with(|q| q.borrow().is_empty()),
        "Root close must decide about recording before ordinary drafts"
    );
    a.desk.choices.borrow_mut().clear();
    let other = rpc(
        &a.client,
        "create_novel",
        json!({"title":"Other synthetic sequence project"}),
    )
    .unwrap();
    TEST_RECORDING_CHOICES.with(|q| q.borrow_mut().push_back(RecordingChoice::Cancel));
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.switch_project(other["project"]["id"].as_str().unwrap().into());
    wait(|| !a.loading.get());
    assert_eq!(*a.pid.borrow(), pid);
    assert!(binding.runtime.recording().is_some());
    assert!(
        TEST_RECORDING_CHOICES.with(|q| q.borrow().is_empty()),
        "Root project change must decide about recording first"
    );
    a.desk.choices.borrow_mut().clear();
    finish_recording(&binding).unwrap();
    assert!(binding.runtime.recording().is_none());
    assert!(!page.dirty.get());
    assert!(p.dirty.get());
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap(),
        base
    );
    let saved = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":pid,"kind":"settings"}),
    )
    .unwrap();
    assert_eq!(
        NativeSequenceSettings::from_project_settings(&saved["payload"])
            .unwrap()
            .sequences[0]
            .actions
            .len(),
        3
    );
    a.install(&page.widget, "Command sequences", false);
    super::super::desk::tests::capture(&a, "notebook-command-sequences");
    a.install(&p.widget, &p.title.borrow(), false);
    wait(|| p.pending.get() == 0);
    // Play performs the real initial manual choice; Cancel performs no command/save.
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    let cancelled = play(&a, &p);
    assert_eq!((cancelled.accepted_steps, cancelled.saved_steps), (0, 0));
    assert!(cancelled.error.is_some());
    assert_eq!(p.frame.borrow()["document"], edited);
    assert!(!a.sequence_window_blocked());
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Discard);
    let queued = input_permit(&p).unwrap();
    let accepted = play(&a, &p);
    assert_eq!(accepted.saved_steps, 3);
    assert!(accepted.error.is_none());
    assert!(!input_allowed(&p, &queued));
    assert!(!p.dirty.get());
    assert_eq!(
        novel_storage::plain_text(&p.frame.borrow()["document"]),
        novel_storage::plain_text(&base["scene"]["document"])
    );
    // Every accepted saved step remains undoable on the actual worker.
    for _ in 0..3 {
        request(&p, json!({"type":"command","command":"undo"}), true).unwrap();
    }
    assert_eq!(p.frame.borrow()["document"], base["scene"]["document"]);
    discard_scene(&p);
    // The actual acknowledgement hook enforces 32 while the 33rd normal editor
    // edit remains a manual unsaved draft. No settings or scene write occurs.
    binding
        .runtime
        .start_recording(binding.target.clone(), 8, "Bounded fixture".into())
        .unwrap();
    let marker = rpc(&a.client, "sync_cursor", json!({})).unwrap();
    for index in 0..33 {
        request(&p,json!({"type":"command","command":"lineSpacing","lineSpacing":if index%2==0{2.0}else{1.5}}),true).unwrap();
    }
    assert_eq!(
        binding.runtime.recording().unwrap().sequence.actions.len(),
        32
    );
    assert!(p.dirty.get());
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    // Recovery refresh must not kill a still-recording worker or its rich Undo.
    let retained=p.frame.borrow()["document"].clone();
    a.reload_scene(&sid);
    assert!(Rc::ptr_eq(a.panes.borrow().get(&sid).unwrap(),&p));
    assert_eq!(p.frame.borrow()["document"],retained);
    assert!(binding.runtime.recording().is_some());
    // Naming a new novel is not a commitment until recording decisions accept.
    TEST_RECORDING_CHOICES.with(|q|q.borrow_mut().push_back(RecordingChoice::Cancel));
    glib::idle_add_local_once(||{
        let dialog=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Dialog>().ok()).find(|d|d.is_visible()&&d.title().as_deref()==Some("New novel")).unwrap();
        dialog.content_area().children().into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap().set_text("Cancelled recording novel");
        dialog.response(gtk::ResponseType::Accept);
    });
    a.new_project();
    wait(||!a.loading.get());
    assert_eq!(rpc(&a.client,"sync_cursor",json!({})).unwrap(),marker);
    assert_eq!(*a.pid.borrow(),pid);
    assert!(TEST_RECORDING_CHOICES.with(|q|q.borrow().is_empty()));
    assert!(binding.runtime.recording().is_some());
    cancel_recording(&p).unwrap();
    assert!(p.dirty.get());
    discard_scene(&p);
    p.composing.set(true);
    assert!(a
        .sequence_play(1, &p, Box::new(|_| panic!("must not start")))
        .is_err());
    p.composing.set(false);
    // Real name settings CAS; stale competing saves preserve local entry edits.
    let page = open_page(&a, &p);
    page.controls.names[0].set_text("Renamed مِ 中文");
    save_page(&a, &page).unwrap();
    let current = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":pid,"kind":"settings"}),
    )
    .unwrap();
    let mut competing = current["payload"].clone();
    competing["anotherPreference"] = json!("Competing 中文");
    rpc(&a.client,"save_project_data_checked",json!({"projectId":pid,"kind":"settings","payload":competing,"parent":current["revisionId"],"dependencies":[]})).unwrap();
    page.controls.names[0].set_text("Retained stale name");
    let marker = rpc(&a.client, "sync_cursor", json!({})).unwrap();
    assert!(save_page(&a, &page).unwrap_err().contains("Conflict"));
    assert!(page.dirty.get());
    assert_eq!(page.controls.names[0].text(), "Retained stale name");
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    page.discard();
    binding.load(Box::new(|r| r.unwrap()));
    wait(|| !binding.loading.get());
    // Cancel recording keeps actual draft edits/Undo without a scene write.
    a.install(&p.widget, &p.title.borrow(), false);
    binding
        .runtime
        .start_recording(binding.target.clone(), 2, "Cancel fixture".into())
        .unwrap();
    let before = p.frame.borrow()["document"].clone();
    let marker = rpc(&a.client, "sync_cursor", json!({})).unwrap();
    request(
        &p,
        json!({"type":"command","command":"lineSpacing","lineSpacing":2}),
        true,
    )
    .unwrap();
    let edited = p.frame.borrow()["document"].clone();
    assert!(confirm_leave(&a, RecordingChoice::Discard));
    assert_eq!(p.frame.borrow()["document"], edited);
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    request(&p, json!({"type":"command","command":"undo"}), true).unwrap();
    assert_eq!(p.frame.borrow()["document"], before);
    discard_scene(&p);
    // Transfer the exact pane/binding/banner while recording. The original window
    // can close; the child continues explicit sequence settings and manuscript saves.
    binding
        .runtime
        .start_recording(binding.target.clone(), 2, "Detached fixture".into())
        .unwrap();
    request(
        &p,
        json!({"type":"command","command":"lineSpacing","lineSpacing":2}),
        true,
    )
    .unwrap();
    let permit = input_permit(&p).unwrap();
    a.detach_current();
    let child = desk::workspace_for_widget(p.widget.upcast_ref()).unwrap();
    assert!(!Rc::ptr_eq(&child, &a));
    assert!(!input_allowed(&p, &permit));
    assert!(Rc::ptr_eq(&binding_for(&p).unwrap(), &binding));
    for page in a.sequence_host.pages.borrow().iter() {
        page.discard();
    }
    a.close();
    wait(|| a.desk.is_closed());
    finish_recording(&binding).unwrap();
    assert!(p.dirty.get());
    let result = Rc::new(Cell::new(false));
    let out = result.clone();
    child.save_pane(p.clone(), move |r| {
        r.unwrap();
        out.set(true);
    });
    wait(|| result.get());
    child.close();
    wait(|| child.desk.is_closed());
}

/// Real multi-step save failure: a competing actual scene CAS is inserted AFTER
/// step 1's acknowledged save and BEFORE step 2. No executor/store mock is used.
/// Parent runs separately under the same serialized GTK gate.
pub(crate) fn failed_cas_keeps_earlier_saved_steps(binaries: &Path) {
    let (_fixture, a, p, pid, sid) = fixture(binaries);
    let binding = binding_for(&p).unwrap();
    let prior = SettingsSnapshot::from_rpc(
        &rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":pid,"kind":"settings"}),
        )
        .unwrap(),
    )
    .unwrap();
    save_settings(
        &a.client,
        &pid,
        SettingsSnapshot {
            payload: settings().merge_into(&prior.payload).unwrap(),
            parent: prior.parent,
        },
    )
    .unwrap();
    binding.load(Box::new(|r| r.unwrap()));
    wait(|| binding.runtime.loaded() && !binding.loading.get());
    request(&p, json!({"type":"command","command":"selectAll"}), false).unwrap();
    let original = p.frame.borrow()["document"].clone();
    let first_head = Rc::new(RefCell::new(None));
    let head = first_head.clone();
    let client = a.client.clone();
    let pane = p.clone();
    let scene_id = sid.clone();
    *binding.after_saved_step.borrow_mut() = Some(Box::new(move |done| {
        assert!(pane.dirty.get() == false);
        *head.borrow_mut() = Some(pane.parent.borrow().clone());
        client.rpc("save_scene",json!({"sceneId":scene_id,"scene":{"schemaVersion":1,"title":"Competing title 中文","document":pane.saved_document.borrow().clone()},"parent":pane.parent.borrow().clone(),"requestId":uuid::Uuid::new_v4().to_string()}),move|reply|done(reply.map(|_|())));
    }));
    let outcome = play(&a, &p);
    assert_eq!((outcome.accepted_steps, outcome.saved_steps), (2, 1));
    assert!(outcome.error.as_ref().unwrap().contains("Conflict"));
    assert!(p.dirty.get());
    assert!(!a.sequence_window_blocked());
    assert!(input_ready(&p));
    let step_one = rpc(
        &a.client,
        "load_revision",
        json!({"sceneId":sid,"revisionId":first_head.borrow().clone().unwrap()}),
    )
    .unwrap();
    assert_eq!(
        p.saved_document.borrow().clone(),
        step_one["scene"]["document"]
    );
    assert_ne!(p.frame.borrow()["document"], step_one["scene"]["document"]);
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap()["scene"]["document"],
        step_one["scene"]["document"]
    );
    request(&p, json!({"type":"command","command":"undo"}), true).unwrap();
    assert_eq!(p.frame.borrow()["document"], step_one["scene"]["document"]);
    request(&p, json!({"type":"command","command":"undo"}), true).unwrap();
    assert_eq!(p.frame.borrow()["document"], original);
    discard_scene(&p);
    a.close();
    wait(|| a.desk.is_closed());
}

/// Parent's initialized GTK workflow can additionally exercise these narrow audit
/// hooks against actual editor/companion workers. Never runs as a cargo test.
pub(crate) fn mutation_composition_and_saved_shortcut_guards(binaries: &Path) {
    let (_fixture, a, p, pid, sid) = fixture(binaries);
    let binding = binding_for(&p).unwrap();
    let before = p.frame.borrow()["document"].clone();
    let marker = rpc(&a.client, "sync_cursor", json!({})).unwrap();
    let preedit = input_permit(&p).unwrap();
    a.desk.busy.set(true);
    assert!(composition_allowed(&p, &preedit));
    assert!(!input_allowed(&p, &preedit));
    // Emulate delivery of the already-started preedit word through real request.
    request(
        &p,
        json!({"type":"text","text":"Existing preedit مِ 中文 "}),
        true,
    )
    .unwrap();
    a.desk.busy.set(false);
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    discard_scene(&p);
    let ordinary = authorize_draft_mutation(&p, DraftMutation::Save).unwrap();
    let input = input_permit(&p).unwrap();
    let executor = HostExecutor {
        binding: binding.clone(),
    };
    executor.block_input().unwrap();
    assert!(!composition_allowed(&p, &input));
    assert!(authorize_draft_mutation(&p, DraftMutation::Save).is_err());
    assert!(authorize_draft_mutation(&p, DraftMutation::Discard).is_err());
    assert!(!draft_mutation_allowed(&p, &ordinary, DraftMutation::Save));
    // Snapshot is read-only and remains available while the mutation gate is held.
    let snapshot = Rc::new(RefCell::new(None));
    let out = snapshot.clone();
    p.snapshot(move |r| *out.borrow_mut() = Some(r));
    wait(|| snapshot.borrow().is_some());
    assert_eq!(snapshot.borrow_mut().take().unwrap().unwrap(), before);
    let drain = with_drain_draft_mutation(&p, DraftMutation::Discard, || {
        assert!(authorize_draft_mutation(&p, DraftMutation::Save).is_err());
        authorize_draft_mutation(&p, DraftMutation::Discard).unwrap()
    })
    .unwrap();
    assert!(draft_mutation_allowed(&p, &drain, DraftMutation::Discard));
    assert!(!draft_mutation_allowed(&p, &drain, DraftMutation::Save));
    let discarded = Rc::new(Cell::new(false));
    let out = discarded.clone();
    with_drain_draft_mutation(&p, DraftMutation::Discard, || {
        p.discard(move |result| {
            result.unwrap();
            out.set(true);
        });
    })
    .unwrap();
    wait(|| discarded.get());
    binding.lease.borrow().as_ref().unwrap().draining.set(false);
    assert!(!draft_mutation_allowed(&p, &drain, DraftMutation::Discard));
    assert!(with_drain_draft_mutation(&p, DraftMutation::Save, || ()).is_err());
    executor.unblock_input();
    assert!(!draft_mutation_allowed(&p, &ordinary, DraftMutation::Save));
    assert!(!draft_mutation_allowed(&p, &drain, DraftMutation::Discard));
    assert!(!composition_allowed(&p, &input));
    let snapshot = SettingsSnapshot::from_rpc(
        &rpc(
            &a.client,
            "load_project_data_unambiguous",
            json!({"projectId":pid,"kind":"settings"}),
        )
        .unwrap(),
    )
    .unwrap();
    save_settings(
        &a.client,
        &pid,
        SettingsSnapshot {
            payload: settings().merge_into(&snapshot.payload).unwrap(),
            parent: snapshot.parent,
        },
    )
    .unwrap();
    assert!(!binding.runtime.loaded());
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    let generation = a.desk.generation.get();
    a.sequence_play_saved(1, &p, Box::new(move |r| *out.borrow_mut() = Some(r)))
        .unwrap();
    // A completed playback elsewhere in this same window changes the window
    // epoch without changing this pane's own gate or desk generation.
    a.sequence_host
        .mutation_epoch
        .set(a.sequence_host.mutation_epoch.get() + 2);
    wait(|| result.borrow().is_some());
    let refused = result.borrow_mut().take().unwrap();
    assert_eq!((refused.accepted_steps, refused.saved_steps), (0, 0));
    assert!(refused.error.is_some());
    assert_eq!(a.desk.generation.get(), generation);
    assert_eq!(p.frame.borrow()["document"], before);
    assert!(!p.dirty.get());
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap()["scene"]["document"],
        before
    );
    assert!(binding.runtime.loaded());
    request(&p, json!({"type":"command","command":"selectAll"}), false).unwrap();
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    a.sequence_play_saved(1, &p, Box::new(move |r| *out.borrow_mut() = Some(r)))
        .unwrap();
    wait(|| result.borrow().is_some());
    assert_eq!(result.borrow().as_ref().unwrap().saved_steps, 3);
    assert!(!p.dirty.get());
    a.close();
    wait(|| a.desk.is_closed());
}

/// Parent calls after GTK initialization, with real editor/companion workers.
/// Covers unmounted cached targets, retained author permits, async creation,
/// ownership/generation changes and a completed same-window playback epoch.
pub(crate) fn cached_tool_input_guards(binaries: &Path) {
    let (_fixture, a, p, pid, sid) = fixture(binaries);
    let before = p.frame.borrow()["document"].clone();
    let cursor = rpc(&a.client, "sync_cursor", json!({})).unwrap();
    let tool_page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    a.pages.remove_page(a.pages.page_num(&p.widget));
    a.install(&tool_page, "Synthetic tool", false);
    assert!(a.pages.page_num(&p.widget).is_none());
    assert!(input_permit(&p).is_none());
    let window = window_input_permit(&a).unwrap();
    assert!(window_input_allowed(&a, &window));
    let permit = tool_input_permit(&a, &p).unwrap();
    assert!(tool_input_allowed(&p, &permit));
    assert!(!input_allowed(&p, &permit));
    assert!(!composition_allowed(&p, &permit));
    let action = tool_action_permit(&a, &sid).unwrap();
    assert!(tool_input_after_ensure(&p, &action).is_some());
    a.desk.busy.set(true);
    assert!(!window_input_allowed(&a, &window));
    assert!(window_input_permit(&a).is_none());
    assert!(!tool_input_allowed(&p, &permit));
    assert!(tool_input_permit(&a, &p).is_none());
    assert!(tool_input_after_ensure(&p, &action).is_none());
    a.desk.busy.set(false);
    p.saving.set(true);
    assert!(!tool_input_allowed(&p, &permit));
    p.saving.set(false);
    let generation = a.desk.generation.get();
    a.desk.generation.set(generation + 1);
    assert!(!window_input_allowed(&a, &window));
    assert!(!tool_input_allowed(&p, &permit));
    assert!(tool_input_after_ensure(&p, &action).is_none());
    a.desk.generation.set(generation);
    *a.pid.borrow_mut() = "different-project".into();
    assert!(!window_input_allowed(&a, &window));
    assert!(!tool_input_allowed(&p, &permit));
    assert!(tool_input_after_ensure(&p, &action).is_none());
    *a.pid.borrow_mut() = pid.clone();
    let binding = binding_for(&p).unwrap();
    let gate = binding.gate.acquire().unwrap();
    assert!(!tool_input_allowed(&p, &permit));
    binding.gate.release(gate);
    assert!(!tool_input_allowed(&p, &permit));
    assert!(tool_input_after_ensure(&p, &action).is_none());
    let fresh = tool_input_permit(&a, &p).unwrap();
    assert!(tool_input_allowed(&p, &fresh));
    // Real formatting of the cached worker remains a draft, with Undo available.
    request(&p, json!({"type":"command","command":"selectAll"}), false).unwrap();
    assert!(tool_input_allowed(&p, &fresh));
    request(&p, json!({"type":"command","command":"bold"}), true).unwrap();
    assert!(p.dirty.get());
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), cursor);
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap()["scene"]["document"],
        before
    );
    request(&p, json!({"type":"command","command":"undo"}), true).unwrap();
    assert_eq!(p.frame.borrow()["document"], before);
    discard_scene(&p);
    // Remove the old registered instance to exercise alternatives' async creation.
    a.panes.borrow_mut().remove(&sid);
    let create_window = window_input_permit(&a).unwrap();
    let create = tool_action_permit(&a, &sid).unwrap();
    assert!(create.existing.is_none());
    let reply = Rc::new(RefCell::new(None));
    let out = reply.clone();
    a.ensure_pane(&sid, move |r| *out.borrow_mut() = Some(r));
    wait(|| reply.borrow().is_some());
    let replacement = reply.borrow_mut().take().unwrap().unwrap();
    assert!(window_input_allowed(&a, &create_window));
    let delayed = tool_input_after_ensure(&replacement, &create).unwrap();
    wait(|| replacement.open.get() && replacement.pending.get() == 0);
    assert!(!Rc::ptr_eq(&p, &replacement));
    assert!(!tool_input_allowed(&p, &fresh));
    assert!(!tool_input_allowed(&replacement, &fresh));
    assert!(tool_input_after_ensure(&replacement, &action).is_none());
    assert!(window_input_allowed(&a, &create_window));
    assert!(tool_input_allowed(&replacement, &delayed));
    // Notebook's original window capability survives readiness polling. Exact
    // registration/current page/readiness remain independent delivery checks.
    a.show_pane(replacement.clone(), false);
    assert!(window_input_allowed(&a, &create_window));
    assert!(a
        .panes
        .borrow()
        .get(&sid)
        .is_some_and(|p| Rc::ptr_eq(p, &replacement)));
    assert_eq!(
        a.current_widget(),
        Some(replacement.widget.clone().upcast())
    );
    assert!(
        replacement.open.get()
            && !replacement.composing.get()
            && !replacement.saving.get()
            && replacement.pending.get() == 0
    );
    request(
        &replacement,
        json!({"type":"key","key":"End","ctrl":true,"shift":false}),
        false,
    )
    .unwrap();
    request(
        &replacement,
        json!({"type":"text","text":"\n\nSynthetic scrap مِ 中文"}),
        true,
    )
    .unwrap();
    assert!(replacement.dirty.get());
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), cursor);
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap()["scene"]["document"],
        before
    );
    request(
        &replacement,
        json!({"type":"command","command":"undo"}),
        true,
    )
    .unwrap();
    assert_eq!(replacement.frame.borrow()["document"], before);
    discard_scene(&replacement);
    // A playback completed during creation/source checks cannot mint a new permit.
    a.sequence_host
        .mutation_epoch
        .set(a.sequence_host.mutation_epoch.get() + 2);
    assert!(!window_input_allowed(&a, &create_window));
    assert!(tool_input_after_ensure(&replacement, &create).is_none());
    assert!(!tool_input_allowed(&replacement, &delayed));
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), cursor);
    p.process.stop();
    a.close();
    wait(|| a.desk.is_closed());
}

/// Parent runs after applying app/desk lifecycle hooks and initializing GTK.
/// Actual close/project-switch entry points must consume the recording decision
/// even when an ordinary tool has replaced the scene tab. Manuscript choices
/// remain separate; settings Save and recording Discard never save draft writing.
pub(crate) fn cached_recording_lifecycle(binaries: &Path) {
    let (_fixture, a, p, pid, sid) = fixture(binaries);
    let binding = binding_for(&p).unwrap();
    let saved_scene = rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap();
    let tool = gtk::Box::new(gtk::Orientation::Vertical, 0);
    a.install(&tool, "Synthetic ordinary tool", true);
    assert!(binding.owner().is_err());
    assert!(binding.recording_owner().is_ok());
    // First load must work while this exact editor is registered but unmounted.
    let loaded = Rc::new(RefCell::new(None));
    let out = loaded.clone();
    binding.load(Box::new(move |r| *out.borrow_mut() = Some(r)));
    wait(|| loaded.borrow().is_some());
    loaded.borrow_mut().take().unwrap().unwrap();
    let other = rpc(
        &a.client,
        "create_novel",
        json!({"title":"Cached recording departure"}),
    )
    .unwrap();
    let other_id = other["project"]["id"].as_str().unwrap().to_owned();
    a.show_pane(p.clone(), false);
    request(&p, json!({"type":"command","command":"selectAll"}), false).unwrap();
    binding
        .runtime
        .start_recording(binding.target.clone(), 3, "Cached recording مِ 中文".into())
        .unwrap();
    request(&p, json!({"type":"command","command":"bold"}), true).unwrap();
    let edited = p.frame.borrow()["document"].clone();
    let actions = binding
        .runtime
        .recording()
        .unwrap()
        .sequence
        .actions
        .clone();
    assert_eq!(
        actions.len(),
        1,
        "Root editor acknowledgement hook must record the accepted format"
    );
    let physical = input_permit(&p).unwrap();
    a.install(&tool, "Synthetic ordinary tool", true);
    assert!(a
        .panes
        .borrow()
        .get(&sid)
        .is_some_and(|cached| Rc::ptr_eq(cached, &p)));
    assert!(a.pages.page_num(&p.widget).is_none());
    assert!(binding.visible().is_err());
    assert!(input_permit(&p).is_none());
    assert!(!input_allowed(&p, &physical));
    assert!(!composition_allowed(&p, &physical));
    assert!(a
        .sequence_play(
            3,
            &p,
            Box::new(|_| panic!("unmounted playback must refuse"))
        )
        .is_err());
    let marker = rpc(&a.client, "sync_cursor", json!({})).unwrap();
    assert!(a.sequence_clear().is_err());
    // Root close must reach recording Cancel before ordinary manuscript choices.
    TEST_RECORDING_CHOICES.with(|q| q.borrow_mut().push_back(RecordingChoice::Cancel));
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.close();
    wait(|| !a.closing.get());
    assert!(
        TEST_RECORDING_CHOICES.with(|q| q.borrow().is_empty()),
        "Close skipped cached recording decision"
    );
    a.desk.choices.borrow_mut().clear();
    assert!(!a.desk.is_closed());
    assert_eq!(
        binding.runtime.recording().unwrap().sequence.actions,
        actions
    );
    assert_eq!(p.frame.borrow()["document"], edited);
    assert!(a.sequence_clear().is_err());
    // The same cached recording must stop project teardown on Cancel.
    TEST_RECORDING_CHOICES.with(|q| q.borrow_mut().push_back(RecordingChoice::Cancel));
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.switch_project(other_id.clone());
    wait(|| !a.loading.get());
    assert!(
        TEST_RECORDING_CHOICES.with(|q| q.borrow().is_empty()),
        "Project switch skipped cached recording decision"
    );
    a.desk.choices.borrow_mut().clear();
    assert_eq!(*a.pid.borrow(), pid);
    assert!(binding.runtime.recording().is_some());
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    // Save only sequence settings, then independently Cancel manuscript departure.
    TEST_RECORDING_CHOICES.with(|q| q.borrow_mut().push_back(RecordingChoice::Save));
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.close();
    wait(|| !a.closing.get());
    assert!(TEST_RECORDING_CHOICES.with(|q| q.borrow().is_empty()));
    assert!(
        a.desk.choices.borrow().is_empty(),
        "Manuscript needs its own manual choice after sequence Save"
    );
    assert!(!a.desk.is_closed());
    assert!(binding.runtime.recording().is_none());
    assert!(p.dirty.get());
    assert_eq!(p.frame.borrow()["document"], edited);
    let settings = rpc(
        &a.client,
        "load_project_data_unambiguous",
        json!({"projectId":pid,"kind":"settings"}),
    )
    .unwrap();
    let stored = NativeSequenceSettings::from_project_settings(&settings["payload"]).unwrap();
    assert_eq!(stored.sequence(3).unwrap().actions, actions);
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap(),
        saved_scene
    );
    // A second recorded edit is Discarded on project departure; draft/Undo remain.
    a.show_pane(p.clone(), false);
    binding
        .runtime
        .start_recording(binding.target.clone(), 4, "Discard cached recording".into())
        .unwrap();
    request(&p, json!({"type":"command","command":"italic"}), true).unwrap();
    let second_edit = p.frame.borrow()["document"].clone();
    a.install(&tool, "Synthetic ordinary tool", true);
    let marker = rpc(&a.client, "sync_cursor", json!({})).unwrap();
    TEST_RECORDING_CHOICES.with(|q| q.borrow_mut().push_back(RecordingChoice::Discard));
    a.desk
        .choices
        .borrow_mut()
        .push_back(desk::SaveChoice::Cancel);
    a.switch_project(other_id);
    wait(|| !a.loading.get());
    assert!(TEST_RECORDING_CHOICES.with(|q| q.borrow().is_empty()));
    assert!(a.desk.choices.borrow().is_empty());
    assert_eq!(*a.pid.borrow(), pid);
    assert!(binding.runtime.recording().is_none());
    assert_eq!(p.frame.borrow()["document"], second_edit);
    assert!(p.dirty.get());
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    assert_eq!(
        rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap(),
        saved_scene
    );
    a.show_pane(p.clone(), false);
    request(&p, json!({"type":"command","command":"undo"}), true).unwrap();
    assert_eq!(p.frame.borrow()["document"], edited);
    request(&p, json!({"type":"command","command":"undo"}), true).unwrap();
    assert_eq!(
        p.frame.borrow()["document"],
        saved_scene["scene"]["document"]
    );
    discard_scene(&p);
    a.sequence_clear().unwrap();
    a.close();
    wait(|| a.desk.is_closed());
}

/// Exercises installed GTK key/pointer/IM/clipboard handlers with synthetic
/// events. This is not a physical keyboard or IME qualification.
pub(crate) fn installed_input_signal_guards(binaries: &Path) {
    fn key(p: &EditorPane, key: gdk::keys::Key, state: gdk::ModifierType) -> bool {
        use glib::translate::ToGlibPtr;
        let mut event = gdk::Event::new(gdk::EventType::KeyPress);
        let raw = event.downcast_mut::<gdk::EventKey>().unwrap().as_mut();
        raw.window = p.area.window().unwrap().to_glib_full();
        raw.keyval = *key;
        raw.state = state.bits();
        p.area.emit_by_name::<bool>("key-press-event", &[&event])
    }
    fn pointer(p: &EditorPane, down: bool) {
        use glib::translate::ToGlibPtr;
        let mut event = gdk::Event::new(if down { gdk::EventType::ButtonPress } else { gdk::EventType::ButtonRelease });
        let raw = event.downcast_mut::<gdk::EventButton>().unwrap().as_mut();
        raw.window = p.area.window().unwrap().to_glib_full();
        raw.button = 1;
        raw.x = 24.;
        raw.y = 24.;
        p.area.emit_by_name::<bool>(if down { "button-press-event" } else { "button-release-event" }, &[&event]);
    }
    fn clipboard_text() -> String {
        let result = Rc::new(RefCell::new(None));
        let out = result.clone();
        gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).request_text(move |_, text| {
            *out.borrow_mut() = Some(text.map(|text| text.to_string()).unwrap_or_default());
        });
        wait(|| result.borrow().is_some());
        let value = result.borrow_mut().take().unwrap();
        value
    }
    let (_fixture, a, p, pid, sid) = fixture(binaries);
    let binding = binding_for(&p).unwrap();
    let original = p.frame.borrow()["document"].clone();
    let prior = SettingsSnapshot::from_rpc(&rpc(&a.client, "load_project_data_unambiguous", json!({"projectId":pid,"kind":"settings"})).unwrap()).unwrap();
    let mut payload = settings().merge_into(&prior.payload).unwrap();
    let mut shortcuts = novel_storage::native_shortcuts::NativeShortcutSettings::default();
    shortcuts.bindings.insert("editor.sequence1".into(), novel_storage::native_shortcuts::parse_native_shortcut_list("Ctrl+Shift+M").unwrap());
    payload["nativeShortcuts"] = serde_json::to_value(shortcuts).unwrap();
    save_settings(&a.client, &pid, SettingsSnapshot { payload, parent: prior.parent }).unwrap();
    a.reload_shortcuts();
    wait(|| a.shortcuts.loaded.get());
    request(&p, json!({"type":"command","command":"selectAll"}), false).unwrap();
    // A newly bound pane has not opened its sequence page. Its configured actual
    // key handler loads the sole saved settings head before explicit playback.
    assert!(!binding.runtime.loaded());
    let first_parent = p.parent.borrow().clone();
    assert!(key(&p, gdk::keys::constants::M, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK));
    wait(|| binding.runtime.loaded() && !binding.loading.get() && *p.parent.borrow() != first_parent && !a.sequence_window_blocked() && p.pending.get() == 0 && !p.saving.get());
    assert!(binding.runtime.message.borrow().contains("Sequence completed"));
    assert!(!p.dirty.get());
    assert_eq!(novel_storage::plain_text(&p.frame.borrow()["document"]), novel_storage::plain_text(&original));

    // Actual asynchronous clipboard delivery is delayed by a real complete
    // three-step playback. Its old permit must fail even after input is restored.
    let stale = "Stale clipboard مِ 中文 𠀀 👩🏽‍🚀 e\u{301}";
    gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).set_text(stale);
    let delivered = Rc::new(Cell::new(false));
    let complete = delivered.clone();
    let expected = Rc::new(RefCell::new(None));
    let after = expected.clone();
    let owner = a.clone();
    let pane = p.clone();
    super::super::shortcuts::before_paste_delivery_once(move || {
        let outcome = play(&owner, &pane);
        assert_eq!(outcome.saved_steps, 3);
        assert!(outcome.error.is_none());
        *after.borrow_mut() = Some(pane.frame.borrow()["document"].clone());
        complete.set(true);
    });
    assert!(key(&p, gdk::keys::constants::v, gdk::ModifierType::CONTROL_MASK));
    wait(|| delivered.get() && p.pending.get() == 0 && !p.saving.get());
    assert_eq!(p.frame.borrow()["document"], expected.borrow().as_ref().unwrap().clone());
    assert!(!p.dirty.get());
    let saved = rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap();
    let marker = rpc(&a.client, "sync_cursor", json!({})).unwrap();
    let before = p.frame.borrow()["document"].clone();
    let position = p.frame.borrow()["position"].clone();
    let im = p.test_input_method.borrow().as_ref().unwrap().clone();

    // F12 is unassigned and traverses the installed ordinary input scheduling
    // path without adding text. It supplies the original synthetic IM permit.
    key(&p, gdk::keys::constants::F12, gdk::ModifierType::empty());
    let executor = HostExecutor { binding: binding.clone() };
    executor.block_input().unwrap();
    gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).set_text("Clipboard remains intact 中文");
    assert!(key(&p, gdk::keys::constants::x, gdk::ModifierType::CONTROL_MASK));
    assert!(key(&p, gdk::keys::constants::a, gdk::ModifierType::empty()));
    pointer(&p, true);
    pointer(&p, false);
    im.emit_by_name::<()>("commit", &[&"Blocked IM مِ 中文"]);
    assert_eq!(clipboard_text(), "Clipboard remains intact 中文");
    assert_eq!(p.frame.borrow()["document"], before);
    assert_eq!(p.frame.borrow()["position"], position);
    assert_eq!(p.pending.get(), 0);
    executor.unblock_input();
    im.emit_by_name::<()>("commit", &[&"Late stale IM مِ 中文"]);
    assert_eq!(p.frame.borrow()["document"], before);
    assert_eq!(p.pending.get(), 0);
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);

    // A freshly scheduled word can settle during ordinary manual draft drain;
    // this uses the real installed IM signals and commit_ordinary mapping path.
    key(&p, gdk::keys::constants::F12, gdk::ModifierType::empty());
    im.emit_by_name::<()>("preedit-start", &[]);
    assert!(p.composing.get());
    a.desk.busy.set(true);
    im.emit_by_name::<()>("preedit-end", &[]);
    let word = "Ordinary مِ 中文 𠀀 👩🏽‍🚀 e\u{301}";
    im.emit_by_name::<()>("commit", &[&word]);
    wait(|| p.pending.get() == 0);
    a.desk.busy.set(false);
    assert!(novel_storage::plain_text(&p.frame.borrow()["document"]).contains(word));
    assert!(p.dirty.get());
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    request(&p, json!({"type":"command","command":"undo"}), true).unwrap();
    assert_eq!(p.frame.borrow()["document"], before);
    discard_scene(&p);

    // Normal literal clipboard input stays manual and undoable after the gate.
    let literal = "Literal مِ 中文 𠀀 👩🏽‍🚀 e\u{301}";
    gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).set_text(literal);
    request(&p, json!({"type":"command","command":"selectAll"}), false).unwrap();
    assert!(key(&p, gdk::keys::constants::v, gdk::ModifierType::CONTROL_MASK));
    wait(|| p.pending.get() == 0 && p.dirty.get());
    assert_eq!(novel_storage::plain_text(&p.frame.borrow()["document"]), literal);
    request(&p, json!({"type":"command","command":"undo"}), true).unwrap();
    assert_eq!(p.frame.borrow()["document"], before);
    assert_eq!(rpc(&a.client, "load_scene", json!({"sceneId":sid})).unwrap(), saved);
    assert_eq!(rpc(&a.client, "sync_cursor", json!({})).unwrap(), marker);
    discard_scene(&p);
    a.close();
    wait(|| a.desk.is_closed());
}
