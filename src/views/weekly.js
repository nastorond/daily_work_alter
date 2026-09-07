import { api } from "../api.js";
import { mountHeader } from "./header.js";
import { isoWeekNumber, shortLabel, todayStr } from "../util/date.js";
import { buildWeeklyCopyText, itemsToText, linesToItems } from "../util/markdown.js";

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

  function renderPastRow(day) {
    const row = document.createElement("div");
    row.className = "week-day";
    row.dataset.date = day.date;

    const listHtml =
      day.items.length > 0
        ? `<ul>${day.items.map((i) => `<li>${escapeHtml(i)}</li>`).join("")}</ul>`
        : `<span class="empty">(작성 없음)</span>`;

    row.innerHTML = `
      <div class="day-label">${day.weekday} ${shortLabel(day.date)}</div>
      <div class="day-items">${listHtml}</div>
    `;

    row.querySelector(".day-items").addEventListener("click", () => startEdit(row, day));
    return row;
  }

  function startEdit(row, day) {
    if (row.querySelector("textarea")) return;
    const itemsEl = row.querySelector(".day-items");
    itemsEl.innerHTML = "";
    const textarea = document.createElement("textarea");
    textarea.value = day.items.length > 0 ? day.items.map((i) => `- ${i}`).join("\n") : "- ";
    itemsEl.appendChild(textarea);
    textarea.focus();
    textarea.selectionStart = textarea.selectionEnd = textarea.value.length;

    const finish = async () => {
      const items = linesToItems(textarea.value);
      day.items = items;
      await api.writeLog(day.date, items);
      itemsEl.innerHTML =
        items.length > 0
          ? `<ul>${items.map((i) => `<li>${escapeHtml(i)}</li>`).join("")}</ul>`
          : `<span class="empty">(작성 없음)</span>`;
    };

    textarea.addEventListener("blur", finish);
    textarea.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && e.ctrlKey) {
        e.preventDefault();
        textarea.blur();
      } else if (e.key === "Escape") {
        e.preventDefault();
        textarea.blur();
      }
    });
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

  todayInput.addEventListener("focus", () => {
    if (todayInput.value === "") {
      todayInput.value = "- ";
      todayInput.selectionStart = todayInput.selectionEnd = todayInput.value.length;
    }
  });
  todayInput.addEventListener("input", scheduleAutosave);
  todayInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && e.ctrlKey) {
      e.preventDefault();
      saveAndClose();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      const start = todayInput.selectionStart;
      const end = todayInput.selectionEnd;
      const value = todayInput.value;
      const insert = "\n- ";
      todayInput.value = value.slice(0, start) + insert + value.slice(end);
      const pos = start + insert.length;
      todayInput.selectionStart = todayInput.selectionEnd = pos;
      scheduleAutosave();
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      saveAndClose();
    }
  });

  async function saveAndClose() {
    await saveTodayInput();
    api.closeWindow();
  }

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
