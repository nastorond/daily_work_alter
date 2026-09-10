use crate::data::config::Config;
use crate::data::storage::{self, DayLog, LogEntry};
use crate::data::storage as storage_mod;
use crate::shell::{shortcut, window};
use crate::{scheduler, AppData};
use chrono::{Duration, Local};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn get_config(state: State<AppData>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_config(app: AppHandle, state: State<AppData>, config: Config) -> Result<(), String> {
    crate::data::config::save(&config).map_err(|e| e.to_string())?;

    shortcut::reregister(&app, &config.hotkey)?;
    crate::shell::autostart::apply(&app, config.autostart);

    *state.config.lock().unwrap() = config;
    Ok(())
}

#[tauri::command]
pub fn open_config_file(app: AppHandle) -> Result<(), String> {
    let path = crate::data::config::config_path();
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_log_dir(app: AppHandle, state: State<AppData>) -> Result<(), String> {
    let cfg = state.config.lock().unwrap().clone();
    let dir = storage_mod::log_dir(&cfg);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_log(state: State<AppData>, date: String) -> LogEntry {
    let cfg = state.config.lock().unwrap().clone();
    storage::read_log(&cfg, &date)
}

#[tauri::command]
pub fn write_log(state: State<AppData>, date: String, items: Vec<String>) -> Result<(), String> {
    let cfg = state.config.lock().unwrap().clone();
    storage::write_log(&cfg, &date, &items).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_week(state: State<AppData>, anchor: String) -> Vec<DayLog> {
    let cfg = state.config.lock().unwrap().clone();
    storage::read_week(&cfg, &anchor)
}

#[tauri::command]
pub fn get_view_mode(state: State<AppData>) -> String {
    let cfg = state.config.lock().unwrap().clone();
    match scheduler::view_mode_for(&cfg, Local::now()) {
        scheduler::ViewMode::Daily => "daily".into(),
        scheduler::ViewMode::Weekly => "weekly".into(),
    }
}

#[tauri::command]
pub fn snooze(app: AppHandle, state: State<AppData>) {
    let cfg = state.config.lock().unwrap().clone();
    {
        let mut st = state.state.lock().unwrap();
        let until = Local::now() + Duration::minutes(cfg.notify.snooze_minutes as i64);
        st.snooze_until = Some(until.to_rfc3339());
        let _ = crate::data::state::save(&st);
    }
    window::destroy_window(&app);
}

#[tauri::command]
pub fn skip_today(app: AppHandle, state: State<AppData>) {
    {
        let mut st = state.state.lock().unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        if !st.skipped_dates.contains(&today) {
            st.skipped_dates.push(today);
        }
        crate::data::state::cleanup_skipped(&mut st);
        let _ = crate::data::state::save(&st);
    }
    window::destroy_window(&app);
}

#[tauri::command]
pub fn close_window(app: AppHandle, state: State<AppData>) {
    // Closing without writing anything is not a dismissal for the day — the
    // reminder repeats. Stamp the time so the next one is a repeat interval
    // away rather than arriving on the very next tick, which would make the
    // box impossible to put down without "오늘 건너뛰기".
    {
        let mut st = state.state.lock().unwrap();
        st.last_notified_at = Some(Local::now().to_rfc3339());
        let _ = crate::data::state::save(&st);
    }
    window::destroy_window(&app);
}
