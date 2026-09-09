use crate::data::state::AppState;
use crate::data::storage;
use crate::AppData;
use chrono::{DateTime, Datelike, Duration, Local, NaiveTime};
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    Daily,
    Weekly,
}

fn parse_time_today(hhmm: &str, today: DateTime<Local>) -> Option<DateTime<Local>> {
    let t = NaiveTime::parse_from_str(hhmm, "%H:%M").ok()?;
    today.date_naive().and_time(t).and_local_timezone(Local).single()
}

/// The single work-day iso-weekday (1=Mon..7=Sun) with the highest value in
/// `work.workdays` — used as "the last workday of the week" for `lastWorkday` mode.
fn last_workday_iso(cfg: &crate::data::config::Config) -> Option<u8> {
    cfg.work.workdays.iter().copied().max()
}

pub fn is_last_workday(cfg: &crate::data::config::Config, now: DateTime<Local>) -> bool {
    let iso = now.weekday().number_from_monday() as u8;
    last_workday_iso(cfg) == Some(iso)
}

pub fn view_mode_for(cfg: &crate::data::config::Config, now: DateTime<Local>) -> ViewMode {
    if !cfg.weekly.enabled {
        return ViewMode::Daily;
    }
    let iso = now.weekday().number_from_monday() as u8;
    let is_weekly = match cfg.weekly.mode.as_str() {
        "fixedDay" => iso == cfg.weekly.day,
        "lastWorkday" => is_last_workday(cfg, now),
        _ => false,
    };
    if is_weekly {
        ViewMode::Weekly
    } else {
        ViewMode::Daily
    }
}

/// Judgement order, short-circuiting on the first rule that says no:
/// workday -> not skipped -> nothing written yet -> not already notified today
/// -> not snoozed -> after startTime -> inside the notify window
/// [endTime - minutesBefore, endTime + catchUpHours].
pub fn should_notify(cfg: &crate::data::config::Config, state: &AppState, now: DateTime<Local>) -> bool {
    let today = now.format("%Y-%m-%d").to_string();
    let iso_weekday = now.weekday().number_from_monday() as u8;

    if !cfg.work.workdays.contains(&iso_weekday) {
        return false;
    }
    if state.skipped_dates.contains(&today) {
        return false;
    }
    if storage::log_has_content(cfg, &today) {
        return false;
    }
    if state.last_notified_date.as_deref() == Some(today.as_str()) {
        return false;
    }
    if let Some(snooze) = &state.snooze_until {
        if let Ok(t) = DateTime::parse_from_rfc3339(snooze) {
            if t.with_timezone(&Local) > now {
                return false;
            }
        }
    }

    // Guard against popping outside working hours: on a machine that woke from
    // sleep long after the catch-up window, or with an unusually large
    // catchUpHours, this keeps the reminder from surfacing before the workday
    // has started.
    if let Some(start) = parse_time_today(&cfg.work.start_time, now) {
        if now < start {
            return false;
        }
    }

    let Some(end) = parse_time_today(&cfg.work.end_time, now) else {
        return false;
    };
    let window_start = end - Duration::minutes(cfg.notify.minutes_before as i64);
    let window_end = end + Duration::hours(cfg.notify.catch_up_hours as i64);
    now >= window_start && now <= window_end
}

/// Called every 60s from the scheduler thread, plus once at startup and on
/// manual "지금 알림 테스트". Not re-entrant-safe across threads by design —
/// only the single scheduler thread and tray "test" action call this.
pub fn tick(app: &AppHandle) {
    let data = app.state::<AppData>();
    let now = Local::now();

    let cfg = data.config.lock().unwrap().clone();
    let mut state = data.state.lock().unwrap();

    state.last_tick_at = Some(now.to_rfc3339());
    crate::data::state::cleanup_skipped(&mut state);

    let should = should_notify(&cfg, &state, now);
    if should {
        state.last_notified_date = Some(now.format("%Y-%m-%d").to_string());
    }
    let _ = crate::data::state::save(&state);
    drop(state);

    if should {
        let mode = view_mode_for(&cfg, now);
        crate::shell::window::show_window(app, mode, false);
        crate::shell::notify::notify_time_to_write(app, mode);
    }
}

/// Tray "지금 알림 테스트" — bypasses all should_notify gating and just shows
/// whichever view would be shown right now.
pub fn force_show(app: &AppHandle) {
    let data = app.state::<AppData>();
    let cfg = data.config.lock().unwrap().clone();
    let mode = view_mode_for(&cfg, Local::now());
    crate::shell::window::show_window(app, mode, false);
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || loop {
        tick(&app);
        std::thread::sleep(std::time::Duration::from_secs(60));
    });
}
