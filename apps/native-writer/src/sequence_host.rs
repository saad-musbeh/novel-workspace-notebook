//! Concrete app-child host for the isolated command sequence framework.
//! Root adds `sequence_host: SequenceHost` to Workspace, imports its page type into
//! desk's Draft, and calls the lifecycle/input hooks below. No background saves.
use super::command_sequences::{
    operation_from_action, CommandSequencesPage, Completion, PlaybackInputPermit, PlaybackOutcome,
    RecordingBanner, RecordingToken, SequenceExecutor, SequenceInputGate, SequenceRuntime,
    SequenceStore, SequenceTarget, SequenceUiActions, SettingsCompletion, SettingsSnapshot,
    UserInputPermit,
};
use super::*;
use novel_storage::native_sequences::{
    NativeCommandSequence, NativeSequenceAction, NativeSequenceSettings,
};
use std::{rc::Weak, time::Instant};

thread_local! {
    static BINDINGS: RefCell<Vec<Weak<PaneBinding>>> = const { RefCell::new(Vec::new()) };
    // A synchronous, pane-specific capability for the host's acknowledged request.
    // It is never left enabled while waiting for a worker or storage response.
    static INTERNAL_REQUEST: RefCell<Option<String>> = const { RefCell::new(None) };
    static DRAFT_MUTATION: RefCell<Option<DraftMutationPermit>> = const { RefCell::new(None) };
}
#[derive(Default)]
pub(super) struct SequenceHost {
    pub panes: RefCell<Vec<Rc<PaneBinding>>>,
    pub pages: RefCell<Vec<Rc<SequencePage>>>,
    playback: RefCell<Option<String>>,
    mutation_epoch: Cell<u64>,
}
struct WindowLease {
    owner: Weak<Workspace>,
    generation: u64,
    permit: PlaybackInputPermit,
    sensitive: bool,
    draining: Cell<bool>,
    mutation_epoch: u64,
}
pub(super) struct PaneBinding {
    pane: Weak<EditorPane>,
    pub runtime: Rc<SequenceRuntime>,
    target: SequenceTarget,
    gate: SequenceInputGate,
    lease: RefCell<Option<WindowLease>>,
    snapshot: RefCell<Option<SettingsSnapshot>>,
    pending_snapshot: RefCell<Option<SettingsSnapshot>>,
    loading: Cell<bool>,
    load_epoch: Cell<u64>,
    names_saving: Cell<bool>,
    banner: RefCell<Option<Rc<RecordingBanner>>>,
    pages: RefCell<Vec<Weak<SequencePage>>>,
    #[cfg(test)]
    after_saved_step: RefCell<Option<Box<dyn FnOnce(Completion)>>>,
}
impl PaneBinding {
    /// Recording and settings remain attached to the exact registered editor even
    /// when an ordinary tool replaces its tab. No focus/mounting is required.
    fn recording_owner(&self) -> Result<(Rc<Workspace>, Rc<EditorPane>), String> {
        let pane = self
            .pane
            .upgrade()
            .ok_or("The captured editor instance closed.")?;
        let owner = mutation_owner(&pane)?;
        if pane.scene_id != self.target.scene_id || *owner.pid.borrow() != self.target.project_id {
            return Err("The captured novel or editor instance changed.".into());
        }
        Ok((owner, pane))
    }
    /// Physical input, composition and playback additionally require mounting.
    fn owner(&self) -> Result<(Rc<Workspace>, Rc<EditorPane>), String> {
        let pane = self
            .pane
            .upgrade()
            .ok_or("The captured editor instance closed.")?;
        let owner = desk::workspace_for_widget(pane.widget.upcast_ref())
            .ok_or("The captured draft is no longer mounted in a live window.")?;
        let registered = owner
            .panes
            .borrow()
            .get(&self.target.scene_id)
            .is_some_and(|p| Rc::ptr_eq(p, &pane));
        if owner.desk.is_closed()
            || *owner.pid.borrow() != self.target.project_id
            || !registered
            || owner.pages.page_num(&pane.widget).is_none()
        {
            return Err("The captured window, novel or editor instance changed.".into());
        }
        Ok((owner, pane))
    }
    fn visible(&self) -> Result<(Rc<Workspace>, Rc<EditorPane>), String> {
        let (owner, pane) = self.owner()?;
        if owner.current_widget() != Some(pane.widget.clone().upcast()) {
            return Err("Show the captured draft before recording or playback.".into());
        }
        Ok((owner, pane))
    }
    fn refresh(&self) {
        if let Some(banner) = self.banner.borrow().as_ref() {
            banner.refresh();
        }
        let pages = self
            .pages
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        for page in pages {
            page.refresh();
        }
        if let Ok((owner, pane)) = self.recording_owner() {
            let message = self.runtime.message.borrow().clone();
            if self.runtime.recording().is_some() || self.runtime.playback().is_some() {
                pane.error.set_text(&message);
            }
            owner.status.set_text(&message);
        }
    }
    fn store(self: &Rc<Self>) -> Result<Rc<HostStore>, String> {
        let (owner, _) = self.recording_owner()?;
        Ok(Rc::new(HostStore {
            binding: Rc::downgrade(self),
            client: owner.client.clone(),
            project_id: self.target.project_id.clone(),
        }))
    }
    fn load(self: &Rc<Self>, done: Completion) {
        let (owner, _) = match self.recording_owner() {
            Ok(v) => v,
            Err(e) => {
                done(Err(e));
                return;
            }
        };
        if self.runtime.busy() || self.names_saving.get() || self.loading.replace(true) {
            done(Err(
                "Finish the sequence operation before reloading settings.".into(),
            ));
            return;
        }
        let epoch = self.load_epoch.get() + 1;
        self.load_epoch.set(epoch);
        let binding = self.clone();
        let window = Rc::downgrade(&owner);
        let generation = owner.desk.generation.get();
        owner.client.rpc(
            "load_project_data_unambiguous",
            json!({"projectId":self.target.project_id,"kind":"settings"}),
            move |reply| {
                if binding.load_epoch.get() != epoch {
                    done(Err("A newer settings load replaced this one.".into()));
                    return;
                }
                binding.loading.set(false);
                let result = reply.and_then(|value| {
                    let (current, _) = binding.recording_owner()?;
                    let original = window
                        .upgrade()
                        .ok_or("The original sequence window closed.")?;
                    if !Rc::ptr_eq(&current, &original)
                        || current.desk.generation.get() != generation
                    {
                        return Err(
                            "The sequence window changed while settings were loading.".into()
                        );
                    }
                    let saved = SettingsSnapshot::from_rpc(&value)?;
                    binding.runtime.load(saved.clone())?;
                    *binding.snapshot.borrow_mut() = Some(saved);
                    binding.reconcile_pages();
                    Ok(())
                });
                if let Err(e) = &result {
                    binding.runtime.report_error(e);
                }
                binding.refresh();
                done(result);
            },
        );
    }
    fn reconcile_pages(&self) {
        for page in self.pages.borrow().iter().filter_map(Weak::upgrade) {
            page.reconcile();
        }
    }
    fn finish_pending_settings(&self) {
        let pending = self.pending_snapshot.borrow_mut().take();
        if let Some(saved) = pending {
            let outcome_message = self.runtime.message.borrow().clone();
            match self.runtime.load(saved) {
                Ok(()) => self.runtime.report_error(&outcome_message),
                Err(e) => self.runtime.report_error(&e),
            }
        }
        self.reconcile_pages();
        self.refresh();
    }
    fn save_recording(self: &Rc<Self>, done: Completion) {
        let (_, pane) = match self.recording_owner() {
            Ok(v) => v,
            Err(e) => {
                done(Err(e));
                return;
            }
        };
        if pane.composing.get() {
            done(Err("Finish composition before saving the sequence.".into()));
            return;
        }
        if self.names_saving.get() || self.loading.get() {
            done(Err("Sequence settings are still busy.".into()));
            return;
        }
        let binding = self.clone();
        self.wait_idle(
            Instant::now(),
            false,
            Box::new(move |result| {
                if let Err(e) = result {
                    done(Err(e));
                    return;
                }
                let store = match binding.store() {
                    Ok(s) => s,
                    Err(e) => {
                        done(Err(e));
                        return;
                    }
                };
                let after = binding.clone();
                binding.runtime.save_recording(
                    store,
                    Box::new(move |result| {
                        after.reconcile_pages();
                        after.refresh();
                        done(result);
                    }),
                );
            }),
        );
    }
    fn wait_idle(self: &Rc<Self>, started: Instant, playback: bool, done: Completion) {
        let context = if playback {
            self.check_playback().map(|_| ())
        } else {
            self.recording_owner().map(|_| ())
        };
        if let Err(e) = context {
            done(Err(e));
            return;
        }
        let pane = self.pane.upgrade().expect("Validated pane");
        if pane.composing.get() {
            done(Err("Finish the current input composition first.".into()));
            return;
        }
        if !pane.open.get() || pane.pending.get() != 0 || pane.saving.get() {
            if started.elapsed() > Duration::from_secs(15) {
                done(Err(
                    "The captured draft is still busy; remaining steps stopped.".into(),
                ));
                return;
            }
            let next = self.clone();
            glib::timeout_add_local_once(Duration::from_millis(20), move || {
                next.wait_idle(started, playback, done)
            });
            return;
        }
        done(Ok(()));
    }
    fn check_playback(&self) -> Result<(Rc<Workspace>, Rc<EditorPane>), String> {
        let (owner, pane) = self.visible()?;
        if pane.composing.get() {
            return Err("Input composition stopped the sequence.".into());
        }
        if !pane.open.get() {
            return Err("The captured draft is not open.".into());
        }
        if let Some(lease) = self.lease.borrow().as_ref() {
            let original = lease.owner.upgrade().ok_or("The playback window closed.")?;
            if !Rc::ptr_eq(&owner, &original)
                || owner.desk.generation.get() != lease.generation
                || !self.gate.allows_playback(lease.permit)
                || owner.sequence_host.mutation_epoch.get() != lease.mutation_epoch
                || owner.sequence_host.playback.borrow().as_deref() != Some(&self.target.editor_id)
            {
                return Err("The playback ownership or input permit changed.".into());
            }
        }
        Ok((owner, pane))
    }
}

/// Real checked project-settings CAS, preserving the exact captured full payload.
struct HostStore {
    binding: Weak<PaneBinding>,
    client: Process,
    project_id: String,
}
fn settings_save_args(project_id: &str, snapshot: &SettingsSnapshot) -> Result<Value, String> {
    if project_id.is_empty() || !snapshot.payload.is_object() {
        return Err("Missing sequence project settings.".into());
    }
    NativeSequenceSettings::from_project_settings(&snapshot.payload)?;
    Ok(
        json!({"projectId":project_id,"kind":"settings","payload":snapshot.payload,"parent":snapshot.parent,"dependencies":[]}),
    )
}
impl SequenceStore for HostStore {
    fn save_checked(&self, snapshot: SettingsSnapshot, done: SettingsCompletion) {
        let binding = match self.binding.upgrade() {
            Some(b) => b,
            None => {
                done(Err("The sequence session closed.".into()));
                return;
            }
        };
        if let Err(e) = binding.recording_owner() {
            done(Err(e));
            return;
        }
        rpc_settings_save(
            &self.client,
            &self.project_id,
            snapshot,
            Box::new(move |result| {
                if let Ok(saved) = &result {
                    *binding.snapshot.borrow_mut() = Some(saved.clone());
                }
                done(result);
            }),
        );
    }
}
fn rpc_settings_save(
    client: &Process,
    project_id: &str,
    snapshot: SettingsSnapshot,
    done: SettingsCompletion,
) {
    let args = match settings_save_args(project_id, &snapshot) {
        Ok(v) => v,
        Err(e) => {
            done(Err(e));
            return;
        }
    };
    let expected = snapshot.payload;
    client.rpc("save_project_data_checked", args, move |reply| {
        done(reply.and_then(|v|SettingsSnapshot::from_rpc(&v)).and_then(|saved| {
            if saved.parent.is_none() || saved.payload != expected { return Err("Settings acknowledgement did not match the requested sequence payload.".into()); }
            Ok(saved)
        }));
    });
}

struct HostExecutor {
    binding: Rc<PaneBinding>,
}
impl SequenceExecutor for HostExecutor {
    fn check_context(&self) -> Result<(), String> {
        self.binding.check_playback().map(|_| ())
    }
    fn block_input(&self) -> Result<(), String> {
        let (owner, pane) = self.binding.visible()?;
        if pane.composing.get()
            || self.binding.names_saving.get()
            || self.binding.loading.get()
            || owner.desk.busy.get()
            || owner.sequence_host.playback.borrow().is_some()
            || self.binding.lease.borrow().is_some()
        {
            return Err("Finish the current composition or window operation first.".into());
        }
        let permit = self.binding.gate.acquire()?;
        owner
            .sequence_host
            .mutation_epoch
            .set(owner.sequence_host.mutation_epoch.get() + 1);
        *owner.sequence_host.playback.borrow_mut() = Some(self.binding.target.editor_id.clone());
        *self.binding.lease.borrow_mut() = Some(WindowLease {
            owner: Rc::downgrade(&owner),
            generation: owner.desk.generation.get(),
            permit,
            sensitive: owner.window.is_sensitive(),
            draining: Cell::new(true),
            mutation_epoch: owner.sequence_host.mutation_epoch.get(),
        });
        // Acquire the logical gate first: focus-out must not commit queued text.
        owner.window.set_sensitive(false);
        Ok(())
    }
    fn drain_manual_drafts(&self, done: Completion) {
        let (owner, _) = match self.binding.check_playback() {
            Ok(v) => v,
            Err(e) => {
                done(Err(e));
                return;
            }
        };
        let binding = self.binding.clone();
        // confirm_all owns desk.busy itself, waits outstanding operations, and uses
        // the actual Save/Discard/Cancel dialogs and all registered page Drafts.
        owner.confirm_all(move |accepted| {
            let result = binding.check_playback().and_then(|(owner, _)| {
                owner.window.set_sensitive(false);
                if let Some(lease) = binding.lease.borrow().as_ref() {
                    lease.draining.set(false);
                }
                if !accepted {
                    return Err("Manual draft choice cancelled or a draft save failed.".into());
                }
                Ok(())
            });
            if let Err(e) = result {
                done(Err(e));
                return;
            }
            binding.wait_idle(Instant::now(), true, done);
        });
    }
    fn execute(&self, action: NativeSequenceAction, done: Completion) {
        let operation = match operation_from_action(&action) {
            Ok(v) => v,
            Err(e) => {
                done(Err(e));
                return;
            }
        };
        let binding = self.binding.clone();
        self.binding.wait_idle(
            Instant::now(),
            true,
            Box::new(move |result| {
                if let Err(e) = result {
                    done(Err(e));
                    return;
                }
                let (_, pane) = match binding.check_playback() {
                    Ok(v) => v,
                    Err(e) => {
                        done(Err(e));
                        return;
                    }
                };
                with_internal_request(&binding.target.editor_id, || {
                    pane.request(operation, true, move |reply| done(reply.map(|_| ())))
                });
            }),
        );
    }
    fn save_step(&self, done: Completion) {
        let binding = self.binding.clone();
        self.binding.wait_idle(
            Instant::now(),
            true,
            Box::new(move |result| {
                if let Err(e) = result {
                    done(Err(e));
                    return;
                }
                let (owner, pane) = match binding.check_playback() {
                    Ok(v) => v,
                    Err(e) => {
                        done(Err(e));
                        return;
                    }
                };
                let after = binding.clone();
                let p = pane.clone();
                let capability =
                    mutation_capability(&pane, DraftMutation::Save, MutationAuthority::Step);
                let capability = match capability {
                    Ok(p) => p,
                    Err(e) => {
                        done(Err(e));
                        return;
                    }
                };
                with_mutation_capability(capability, || {
                    owner.save_pane(pane, move |result| {
                        if let Err(e) = result {
                            done(Err(e));
                            return;
                        }
                        #[cfg(test)]
                        let final_binding = after.clone();
                        after.wait_idle(
                            Instant::now(),
                            true,
                            Box::new(move |result| {
                                let result = result.and_then(|_| {
                                    if p.dirty.get() {
                                        Err("The accepted step was not fully saved; draft retained."
                                        .into())
                                    } else {
                                        Ok(())
                                    }
                                });
                                #[cfg(test)]
                                if result.is_ok() {
                                    let hook = final_binding.after_saved_step.borrow_mut().take();
                                    if let Some(hook) = hook {
                                        hook(done);
                                        return;
                                    }
                                }
                                done(result);
                            }),
                        );
                    })
                });
            }),
        );
    }
    fn unblock_input(&self) {
        let lease = self.binding.lease.borrow_mut().take();
        if let Some(lease) = lease {
            if !self.binding.gate.release(lease.permit) {
                return;
            }
            if let Some(owner) = lease.owner.upgrade() {
                if owner.sequence_host.playback.borrow().as_deref()
                    == Some(&self.binding.target.editor_id)
                {
                    owner.sequence_host.playback.borrow_mut().take();
                    owner
                        .sequence_host
                        .mutation_epoch
                        .set(owner.sequence_host.mutation_epoch.get() + 1);
                    if !owner.desk.is_closed()
                        && owner.desk.generation.get() == lease.generation
                        && !owner.desk.busy.get()
                    {
                        owner.window.set_sensitive(lease.sensitive);
                    }
                }
            }
        }
    }
}
fn with_internal_request<T>(editor_id: &str, operation: impl FnOnce() -> T) -> T {
    struct Restore(Option<String>);
    impl Drop for Restore {
        fn drop(&mut self) {
            INTERNAL_REQUEST.with(|v| *v.borrow_mut() = self.0.take());
        }
    }
    let prior = INTERNAL_REQUEST.with(|v| v.replace(Some(editor_id.into())));
    let _restore = Restore(prior);
    operation()
}
fn binding_for(pane: &Rc<EditorPane>) -> Option<Rc<PaneBinding>> {
    BINDINGS.with(|bindings| {
        let mut bindings = bindings.borrow_mut();
        bindings.retain(|b| b.strong_count() != 0);
        bindings
            .iter()
            .filter_map(Weak::upgrade)
            .find(|b| b.pane.upgrade().is_some_and(|p| Rc::ptr_eq(&p, pane)))
    })
}

/// Capture at user-input scheduling time, especially before request_text. Never
/// reacquire a fresh permit inside a delayed clipboard/IME callback.
#[derive(Clone)]
pub(crate) struct InputPermit {
    binding: Weak<PaneBinding>,
    owner: Weak<Workspace>,
    generation: u64,
    permit: UserInputPermit,
    mutation_epoch: u64,
    ownership: InputOwnership,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum InputOwnership {
    Mounted,
    Registered,
}
pub(crate) fn input_permit(pane: &Rc<EditorPane>) -> Option<InputPermit> {
    let owner = desk::workspace_for_widget(pane.widget.upcast_ref())?;
    owner.sequence_bind_pane(pane);
    let binding = binding_for(pane)?;
    let (owner, p) = binding.owner().ok()?;
    if owner.sequence_window_blocked() || owner.desk.busy.get() || p.saving.get() || !p.open.get() {
        return None;
    }
    Some(InputPermit {
        binding: Rc::downgrade(&binding),
        owner: Rc::downgrade(&owner),
        generation: owner.desk.generation.get(),
        permit: binding.gate.user_permit()?,
        mutation_epoch: owner.sequence_host.mutation_epoch.get(),
        ownership: InputOwnership::Mounted,
    })
}
pub(crate) fn input_allowed(pane: &Rc<EditorPane>, permit: &InputPermit) -> bool {
    permitted_input(pane, permit, false, InputOwnership::Mounted)
}
/// Use only a permit captured at preedit-start (never mint one during drain).
/// Ordinary focus-out/manual draft drain may commit that existing word while
/// desk.busy is true. Playback, saving and changed ownership still reject it.
pub(crate) fn composition_allowed(pane: &Rc<EditorPane>, permit: &InputPermit) -> bool {
    permitted_input(pane, permit, true, InputOwnership::Mounted)
}
fn input_state_allows(busy: bool, saving: bool, playback: bool, composition: bool) -> bool {
    !saving && !playback && (composition || !busy)
}
fn permitted_input(
    pane: &Rc<EditorPane>,
    permit: &InputPermit,
    composition: bool,
    ownership: InputOwnership,
) -> bool {
    if permit.ownership != ownership {
        return false;
    }
    let Some(binding) = permit.binding.upgrade() else {
        return false;
    };
    let Some(original) = permit.owner.upgrade() else {
        return false;
    };
    let resolved = match ownership {
        InputOwnership::Mounted => binding.owner(),
        InputOwnership::Registered => mutation_owner(pane).and_then(|owner| {
            let p = binding.pane.upgrade().ok_or("The target editor closed.")?;
            Ok((owner, p))
        }),
    };
    let Ok((owner, p)) = resolved else {
        return false;
    };
    Rc::ptr_eq(&p, pane)
        && Rc::ptr_eq(&owner, &original)
        && binding.target.project_id == *owner.pid.borrow()
        && captured_context_matches(
            permit.generation,
            permit.mutation_epoch,
            owner.desk.generation.get(),
            owner.sequence_host.mutation_epoch.get(),
        )
        && input_state_allows(
            owner.desk.busy.get(),
            p.saving.get(),
            owner.sequence_window_blocked(),
            composition,
        )
        && binding.gate.allows_user(permit.permit)
}
fn captured_context_matches(
    generation: u64,
    epoch: u64,
    current_generation: u64,
    current_epoch: u64,
) -> bool {
    generation == current_generation && epoch == current_epoch
}

/// Tool pages may replace the scene tab while retaining its registered editor.
/// Capture at the author action's entry, before any async source checks. Physical
/// input and IME still require a mounted pane and cannot use this permit. Capture
/// may precede worker opening; delivery requires the editor to be open.
pub(crate) fn tool_input_permit(
    owner: &Rc<Workspace>,
    pane: &Rc<EditorPane>,
) -> Option<InputPermit> {
    let registered = mutation_owner(pane).ok()?;
    if !Rc::ptr_eq(owner, &registered)
        || owner.pid.borrow().is_empty()
        || !input_state_allows(
            owner.desk.busy.get(),
            pane.saving.get(),
            owner.sequence_window_blocked(),
            false,
        )
    {
        return None;
    }
    owner.sequence_bind_pane(pane);
    let binding = binding_for(pane)?;
    if binding.target.project_id != *owner.pid.borrow() {
        return None;
    }
    Some(InputPermit {
        binding: Rc::downgrade(&binding),
        owner: Rc::downgrade(owner),
        generation: owner.desk.generation.get(),
        permit: binding.gate.user_permit()?,
        mutation_epoch: owner.sequence_host.mutation_epoch.get(),
        ownership: InputOwnership::Registered,
    })
}
pub(crate) fn tool_input_allowed(pane: &Rc<EditorPane>, permit: &InputPermit) -> bool {
    pane.open.get() && permitted_input(pane, permit, false, InputOwnership::Registered)
}

/// Capture BEFORE ensure_pane when the tool can create a missing cached editor.
/// An existing editor retains its exact original permit/instance. A new editor
/// may receive a pane gate only while the original owner/window epoch is valid.
pub(crate) struct ToolActionPermit {
    window: WindowInputPermit,
    scene_id: String,
    existing: Option<InputPermit>,
}
pub(crate) fn tool_action_permit(
    owner: &Rc<Workspace>,
    scene_id: &str,
) -> Option<ToolActionPermit> {
    let window = window_input_permit(owner)?;
    if scene_id.is_empty() {
        return None;
    }
    let pane = owner.panes.borrow().get(scene_id).cloned();
    let existing = match pane {
        Some(pane) => Some(tool_input_permit(owner, &pane)?),
        None => None,
    };
    Some(ToolActionPermit {
        window,
        scene_id: scene_id.into(),
        existing,
    })
}

/// Capture synchronously before notebook append's ensure_pane/readiness polling.
/// This authorizes only the original window context, never a pane by itself.
/// Delivery must ALSO check the exact registered target, expected current page,
/// editor open/composition/saving/pending state and any action-specific checks.
pub(crate) struct WindowInputPermit {
    owner: Weak<Workspace>,
    project_id: String,
    generation: u64,
    mutation_epoch: u64,
}
pub(crate) fn window_input_permit(owner: &Rc<Workspace>) -> Option<WindowInputPermit> {
    if !tool_owner_ready(owner) {
        return None;
    }
    Some(WindowInputPermit {
        owner: Rc::downgrade(owner),
        project_id: owner.pid.borrow().clone(),
        generation: owner.desk.generation.get(),
        mutation_epoch: owner.sequence_host.mutation_epoch.get(),
    })
}
pub(crate) fn window_input_allowed(owner: &Rc<Workspace>, permit: &WindowInputPermit) -> bool {
    permit
        .owner
        .upgrade()
        .is_some_and(|original| Rc::ptr_eq(&original, owner))
        && tool_owner_ready(owner)
        && *owner.pid.borrow() == permit.project_id
        && captured_context_matches(
            permit.generation,
            permit.mutation_epoch,
            owner.desk.generation.get(),
            owner.sequence_host.mutation_epoch.get(),
        )
}
fn tool_owner_ready(owner: &Rc<Workspace>) -> bool {
    !owner.desk.is_closed()
        && !owner.pid.borrow().is_empty()
        && !owner.desk.busy.get()
        && !owner.sequence_window_blocked()
        && WINDOWS.with(|windows| windows.borrow().iter().any(|a| Rc::ptr_eq(a, owner)))
}
/// Convert only the action-start capability, never reacquire after ensure_pane.
/// Keep the returned permit through all later RPCs and recheck before mutation.
pub(crate) fn tool_input_after_ensure(
    pane: &Rc<EditorPane>,
    action: &ToolActionPermit,
) -> Option<InputPermit> {
    let owner = action.window.owner.upgrade()?;
    if !window_input_allowed(&owner, &action.window) || pane.scene_id != action.scene_id {
        return None;
    }
    if let Some(original) = &action.existing {
        return permitted_input(pane, original, false, InputOwnership::Registered)
            .then(|| original.clone());
    }
    tool_input_permit(&owner, pane)
}
pub(crate) fn input_ready(pane: &Rc<EditorPane>) -> bool {
    input_permit(pane).is_some_and(|permit| input_allowed(pane, &permit))
}
/// EditorPane::request's final playback gate, before recording/augmentation.
/// Existing host operations on cached/unmounted panes remain available outside
/// playback. Physical input handlers additionally check input_ready/old permits;
/// this final gate alone cannot identify an old clipboard callback after release.
pub(crate) fn request_allowed(pane: &Rc<EditorPane>, value: &Value) -> bool {
    if value["type"] == "refresh" {
        return true;
    }
    if let Some(binding) = binding_for(pane) {
        let internal =
            INTERNAL_REQUEST.with(|v| v.borrow().as_deref() == Some(&binding.target.editor_id));
        if internal {
            return binding.check_playback().is_ok();
        }
        if binding.lease.borrow().is_some() || binding.gate.user_permit().is_none() {
            return false;
        }
    }
    !desk::workspace_for_widget(pane.widget.upcast_ref())
        .is_some_and(|owner| owner.sequence_window_blocked())
}
pub(crate) struct RecordAcceptance {
    binding: Weak<PaneBinding>,
    token: RecordingToken,
    action: NativeSequenceAction,
}
pub(crate) fn record_before(
    pane: &Rc<EditorPane>,
    value: &Value,
    editing: bool,
) -> Option<RecordAcceptance> {
    if !editing || !input_ready(pane) {
        return None;
    }
    let binding = binding_for(pane)?;
    let (token, action) = binding.runtime.recording_command(&binding.target, value)?;
    Some(RecordAcceptance {
        binding: Rc::downgrade(&binding),
        token,
        action,
    })
}
pub(crate) fn record_after(
    pane: &Rc<EditorPane>,
    captured: Option<RecordAcceptance>,
    reply: &crate::process::Reply,
) {
    if reply.is_err() {
        return;
    }
    let Some(captured) = captured else {
        return;
    };
    let Some(binding) = captured.binding.upgrade() else {
        return;
    };
    if binding
        .recording_owner()
        .is_ok_and(|(_, p)| Rc::ptr_eq(&p, pane))
    {
        binding
            .runtime
            .record_accepted(captured.token, captured.action);
    }
}
/// Detach/ownership transfer invalidates queued callbacks without cancelling edits.
pub(crate) fn invalidate_input(pane: &Rc<EditorPane>) -> Result<(), String> {
    if let Some(binding) = binding_for(pane) {
        let permit = binding.gate.acquire()?;
        binding.gate.release(permit);
    }
    Ok(())
}
#[cfg(test)]
pub(crate) fn cancel_recording(pane: &Rc<EditorPane>) -> Result<(), String> {
    if let Some(binding) = binding_for(pane) {
        binding.runtime.cancel_recording()?;
        binding.reconcile_pages();
    }
    Ok(())
}

/// A scene save changes storage; discard reopens the worker and clears its Undo.
/// Read-only EditorPane::snapshot intentionally needs no mutation capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DraftMutation {
    Save,
    Discard,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutationAuthority {
    Ordinary,
    Drain,
    Step,
}
#[derive(Clone)]
pub(crate) struct DraftMutationPermit {
    pane: Weak<EditorPane>,
    owner: Weak<Workspace>,
    project_id: String,
    generation: u64,
    epoch: u64,
    kind: DraftMutation,
    authority: MutationAuthority,
    lease: Option<(Weak<PaneBinding>, PlaybackInputPermit)>,
}
fn mutation_authority_allows(
    authority: MutationAuthority,
    kind: DraftMutation,
    playback: bool,
    draining: bool,
) -> bool {
    match authority {
        MutationAuthority::Ordinary => !playback,
        MutationAuthority::Drain => playback && draining,
        MutationAuthority::Step => playback && !draining && kind == DraftMutation::Save,
    }
}
fn mutation_owner(pane: &Rc<EditorPane>) -> Result<Rc<Workspace>, String> {
    // Manual confirmation also saves cached, currently unmounted panes. Resolve
    // their exact registered instance; do not require GTK focus or input_ready.
    let owners = WINDOWS.with(|windows| {
        windows
            .borrow()
            .iter()
            .filter(|a| {
                !a.desk.is_closed()
                    && a.panes
                        .borrow()
                        .get(&pane.scene_id)
                        .is_some_and(|p| Rc::ptr_eq(p, pane))
            })
            .cloned()
            .collect::<Vec<_>>()
    });
    if owners.len() != 1 {
        return Err("The draft has no unique live window owner.".into());
    }
    let owner = owners.into_iter().next().unwrap();
    if binding_for(pane).is_some_and(|binding| binding.target.project_id != *owner.pid.borrow()) {
        return Err("The draft's novel changed.".into());
    }
    Ok(owner)
}
fn mutation_capability(
    pane: &Rc<EditorPane>,
    kind: DraftMutation,
    authority: MutationAuthority,
) -> Result<DraftMutationPermit, String> {
    let owner = mutation_owner(pane)?;
    let lease = if authority == MutationAuthority::Ordinary {
        if owner.sequence_window_blocked() {
            return Err("Ordinary save/discard is paused during playback.".into());
        }
        None
    } else {
        let id = owner
            .sequence_host
            .playback
            .borrow()
            .clone()
            .ok_or("No playback lease authorizes this mutation.")?;
        let source = owner
            .sequence_host
            .panes
            .borrow()
            .iter()
            .find(|b| b.target.editor_id == id)
            .cloned()
            .ok_or("The playback pane lease closed.")?;
        let (current, _) = source.check_playback()?;
        if !Rc::ptr_eq(&owner, &current) {
            return Err("The mutation belongs to another playback window.".into());
        }
        let permit = {
            let lease = source.lease.borrow();
            let lease = lease.as_ref().ok_or("The playback lease ended.")?;
            if !mutation_authority_allows(authority, kind, true, lease.draining.get()) {
                return Err(
                    "This playback phase does not authorize the requested save/discard.".into(),
                );
            }
            lease.permit
        };
        Some((Rc::downgrade(&source), permit))
    };
    let project_id = owner.pid.borrow().clone();
    Ok(DraftMutationPermit {
        pane: Rc::downgrade(pane),
        owner: Rc::downgrade(&owner),
        project_id,
        generation: owner.desk.generation.get(),
        epoch: owner.sequence_host.mutation_epoch.get(),
        kind,
        authority,
        lease,
    })
}
/// At save_pane entry, capture Save; at discard entry, capture Discard. A drain
/// phase alone grants nothing: only the explicit choice's scoped capability does.
pub(crate) fn authorize_draft_mutation(
    pane: &Rc<EditorPane>,
    kind: DraftMutation,
) -> Result<DraftMutationPermit, String> {
    let scoped = DRAFT_MUTATION.with(|current| current.borrow().clone());
    if let Some(permit) = scoped {
        if draft_mutation_allowed(pane, &permit, kind) {
            return Ok(permit);
        }
        return Err(
            "The save/discard capability is stale or belongs to another pane/action.".into(),
        );
    }
    mutation_capability(pane, kind, MutationAuthority::Ordinary)
}
/// Recheck the captured Save permit in snapshot's callback immediately BEFORE the
/// save_scene RPC, or Discard immediately BEFORE its worker open request. Never
/// reacquire a fresh permit in an asynchronous user callback. An already-dispatched
/// operation's acknowledgement remains authoritative; do not undo it on epoch change.
pub(crate) fn draft_mutation_allowed(
    pane: &Rc<EditorPane>,
    permit: &DraftMutationPermit,
    kind: DraftMutation,
) -> bool {
    let Some(captured_pane) = permit.pane.upgrade() else {
        return false;
    };
    let Some(original) = permit.owner.upgrade() else {
        return false;
    };
    let Ok(owner) = mutation_owner(pane) else {
        return false;
    };
    if !Rc::ptr_eq(pane, &captured_pane)
        || !Rc::ptr_eq(&owner, &original)
        || kind != permit.kind
        || *owner.pid.borrow() != permit.project_id
        || owner.desk.generation.get() != permit.generation
        || owner.sequence_host.mutation_epoch.get() != permit.epoch
    {
        return false;
    }
    let draining = if let Some((source, gate_permit)) = &permit.lease {
        let Some(source) = source.upgrade() else {
            return false;
        };
        if source.check_playback().is_err() || !source.gate.allows_playback(*gate_permit) {
            return false;
        }
        let lease = source.lease.borrow();
        let Some(lease) = lease.as_ref() else {
            return false;
        };
        lease.draining.get()
    } else {
        false
    };
    mutation_authority_allows(
        permit.authority,
        kind,
        owner.sequence_window_blocked(),
        draining,
    )
}
fn with_mutation_capability<T>(permit: DraftMutationPermit, operation: impl FnOnce() -> T) -> T {
    struct Restore(Option<DraftMutationPermit>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DRAFT_MUTATION.with(|current| *current.borrow_mut() = self.0.take());
        }
    }
    let prior = DRAFT_MUTATION.with(|current| current.replace(Some(permit)));
    let _restore = Restore(prior);
    operation()
}
/// Root calls ONLY around the Scene Save/Discard branch AFTER the author chooses
/// it in desk::confirm. The capability targets that one participant pane and the
/// captured playback lease, including cached panes drained by confirm_all. Outside
/// playback, ordinary manual confirmations remain supported despite desk.busy.
pub(crate) fn with_drain_draft_mutation<T>(
    pane: &Rc<EditorPane>,
    kind: DraftMutation,
    operation: impl FnOnce() -> T,
) -> Result<T, String> {
    let owner = mutation_owner(pane)?;
    let authority = if owner.sequence_window_blocked() {
        MutationAuthority::Drain
    } else {
        MutationAuthority::Ordinary
    };
    let permit = mutation_capability(pane, kind, authority)?;
    Ok(with_mutation_capability(permit, operation))
}

/// Root's ordinary Draft variant. New-slot names cannot become a saved empty
/// sequence: Save requests recording first; Discard restores only name fields.
pub(super) struct SequencePage {
    pub widget: gtk::Box,
    pub dirty: Cell<bool>,
    pub saving: Cell<bool>,
    pub composing: Cell<bool>,
    pub loaded: Cell<bool>,
    pub controls: Rc<CommandSequencesPage>,
    binding: Rc<PaneBinding>,
    updating: Cell<bool>,
    confirmed: RefCell<Vec<String>>,
    preedit: RefCell<Vec<bool>>,
}
fn renamed_settings(
    settings: &NativeSequenceSettings,
    names: &[String],
) -> Result<NativeSequenceSettings, String> {
    if names.len() != 8 {
        return Err("Expected eight sequence name fields.".into());
    }
    let mut settings = settings.clone();
    for (i, name) in names.iter().enumerate() {
        if let Some(sequence) = settings
            .sequences
            .iter_mut()
            .find(|s| s.slot as usize == i + 1)
        {
            NativeCommandSequence::validate_name(sequence.slot, name)?;
            sequence.name = name.clone();
        } else if !name.trim().is_empty() {
            return Err(format!("Slot {} needs recorded formatting commands before its name can be saved. Choose Record, Discard or Cancel.", i + 1));
        }
    }
    settings.validate()?;
    Ok(settings)
}
impl SequencePage {
    fn owner(&self) -> Result<Rc<Workspace>, String> {
        desk::workspace_for_widget(self.widget.upcast_ref())
            .filter(|a| !a.desk.is_closed() && *a.pid.borrow() == self.binding.target.project_id)
            .ok_or_else(|| "The sequence settings page's window or novel changed.".into())
    }
    fn new(binding: Rc<PaneBinding>) -> Rc<Self> {
        let handle: Rc<RefCell<Weak<SequencePage>>> = Rc::new(RefCell::new(Weak::new()));
        let record_handle = handle.clone();
        let weak = Rc::downgrade(&binding);
        let record = Rc::new(move |slot, name: String| {
            if let Some(binding) = weak.upgrade() {
                let result = page_target(&record_handle, &binding).and_then(|owner| {
                    let (_, pane) = binding.recording_owner()?;
                    if binding.names_saving.get()
                        || binding.loading.get()
                        || owner.sequence_window_blocked()
                        || owner.desk.busy.get()
                        || pane.composing.get()
                        || pane.pending.get() != 0
                        || pane.saving.get()
                    {
                        return Err("Finish input and pending draft work before recording.".into());
                    }
                    if binding
                        .pages
                        .borrow()
                        .iter()
                        .filter_map(Weak::upgrade)
                        .any(|p| p.composing.get())
                    {
                        return Err("Finish entering the sequence name first.".into());
                    }
                    // Explicit Record shows the existing pane while retaining the tracked
                    // names page in its tab. It saves neither manuscript nor names.
                    binding
                        .runtime
                        .start_recording(binding.target.clone(), slot, name)?;
                    owner.install(&pane.widget, &pane.title.borrow(), false);
                    pane.area.grab_focus();
                    binding.refresh();
                    Ok(())
                });
                if let Err(e) = result {
                    binding.runtime.report_error(&e);
                }
            }
        });
        let weak = Rc::downgrade(&binding);
        let play_handle = handle.clone();
        let play = Rc::new(move |slot| {
            if let Some(binding) = weak.upgrade() {
                if let Ok(owner) = page_target(&play_handle, &binding) {
                    let (_, pane) = binding.recording_owner().expect("Validated target");
                    if owner.sequence_window_blocked() || owner.desk.busy.get() {
                        return;
                    }
                    owner.install(&pane.widget, &pane.title.borrow(), false);
                    if let Err(e) = owner.sequence_play(slot, &pane, Box::new(|_| {})) {
                        binding.runtime.report_error(&e);
                    }
                } else {
                    binding.runtime.report_error(
                        "The captured draft closed; open sequences from its intended draft again.",
                    );
                }
            }
        });
        let base = binding_actions(&binding, record, play);
        let remove_handle = handle.clone();
        let remove_binding = Rc::downgrade(&binding);
        let remove_action = base.remove.clone();
        let actions = Rc::new(SequenceUiActions {
            record: base.record.clone(),
            play: base.play.clone(),
            save_recording: base.save_recording.clone(),
            cancel_recording: base.cancel_recording.clone(),
            remove: Rc::new(move |slot| {
                if let Some(b) = remove_binding.upgrade() {
                    if let Err(e) = page_target(&remove_handle, &b) {
                        b.runtime.report_error(&e);
                        return;
                    }
                }
                remove_action(slot);
            }),
        });
        let controls = CommandSequencesPage::new(binding.runtime.clone(), actions);
        controls.target_label.set_text(&format!(
            "Captured draft: {}",
            binding
                .pane
                .upgrade()
                .map(|p| p.title.borrow().clone())
                .unwrap_or_else(|| "Closed".into())
        ));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.widget.pack_start(&row, false, false, 0);
        let save = button(&row, "Save names", || {});
        let discard = button(&row, "Discard name changes", || {});
        let reload = button(&row, "Reload saved settings", || {});
        let page = Rc::new(Self {
            widget: controls.widget.clone(),
            controls,
            binding,
            dirty: Cell::new(false),
            saving: Cell::new(false),
            composing: Cell::new(false),
            loaded: Cell::new(false),
            updating: Cell::new(false),
            confirmed: RefCell::new(vec![String::new(); 8]),
            preedit: RefCell::new(vec![false; 8]),
        });
        *handle.borrow_mut() = Rc::downgrade(&page);
        for (i, entry) in page.controls.names.iter().enumerate() {
            let weak = Rc::downgrade(&page);
            entry.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    if !p.updating.get() {
                        p.update_dirty();
                    }
                }
            });
            let weak = Rc::downgrade(&page);
            entry.connect_preedit_changed(move |_, text| {
                if let Some(p) = weak.upgrade() {
                    p.preedit.borrow_mut()[i] = !text.is_empty();
                    p.composing.set(p.preedit.borrow().iter().any(|v| *v));
                }
            });
        }
        let weak = Rc::downgrade(&page);
        save.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if let Ok(owner) = p.owner() {
                    p.save(&owner, |_| {});
                }
            }
        });
        let weak = Rc::downgrade(&page);
        discard.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if !p.composing.get() && !p.saving.get() {
                    p.discard();
                }
            }
        });
        let weak = Rc::downgrade(&page);
        reload.connect_clicked(move |_| if let Some(p) = weak.upgrade() {
            if p.dirty.get() || p.binding.runtime.busy() { p.binding.runtime.report_error("Save or discard name changes and finish/cancel recording before reloading settings."); return; }
            p.binding.load(Box::new(|_| {}));
        });
        page.binding.pages.borrow_mut().push(Rc::downgrade(&page));
        page.reconcile();
        page.refresh();
        page
    }
    fn names(&self) -> Vec<String> {
        self.controls
            .names
            .iter()
            .map(|n| n.text().to_string())
            .collect()
    }
    fn update_dirty(&self) {
        self.dirty.set(self.names() != *self.confirmed.borrow());
    }
    fn reconcile(&self) {
        let settings = self.binding.runtime.settings();
        let prior = self.confirmed.borrow().clone();
        let names = (1..=8)
            .map(|slot| {
                settings
                    .sequence(slot)
                    .map(|s| s.name.clone())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        self.updating.set(true);
        for (i, entry) in self.controls.names.iter().enumerate() {
            if entry.text().as_str() == prior[i] || entry.text().as_str() == names[i] {
                entry.set_text(&names[i]);
            }
        }
        *self.confirmed.borrow_mut() = names;
        self.updating.set(false);
        self.loaded.set(self.binding.runtime.loaded());
        self.update_dirty();
    }
    fn refresh(&self) {
        self.loaded.set(self.binding.runtime.loaded());
        self.controls.refresh();
        self.widget
            .set_sensitive(!self.binding.names_saving.get() && !self.binding.loading.get());
        if self.dirty.get() && !self.binding.runtime.busy() {
            self.controls
                .status
                .set_text("Unsaved sequence names · Save names / Discard / Cancel before leaving");
        }
    }
    pub fn discard(&self) {
        self.updating.set(true);
        for (entry, name) in self
            .controls
            .names
            .iter()
            .zip(self.confirmed.borrow().iter())
        {
            entry.set_text(name);
        }
        self.updating.set(false);
        self.dirty.set(false);
        self.refresh();
    }
    pub fn save(
        self: &Rc<Self>,
        owner: &Rc<Workspace>,
        callback: impl FnOnce(Result<(), String>) + 'static,
    ) {
        let done: Completion = Box::new(callback);
        let result: Result<SettingsSnapshot, String> = (|| {
            let current = self.owner()?;
            if !Rc::ptr_eq(&current, owner) {
                return Err("Sequence page belongs to another window.".into());
            }
            if self.saving.get()
                || self.binding.names_saving.get()
                || self.binding.runtime.saving()
                || self.binding.loading.get()
            {
                return Err("Sequence settings are still busy.".into());
            }
            if self.composing.get() {
                return Err("Finish entering sequence names first.".into());
            }
            if !self.loaded.get() {
                return Err("Wait for unambiguous saved settings to load.".into());
            }
            if self.binding.runtime.recording().is_some() {
                return Err(
                    "Save sequence or Cancel recording explicitly in the draft banner first."
                        .into(),
                );
            }
            if self.binding.runtime.playback().is_some()
                && !self
                    .binding
                    .lease
                    .borrow()
                    .as_ref()
                    .is_some_and(|lease| lease.draining.get())
            {
                return Err(
                    "Name saves are paused after the initial playback draft choice.".into(),
                );
            }
            let settings = renamed_settings(&self.binding.runtime.settings(), &self.names())?;
            let snapshot = self
                .binding
                .snapshot
                .borrow()
                .clone()
                .ok_or("Saved settings are unavailable.")?;
            Ok(SettingsSnapshot {
                payload: settings.merge_into(&snapshot.payload)?,
                parent: snapshot.parent,
            })
        })();
        let request = match result {
            Ok(r) => r,
            Err(e) => {
                self.controls.status.set_text(&e);
                done(Err(e));
                return;
            }
        };
        if !self.dirty.get() {
            done(Ok(()));
            return;
        }
        let store = match self.binding.store() {
            Ok(s) => s,
            Err(e) => {
                done(Err(e));
                return;
            }
        };
        self.saving.set(true);
        self.binding.names_saving.set(true);
        let p = self.clone();
        self.widget.set_sensitive(false);
        self.binding.refresh();
        store.save_checked(
            request,
            Box::new(move |result| {
                p.saving.set(false);
                p.binding.names_saving.set(false);
                p.widget.set_sensitive(true);
                let result = result.and_then(|saved| {
                    if p.binding.runtime.playback().is_some() {
                        *p.binding.pending_snapshot.borrow_mut() = Some(saved.clone());
                    } else {
                        p.binding.runtime.load(saved.clone())?;
                    }
                    let settings = NativeSequenceSettings::from_project_settings(&saved.payload)?;
                    *p.confirmed.borrow_mut() = (1..=8)
                        .map(|slot| {
                            settings
                                .sequence(slot)
                                .map(|s| s.name.clone())
                                .unwrap_or_default()
                        })
                        .collect();
                    p.update_dirty();
                    p.refresh();
                    Ok(())
                });
                if result.is_ok() && p.binding.runtime.playback().is_none() {
                    p.binding.reconcile_pages();
                }
                p.binding.refresh();
                if let Err(e) = &result {
                    p.controls
                        .status
                        .set_text(&format!("Names not saved; edits retained. {e}"));
                }
                done(result);
            }),
        );
    }
}
fn page_target(
    handle: &Rc<RefCell<Weak<SequencePage>>>,
    binding: &PaneBinding,
) -> Result<Rc<Workspace>, String> {
    let page = handle
        .borrow()
        .upgrade()
        .ok_or("The sequence page closed.")?;
    let owner = page.owner()?;
    let (pane_owner, _) = binding.recording_owner()?;
    if !Rc::ptr_eq(&owner, &pane_owner)
        || owner.current_widget() != Some(page.widget.clone().upcast())
    {
        return Err("This sequence page and its captured draft must be in the same current window. Open sequences from the intended draft again.".into());
    }
    Ok(owner)
}

fn binding_actions(
    binding: &Rc<PaneBinding>,
    record: Rc<dyn Fn(u8, String)>,
    play: Rc<dyn Fn(u8)>,
) -> Rc<SequenceUiActions> {
    let weak = Rc::downgrade(binding);
    let remove = Rc::new(move |slot| {
        if let Some(b) = weak.upgrade() {
            if b.names_saving.get() || b.loading.get() {
                b.runtime
                    .report_error("Wait for sequence settings to finish first.");
                return;
            }
            match b.store() {
                Ok(store) => {
                    let out = b.clone();
                    b.runtime.remove(
                        slot,
                        store,
                        Box::new(move |r| {
                            if let Err(e) = r {
                                out.runtime.report_error(&e);
                            }
                            out.reconcile_pages();
                        }),
                    );
                }
                Err(e) => b.runtime.report_error(&e),
            }
        }
    });
    let weak = Rc::downgrade(binding);
    let save_recording = Rc::new(move || {
        if let Some(b) = weak.upgrade() {
            let out = b.clone();
            b.save_recording(Box::new(move |r| {
                if let Err(e) = r {
                    out.runtime.report_error(&e);
                }
            }));
        }
    });
    let weak = Rc::downgrade(binding);
    let cancel_recording = Rc::new(move || {
        if let Some(b) = weak.upgrade() {
            match b.runtime.cancel_recording() {
                Ok(()) => {
                    b.reconcile_pages();
                    b.refresh();
                }
                Err(e) => b.runtime.report_error(&e),
            }
        }
    });
    Rc::new(SequenceUiActions {
        record,
        play,
        remove,
        save_recording,
        cancel_recording,
    })
}

/// Keep the asynchronous load/refusal and playback-start completion contract in
/// one place. Refused loading/context/start never runs a command or saves a step.
fn continue_saved_play<T>(
    context: Result<T, String>,
    done: Box<dyn FnOnce(PlaybackOutcome)>,
    start: impl FnOnce(T, Box<dyn FnOnce(PlaybackOutcome)>) -> Result<(), String>,
) -> Result<(), String> {
    let completion = Rc::new(RefCell::new(Some(done)));
    let out = completion.clone();
    let result = context.and_then(|context| {
        start(
            context,
            Box::new(move |outcome| {
                let done = out.borrow_mut().take();
                if let Some(done) = done {
                    done(outcome);
                }
            }),
        )
    });
    if let Err(error) = &result {
        let done = completion.borrow_mut().take();
        if let Some(done) = done {
            done(PlaybackOutcome {
                accepted_steps: 0,
                saved_steps: 0,
                error: Some(error.clone()),
            });
        }
    }
    result
}

impl Workspace {
    /// Root calls after mounting/registering each pane. Also called lazily by input
    /// hooks. Returning editors reuse their exact session, gate and recording.
    pub(super) fn sequence_bind_pane(self: &Rc<Self>, pane: &Rc<EditorPane>) {
        if let Some(binding) = binding_for(pane) {
            if !self
                .sequence_host
                .panes
                .borrow()
                .iter()
                .any(|b| Rc::ptr_eq(b, &binding))
            {
                self.sequence_host.panes.borrow_mut().push(binding);
            }
            return;
        }
        if self.pid.borrow().is_empty() {
            return;
        }
        let binding = Rc::new(PaneBinding {
            pane: Rc::downgrade(pane),
            runtime: SequenceRuntime::new(self.pid.borrow().clone()),
            target: SequenceTarget {
                project_id: self.pid.borrow().clone(),
                scene_id: pane.scene_id.clone(),
                editor_id: uuid::Uuid::new_v4().to_string(),
            },
            gate: Default::default(),
            lease: RefCell::new(None),
            snapshot: RefCell::new(None),
            pending_snapshot: RefCell::new(None),
            loading: Cell::new(false),
            load_epoch: Cell::new(0),
            names_saving: Cell::new(false),
            banner: RefCell::new(None),
            pages: RefCell::new(vec![]),
            #[cfg(test)]
            after_saved_step: RefCell::new(None),
        });
        let weak = Rc::downgrade(&binding);
        *binding.runtime.on_change.borrow_mut() = Some(Box::new(move || {
            if let Some(b) = weak.upgrade() {
                b.refresh();
            }
        }));
        let actions = binding_actions(&binding, Rc::new(|_, _| {}), Rc::new(|_| {}));
        let banner = RecordingBanner::new(binding.runtime.clone(), actions);
        pane.widget.pack_start(&banner.widget, false, false, 0);
        pane.widget.reorder_child(&banner.widget, 0);
        *binding.banner.borrow_mut() = Some(banner);
        BINDINGS.with(|all| all.borrow_mut().push(Rc::downgrade(&binding)));
        self.sequence_host.panes.borrow_mut().push(binding);
    }
    pub(super) fn command_sequences(self: &Rc<Self>) {
        let owner = desk::effective(self);
        let pane = owner
            .active
            .borrow()
            .clone()
            .filter(|p| owner.current_widget() == Some(p.widget.clone().upcast()));
        let Some(pane) = pane else {
            owner
                .error("Show and focus the intended scene draft before opening Command sequences.");
            return;
        };
        if owner.sequence_window_blocked() || pane.composing.get() {
            owner.error("Finish the current input or playback first.");
            return;
        }
        owner.sequence_bind_pane(&pane);
        let binding = binding_for(&pane).unwrap();
        let a = owner.clone();
        owner.navigate(move || {
            let page = SequencePage::new(binding.clone());
            a.sequence_host.pages.borrow_mut().push(page.clone());
            a.install(&page.widget, "Command sequences", false);
            if !binding.runtime.loaded() {
                binding.load(Box::new(|_| {}));
            }
        });
    }
    pub(super) fn sequence_play(
        self: &Rc<Self>,
        slot: u8,
        pane: &Rc<EditorPane>,
        done: Box<dyn FnOnce(PlaybackOutcome)>,
    ) -> Result<(), String> {
        self.sequence_bind_pane(pane);
        let binding = binding_for(pane).ok_or("The captured editor has no sequence host.")?;
        let (owner, _) = binding.visible()?;
        if !Rc::ptr_eq(&owner, self) {
            return Err("Play belongs to the captured draft's current window.".into());
        }
        let executor = Rc::new(HostExecutor {
            binding: binding.clone(),
        });
        let out = binding.clone();
        binding.runtime.play(
            slot,
            executor,
            Box::new(move |outcome| {
                out.finish_pending_settings();
                done(outcome)
            }),
        )
    }
    /// Explicit shortcut Play on a fresh binding. Loading settings is read-only;
    /// input/draft decisions begin only after the captured visible context survives
    /// the reply. Synchronous refusals return Err; asynchronous refusals call done
    /// once with zero accepted/saved steps.
    pub(super) fn sequence_play_saved(
        self: &Rc<Self>,
        slot: u8,
        pane: &Rc<EditorPane>,
        done: Box<dyn FnOnce(PlaybackOutcome)>,
    ) -> Result<(), String> {
        if !(1..=8).contains(&slot) {
            return Err("Choose a sequence slot from 1 to 8.".into());
        }
        let original_input = input_permit(pane)
            .ok_or("The captured draft's input is not ready for explicit playback.")?;
        self.sequence_bind_pane(pane);
        let binding = binding_for(pane).ok_or("The captured editor has no sequence host.")?;
        let (owner, _) = binding.visible()?;
        if !Rc::ptr_eq(&owner, self) {
            return Err("Play belongs to the captured draft's current window.".into());
        }
        if binding.runtime.loaded() {
            return self.sequence_play(slot, pane, done);
        }
        if binding.runtime.busy()
            || binding.names_saving.get()
            || binding.loading.get()
            || self.sequence_window_blocked()
            || self.desk.busy.get()
        {
            return Err("Finish the current window or sequence operation before playing.".into());
        }
        let original = Rc::downgrade(self);
        let generation = self.desk.generation.get();
        let captured = pane.clone();
        let b = binding.clone();
        binding.load(Box::new(move |loaded| {
            let context = loaded.and_then(|_| {
                if !input_allowed(&captured, &original_input) {
                    return Err(
                        "The original input permit changed while sequences were loading.".into(),
                    );
                }
                let owner = original
                    .upgrade()
                    .ok_or("The shortcut's captured window closed.")?;
                let (current, pane) = b.visible()?;
                if !Rc::ptr_eq(&owner, &current)
                    || !Rc::ptr_eq(&pane, &captured)
                    || owner.desk.generation.get() != generation
                {
                    return Err(
                        "The captured visible draft changed while sequences were loading.".into(),
                    );
                }
                Ok(owner)
            });
            if let Err(error) = continue_saved_play(context, done, |owner, done| {
                owner.sequence_play(slot, &captured, done)
            }) {
                b.runtime.report_error(&error);
            }
        }));
        Ok(())
    }
    pub(super) fn sequence_window_blocked(&self) -> bool {
        self.sequence_host.playback.borrow().is_some()
    }
    pub(super) fn sequence_page(&self, widget: &gtk::Widget) -> Option<Rc<SequencePage>> {
        self.sequence_host
            .pages
            .borrow()
            .iter()
            .find(|p| p.widget.upcast_ref::<gtk::Widget>() == widget)
            .cloned()
    }
    /// Root calls from transfer_ownership after moving ordinary pane/page ownership.
    pub(super) fn sequence_transfer(self: &Rc<Self>, widget: &gtk::Widget, child: &Rc<Self>) {
        let moved = self
            .sequence_host
            .pages
            .borrow()
            .iter()
            .find(|p| p.widget.upcast_ref::<gtk::Widget>() == widget)
            .cloned();
        if let Some(page) = moved {
            self.sequence_host
                .pages
                .borrow_mut()
                .retain(|p| !Rc::ptr_eq(p, &page));
            child.sequence_host.pages.borrow_mut().push(page);
        }
        let panes = self
            .sequence_host
            .panes
            .borrow()
            .iter()
            .filter(|b| {
                b.pane
                    .upgrade()
                    .is_some_and(|p| p.widget.upcast_ref::<gtk::Widget>() == widget)
            })
            .cloned()
            .collect::<Vec<_>>();
        for binding in panes {
            if let Some(pane) = binding.pane.upgrade() {
                let _ = invalidate_input(&pane);
            }
            self.sequence_host
                .panes
                .borrow_mut()
                .retain(|b| !Rc::ptr_eq(b, &binding));
            if !child
                .sequence_host
                .panes
                .borrow()
                .iter()
                .any(|b| Rc::ptr_eq(b, &binding))
            {
                child.sequence_host.panes.borrow_mut().push(binding.clone());
            }
            binding.refresh();
        }
    }
    pub(super) fn sequence_release_view(&self, widget: &gtk::Widget) {
        self.sequence_host
            .pages
            .borrow_mut()
            .retain(|p| p.widget.upcast_ref::<gtk::Widget>() != widget);
    }
    /// Close/project-switch/replacing the recording pane must call this BEFORE
    /// removing widgets/stopping workers. Switching retained tabs can keep recording.
    /// Root follows acceptance with its ordinary manuscript/form Draft confirmation.
    pub(super) fn sequence_confirm_leave(
        self: &Rc<Self>,
        widget: Option<gtk::Widget>,
        done: impl FnOnce(bool) + 'static,
    ) {
        if self.sequence_window_blocked() {
            done(false);
            return;
        }
        let bindings = self
            .sequence_host
            .panes
            .borrow()
            .iter()
            .filter(|b| {
                b.recording_owner()
                    .is_ok_and(|(owner, _)| Rc::ptr_eq(&owner, self))
                    && widget.as_ref().is_none_or(|w| {
                        b.pane
                            .upgrade()
                            .is_some_and(|p| p.widget.upcast_ref::<gtk::Widget>() == w)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        self.sequence_confirm_next(bindings, 0, Box::new(done));
    }
    fn sequence_confirm_next(
        self: &Rc<Self>,
        bindings: Vec<Rc<PaneBinding>>,
        index: usize,
        done: Box<dyn FnOnce(bool)>,
    ) {
        if index == bindings.len() {
            done(true);
            return;
        }
        let binding = bindings[index].clone();
        if !binding
            .recording_owner()
            .is_ok_and(|(owner, _)| Rc::ptr_eq(&owner, self))
        {
            self.error("The recording's captured owner changed; this window stays open.");
            done(false);
            return;
        }
        if binding.loading.get() || binding.names_saving.get() || binding.runtime.saving() {
            self.error("Wait for sequence settings to finish; this window stays open.");
            done(false);
            return;
        }
        if binding.runtime.recording().is_none() {
            self.sequence_confirm_next(bindings, index + 1, done);
            return;
        }
        if binding.pane.upgrade().is_some_and(|p| p.composing.get()) {
            self.error("Finish composition before deciding about the recording.");
            done(false);
            return;
        }
        let choice = recording_choice(self);
        match choice {
            RecordingChoice::Cancel => done(false),
            RecordingChoice::Discard => {
                if let Err(e) = binding.runtime.cancel_recording() {
                    self.error(&e);
                    done(false);
                    return;
                }
                binding.reconcile_pages();
                self.sequence_confirm_next(bindings, index + 1, done);
            }
            RecordingChoice::Save => {
                let a = self.clone();
                binding.save_recording(Box::new(move |result| match result {
                    Ok(()) => a.sequence_confirm_next(bindings, index + 1, done),
                    Err(e) => {
                        a.error(&e);
                        done(false);
                    }
                }));
            }
        }
    }
    /// Call after accepted whole-window/project departure. Never silently clears a
    /// recording if root omitted the decision hook; returns Err instead.
    pub(super) fn sequence_clear(&self) -> Result<(), String> {
        if self.sequence_window_blocked()
            || self
                .sequence_host
                .panes
                .borrow()
                .iter()
                .any(|b| b.runtime.busy() || b.names_saving.get() || b.loading.get())
        {
            return Err(
                "Sequence work is still retained; confirm it before closing or changing novels."
                    .into(),
            );
        }
        if self
            .sequence_host
            .pages
            .borrow()
            .iter()
            .any(|p| p.dirty.get() || p.saving.get() || p.composing.get())
        {
            return Err("Sequence name drafts remain; Save, Discard or Cancel first.".into());
        }
        self.sequence_host.pages.borrow_mut().clear();
        self.sequence_host.panes.borrow_mut().clear();
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecordingChoice {
    Save,
    Discard,
    Cancel,
}
fn recording_choice(owner: &Rc<Workspace>) -> RecordingChoice {
    #[cfg(test)]
    if let Some(choice) = TEST_RECORDING_CHOICES.with(|q| q.borrow_mut().pop_front()) {
        return choice;
    }
    let dialog = gtk::Dialog::with_buttons(
        Some("Unsaved command recording"),
        Some(&owner.window),
        gtk::DialogFlags::MODAL,
        &[
            ("Cancel", gtk::ResponseType::Cancel),
            ("Discard recording", gtk::ResponseType::Reject),
            ("Save sequence", gtk::ResponseType::Accept),
        ],
    );
    let label=gtk::Label::new(Some("Save this formatting sequence before leaving? Manuscript edits stay in the draft and have their own manual Save / Discard / Cancel decision."));
    label.set_line_wrap(true);
    label.set_margin_start(18);
    label.set_margin_end(18);
    label.set_margin_top(18);
    label.set_margin_bottom(18);
    dialog.content_area().pack_start(&label, false, false, 0);
    dialog.set_default_response(gtk::ResponseType::Cancel);
    dialog.set_accept_focus(owner.window.accepts_focus());
    dialog.set_focus_on_map(owner.window.accepts_focus());
    dialog.show_all();
    let response = dialog.run();
    dialog.close();
    match response {
        gtk::ResponseType::Accept => RecordingChoice::Save,
        gtk::ResponseType::Reject => RecordingChoice::Discard,
        _ => RecordingChoice::Cancel,
    }
}
#[cfg(test)]
thread_local! {static TEST_RECORDING_CHOICES:RefCell<std::collections::VecDeque<RecordingChoice>>=const{RefCell::new(std::collections::VecDeque::new())};}
#[cfg(test)]
#[path = "sequence_host_tests.rs"]
pub(crate) mod tests;
