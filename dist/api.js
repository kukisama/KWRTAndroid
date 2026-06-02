// 统一封装 Tauri invoke，便于 view 调用。
const { invoke: rawInvoke } = window.__TAURI__.core;

// 包装一层：每次 invoke 都把 cmd 和耗时打到诊断面板。
async function invoke(cmd, args) {
  const t0 = performance.now();
  try {
    const r = await rawInvoke(cmd, args);
    if (window.__dbg) window.__dbg(`invoke ok  ${cmd}  ${(performance.now()-t0).toFixed(0)}ms`);
    return r;
  } catch (e) {
    if (window.__dbg) window.__dbg(`invoke err ${cmd}  ${(performance.now()-t0).toFixed(0)}ms  ${typeof e === "string" ? e : (e && e.message) || JSON.stringify(e)}`);
    throw e;
  }
}

export async function call(cmd, args) {
  return invoke(cmd, args);
}

// === KWRT 三按钮模型（保存 / 保存并应用 / 复位）===========================
// LuCI footer.htm 原意（见 《直连kwrt进行资料查询和处理.md》 §6）：
//   · 保存       = 表单写入 staging（uci set）不 reload
//   · 保存并应用 = staging + commit + /etc/init.d/<config> reload
//   · 复位       = 丢弃 staging，重拉表单（uci revert + GET）
//
// 本 App 现状：所有 patch_* / add_* / delete_* 默认 commit=true（立即落盘），以保证
// “点一下就生效” 的用户期望。变更会被 「markDirty」记住，顶栏 saveBar 提示“未应用”，
// 点「保存并应用」调 pw_reload；点「复位」调 pw_revert + 重拉 overview。
// 未来如果某个 view 要实现“真 staging”，可传 commit=false 调 patchSection 等。

const _dirty = new Map();     // config -> bool
const _dirtyListeners = new Set(); // (config, dirty) => void
function notifyDirty(config) {
  for (const fn of _dirtyListeners) { try { fn(config, !!_dirty.get(config)); } catch {} }
}
export function markDirty(config) {
  if (!config) return;
  _dirty.set(config, true);
  notifyDirty(config);
}
export function clearDirty(config) {
  if (!config) return;
  _dirty.delete(config);
  notifyDirty(config);
}
export function isDirty(config) { return !!_dirty.get(config); }
export function onDirty(fn) { _dirtyListeners.add(fn); return () => _dirtyListeners.delete(fn); }

async function invokeAndMarkDirty(cmd, args, config) {
  const r = await invoke(cmd, args);
  if (config) markDirty(config);
  return r;
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
  directListStructured: (path) => invoke("direct_list_structured", { path }),
  directSaveStructured: (path, entries, extras = []) =>
    invoke("direct_save_structured", { args: { path, entries, extras } }),
  sysInfo: () => invoke("sys_info"),
  restartService: (name) => invoke("restart_service", { name }),
  reloadService: (name) => invoke("reload_service", { name }),

  // PassWall 高层
  overview: (config) => invoke("pw_overview", { config }),
  patchGlobal: (config, patch, commit = true) =>
    invokeAndMarkDirty("pw_patch_global", { args: { config, patch, commit } }, config),

  // 三按钮原子能力
  pwReload: (config) => invoke("pw_reload", { config }),       // 「保存并应用」中的 reload 一步
  pwCommit: (config) => invoke("pw_commit", { config }),       // 「保存」：仅 uci commit
  pwRevert: (config) => invoke("pw_revert", { config }),       // 「复位」：uci revert
  pwChanges: (config) => invoke("pw_changes", { config }),     // uci changes <config>

  patchSection: (config, section, patch, commit = true) =>
    invokeAndMarkDirty("pw_patch_section", { args: { config, section, patch, commit } }, commit ? config : null),
  deleteSection: (config, section, commit = true) =>
    invokeAndMarkDirty("pw_delete_section", { args: { config, section, commit } }, commit ? config : null),
  addSection: (config, section_type, values, commit = true) =>
    invokeAndMarkDirty("pw_add_section", { args: { config, section_type, values, commit } }, commit ? config : null),
  readLog: (config, lines = 300) => invoke("pw_read_log", { args: { config, lines } }),
  backup: (config) => invoke("pw_backup", { args: { config } }),
  restore: (config, content) => invoke("pw_restore", { args: { config, content } }),
  updateSubscribe: (config, section) => invoke("pw_update_subscribe", { args: { config, section } }),
  updateRules: (config) => invoke("pw_update_rules", { args: { config } }),
  pingNode: (address, port) => invoke("pw_ping_node", { args: { address, port } }),
  testIcmp: (address, port) => invoke("pw_test_icmp", { args: { address, port } }),
  testTcping: (address, port) => invoke("pw_test_tcping", { args: { address, port } }),
  testUrl: (section, url = "") => invoke("pw_test_url", { args: { section, url } }),
  components: (config) => invoke("pw_components", { config }),
  importNode: (config, link) => invokeAndMarkDirty("pw_import_node", { args: { config, link } }, config),

  // ACL（按源 IP/MAC 给某些设备绕代理 / 走指定节点）
  aclRead: (config) => invoke("acl_read", { config }),
  aclSetGlobalEnable: (config, on) => invoke("acl_set_global_enable", { config, on }),
  aclAdd: (config, rule) => invoke("acl_add", { config, rule }),
  aclUpdate: (config, rule) => invoke("acl_update", { config, rule }),
  aclDelete: (config, section) => invoke("acl_delete", { config, section }),
  aclReload: (config) => invoke("acl_reload", { config }),

  // 远端文件浏览（只读）
  fsList: (path) => invoke("fs_list", { path }),
  fsPreview: (path) => invoke("fs_preview", { path }),
};

export function formatError(e) {
  if (!e) return "未知错误";
  let raw;
  if (typeof e === "string") raw = e;
  else if (e.message) raw = e.message;
  else { try { raw = JSON.stringify(e); } catch { raw = String(e); } }
  // 常见错误友好化（保留原文在 detail 里展示）
  if (/Access denied|-32002|PERMISSION_DENIED|code\s*=\s*6/i.test(raw))
    return "路由器拒绝了该操作（rpcd ACL 不足）。\n原始：" + raw;
  if (/NOT_FOUND|code\s*=\s*4/i.test(raw))
    return "目标不存在或路径错误。\n原始：" + raw;
  if (/TIMEOUT|code\s*=\s*7|timed out/i.test(raw))
    return "路由器响应超时，请检查网络/服务是否在跑。\n原始：" + raw;
  if (/INVALID_(COMMAND|ARGUMENT)|code\s*=\s*2/i.test(raw))
    return "调用参数有误。\n原始：" + raw;
  if (/ECONNREFUSED|Connection refused/i.test(raw))
    return "连不上路由器（端口被拒绝）。\n原始：" + raw;
  return raw;
}
