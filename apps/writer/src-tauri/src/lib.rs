mod editor;
mod companion;
mod files;
mod windows;
mod project_exports;
use novel_storage::{Scene, Snapshot};
use tauri::Manager;
#[tauri::command]
fn enable_close_guard(window: tauri::Window) -> Result<(), String> { windows::enable_guard(&window) }
#[tauri::command]
async fn complete_close(window: tauri::Window) -> Result<(), String> {
    if close_probe_enabled() {
        if !windows::close_requested(&window) { return Err("The close test did not intercept a close request.".into()); }
        if let Some(directory) = probe_directory() {
            std::fs::write(directory.join("closed-cleanly.json"), b"{\"closeIntercepted\":true,\"saveDrained\":true}").map_err(|_| "Could not record the close test.")?;
        }
    }
    windows::complete_window_close(window).await
}
#[tauri::command]
fn close_probe_enabled() -> bool { probe_directory().is_some() && std::env::var_os("NOVEL_CLOSE_PROBE").is_some() }
#[tauri::command]
fn request_probe_close(window: tauri::Window) -> Result<(), String> {
    if !close_probe_enabled() || !windows::guard_enabled(&window) { return Err("The isolated close probe is not ready.".into()); }
    window.close().map_err(|error| error.to_string())
}

#[tauri::command]
async fn load_scene(storage: tauri::State<'_, companion::CompanionState>) -> Result<Option<Snapshot>, String> {
    serde_json::from_value(storage.0.request("load",serde_json::json!({})).await?).map_err(|_|"Could not read the local scene.".into())
}
#[tauri::command]
async fn save_scene(storage: tauri::State<'_, companion::CompanionState>, scene: Scene, parent: Option<String>) -> Result<Snapshot, String> {
    serde_json::from_value(storage.0.request("save",serde_json::json!({"scene":scene,"parent":parent})).await?).map_err(|_|"Could not acknowledge the local save.".into())
}
fn probe_directory() -> Option<std::path::PathBuf> {
    if cfg!(debug_assertions) { std::env::var_os("NOVEL_PERFORMANCE_PROBE").map(std::path::PathBuf::from) } else { None }
}
#[tauri::command]
fn workspace_probe_enabled() -> bool {probe_directory().is_some() && std::env::var_os("NOVEL_WORKSPACE_PROBE").is_some()}
#[tauri::command]
fn resize_workspace_probe(window: tauri::Window, width: u32) -> Result<(), String> {
    if !workspace_probe_enabled() || !matches!(width, 760 | 900 | 1280) { return Err("The isolated layout probe is not enabled.".into()); }
    window.set_size(tauri::LogicalSize::new(width, 860)).map_err(|error| error.to_string())
}
#[tauri::command]
fn long_writing_probe_enabled() -> bool {probe_directory().is_some() && std::env::var_os("NOVEL_LONG_WRITING_PROBE").is_some()}
#[tauri::command]
fn resize_probe_enabled() -> bool {probe_directory().is_some() && std::env::var_os("NOVEL_RESIZE_PROBE").is_some()}
#[tauri::command]
fn change_editor_probe_window(window: tauri::Window, mode: String) -> Result<(), String> {
    if !resize_probe_enabled() { return Err("The isolated resize probe is not enabled.".into()); }
    match mode.as_str() {
        "fullscreen" => window.set_fullscreen(true),
        "maximized" => {window.set_fullscreen(false).map_err(|error|error.to_string())?;window.maximize()},
        "760" | "900" | "1280" => {
            window.set_fullscreen(false).map_err(|error|error.to_string())?;
            window.unmaximize().map_err(|error|error.to_string())?;
            let width=mode.parse::<u32>().map_err(|_|"Invalid probe width.")?;
            window.set_size(tauri::LogicalSize::new(width,860))
        },
        _ => return Err("Unsupported isolated resize mode.".into())
    }.map_err(|error|error.to_string())
}
#[tauri::command]
fn performance_probe_enabled() -> bool { probe_directory().is_some() }
#[tauri::command]
fn finish_performance_probe(app: tauri::AppHandle, report: serde_json::Value) -> Result<(), String> {
    let directory = probe_directory().ok_or("Performance probe is not enabled.")?;
    let data = serde_json::to_vec_pretty(&report).map_err(|_| "Could not encode probe report.")?;
    if data.len() > 32_000 { return Err("Probe report is too large.".into()); }
    std::fs::write(directory.join("performance.json"), data).map_err(|_| "Could not save probe report.")?;
    app.exit(0); Ok(())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default().manage(windows::WindowManager::default()).manage(project_exports::ProjectExports::default()).on_window_event(windows::on_window_event).setup(|app| {
        // The bundled page uses clipboard access only for explicit editor actions.
        let main_config=app.config().app.windows.iter().find(|config|config.label=="main").ok_or("Missing main window configuration.")?;
        tauri::WebviewWindowBuilder::from_config(app,main_config)?.enable_clipboard_access().build()?;
        let directory = if let Some(directory) = probe_directory() {
            if let Some(window) = app.get_webview_window("main") { window.set_title("Novel Workspace — Synthetic performance test")?; }
            directory
        } else { app.path().app_data_dir()? };
        std::fs::create_dir_all(&directory)?;
        let storage = companion::Companion::open(&directory, &app.path().resource_dir()?)?;
        app.manage(storage); app.manage(editor::EditorState::default());
        windows::initialize(app.handle()).map_err(std::io::Error::other)?; Ok(())
    }).invoke_handler(tauri::generate_handler![load_scene, save_scene, companion::workspace_command, files::export_document, files::export_docx, files::import_document, project_exports::workspace_export_save_ready, editor::editor_open, editor::editor_edit, editor::editor_release, editor::editor_park,
        windows::workspace_window_state, windows::register_tab, windows::unregister_tab, windows::workspace_find_tab, windows::workspace_focus_tab, windows::workspace_notify_change, windows::workspace_toggle_fullscreen, windows::workspace_request_tab_move, windows::workspace_close_empty_window, windows::workspace_transfer_tab, windows::workspace_transfer_ready, windows::workspace_transfer_abort, windows::workspace_cancel_close, windows::workspace_request_quit, performance_probe_enabled, long_writing_probe_enabled, resize_probe_enabled, change_editor_probe_window, workspace_probe_enabled, resize_workspace_probe, finish_performance_probe, enable_close_guard, complete_close, close_probe_enabled, request_probe_close])
      .build(tauri::generate_context!()).expect("Could not start the local editor prototype");
    app.run(|app, event| windows::on_run_event(app, &event));
}
