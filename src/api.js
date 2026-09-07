const { invoke } = window.__TAURI__.core;

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
};
