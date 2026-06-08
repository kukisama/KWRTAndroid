// 入口：登录 + 主视图 tab 调度
import "./tooltip.js";
import { $, el, clear, toast } from "./dom.js";
import { api, formatError } from "./api.js";
import { register, renderTabBar, refreshCurrent } from "./tabs.js";
import { bindThemeButton } from "./theme.js";

window.__dbg && window.__dbg("app.js module top reached");

import mountDashboard from "./views/dashboard.js";
import mountNodes from "./views/nodes.js";
import mountShunt from "./views/shunt.js";
import mountSubscribe from "./views/subscribe.js";
import mountDns from "./views/dns.js";
import mountRules from "./views/rules.js";
import mountForwarding from "./views/forwarding.js";
import mountDirect from "./views/direct.js";
import mountLog from "./views/log.js";
import mountBackup from "./views/backup.js";
import mountRaw from "./views/raw.js";

const PREF_KEY = "kwrt.prefs.v1";

// 共享 ctx：所有 view 都通过它访问 overview / 配置名 / 刷新
const ctx = {
  config: "passwall",
  report: null,
  overview: null,
  async refresh() {
    if (!ctx.config) return;
    ctx.overview = await api.overview(ctx.config);
    return ctx.overview;
  },
  async refreshAndRedraw() {
    await ctx.refresh();
    refreshCurrent($("#tabbar"), $("#tabview"), ctx);
  },
};

// 注册所有 tab。文案 + 提示都是中文。
register("dashboard", "总览", mountDashboard, "主开关、当前 TCP/UDP 节点、运行状态一览");
register("nodes", "节点", mountNodes, "查看 / 设为出口 / 删除 / 导入链接（vless/vmess/hy2/trojan/ss）");
register("shunt", "分流", mountShunt, "按域名 / IP 段把流量分到不同节点，对应 PassWall 的『分流』页");
register("subscribe", "订阅", mountSubscribe, "管理订阅链接并立即更新节点池");
register("dns", "DNS", mountDns, "DNS 模式 / 远程 DNS / 是否过滤 IPv6 等");
register("rules", "规则源", mountRules, "GFWList / ChnRoute / 中国域名表等规则的更新与源");
register("forwarding", "端口/转发", mountForwarding, "TCP/UDP 重定向端口、丢弃端口、转发方式");
register("direct", "直连列表", mountDirect, "PassWall 直连出局的 IP / 域名清单（即原『加入直连』功能）");
register("log", "日志", mountLog, "实时查看 PassWall 运行日志");
register("backup", "备份/恢复", mountBackup, "导出 / 导入 /etc/config/passwall 整个配置文件");
register("raw", "原始配置", mountRaw, "展开后能看到 22 个 section 全部原始字段，遇到 UI 没覆盖的设置就来这里");

// ───── 启动流程 ─────
// 注意：本文件以 <script type="module"> 加载，等价于 defer，
// 执行时 DOMContentLoaded 可能已经触发完，所以这里不要用 addEventListener("DOMContentLoaded")，
// 直接立即初始化即可（body 内的元素一定已经存在）。
init();

function init() {
  window.__dbg && window.__dbg("init() running");
  bindThemeButton();
  // 全局错误捕获 -> toast + console，便于在 UI 上看到 JS 异常
  window.addEventListener("error", (e) => {
    console.error("window.error", e.error || e.message, e);
    toast("脚本错误：" + (e.error?.message || e.message || "未知"), "warn");
  });
  window.addEventListener("unhandledrejection", (e) => {
    console.error("unhandledrejection", e.reason);
    toast("未处理异常：" + formatError(e.reason), "warn");
  });

  const f = $("#form-login");
  if (!f) {
    console.error("找不到 #form-login，初始化失败");
    return;
  }
  const p = loadPrefs();
  if (p.host) f.host.value = p.host;
  if (p.scheme) f.scheme.value = p.scheme;
  if (p.port) f.port.value = p.port;
  if (p.username) f.username.value = p.username;
  if (p.accept_invalid_certs) f.accept_invalid_certs.checked = true;
  // remember 默认为勾选；只有用户上次显式取消才不勾
  if (p.remember === false) f.remember.checked = false;
  if (f.remember.checked && (p.host || f.host.value) && (p.username || f.username.value)) {
    const h = p.host || f.host.value;
    const u = p.username || f.username.value;
    api.loadPassword(h, u).then((pwd) => {
      if (pwd && !f.password.value) f.password.value = pwd;
    }).catch((e) => console.warn("load_saved_password failed", e));
  }

  // 输入即保存（不含密码），登录失败也不丢
  const persistFields = ["host", "scheme", "port", "username", "accept_invalid_certs", "remember"];
  persistFields.forEach((name) => {
    const el = f.elements[name];
    if (!el) return;
    el.addEventListener("change", () => {
      savePrefs({
        host: f.host.value.trim(),
        scheme: f.scheme.value,
        port: f.port.value ? Number(f.port.value) : null,
        username: f.username.value.trim(),
        accept_invalid_certs: f.accept_invalid_certs.checked,
        remember: f.remember.checked,
      });
    });
  });

  f.addEventListener("submit", onLogin);
  $("#btn-forget").addEventListener("click", onForget);
  $("#btn-disconnect").addEventListener("click", onDisconnect);
  $("#btn-redetect").addEventListener("click", onRedetect);
  $("#btn-refresh-all").addEventListener("click", onRefreshAll);
  $("#btn-reload").addEventListener("click", () => onService("reload"));
  $("#btn-restart").addEventListener("click", () => onService("restart"));
  console.log("[kwrt] init done");
  window.__dbg && window.__dbg("init() done: submit handler bound");
}

function loadPrefs() {
  try { return JSON.parse(localStorage.getItem(PREF_KEY) || "{}"); } catch { return {}; }
}
function savePrefs(patch) {
  const cur = loadPrefs();
  localStorage.setItem(PREF_KEY, JSON.stringify({ ...cur, ...patch }));
}

async function onLogin(ev) {
  ev.preventDefault();
  const f = ev.currentTarget;
  const errEl = $("#login-error");
  errEl.hidden = true; errEl.textContent = "";
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
    const report = await api.connect(opts, remember);
    savePrefs({
      host: opts.host, scheme: opts.scheme, port: opts.port || null,
      username: opts.username, accept_invalid_certs: opts.accept_invalid_certs, remember,
    });
    if (!remember) {
      try { await api.deletePassword(opts.host, opts.username); } catch {}
    }
    ctx.report = report;
    ctx.config = report.primary_config || "passwall";
    await enterMain();
  } catch (e) {
    errEl.textContent = formatError(e);
    errEl.hidden = false;
  } finally {
    btn.disabled = false; btn.textContent = "登录并检测";
  }
}

async function onForget() {
  const f = $("#form-login");
  const host = f.host.value.trim(); const user = f.username.value.trim();
  if (!host || !user) return;
  if (!confirm(`确认删除 ${user}@${host} 的保存密码？`)) return;
  try { await api.deletePassword(host, user); f.password.value = ""; toast("已删除保存密码"); }
  catch (e) { toast(formatError(e), "warn"); }
}

async function onDisconnect() {
  try { await api.disconnect(); } catch {}
  ctx.report = null; ctx.overview = null;
  $("#view-main").hidden = true;
  $("#view-login").hidden = false;
  $("#btn-disconnect").hidden = true;
  $("#status-line").textContent = "未连接";
}

async function enterMain() {
  $("#view-login").hidden = true;
  $("#view-main").hidden = false;
  $("#btn-disconnect").hidden = false;
  setStatus();
  renderBanner();
  try { await ctx.refresh(); } catch (e) { toast(`拉取概览失败：${formatError(e)}`, "warn"); }
  renderTabBar($("#tabbar"), $("#tabview"), ctx);
}

function setStatus() {
  const p = loadPrefs();
  const r = ctx.report;
  $("#status-line").textContent = `${p.username}@${p.host} · ${r?.hostname || "?"} · ${r?.openwrt_release || "?"}`;
}

function renderBanner() {
  const r = ctx.report;
  const banner = $("#banner");
  let label = "未识别到 PassWall";
  let bad = true;
  if (r?.variant === "v1") { label = "已识别：PassWall v1"; bad = false; }
  else if (r?.variant === "v2") { label = "已识别：PassWall v2"; bad = false; }
  else if (r?.variant === "both") { label = "v1 + v2 同时安装，默认操作 v1"; bad = false; }
  banner.className = "banner" + (bad ? " bad" : "");
  banner.textContent = `${label}  ·  主配置: ${r?.primary_config || "-"}  ·  init: ${r?.init_script || "-"}  ·  direct_ip: ${r?.direct_ip_path || "-"}`;
}

async function onRedetect() {
  try {
    ctx.report = await api.redetect();
    ctx.config = ctx.report.primary_config || ctx.config;
    renderBanner();
    await ctx.refreshAndRedraw();
    toast("已重新检测");
  } catch (e) { toast(formatError(e), "warn"); }
}

async function onRefreshAll() {
  try { await ctx.refreshAndRedraw(); toast("已刷新"); }
  catch (e) { toast(formatError(e), "warn"); }
}

async function onService(action) {
  const name = ctx.report?.init_script || ctx.config;
  if (!name) return;
  const desc = action === "restart" ? "重启" : "重载";
  if (!confirm(`确认 ${desc} ${name}？这将短暂中断代理。`)) return;
  try {
    if (action === "restart") await api.restartService(name);
    else await api.reloadService(name);
    toast(`${desc} ${name} 成功`);
  } catch (e) { toast(formatError(e), "warn"); }
}

// 暴露给 view 内部统一刷新
export { ctx };
