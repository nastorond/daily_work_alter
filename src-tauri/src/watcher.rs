use crate::config::{self, ReloadResult};
use crate::AppData;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Watches %APPDATA%\DailyWorkAlter\ for changes to config.json so hand edits are
/// picked up without an app restart. See 4.4 in 실행계획.md.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut debouncer = match new_debouncer(Duration::from_millis(500), tx) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("config watcher init failed: {e}");
                return;
            }
        };
        if let Err(e) = debouncer
            .watcher()
            .watch(&config::app_data_dir(), notify::RecursiveMode::NonRecursive)
        {
            eprintln!("config watch failed: {e}");
            return;
        }

        for res in rx {
            let Ok(events) = res else { continue };
            let touched = events.iter().any(|e| {
                e.kind == DebouncedEventKind::Any
                    && e.path.file_name().map(|n| n == "config.json").unwrap_or(false)
            });
            if touched {
                reload(&app);
            }
        }
    });
}

fn reload(app: &AppHandle) {
    match config::try_reload() {
        ReloadResult::Applied(new_cfg) => {
            let data = app.state::<AppData>();
            *data.config.lock().unwrap() = new_cfg.clone();
            let _ = crate::shortcut::reregister(app, &new_cfg.hotkey);
            crate::autostart::apply(app, new_cfg.autostart);
        }
        ReloadResult::ParseError => {
            crate::notify::notify_config_error(app);
        }
        ReloadResult::Unchanged => {}
    }
}
