use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn default_start_time() -> String {
    "09:00".into()
}
fn default_end_time() -> String {
    "18:00".into()
}
fn default_workdays() -> Vec<u8> {
    vec![1, 2, 3, 4, 5]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkConfig {
    #[serde(default = "default_start_time")]
    pub start_time: String,
    #[serde(default = "default_end_time")]
    pub end_time: String,
    #[serde(default = "default_workdays")]
    pub workdays: Vec<u8>,
}

impl Default for WorkConfig {
    fn default() -> Self {
        Self {
            start_time: default_start_time(),
            end_time: default_end_time(),
            workdays: default_workdays(),
        }
    }
}

fn default_minutes_before() -> u32 {
    30
}
fn default_snooze_minutes() -> u32 {
    10
}
fn default_catch_up_hours() -> u32 {
    4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifyConfig {
    #[serde(default = "default_minutes_before")]
    pub minutes_before: u32,
    #[serde(default = "default_snooze_minutes")]
    pub snooze_minutes: u32,
    #[serde(default = "default_catch_up_hours")]
    pub catch_up_hours: u32,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            minutes_before: default_minutes_before(),
            snooze_minutes: default_snooze_minutes(),
            catch_up_hours: default_catch_up_hours(),
        }
    }
}

fn default_weekly_enabled() -> bool {
    true
}
fn default_weekly_mode() -> String {
    "fixedDay".into()
}
fn default_weekly_day() -> u8 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyConfig {
    #[serde(default = "default_weekly_enabled")]
    pub enabled: bool,
    #[serde(default = "default_weekly_mode")]
    pub mode: String,
    #[serde(default = "default_weekly_day")]
    pub day: u8,
}

impl Default for WeeklyConfig {
    fn default() -> Self {
        Self {
            enabled: default_weekly_enabled(),
            mode: default_weekly_mode(),
            day: default_weekly_day(),
        }
    }
}

fn default_version() -> u32 {
    1
}
fn default_hotkey() -> String {
    "CommandOrControl+Alt+D".into()
}
fn default_autostart() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub work: WorkConfig,
    #[serde(default)]
    pub notify: NotifyConfig,
    #[serde(default)]
    pub weekly: WeeklyConfig,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_autostart")]
    pub autostart: bool,
    #[serde(default)]
    pub log_dir: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            work: WorkConfig::default(),
            notify: NotifyConfig::default(),
            weekly: WeeklyConfig::default(),
            hotkey: default_hotkey(),
            autostart: default_autostart(),
            log_dir: None,
        }
    }
}

pub fn app_data_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA").expect("APPDATA environment variable not set");
    PathBuf::from(appdata).join("DailyWorkAlter")
}

pub fn config_path() -> PathBuf {
    app_data_dir().join("config.json")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    fs::create_dir_all(app_data_dir().join("logs"))
}

/// Whether config.json exists yet. Used to detect the very first run (onboarding).
pub fn exists() -> bool {
    config_path().exists()
}

/// Initial load at app startup. Missing keys are filled in-memory via serde defaults.
/// If the file is missing, defaults are written out so the user has something to edit.
/// If the file is unparsable, defaults are used in-memory without touching the file.
pub fn load() -> Config {
    let _ = ensure_dirs();
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<Config>(&content).unwrap_or_default(),
        Err(_) => {
            let cfg = Config::default();
            let _ = save(&cfg);
            cfg
        }
    }
}

pub fn save(cfg: &Config) -> std::io::Result<()> {
    ensure_dirs()?;
    let json = serde_json::to_string_pretty(cfg)?;
    fs::write(config_path(), json)
}

pub enum ReloadResult {
    Applied(Config),
    /// File is transiently unreadable (e.g. mid-atomic-save) — not a user error.
    Unchanged,
    /// File exists but doesn't parse — keep the in-memory config, surface a warning.
    ParseError,
}

/// Used by the file watcher: on parse failure, keep whatever is already loaded in memory
/// rather than resetting to defaults or crashing.
pub fn try_reload() -> ReloadResult {
    match fs::read_to_string(config_path()) {
        Ok(content) => match serde_json::from_str::<Config>(&content) {
            Ok(cfg) => ReloadResult::Applied(cfg),
            Err(_) => ReloadResult::ParseError,
        },
        Err(_) => ReloadResult::Unchanged,
    }
}
