use crate::scheduler::ViewMode;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn notify_time_to_write(app: &AppHandle, mode: ViewMode) {
    let body = match mode {
        ViewMode::Daily => "오늘 한 일을 기록할 시간이에요.",
        ViewMode::Weekly => "이번 주 요약을 정리할 시간이에요.",
    };
    let _ = app
        .notification()
        .builder()
        .title("DailyWorkAlter")
        .body(body)
        .show();
}

pub fn notify_config_error(app: &AppHandle) {
    let _ = app
        .notification()
        .builder()
        .title("DailyWorkAlter")
        .body("설정 파일(config.json) 오류 — 이전 설정을 계속 사용합니다.")
        .show();
}
