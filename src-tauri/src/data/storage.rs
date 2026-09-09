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

/// Nesting is carried inside the item string itself: a child item starts with
/// two spaces. Only one level is supported, so deeper indentation clamps to it.
/// Keeping items as plain strings means the Rust <-> JS command signatures and
/// the on-disk format stay unchanged.
pub const CHILD_PREFIX: &str = "  ";

/// Splits an item into (is_child, trimmed text).
pub fn split_depth(item: &str) -> (bool, &str) {
    match item.strip_prefix(CHILD_PREFIX) {
        Some(rest) => (true, rest.trim()),
        None => (false, item.trim()),
    }
}

/// Leading-whitespace width of a line, counting a tab as two columns.
fn indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 2 } else { 1 })
        .sum()
}

fn parse_body(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let indent = indent_width(line);
            let t = line.trim();
            if t.is_empty() {
                return None;
            }
            let stripped = t.strip_prefix("- ").or_else(|| t.strip_prefix('-')).unwrap_or(t);
            let stripped = stripped.trim();
            if stripped.is_empty() {
                return None;
            }
            Some(if indent >= CHILD_PREFIX.len() {
                format!("{CHILD_PREFIX}{stripped}")
            } else {
                stripped.to_string()
            })
        })
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
        let (is_child, text) = split_depth(item);
        if text.is_empty() {
            continue;
        }
        if is_child {
            out.push_str(CHILD_PREFIX);
        }
        out.push_str("- ");
        out.push_str(text);
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

/// All configured workdays in the week containing `anchor`, oldest first.
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
    fn flat_items_keep_no_indent() {
        assert_eq!(items("- 가\n- 나"), vec!["가", "나"]);
    }

    #[test]
    fn two_spaces_make_a_child() {
        assert_eq!(items("- 부모\n  - 자식"), vec!["부모", "  자식"]);
    }

    #[test]
    fn a_tab_counts_as_a_child() {
        assert_eq!(items("- 부모\n\t- 자식"), vec!["부모", "  자식"]);
    }

    #[test]
    fn deeper_indent_clamps_to_one_level() {
        assert_eq!(items("- 부모\n      - 손자"), vec!["부모", "  손자"]);
    }

    #[test]
    fn blank_and_bare_dash_lines_are_dropped() {
        assert_eq!(items("- 가\n\n-\n-   \n- 나"), vec!["가", "나"]);
    }

    #[test]
    fn bullet_prefix_is_optional() {
        assert_eq!(items("가\n  나"), vec!["가", "  나"]);
    }

    #[test]
    fn split_depth_strips_the_child_prefix() {
        assert_eq!(split_depth("부모"), (false, "부모"));
        assert_eq!(split_depth("  자식"), (true, "자식"));
    }

    #[test]
    fn frontmatter_is_split_off_when_present() {
        let (fm, body) = split_frontmatter("---\ndate: 2026-09-07\n---\n\n- 가\n");
        assert!(fm.unwrap().contains("date: 2026-09-07"));
        assert_eq!(parse_body(body), vec!["가"]);
    }

    #[test]
    fn missing_frontmatter_treats_everything_as_body() {
        let (fm, body) = split_frontmatter("- 가\n- 나\n");
        assert!(fm.is_none());
        assert_eq!(parse_body(body), vec!["가", "나"]);
    }
}
