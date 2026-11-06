import { api } from "../api.js";
import { mountHeader } from "./header.js";
import { addDays, isoWeekNumber, mondayOf, shortLabel, todayStr } from "../util/date.js";
import {
  hasBullet,
  itemDepth,
  itemText,
  itemsToText,
  linesToItems,
} from "../util/markdown.js";
import { attachBulletEditor } from "../util/bullet-editor.js";
import { openCopyProjects } from "./copy-projects.js";
import { renderHistory } from "./history.js";

/**
 * The week view, for the week containing `anchor` (today by default).
 *
 * Any past week can be shown: "← 목록" goes to the "이전 기록" list, and ◀ ▶
 * step a week at a time. There is no step past the current week — nothing has
 * been written there yet. Today's always-open input only appears when the
 * week shown is the current one.
 */
export async function renderWeekly(root, anchor = todayStr()) {
  const today = todayStr();
  const isCurrent = mondayOf(anchor) === mondayOf(today);

  const days = await api.readWeek(anchor);
  const { year, week } = isoWeekNumber(anchor);
  const first = days[0]?.date ?? anchor;
  const last = days[days.length - 1]?.date ?? anchor;

  root.innerHTML = "";
  mountHeader(root, `${year}년 ${week}주차 (${shortLabel(first)} ~ ${shortLabel(last)})`, {
    onBeforeClose: () => saveTodayInput(),
  });

  const body = document.createElement("div");
  body.className = "body";
  root.appendChild(body);

  const nav = document.createElement("div");
  nav.className = "week-nav";
  nav.innerHTML = `
    <button class="nav-btn" data-go="list">← 목록</button>
    <span class="nav-spacer"></span>
    <button class="nav-btn" data-go="prev" title="이전 주">◀ 이전 주</button>
    <button class="nav-btn" data-go="next" title="다음 주" ${isCurrent ? "disabled" : ""}>다음 주 ▶</button>
  `;
  body.appendChild(nav);

  const pastList = document.createElement("div");
  pastList.className = "week-days";
  body.appendChild(pastList);

  const footer = document.createElement("div");
  footer.className = "footer";
  footer.innerHTML = `
    <button class="btn" id="btn-copy">프로젝트별 복사</button>
    <button class="btn primary" id="btn-save-close">${isCurrent ? "저장하고 닫기" : "닫기"}</button>
  `;
  root.appendChild(footer);

  /**
   * Lines as written. A bullet is drawn only where the user typed `- `; plain
   * lines stay plain. Indented lines sit one level in, and `#` lines are shown
   * as headings rather than as literal "####".
   */
  function itemsHtml(items) {
    if (items.length === 0) return `<span class="empty">(작성 없음)</span>`;
    const lis = items
      .map((i) => {
        const text = itemText(i);
        const heading = /^#+\s*/.exec(text);
        if (heading) {
          return `<li class="heading">${escapeHtml(text.slice(heading[0].length))}</li>`;
        }
        const classes = [itemDepth(i) === 1 ? "child" : "", hasBullet(i) ? "" : "plain"];
        return `<li class="${classes.join(" ").trim()}">${escapeHtml(text)}</li>`;
      })
      .join("");
    return `<ul>${lis}</ul>`;
  }

  function renderPastRow(day) {
    const row = document.createElement("div");
    row.className = "week-day";
    row.dataset.date = day.date;
    row.innerHTML = `
      <div class="day-label">${day.weekday} ${shortLabel(day.date)}</div>
      <div class="day-items">${itemsHtml(day.items)}</div>
    `;
    row.querySelector(".day-items").addEventListener("click", () => startEdit(row, day));
    return row;
  }

  function startEdit(row, day) {
    if (row.querySelector("textarea")) return;
    const itemsEl = row.querySelector(".day-items");
    itemsEl.innerHTML = "";
    const textarea = document.createElement("textarea");
    textarea.value = itemsToText(day.items);
    itemsEl.appendChild(textarea);
    textarea.focus();
    textarea.selectionStart = textarea.selectionEnd = textarea.value.length;

    const finish = async () => {
      const items = linesToItems(textarea.value);
      day.items = items;
      await api.writeLog(day.date, items);
      itemsEl.innerHTML = itemsHtml(items);
    };

    textarea.addEventListener("blur", finish);
    attachBulletEditor(textarea, { onSubmit: () => textarea.blur() });
  }

  // Today gets its own always-open input, but only in the current week; last
  // week is all past days, each editable in place.
  const todayEntry = isCurrent
    ? days.find((d) => d.isToday) ?? { date: today, weekday: "", items: [], isToday: true }
    : null;

  for (const day of days) {
    if (todayEntry && day.isToday) continue;
    pastList.appendChild(renderPastRow(day));
  }

  let todayInput = null;
  let debounceTimer = null;

  if (todayEntry) {
    const todaySection = document.createElement("div");
    todaySection.className = "week-day today";
    todaySection.innerHTML = `
      <div style="flex:1">
        <label class="field-label">${todayEntry.weekday} ${shortLabel(todayEntry.date)} 오늘 한 일</label>
        <textarea class="bullet-input" id="today-input" spellcheck="false" style="margin-top:8px"></textarea>
      </div>
    `;
    body.appendChild(todaySection);
    todayInput = todaySection.querySelector("#today-input");
    todayInput.value = itemsToText(todayEntry.items);
    attachBulletEditor(todayInput, { onChange: scheduleAutosave, onSubmit: saveAndClose });
  }

  function scheduleAutosave() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(saveTodayInput, 500);
  }

  async function saveTodayInput() {
    clearTimeout(debounceTimer);
    if (!todayInput) return;
    const items = linesToItems(todayInput.value);
    todayEntry.items = items;
    await api.writeLog(todayEntry.date, items);
  }

  async function saveAndClose() {
    await saveTodayInput();
    api.closeWindow();
  }

  for (const btn of nav.querySelectorAll(".nav-btn")) {
    btn.addEventListener("click", async () => {
      if (btn.disabled) return;
      await saveTodayInput();
      const go = btn.dataset.go;
      if (go === "list") renderHistory(root);
      else renderWeekly(root, addDays(anchor, go === "prev" ? -7 : 7));
    });
  }

  footer.querySelector("#btn-copy").addEventListener("click", async () => {
    await saveTodayInput();
    const allDays = days.map((d) => (todayEntry && d.isToday ? todayEntry : d));
    openCopyProjects(`${year}년 ${week}주차 (${shortLabel(first)} ~ ${shortLabel(last)})`, allDays);
  });
  footer.querySelector("#btn-save-close").addEventListener("click", saveAndClose);

  if (todayInput) {
    todayInput.focus();
    todayInput.selectionStart = todayInput.selectionEnd = todayInput.value.length;
  }
}

function escapeHtml(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
