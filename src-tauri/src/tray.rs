use crate::scheduler::ViewMode;
use crate::window;
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let write_now = MenuItemBuilder::with_id("write_now", "지금 작성하기").build(app)?;
    let view_week = MenuItemBuilder::with_id("view_week", "이번 주 보기").build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "설정").build(app)?;
    let open_log_dir = MenuItemBuilder::with_id("open_log_dir", "로그 폴더 열기").build(app)?;
    let test_notify = MenuItemBuilder::with_id("test_notify", "지금 알림 테스트").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "종료").build(app)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&write_now, &view_week, &sep1, &settings, &open_log_dir, &sep2, &test_notify, &quit])
        .build()?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("app icon missing");

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("DailyWorkAlter")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "write_now" => window::show_window(app, ViewMode::Daily, false),
            "view_week" => window::show_window(app, ViewMode::Weekly, false),
            "settings" => window::show_settings(app),
            "open_log_dir" => {
                let data = app.state::<crate::AppData>();
                let cfg = data.config.lock().unwrap().clone();
                let dir = crate::storage::log_dir(&cfg);
                let _ = std::fs::create_dir_all(&dir);
                let _ = app.opener().open_path(dir.to_string_lossy().to_string(), None::<String>);
            }
            "test_notify" => crate::scheduler::force_show(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}
