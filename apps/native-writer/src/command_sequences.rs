//! Explicit native formatting recording/playback; no timers or manuscript autosave.
//!
//! Root integration contract:
//! * Share one runtime per project session; bind recording to a unique pane instance.
//! * Capture `recording_command` BEFORE enqueueing an explicit editor request, then
//!   call `record_accepted` only on its successful acknowledgement. Never hook text.
//! * Implement SequenceExecutor against the captured pane, not a later active pane.
//!   Acquire a logical input/command gate BEFORE changing GTK sensitivity. Check IM
//!   composition first: focus-out can commit text. Gate key/IM/paste/drop/mouse and
//!   toolbar/navigation/close/detach paths, including already queued callbacks.
//! * drain_manual_drafts waits for pending operations and uses Save/Discard/Cancel;
//!   Cancel is an error. Existing confirmations can restore window sensitivity, so
//!   reassert the visual gate afterwards while keeping the logical gate held.
//! * execute uses EditorPane::request(..., true, ...); save_step uses save_pane and
//!   verifies the acknowledged save left no pending/dirty work. Neither reopens the
//!   editor nor clears Undo. A failed step's draft stays available for manual Save.
//! * SequenceStore uses save_project_data_checked(kind=settings, dependencies=[])
//!   with the captured payload/head. Never retry against a newer head silently.
use gtk::prelude::*;
use novel_storage::native_sequences::{
    NativeCommandSequence, NativeSequenceAction, NativeSequenceSettings, MAX_SEQUENCE_COMMANDS,
};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

pub type Completion = Box<dyn FnOnce(Result<(), String>)>;
pub type SettingsCompletion = Box<dyn FnOnce(Result<SettingsSnapshot, String>)>;

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsSnapshot {
    pub payload: Value,
    pub parent: Option<String>,
}
impl SettingsSnapshot {
    pub fn from_rpc(saved: &Value) -> Result<Self, String> {
        if saved.is_null() {
            return Ok(Self {
                payload: json!({"schemaVersion":1}),
                parent: None,
            });
        }
        let parent = saved["revisionId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Missing settings revision.")?
            .to_owned();
        if !saved["payload"].is_object() {
            return Err("Missing project settings payload.".into());
        }
        NativeSequenceSettings::from_project_settings(&saved["payload"])?;
        Ok(Self {
            payload: saved["payload"].clone(),
            parent: Some(parent),
        })
    }
}
/// The store is permanently bound to this runtime's project, including after detach.
pub trait SequenceStore {
    fn save_checked(&self, snapshot: SettingsSnapshot, done: SettingsCompletion);
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequenceTarget {
    pub project_id: String,
    pub scene_id: String,
    /// A fresh identity for each EditorPane, even two panes of the same scene.
    pub editor_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingToken {
    generation: u64,
    target: SequenceTarget,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    pub sequence: NativeCommandSequence,
    pub target: SequenceTarget,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackProgress {
    pub slot: u8,
    pub total: usize,
    pub accepted_steps: usize,
    pub saved_steps: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackOutcome {
    pub accepted_steps: usize,
    pub saved_steps: usize,
    pub error: Option<String>,
}
/// All callbacks are on the GTK thread and must complete exactly once, including
/// window closure and worker failure. check_context rejects composition, a changed
/// project/pane/session or lost input gate; it must never redirect to another scene.
pub trait SequenceExecutor {
    fn check_context(&self) -> Result<(), String>;
    /// Atomic acquisition. On Err, leave any pre-existing gate untouched.
    fn block_input(&self) -> Result<(), String>;
    fn drain_manual_drafts(&self, done: Completion);
    /// Successful completion means the editor acknowledged this one command.
    fn execute(&self, action: NativeSequenceAction, done: Completion);
    /// Successful completion means this step has been acknowledged by storage.
    fn save_step(&self, done: Completion);
    /// Release only the gate acquired by this executor; called once after acquisition.
    fn unblock_input(&self);
}

/// Root puts one gate on each EditorPane. Capture a user permit at scheduling
/// time for clipboard/IME/drag callbacks, then check it at delivery time. Merely
/// making GTK insensitive does not stop those callbacks. The explicit executor
/// carries its playback permit through request/snapshot/save continuations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserInputPermit {
    gate_id: u64,
    epoch: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaybackInputPermit {
    gate_id: u64,
    epoch: u64,
}
pub struct SequenceInputGate {
    id: u64,
    epoch: Cell<u64>,
    held: Cell<bool>,
}
impl Default for SequenceInputGate {
    fn default() -> Self {
        static NEXT_GATE: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_GATE.fetch_add(1, Ordering::Relaxed),
            epoch: Cell::new(0),
            held: Cell::new(false),
        }
    }
}
impl SequenceInputGate {
    pub fn user_permit(&self) -> Option<UserInputPermit> {
        (!self.held.get()).then(|| UserInputPermit {
            gate_id: self.id,
            epoch: self.epoch.get(),
        })
    }
    pub fn allows_user(&self, permit: UserInputPermit) -> bool {
        !self.held.get() && permit.gate_id == self.id && permit.epoch == self.epoch.get()
    }
    pub fn acquire(&self) -> Result<PlaybackInputPermit, String> {
        if self.held.replace(true) {
            return Err("Editor input is already paused.".into());
        }
        self.epoch.set(self.epoch.get() + 1);
        Ok(PlaybackInputPermit {
            gate_id: self.id,
            epoch: self.epoch.get(),
        })
    }
    pub fn allows_playback(&self, permit: PlaybackInputPermit) -> bool {
        self.held.get() && permit.gate_id == self.id && permit.epoch == self.epoch.get()
    }
    pub fn release(&self, permit: PlaybackInputPermit) -> bool {
        if !self.allows_playback(permit) {
            return false;
        }
        self.epoch.set(self.epoch.get() + 1);
        self.held.set(false);
        true
    }
}

pub struct SequenceRuntime {
    pub project_id: String,
    snapshot: RefCell<Option<SettingsSnapshot>>,
    settings: RefCell<NativeSequenceSettings>,
    recording: RefCell<Option<Recording>>,
    playback: RefCell<Option<PlaybackProgress>>,
    generation: Cell<u64>,
    settings_saving: Cell<bool>,
    pub message: RefCell<String>,
    /// Root refreshes all mounted pages/banners with a weak-owner callback.
    pub on_change: RefCell<Option<Box<dyn Fn()>>>,
}
impl SequenceRuntime {
    pub fn new(project_id: String) -> Rc<Self> {
        Rc::new(Self {
            project_id,
            snapshot: RefCell::new(None),
            settings: RefCell::new(Default::default()),
            recording: RefCell::new(None),
            playback: RefCell::new(None),
            generation: Cell::new(0),
            settings_saving: Cell::new(false),
            message: RefCell::new(String::new()),
            on_change: RefCell::new(None),
        })
    }
    fn notify(&self, message: impl Into<String>) {
        *self.message.borrow_mut() = message.into();
        if let Some(changed) = self.on_change.borrow().as_ref() {
            changed();
        }
    }
    /// Root action callbacks can surface refused recording/playback before it starts.
    pub fn report_error(&self, error: &str) {
        self.notify(error);
    }
    pub fn loaded(&self) -> bool {
        self.snapshot.borrow().is_some()
    }
    pub fn saving(&self) -> bool {
        self.settings_saving.get()
    }
    pub fn recording(&self) -> Option<Recording> {
        self.recording.borrow().clone()
    }
    pub fn playback(&self) -> Option<PlaybackProgress> {
        self.playback.borrow().clone()
    }
    pub fn settings(&self) -> NativeSequenceSettings {
        self.settings.borrow().clone()
    }
    pub fn busy(&self) -> bool {
        self.saving() || self.recording.borrow().is_some() || self.playback.borrow().is_some()
    }
    /// Root must generation-guard asynchronous load replies before calling this.
    pub fn load(&self, snapshot: SettingsSnapshot) -> Result<(), String> {
        if self.busy() {
            return Err("Finish recording, playback or saving before reloading sequences.".into());
        }
        if !snapshot.payload.is_object() {
            return Err("Project settings must be an object.".into());
        }
        let settings = NativeSequenceSettings::from_project_settings(&snapshot.payload)?;
        *self.snapshot.borrow_mut() = Some(snapshot);
        *self.settings.borrow_mut() = settings;
        self.notify("Sequences loaded locally. Recording leaves manuscript Save manual.");
        Ok(())
    }
    pub fn start_recording(
        &self,
        target: SequenceTarget,
        slot: u8,
        name: String,
    ) -> Result<(), String> {
        if !self.loaded() || self.busy() {
            return Err("Load sequences and finish the current operation first.".into());
        }
        NativeCommandSequence::validate_name(slot, &name)?;
        if target.project_id != self.project_id
            || target.scene_id.is_empty()
            || target.editor_id.is_empty()
        {
            return Err("Record in an open draft of this novel.".into());
        }
        self.generation.set(self.generation.get() + 1);
        *self.recording.borrow_mut() = Some(Recording {
            sequence: NativeCommandSequence {
                slot,
                name,
                actions: vec![],
            },
            target,
        });
        self.notify("Recording formatting commands · Manuscript Save remains manual");
        Ok(())
    }
    pub fn recording_command(
        &self,
        target: &SequenceTarget,
        operation: &Value,
    ) -> Option<(RecordingToken, NativeSequenceAction)> {
        if self.saving() || self.playback.borrow().is_some() {
            return None;
        }
        let recording = self.recording.borrow();
        let recording = recording.as_ref().filter(|r| &r.target == target)?;
        Some((
            RecordingToken {
                generation: self.generation.get(),
                target: recording.target.clone(),
            },
            action_from_operation(operation)?,
        ))
    }
    pub fn record_accepted(&self, token: RecordingToken, action: NativeSequenceAction) -> bool {
        if self.saving()
            || self.playback.borrow().is_some()
            || token.generation != self.generation.get()
            || action.validate().is_err()
        {
            return false;
        }
        let count = {
            let mut recording = self.recording.borrow_mut();
            let Some(recording) = recording.as_mut().filter(|r| r.target == token.target) else {
                return false;
            };
            if recording.sequence.actions.len() == MAX_SEQUENCE_COMMANDS {
                return false;
            }
            recording.sequence.actions.push(action);
            recording.sequence.actions.len()
        };
        self.notify(format!(
            "Recording formatting commands · {count}/32 · Manuscript Save remains manual"
        ));
        true
    }

    pub fn cancel_recording(&self) -> Result<(), String> {
        if self.saving() {
            return Err("Wait for the sequence settings save to finish.".into());
        }
        self.recording.borrow_mut().take();
        self.generation.set(self.generation.get() + 1);
        self.notify("Recording cancelled · Edits already made remain in the draft");
        Ok(())
    }
    pub fn save_recording(self: &Rc<Self>, store: Rc<dyn SequenceStore>, done: Completion) {
        if self.saving() || self.playback.borrow().is_some() {
            done(Err("A sequence operation is still running.".into()));
            return;
        }
        let Some(recording) = self.recording() else {
            done(Err("No sequence is being recorded.".into()));
            return;
        };
        if let Err(error) = recording.sequence.validate() {
            done(Err(error));
            return;
        }
        let mut settings = self.settings();
        settings
            .sequences
            .retain(|s| s.slot != recording.sequence.slot);
        settings.sequences.push(recording.sequence);
        settings.sequences.sort_by_key(|s| s.slot);
        self.persist(settings, store, true, done);
    }
    pub fn remove(self: &Rc<Self>, slot: u8, store: Rc<dyn SequenceStore>, done: Completion) {
        if self.busy() {
            done(Err("Finish the current sequence operation first.".into()));
            return;
        }
        if self.settings.borrow().sequence(slot).is_none() {
            done(Err("No sequence is saved in this slot.".into()));
            return;
        }
        let mut settings = self.settings();
        settings.sequences.retain(|s| s.slot != slot);
        self.persist(settings, store, false, done);
    }
    fn persist(
        self: &Rc<Self>,
        settings: NativeSequenceSettings,
        store: Rc<dyn SequenceStore>,
        finish_recording: bool,
        done: Completion,
    ) {
        let request = self
            .snapshot
            .borrow()
            .clone()
            .ok_or_else(|| "Sequences are still loading.".to_owned())
            .and_then(|snapshot| {
                Ok(SettingsSnapshot {
                    payload: settings.merge_into(&snapshot.payload)?,
                    parent: snapshot.parent,
                })
            });
        let request = match request {
            Ok(r) => r,
            Err(e) => {
                done(Err(e));
                return;
            }
        };
        self.settings_saving.set(true);
        // Explicit Save sequence ends the capture interval. Late acknowledgements
        // cannot enter it; after failure a new token can continue the retained list.
        self.generation.set(self.generation.get() + 1);
        self.notify("Saving sequence settings · Manuscript Save remains manual");
        let runtime = self.clone();
        let expected_payload = request.payload.clone();
        store.save_checked(request, Box::new(move |reply| {
            runtime.settings_saving.set(false);
            let reply = reply.and_then(|saved| {
                if saved.parent.as_ref().is_none_or(|s| s.is_empty()) || saved.payload != expected_payload {
                    return Err("Sequence save acknowledgement did not match; reload settings explicitly.".into());
                }
                NativeSequenceSettings::from_project_settings(&saved.payload)?;
                Ok(saved)
            });
            match reply {
                Ok(saved) => {
                    *runtime.snapshot.borrow_mut() = Some(saved);
                    *runtime.settings.borrow_mut() = settings;
                    if finish_recording { runtime.recording.borrow_mut().take(); }
                    runtime.notify("Sequence settings saved locally · Manuscript Save remains manual");
                    done(Ok(()));
                }
                Err(error) => {
                    runtime.notify(format!("Sequence settings save failed; recording retained. {error}"));
                    done(Err(error));
                }
            }
        }));
    }
    pub fn play(
        self: &Rc<Self>,
        slot: u8,
        executor: Rc<dyn SequenceExecutor>,
        done: Box<dyn FnOnce(PlaybackOutcome)>,
    ) -> Result<(), String> {
        if !self.loaded() || self.busy() {
            return Err("Load sequences and finish the current operation first.".into());
        }
        let sequence = self
            .settings
            .borrow()
            .sequence(slot)
            .cloned()
            .ok_or("No sequence is saved in this slot.")?;
        sequence.validate()?;
        executor.check_context()?; // before sensitivity changes can commit IM text
        executor.block_input()?;
        *self.playback.borrow_mut() = Some(PlaybackProgress {
            slot,
            total: sequence.actions.len(),
            accepted_steps: 0,
            saved_steps: 0,
        });
        self.notify("Playing and saving each step · Input paused");
        let run = Rc::new(PlaybackRun {
            runtime: self.clone(),
            executor,
            actions: sequence.actions,
            done: RefCell::new(Some(done)),
            finished: Cell::new(false),
        });
        if let Err(error) = run.executor.check_context() {
            run.finish(Some(error));
            return Ok(());
        }
        let next = run.clone();
        run.executor
            .drain_manual_drafts(Box::new(move |result| match result {
                Ok(()) => next.step(0),
                Err(error) => next.finish(Some(error)),
            }));
        Ok(())
    }
}

struct PlaybackRun {
    runtime: Rc<SequenceRuntime>,
    executor: Rc<dyn SequenceExecutor>,
    actions: Vec<NativeSequenceAction>,
    done: RefCell<Option<Box<dyn FnOnce(PlaybackOutcome)>>>,
    finished: Cell<bool>,
}
impl PlaybackRun {
    fn step(self: &Rc<Self>, index: usize) {
        if self.finished.get() {
            return;
        }
        if let Err(error) = self.executor.check_context() {
            self.finish(Some(error));
            return;
        }
        if index == self.actions.len() {
            self.finish(None);
            return;
        }
        let next = self.clone();
        self.executor.execute(
            self.actions[index].clone(),
            Box::new(move |result| {
                if let Err(error) = result {
                    next.finish(Some(error));
                    return;
                }
                if let Some(progress) = next.runtime.playback.borrow_mut().as_mut() {
                    progress.accepted_steps += 1;
                }
                if let Err(error) = next.executor.check_context() {
                    next.finish(Some(error));
                    return;
                }
                let after_save = next.clone();
                next.executor.save_step(Box::new(move |result| {
                    if let Err(error) = result {
                        after_save.finish(Some(error));
                        return;
                    }
                    if let Some(progress) = after_save.runtime.playback.borrow_mut().as_mut() {
                        progress.saved_steps += 1;
                    }
                    after_save.runtime.notify(format!(
                        "{} of {} commands saved · Input paused",
                        index + 1,
                        after_save.actions.len()
                    ));
                    after_save.step(index + 1);
                }));
            }),
        );
    }
    fn finish(&self, error: Option<String>) {
        if self.finished.replace(true) {
            return;
        }
        let progress = self
            .runtime
            .playback
            .borrow_mut()
            .take()
            .expect("Active playback");
        self.executor.unblock_input();
        let message = match &error {
            Some(error) => format!("Sequence stopped after {} saved commands ({} acknowledged). Draft and Undo retained. {error}", progress.saved_steps, progress.accepted_steps),
            None => format!("Sequence completed · {} commands saved", progress.saved_steps),
        };
        self.runtime.notify(message);
        let done = self.done.borrow_mut().take();
        if let Some(done) = done {
            done(PlaybackOutcome {
                accepted_steps: progress.accepted_steps,
                saved_steps: progress.saved_steps,
                error,
            });
        }
    }
}

/// Strict extraction BEFORE EditorPane adds preview/viewport fields. Never retains
/// an operation's arbitrary fields, manuscript text, offsets or clipboard contents.
pub fn action_from_operation(operation: &Value) -> Option<NativeSequenceAction> {
    let object = operation.as_object()?;
    if operation["type"] != "command" {
        return None;
    }
    let (fields, action) = match operation["command"].as_str()? {
        command @ ("bold" | "italic" | "underline" | "clearFormatting" | "ltr" | "rtl") => (
            vec!["type", "command"],
            json!({"kind":"format","command":command}),
        ),
        "lineSpacing" => (
            vec!["type", "command", "lineSpacing"],
            json!({"kind":"spacing","value":operation["lineSpacing"]}),
        ),
        "blockStyle" => (
            vec!["type", "command", "style", "language", "direction"],
            json!({
            "kind":"paragraph","style":operation["style"],"language":operation["language"],"direction":operation["direction"]}),
        ),
        _ => return None,
    };
    if object.len() != fields.len() || object.keys().any(|key| !fields.contains(&key.as_str())) {
        return None;
    }
    let action: NativeSequenceAction = serde_json::from_value(action).ok()?;
    action.validate().ok()?;
    Some(action)
}
pub fn operation_from_action(action: &NativeSequenceAction) -> Result<Value, String> {
    action.validate()?;
    Ok(match action {
        NativeSequenceAction::Format { command } => json!({"type":"command","command":command}),
        NativeSequenceAction::Spacing { value } => {
            json!({"type":"command","command":"lineSpacing","lineSpacing":value})
        }
        NativeSequenceAction::Paragraph {
            style,
            language,
            direction,
        } => {
            json!({"type":"command","command":"blockStyle","style":style,"language":language,"direction":direction})
        }
    })
}
pub fn action_label(action: &NativeSequenceAction) -> String {
    let operation = operation_from_action(action).unwrap_or(Value::Null);
    match action {
        NativeSequenceAction::Format { .. } => match operation["command"].as_str().unwrap_or("") {
            "clearFormatting" => "Clear character formatting",
            "ltr" => "Left to right",
            "rtl" => "Right to left",
            "bold" => "Bold",
            "italic" => "Italic",
            "underline" => "Underline",
            _ => "Invalid command",
        }
        .into(),
        NativeSequenceAction::Spacing { value } => format!("Line spacing {value}"),
        NativeSequenceAction::Paragraph { .. } => format!(
            "{} · {} · {}",
            operation["style"].as_str().unwrap_or(""),
            operation["language"].as_str().unwrap_or(""),
            operation["direction"].as_str().unwrap_or("")
        ),
    }
}

/// Root binds actions to the correct window/pane and reports errors on the runtime.
/// Play is visibly labelled with its saving behaviour; no shortcut schema required.
pub struct SequenceUiActions {
    pub record: Rc<dyn Fn(u8, String)>,
    pub play: Rc<dyn Fn(u8)>,
    pub remove: Rc<dyn Fn(u8)>,
    pub save_recording: Rc<dyn Fn()>,
    pub cancel_recording: Rc<dyn Fn()>,
}
pub struct CommandSequencesPage {
    pub widget: gtk::Box,
    pub names: Vec<gtk::Entry>,
    rows: Vec<(gtk::Button, gtk::Button, gtk::Button, gtk::Label)>,
    pub status: gtk::Label,
    /// Root identifies the explicitly captured source pane; do not silently use a
    /// remembered, hidden scene when this tool page is active.
    pub target_label: gtk::Label,
    runtime: Rc<SequenceRuntime>,
}
impl CommandSequencesPage {
    pub fn new(runtime: Rc<SequenceRuntime>, actions: Rc<SequenceUiActions>) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        widget.set_margin_start(32);
        widget.set_margin_end(32);
        widget.set_margin_top(26);
        let title = gtk::Label::new(Some("Command sequences"));
        title.set_xalign(0.);
        title.style_context().add_class("page-title");
        widget.pack_start(&title, false, false, 0);
        let hint = gtk::Label::new(Some("Eight named slots, up to 32 formatting commands each. Recording does not save manuscript edits. Play and save each step asks about existing drafts, then pauses input and saves each acknowledged command. A failure stops the remaining steps; earlier steps remain undoable."));
        hint.set_xalign(0.);
        hint.set_line_wrap(true);
        widget.pack_start(&hint, false, false, 0);
        let target_label = gtk::Label::new(Some(
            "Choose the intended open draft before recording or playback.",
        ));
        target_label.set_xalign(0.);
        target_label.set_line_wrap(true);
        widget.pack_start(&target_label, false, false, 0);
        let mut names = vec![];
        let mut rows = vec![];
        for slot in 1..=8 {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.pack_start(&gtk::Label::new(Some(&slot.to_string())), false, false, 0);
            let name = gtk::Entry::new();
            name.set_max_length(80);
            name.set_placeholder_text(Some("Sequence name"));
            row.pack_start(&name, true, true, 0);
            let record = gtk::Button::with_label("Record");
            let play = gtk::Button::with_label("Play and save each step");
            let remove = gtk::Button::with_label("Remove");
            let a = actions.clone();
            let entry = name.clone();
            record.connect_clicked(move |_| (a.record)(slot, entry.text().to_string()));
            let a = actions.clone();
            play.connect_clicked(move |_| (a.play)(slot));
            let a = actions.clone();
            remove.connect_clicked(move |_| (a.remove)(slot));
            row.pack_start(&record, false, false, 0);
            row.pack_start(&play, false, false, 0);
            row.pack_start(&remove, false, false, 0);
            let details = gtk::Label::new(None);
            details.set_xalign(0.);
            details.set_line_wrap(true);
            widget.pack_start(&row, false, false, 0);
            widget.pack_start(&details, false, false, 0);
            names.push(name);
            rows.push((record, play, remove, details));
        }
        let status = gtk::Label::new(None);
        status.set_xalign(0.);
        status.set_line_wrap(true);
        widget.pack_start(&status, false, false, 0);
        let page = Rc::new(Self {
            widget,
            names,
            rows,
            status,
            target_label,
            runtime,
        });
        page.restore_names();
        page.refresh();
        page
    }
    /// Call after initial load or an explicit settings save; refresh alone preserves
    /// names currently being typed. Name fields are setup for Record, not saved edits.
    pub fn restore_names(&self) {
        let settings = self.runtime.settings();
        for (i, name) in self.names.iter().enumerate() {
            name.set_text(
                settings
                    .sequence(i as u8 + 1)
                    .map(|s| s.name.as_str())
                    .unwrap_or(""),
            );
        }
    }
    pub fn refresh(&self) {
        let settings = self.runtime.settings();
        let ready = self.runtime.loaded() && !self.runtime.busy();
        for (i, (record, play, remove, details)) in self.rows.iter().enumerate() {
            let sequence = settings.sequence(i as u8 + 1);
            self.names[i].set_sensitive(ready);
            record.set_sensitive(ready);
            play.set_sensitive(ready && sequence.is_some());
            remove.set_sensitive(ready && sequence.is_some());
            details.set_text(
                &sequence
                    .map(|s| {
                        format!(
                            "{} commands: {}",
                            s.actions.len(),
                            s.actions
                                .iter()
                                .map(action_label)
                                .collect::<Vec<_>>()
                                .join("; ")
                        )
                    })
                    .unwrap_or_else(|| "Empty slot".into()),
            );
        }
        self.status.set_text(&self.runtime.message.borrow());
    }
}
/// Mount on the recorded pane/chrome so saving/cancelling never requires leaving
/// its draft and triggering a manuscript navigation save. Root refreshes on_change.
pub struct RecordingBanner {
    pub widget: gtk::Box,
    pub save: gtk::Button,
    pub cancel: gtk::Button,
    pub status: gtk::Label,
    runtime: Rc<SequenceRuntime>,
}
impl RecordingBanner {
    pub fn new(runtime: Rc<SequenceRuntime>, actions: Rc<SequenceUiActions>) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        widget.set_no_show_all(true);
        let status = gtk::Label::new(None);
        status.set_xalign(0.);
        status.set_line_wrap(true);
        widget.pack_start(&status, true, true, 0);
        let save = gtk::Button::with_label("Save sequence");
        let cancel = gtk::Button::with_label("Cancel recording");
        let a = actions.clone();
        save.connect_clicked(move |_| (a.save_recording)());
        cancel.connect_clicked(move |_| (actions.cancel_recording)());
        widget.pack_start(&save, false, false, 0);
        widget.pack_start(&cancel, false, false, 0);
        let banner = Rc::new(Self {
            widget,
            save,
            cancel,
            status,
            runtime,
        });
        banner.refresh();
        banner
    }
    pub fn refresh(&self) {
        if let Some(recording) = self.runtime.recording() {
            self.status.set_text(&format!(
                "Recording {} · {}/32 commands · Manuscript Save remains manual{}",
                recording.sequence.name,
                recording.sequence.actions.len(),
                if recording.sequence.actions.len() == 32 {
                    " · Limit reached"
                } else {
                    ""
                }
            ));
            self.save
                .set_sensitive(!self.runtime.saving() && !recording.sequence.actions.is_empty());
            self.cancel.set_sensitive(!self.runtime.saving());
            self.widget.set_no_show_all(false);
            self.widget.show_all();
            self.widget.set_no_show_all(true);
        } else {
            self.widget.hide();
        }
    }
}

#[cfg(test)]
#[path = "command_sequence_tests.rs"]
pub(crate) mod tests;
