import { isoWeekNumber, shortLabel } from "./date.js";

/**
 * Lines are kept exactly as written — indentation, and a leading `- ` only if
 * the user typed one. Mirrors src-tauri/src/data/storage.rs.
 *
 * This used to normalise every line into a bullet: the dash was stripped on
 * save and `- ` was stamped back on for display, the file and the clipboard. So
 * a plain line came back with a dash, and `### 제목` turned into `- ### 제목`.
 * Nothing is added any more; these helpers only *read* structure out of a line.
 */
export const CHILD_PREFIX = "  ";

/** Leading-whitespace width of a line, counting a tab as two columns. */
function indentWidth(line) {
  let width = 0;
  for (const ch of line) {
    if (ch === " ") width += 1;
    else if (ch === "\t") width += 2;
    else break;
  }
  return width;
}

/** 1 when the line is indented under the one above, else 0. */
export function itemDepth(item) {
  return indentWidth(item) >= CHILD_PREFIX.length ? 1 : 0;
}

/** Whether the line was written with a leading bullet. */
export function hasBullet(item) {
  return /^-(\s|$)/.test(item.trim());
}

/** The line's text without indentation or bullet marker. */
export function itemText(item) {
  const t = item.trim();
  return (t.startsWith("- ") ? t.slice(2) : t.startsWith("-") ? t.slice(1) : t).trim();
}

/**
 * Textarea content -> stored lines: each line as typed, trailing spaces
 * trimmed, blank lines and bare bullets (a `-` with nothing after it) dropped.
 */
export function linesToItems(text) {
  return text
    .split("\n")
    .map((line) => line.replace(/\s+$/, ""))
    .filter((line) => itemText(line).length > 0);
}

/** Stored lines -> textarea content, unchanged. */
export function itemsToText(items) {
  return items.join("\n");
}

/** One stored line as it should appear in copied text — as written. */
export function itemToLine(item) {
  return item;
}

/** Builds the text for "주간 전체 복사": one h3 per day, lines as written. */
export function buildWeeklyCopyText(weekAnchorDate, days) {
  const { year, week } = isoWeekNumber(weekAnchorDate);
  const nonEmpty = days.filter((d) => d.items.length > 0);
  const first = days[0]?.date;
  const last = days[days.length - 1]?.date;
  const range = first && last ? `${shortLabel(first)} ~ ${shortLabel(last)}` : "";

  let out = `## ${year}년 ${week}주차 (${range})\n\n`;
  out += nonEmpty
    .map((d) => `### ${shortLabel(d.date)} (${d.weekday})\n${d.items.join("\n")}`)
    .join("\n\n");
  return out;
}
