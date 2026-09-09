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

fn build_window(app: &AppHandle, url: String) -> tauri::Result<tauri::WebviewWindow> {
    WebviewWindowBuilder::new(app, MAIN_LABEL, WebviewUrl::App(url.into()))
        .title("DailyWorkAlter")
        .inner_size(WIN_WIDTH, WIN_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
}

fn present(win: &tauri::WebviewWindow) {
    position_bottom_right(win);
    let _ = win.show();
    let _ = win.set_focus();
}

fn bring_forward(win: &tauri::WebviewWindow) {
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
}

/// Opens the given view, rebuilding the window unless it is already showing
/// exactly that view.
///
/// Rebuilding is the normal path and is deliberate: the window is destroyed
/// rather than hidden when it closes, so the WebView2 process is actually torn
/// down. That is what keeps idle RAM around 20MB for the ~23.5h/day this app
/// spends doing nothing, and it is also why the schedule timer lives in Rust —
/// with no window there is no JS to run it.
///
/// But rebuilding a window that is already up and being typed into loses work:
/// autosave runs on a 500ms debounce and a Rust-side destroy() gives the page no
/// chance to flush. So an identical view is reused instead. Opening settings
/// always rebuilds, because the modal is driven by the query string rather than
/// by a message to the live page.
///
/// Concurrency: the lock is used to *claim* the view, and released before any
/// windowing call. Claiming before building is what stops the startup race —
/// the scheduler's first tick and the manual-launch path can arrive here at the
/// same moment, and a concurrent caller asking for the same view now sees the
/// claim and backs off instead of building a rival window that destroys the
/// first. The lock is deliberately not held across window creation: build() from
/// a background thread dispatches to the main thread, which would deadlock if
/// the main thread were itself waiting on this lock.
pub fn show_window(app: &AppHandle, mode: ViewMode, open_settings: bool) {
    let Some(data) = app.try_state::<AppData>() else {
        return;
    };

    let already_claimed = {
        let mut current = data.current_view.lock().unwrap();
        if !open_settings && *current == Some(mode) {
            true
        } else {
            *current = Some(mode);
            false
        }
    };

    if already_claimed {
        if let Some(win) = app.get_webview_window(MAIN_LABEL) {
            bring_forward(&win);
        }
        // No window yet means another caller is mid-build; it will present it.
        return;
    }

    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.destroy();
    }

    let mut url = format!("index.html?view={}", route_for(mode));
    if open_settings {
        url.push_str("&settings=1");
    }

    match build_window(app, url) {
        Ok(win) => present(&win),
        Err(e) => {
            eprintln!("failed to create window: {e}");
            *data.current_view.lock().unwrap() = None;
        }
    }
}

/// Used by the tray "지금 작성하기" — opens whatever view is due right now
/// (daily vs weekly) without touching notify state.
pub fn open_window_default(app: &AppHandle) {
    let mode = due_mode(app);
    show_window(app, mode, false);
}

pub fn show_settings(app: &AppHandle) {
    let mode = due_mode(app);
    show_window(app, mode, true);
}

fn due_mode(app: &AppHandle) -> ViewMode {
    let data = app.state::<AppData>();
    let cfg = data.config.lock().unwrap().clone();
    crate::scheduler::view_mode_for(&cfg, chrono::Local::now())
}

/// First-run only: a trimmed-down settings form asking just for work hours and
/// the weekly-summary day. Also reachable from the debug-only tray test menu.
pub fn show_onboarding(app: &AppHandle) {
    // Onboarding is neither daily nor weekly, so leaving the claim as None means
    // a later request for a real view rebuilds rather than reusing this window.
    if let Some(data) = app.try_state::<AppData>() {
        *data.current_view.lock().unwrap() = None;
    }

    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.destroy();
    }

    match build_window(app, "index.html?view=onboarding".to_string()) {
        Ok(win) => present(&win),
        Err(e) => eprintln!("failed to create onboarding window: {e}"),
    }
}

/// Brings an already-open window forward, opening the due view if there is none.
///
/// Used by the relaunch and global-shortcut paths, where the user is asking to
/// get to the window rather than for a particular view — so whatever is already
/// up is what they want, even if it is not the view that is due now.
pub fn focus_or_open(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        bring_forward(&win);
        return;
    }
    open_window_default(app);
}

pub fn destroy_window(app: &AppHandle) {
    if let Some(data) = app.try_state::<AppData>() {
        *data.current_view.lock().unwrap() = None;
    }
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
