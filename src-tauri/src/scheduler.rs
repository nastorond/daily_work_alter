use crate::data::state::AppState;
use crate::shell::window::ShowResult;
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
    Onboarding,
    /// Today was missed entirely; show it late, once.
    DueCatchUp,
    NotWorkday,
    Skipped,
    RecentlyNotified,
    WindowAlreadyOpen,
    Snoozed,
    OutsideWindow,
    AfterEndTime,
    BadEndTime,
}

impl Decision {
    fn label(self) -> &'static str {
        match self {
            Decision::Due => "띄울 차례",
            Decision::Onboarding => "최초 설정 중",
            Decision::DueCatchUp => "놓친 날 보충",
            Decision::NotWorkday => "근무 요일이 아님",
            Decision::Skipped => "오늘 건너뛰기 상태",
            Decision::RecentlyNotified => "방금 띄웠음 (다시 알림 대기 중)",
            Decision::WindowAlreadyOpen => "창이 이미 열려 있음",
            Decision::Snoozed => "스누즈 중",
            Decision::OutsideWindow => "알림 시간대가 아님",
            Decision::AfterEndTime => "퇴근 시간이 지났고 오늘은 이미 띄웠음",
            Decision::BadEndTime => "퇴근 시간 형식을 읽을 수 없음",
        }
    }
}

/// Judgement order, short-circuiting on the first rule that declines:
/// workday -> not skipped -> not snoozed -> repeat interval elapsed ->
/// inside the notify window [endTime - minutesBefore, endTime].
///
/// There is no start-of-day guard: the window is the half hour before endTime,
/// so a start time could only matter if it fell inside that half hour, which no
/// real shift does. Keeping the setting just to have something to check would be
/// the tail wagging the dog.
///
/// Two deliberate choices here:
///
/// Whether anything is already written does not matter. It used to stop the
/// reminder, but "I jotted one line at lunch" is not a reason to skip the
/// end-of-day pass, and it made an accidental early save silently cancel the day.
///
/// endTime is a hard deadline. There is no catch-up grace past it: the window is
/// for the half hour before leaving, so if the machine was asleep through that
/// half hour the day is simply missed. The cost is a missed reminder after a long
/// sleep; the gain is that "past endTime" means one thing everywhere, including
/// the auto-close in `tick`.
///
/// The reminder repeats inside the window rather than firing once: closing the
/// box without writing used to end the day, so one missed pop lost it entirely.
/// `notify.repeatMinutes` sets the gap (0 restores once-a-day). Skipping the day
/// or snoozing are the explicit ways to say "not now".
/// Whether the repeat gap since the last pop has elapsed. Returns the reason to
/// keep waiting, or None to carry on with the remaining rules.
fn repeat_wait(
    cfg: &crate::data::config::Config,
    state: &AppState,
    now: DateTime<Local>,
) -> Option<Decision> {
    let last = state.last_notified_at.as_deref()?;
    let last = DateTime::parse_from_rfc3339(last).ok()?.with_timezone(&Local);

    if cfg.notify.repeat_minutes == 0 {
        // Once-a-day mode: anything shown today is enough.
        return (last.date_naive() == now.date_naive()).then_some(Decision::RecentlyNotified);
    }
    let due_again = last + Duration::minutes(cfg.notify.repeat_minutes as i64);
    (now < due_again).then_some(Decision::RecentlyNotified)
}

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
    // Snooze is checked before the repeat gap so an explicit "10분 뒤 다시"
    // governs, rather than being overridden by whichever interval is longer.
    if let Some(snooze) = &state.snooze_until {
        if let Ok(t) = DateTime::parse_from_rfc3339(snooze) {
            if t.with_timezone(&Local) > now {
                return Decision::Snoozed;
            }
        }
    }

    let Some(end) = parse_time_today(&cfg.work.end_time, now) else {
        return Decision::BadEndTime;
    };
    let window_start = end - Duration::minutes(cfg.notify.minutes_before as i64);

    if now < window_start {
        return Decision::OutsideWindow;
    }

    if now <= end {
        return match repeat_wait(cfg, state, now) {
            Some(wait) => wait,
            None => Decision::Due,
        };
    }

    // Past endTime. Normally done for the day — but if the reminder never got
    // shown today at all (machine asleep or locked through the whole window,
    // app not running), the stored date says so and it gets one late showing.
    if state.last_shown_date.as_deref() == Some(today.as_str()) {
        Decision::AfterEndTime
    } else {
        Decision::DueCatchUp
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

static LAST_DECISION: std::sync::Mutex<Option<Decision>> = std::sync::Mutex::new(None);

/// Logs a declined decision only when it differs from the previous tick, so a
/// whole quiet day is a handful of lines instead of 1440.
fn log_if_changed(decision: Decision) {
    let mut last = LAST_DECISION.lock().unwrap();
    if *last == Some(decision) {
        return;
    }
    *last = Some(decision);
    crate::data::log::line(&format!("대기: {}", decision.label()));
}

/// Records a decision without writing a line — used after a successful pop,
/// which logs its own message, so the next decline is still recognised as a
/// change without "대기: 띄울 차례" appearing in the file.
fn mark_decision(decision: Decision) {
    *LAST_DECISION.lock().unwrap() = Some(decision);
}

/// Called every 60s from the scheduler thread, plus once at startup and on
/// manual "지금 알림 테스트". Not re-entrant-safe across threads by design —
/// only the single scheduler thread and tray "test" action call this.
pub fn tick(app: &AppHandle) {
    // Never interrupt first-run onboarding: someone who hasn't set a 퇴근 시간
    // yet has nothing useful to be reminded about.
    if crate::shell::window::is_onboarding(app) {
        log_if_changed(Decision::Onboarding);
        return;
    }

    let data = app.state::<AppData>();
    let now = Local::now();

    let cfg = data.config.lock().unwrap().clone();
    let mut state = data.state.lock().unwrap();

    state.last_tick_at = Some(now.to_rfc3339());
    crate::data::state::cleanup_skipped(&mut state);

    let decision = notify_decision(&cfg, &state, now);
    let _ = crate::data::state::save(&state);
    drop(state);

    // A box that sat open across endTime is stale — whoever it was for has gone
    // home. Close it so it isn't on the desktop tomorrow morning. Only windows
    // opened *before* endTime qualify: a late catch-up window is opened after it
    // and must survive, or it would be shut on the very next tick. Autosave runs
    // on a 500ms debounce, so anything typed is already on disk.
    if let Some(end) = parse_time_today(&cfg.work.end_time, now) {
        let opened_before_end = crate::shell::window::opened_at(app).is_some_and(|t| t <= end);
        if now > end && opened_before_end && crate::shell::window::is_open(app) {
            crate::shell::window::destroy_window(app);
            crate::data::log::line("퇴근 시간이 지나 열려 있던 창을 닫음");
            return;
        }
    }

    if !matches!(decision, Decision::Due | Decision::DueCatchUp) {
        log_if_changed(decision);
        return;
    }

    // Don't re-pop over a window that is already up: the reminder repeats now,
    // and stealing focus every few minutes while someone is typing into it
    // would be worse than not reminding at all.
    if crate::shell::window::is_open(app) {
        log_if_changed(Decision::WindowAlreadyOpen);
        return;
    }

    // The day is only marked as reminded once a window actually exists. Marking
    // it before knowing that turned any single failed window creation into
    // "never popped all day": the date was stamped, so every later tick
    // short-circuited on "already notified today".
    let mode = view_mode_for(&cfg, now);
    match crate::shell::window::show_window(app, mode, false) {
        ShowResult::Shown => {}
        // Someone else is building it right now (a manual launch landing on the
        // same moment). Not a failure, and not ours to record either — leave the
        // stamp alone so whoever closes the window sets it.
        ShowResult::Building => return,
        ShowResult::Failed => {
            crate::data::log::line("띄우려 했으나 창 생성 실패, 다음 tick에서 재시도");
            return;
        }
    }

    let today = now.format("%Y-%m-%d").to_string();
    {
        let mut state = data.state.lock().unwrap();
        state.last_notified_at = Some(now.to_rfc3339());
        state.last_shown_date = Some(today.clone());
        let _ = crate::data::state::save(&state);
    }
    crate::shell::notify::notify_time_to_write(app, mode);
    let tag = if decision == Decision::DueCatchUp { " 놓친 날 보충" } else { "" };
    crate::data::log::line(&format!("띄움: {mode:?} ({today}){tag}"));
    mark_decision(decision);
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
        c.work.end_time = "17:00".into();
        c.work.workdays = vec![1, 2, 3, 4, 5];
        c.notify.minutes_before = 30;
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
        assert!(should_notify(&c, &fresh_state(), at(17, 0)));
    }

    #[test]
    fn silent_before_the_window_opens() {
        let c = cfg("before");
        assert!(!should_notify(&c, &fresh_state(), at(16, 29)));
    }

    /// Past endTime, a day already shown is done.
    #[test]
    fn silent_after_end_time_once_the_day_was_shown() {
        let c = cfg("after");
        let mut st = fresh_state();
        st.last_shown_date = Some(TODAY.to_string());
        assert!(!should_notify(&c, &st, at(17, 1)));
        assert_eq!(notify_decision(&c, &st, at(21, 0)), Decision::AfterEndTime);
    }

    /// ...but a day that was never shown at all gets one late showing. This is
    /// the sleep/lock recovery: the stored date, not a time-based grace.
    #[test]
    fn a_day_never_shown_is_caught_up_after_end_time() {
        let c = cfg("catchup");
        let st = fresh_state();
        assert_eq!(notify_decision(&c, &st, at(21, 0)), Decision::DueCatchUp);

        // Yesterday's showing doesn't count as today's.
        let mut stale = fresh_state();
        stale.last_shown_date = Some("2026-09-09".to_string());
        assert_eq!(notify_decision(&c, &stale, at(21, 0)), Decision::DueCatchUp);
    }

    /// Written content no longer suppresses the reminder — one line jotted at
    /// lunch is not a reason to skip the end-of-day pass.
    #[test]
    fn already_written_content_does_not_stop_the_reminder() {
        let c = cfg("saved");
        storage::write_log(&c, TODAY, &["한 줄 썼음".to_string()]).unwrap();
        assert!(storage::log_has_content(&c, TODAY));
        assert!(should_notify(&c, &fresh_state(), at(16, 30)));
    }

    /// A pop holds the reminder off for `repeatMinutes`, then it comes back —
    /// closing the box without writing anything must not end the day.
    #[test]
    fn reminder_repeats_after_the_gap() {
        let mut c = cfg("repeat");
        c.notify.repeat_minutes = 10;
        let mut st = fresh_state();
        st.last_notified_at = Some(at(16, 30).to_rfc3339());

        assert!(!should_notify(&c, &st, at(16, 35)));
        assert_eq!(
            notify_decision(&c, &st, at(16, 35)),
            Decision::RecentlyNotified
        );
        assert!(should_notify(&c, &st, at(16, 40)));
        assert!(should_notify(&c, &st, at(17, 0)));
    }

    /// repeatMinutes = 0 keeps the old once-a-day behaviour.
    #[test]
    fn zero_repeat_means_once_a_day() {
        let mut c = cfg("once");
        c.notify.repeat_minutes = 0;
        let mut st = fresh_state();
        st.last_notified_at = Some(at(16, 30).to_rfc3339());
        st.last_shown_date = Some(TODAY.to_string());
        assert!(!should_notify(&c, &st, at(16, 50)));

        // Yesterday's pop does not carry over.
        st.last_notified_at = Some(
            Local
                .with_ymd_and_hms(2026, 9, 9, 16, 30, 0)
                .unwrap()
                .to_rfc3339(),
        );
        assert!(should_notify(&c, &st, at(16, 30)));
    }

    /// An explicit snooze is checked before the repeat gap, so it governs even
    /// when it is shorter.
    #[test]
    fn snooze_wins_over_the_repeat_gap() {
        let mut c = cfg("snooze_vs_repeat");
        c.notify.repeat_minutes = 60;
        let mut st = fresh_state();
        st.last_notified_at = Some(at(16, 30).to_rfc3339());
        st.snooze_until = Some(at(16, 40).to_rfc3339());
        assert_eq!(notify_decision(&c, &st, at(16, 35)), Decision::Snoozed);
    }

    /// The reason is what ends up in log.txt, so it is worth pinning down.
    #[test]
    fn declining_reports_which_rule_declined() {
        let c = cfg("reasons");
        let mut st = fresh_state();
        st.last_notified_at = Some(at(16, 25).to_rfc3339());
        assert_eq!(
            notify_decision(&c, &st, at(16, 30)),
            Decision::RecentlyNotified
        );

        let mut st = fresh_state();
        st.skipped_dates = vec![TODAY.to_string()];
        assert_eq!(notify_decision(&c, &st, at(16, 30)), Decision::Skipped);

        assert_eq!(
            notify_decision(&c, &fresh_state(), at(16, 29)),
            Decision::OutsideWindow
        );
        assert_eq!(
            notify_decision(&c, &fresh_state(), at(7, 0)),
            Decision::OutsideWindow
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
            Decision::Due
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
