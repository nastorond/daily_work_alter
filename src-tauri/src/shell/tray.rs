use crate::scheduler::ViewMode;
use crate::shell::window;
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

fn open_path(app: &AppHandle, path: std::path::PathBuf) {
    let _ = app
        .opener()
        .open_path(path.to_string_lossy().to_string(), None::<String>);
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let write_now = MenuItemBuilder::with_id("write_now", "지금 작성하기").build(app)?;
    let view_week = MenuItemBuilder::with_id("view_week", "이번 주 보기").build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "설정").build(app)?;
    let open_config = MenuItemBuilder::with_id("open_config", "config.json 열기").build(app)?;
    let open_log_dir = MenuItemBuilder::with_id("open_log_dir", "로그 폴더 열기").build(app)?;
    let test_notify = MenuItemBuilder::with_id("test_notify", "지금 알림 테스트").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "종료").build(app)?;

    // Debug-build-only screen/data helpers — see devtools.rs.
    let t_onboarding = MenuItemBuilder::with_id("t_onboarding", "온보딩 화면 보기").build(app)?;
    let t_daily = MenuItemBuilder::with_id("t_daily", "일일 화면 강제 열기").build(app)?;
    let t_weekly = MenuItemBuilder::with_id("t_weekly", "주간 화면 강제 열기").build(app)?;
    let t_seed = MenuItemBuilder::with_id("t_seed", "샘플 로그 생성 (이번 주)").build(app)?;
    let t_clear = MenuItemBuilder::with_id("t_clear", "이번 주 로그 삭제").build(app)?;
    let t_reset = MenuItemBuilder::with_id("t_reset", "알림 상태 초기화").build(app)?;
    let test_menu = SubmenuBuilder::new(app, "테스트")
        .item(&t_onboarding)
        .item(&t_daily)
        .item(&t_weekly)
        .separator()
        .item(&t_seed)
        .item(&t_clear)
        .separator()
        .item(&t_reset)
        .build()?;

    let mut builder = MenuBuilder::new(app)
        .item(&write_now)
        .item(&view_week)
        .separator()
        .item(&settings)
        .item(&open_config)
        .item(&open_log_dir);

    if cfg!(debug_assertions) {
        builder = builder.separator().item(&test_menu);
    }

    let menu = builder
        .separator()
        .item(&test_notify)
        .item(&quit)
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
            "open_config" => open_path(app, crate::data::config::config_path()),
            "open_log_dir" => {
                let data = app.state::<crate::AppData>();
                let cfg = data.config.lock().unwrap().clone();
                let dir = crate::data::storage::log_dir(&cfg);
                let _ = std::fs::create_dir_all(&dir);
                open_path(app, dir);
            }
            "test_notify" => crate::scheduler::force_show(app),

            "t_onboarding" => window::show_onboarding(app),
            "t_daily" => window::show_window(app, ViewMode::Daily, false),
            "t_weekly" => window::show_window(app, ViewMode::Weekly, false),
            // Seed/clear reopen the weekly view so the result is visible immediately.
            "t_seed" => {
                crate::devtools::seed_week(app);
                window::show_window(app, ViewMode::Weekly, false);
            }
            "t_clear" => {
                crate::devtools::clear_week(app);
                window::show_window(app, ViewMode::Weekly, false);
            }
            "t_reset" => crate::devtools::reset_state(app),

            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}
