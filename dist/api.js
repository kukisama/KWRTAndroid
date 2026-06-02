// 统一封装 Tauri invoke，便于 view 调用。
const { invoke } = window.__TAURI__.core;

export async function call(cmd, args) {
  return invoke(cmd, args);
}

export const api = {
  connect: (opts, remember) => invoke("connect", { opts, remember }),
  disconnect: () => invoke("disconnect"),
  redetect: () => invoke("redetect"),
  loadPassword: (host, username) => invoke("load_saved_password", { q: { host, username } }),
  deletePassword: (host, username) => invoke("delete_saved_password", { q: { host, username } }),

  // 直连文件
  listDirect: (path) => invoke("list_direct", { path }),
  addDirect: (path, entry) => invoke("add_direct", { path, entry }),
  removeDirect: (path, entry) => invoke("remove_direct", { path, entry }),
  restartService: (name) => invoke("restart_service", { name }),
  reloadService: (name) => invoke("reload_service", { name }),

  // PassWall 高层
  overview: (config) => invoke("pw_overview", { config }),
  patchGlobal: (config, patch) => invoke("pw_patch_global", { args: { config, patch } }),
  patchSection: (config, section, patch, reload = true) =>
    invoke("pw_patch_section", { args: { config, section, patch, reload } }),
  deleteSection: (config, section, reload = true) =>
    invoke("pw_delete_section", { args: { config, section, reload } }),
  addSection: (config, section_type, values, reload = true) =>
    invoke("pw_add_section", { args: { config, section_type, values, reload } }),
  readLog: (config, lines = 300) => invoke("pw_read_log", { args: { config, lines } }),
  backup: (config) => invoke("pw_backup", { args: { config } }),
  restore: (config, content) => invoke("pw_restore", { args: { config, content } }),
  updateSubscribe: (config, section) => invoke("pw_update_subscribe", { args: { config, section } }),
  updateRules: (config) => invoke("pw_update_rules", { args: { config } }),
  pingNode: (address, port) => invoke("pw_ping_node", { args: { address, port } }),
  importNode: (config, link) => invoke("pw_import_node", { args: { config, link } }),
};

export function formatError(e) {
  if (!e) return "未知错误";
  if (typeof e === "string") return e;
  if (e.message) return e.message;
  try { return JSON.stringify(e); } catch { return String(e); }
}
