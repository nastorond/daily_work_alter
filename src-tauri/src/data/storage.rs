use crate::data::config::Config;
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

pub fn log_dir(cfg: &Config) -> PathBuf {
    match &cfg.log_dir {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::data::config::app_data_dir().join("logs"),
    }
}

pub fn log_path(cfg: &Config, date: &str) -> PathBuf {
    log_dir(cfg).join(format!("{date}.md"))
}

pub fn weekday_kr(nd: NaiveDate) -> &'static str {
    match nd.weekday() {
        Weekday::Mon => "월",
        Weekday::Tue => "화",
        Weekday::Wed => "수",
        Weekday::Thu => "목",
        Weekday::Fri => "금",
        Weekday::Sat => "토",
        Weekday::Sun => "일",
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub date: String,
    pub items: Vec<String>,
    pub exists: bool,
}

/// Splits a lenient frontmatter block off the front of a log file's content.
/// If the file doesn't start with `---`, the whole content is treated as the body.
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    if let Some(rest) = content.strip_prefix("---") {
        let rest = rest.trim_start_matches(['\r', '\n']);
        if let Some(idx) = rest.find("\n---") {
            let fm = &rest[..idx];
            let after = &rest[idx + 4..];
            let after = after.trim_start_matches(['\r', '\n']);
            return (Some(fm), after);
        }
    }
    (None, content)
}

fn frontmatter_value<'a>(fm: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}:");
    fm.lines()
        .find_map(|l| l.trim_start().strip_prefix(prefix.as_str()))
        .map(|v| v.trim())
}

/// Indentation (two columns or more) marks a line as a child of the one above.
/// Only one level is distinguished; deeper indentation counts the same.
pub const CHILD_PREFIX: &str = "  ";

/// Items are stored **exactly as written** — one per line, indentation and any
/// leading `- ` kept as the user typed them.
///
/// This used to be normalised: every line was stripped of its dash on save and
/// had `- ` stamped back on when written, displayed or copied. That threw away
/// whether a line had a bullet at all, so plain lines came back with one, and a
/// heading typed as `### 학습` was saved as `- ### 학습`. Now nothing is added.
///
/// Splits a stored line into (is_child, text without indentation or bullet) for
/// the places that need the structure rather than the raw line — rendering and
/// the report export.
pub fn split_depth(item: &str) -> (bool, &str) {
    let is_child = indent_width(item) >= CHILD_PREFIX.len();
    let t = item.trim();
    let t = t.strip_prefix("- ").or_else(|| t.strip_prefix('-')).unwrap_or(t);
    (is_child, t.trim())
}

/// Leading-whitespace width of a line, counting a tab as two columns.
fn indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 2 } else { 1 })
        .sum()
}

/// Repairs what the old normaliser did to headings: it prefixed `- ` to every
/// line, so `### 학습` was saved as `- ### 학습`. A bullet in front of a markdown
/// heading is never something a person types, so it is safe to take back off.
fn repair_legacy_heading(line: &str) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);
    match rest.strip_prefix("- ") {
        Some(after) if after.starts_with('#') => format!("{indent}{after}"),
        _ => line.to_string(),
    }
}

/// Body lines as written, minus blank lines and bare bullets (`-` with nothing
/// after it, the leftover of an empty line in the old editor).
fn parse_body(body: &str) -> Vec<String> {
    body.lines()
        .map(|line| line.trim_end())
        .filter(|line| !split_depth(line).1.is_empty())
        .map(repair_legacy_heading)
        .collect()
}

pub fn read_log(cfg: &Config, date: &str) -> LogEntry {
    let path = log_path(cfg, date);
    match fs::read_to_string(&path) {
        Ok(content) => {
            let (_fm, body) = split_frontmatter(&content);
            LogEntry {
                date: date.to_string(),
                items: parse_body(body),
                exists: true,
            }
        }
        Err(_) => LogEntry {
            date: date.to_string(),
            items: vec![],
            exists: false,
        },
    }
}

/// Whether a day has anything recorded in it.
pub fn log_has_content(cfg: &Config, date: &str) -> bool {
    let entry = read_log(cfg, date);
    entry.exists && !entry.items.is_empty()
}

pub fn write_log(cfg: &Config, date: &str, items: &[String]) -> std::io::Result<()> {
    fs::create_dir_all(log_dir(cfg))?;
    let path = log_path(cfg, date);
    let now = Local::now();
    let nd = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap_or_else(|_| now.date_naive());
    let weekday = weekday_kr(nd);

    let created = fs::read_to_string(&path)
        .ok()
        .and_then(|existing| {
            let (fm, _) = split_frontmatter(&existing);
            fm.and_then(|f| frontmatter_value(f, "created").map(|v| v.to_string()))
        })
        .unwrap_or_else(|| now.to_rfc3339());

    let updated = now.to_rfc3339();
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("date: {date}\n"));
    out.push_str(&format!("weekday: {weekday}\n"));
    out.push_str(&format!("created: {created}\n"));
    out.push_str(&format!("updated: {updated}\n"));
    out.push_str("---\n\n");
    for item in items {
        let line = item.trim_end();
        if split_depth(line).1.is_empty() {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    fs::write(path, out)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayLog {
    pub date: String,
    pub weekday: String,
    pub items: Vec<String>,
    pub is_today: bool,
}

/// Monday of the week containing `anchor`.
pub fn week_monday(anchor: NaiveDate) -> NaiveDate {
    let offset = anchor.weekday().num_days_from_monday();
    anchor - Duration::days(offset as i64)
}

/// The working days of the week containing `anchor`, oldest first. Days
/// marked off in `holidays` are left out unless something was recorded on them.
pub fn read_week(cfg: &Config, anchor: &str) -> Vec<DayLog> {
    let anchor_date =
        NaiveDate::parse_from_str(anchor, "%Y-%m-%d").unwrap_or_else(|_| Local::now().date_naive());
    let monday = week_monday(anchor_date);
    let today = Local::now().date_naive();
    (0..7i64)
        .filter_map(|i| {
            let d = monday + Duration::days(i);
            let iso = d.weekday().number_from_monday() as u8;
            if !cfg.work.workdays.contains(&iso) {
                return None;
            }
            let date_str = d.format("%Y-%m-%d").to_string();
            let entry = read_log(cfg, &date_str);
            // A day off drops out of the week — unless something was written on
            // it anyway, which should stay visible rather than vanish.
            if cfg.is_holiday(d) && entry.items.is_empty() {
                return None;
            }
            Some(DayLog {
                date: date_str,
                weekday: weekday_kr(d).to_string(),
                items: entry.items,
                is_today: d == today,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(body: &str) -> Vec<String> {
        parse_body(body)
    }

    #[test]
    fn lines_are_kept_as_written() {
        assert_eq!(items("- 가\n나\n  - 다"), vec!["- 가", "나", "  - 다"]);
    }

    #[test]
    fn blank_and_bare_dash_lines_are_dropped() {
        assert_eq!(items("- 가\n\n-\n-   \n  - \n- 나"), vec!["- 가", "- 나"]);
    }

    #[test]
    fn a_heading_the_old_normaliser_bulleted_is_repaired() {
        assert_eq!(items("- ### 태양광\n- 내용"), vec!["### 태양광", "- 내용"]);
        // A heading typed without the dash is untouched.
        assert_eq!(items("#### 학습"), vec!["#### 학습"]);
    }

    #[test]
    fn split_depth_reads_indent_and_strips_the_bullet() {
        assert_eq!(split_depth("부모"), (false, "부모"));
        assert_eq!(split_depth("- 부모"), (false, "부모"));
        assert_eq!(split_depth("  - 자식"), (true, "자식"));
        assert_eq!(split_depth("  자식"), (true, "자식"));
        assert_eq!(split_depth("\t- 자식"), (true, "자식"));
        assert_eq!(split_depth("      - 손자"), (true, "손자"));
    }

    #[test]
    fn frontmatter_is_split_off_when_present() {
        let (fm, body) = split_frontmatter("---\ndate: 2026-09-07\n---\n\n- 가\n");
        assert!(fm.unwrap().contains("date: 2026-09-07"));
        assert_eq!(parse_body(body), vec!["- 가"]);
    }

    #[test]
    fn missing_frontmatter_treats_everything_as_body() {
        let (fm, body) = split_frontmatter("- 가\n나\n");
        assert!(fm.is_none());
        assert_eq!(parse_body(body), vec!["- 가", "나"]);
    }

    /// What is typed is what is written: no dash appears on a line that had none.
    #[test]
    fn write_then_read_round_trips_verbatim() {
        let dir = std::env::temp_dir().join("dwa_storage_verbatim");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = Config::default();
        cfg.log_dir = Some(dir.to_string_lossy().to_string());
        let typed = vec![
            "### 학습".to_string(),
            "평문 한 줄".to_string(),
            "- 불릿 줄".to_string(),
            "  - 하위 불릿".to_string(),
            "  하위 평문".to_string(),
        ];
        write_log(&cfg, "2026-10-22", &typed).unwrap();
        assert_eq!(read_log(&cfg, "2026-10-22").items, typed);
    }
}
