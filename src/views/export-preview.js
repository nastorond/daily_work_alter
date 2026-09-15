import { api } from "../api.js";
import { shortLabel } from "../util/date.js";

const LEVELS = ["heading", "item", "detail"];
const LABEL = { heading: "1.", item: "ㅇ", detail: "-" };
const TITLE = {
  heading: "제목 (HY견고딕 13pt)",
  item: "항목 (한컴돋움 12pt)",
  detail: "세부 (휴먼명조 12pt)",
};

/**
 * One pass over the week before it becomes a 한글 document.
 *
 * The level of each line is guessed — the daily box is written flat, so a
 * heading looks the same as anything else. The guess is right most of the time
 * but not always, and this goes to a manager, so it gets shown rather than
 * applied silently. Clicking a line cycles its level.
 */
export async function openExportPreview(anchor) {
  if (document.querySelector(".modal-overlay")) return;

  const [lines, suggested] = await Promise.all([
    api.exportPreview(anchor),
    api.exportDefaultName(anchor),
  ]);

  const overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  document.getElementById("app").appendChild(overlay);

  const modal = document.createElement("div");
  modal.className = "modal export-preview";
  modal.innerHTML = `
    <div class="header">
      <span class="title">한글 문서로 저장</span>
      <span class="actions"><button class="icon-btn" id="xp-close" title="닫기">✕</button></span>
    </div>
    <div class="body">
      <p class="xp-hint">단계를 클릭하면 <b>1.</b> → <b>ㅇ</b> → <b>-</b> 순으로 바뀝니다.</p>
      <div class="error-banner" id="xp-error" hidden></div>
      <div class="xp-lines" id="xp-lines"></div>
    </div>
    <div class="footer">
      <span class="xp-filename">${suggested}</span>
      <button class="btn" id="xp-cancel">취소</button>
      <button class="btn primary" id="xp-save">저장</button>
    </div>
  `;
  overlay.appendChild(modal);

  const list = modal.querySelector("#xp-lines");
  if (lines.length === 0) {
    list.innerHTML = `<p class="empty">이 주에 기록된 내용이 없습니다.</p>`;
  }

  for (const line of lines) {
    const row = document.createElement("div");
    row.className = `xp-row lvl-${line.level}`;
    row.innerHTML = `
      <button class="xp-level" type="button"></button>
      <span class="xp-date">${shortLabel(line.date)}</span>
      <span class="xp-text"></span>
    `;
    row.querySelector(".xp-text").textContent = line.text;

    const chip = row.querySelector(".xp-level");
    const paint = () => {
      chip.textContent = LABEL[line.level];
      chip.title = TITLE[line.level];
      row.className = `xp-row lvl-${line.level}`;
    };
    chip.addEventListener("click", () => {
      line.level = LEVELS[(LEVELS.indexOf(line.level) + 1) % LEVELS.length];
      paint();
    });
    paint();
    list.appendChild(row);
  }

  const close = () => overlay.remove();
  modal.querySelector("#xp-close").addEventListener("click", close);
  modal.querySelector("#xp-cancel").addEventListener("click", close);

  modal.querySelector("#xp-save").addEventListener("click", async () => {
    const banner = modal.querySelector("#xp-error");
    banner.hidden = true;
    const dest = await api.saveDialog(suggested);
    if (!dest) return;
    try {
      await api.exportWrite(anchor, lines, dest);
      close();
    } catch (e) {
      banner.textContent = String(e);
      banner.hidden = false;
    }
  });
}
