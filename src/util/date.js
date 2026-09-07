const WEEKDAY_KR = ["일", "월", "화", "수", "목", "금", "토"];

function pad2(n) {
  return String(n).padStart(2, "0");
}

export function formatDate(d) {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

export function todayStr() {
  return formatDate(new Date());
}

export function parseDate(dateStr) {
  const [y, m, d] = dateStr.split("-").map(Number);
  return new Date(y, m - 1, d);
}

export function weekdayKr(dateStr) {
  return WEEKDAY_KR[parseDate(dateStr).getDay()];
}

export function shortLabel(dateStr) {
  const [, m, d] = dateStr.split("-");
  return `${m}-${d}`;
}

export function addDays(dateStr, n) {
  const d = parseDate(dateStr);
  d.setDate(d.getDate() + n);
  return formatDate(d);
}

/** Monday (ISO week start) of the week containing dateStr. Mirrors storage::week_monday in Rust. */
export function mondayOf(dateStr) {
  const d = parseDate(dateStr);
  const dow = d.getDay(); // 0=Sun..6=Sat
  const offset = dow === 0 ? 6 : dow - 1; // days since Monday
  d.setDate(d.getDate() - offset);
  return formatDate(d);
}

/** ISO-8601 week number for the given date string. */
export function isoWeekNumber(dateStr) {
  const d = parseDate(dateStr);
  const target = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const dayNr = (target.getDay() + 6) % 7; // 0=Mon..6=Sun
  target.setDate(target.getDate() - dayNr + 3); // nearest Thursday
  const firstThursday = new Date(target.getFullYear(), 0, 4);
  const firstDayNr = (firstThursday.getDay() + 6) % 7;
  firstThursday.setDate(firstThursday.getDate() - firstDayNr + 3);
  const weekNumber = 1 + Math.round((target - firstThursday) / (7 * 24 * 60 * 60 * 1000));
  return { year: target.getFullYear(), week: weekNumber };
}

export function getQueryParam(name) {
  return new URLSearchParams(window.location.search).get(name);
}
