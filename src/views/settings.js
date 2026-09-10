import { api } from "../api.js";
import { attachTimeField, getTime, setTime, timeFieldHtml } from "./time-field.js";

const WEEKDAY_LABELS = [
  { iso: 1, label: "월" },
  { iso: 2, label: "화" },
  { iso: 3, label: "수" },
  { iso: 4, label: "목" },
  { iso: 5, label: "금" },
  { iso: 6, label: "토" },
  { iso: 7, label: "일" },
];

export async function renderSettings(container, { onClose } = {}) {
  const config = await api.getConfig();

  const modal = document.createElement("div");
  modal.className = "modal";
  modal.innerHTML = `
    <div class="header">
      <span class="title">설정</span>
      <span class="actions"><button class="icon-btn" id="settings-close" title="닫기">✕</button></span>
    </div>
    <div class="body">
      <div class="error-banner" id="settings-error" hidden></div>

      <div class="form-row">
        <label>퇴근 시간</label>
        ${timeFieldHtml("f-end-time")}
      </div>
      <div class="form-row">
        <label>알림 시점</label>
        <span>퇴근 <input type="number" id="f-minutes-before" min="0" max="180" style="width:70px" /> 분 전</span>
      </div>
      <div class="form-row">
        <label>근무 요일</label>
        <div class="weekday-picker" id="f-workdays">
          ${WEEKDAY_LABELS.map(
            (w) => `<label><input type="checkbox" value="${w.iso}" />${w.label}</label>`
          ).join("")}
        </div>
      </div>
      <div class="form-row">
        <label>주간 요약</label>
        <label style="flex:1"><input type="checkbox" id="f-weekly-enabled" /> 사용</label>
      </div>
      <div class="form-row radios">
        <label></label>
        <div class="radio-group">
          <label><input type="radio" name="weekly-mode" value="fixedDay" /> 지정 요일
            <select id="f-weekly-day">
              ${WEEKDAY_LABELS.map((w) => `<option value="${w.iso}">${w.label}</option>`).join("")}
            </select>
          </label>
          <label><input type="radio" name="weekly-mode" value="lastWorkday" /> 그 주 마지막 근무일</label>
        </div>
      </div>
      <div class="form-row">
        <label>전역 단축키</label>
        <input type="text" id="f-hotkey" readonly placeholder="클릭 후 키 조합 입력" />
      </div>
      <div class="form-row">
        <label>스누즈 간격</label>
        <span><input type="number" id="f-snooze" min="1" max="120" style="width:70px" /> 분</span>
      </div>
      <div class="form-row">
        <label>다시 알림</label>
        <span><input type="number" id="f-repeat" min="0" max="240" style="width:70px" /> 분마다 (0이면 하루 한 번)</span>
      </div>
      <div class="form-row">
        <label>로그인 시 자동 실행</label>
        <label style="flex:1"><input type="checkbox" id="f-autostart" /></label>
      </div>
      <div class="form-row">
        <label></label>
        <div style="display:flex; gap:8px;">
          <button class="btn" id="f-open-config">config.json 열기</button>
          <button class="btn" id="f-open-logdir">로그 폴더 열기</button>
        </div>
      </div>
    </div>
    <div class="footer">
      <button class="btn" id="f-cancel">취소</button>
      <button class="btn primary" id="f-save">저장</button>
    </div>
  `;
  container.appendChild(modal);

  const q = (sel) => modal.querySelector(sel);
  setTime(modal, "f-end-time", config.work.endTime);
  attachTimeField(modal, "f-end-time");
  q("#f-minutes-before").value = config.notify.minutesBefore;
  q("#f-snooze").value = config.notify.snoozeMinutes;
  q("#f-repeat").value = config.notify.repeatMinutes;
  q("#f-hotkey").value = config.hotkey;
  q("#f-autostart").checked = config.autostart;
  q("#f-weekly-enabled").checked = config.weekly.enabled;
  q(`input[name="weekly-mode"][value="${config.weekly.mode}"]`).checked = true;
  q("#f-weekly-day").value = String(config.weekly.day);
  for (const cb of modal.querySelectorAll('#f-workdays input[type="checkbox"]')) {
    cb.checked = config.work.workdays.includes(Number(cb.value));
  }

  function close() {
    modal.remove();
    if (onClose) onClose();
  }
  q("#settings-close").addEventListener("click", close);
  q("#f-cancel").addEventListener("click", close);

  const hotkeyInput = q("#f-hotkey");
  hotkeyInput.addEventListener("keydown", (e) => {
    e.preventDefault();
    if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
    const parts = [];
    if (e.ctrlKey || e.metaKey) parts.push("CommandOrControl");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    let key = e.key;
    if (key.length === 1) key = key.toUpperCase();
    parts.push(key);
    hotkeyInput.value = parts.join("+");
  });

  q("#f-open-config").addEventListener("click", () => api.openConfigFile());
  q("#f-open-logdir").addEventListener("click", () => api.openLogDir());

  function showError(msg) {
    const banner = q("#settings-error");
    banner.textContent = msg;
    banner.hidden = false;
  }

  q("#f-save").addEventListener("click", async () => {
    const workdays = [...modal.querySelectorAll('#f-workdays input:checked')].map((cb) => Number(cb.value));
    const endTime = getTime(modal, "f-end-time");

    if (workdays.length === 0) {
      showError("근무 요일을 최소 1개 선택하세요.");
      return;
    }
    if (!endTime) {
      showError("퇴근 시간을 시 0~23, 분 0~59 범위로 입력하세요.");
      return;
    }
    if (!hotkeyInput.value.trim()) {
      showError("전역 단축키를 입력하세요.");
      return;
    }

    const updated = {
      ...config,
      work: { ...config.work, endTime, workdays },
      notify: {
        ...config.notify,
        minutesBefore: Number(q("#f-minutes-before").value) || 0,
        snoozeMinutes: Number(q("#f-snooze").value) || 1,
        repeatMinutes: Math.max(0, Number(q("#f-repeat").value) || 0),
      },
      weekly: {
        ...config.weekly,
        enabled: q("#f-weekly-enabled").checked,
        mode: q('input[name="weekly-mode"]:checked').value,
        day: Number(q("#f-weekly-day").value),
      },
      hotkey: hotkeyInput.value.trim(),
      autostart: q("#f-autostart").checked,
    };

    try {
      await api.saveConfig(updated);
      close();
    } catch (err) {
      showError(typeof err === "string" ? err : "설정 저장에 실패했습니다.");
    }
  });
}
