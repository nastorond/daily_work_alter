use crate::data::config::Config;
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// ── Layout ──────────────────────────────────────────────────────────────────
//
// One markdown file per week, days inside it:
//
//   logs/2026년 45주차 (11-02 ~ 11-06).md
//
//     ## 2026-11-02 (월)
//     #### 태양광
//     - ...
//
//     ## 2026-11-03 (화)
//     ...
//
// It used to be one file per day, which piled up ~250 files a year in a folder
// people do open by hand — and when they do, it is to read a week. The day
// marker carries the full date so it can't be confused with the `###`/`####`
// project headings written in the body, or with an ordinary `## 메모` line.

pub fn log_dir(cfg: &Config) -> PathBuf {
    match &cfg.log_dir {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::data::config::app_data_dir().join("logs"),
    }
}

/// Where the pre-weekly daily files are moved to, inside the log folder.
pub const DAILY_BACKUP_DIR: &str = "_이전_일별";

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

/// Monday of the week containing `anchor`.
pub fn week_monday(anchor: NaiveDate) -> NaiveDate {
    let offset = anchor.weekday().num_days_from_monday();
    anchor - Duration::days(offset as i64)
}

/// "2026년 45주차" — the part of a week file's name that identifies the week.
/// ISO week numbering, so the last days of December can belong to week 53 of
/// the year they started in, and the name says so.
fn week_key(monday: NaiveDate) -> String {
    let iso = monday.iso_week();
    format!("{}년 {:02}주차", iso.year(), iso.week())
}

fn week_file_name(monday: NaiveDate) -> String {
    let friday = monday + Duration::days(4);
    format!(
        "{} ({} ~ {}).md",
        week_key(monday),
        monday.format("%m-%d"),
        friday.format("%m-%d")
    )
}

/// The file holding `monday`'s week. Matched on the "YYYY년 WW주차" prefix
/// rather than the exact name, so a file that was renamed by hand (say, to fix
/// the date range) is still found instead of silently starting a second one.
pub fn week_path(cfg: &Config, monday: NaiveDate) -> PathBuf {
    let dir = log_dir(cfg);
    let key = week_key(monday);
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&key) && name.ends_with(".md") {
                return entry.path();
            }
        }
    }
    dir.join(week_file_name(monday))
}

/// Parses "2026년 45주차 …" back to that week's Monday.
fn monday_from_file_name(name: &str) -> Option<NaiveDate> {
    let stem = name.strip_suffix(".md")?;
    let (year, rest) = stem.split_once("년 ")?;
    let week = rest.split("주차").next()?;
    NaiveDate::from_isoywd_opt(year.trim().parse().ok()?, week.trim().parse().ok()?, Weekday::Mon)
}

// ── Lines ───────────────────────────────────────────────────────────────────

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
/// the history headings.
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

// ── Week file ───────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct WeekFile {
    /// Anything written above the first day marker, kept as is.
    preamble: Vec<String>,
    days: BTreeMap<NaiveDate, Vec<String>>,
}

/// `## 2026-11-02 (월)` → that date. Anything else is body text.
fn day_marker(line: &str) -> Option<NaiveDate> {
    let rest = line.strip_prefix("## ")?;
    NaiveDate::parse_from_str(rest.get(..10)?, "%Y-%m-%d").ok()
}

fn parse_week(content: &str) -> WeekFile {
    let mut wf = WeekFile::default();
    let mut current: Option<NaiveDate> = None;
    let mut raw: BTreeMap<NaiveDate, Vec<&str>> = BTreeMap::new();

    for line in content.lines() {
        if let Some(d) = day_marker(line.trim_end()) {
            current = Some(d);
            raw.entry(d).or_default();
            continue;
        }
        match current {
            Some(d) => raw.entry(d).or_default().push(line),
            None => wf.preamble.push(line.trim_end().to_string()),
        }
    }

    while wf.preamble.first().is_some_and(|l| l.is_empty()) {
        wf.preamble.remove(0);
    }
    while wf.preamble.last().is_some_and(|l| l.is_empty()) {
        wf.preamble.pop();
    }
    for (d, lines) in raw {
        wf.days.insert(d, parse_body(&lines.join("\n")));
    }
    wf
}

fn render_week(wf: &WeekFile) -> String {
    let mut out = String::new();
    if !wf.preamble.is_empty() {
        out.push_str(&wf.preamble.join("\n"));
        out.push_str("\n\n");
    }
    for (d, items) in &wf.days {
        if items.is_empty() {
            continue;
        }
        out.push_str(&format!("## {} ({})\n", d.format("%Y-%m-%d"), weekday_kr(*d)));
        for line in items {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }
    let trimmed = out.trim_end();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

fn load_week(cfg: &Config, monday: NaiveDate) -> (PathBuf, WeekFile) {
    let path = week_path(cfg, monday);
    let wf = fs::read_to_string(&path)
        .map(|c| parse_week(&c))
        .unwrap_or_default();
    (path, wf)
}

/// Writes through a temp file and a rename, so a crash mid-save can't leave a
/// half-written week behind — one file now holds five days, not one. If the
/// rename is refused (the file open in another program), it falls back to a
/// direct write rather than losing the save.
fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, content)?;
    if fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
        fs::write(path, content)?;
    }
    Ok(())
}

/// Saves a week, or removes its file once nothing is left in it.
fn save_week(path: &Path, wf: &WeekFile) -> std::io::Result<()> {
    let content = render_week(wf);
    if content.is_empty() {
        if path.exists() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    write_atomic(path, &content)
}

// ── Days ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub date: String,
    pub items: Vec<String>,
    pub exists: bool,
}

fn parse_date(date: &str) -> NaiveDate {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap_or_else(|_| Local::now().date_naive())
}

pub fn read_log(cfg: &Config, date: &str) -> LogEntry {
    let d = parse_date(date);
    let (_, wf) = load_week(cfg, week_monday(d));
    match wf.days.get(&d) {
        Some(items) => LogEntry { date: date.to_string(), items: items.clone(), exists: true },
        None => LogEntry { date: date.to_string(), items: vec![], exists: false },
    }
}

/// Whether a day has anything recorded in it.
pub fn log_has_content(cfg: &Config, date: &str) -> bool {
    let entry = read_log(cfg, date);
    entry.exists && !entry.items.is_empty()
}

pub fn write_log(cfg: &Config, date: &str, items: &[String]) -> std::io::Result<()> {
    let d = parse_date(date);
    let (path, mut wf) = load_week(cfg, week_monday(d));
    let lines: Vec<String> = items
        .iter()
        .map(|i| i.trim_end().to_string())
        .filter(|l| !split_depth(l).1.is_empty())
        .collect();
    if lines.is_empty() {
        wf.days.remove(&d);
    } else {
        wf.days.insert(d, lines);
    }
    save_week(&path, &wf)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayLog {
    pub date: String,
    pub weekday: String,
    pub items: Vec<String>,
    pub is_today: bool,
}

/// The working days of the week containing `anchor`, oldest first. Days
/// marked off in `holidays` are left out unless something was recorded on them.
pub fn read_week(cfg: &Config, anchor: &str) -> Vec<DayLog> {
    let monday = week_monday(parse_date(anchor));
    let (_, wf) = load_week(cfg, monday);
    let today = Local::now().date_naive();
    (0..7i64)
        .filter_map(|i| {
            let d = monday + Duration::days(i);
            let iso = d.weekday().number_from_monday() as u8;
            if !cfg.work.workdays.contains(&iso) {
                return None;
            }
            let items = wf.days.get(&d).cloned().unwrap_or_default();
            // A day off drops out of the week — unless something was written on
            // it anyway, which should stay visible rather than vanish.
            if cfg.is_holiday(d) && items.is_empty() {
                return None;
            }
            Some(DayLog {
                date: d.format("%Y-%m-%d").to_string(),
                weekday: weekday_kr(d).to_string(),
                items,
                is_today: d == today,
            })
        })
        .collect()
}

// ── Migration from daily files ──────────────────────────────────────────────

/// Splits a lenient frontmatter block off the front of an old daily file.
/// If the file doesn't start with `---`, the whole content is the body.
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

/// Folds the old one-file-per-day logs (`2026-11-02.md`) into week files, then
/// moves the daily files into `_이전_일별` rather than deleting them, so the
/// change can be undone by hand. Runs at startup and does nothing once there
/// are no daily files left.
///
/// Week files are all written before any daily file is moved: if this is
/// interrupted, the daily files are still in place and the next run simply
/// repeats it. Where a week file already has a day, it wins — it is the newer
/// copy.
///
/// Returns how many daily files were moved.
pub fn migrate_daily_files(cfg: &Config) -> std::io::Result<usize> {
    let dir = log_dir(cfg);
    let mut daily: Vec<(NaiveDate, PathBuf)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(stem) = name.strip_suffix(".md") else { continue };
            if let Ok(d) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                daily.push((d, entry.path()));
            }
        }
    }
    if daily.is_empty() {
        return Ok(0);
    }
    daily.sort();

    let mut weeks: BTreeMap<NaiveDate, Vec<(NaiveDate, Vec<String>)>> = BTreeMap::new();
    for (d, path) in &daily {
        let content = fs::read_to_string(path)?;
        let (_, body) = split_frontmatter(&content);
        weeks.entry(week_monday(*d)).or_default().push((*d, parse_body(body)));
    }

    for (monday, days) in weeks {
        let (path, mut wf) = load_week(cfg, monday);
        for (d, items) in days {
            let already = wf.days.get(&d).is_some_and(|v| !v.is_empty());
            if !items.is_empty() && !already {
                wf.days.insert(d, items);
            }
        }
        save_week(&path, &wf)?;
    }

    let backup = dir.join(DAILY_BACKUP_DIR);
    fs::create_dir_all(&backup)?;
    for (_, path) in &daily {
        let name = path.file_name().unwrap_or_default();
        let mut dest = backup.join(name);
        let mut n = 1;
        while dest.exists() {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            dest = backup.join(format!("{stem}-{n}.md"));
            n += 1;
        }
        fs::rename(path, &dest)?;
    }
    Ok(daily.len())
}

// ── History ─────────────────────────────────────────────────────────────────

/// One day's slot in the history list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaySlot {
    pub date: String,
    pub filled: bool,
}

/// A week as shown in "이전 기록": enough to pick it out of a list without
/// opening it — which days have entries, and what the week was about.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekSummary {
    /// Monday of the week, the anchor the week view opens with.
    pub monday: String,
    pub year: i32,
    pub week: u32,
    pub days: Vec<DaySlot>,
    /// Workdays of this week that were marked off.
    pub holidays: usize,
    /// The `#` headings written that week, first-seen order, deduplicated.
    /// They name what the week was about, which is how a past week gets looked
    /// for — "the 태양광 week", not "week 41".
    pub headings: Vec<String>,
}

const MAX_HEADINGS: usize = 4;

/// Every week that has a file, newest first, plus the current week even if
/// nothing is written yet. Future weeks are left out: early test runs created
/// empty files ahead of time, and those are not history.
pub fn list_weeks(cfg: &Config) -> Vec<WeekSummary> {
    let this_monday = week_monday(Local::now().date_naive());
    let mut mondays = std::collections::BTreeSet::new();
    mondays.insert(this_monday);

    if let Ok(entries) = fs::read_dir(log_dir(cfg)) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(m) = monday_from_file_name(&name) {
                if m <= this_monday {
                    mondays.insert(m);
                }
            }
        }
    }

    mondays.into_iter().rev().map(|m| summarize_week(cfg, m)).collect()
}

fn summarize_week(cfg: &Config, monday: NaiveDate) -> WeekSummary {
    let anchor = monday.format("%Y-%m-%d").to_string();
    let days = read_week(cfg, &anchor);

    let holidays = (0..7i64)
        .map(|i| monday + Duration::days(i))
        .filter(|d| {
            let iso = d.weekday().number_from_monday() as u8;
            cfg.work.workdays.contains(&iso) && cfg.is_holiday(*d)
        })
        .count();

    let mut headings: Vec<String> = Vec::new();
    for day in &days {
        for item in &day.items {
            let text = split_depth(item).1;
            if let Some(rest) = text.strip_prefix('#') {
                let h = rest.trim_start_matches('#').trim().to_string();
                if !h.is_empty() && !headings.contains(&h) {
                    headings.push(h);
                }
            }
        }
    }
    headings.truncate(MAX_HEADINGS);

    let iso = monday.iso_week();
    WeekSummary {
        monday: anchor,
        year: iso.year(),
        week: iso.week(),
        days: days
            .iter()
            .map(|d| DaySlot { date: d.date.clone(), filled: !d.items.is_empty() })
            .collect(),
        holidays,
        headings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(body: &str) -> Vec<String> {
        parse_body(body)
    }

    fn temp_cfg(tag: &str) -> Config {
        let dir = std::env::temp_dir().join(format!("dwa_storage_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = Config::default();
        cfg.log_dir = Some(dir.to_string_lossy().to_string());
        cfg
    }

    fn files_in(cfg: &Config) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(log_dir(cfg))
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
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

    /// What is typed is what is written: no dash appears on a line that had none.
    #[test]
    fn write_then_read_round_trips_verbatim() {
        let cfg = temp_cfg("verbatim");
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

    #[test]
    fn a_week_is_one_file_with_days_in_date_order() {
        let cfg = temp_cfg("one_file");
        write_log(&cfg, "2026-11-03", &["#### I3View".into(), "- 분석".into()]).unwrap();
        write_log(&cfg, "2026-11-02", &["#### 태양광".into(), "- 이관".into()]).unwrap();

        assert_eq!(files_in(&cfg), vec!["2026년 45주차 (11-02 ~ 11-06).md"]);
        let text = std::fs::read_to_string(week_path(&cfg, parse_date("2026-11-02"))).unwrap();
        assert_eq!(
            text,
            "## 2026-11-02 (월)\n#### 태양광\n- 이관\n\n## 2026-11-03 (화)\n#### I3View\n- 분석\n"
        );
        // Rewriting one day leaves the other alone.
        write_log(&cfg, "2026-11-02", &["- 바뀜".into()]).unwrap();
        assert_eq!(read_log(&cfg, "2026-11-03").items, vec!["#### I3View", "- 분석"]);
    }

    #[test]
    fn single_digit_weeks_are_zero_padded_so_they_sort() {
        let cfg = temp_cfg("padding");
        write_log(&cfg, "2026-01-07", &["가".into()]).unwrap();
        assert_eq!(files_in(&cfg), vec!["2026년 02주차 (01-05 ~ 01-09).md"]);
    }

    #[test]
    fn emptying_a_day_drops_it_and_an_empty_week_drops_the_file() {
        let cfg = temp_cfg("emptying");
        write_log(&cfg, "2026-11-02", &["가".into()]).unwrap();
        write_log(&cfg, "2026-11-03", &["나".into()]).unwrap();
        write_log(&cfg, "2026-11-02", &[]).unwrap();
        assert!(!read_log(&cfg, "2026-11-02").exists);
        write_log(&cfg, "2026-11-03", &["- ".into()]).unwrap();
        assert!(files_in(&cfg).is_empty());
    }

    /// `## 메모` in the body is text; only a full-date `##` line starts a day.
    /// Notes written above the first day are kept through a rewrite.
    #[test]
    fn hand_edits_to_a_week_file_survive_a_save() {
        let cfg = temp_cfg("hand_edits");
        let path = week_path(&cfg, parse_date("2026-11-02"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "이번 주 메모\n\n## 2026-11-02 (월)\n## 메모\n- 가\n").unwrap();

        assert_eq!(read_log(&cfg, "2026-11-02").items, vec!["## 메모", "- 가"]);
        write_log(&cfg, "2026-11-03", &["나".into()]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("이번 주 메모\n\n## 2026-11-02 (월)\n## 메모\n- 가\n"));
    }

    /// A week file renamed by hand is still the one that is read and written.
    #[test]
    fn a_renamed_week_file_is_still_found() {
        let cfg = temp_cfg("renamed");
        write_log(&cfg, "2026-11-02", &["가".into()]).unwrap();
        let old = week_path(&cfg, parse_date("2026-11-02"));
        let renamed = old.with_file_name("2026년 45주차 (내가 바꾼 이름).md");
        std::fs::rename(&old, &renamed).unwrap();
        write_log(&cfg, "2026-11-03", &["나".into()]).unwrap();
        assert_eq!(files_in(&cfg), vec!["2026년 45주차 (내가 바꾼 이름).md"]);
    }

    #[test]
    fn daily_files_are_folded_into_weeks_and_kept_as_backup() {
        let cfg = temp_cfg("migrate");
        let dir = log_dir(&cfg);
        std::fs::create_dir_all(&dir).unwrap();
        let old = |name: &str, body: &str| {
            std::fs::write(
                dir.join(name),
                format!("---\ndate: x\ncreated: y\n---\n\n{body}"),
            )
            .unwrap();
        };
        old("2026-11-02.md", "- ### 태양광\n- 이관\n");
        old("2026-11-03.md", "#### I3View\n");
        old("2026-11-04.md", ""); // an empty stub
        old("2026-10-27.md", "- 지난주\n");
        // Already in a week file: that copy is newer and wins.
        write_log(&cfg, "2026-11-03", &["#### I3View".into(), "- 최신".into()]).unwrap();

        assert_eq!(migrate_daily_files(&cfg).unwrap(), 4);

        assert_eq!(read_log(&cfg, "2026-11-02").items, vec!["### 태양광", "- 이관"]);
        assert_eq!(read_log(&cfg, "2026-11-03").items, vec!["#### I3View", "- 최신"]);
        assert_eq!(read_log(&cfg, "2026-10-27").items, vec!["- 지난주"]);
        assert!(!read_log(&cfg, "2026-11-04").exists);

        let top: Vec<String> = files_in(&cfg);
        assert_eq!(
            top,
            vec![
                "2026년 44주차 (10-26 ~ 10-30).md",
                "2026년 45주차 (11-02 ~ 11-06).md",
                DAILY_BACKUP_DIR,
            ]
        );
        let backup = std::fs::read_dir(dir.join(DAILY_BACKUP_DIR)).unwrap().count();
        assert_eq!(backup, 4);

        // Nothing left to do on the next start.
        assert_eq!(migrate_daily_files(&cfg).unwrap(), 0);
    }

    #[test]
    fn weeks_are_listed_newest_first_with_their_headings() {
        let mut cfg = temp_cfg("weeks");
        cfg.holidays = vec!["2026-10-09".into()];

        write_log(&cfg, "2026-10-06", &["### 태양광".into(), "- 이관".into()]).unwrap();
        write_log(&cfg, "2026-10-07", &["### 태양광".into(), "### L3View".into()]).unwrap();
        write_log(&cfg, "2026-09-28", &["평문".into()]).unwrap();
        // A file dated in the future (an early test stub) is not history.
        write_log(&cfg, "2099-01-05", &["미래".into()]).unwrap();

        let weeks = list_weeks(&cfg);
        let mondays: Vec<&str> = weeks.iter().map(|w| w.monday.as_str()).collect();
        assert!(!mondays.contains(&"2099-01-05"));
        let i = mondays.iter().position(|m| *m == "2026-10-05").unwrap();
        let j = mondays.iter().position(|m| *m == "2026-09-28").unwrap();
        assert!(i < j, "newest first");

        let w = &weeks[i];
        assert_eq!(w.week, 41);
        assert_eq!(w.headings, vec!["태양광", "L3View"]);
        assert_eq!(w.holidays, 1);
        // The holiday Friday drops out; Mon..Thu remain, two of them written.
        assert_eq!(w.days.len(), 4);
        assert_eq!(w.days.iter().filter(|d| d.filled).count(), 2);
    }
}
