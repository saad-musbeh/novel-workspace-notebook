//! Project-scoped save acknowledgements from every owning desktop window.
use serde_json::json;
use std::{collections::BTreeSet, sync::{Mutex, atomic::{AtomicU64, Ordering}}, time::Duration};
use tauri::{Emitter, Manager};
use tokio::sync::oneshot;

struct Pending {
    id: u64,
    waiting: BTreeSet<String>,
    complete: oneshot::Sender<Result<(), String>>,
}
#[derive(Default)]
pub struct ProjectExports {
    next: AtomicU64,
    pending: Mutex<Option<Pending>>,
}
impl ProjectExports {
    fn begin(&self, waiting: BTreeSet<String>) -> Result<(u64, oneshot::Receiver<Result<(), String>>), String> {
        if waiting.is_empty() { return Err("No manuscript windows are available for export.".into()); }
        let mut pending = self.pending.lock().map_err(|_| "The manuscript save barrier is unavailable.")?;
        if pending.is_some() { return Err("Another Word export is still saving. Try again when it finishes.".into()); }
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (complete, receiver) = oneshot::channel();
        *pending = Some(Pending { id, waiting, complete });
        Ok((id, receiver))
    }
    fn acknowledge(&self, label: &str, id: u64, error: Option<String>) -> Result<(), String> {
        let mut slot = self.pending.lock().map_err(|_| "The manuscript save barrier is unavailable.")?;
        let pending = slot.as_mut().ok_or("The Word export save request is no longer active.")?;
        if pending.id != id || !pending.waiting.remove(label) {
            return Err("This window does not own that Word export acknowledgement.".into());
        }
        if error.is_some() || pending.waiting.is_empty() {
            let done = slot.take().ok_or("The Word export save request is no longer active.")?;
            let result = error.map_or(Ok(()), |message| Err(format!("An open manuscript could not save: {}", message.chars().take(1000).collect::<String>())));
            let _ = done.complete.send(result);
        }
        Ok(())
    }
    fn cancel(&self, id: u64) {
        if let Ok(mut slot) = self.pending.lock() {
            if slot.as_ref().is_some_and(|pending| pending.id == id) { *slot = None; }
        }
    }
}
struct Cleanup<'a> { state: &'a ProjectExports, id: u64 }
impl Drop for Cleanup<'_> { fn drop(&mut self) { self.state.cancel(self.id); } }

pub async fn drain_project(window: &tauri::Window, project_id: &str) -> Result<(), String> {
    let app = window.app_handle();
    let targets = crate::windows::project_export_targets(window, project_id)?;
    let state = app.state::<ProjectExports>();
    let (id, receiver) = state.begin(targets.clone())?;
    let _cleanup = Cleanup { state: &state, id };
    for label in &targets {
        app.emit_to(label.as_str(), "workspace-export-save", json!({"requestId":id,"projectId":project_id})).map_err(|_| "Could not ask every manuscript window to save. Word export stopped.")?;
    }
    tokio::time::timeout(Duration::from_secs(30), receiver).await
        .map_err(|_| "An open manuscript window did not finish saving. Keep its draft open, finish composition or saving, then retry Word export.")?
        .map_err(|_| "Word export saving was interrupted.")??;
    if crate::windows::project_export_targets(window, project_id)? != targets {
        return Err("The project's open windows changed while saving. Retry Word export.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn workspace_export_save_ready(window: tauri::Window, state: tauri::State<'_, ProjectExports>, request_id: u64, error: Option<String>) -> Result<(), String> {
    state.acknowledge(window.label(), request_id, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn windows() -> BTreeSet<String> { ["main".into(), "workspace-2".into()].into_iter().collect() }
    #[test]
    fn export_waits_for_every_window_and_refuses_foreign_or_duplicate_acknowledgements() {
        let state = ProjectExports::default();
        let (id, mut receiver) = state.begin(windows()).unwrap();
        assert!(state.acknowledge("foreign", id, None).is_err());
        assert!(state.acknowledge("main", id + 1, None).is_err());
        state.acknowledge("main", id, None).unwrap();
        assert!(receiver.try_recv().is_err());
        assert!(state.acknowledge("main", id, None).is_err());
        state.acknowledge("workspace-2", id, None).unwrap();
        assert_eq!(receiver.try_recv().unwrap(), Ok(()));
    }
    #[test]
    fn failed_save_refuses_export_and_does_not_leave_a_stuck_barrier() {
        let state = ProjectExports::default();
        let (id, mut receiver) = state.begin(windows()).unwrap();
        assert!(state.begin(windows()).is_err());
        state.acknowledge("workspace-2", id, Some("Save failed".into())).unwrap();
        assert!(receiver.try_recv().unwrap().unwrap_err().contains("Save failed"));
        assert!(state.begin(windows()).is_ok());
    }
    #[test]
    fn cancelled_request_cannot_acknowledge_a_new_export() {
        let state = ProjectExports::default();
        let (old, _) = state.begin(windows()).unwrap();
        state.cancel(old);
        let (new, _) = state.begin(windows()).unwrap();
        assert_ne!(new, old);
        assert!(state.acknowledge("main", old, None).is_err());
        state.cancel(old);
        assert!(state.begin(windows()).is_err());
        state.cancel(new);
        assert!(state.begin(windows()).is_ok());
    }
}
