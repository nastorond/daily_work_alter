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

export async function renderOnboarding(root) {
  const config = await api.getConfig();

  root.innerHTML = `
    <div class="header" data-tauri-drag-region>
      <span class="title">DailyWorkAlter 시작하기</span>
    </div>
    <div class="body">
      <p class="onboarding-intro">
        퇴근 30분 전에 자동으로 떠서 오늘 한 일을 기록하는 앱이에요.<br />
        출근/퇴근 시간과 주간 요약 요일만 먼저 정해주세요. 나머지는 트레이 메뉴의
        "설정"에서 언제든 바꿀 수 있어요.
      </p>
      <div class="error-banner" id="ob-error" hidden></div>
      <div class="form-row">
        <label>출근 시간</label>
        ${timeFieldHtml("ob-start-time")}
      </div>
      <div class="form-row">
        <label>퇴근 시간</label>
        ${timeFieldHtml("ob-end-time")}
      </div>
      <div class="form-row">
        <label>주간 요약 요일</label>
        <select id="ob-weekly-day">
          ${WEEKDAY_LABELS.map((w) => `<option value="${w.iso}">${w.label}</option>`).join("")}
        </select>
      </div>
    </div>
    <div class="footer">
      <button class="btn primary" id="ob-start">시작하기</button>
    </div>
  `;

  const q = (sel) => root.querySelector(sel);
  setTime(root, "ob-start-time", config.work.startTime);
  setTime(root, "ob-end-time", config.work.endTime);
  attachTimeField(root, "ob-start-time");
  attachTimeField(root, "ob-end-time");
  q("#ob-weekly-day").value = String(config.weekly.day);

  q("#ob-start").addEventListener("click", async () => {
    const startTime = getTime(root, "ob-start-time");
    const endTime = getTime(root, "ob-end-time");
    if (!startTime || !endTime || startTime >= endTime) {
      const banner = q("#ob-error");
      banner.textContent = !startTime || !endTime
        ? "시간을 시 0~23, 분 0~59 범위로 입력하세요."
        : "출근 시간은 퇴근 시간보다 빨라야 합니다.";
      banner.hidden = false;
      return;
    }
    const updated = {
      ...config,
      work: { ...config.work, startTime, endTime },
      weekly: { ...config.weekly, day: Number(q("#ob-weekly-day").value) },
    };
    await api.saveConfig(updated);
    api.closeWindow();
  });
}
