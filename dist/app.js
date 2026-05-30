// 与 Rust 后端通过 Tauri invoke 通信。无打包器，直接用 window.__TAURI__ 全局。
const { invoke } = window.__TAURI__.core;

const $ = (sel) => document.querySelector(sel);
const PREF_KEY = "kwrt.prefs.v1";

const state = {
  connected: false,
  report: null,
  prefs: loadPrefs(),
};

// ---------------- 偏好（非敏感） ----------------
function loadPrefs() {
  try {
    return JSON.parse(localStorage.getItem(PREF_KEY) || "{}");
  } catch {
    return {};
  }
}
function savePrefs(p) {
  state.prefs = { ...state.prefs, ...p };
  localStorage.setItem(PREF_KEY, JSON.stringify(state.prefs));
}

// ---------------- 启动：回填上次表单 ----------------
window.addEventListener("DOMContentLoaded", async () => {
  const f = $("#form-login");
  const p = state.prefs;
  if (p.host) f.host.value = p.host;
  if (p.scheme) f.scheme.value = p.scheme;
  if (p.port) f.port.value = p.port;
  if (p.username) f.username.value = p.username;
  if (p.accept_invalid_certs) f.accept_invalid_certs.checked = true;
  if (p.remember) f.remember.checked = true;

  // 尝试从 keyring 加载密码
  if (p.remember && p.host && p.username) {
    try {
      const pwd = await invoke("load_saved_password", { q: { host: p.host, username: p.username } });
      if (pwd) f.password.value = pwd;
    } catch (e) {
      console.warn("load_saved_password failed", e);
    }
  }

  bindHandlers();
});

function bindHandlers() {
  $("#form-login").addEventListener("submit", onLogin);
  $("#btn-forget").addEventListener("click", onForget);
  $("#btn-disconnect").addEventListener("click", onDisconnect);
  $("#btn-redetect").addEventListener("click", onRedetect);
  $("#form-add").addEventListener("submit", onAddEntry);
  $("#btn-reload").addEventListener("click", () => onServiceAction("reload"));
  $("#btn-restart").addEventListener("click", () => onServiceAction("restart"));
  $("#btn-refresh-list").addEventListener("click", refreshList);
}

// ---------------- 登录 ----------------
async function onLogin(ev) {
  ev.preventDefault();
  const f = ev.currentTarget;
  const errEl = $("#login-error");
  errEl.hidden = true;
  errEl.textContent = "";
  const btn = $("#btn-login");
  btn.disabled = true; btn.textContent = "登录中…";

  const opts = {
    host: f.host.value.trim(),
    scheme: f.scheme.value,
    port: f.port.value ? Number(f.port.value) : null,
    username: f.username.value.trim(),
    password: f.password.value,
    accept_invalid_certs: f.accept_invalid_certs.checked,
    timeout_secs: 12,
  };
  const remember = f.remember.checked;

  try {
    const report = await invoke("connect", { opts, remember });
    savePrefs({
      host: opts.host, scheme: opts.scheme, port: opts.port || null,
      username: opts.username, accept_invalid_certs: opts.accept_invalid_certs,
      remember,
    });
    if (!remember) {
      // 用户取消"记住密码"时，删掉之前保存的
      try {
        await invoke("delete_saved_password", { q: { host: opts.host, username: opts.username } });
      } catch {}
    }
    state.connected = true;
    state.report = report;
    enterMainView(report);
  } catch (e) {
    errEl.textContent = formatError(e);
    errEl.hidden = false;
  } finally {
    btn.disabled = false; btn.textContent = "登录并检测";
  }
}

async function onForget() {
  const f = $("#form-login");
  const host = f.host.value.trim();
  const user = f.username.value.trim();
  if (!host || !user) return;
  if (!confirm(`确认删除 ${user}@${host} 的保存密码？`)) return;
  try {
    await invoke("delete_saved_password", { q: { host, username: user } });
    f.password.value = "";
    alert("已删除");
  } catch (e) {
    alert("删除失败：" + formatError(e));
  }
}

async function onDisconnect() {
  try { await invoke("disconnect"); } catch {}
  state.connected = false;
  state.report = null;
  $("#view-main").hidden = true;
  $("#view-login").hidden = false;
  $("#btn-disconnect").hidden = true;
  setStatus("未连接");
}

// ---------------- 主界面 ----------------
function enterMainView(report) {
  $("#view-login").hidden = true;
  $("#view-main").hidden = false;
  $("#btn-disconnect").hidden = false;
  renderReport(report);
}

function renderReport(report) {
  setStatus(`${state.prefs.username}@${state.prefs.host} · ${report.hostname || "?"} · ${report.openwrt_release || "?"}`);

  const banner = $("#banner");
  let label = "未识别到 PassWall";
  let bad = true;
  if (report.variant === "v1") { label = "已识别：PassWall v1"; bad = false; }
  else if (report.variant === "v2") { label = "已识别：PassWall v2"; bad = false; }
  else if (report.variant === "both") { label = "同时安装了 PassWall v1 和 v2，默认操作 v1"; bad = false; }
  banner.className = "banner" + (bad ? " bad" : "");
  banner.textContent = `${label}  ·  主配置: ${report.primary_config || "-"}  ·  init: ${report.init_script || "-"}  ·  direct_ip: ${report.direct_ip_path || "-"}`;

  // checks
  const list = $("#checks");
  list.replaceChildren(...report.checks.map((c) => {
    const li = document.createElement("li");
    li.className = c.ok ? "ok" : "ng";
    const m = document.createElement("span"); m.className = "mark"; m.textContent = c.ok ? "✓" : "✗";
    const n = document.createElement("span"); n.className = "name"; n.textContent = c.name;
    const d = document.createElement("span"); d.className = "detail"; d.textContent = c.detail;
    li.append(m, n, d);
    return li;
  }));

  $("#configs").textContent = report.uci_configs.join("  ");

  // actions
  const canAct = !!report.direct_ip_path && !!report.init_script;
  $("#actions-enabled").hidden = !canAct;
  $("#actions-disabled").hidden = canAct;
  if (canAct) {
    $("#direct-path").textContent = report.direct_ip_path;
    refreshList();
  }
}

async function onRedetect() {
  try {
    const report = await invoke("redetect");
    state.report = report;
    renderReport(report);
  } catch (e) {
    alert(formatError(e));
  }
}

// ---------------- 直连列表 ----------------
async function refreshList() {
  if (!state.report?.direct_ip_path) return;
  try {
    const data = await invoke("list_direct", { path: state.report.direct_ip_path });
    $("#entry-count").textContent = String(data.entries.length);
    const ul = $("#direct-list");
    ul.replaceChildren(...data.entries.map((entry) => {
      const li = document.createElement("li");
      const span = document.createElement("span"); span.textContent = entry;
      const btn = document.createElement("button"); btn.className = "danger ghost"; btn.textContent = "删除";
      btn.addEventListener("click", () => onRemoveEntry(entry));
      li.append(span, btn);
      return li;
    }));
  } catch (e) {
    showAddMsg(formatError(e), true);
  }
}

async function onAddEntry(ev) {
  ev.preventDefault();
  const f = ev.currentTarget;
  const entry = f.entry.value.trim();
  if (!entry) return;
  try {
    const added = await invoke("add_direct", { path: state.report.direct_ip_path, entry });
    if (added) {
      showAddMsg(`已加入直连：${entry}（注意：需 reload / restart PassWall 才会生效）`);
      f.entry.value = "";
      refreshList();
    } else {
      showAddMsg(`${entry} 已存在，未重复添加`, true);
    }
  } catch (e) {
    showAddMsg(formatError(e), true);
  }
}

async function onRemoveEntry(entry) {
  if (!confirm(`从直连列表删除 ${entry}？`)) return;
  try {
    await invoke("remove_direct", { path: state.report.direct_ip_path, entry });
    refreshList();
  } catch (e) {
    showAddMsg(formatError(e), true);
  }
}

async function onServiceAction(action) {
  const name = state.report?.init_script;
  if (!name) return;
  const desc = action === "restart" ? "重启" : "重载";
  if (!confirm(`确认 ${desc} ${name}？这将短暂中断代理连接。`)) return;
  try {
    await invoke(action === "restart" ? "restart_service" : "reload_service", { name });
    showAddMsg(`${desc} ${name} 成功`);
  } catch (e) {
    showAddMsg(formatError(e), true);
  }
}

// ---------------- 工具 ----------------
function setStatus(text) { $("#status-line").textContent = text; }

function showAddMsg(text, warn = false) {
  const el = $("#add-msg");
  el.textContent = text;
  el.className = "msg" + (warn ? " warn" : "");
  el.hidden = false;
}

function formatError(e) {
  if (!e) return "未知错误";
  if (typeof e === "string") return e;
  if (e.message) return e.message;
  try { return JSON.stringify(e); } catch { return String(e); }
}
