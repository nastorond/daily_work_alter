import { api } from "../api.js";
import { renderSettings } from "./settings.js";

/** Shared header bar (title + ⚙ settings + ✕ close) used by daily & weekly views. */
export function mountHeader(container, title, { onBeforeClose } = {}) {
  const header = document.createElement("div");
  header.className = "header";
  header.setAttribute("data-tauri-drag-region", "");
  header.innerHTML = `
    <span class="title">${title}</span>
    <span class="actions">
      <button class="icon-btn" data-action="settings" title="설정">⚙</button>
      <button class="icon-btn" data-action="close" title="닫기">✕</button>
    </span>
  `;

  header.querySelector('[data-action="settings"]').addEventListener("click", () => {
    openSettingsModal();
  });
  header.querySelector('[data-action="close"]').addEventListener("click", async () => {
    if (onBeforeClose) await onBeforeClose();
    api.closeWindow();
  });

  container.appendChild(header);
  return header;
}

export function openSettingsModal() {
  if (document.querySelector(".modal-overlay")) return;
  const overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  document.getElementById("app").appendChild(overlay);
  renderSettings(overlay, { asModal: true, onClose: () => overlay.remove() });
}
