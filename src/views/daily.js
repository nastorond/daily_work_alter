import { api } from "../api.js";
import { mountHeader } from "./header.js";
import { addDays, shortLabel, todayStr, weekdayKr } from "../util/date.js";
import { itemsToText, itemToLine, linesToItems } from "../util/markdown.js";
import { attachBulletEditor } from "../util/bullet-editor.js";

export async function renderDaily(root) {
  const today = todayStr();
  const yesterday = addDays(today, -1);

  const [todayLog, yesterdayLog] = await Promise.all([api.readLog(today), api.readLog(yesterday)]);

  root.innerHTML = "";
  mountHeader(root, `${today} (${weekdayKr(today)})`, { onBeforeClose: () => saveCurrent() });

  const body = document.createElement("div");
  body.className = "body";
  body.innerHTML = `
    <label class="field-label">오늘 한 일</label>
    <textarea class="bullet-input" id="daily-input" spellcheck="false"></textarea>
    <details class="collapsible" id="yesterday-ref">
      <summary>어제 (${shortLabel(yesterday)}) 참고</summary>
      <div class="content"></div>
    </details>
  `;
  root.appendChild(body);

  const footer = document.createElement("div");
  footer.className = "footer";
  footer.innerHTML = `
    <button class="btn" id="btn-snooze">10분 뒤 다시</button>
    <button class="btn" id="btn-skip">오늘 건너뛰기</button>
    <button class="btn primary" id="btn-save">저장</button>
  `;
  root.appendChild(footer);

  const textarea = body.querySelector("#daily-input");
  textarea.value = itemsToText(todayLog.items);

  const yesterdayContent = body.querySelector("#yesterday-ref .content");
  yesterdayContent.textContent =
    yesterdayLog.items.length > 0 ? yesterdayLog.items.map(itemToLine).join("\n") : "작성 없음";

  let debounceTimer = null;
  function scheduleAutosave() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(saveCurrent, 500);
  }

  async function saveCurrent() {
    clearTimeout(debounceTimer);
    await api.writeLog(today, linesToItems(textarea.value));
  }

  async function saveAndClose() {
    await saveCurrent();
    api.closeWindow();
  }

  attachBulletEditor(textarea, { onChange: scheduleAutosave, onSubmit: saveAndClose });

  footer.querySelector("#btn-snooze").addEventListener("click", async () => {
    await saveCurrent();
    api.snooze();
  });
  footer.querySelector("#btn-skip").addEventListener("click", async () => {
    await saveCurrent();
    api.skipToday();
  });
  footer.querySelector("#btn-save").addEventListener("click", saveAndClose);

  textarea.focus();
  textarea.selectionStart = textarea.selectionEnd = textarea.value.length;
}
