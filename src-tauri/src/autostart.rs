use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// Registers / unregisters the login-time autostart entry.
///
/// Two deliberate deviations from the obvious implementation:
///
/// 1. **Debug builds never touch it.** `npm run dev` would otherwise repoint the
///    login entry at `target\debug\`, so the deployed exe would silently stop
///    being the one that launches at login.
/// 2. **When enabled we always re-register**, rather than skipping when
///    `is_enabled()` is already true. The plugin's `is_enabled` only checks that
///    the registry key exists — not that it points at *this* exe — so moving or
///    replacing the executable would otherwise leave a stale path behind.
pub fn apply(app: &AppHandle, enabled: bool) {
    if cfg!(debug_assertions) {
        return;
    }

    let mgr = app.autolaunch();
    let _ = mgr.disable();
    if enabled {
        let _ = mgr.enable();
    }
}
