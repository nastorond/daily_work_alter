use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

pub fn apply(app: &AppHandle, enabled: bool) {
    let mgr = app.autolaunch();
    let is_enabled = mgr.is_enabled().unwrap_or(false);
    if enabled && !is_enabled {
        let _ = mgr.enable();
    } else if !enabled && is_enabled {
        let _ = mgr.disable();
    }
}
