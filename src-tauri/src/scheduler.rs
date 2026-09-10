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

/// Why the reminder is or isn't showing right now.
///
/// This is an enum rather than a bool so the log can say *which* rule declined.
/// "It didn't pop and I don't know why" was costing a state.json autopsy every
/// time; now the reason is in log.txt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Due,
    NotWorkday,
    Skipped,
    AlreadyWritten,
    AlreadyNotified,
    Snoozed,
    BeforeWorkStart,
    OutsideWindow,
    BadEndTime,
}

impl Decision {
    fn label(self) -> &'static str {
        match self {
            Decision::Due => "띄울 차례",
            Decision::NotWorkday => "근무 요일이 아님",
            Decision::Skipped => "오늘 건너뛰기 상태",
            Decision::AlreadyWritten => "오늘 기록에 이미 내용이 있음",
            Decision::AlreadyNotified => "오늘 이미 띄웠음",
            Decision::Snoozed => "스누즈 중",
            Decision::BeforeWorkStart => "출근 시간 전",
            Decision::OutsideWindow => "알림 시간대가 아님",
            Decision::BadEndTime => "퇴근 시간 형식을 읽을 수 없음",
        }
    }
}

/// Judgement order, short-circuiting on the first rule that declines:
/// workday -> not skipped -> nothing written yet -> not already notified today
/// -> not snoozed -> after startTime -> inside the notify window
/// [endTime - minutesBefore, endTime + catchUpHours].
pub fn notify_decision(
    cfg: &crate::data::config::Config,
    state: &AppState,
    now: DateTime<Local>,
) -> Decision {
    let today = now.format("%Y-%m-%d").to_string();
    let iso_weekday = now.weekday().number_from_monday() as u8;

    if !cfg.work.workdays.contains(&iso_weekday) {
        return Decision::NotWorkday;
    }
    if state.skipped_dates.contains(&today) {
        return Decision::Skipped;
    }
    if storage::log_has_content(cfg, &today) {
        return Decision::AlreadyWritten;
    }
    if state.last_notified_date.as_deref() == Some(today.as_str()) {
        return Decision::AlreadyNotified;
    }
    if let Some(snooze) = &state.snooze_until {
        if let Ok(t) = DateTime::parse_from_rfc3339(snooze) {
            if t.with_timezone(&Local) > now {
                return Decision::Snoozed;
            }
        }
    }

    // Guard against popping outside working hours: on a machine that woke from
    // sleep long after the catch-up window, or with an unusually large
    // catchUpHours, this keeps the reminder from surfacing before the workday
    // has started.
    if let Some(start) = parse_time_today(&cfg.work.start_time, now) {
        if now < start {
            return Decision::BeforeWorkStart;
        }
    }

    let Some(end) = parse_time_today(&cfg.work.end_time, now) else {
        return Decision::BadEndTime;
    };
    let window_start = end - Duration::minutes(cfg.notify.minutes_before as i64);
    let window_end = end + Duration::hours(cfg.notify.catch_up_hours as i64);
    if now >= window_start && now <= window_end {
        Decision::Due
    } else {
        Decision::OutsideWindow
    }
}

/// Test-only shorthand; production code matches on the reason instead.
#[cfg(test)]
fn should_notify(
    cfg: &crate::data::config::Config,
    state: &AppState,
    now: DateTime<Local>,
) -> bool {
    notify_decision(cfg, state, now) == Decision::Due
}

/// Logs a declined decision only when it differs from the previous tick, so a
/// whole quiet day is a handful of lines instead of 1440.
fn log_if_changed(decision: Decision) {
    static LAST: std::sync::Mutex<Option<Decision>> = std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap();
    if *last == Some(decision) {
        return;
    }
    *last = Some(decision);
    crate::data::log::line(&format!("대기: {}", decision.label()));
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

    let decision = notify_decision(&cfg, &state, now);
    let _ = crate::data::state::save(&state);
    drop(state);

    if decision != Decision::Due {
        log_if_changed(decision);
        return;
    }

    // The day is only marked as reminded once a window actually exists. Marking
    // it before knowing that turned any single failed window creation into
    // "never popped all day": the date was stamped, so every later tick
    // short-circuited on "already notified today".
    let mode = view_mode_for(&cfg, now);
    if !crate::shell::window::show_window(app, mode, false) {
        crate::data::log::line("notify due but no window appeared; retrying next tick");
        return;
    }

    let today = now.format("%Y-%m-%d").to_string();
    {
        let mut state = data.state.lock().unwrap();
        state.last_notified_date = Some(today.clone());
        let _ = crate::data::state::save(&state);
    }
    crate::shell::notify::notify_time_to_write(app, mode);
    crate::data::log::line(&format!("띄움: {mode:?} ({today})"));
    log_if_changed(Decision::Due);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::config::Config;
    use crate::data::storage;
    use chrono::TimeZone;

    /// 2026-09-10 is a Thursday (ISO weekday 4).
    const TODAY: &str = "2026-09-10";

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("dwa_sched_test_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Mirrors the user's real settings: 08:00-17:00 Mon-Fri, 30 minutes before.
    /// So the notify window is [16:30, 21:00].
    fn cfg(tag: &str) -> Config {
        let mut c = Config::default();
        c.work.start_time = "08:00".into();
        c.work.end_time = "17:00".into();
        c.work.workdays = vec![1, 2, 3, 4, 5];
        c.notify.minutes_before = 30;
        c.notify.catch_up_hours = 4;
        c.log_dir = Some(temp_dir(tag).to_string_lossy().to_string());
        c
    }

    fn at(h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 9, 10, h, m, 0).unwrap()
    }

    fn fresh_state() -> AppState {
        AppState::default()
    }

    #[test]
    fn fires_inside_the_notify_window() {
        let c = cfg("inside");
        assert!(should_notify(&c, &fresh_state(), at(16, 30)));
        assert!(should_notify(&c, &fresh_state(), at(18, 0)));
    }

    #[test]
    fn silent_before_the_window_opens() {
        let c = cfg("before");
        assert!(!should_notify(&c, &fresh_state(), at(16, 29)));
    }

    #[test]
    fn silent_after_the_catch_up_window() {
        let c = cfg("after");
        assert!(!should_notify(&c, &fresh_state(), at(21, 1)));
    }

    /// The question this file exists to answer: saving real content stops the
    /// reminder for the rest of the day.
    #[test]
    fn saved_content_stops_the_reminder() {
        let c = cfg("saved");
        storage::write_log(&c, TODAY, &["한 줄 썼음".to_string()]).unwrap();
        assert!(storage::log_has_content(&c, TODAY));
        assert!(!should_notify(&c, &fresh_state(), at(16, 30)));
    }

    /// ...but saving an *empty* box does not. The file gets written with only
    /// frontmatter, which does not count as content.
    #[test]
    fn saving_an_empty_box_does_not_stop_the_reminder() {
        let c = cfg("empty");
        storage::write_log(&c, TODAY, &[]).unwrap();
        assert!(!storage::log_has_content(&c, TODAY));
        assert!(should_notify(&c, &fresh_state(), at(16, 30)));
    }

    /// Once a pop has happened, that is it for the day — regardless of whether
    /// anything was actually written.
    #[test]
    fn only_pops_once_a_day() {
        let c = cfg("once");
        let mut st = fresh_state();
        st.last_notified_date = Some(TODAY.to_string());
        assert!(!should_notify(&c, &st, at(16, 30)));
    }

    /// The reason is what ends up in log.txt, so it is worth pinning down.
    #[test]
    fn declining_reports_which_rule_declined() {
        let c = cfg("reasons");
        let mut st = fresh_state();
        st.last_notified_date = Some(TODAY.to_string());
        assert_eq!(notify_decision(&c, &st, at(16, 30)), Decision::AlreadyNotified);

        let mut st = fresh_state();
        st.skipped_dates = vec![TODAY.to_string()];
        assert_eq!(notify_decision(&c, &st, at(16, 30)), Decision::Skipped);

        assert_eq!(
            notify_decision(&c, &fresh_state(), at(16, 29)),
            Decision::OutsideWindow
        );
        assert_eq!(
            notify_decision(&c, &fresh_state(), at(7, 0)),
            Decision::BeforeWorkStart
        );

        let mut weekend = cfg("reasons_weekend");
        weekend.work.workdays = vec![1, 2, 3];
        assert_eq!(
            notify_decision(&weekend, &fresh_state(), at(16, 30)),
            Decision::NotWorkday
        );

        let written = cfg("reasons_written");
        storage::write_log(&written, TODAY, &["뭔가 씀".to_string()]).unwrap();
        assert_eq!(
            notify_decision(&written, &fresh_state(), at(16, 30)),
            Decision::AlreadyWritten
        );
    }

    #[test]
    fn skipping_the_day_stops_the_reminder() {
        let c = cfg("skip");
        let mut st = fresh_state();
        st.skipped_dates = vec![TODAY.to_string()];
        assert!(!should_notify(&c, &st, at(16, 30)));
    }

    #[test]
    fn snooze_holds_until_it_expires() {
        let c = cfg("snooze");
        let mut st = fresh_state();
        st.snooze_until = Some(at(16, 40).to_rfc3339());
        assert!(!should_notify(&c, &st, at(16, 35)));
        assert!(should_notify(&c, &st, at(16, 45)));
    }

    #[test]
    fn a_stale_snooze_from_an_earlier_day_does_not_block() {
        let c = cfg("stale_snooze");
        let mut st = fresh_state();
        st.snooze_until = Some(
            Local
                .with_ymd_and_hms(2026, 9, 8, 16, 40, 0)
                .unwrap()
                .to_rfc3339(),
        );
        assert!(should_notify(&c, &st, at(16, 30)));
    }

    #[test]
    fn non_workdays_are_silent() {
        let mut c = cfg("weekend");
        c.work.workdays = vec![1, 2, 3]; // Mon-Wed only; the 10th is a Thursday
        assert!(!should_notify(&c, &fresh_state(), at(16, 30)));
    }

    #[test]
    fn thursday_uses_the_weekly_view_when_configured() {
        let mut c = cfg("weekly_mode");
        c.weekly.enabled = true;
        c.weekly.mode = "fixedDay".into();
        c.weekly.day = 4; // Thursday
        assert_eq!(view_mode_for(&c, at(16, 30)), ViewMode::Weekly);
        c.weekly.day = 5; // Friday
        assert_eq!(view_mode_for(&c, at(16, 30)), ViewMode::Daily);
    }
}
