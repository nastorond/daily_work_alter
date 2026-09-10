use crate::data::log;
use crate::scheduler::ViewMode;
use crate::AppData;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const WIN_WIDTH: f64 = 620.0;
const WIN_HEIGHT: f64 = 560.0;
const MAIN_LABEL: &str = "main";

/// Tracks the window across the gap between "someone decided to open one" and
/// "one exists".
///
/// `Building` and `Showing` have to be distinguished. Collapsing them into a
/// single "claimed" flag is what broke the reminder before: a claim left behind
/// by a window that vanished without going through `destroy_window` (a native
/// close, a webview crash) made every later request believe someone else was
/// already handling it, so the scheduler's pop was swallowed and nothing ever
/// appeared again until restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    Closed,
    Building(ViewMode),
    Showing(ViewMode),
    /// First-run onboarding is up, or about to be. Claimed before the scheduler
    /// thread starts so its first tick can't race in and replace the onboarding
    /// form with the daily box — which would ask someone to write their log
    /// before they had set a single option.
    Onboarding,
}

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

enum Plan {
    /// Another caller is mid-build; it will present the window.
    Skip,
    /// This exact view is already up.
    Focus,
    Build,
}

/// What `show_window` actually managed to do.
///
/// `Building` has to be distinguished from `Failed`: when two callers race (the
/// scheduler tick landing at the same moment as a manual launch), the loser used
/// to be told "no window appeared" and logged a failure for a window that showed
/// up a moment later. The scheduler needs to tell "it didn't work" apart from
/// "someone else is doing it".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowResult {
    Shown,
    Building,
    Failed,
}

/// Opens the given view, reusing the window when it is already showing exactly
/// that view. Returns whether a window is up as a result — callers that must
/// know the reminder was actually delivered depend on this.
///
/// Rebuilding is the normal path and is deliberate: the window is destroyed
/// rather than hidden when it closes, so the WebView2 process is actually torn
/// down. That is what keeps idle RAM around 20MB for the ~23.5h/day this app
/// spends doing nothing, and it is also why the schedule timer lives in Rust —
/// with no window there is no JS to run it.
///
/// But rebuilding a window that is already up and being typed into loses work:
/// autosave runs on a 500ms debounce and a Rust-side destroy() gives the page no
/// chance to flush. So an identical view is reused. Opening settings always
/// rebuilds, because the modal is driven by the query string rather than by a
/// message to the live page.
///
/// The lock is released before any windowing call: build() from a background
/// thread dispatches to the main thread, which would deadlock if the main thread
/// were itself waiting on this lock.
pub fn show_window(app: &AppHandle, mode: ViewMode, open_settings: bool) -> ShowResult {
    let Some(data) = app.try_state::<AppData>() else {
        return ShowResult::Failed;
    };

    let plan = {
        let mut st = data.window_state.lock().unwrap();
        match *st {
            WindowState::Building(_) => Plan::Skip,
            // The window-exists check is what makes a stale Showing self-heal:
            // if the window is gone, this falls through and rebuilds.
            WindowState::Showing(m)
                if !open_settings
                    && m == mode
                    && app.get_webview_window(MAIN_LABEL).is_some() =>
            {
                Plan::Focus
            }
            _ => {
                *st = WindowState::Building(mode);
                Plan::Build
            }
        }
    };

    match plan {
        Plan::Skip => return ShowResult::Building,
        Plan::Focus => {
            if let Some(win) = app.get_webview_window(MAIN_LABEL) {
                bring_forward(&win);
                return ShowResult::Shown;
            }
            return ShowResult::Failed;
        }
        Plan::Build => {}
    }

    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.destroy();
    }

    let mut url = format!("index.html?view={}", route_for(mode));
    if open_settings {
        url.push_str("&settings=1");
    }

    match build_window(app, url) {
        Ok(win) => {
            *data.window_state.lock().unwrap() = WindowState::Showing(mode);
            *data.window_opened_at.lock().unwrap() = Some(chrono::Local::now());
            present(&win);
            ShowResult::Shown
        }
        Err(e) => {
            *data.window_state.lock().unwrap() = WindowState::Closed;
            log::line(&format!("창 생성 실패 ({mode:?}): {e}"));
            ShowResult::Failed
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
    claim_onboarding(app);

    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.destroy();
    }

    match build_window(app, "index.html?view=onboarding".to_string()) {
        Ok(win) => {
            if let Some(data) = app.try_state::<AppData>() {
                *data.window_opened_at.lock().unwrap() = Some(chrono::Local::now());
            }
            present(&win);
        }
        Err(e) => log::line(&format!("온보딩 창 생성 실패: {e}")),
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

/// Marks onboarding as owning the window. Called both by `show_onboarding` and
/// by startup before the scheduler thread exists, so there is no gap to race in.
pub fn claim_onboarding(app: &AppHandle) {
    if let Some(data) = app.try_state::<AppData>() {
        *data.window_state.lock().unwrap() = WindowState::Onboarding;
    }
}

/// Whether onboarding currently owns the window.
pub fn is_onboarding(app: &AppHandle) -> bool {
    app.try_state::<AppData>()
        .is_some_and(|d| *d.window_state.lock().unwrap() == WindowState::Onboarding)
}

/// Whether the main window currently exists.
pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN_LABEL).is_some()
}

/// When the live window was opened, if one is up.
pub fn opened_at(app: &AppHandle) -> Option<chrono::DateTime<chrono::Local>> {
    let data = app.try_state::<AppData>()?;
    let at = *data.window_opened_at.lock().unwrap();
    at
}

pub fn destroy_window(app: &AppHandle) {
    if let Some(data) = app.try_state::<AppData>() {
        *data.window_state.lock().unwrap() = WindowState::Closed;
        *data.window_opened_at.lock().unwrap() = None;
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
