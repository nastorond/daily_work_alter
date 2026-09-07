//! Helpers behind the tray "테스트" submenu.
//!
//! Everything here exists so the app's screens can be exercised without waiting
//! for 17:30 or for Friday: seed a week of logs, wipe them, reset the notify
//! state, replay onboarding. The submenu is only attached in debug builds
//! (see tray.rs), so the shipped exe has no menu path that can wipe logs.

use crate::config::Config;
use crate::storage;
use crate::AppData;
use chrono::{Datelike, Duration, Local, NaiveDate};
use tauri::{AppHandle, Manager};

/// Fake worklog lines, cycled across the week's workdays.
const SAMPLES: [&[&str]; 5] = [
    &[
        "OCR 파이프라인 전처리 단계 리팩터링",
        "배치 큐 타임아웃 원인 파악 (재시도에서 커넥션 반환 누락)",
    ],
    &["API 응답 스키마 정리", "그룹웨어 연동 스펙 회의"],
    &["배포 스크립트 롤백 경로 수정"],
    &[
        "주간 배포 리뷰 참석",
        "리포트 쿼리 인덱스 추가 — 응답 12s → 0.4s",
        "신규 입사자 온보딩 문서 보완",
    ],
    &["로그 수집기 디스크 사용량 알람 임계치 조정"],
];

fn current_config(app: &AppHandle) -> Config {
    let data = app.state::<AppData>();
    let cfg = data.config.lock().unwrap().clone();
    cfg
}

/// Configured workdays of the week containing today, oldest first.
fn week_workdays(cfg: &Config) -> Vec<NaiveDate> {
    let monday = storage::week_monday(Local::now().date_naive());
    (0..7i64)
        .map(|i| monday + Duration::days(i))
        .filter(|d| {
            cfg.work
                .workdays
                .contains(&(d.weekday().number_from_monday() as u8))
        })
        .collect()
}

/// One workday deliberately left blank so the weekly view's "(작성 없음)" branch
/// gets exercised too. Wednesday, unless today is Wednesday.
fn gap_iso(today: NaiveDate) -> u8 {
    if today.weekday().number_from_monday() as u8 == 3 {
        4
    } else {
        3
    }
}

/// Fills this week's workdays with sample entries, leaving today empty (so the
/// input box is still testable) and one day blank (so the empty-day rendering is).
/// Returns how many files were written.
pub fn seed_week(app: &AppHandle) -> usize {
    let cfg = current_config(app);
    let today = Local::now().date_naive();
    let gap = gap_iso(today);
    let mut written = 0;

    for (i, d) in week_workdays(&cfg).into_iter().enumerate() {
        if d == today || d.weekday().number_from_monday() as u8 == gap {
            continue;
        }
        let items: Vec<String> = SAMPLES[i % SAMPLES.len()]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let date = d.format("%Y-%m-%d").to_string();
        if storage::write_log(&cfg, &date, &items).is_ok() {
            written += 1;
        }
    }
    written
}

/// Deletes this week's log files. Returns how many were removed.
pub fn clear_week(app: &AppHandle) -> usize {
    let cfg = current_config(app);
    let mut removed = 0;
    for d in week_workdays(&cfg) {
        let path = storage::log_path(&cfg, &d.format("%Y-%m-%d").to_string());
        if std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Clears snooze / skipped / last-notified so the next tick can pop again.
/// Resets the in-memory copy too, not just state.json.
pub fn reset_state(app: &AppHandle) {
    let data = app.state::<AppData>();
    let mut st = data.state.lock().unwrap();
    *st = crate::state::AppState::default();
    let _ = crate::state::save(&st);
}
