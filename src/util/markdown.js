import { isoWeekNumber, shortLabel } from "./date.js";

/**
 * Nesting is carried inside the item string itself: a child item starts with two
 * spaces. Only one level is supported, which keeps items plain strings all the
 * way through the Rust commands and the on-disk markdown. Mirrors CHILD_PREFIX
 * in src-tauri/src/storage.rs — change both together.
 */
export const CHILD_PREFIX = "  ";

export function itemDepth(item) {
  return item.startsWith(CHILD_PREFIX) ? 1 : 0;
}

export function itemText(item) {
  return item.startsWith(CHILD_PREFIX) ? item.slice(CHILD_PREFIX.length) : item;
}

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

/** Splits free-form textarea content into items, preserving one level of indent. */
export function linesToItems(text) {
  return text
    .split("\n")
    .map((line) => {
      const indent = indentWidth(line);
      const t = line.trim();
      const body = t.startsWith("- ") ? t.slice(2) : t.startsWith("-") ? t.slice(1) : t;
      return { indent, text: body.trim() };
    })
    .filter((row) => row.text.length > 0)
    .map((row) => (row.indent >= CHILD_PREFIX.length ? CHILD_PREFIX + row.text : row.text));
}

/** Renders items back into bullet lines for a textarea, indent included. */
export function itemsToText(items) {
  if (items.length === 0) return "- ";
  return items.map(itemToLine).join("\n") + "\n- ";
}

/** One item as a markdown bullet line, e.g. "  - 세부 항목". */
export function itemToLine(item) {
  const indent = itemDepth(item) === 1 ? CHILD_PREFIX : "";
  return `${indent}- ${itemText(item)}`;
}

/** Builds the text for "주간 전체 복사": one h3 per day, bullets underneath. */
export function buildWeeklyCopyText(weekAnchorDate, days) {
  const { year, week } = isoWeekNumber(weekAnchorDate);
  const nonEmpty = days.filter((d) => d.items.length > 0);
  const first = days[0]?.date;
  const last = days[days.length - 1]?.date;
  const range = first && last ? `${shortLabel(first)} ~ ${shortLabel(last)}` : "";

  let out = `## ${year}년 ${week}주차 (${range})\n\n`;
  out += nonEmpty
    .map((d) => {
      const body = d.items.map(itemToLine).join("\n");
      return `### ${shortLabel(d.date)} (${d.weekday})\n${body}`;
    })
    .join("\n\n");
  return out;
}
