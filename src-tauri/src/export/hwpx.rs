//! Filling the 한글 report template.
//!
//! A `.hwpx` is a zip of XML (OWPML), so the document is produced by swapping
//! the contents of two table cells and rezipping — no COM, no 한글 launched, no
//! security prompt. The binary `.hwp` format offers none of that: automating it
//! means driving 한글 itself, which pops a file-access dialog on every run.
//!
//! Two things learned the hard way, both verified by opening the output:
//!
//! - Paragraph and character style ids are per-document. Ids lifted from a
//!   filled example rendered as centred text in the blank template, because 62
//!   means something else there. They are read back from the template instead.
//! - `<hp:linesegarray>` is a layout cache. Emitting one with made-up positions
//!   makes 한글 draw every line at the same height, overlapping. Omitting it
//!   entirely makes 한글 recompute on open, which is what we want.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use quick_xml::events::Event;
use quick_xml::Reader;

use super::outline::{Level, OutlineLine};

const SECTION: &str = "Contents/section0.xml";

/// Style ids for one outline level, as used by this particular template.
#[derive(Debug, Clone, Copy)]
pub struct LevelStyle {
    pub para_pr: u32,
    pub char_pr: u32,
}

#[derive(Debug, Clone)]
pub struct TemplateStyles {
    pub header: LevelStyle,
    pub heading: LevelStyle,
    pub item: LevelStyle,
    pub detail: LevelStyle,
}

/// Fallbacks matching the template this was built against, used only when a
/// level's sample line can't be found.
impl Default for TemplateStyles {
    fn default() -> Self {
        Self {
            header: LevelStyle { para_pr: 20, char_pr: 64 },
            heading: LevelStyle { para_pr: 67, char_pr: 78 },
            item: LevelStyle { para_pr: 70, char_pr: 80 },
            detail: LevelStyle { para_pr: 68, char_pr: 63 },
        }
    }
}

/// Byte range of one cell's paragraph list, plus the styles seen inside it.
#[derive(Debug, Default)]
struct CellScan {
    /// Inner byte range of the cell's outermost `<hp:subList>`.
    content: Option<(usize, usize)>,
    /// (paraPr, charPr, leading text) per paragraph, in order.
    paragraphs: Vec<(u32, u32, String)>,
}

/// `next_header` labels the untouched right-hand column. The template ships
/// with a worked example in it (a city-government contract), which must not go
/// out inside someone's report — so that cell is emptied and only its heading
/// date range is kept current.
impl CellScan {
    /// Style of this cell's first paragraph, so a replacement keeps the look
    /// the template gave it.
    fn first_style(&self, fallback: LevelStyle) -> LevelStyle {
        self.paragraphs
            .first()
            .map(|(para_pr, char_pr, _)| LevelStyle { para_pr: *para_pr, char_pr: *char_pr })
            .unwrap_or(fallback)
    }
}

pub fn fill(
    template: &[u8],
    header_text: &str,
    next_header: &str,
    lines: &[OutlineLine],
) -> Result<Vec<u8>, String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(template)).map_err(|e| format!("템플릿을 열 수 없습니다: {e}"))?;

    let mut entries: Vec<(String, Vec<u8>, bool)> = Vec::new();
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("템플릿 항목을 읽을 수 없습니다: {e}"))?;
        let name = f.name().to_string();
        let stored = name == "mimetype";
        let mut data = Vec::new();
        f.read_to_end(&mut data)
            .map_err(|e| format!("{name} 를 읽을 수 없습니다: {e}"))?;
        entries.push((name, data, stored));
    }

    let section = entries
        .iter()
        .find(|(n, _, _)| n == SECTION)
        .map(|(_, d, _)| d.clone())
        .ok_or_else(|| format!("템플릿에 {SECTION} 이 없습니다"))?;
    let xml = String::from_utf8(section).map_err(|e| format!("{SECTION} 이 UTF-8이 아닙니다: {e}"))?;

    let cells = scan_cells(&xml)?;
    let styles = derive_styles(&cells[0][0], &cells[1][0]);

    let mut edits: Vec<((usize, usize), String)> = Vec::new();
    let mut push = |cell: &CellScan, xml: String| {
        if let Some(range) = cell.content {
            edits.push((range, xml));
        }
    };
    push(&cells[0][0], paragraph(styles.header, header_text));
    push(&cells[1][0], render_body(&styles, lines));
    push(&cells[0][1], paragraph(cells[0][1].first_style(styles.header), next_header));
    push(&cells[1][1], paragraph(styles.item, "ㅇ "));

    if !edits.iter().any(|((s, _), _)| *s > 0) {
        return Err("템플릿에서 표의 칸을 찾지 못했습니다".into());
    }

    // Apply from the end so earlier offsets stay valid.
    edits.sort_by_key(|((s, _), _)| std::cmp::Reverse(*s));
    let mut out = xml.clone();
    for ((s, e), replacement) in edits {
        out.replace_range(s..e, &replacement);
    }

    let mut buf = Cursor::new(Vec::new());
    {
        let mut zw = zip::ZipWriter::new(&mut buf);
        for (name, data, stored) in &entries {
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(if *stored {
                    zip::CompressionMethod::Stored
                } else {
                    zip::CompressionMethod::Deflated
                });
            zw.start_file(name, opts)
                .map_err(|e| format!("{name} 기록 실패: {e}"))?;
            let payload: &[u8] = if name == SECTION { out.as_bytes() } else { data };
            zw.write_all(payload)
                .map_err(|e| format!("{name} 기록 실패: {e}"))?;
        }
        zw.finish().map_err(|e| format!("압축 마무리 실패: {e}"))?;
    }
    Ok(buf.into_inner())
}

/// Finds the four cells of the outer table, as `[row][col]`.
///
/// Depth matters: the right-hand cell contains a nested table, so counting
/// `<hp:tr>` or `<hp:tc>` without tracking which table they belong to picks up
/// rows from inside it. Only elements at table depth 1 are considered.
fn scan_cells(xml: &str) -> Result<[[CellScan; 2]; 2], String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut tbl_depth = 0usize;
    let mut row = 0usize;
    let mut col = 0usize;
    let mut in_target = false;
    let mut sublist_depth = 0usize;

    let mut cells: [[CellScan; 2]; 2] = Default::default();

    // Paragraph being read inside a target cell.
    let mut cur_para: Option<(u32, u32, String)> = None;
    let mut in_text = false;

    loop {
        let before = reader.buffer_position() as usize;
        let ev = reader
            .read_event()
            .map_err(|e| format!("section0.xml 파싱 실패: {e}"))?;
        let after = reader.buffer_position() as usize;

        match ev {
            Event::Eof => break,
            Event::Start(ref e) => {
                match e.name().as_ref() {
                    b"hp:tbl" => {
                        tbl_depth += 1;
                        if tbl_depth == 1 {
                            row = 0;
                        }
                    }
                    b"hp:tr" if tbl_depth == 1 => {
                        row += 1;
                        col = 0;
                    }
                    b"hp:tc" if tbl_depth == 1 => {
                        col += 1;
                        in_target = (1..=2).contains(&row) && (1..=2).contains(&col);
                        sublist_depth = 0;
                    }
                    b"hp:subList" if in_target => {
                        sublist_depth += 1;
                        if sublist_depth == 1 {
                            cells[row - 1][col - 1].content = Some((after, after));
                        }
                    }
                    b"hp:p" if in_target && sublist_depth == 1 => {
                        cur_para = Some((attr_u32(e, b"paraPrIDRef"), 0, String::new()));
                    }
                    b"hp:run" if cur_para.is_some() => {
                        if let Some(p) = cur_para.as_mut() {
                            if p.1 == 0 {
                                p.1 = attr_u32(e, b"charPrIDRef");
                            }
                        }
                    }
                    b"hp:t" if cur_para.is_some() => in_text = true,
                    _ => {}
                }
            }
            Event::Text(ref t) if in_text => {
                if let Some(p) = cur_para.as_mut() {
                    p.2.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Event::End(ref e) => match e.name().as_ref() {
                b"hp:t" => in_text = false,
                b"hp:p" if in_target && sublist_depth == 1 => {
                    if let Some(p) = cur_para.take() {
                        cells[row - 1][col - 1].paragraphs.push(p);
                    }
                }
                b"hp:subList" if in_target => {
                    if sublist_depth == 1 {
                        let slot = &mut cells[row - 1][col - 1];
                        if let Some((s, _)) = slot.content {
                            slot.content = Some((s, before));
                        }
                    }
                    sublist_depth = sublist_depth.saturating_sub(1);
                }
                b"hp:tc" if tbl_depth == 1 => in_target = false,
                b"hp:tbl" => tbl_depth = tbl_depth.saturating_sub(1),
                _ => {}
            },
            _ => {}
        }
    }

    Ok(cells)
}

fn attr_u32(e: &quick_xml::events::BytesStart, key: &[u8]) -> u32 {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Reads the level styles back out of the template's own sample lines, so a
/// template whose styles were renumbered still exports correctly.
fn derive_styles(header: &CellScan, body: &CellScan) -> TemplateStyles {
    let mut styles = TemplateStyles::default();

    if let Some((para_pr, char_pr, _)) = header.paragraphs.first() {
        styles.header = LevelStyle { para_pr: *para_pr, char_pr: *char_pr };
    }

    let mut seen: HashMap<Level, LevelStyle> = HashMap::new();
    for (para_pr, char_pr, text) in &body.paragraphs {
        let t = text.trim_start();
        let level = if t.starts_with('ㅇ') {
            Level::Item
        } else if t.starts_with('-') {
            Level::Detail
        } else if t.starts_with("#.") || t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            Level::Heading
        } else {
            continue;
        };
        seen.entry(level)
            .or_insert(LevelStyle { para_pr: *para_pr, char_pr: *char_pr });
    }
    if let Some(s) = seen.get(&Level::Heading) {
        styles.heading = *s;
    }
    if let Some(s) = seen.get(&Level::Item) {
        styles.item = *s;
    }
    if let Some(s) = seen.get(&Level::Detail) {
        styles.detail = *s;
    }
    styles
}

fn render_body(styles: &TemplateStyles, lines: &[OutlineLine]) -> String {
    let mut out = String::new();
    let mut heading_no = 0usize;
    for line in lines {
        let (style, text) = match line.level {
            Level::Heading => {
                heading_no += 1;
                (styles.heading, super::outline::render(line, heading_no))
            }
            Level::Item => (styles.item, super::outline::render(line, heading_no)),
            Level::Detail => (styles.detail, super::outline::render(line, heading_no)),
        };
        out.push_str(&paragraph(style, &text));
    }
    if out.is_empty() {
        out.push_str(&paragraph(styles.item, "ㅇ "));
    }
    out
}

/// One paragraph. No `<hp:linesegarray>` — see the module comment.
fn paragraph(style: LevelStyle, text: &str) -> String {
    format!(
        "<hp:p id=\"0\" paraPrIDRef=\"{}\" styleIDRef=\"0\" pageBreak=\"0\" columnBreak=\"0\" \
         merged=\"0\"><hp:run charPrIDRef=\"{}\"><hp:t>{}</hp:t></hp:run></hp:p>",
        style.para_pr,
        style.char_pr,
        escape(text)
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
fn left_body_range(xml: &str) -> (usize, usize) {
    scan_cells(xml).unwrap()[1][0]
        .content
        .expect("본문 칸을 찾지 못했습니다")
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::data::config::Config;
    use crate::export::outline;
    use std::path::Path;

    /// Fills the real template with the real week of logs and writes the result
    /// next to it. Skipped when `refs/` is absent (it is gitignored, so a fresh
    /// clone has no template).
    #[test]
    fn fills_the_real_template() {
        let template_path = Path::new("../refs/0월 0일 주간업무 일지.hwpx");
        if !template_path.exists() {
            eprintln!("refs/ 템플릿 없음 - 건너뜀");
            return;
        }
        let template = std::fs::read(template_path).unwrap();

        let mut cfg = Config::default();
        cfg.log_dir = Some(
            std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
                .join("DailyWorkAlter")
                .join("logs")
                .to_string_lossy()
                .to_string(),
        );

        let lines = outline::build(&cfg, "2026-09-07");
        assert!(!lines.is_empty(), "지난주 기록이 비어 있음");

        let header = crate::export::header_text(&cfg, "2026-09-07");
        assert_eq!(header, "지난주 실적 (9. 7. ~ 9. 11.)");

        let next = crate::export::next_header_text(&cfg, "2026-09-07");
        let out = fill(&template, &header, &next, &lines).expect("fill 실패");

        // Valid archive, mimetype still stored uncompressed.
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(out.clone())).expect("zip 손상");
        assert_eq!(
            z.by_name("mimetype").unwrap().compression(),
            zip::CompressionMethod::Stored
        );

        let mut xml = String::new();
        use std::io::Read;
        z.by_name("Contents/section0.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("지난주 실적 (9. 7. ~ 9. 11.)"));

        // The paragraphs we generate must carry no linesegarray, or 한글 draws
        // them all at the same height. Paragraphs we did not touch (the title,
        // the right-hand column) keep theirs, so only the replaced cell counts.
        let cells = scan_cells(&xml).unwrap();
        let (bs, be) = left_body_range(&xml);
        assert!(
            !xml[bs..be].contains("linesegarray"),
            "생성한 문단에 linesegarray 가 남아 있으면 글자가 겹친다"
        );
        assert!(cells[1][0].content.is_some());
        assert!(!xml.contains("인쇄소모품"), "템플릿 예시가 남아 있으면 안 된다");

        let heading_count = lines
            .iter()
            .filter(|l| l.level == outline::Level::Heading)
            .count();
        eprintln!(
            "문단 {}개 (제목 {}개), 출력 {} bytes",
            lines.len(),
            heading_count,
            out.len()
        );

        // Somewhere inspectable, but not inside the repo.
        let dump = std::env::temp_dir().join("dailyworkalter_export_test.hwpx");
        std::fs::write(&dump, &out).unwrap();
        eprintln!("확인용 출력: {}", dump.display());
    }
}
