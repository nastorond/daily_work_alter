import { api } from "../api.js";
import { mountHeader } from "./header.js";
import { isoWeekNumber, shortLabel, todayStr } from "../util/date.js";
import {
  buildWeeklyCopyText,
  itemDepth,
  itemText,
  itemsToText,
  itemToLine,
  linesToItems,
} from "../util/markdown.js";
import { attachBulletEditor } from "../util/bullet-editor.js";

export async function renderWeekly(root) {
  const today = todayStr();
  const days = await api.readWeek(today);
  const { year, week } = isoWeekNumber(today);
  const first = days[0]?.date ?? today;
  const last = days[days.length - 1]?.date ?? today;

  root.innerHTML = "";
  mountHeader(root, `${year}년 ${week}주차 (${shortLabel(first)} ~ ${shortLabel(last)})`, {
    onBeforeClose: () => saveTodayInput(),
  });

  const body = document.createElement("div");
  body.className = "body";
  root.appendChild(body);

  const pastList = document.createElement("div");
  pastList.className = "week-days";
  body.appendChild(pastList);

  const todaySection = document.createElement("div");
  todaySection.className = "week-day today";
  body.appendChild(todaySection);

  const footer = document.createElement("div");
  footer.className = "footer";
  footer.innerHTML = `
    <button class="btn" id="btn-copy">주간 전체 복사</button>
    <button class="btn primary" id="btn-save-close">저장하고 닫기</button>
  `;
  root.appendChild(footer);

  const todayEntry = days.find((d) => d.isToday) ?? {
    date: today,
    weekday: "",
    items: [],
    isToday: true,
  };

  /** Bullet list markup, with child items indented one level. */
  function itemsHtml(items) {
    if (items.length === 0) return `<span class="empty">(작성 없음)</span>`;
    const lis = items
      .map((i) => `<li class="${itemDepth(i) === 1 ? "child" : ""}">${escapeHtml(itemText(i))}</li>`)
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

  for (const day of days) {
    if (day.isToday) continue;
    pastList.appendChild(renderPastRow(day));
  }

  todaySection.innerHTML = `
    <div style="flex:1">
      <label class="field-label">${todayEntry.weekday} ${shortLabel(todayEntry.date)} 오늘 한 일</label>
      <textarea class="bullet-input" id="today-input" spellcheck="false" style="margin-top:8px"></textarea>
    </div>
  `;
  const todayInput = todaySection.querySelector("#today-input");
  todayInput.value = itemsToText(todayEntry.items);

  let debounceTimer = null;
  function scheduleAutosave() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(saveTodayInput, 500);
  }

  async function saveTodayInput() {
    clearTimeout(debounceTimer);
    const items = linesToItems(todayInput.value);
    todayEntry.items = items;
    await api.writeLog(todayEntry.date, items);
  }

  async function saveAndClose() {
    await saveTodayInput();
    api.closeWindow();
  }

  attachBulletEditor(todayInput, { onChange: scheduleAutosave, onSubmit: saveAndClose });

  footer.querySelector("#btn-copy").addEventListener("click", async () => {
    const allDays = days.map((d) => (d.isToday ? todayEntry : d));
    const text = buildWeeklyCopyText(today, allDays);
    await navigator.clipboard.writeText(text);
    const btn = footer.querySelector("#btn-copy");
    const original = btn.textContent;
    btn.textContent = "복사됨!";
    setTimeout(() => (btn.textContent = original), 1200);
  });
  footer.querySelector("#btn-save-close").addEventListener("click", saveAndClose);

  todayInput.focus();
  todayInput.selectionStart = todayInput.selectionEnd = todayInput.value.length;
}

function escapeHtml(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
