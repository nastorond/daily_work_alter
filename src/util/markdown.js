import { isoWeekNumber, shortLabel } from "./date.js";

/** Splits free-form textarea content into bullet items, stripping "- " prefixes. */
export function linesToItems(text) {
  return text
    .split("\n")
    .map((line) => line.trim())
    .map((line) => (line.startsWith("- ") ? line.slice(2) : line.startsWith("-") ? line.slice(1) : line))
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

/** Renders items back into "- item" lines for a textarea. */
export function itemsToText(items) {
  if (items.length === 0) return "- ";
  return items.map((i) => `- ${i}`).join("\n") + "\n- ";
}

/** Builds the "주간 전체 복사" markdown per 실행계획.md 4.2. */
export function buildWeeklyCopyText(weekAnchorDate, days) {
  const { year, week } = isoWeekNumber(weekAnchorDate);
  const nonEmpty = days.filter((d) => d.items.length > 0);
  const first = days[0]?.date;
  const last = days[days.length - 1]?.date;
  const range = first && last ? `${shortLabel(first)} ~ ${shortLabel(last)}` : "";

  let out = `## ${year}년 ${week}주차 (${range})\n\n`;
  out += nonEmpty
    .map((d) => {
      const body = d.items.map((item) => `- ${item}`).join("\n");
      return `### ${shortLabel(d.date)} (${d.weekday})\n${body}`;
    })
    .join("\n\n");
  return out;
}
