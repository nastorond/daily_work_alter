//! Exporting a week of logs into the 한글 weekly report.

pub mod hwpx;
pub mod outline;

use crate::data::config::{app_data_dir, Config};
use crate::data::storage;
use chrono::{Datelike, Duration, NaiveDate};
use outline::OutlineLine;

include!(concat!(env!("OUT_DIR"), "/bundled_template.rs"));

/// A template dropped here wins over the one baked into the exe, so the form can
/// be swapped without a rebuild.
pub fn override_path() -> std::path::PathBuf {
    app_data_dir().join("template.hwpx")
}

fn load_template() -> Result<Vec<u8>, String> {
    let path = override_path();
    if path.exists() {
        return std::fs::read(&path).map_err(|e| format!("{} 를 읽을 수 없습니다: {e}", path.display()));
    }
    BUNDLED_TEMPLATE.map(|b| b.to_vec()).ok_or_else(|| {
        format!(
            "내보낼 템플릿이 없습니다. 주간업무 일지 양식을 hwpx로 저장해 {} 에 두세요.",
            path.display()
        )
    })
}

/// Monday of the week containing `anchor`.
fn monday(anchor: &str) -> NaiveDate {
    let d = NaiveDate::parse_from_str(anchor, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Local::now().date_naive());
    storage::week_monday(d)
}

/// The workday span of the reported week, formatted as the form writes it:
/// `지난주 실적 (9. 7. ~ 9. 11.)`.
pub fn header_text(cfg: &Config, anchor: &str) -> String {
    match span(cfg, anchor) {
        Some((a, b)) => format!("지난주 실적 ({a} ~ {b})"),
        None => "지난주 실적".to_string(),
    }
}

/// Heading for the right-hand column: the week after the one being reported.
/// Its content is left empty — the app records what was done, not what is
/// planned — but the dates should still read correctly.
pub fn next_header_text(cfg: &Config, anchor: &str) -> String {
    let next = (monday(anchor) + Duration::days(7)).format("%Y-%m-%d").to_string();
    match span(cfg, &next) {
        Some((a, b)) => format!("이번주 계획 ({a} ~ {b})"),
        None => "이번주 계획".to_string(),
    }
}

fn span(cfg: &Config, anchor: &str) -> Option<(String, String)> {
    let days = storage::read_week(cfg, anchor);
    let a = short(&days.first()?.date)?;
    let b = short(&days.last()?.date)?;
    Some((a, b))
}

fn short(date: &str) -> Option<String> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    Some(format!("{}. {}.", d.month(), d.day()))
}

/// Default filename, matching how the forms are named: the Monday *after* the
/// week being reported, since the report is handed in at the start of the next
/// week. e.g. a 9/7~9/11 report is "9월 14일 주간업무 일지.hwpx".
pub fn suggested_filename(anchor: &str) -> String {
    let due = monday(anchor) + Duration::days(7);
    format!("{}월 {}일 주간업무 일지.hwpx", due.month(), due.day())
}

pub fn preview(cfg: &Config, anchor: &str) -> Vec<OutlineLine> {
    outline::build(cfg, anchor)
}

pub fn write(cfg: &Config, anchor: &str, lines: &[OutlineLine], dest: &str) -> Result<(), String> {
    let template = load_template()?;
    let bytes = hwpx::fill(
        &template,
        &header_text(cfg, anchor),
        &next_header_text(cfg, anchor),
        lines,
    )?;
    std::fs::write(dest, bytes).map_err(|e| format!("{dest} 에 저장할 수 없습니다: {e}"))
}
