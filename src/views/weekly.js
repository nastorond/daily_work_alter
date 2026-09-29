import { api } from "../api.js";
import { mountHeader } from "./header.js";
import { addDays, isoWeekNumber, shortLabel, todayStr } from "../util/date.js";
import {
  buildWeeklyCopyText,
  itemDepth,
  itemText,
  itemsToText,
  itemToLine,
  linesToItems,
} from "../util/markdown.js";
import { attachBulletEditor } from "../util/bullet-editor.js";
import { openExportPreview } from "./export-preview.js";

/**
 * The week view.
 *
 * `weekOffset` is 0 for this week or -1 for last week — and no further. The
 * report is "지난주 실적", so the week before this one has to be reachable (a
 * report missed on Friday gets written on Monday, when "this week" is empty);
 * anything older is history, which the log folder already keeps.
 */
export async function renderWeekly(root, weekOffset = 0) {
  const today = todayStr();
  const isCurrent = weekOffset === 0;
  const anchor = isCurrent ? today : addDays(today, -7);

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

  const switcher = document.createElement("div");
  switcher.className = "week-switch";
  switcher.innerHTML = `
    <button class="seg ${isCurrent ? "" : "active"}" data-offset="-1">지난주</button>
    <button class="seg ${isCurrent ? "active" : ""}" data-offset="0">이번 주</button>
  `;
  body.appendChild(switcher);

  const pastList = document.createElement("div");
  pastList.className = "week-days";
  body.appendChild(pastList);

  const footer = document.createElement("div");
  footer.className = "footer";
  footer.innerHTML = `
    <button class="btn" id="btn-copy">주간 전체 복사</button>
    <button class="btn" id="btn-hwp">한글 문서로 저장</button>
    <button class="btn primary" id="btn-save-close">${isCurrent ? "저장하고 닫기" : "닫기"}</button>
  `;
  root.appendChild(footer);

  /**
   * Bullet list markup. Child items are indented one level; lines the writer
   * marked with `#` are shown as headings rather than as literal "####".
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
        return `<li class="${itemDepth(i) === 1 ? "child" : ""}">${escapeHtml(text)}</li>`;
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
    textarea.value = day.items.length > 0 ? day.items.map(itemToLine).join("\n") : "- ";
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

  for (const btn of switcher.querySelectorAll(".seg")) {
    btn.addEventListener("click", async () => {
      const offset = Number(btn.dataset.offset);
      if (offset === weekOffset) return;
      await saveTodayInput();
      renderWeekly(root, offset);
    });
  }

  footer.querySelector("#btn-copy").addEventListener("click", async () => {
    const allDays = days.map((d) => (todayEntry && d.isToday ? todayEntry : d));
    const text = buildWeeklyCopyText(anchor, allDays);
    await navigator.clipboard.writeText(text);
    const btn = footer.querySelector("#btn-copy");
    const original = btn.textContent;
    btn.textContent = "복사됨!";
    setTimeout(() => (btn.textContent = original), 1200);
  });
  footer.querySelector("#btn-hwp").addEventListener("click", async () => {
    await saveTodayInput();
    openExportPreview(anchor);
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
