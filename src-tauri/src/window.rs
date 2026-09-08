use crate::scheduler::ViewMode;
use crate::AppData;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const WIN_WIDTH: f64 = 620.0;
const WIN_HEIGHT: f64 = 560.0;
const MAIN_LABEL: &str = "main";

fn route_for(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Daily => "daily",
        ViewMode::Weekly => "weekly",
    }
}

/// Destroys any existing main window and creates a fresh one. We destroy rather
/// than hide/reuse so the WebView2 process is actually torn down between pops.
/// That is what keeps idle RAM around 20MB: for the ~23.5h/day this app spends
/// doing nothing, only the Rust process is resident. It is also why the schedule
/// timer lives in Rust — with no window there is no JS to run it.
pub fn show_window(app: &AppHandle, mode: ViewMode, open_settings: bool) {
    destroy_window(app);

    let mut url = format!("index.html?view={}", route_for(mode));
    if open_settings {
        url.push_str("&settings=1");
    }

    let win = WebviewWindowBuilder::new(app, MAIN_LABEL, WebviewUrl::App(url.into()))
        .title("DailyWorkAlter")
        .inner_size(WIN_WIDTH, WIN_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build();

    match win {
        Ok(win) => {
            position_bottom_right(&win);
            let _ = win.show();
            let _ = win.set_focus();
        }
        Err(e) => {
            eprintln!("failed to create window: {e}");
        }
    }
}

/// Used by the tray "지금 작성하기" / global shortcut — opens whatever view
/// would currently be shown (daily vs weekly) without touching notify state.
pub fn open_window_default(app: &AppHandle) {
    let data = app.state::<AppData>();
    let cfg = data.config.lock().unwrap().clone();
    let mode = crate::scheduler::view_mode_for(&cfg, chrono::Local::now());
    show_window(app, mode, false);
}

pub fn show_settings(app: &AppHandle) {
    let data = app.state::<AppData>();
    let cfg = data.config.lock().unwrap().clone();
    let mode = crate::scheduler::view_mode_for(&cfg, chrono::Local::now());
    show_window(app, mode, true);
}

/// First-run only: a trimmed-down settings form asking just for work hours and
/// the weekly-summary day. Also reachable from the debug-only tray test menu.
pub fn show_onboarding(app: &AppHandle) {
    destroy_window(app);
    let win = WebviewWindowBuilder::new(app, MAIN_LABEL, WebviewUrl::App("index.html?view=onboarding".into()))
        .title("DailyWorkAlter")
        .inner_size(WIN_WIDTH, WIN_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build();

    match win {
        Ok(win) => {
            position_bottom_right(&win);
            let _ = win.show();
            let _ = win.set_focus();
        }
        Err(e) => eprintln!("failed to create onboarding window: {e}"),
    }
}

/// Brings an already-open window forward instead of rebuilding it.
///
/// Every other entry point goes through show_window, which destroys first — that
/// is deliberate for a *new* pop, but wrong when the window is already up and
/// being typed into: the autosave debounce is 500ms and a Rust-side destroy()
/// gives the page no chance to flush, so a rebuild can eat the last keystrokes.
/// Used by the relaunch and global-shortcut paths, where the user is asking to
/// get to the window rather than for a particular view.
pub fn focus_or_open(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    open_window_default(app);
}

pub fn destroy_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.destroy();
    }
}

/// Approximate bottom-right placement, clear of the taskbar. Tauri doesn't expose
/// the taskbar-excluded work area directly, so we subtract a fixed margin instead.
fn position_bottom_right(win: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = win.primary_monitor() else {
        return;
    };
    let size = monitor.size();
    let scale = monitor.scale_factor();
    let margin = (20.0 * scale) as i32;
    let taskbar_guess = (56.0 * scale) as i32;
    let win_w = (WIN_WIDTH * scale) as i32;
    let win_h = (WIN_HEIGHT * scale) as i32;

    let x = size.width as i32 - win_w - margin;
    let y = size.height as i32 - win_h - margin - taskbar_guess;
    let _ = win.set_position(PhysicalPosition::new(x.max(0), y.max(0)));
}
