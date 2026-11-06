import { api } from "../api.js";
import { mountHeader } from "./header.js";
import { shortLabel } from "../util/date.js";
import { renderWeekly } from "./weekly.js";

/**
 * "이전 기록": every week that has entries, newest first, grouped by month.
 *
 * Each row carries what is needed to pick a week without opening it: which
 * days were written (■) or left empty (□), and the `#` headings of that week —
 * past weeks get looked for by what was going on ("the 태양광 week"), not by
 * their number. Clicking a row opens the ordinary week view on that week.
 */
export async function renderHistory(root) {
  const weeks = await api.listWeeks();

  root.innerHTML = "";
  mountHeader(root, "이전 기록");

  const body = document.createElement("div");
  body.className = "body history";
  root.appendChild(body);

  if (weeks.length === 0) {
    body.innerHTML = `<p class="empty">아직 기록이 없습니다.</p>`;
    return;
  }

  let currentMonth = null;
  for (const w of weeks) {
    const month = Number(w.monday.split("-")[1]);
    if (month !== currentMonth) {
      currentMonth = month;
      const label = document.createElement("div");
      label.className = "history-month";
      label.textContent = `${month}월`;
      body.appendChild(label);
    }
    body.appendChild(renderRow(root, w));
  }
}

function renderRow(root, w) {
  const first = w.days[0]?.date ?? w.monday;
  const last = w.days[w.days.length - 1]?.date ?? w.monday;
  const filled = w.days.filter((d) => d.filled).length;
  // Drawn as boxes rather than ■/□ characters: the two glyphs come out at
  // different sizes in the system font, which made empty days look like dots.
  const marks = w.days
    .map((d) => `<i class="mark ${d.filled ? "on" : ""}"></i>`)
    .join("");
  const holidayNote = w.holidays > 0 ? ` · 휴일 ${w.holidays}` : "";

  const row = document.createElement("button");
  row.type = "button";
  row.className = "history-row";
  row.innerHTML = `
    <div class="history-line">
      <span class="history-week">${w.week}주차</span>
      <span class="history-range">${shortLabel(first)} ~ ${shortLabel(last)}</span>
      <span class="history-marks" title="채운 칸: 기록 있음 / 빈 칸: 비어 있음">${marks}</span>
      <span class="history-count">${filled}일${holidayNote}</span>
    </div>
    <div class="history-headings"></div>
  `;
  // Headings are user text, so set them as text rather than markup.
  row.querySelector(".history-headings").textContent = w.headings.join(" · ");
  row.addEventListener("click", () => renderWeekly(root, w.monday));
  return row;
}
