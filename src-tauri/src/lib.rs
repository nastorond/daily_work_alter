mod autostart;
mod commands;
mod config;
mod devtools;
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

/// Passed on the command line by the autostart registry entry, so a login launch
/// can be told apart from the user double-clicking the exe.
const AUTOSTART_FLAG: &str = "--autostart";

fn launched_by_autostart() -> bool {
    std::env::args().any(|a| a == AUTOSTART_FLAG)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let first_run = !config::exists();

    tauri::Builder::default()
        // Must be registered first. Fires in the *existing* instance when the exe
        // is launched again; the newcomer exits on its own. A second launch is the
        // user asking for the window (the tray icon is hidden by default on
        // Windows 11, so a silent launch looks like nothing happened), so surface
        // it rather than letting a rival scheduler write the same log file.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::open_window_default(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_FLAG]),
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

            // Three ways in, three behaviours. Getting this wrong in either
            // direction is bad: popping the window at login defeats the point of
            // the app, and staying silent on a manual launch looks broken.
            if first_run {
                window::show_onboarding(&handle);
            } else if !launched_by_autostart() {
                window::open_window_default(&handle);
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            // This is a tray-resident app: the main window is destroyed after every
            // pop (see window.rs), so "zero windows open" is the normal 23.5h/day
            // state, not a reason to quit.
            //
            // `code` tells the two cases apart. Closing the last window raises
            // ExitRequested with `None` — that one gets blocked. The tray's "종료"
            // calls app.exit(0), which arrives as `Some(0)` and must be let
            // through, or the menu item does nothing and the only way out is
            // Task Manager.
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
