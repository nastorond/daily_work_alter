mod autostart;
mod commands;
mod config;
mod notify;
mod scheduler;
mod shortcut;
mod state;
mod storage;
mod tray;
mod watcher;
mod window;

use config::Config;
use state::AppState;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_global_shortcut::ShortcutState;

pub struct AppData {
    pub config: Mutex<Config>,
    pub state: Mutex<AppState>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let first_run = !config::exists();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        window::open_window_default(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::open_config_file,
            commands::open_log_dir,
            commands::read_log,
            commands::write_log,
            commands::read_week,
            commands::get_view_mode,
            commands::snooze,
            commands::skip_today,
            commands::close_window,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            let cfg = config::load();
            let app_state = state::load();
            app.manage(AppData {
                config: Mutex::new(cfg.clone()),
                state: Mutex::new(app_state),
            });

            tray::build(&handle)?;

            if let Err(e) = shortcut::register(&handle, &cfg.hotkey) {
                eprintln!("global shortcut not registered: {e}");
            }
            autostart::apply(&handle, cfg.autostart);

            watcher::spawn(handle.clone());
            scheduler::spawn(handle.clone());

            if first_run {
                window::show_onboarding(&handle);
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            // This is a tray-resident app: the main window is destroyed after every
            // pop (see window.rs), so "zero windows open" is the normal 23.5h/day
            // state, not a reason to quit. Only the tray's "종료" (app.exit) should.
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}
