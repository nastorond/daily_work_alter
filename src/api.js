const { invoke } = window.__TAURI__.core;
const { save } = window.__TAURI__.dialog;

export const api = {
  getConfig: () => invoke("get_config"),
  saveConfig: (config) => invoke("save_config", { config }),
  openConfigFile: () => invoke("open_config_file"),
  openLogDir: () => invoke("open_log_dir"),
  readLog: (date) => invoke("read_log", { date }),
  writeLog: (date, items) => invoke("write_log", { date, items }),
  readWeek: (anchor) => invoke("read_week", { anchor }),
  getViewMode: () => invoke("get_view_mode"),
  snooze: () => invoke("snooze"),
  skipToday: () => invoke("skip_today"),
  closeWindow: () => invoke("close_window"),
  exportPreview: (anchor) => invoke("export_preview", { anchor }),
  exportDefaultName: (anchor) => invoke("export_default_name", { anchor }),
  exportWrite: (anchor, lines, dest) => invoke("export_write", { anchor, lines, dest }),
  /** Native save dialog; resolves to a path or null when cancelled. */
  saveDialog: (defaultPath) =>
    save({ defaultPath, filters: [{ name: "한글 문서", extensions: ["hwpx"] }] }),
};
