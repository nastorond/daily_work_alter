use crate::data::config::app_data_dir;
use chrono::{Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    /// When the reminder was last shown (RFC3339). A timestamp rather than a
    /// date because the reminder now repeats until something is written; the
    /// date alone could only express "once a day".
    pub last_notified_at: Option<String>,
    /// The date the reminder was shown on at least once (YYYY-MM-DD). Comparing
    /// this against today is what recovers a day missed entirely — asleep,
    /// locked, or the app not running through the whole notify window. A
    /// time-based grace can't do that: it can only ask "how long since endTime",
    /// which says nothing about whether the day was ever handled.
    pub last_shown_date: Option<String>,
    pub snooze_until: Option<String>,
    #[serde(default)]
    pub skipped_dates: Vec<String>,
    pub last_tick_at: Option<String>,
}

pub fn state_path() -> PathBuf {
    app_data_dir().join("state.json")
}

pub fn load() -> AppState {
    match fs::read_to_string(state_path()) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppState::default(),
    }
}

pub fn save(state: &AppState) -> std::io::Result<()> {
    let _ = crate::data::config::ensure_dirs();
    let json = serde_json::to_string_pretty(state)?;
    fs::write(state_path(), json)
}

/// Drops entries in `skippedDates` older than 30 days.
pub fn cleanup_skipped(state: &mut AppState) {
    let cutoff = Local::now().date_naive() - Duration::days(30);
    state
        .skipped_dates
        .retain(|d| match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(nd) => nd >= cutoff,
            Err(_) => false,
        });
}
