import { getQueryParam } from "./util/date.js";
import { renderDaily } from "./views/daily.js";
import { renderWeekly } from "./views/weekly.js";
import { renderOnboarding } from "./views/onboarding.js";
import { openSettingsModal } from "./views/header.js";

window.addEventListener("DOMContentLoaded", async () => {
  const app = document.getElementById("app");
  const view = getQueryParam("view") ?? "daily";
  const openSettings = getQueryParam("settings") === "1";

  if (view === "onboarding") {
    await renderOnboarding(app);
    return;
  }

  if (view === "weekly") {
    await renderWeekly(app);
  } else {
    await renderDaily(app);
  }

  if (openSettings) {
    openSettingsModal();
  }
});
