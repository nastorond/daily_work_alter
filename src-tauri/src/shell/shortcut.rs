use std::str::FromStr;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

pub fn register(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let shortcut = Shortcut::from_str(hotkey).map_err(|e| format!("단축키 형식 오류: {e}"))?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| format!("단축키 등록 실패(다른 앱과 충돌?): {e}"))
}

pub fn unregister_all(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
}

/// Swaps the currently registered global shortcut for a new one. On failure the
/// old shortcut stays unregistered — caller surfaces the error to the settings UI.
pub fn reregister(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    unregister_all(app);
    register(app, hotkey)
}
