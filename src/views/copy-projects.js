import { groupByProject, renderAll, toMarkdown, toReport } from "../util/projects.js";

const FORMAT_KEY = "dailyworkalter.copyFormat";
// Markdown first and by default: it is what the logs are written in.
const DEFAULT_FORMAT = "markdown";
const FORMATS = {
  markdown: "마크다운 (### / -)",
  report: "보고서 형식 (1. / ㅇ / -)",
};

function loadFormat() {
  try {
    const f = localStorage.getItem(FORMAT_KEY);
    return f in FORMATS ? f : DEFAULT_FORMAT;
  } catch {
    return DEFAULT_FORMAT;
  }
}

function saveFormat(f) {
  try {
    localStorage.setItem(FORMAT_KEY, f);
  } catch {
    // Remembering the choice is a convenience; losing it is fine.
  }
}

async function copyWithFeedback(button, text) {
  await navigator.clipboard.writeText(text);
  const original = button.textContent;
  button.textContent = "복사됨!";
  setTimeout(() => (button.textContent = original), 1200);
}

/**
 * The week regrouped by project, ready to paste: one copy button per project
 * and one for everything. Replaces both the old day-by-day "주간 전체 복사" and
 * the 한글 export — the work it saves is the Friday re-sorting of a week of
 * daily notes into per-project blocks.
 *
 * @param title  heading shown in the modal (the week label)
 * @param days   [{ date, items }] for the week, today's latest input included
 */
export function openCopyProjects(title, days) {
  if (document.querySelector(".modal-overlay")) return;

  const projects = groupByProject(days);
  let format = loadFormat();

  const overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  document.getElementById("app").appendChild(overlay);

  const modal = document.createElement("div");
  modal.className = "modal copy-projects";
  modal.innerHTML = `
    <div class="header">
      <span class="title">프로젝트별 복사</span>
      <span class="actions"><button class="icon-btn" id="cp-close" title="닫기">✕</button></span>
    </div>
    <div class="body">
      <div class="cp-top">
        <span class="cp-week"></span>
        <span class="cp-formats">
          ${Object.entries(FORMATS)
            .map(([k, label]) => `<button class="seg" data-format="${k}">${label}</button>`)
            .join("")}
        </span>
      </div>
      <div class="cp-list" id="cp-list"></div>
    </div>
    <div class="footer">
      <button class="btn" id="cp-cancel">닫기</button>
      <button class="btn primary" id="cp-all">전체 복사</button>
    </div>
  `;
  overlay.appendChild(modal);
  modal.querySelector(".cp-week").textContent = title;

  const list = modal.querySelector("#cp-list");
  const allBtn = modal.querySelector("#cp-all");

  function render() {
    for (const b of modal.querySelectorAll(".cp-formats .seg")) {
      b.classList.toggle("active", b.dataset.format === format);
    }
    list.innerHTML = "";
    if (projects.length === 0) {
      list.innerHTML = `<p class="empty">이 주에 기록된 내용이 없습니다.</p>`;
      allBtn.disabled = true;
      return;
    }
    projects.forEach((p, i) => {
      const text = format === "report" ? toReport(p, i + 1) : toMarkdown(p);
      const card = document.createElement("div");
      card.className = "cp-card";
      card.innerHTML = `
        <div class="cp-card-head">
          <span class="cp-name"></span>
          <button class="btn cp-copy">복사</button>
        </div>
        <pre class="cp-preview"></pre>
      `;
      // Project names and lines are user text: set as text, not markup.
      card.querySelector(".cp-name").textContent = p.name;
      card.querySelector(".cp-preview").textContent = text;
      const btn = card.querySelector(".cp-copy");
      btn.addEventListener("click", () => copyWithFeedback(btn, text));
      list.appendChild(card);
    });
  }

  for (const b of modal.querySelectorAll(".cp-formats .seg")) {
    b.addEventListener("click", () => {
      format = b.dataset.format;
      saveFormat(format);
      render();
    });
  }

  const close = () => overlay.remove();
  modal.querySelector("#cp-close").addEventListener("click", close);
  modal.querySelector("#cp-cancel").addEventListener("click", close);
  allBtn.addEventListener("click", () => copyWithFeedback(allBtn, renderAll(projects, format)));

  render();
}
