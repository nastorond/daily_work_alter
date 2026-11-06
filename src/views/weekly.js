import { api } from "../api.js";
import { mountHeader } from "./header.js";
import { addDays, isoWeekNumber, mondayOf, shortLabel, todayStr, weekdayKr } from "../util/date.js";
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
 * been written there yet.
 *
 * Every day, today included, is shown as written and turns into an editor when
 * clicked. Today used to be a permanently open box pinned under the list, which
 * showed a few scrolled lines of a full day instead of the day itself. The one
 * exception: an empty today in the current week opens straight into the editor,
 * since this view is also what pops up before 퇴근 to have today written.
 */
export async function renderWeekly(root, anchor = todayStr()) {
  const today = todayStr();
  const isCurrent = mondayOf(anchor) === mondayOf(today);

  const days = await api.readWeek(anchor);
  // A day off that wasn't written is left out of the week; today still gets a
  // row so there is somewhere to write.
  if (isCurrent && !days.some((d) => d.date === today)) {
    days.push({ date: today, weekday: weekdayKr(today), items: [], isToday: true });
    days.sort((a, b) => a.date.localeCompare(b.date));
  }

  const { year, week } = isoWeekNumber(anchor);
  const first = days[0]?.date ?? anchor;
  const last = days[days.length - 1]?.date ?? anchor;
  const title = `${year}년 ${week}주차 (${shortLabel(first)} ~ ${shortLabel(last)})`;

  root.innerHTML = "";
  mountHeader(root, title, { onBeforeClose: () => finishEdit() });

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

  const dayList = document.createElement("div");
  dayList.className = "week-days";
  body.appendChild(dayList);

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
  function itemsHtml(day) {
    if (day.items.length === 0) {
      const hint = day.date === today ? "(아직 작성 없음 — 눌러서 작성)" : "(작성 없음)";
      return `<span class="empty">${hint}</span>`;
    }
    const lis = day.items
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

  function renderRow(day) {
    const isToday = day.date === today;
    const row = document.createElement("div");
    row.className = isToday ? "week-day is-today" : "week-day";
    row.dataset.date = day.date;
    row.innerHTML = `
      <div class="day-label">${day.weekday} ${shortLabel(day.date)}${isToday ? `<span class="today-tag">오늘</span>` : ""}</div>
      <div class="day-items">${itemsHtml(day)}</div>
    `;
    row.querySelector(".day-items").addEventListener("click", () => startEdit(row, day));
    return row;
  }

  // The one day being edited, if any: how to save it and put it back to the
  // read-only view. Closing, navigating and copying all go through finishEdit
  // first, so nothing typed is lost to a window that closed before blur fired.
  let active = null;

  async function finishEdit() {
    if (!active) return;
    const current = active;
    active = null;
    await current.finish();
  }

  async function startEdit(row, day) {
    if (active?.row === row) return;
    await finishEdit();

    const itemsEl = row.querySelector(".day-items");
    itemsEl.innerHTML = "";
    const textarea = document.createElement("textarea");
    textarea.className = "inline-edit";
    textarea.spellcheck = false;
    textarea.value = itemsToText(day.items);
    itemsEl.appendChild(textarea);

    // Grow to fit every line. A fixed two-line box showed only the tail of a
    // long day, which made editing it guesswork.
    const fit = () => {
      textarea.style.height = "auto";
      textarea.style.height = `${textarea.scrollHeight + 2}px`;
    };
    fit();

    let timer = null;
    const save = async () => {
      clearTimeout(timer);
      day.items = linesToItems(textarea.value);
      await api.writeLog(day.date, day.items);
    };
    const changed = () => {
      fit();
      clearTimeout(timer);
      timer = setTimeout(save, 500);
    };

    active = {
      row,
      finish: async () => {
        await save();
        itemsEl.innerHTML = itemsHtml(day);
      },
    };
    const mine = active;

    textarea.addEventListener("blur", () => {
      if (active === mine) finishEdit();
    });
    // Typing, and Enter/Tab (which rewrite the value without an "input"
    // event), all come through onChange. Ctrl+Enter / Esc end the edit and leave the day as written.
    attachBulletEditor(textarea, { onChange: changed, onSubmit: () => textarea.blur() });

    textarea.focus({ preventScroll: !isTodayRow(row) });
    textarea.selectionStart = textarea.selectionEnd = textarea.value.length;
  }

  function isTodayRow(row) {
    return row.dataset.date === today;
  }

  for (const day of days) dayList.appendChild(renderRow(day));

  async function saveAndClose() {
    await finishEdit();
    api.closeWindow();
  }

  for (const btn of nav.querySelectorAll(".nav-btn")) {
    btn.addEventListener("click", async () => {
      if (btn.disabled) return;
      await finishEdit();
      const go = btn.dataset.go;
      if (go === "list") renderHistory(root);
      else renderWeekly(root, addDays(anchor, go === "prev" ? -7 : 7));
    });
  }

  footer.querySelector("#btn-copy").addEventListener("click", async () => {
    await finishEdit();
    openCopyProjects(title, days);
  });
  footer.querySelector("#btn-save-close").addEventListener("click", saveAndClose);

  // Esc with nothing being edited closes, as it did when today's box was
  // always open. (Inside an editor, Esc first ends the edit.)
  document.addEventListener("keydown", function onKey(e) {
    if (!root.isConnected || !dayList.isConnected) {
      document.removeEventListener("keydown", onKey);
      return;
    }
    // The editor's own Esc is marked handled, so it ends the edit without also
    // closing the window.
    if (e.key === "Escape" && !e.defaultPrevented && !active && !document.querySelector(".modal-overlay")) {
      e.preventDefault();
      saveAndClose();
    }
  });

  const todayDay = isCurrent ? days.find((d) => d.date === today) : null;
  if (todayDay) {
    const todayRow = dayList.querySelector(`[data-date="${today}"]`);
    if (todayDay.items.length === 0) startEdit(todayRow, todayDay);
    else todayRow.scrollIntoView({ block: "end" });
  }
}

function escapeHtml(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
