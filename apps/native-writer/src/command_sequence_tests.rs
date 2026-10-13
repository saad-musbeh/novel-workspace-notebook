use super::*;
use novel_editor::model::TextModel;
use novel_storage::native_sequences::{
    SequenceDirection, SequenceFormat, SequenceLanguage, SequenceStyle,
};

fn bold() -> NativeSequenceAction {
    NativeSequenceAction::Format {
        command: SequenceFormat::Bold,
    }
}
fn target() -> SequenceTarget {
    SequenceTarget {
        project_id: "project".into(),
        scene_id: "scene".into(),
        editor_id: "pane-instance".into(),
    }
}
fn runtime(actions: Vec<NativeSequenceAction>) -> Rc<SequenceRuntime> {
    let runtime = SequenceRuntime::new("project".into());
    let settings = NativeSequenceSettings {
        schema_version: 1,
        sequences: if actions.is_empty() {
            vec![]
        } else {
            vec![NativeCommandSequence {
                slot: 1,
                name: "مِ 中文".into(),
                actions,
            }]
        },
    };
    runtime
        .load(SettingsSnapshot {
            parent: Some("head0".into()),
            payload: settings
                .merge_into(&json!({"schemaVersion":1,"unrelated":"preserved"}))
                .unwrap(),
        })
        .unwrap();
    runtime
}
struct Store {
    requests: RefCell<Vec<SettingsSnapshot>>,
    reply: RefCell<Option<SettingsCompletion>>,
}
impl Store {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            requests: RefCell::new(vec![]),
            reply: RefCell::new(None),
        })
    }
    fn finish(&self, error: Option<&str>) {
        let done = self.reply.borrow_mut().take().unwrap();
        let requested = self.requests.borrow().last().unwrap().clone();
        done(match error {
            Some(e) => Err(e.into()),
            None => Ok(SettingsSnapshot {
                parent: Some("head1".into()),
                payload: requested.payload,
            }),
        });
    }
}
impl SequenceStore for Store {
    fn save_checked(&self, request: SettingsSnapshot, done: SettingsCompletion) {
        self.requests.borrow_mut().push(request);
        *self.reply.borrow_mut() = Some(done);
    }
}
struct Executor {
    events: RefCell<Vec<String>>,
    model: RefCell<TextModel>,
    saved: RefCell<Vec<Value>>,
    gate: SequenceInputGate,
    permit: Cell<Option<PlaybackInputPermit>>,
    compose: Cell<bool>,
    closed: Cell<bool>,
    fail_drain: Cell<bool>,
    fail_execute: Cell<usize>,
    fail_save: Cell<usize>,
    defer_execute: Cell<bool>,
    defer_save: Cell<bool>,
    save_reply: RefCell<Option<Completion>>,
    execute_reply: RefCell<Option<Completion>>,
    composing_after_execute: Cell<bool>,
    closed_after_save: Cell<bool>,
    commands: Cell<usize>,
    saves: Cell<usize>,
    releases: Cell<usize>,
}
impl Executor {
    fn new() -> Rc<Self> {
        let mut model = TextModel::new(json!({"type":"doc","content":[{"type":"paragraph","attrs":{"lang":"ar","dir":"rtl"},"content":[{"type":"text","text":"مِ 中文 𠀀 👩🏽‍🚀"}]}]})).unwrap();
        model
            .edit(&json!({"type":"command","command":"selectAll"}))
            .unwrap();
        Rc::new(Self {
            events: RefCell::new(vec![]),
            model: RefCell::new(model),
            saved: RefCell::new(vec![]),
            gate: Default::default(),
            permit: Cell::new(None),
            compose: Cell::new(false),
            closed: Cell::new(false),
            fail_drain: Cell::new(false),
            fail_execute: Cell::new(0),
            fail_save: Cell::new(0),
            defer_execute: Cell::new(false),
            defer_save: Cell::new(false),
            save_reply: RefCell::new(None),
            execute_reply: RefCell::new(None),
            composing_after_execute: Cell::new(false),
            closed_after_save: Cell::new(false),
            commands: Cell::new(0),
            saves: Cell::new(0),
            releases: Cell::new(0),
        })
    }
}
impl SequenceExecutor for Executor {
    fn check_context(&self) -> Result<(), String> {
        if self.compose.get() {
            return Err("composition".into());
        }
        if self.closed.get() {
            return Err("pane closed or project changed".into());
        }
        if let Some(permit) = self.permit.get() {
            if !self.gate.allows_playback(permit) {
                return Err("input gate lost".into());
            }
        }
        Ok(())
    }
    fn block_input(&self) -> Result<(), String> {
        let permit = self.gate.acquire()?;
        self.permit.set(Some(permit));
        self.events.borrow_mut().push("block".into());
        Ok(())
    }
    fn drain_manual_drafts(&self, done: Completion) {
        self.events.borrow_mut().push("manual choice".into());
        done(if self.fail_drain.get() {
            Err("Cancel or failed manual save".into())
        } else {
            Ok(())
        });
    }
    fn execute(&self, action: NativeSequenceAction, done: Completion) {
        assert!(self.gate.user_permit().is_none());
        let step = self.commands.get() + 1;
        self.commands.set(step);
        self.events.borrow_mut().push(format!("execute {step}"));
        if self.fail_execute.get() == step {
            done(Err("editor failure".into()));
            return;
        }
        self.model
            .borrow_mut()
            .edit(&operation_from_action(&action).unwrap())
            .unwrap();
        if self.composing_after_execute.get() {
            self.compose.set(true);
        }
        if self.defer_execute.get() {
            *self.execute_reply.borrow_mut() = Some(done);
        } else {
            done(Ok(()));
        }
    }
    fn save_step(&self, done: Completion) {
        assert!(self.gate.user_permit().is_none());
        let step = self.saves.get() + 1;
        self.saves.set(step);
        self.events.borrow_mut().push(format!("save {step}"));
        if self.fail_save.get() == step {
            done(Err("CAS Conflict".into()));
            return;
        }
        self.saved.borrow_mut().push(self.model.borrow().document());
        if self.closed_after_save.get() {
            self.closed.set(true);
        }
        if self.defer_save.get() {
            *self.save_reply.borrow_mut() = Some(done);
        } else {
            done(Ok(()));
        }
    }
    fn unblock_input(&self) {
        assert!(self.gate.release(self.permit.take().unwrap()));
        self.releases.set(self.releases.get() + 1);
        self.events.borrow_mut().push("unblock".into());
    }
}
fn play(
    runtime: &Rc<SequenceRuntime>,
    executor: Rc<Executor>,
) -> Rc<RefCell<Option<PlaybackOutcome>>> {
    let outcome = Rc::new(RefCell::new(None));
    let out = outcome.clone();
    runtime
        .play(
            1,
            executor,
            Box::new(move |result| *out.borrow_mut() = Some(result)),
        )
        .unwrap();
    outcome
}
fn actions() -> Vec<NativeSequenceAction> {
    vec![
        bold(),
        NativeSequenceAction::Spacing { value: 1.5 },
        NativeSequenceAction::Format {
            command: SequenceFormat::Italic,
        },
    ]
}

#[test]
fn operation_whitelist_roundtrips_without_text_or_position() {
    let mut allowed = actions();
    allowed.extend([
        NativeSequenceAction::Paragraph {
            style: SequenceStyle::Heading,
            language: SequenceLanguage::ChineseTraditional,
            direction: SequenceDirection::Auto,
        },
        NativeSequenceAction::Format {
            command: SequenceFormat::ClearFormatting,
        },
        NativeSequenceAction::Format {
            command: SequenceFormat::Ltr,
        },
        NativeSequenceAction::Format {
            command: SequenceFormat::Rtl,
        },
    ]);
    for action in allowed {
        assert_eq!(
            action_from_operation(&operation_from_action(&action).unwrap()),
            Some(action)
        );
    }
    for op in [
        json!({"type":"text","text":"private"}),
        json!({"type":"key","key":"Delete"}),
        json!({"type":"mouse","phase":"down","x":99}),
        json!({"type":"command","command":"undo"}),
        json!({"type":"command","command":"paste"}),
        json!({"type":"command","command":"bold","text":"private"}),
        json!({"type":"command","command":"bold","offset":1}),
        json!({"type":"command","command":"lineSpacing","lineSpacing":1.2}),
        json!({"type":"command","command":"blockStyle","style":"heading","language":"xx","direction":"auto"}),
    ] {
        assert!(action_from_operation(&op).is_none(), "{op}");
    }
}
#[test]
fn record_only_acknowledged_commands_for_bound_pane_and_generation() {
    let r = runtime(vec![]);
    r.start_recording(target(), 8, "Formatting مِ".into())
        .unwrap();
    let op = operation_from_action(&bold()).unwrap();
    let (token, action) = r.recording_command(&target(), &op).unwrap();
    // An unacknowledged/failed request contributes nothing.
    assert!(r.recording().unwrap().sequence.actions.is_empty());
    let mut other = target();
    other.editor_id = "same-scene-second-window".into();
    assert!(r.recording_command(&other, &op).is_none());
    assert!(r
        .recording_command(&target(), &json!({"type":"text","text":"private"}))
        .is_none());
    assert!(r.record_accepted(token.clone(), action.clone()));
    r.cancel_recording().unwrap();
    r.start_recording(target(), 1, "New".into()).unwrap();
    assert!(!r.record_accepted(token, action));
    assert!(r.recording().unwrap().sequence.actions.is_empty());
}
#[test]
fn recording_limit_cancel_and_notifications_never_save_manuscript() {
    let r = runtime(vec![]);
    let notifications = Rc::new(Cell::new(0));
    let count = notifications.clone();
    let weak = Rc::downgrade(&r);
    *r.on_change.borrow_mut() = Some(Box::new(move || {
        let r = weak.upgrade().unwrap();
        r.recording();
        count.set(count.get() + 1);
    }));
    r.start_recording(target(), 1, "Format".into()).unwrap();
    for _ in 0..32 {
        let (token, action) = r
            .recording_command(&target(), &operation_from_action(&bold()).unwrap())
            .unwrap();
        assert!(r.record_accepted(token, action));
    }
    let (token, action) = r
        .recording_command(&target(), &operation_from_action(&bold()).unwrap())
        .unwrap();
    assert!(!r.record_accepted(token, action));
    assert_eq!(r.recording().unwrap().sequence.actions.len(), 32);
    assert!(r.settings().sequences.is_empty());
    r.cancel_recording().unwrap();
    assert!(r.recording().is_none());
    assert!(r.settings().sequences.is_empty());
    assert_eq!(notifications.get(), 34);
}
#[test]
fn explicit_sequence_save_cas_retains_recording_on_failure() {
    let r = runtime(vec![]);
    let store = Store::new();
    r.start_recording(target(), 1, "مِ 中文".into()).unwrap();
    let (token, action) = r
        .recording_command(&target(), &operation_from_action(&bold()).unwrap())
        .unwrap();
    r.record_accepted(token.clone(), action.clone());
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    r.save_recording(
        store.clone(),
        Box::new(move |r| *out.borrow_mut() = Some(r)),
    );
    assert!(r.saving());
    assert!(!r.record_accepted(token, action));
    assert!(r.cancel_recording().is_err());
    let request = store.requests.borrow()[0].clone();
    assert_eq!(request.parent.as_deref(), Some("head0"));
    assert_eq!(request.payload["unrelated"], "preserved");
    store.finish(Some("Conflict"));
    assert!(result.borrow().as_ref().unwrap().is_err());
    assert!(r.recording().is_some());
    assert!(r.settings().sequences.is_empty());
    r.save_recording(store.clone(), Box::new(|r| r.unwrap()));
    assert_eq!(store.requests.borrow()[1].parent, request.parent);
    store.finish(None);
    assert!(r.recording().is_none());
    assert_eq!(r.settings().sequences[0].actions, vec![bold()]);
    r.remove(1, store.clone(), Box::new(|r| r.unwrap()));
    assert_eq!(store.requests.borrow()[2].parent.as_deref(), Some("head1"));
    store.finish(None);
    assert!(r.settings().sequences.is_empty());
}
#[test]
fn empty_recording_cannot_save_and_invalid_load_cannot_erase_preferences() {
    let r = runtime(vec![]);
    r.start_recording(target(), 1, "Name".into()).unwrap();
    let store = Store::new();
    r.save_recording(store.clone(), Box::new(|r| assert!(r.is_err())));
    assert!(store.requests.borrow().is_empty());
    r.cancel_recording().unwrap();
    let settings = r.settings();
    assert!(r
        .load(SettingsSnapshot {
            parent: None,
            payload: json!({"nativeCommandSequences":false})
        })
        .is_err());
    assert_eq!(r.settings(), settings);
    assert!(SettingsSnapshot::from_rpc(&json!({"payload":{}})).is_err());
    assert!(SettingsSnapshot::from_rpc(&Value::Null)
        .unwrap()
        .parent
        .is_none());
}
#[test]
fn playback_alternates_acknowledgement_and_save_preserving_unicode_and_undo() {
    let r = runtime(actions());
    let e = Executor::new();
    let before = e.model.borrow().document();
    let text = e.model.borrow().flat_text();
    let outcome = play(&r, e.clone());
    assert_eq!(
        *outcome.borrow(),
        Some(PlaybackOutcome {
            accepted_steps: 3,
            saved_steps: 3,
            error: None
        })
    );
    assert_eq!(
        *e.events.borrow(),
        [
            "block",
            "manual choice",
            "execute 1",
            "save 1",
            "execute 2",
            "save 2",
            "execute 3",
            "save 3",
            "unblock"
        ]
    );
    assert_eq!(e.model.borrow().flat_text(), text);
    assert_eq!(e.saved.borrow().len(), 3);
    assert!(r.playback().is_none());
    assert_eq!(e.releases.get(), 1);
    for _ in 0..3 {
        assert!(e.model.borrow_mut().history(false));
    }
    assert_eq!(e.model.borrow().document(), before);
    assert!(e.gate.user_permit().is_some());
}
#[test]
fn no_next_command_or_save_before_editor_acknowledgement() {
    let r = runtime(actions());
    let e = Executor::new();
    e.defer_execute.set(true);
    let outcome = play(&r, e.clone());
    assert!(outcome.borrow().is_none());
    assert_eq!(e.commands.get(), 1);
    assert_eq!(e.saves.get(), 0);
    assert!(e.gate.user_permit().is_none());
    assert!(r.start_recording(target(), 1, "New".into()).is_err());
    assert!(r.play(1, e.clone(), Box::new(|_| {})).is_err());
    let done = e.execute_reply.borrow_mut().take().unwrap();
    e.defer_execute.set(false);
    done(Ok(()));
    assert_eq!(outcome.borrow().as_ref().unwrap().saved_steps, 3);
}
#[test]
fn failed_editor_step_stops_remaining_commands_and_preserves_saved_undo() {
    let r = runtime(actions());
    let e = Executor::new();
    e.fail_execute.set(2);
    let outcome = play(&r, e.clone());
    let outcome = outcome.borrow().clone().unwrap();
    assert_eq!((outcome.accepted_steps, outcome.saved_steps), (1, 1));
    assert_eq!(e.commands.get(), 2);
    assert_eq!(e.saves.get(), 1);
    assert!(outcome.error.is_some());
    assert!(e.model.borrow_mut().history(false));
    assert_eq!(e.releases.get(), 1);
}
#[test]
fn failed_step_save_keeps_accepted_unsaved_draft_and_earlier_saved_step_undoable() {
    let r = runtime(actions());
    let e = Executor::new();
    let original = e.model.borrow().document();
    e.fail_save.set(2);
    let outcome = play(&r, e.clone());
    let outcome = outcome.borrow().clone().unwrap();
    assert_eq!((outcome.accepted_steps, outcome.saved_steps), (2, 1));
    assert_eq!(e.commands.get(), 2);
    assert_eq!(e.saved.borrow().len(), 1);
    assert_ne!(e.model.borrow().document(), e.saved.borrow()[0]);
    assert!(outcome.error.unwrap().contains("Conflict"));
    assert!(e.model.borrow_mut().history(false));
    assert_eq!(e.model.borrow().document(), e.saved.borrow()[0]);
    assert!(e.model.borrow_mut().history(false));
    assert_eq!(e.model.borrow().document(), original);
    assert_eq!(e.releases.get(), 1);
}
#[test]
fn manual_choice_cancel_or_save_failure_runs_no_commands() {
    let r = runtime(actions());
    let e = Executor::new();
    e.fail_drain.set(true);
    let original = e.model.borrow().document();
    let outcome = play(&r, e.clone());
    assert_eq!(outcome.borrow().as_ref().unwrap().saved_steps, 0);
    assert_eq!(e.commands.get(), 0);
    assert_eq!(e.saves.get(), 0);
    assert_eq!(e.model.borrow().document(), original);
    assert_eq!(e.releases.get(), 1);
}
#[test]
fn composition_before_play_refuses_input_acquisition_and_drain() {
    let r = runtime(actions());
    let e = Executor::new();
    e.compose.set(true);
    assert!(r
        .play(1, e.clone(), Box::new(|_| panic!("not started")))
        .is_err());
    assert!(e.events.borrow().is_empty());
    assert!(r.playback().is_none());
}
#[test]
fn composition_after_editor_ack_stops_before_save_and_retains_draft() {
    let r = runtime(actions());
    let e = Executor::new();
    e.composing_after_execute.set(true);
    let original = e.model.borrow().document();
    let outcome = play(&r, e.clone());
    assert_eq!(
        (
            outcome.borrow().as_ref().unwrap().accepted_steps,
            outcome.borrow().as_ref().unwrap().saved_steps
        ),
        (1, 0)
    );
    assert_eq!(e.saves.get(), 0);
    assert_ne!(e.model.borrow().document(), original);
    assert!(e.model.borrow_mut().history(false));
    assert_eq!(e.model.borrow().document(), original);
}
#[test]
fn ownership_change_after_save_stops_before_remaining_commands() {
    let r = runtime(actions());
    let e = Executor::new();
    e.closed_after_save.set(true);
    let outcome = play(&r, e.clone());
    assert_eq!(outcome.borrow().as_ref().unwrap().saved_steps, 1);
    assert_eq!(e.commands.get(), 1);
    assert_eq!(e.releases.get(), 1);
}
#[test]
fn input_gate_invalidates_queued_paste_and_does_not_release_another_owner() {
    let gate = SequenceInputGate::default();
    let queued = gate.user_permit().unwrap();
    let first = gate.acquire().unwrap();
    assert!(!gate.allows_user(queued));
    assert!(gate.user_permit().is_none());
    assert!(gate.acquire().is_err());
    assert!(gate.allows_playback(first));
    assert!(gate.release(first));
    assert!(!gate.allows_user(queued));
    let current = gate.user_permit().unwrap();
    assert!(gate.allows_user(current));
    let second = gate.acquire().unwrap();
    assert!(!gate.release(first));
    assert!(gate.allows_playback(second));
    assert!(gate.release(second));
}
#[test]
fn existing_input_lock_is_not_released_on_refused_play() {
    let r = runtime(actions());
    let e = Executor::new();
    let held = e.gate.acquire().unwrap();
    assert!(r
        .play(1, e.clone(), Box::new(|_| panic!("not started")))
        .is_err());
    assert_eq!(e.releases.get(), 0);
    assert!(e.gate.allows_playback(held));
}

#[test]
fn input_permits_are_bound_to_their_pane_gate() {
    let one = SequenceInputGate::default();
    let two = SequenceInputGate::default();
    assert!(!two.allows_user(one.user_permit().unwrap()));
    let first = one.acquire().unwrap();
    let second = two.acquire().unwrap();
    assert!(!two.allows_playback(first));
    assert!(!two.release(first));
    assert!(two.allows_playback(second));
    assert!(one.release(first));
    assert!(two.release(second));
}
#[test]
fn cancelling_capture_preserves_the_real_editor_edit_and_undo() {
    let r = runtime(vec![]);
    let e = Executor::new();
    let before = e.model.borrow().document();
    r.start_recording(target(), 1, "Format".into()).unwrap();
    let op = operation_from_action(&bold()).unwrap();
    let (token, action) = r.recording_command(&target(), &op).unwrap();
    e.model.borrow_mut().edit(&op).unwrap();
    r.record_accepted(token, action);
    let edited = e.model.borrow().document();
    r.cancel_recording().unwrap();
    assert_eq!(e.model.borrow().document(), edited);
    assert_ne!(edited, before);
    assert_eq!(e.saves.get(), 0);
    assert!(e.model.borrow_mut().history(false));
    assert_eq!(e.model.borrow().document(), before);
}

#[test]
fn next_command_waits_for_storage_acknowledgement_with_input_still_paused() {
    let r = runtime(actions());
    let e = Executor::new();
    e.defer_save.set(true);
    let outcome = play(&r, e.clone());
    assert!(outcome.borrow().is_none());
    assert_eq!(e.commands.get(), 1);
    assert_eq!(e.saves.get(), 1);
    let progress = r.playback().unwrap();
    assert_eq!((progress.accepted_steps, progress.saved_steps), (1, 0));
    assert!(e.gate.user_permit().is_none());
    let done = e.save_reply.borrow_mut().take().unwrap();
    e.defer_save.set(false);
    done(Ok(()));
    assert_eq!(outcome.borrow().as_ref().unwrap().saved_steps, 3);
    assert_eq!(e.releases.get(), 1);
}
#[test]
fn mismatched_settings_ack_keeps_recording_and_requires_explicit_reload() {
    let r = runtime(vec![]);
    let store = Store::new();
    r.start_recording(target(), 1, "Name".into()).unwrap();
    let (token, action) = r
        .recording_command(&target(), &operation_from_action(&bold()).unwrap())
        .unwrap();
    r.record_accepted(token, action);
    r.save_recording(store.clone(), Box::new(|r| assert!(r.is_err())));
    let done = store.reply.borrow_mut().take().unwrap();
    done(Ok(SettingsSnapshot {
        parent: Some("other-head".into()),
        payload: json!({"schemaVersion":1}),
    }));
    assert!(r.recording().is_some());
    assert!(r.settings().sequences.is_empty());
    assert!(!r.saving());
}

/// CALLABLE ONLY by root's already initialized, serialized opt-in GTK workflow.
/// This is intentionally not #[test]: cargo test must never open a desktop.
pub(crate) fn gtk_sequence_controls() {
    assert!(
        gtk::is_initialized_main_thread(),
        "Parent initializes GTK and owns the desktop gate"
    );
    let r = runtime(actions());
    let played = Rc::new(Cell::new(0));
    let recorded = Rc::new(RefCell::new(None));
    let play_count = played.clone();
    let record_out = recorded.clone();
    let actions = Rc::new(SequenceUiActions {
        record: Rc::new(move |slot, name| *record_out.borrow_mut() = Some((slot, name))),
        play: Rc::new(move |slot| {
            assert_eq!(slot, 1);
            play_count.set(play_count.get() + 1);
        }),
        remove: Rc::new(|_| {}),
        save_recording: Rc::new(|| {}),
        cancel_recording: Rc::new(|| {}),
    });
    let page = CommandSequencesPage::new(r.clone(), actions.clone());
    let banner = RecordingBanner::new(r.clone(), actions);
    assert_eq!(page.names.len(), 8);
    assert_eq!(page.names[0].text(), "مِ 中文");
    assert!(page.rows[0].1.is_sensitive());
    assert!(!page.rows[1].1.is_sensitive());
    page.rows[0].1.emit_clicked();
    assert_eq!(played.get(), 1);
    page.names[7].set_text("مِ 中文 new");
    page.rows[7].0.emit_clicked();
    assert_eq!(*recorded.borrow(), Some((8, "مِ 中文 new".into())));
    r.start_recording(target(), 8, "مِ 中文 new".into()).unwrap();
    page.refresh();
    banner.refresh();
    assert!(!page.rows[0].1.is_sensitive());
    assert!(banner.widget.is_visible());
    assert!(!banner.save.is_sensitive());
    let (token, action) = r
        .recording_command(&target(), &operation_from_action(&bold()).unwrap())
        .unwrap();
    r.record_accepted(token, action);
    banner.refresh();
    assert!(banner.save.is_sensitive());
    r.cancel_recording().unwrap();
    banner.refresh();
    page.refresh();
    assert!(!banner.widget.is_visible());
    assert!(page.rows[0].1.is_sensitive());
}
