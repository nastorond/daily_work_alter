//! Turning a week of log lines into the three-level outline the report uses.
//!
//! The report has `1. 제목` / `ㅇ 항목` / `- 세부`, but the daily box is written
//! flat — headings get typed as ordinary lines because stopping to indent at
//! 18:30 is exactly the friction this app exists to avoid. So the level is
//! guessed here and corrected in the export preview, rather than demanded at
//! write time.

use crate::data::config::Config;
use crate::data::storage::{self, CHILD_PREFIX};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    /// `1. 제목` — HY견고딕, the numbered grouping.
    Heading,
    /// `ㅇ 항목`
    Item,
    /// `- 세부`
    Detail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineLine {
    pub text: String,
    pub level: Level,
    /// The day this came from, so the preview can show where a line originated.
    pub date: String,
}

/// Lines that end like a sentence, or that are long, are content rather than a
/// heading. Digits usually mean a measurement, which is content too — but short
/// labels such as "130기 Gold 조립" are still headings, so the digit rule only
/// applies past a certain length.
fn looks_like_heading(text: &str) -> bool {
    const MAX_HEADING_CHARS: usize = 22;
    const DIGIT_TOLERANCE_CHARS: usize = 14;

    let chars = text.chars().count();
    if chars == 0 || chars > MAX_HEADING_CHARS {
        return false;
    }
    if let Some(last) = text.chars().last() {
        if ".·,—–:;)%".contains(last) {
            return false;
        }
    }
    if chars > DIGIT_TOLERANCE_CHARS && text.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    true
}

/// Strips the markdown that creeps in when logs are edited by hand.
fn strip_markdown(text: &str) -> String {
    let without_heading = text.trim_start_matches('#').trim_start();
    without_heading.replace("**", "").trim().to_string()
}

/// The week's workday entries, oldest first, with a level guessed for each line.
///
/// An indented log line is always a detail — the writer already said it belongs
/// under the line above. Only top-level lines are guessed at.
pub fn build(cfg: &Config, anchor: &str) -> Vec<OutlineLine> {
    let mut out = Vec::new();
    for day in storage::read_week(cfg, anchor) {
        for item in &day.items {
            let (is_child, text) = storage::split_depth(item);
            let text = strip_markdown(text);
            if text.is_empty() {
                continue;
            }
            let level = if is_child {
                Level::Detail
            } else if looks_like_heading(&text) {
                Level::Heading
            } else {
                Level::Item
            };
            out.push(OutlineLine {
                text,
                level,
                date: day.date.clone(),
            });
        }
    }
    out
}

/// Renders a line as it appears in the document, bullet included.
pub fn render(line: &OutlineLine, heading_number: usize) -> String {
    match line.level {
        Level::Heading => format!("{heading_number}. {}", line.text),
        Level::Item => format!("ㅇ {}", line.text),
        Level::Detail => format!("- {}", line.text),
    }
}

/// Unused today but kept next to `split_depth`'s convention so the two stay
/// visibly paired.
#[allow(dead_code)]
pub const INDENT: &str = CHILD_PREFIX;

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, level: Level) -> OutlineLine {
        OutlineLine {
            text: text.into(),
            level,
            date: "2026-09-08".into(),
        }
    }

    #[test]
    fn short_labels_are_headings() {
        assert!(looks_like_heading("감시 체계"));
        assert!(looks_like_heading("운영"));
        assert!(looks_like_heading("미래 예보 경로 완성"));
        assert!(looks_like_heading("130기 Gold 조립"));
    }

    #[test]
    fn sentences_are_not_headings() {
        assert!(!looks_like_heading(
            "감시 지표 DB 적재 개통. Grafana 3개 대시보드 전부 값 표출 시작"
        ));
        assert!(!looks_like_heading("검사 486건 통과, 커버리지 88.18%"));
        // Trailing punctuation alone is enough to disqualify a short line.
        assert!(!looks_like_heading("정리 완료."));
    }

    #[test]
    fn long_lines_with_numbers_are_content() {
        assert!(!looks_like_heading("포함률 0.682 → 0.779 확보"));
    }

    #[test]
    fn markdown_is_stripped() {
        assert_eq!(strip_markdown("## 진행 현황"), "진행 현황");
        assert_eq!(strip_markdown("**정확도 개선 요인 규명**"), "정확도 개선 요인 규명");
    }

    #[test]
    fn rendering_uses_the_report_bullets() {
        assert_eq!(render(&line("감시 체계", Level::Heading), 3), "3. 감시 체계");
        assert_eq!(render(&line("복구 리허설 통과", Level::Item), 1), "ㅇ 복구 리허설 통과");
        assert_eq!(render(&line("결함 4건 제거", Level::Detail), 1), "- 결함 4건 제거");
    }
}
