// 入口：登录 + 主视图 tab 调度
import "./tooltip.js";
import { $, el, clear, toast } from "./dom.js";
import { api, formatError, onDirty, isDirty, clearDirty, markDirty } from "./api.js";
import { register, renderTabBar, refreshCurrent } from "./tabs.js";
import { bindThemeButton } from "./theme.js";

window.__dbg && window.__dbg("app.js module top reached");

import mountDashboard from "./views/dashboard.js";
import mountClients from "./views/clients.js";
import mountNodes from "./views/nodes.js";
import mountShunt from "./views/shunt.js";
import mountSubscribe from "./views/subscribe.js";
import mountDns from "./views/dns.js";
import mountRules from "./views/rules.js";
import mountForwarding from "./views/forwarding.js";
import mountDirect from "./views/direct.js";
import mountAcl from "./views/acl.js";
import mountFs from "./views/fs.js";
import mountLog from "./views/log.js";
import mountBackup from "./views/backup.js";
import mountRaw from "./views/raw.js";
import mountComponents from "./views/components.js";

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

// 注册所有 tab。把文案集中到数组里，全部用反引号字符串，
// 这样以后随便加全角/半角符号都不会再因为引号嵌套把整个模块炸掉。
const TAB_DEFS = [
  [`dashboard`,  `总览`,       mountDashboard,  `主开关、当前 TCP/UDP 节点、运行状态一览`],
  [`clients`,    `客户端开关`, mountClients,    `只显示已有 ACL 规则的启用/停用，绿=开 灰=关，配合 应用配置 一次性 reload`],
  [`nodes`,      `节点`,       mountNodes,      `查看、设为出口、删除、导入链接（vless / vmess / hy2 / trojan / ss）`],
  [`shunt`,      `分流`,       mountShunt,      `按域名 / IP 段把流量分到不同节点，对应 PassWall 的 分流 页`],
  [`subscribe`,  `订阅`,       mountSubscribe,  `管理订阅链接并立即更新节点池`],
  [`dns`,        `DNS`,        mountDns,        `DNS 模式 / 远程 DNS / 是否过滤 IPv6 等`],
  [`rules`,      `规则源`,     mountRules,      `GFWList / ChnRoute / 中国域名表等规则的更新与源`],
  [`forwarding`, `端口/转发`,  mountForwarding, `TCP/UDP 重定向端口、丢弃端口、转发方式`],
  [`direct`,     `直连列表`,   mountDirect,     `PassWall 直连出局的 IP / 域名清单（即原 加入直连 功能）`],
  [`acl`,        `客户端例外`, mountAcl,        `按源 IP / MAC 给某些设备指定走哪个节点或整机绕过 PassWall（对应 LuCI 访问控制）`],
  [`log`,        `日志`,       mountLog,        `实时查看 PassWall 运行日志`],
  [`backup`,     `备份/恢复`,  mountBackup,     `导出 / 导入 /etc/config/passwall 整个配置文件`],
  [`components`, `组件信息`,   mountComponents, `xray / sing-box / hysteria 等核心二进制的版本、路径、大小（只读）`],
  [`raw`,        `原始配置`,   mountRaw,        `展开后能看到 22 个 section 全部原始字段，遇到 UI 没覆盖的设置就来这里`],
  [`fs`,         `文件浏览`,   mountFs,         `只读浏览路由器文件系统：看分区里到底有什么，可预览小文本文件`],
];
for (const [id, label, mount, tip] of TAB_DEFS) register(id, label, mount, tip);

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
  // 兼容老 prefs：把 scheme/host/port 拼成 url
  if (p.url) f.url.value = p.url;
  else if (p.host) f.url.value = composeUrl(p.scheme || "http", p.host, p.port);
  if (p.username) f.username.value = p.username;
  // remember 默认为勾选；只有用户上次显式取消才不勾
  if (p.remember === false) f.remember.checked = false;
  // 异步：拉取保存的密码 → 如果命中且勾了记住，自动登录
  let autoLoginAttempted = false;
  // 兼容用户裸 IP（10.0.0.1）等不能直接 new URL 的写法：用 parseRouterUrl 容错解析
  const savedHost = (() => {
    const v = (f.url.value || "").trim();
    if (!v) return "";
    try { return new URL(v).hostname; } catch {}
    try { return parseRouterUrl(v).host; } catch { return ""; }
  })();
  const savedUser = (p.username || f.username.value || "").trim();
  console.log("[autologin] remember=", f.remember.checked, "host=", savedHost, "user=", savedUser);
  if (f.remember.checked && savedHost && savedUser) {
    $("#status-line").textContent = `读取 ${savedUser}@${savedHost} 的保存密码…`;
    api.loadPassword(savedHost, savedUser).then((pwd) => {
      if (pwd && !f.password.value) f.password.value = pwd;
      if (pwd && !autoLoginAttempted) {
        autoLoginAttempted = true;
        $("#status-line").textContent = `正在自动登录 ${savedUser}@${savedHost} …`;
        // 自动登录失败 onLogin 内部已把错误显示到 #login-error；这里只兜底防 unhandled
        onLogin().catch((e) => console.warn("auto login failed", e));
      } else if (!pwd) {
        $("#status-line").textContent = "未连接";
      }
    }).catch((e) => {
      console.warn("load_saved_password failed", e);
      $("#status-line").textContent = "读取保存密码失败：" + formatError(e);
    });
  }

  // 输入即保存（不含密码），登录失败也不丢
  const persistFields = ["url", "username", "remember"];
  persistFields.forEach((name) => {
    const el = f.elements[name];
    if (!el) return;
    el.addEventListener("change", () => {
      savePrefs({
        url: f.url.value.trim(),
        username: f.username.value.trim(),
        remember: f.remember.checked,
      });
    });
  });

  f.addEventListener("submit", onLogin);
  $("#btn-login").addEventListener("click", onLogin);
  $("#btn-forget").addEventListener("click", onForget);
  $("#btn-disconnect").addEventListener("click", onDisconnect);
  $("#btn-redetect").addEventListener("click", onRedetect);
  $("#btn-refresh-all").addEventListener("click", onRefreshAll);
  $("#btn-save").addEventListener("click", onSave);
  $("#btn-save-apply").addEventListener("click", onSaveApply);
  $("#btn-revert").addEventListener("click", onRevert);
  $("#btn-restart").addEventListener("click", () => onService("restart"));
  // 监听 dirty 状态：顶栏红点指示器
  onDirty((cfg) => {
    if (cfg !== ctx.config) return;
    const ind = $("#dirty-indicator");
    if (ind) ind.hidden = !isDirty(cfg);
  });
  // dirty-indicator 悬浮：显示具体 uci changes 行，方便决定保存还是放弃
  bindDirtyHover();
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
  if (ev && ev.preventDefault) ev.preventDefault();
  const f = $("#form-login");
  const errEl = $("#login-error");
  errEl.hidden = true; errEl.textContent = "";
  const btn = $("#btn-login");
  btn.disabled = true; btn.textContent = "登录中…";

  let parsed;
  try {
    parsed = parseRouterUrl(f.url.value.trim());
  } catch (e) {
    errEl.textContent = "路由器地址格式有误：" + e.message + "\n示例：http://10.0.0.1  或  http://10.0.0.1:8080  或  https://router.lan";
    errEl.hidden = false;
    btn.disabled = false; btn.textContent = "登录并检测";
    return;
  }
  const opts = {
    host: parsed.host,
    scheme: parsed.scheme,
    port: parsed.port,
    username: f.username.value.trim(),
    password: f.password.value,
    accept_invalid_certs: true,  // 默认允许自签证书（路由器多用自签），不再让用户选
    timeout_secs: 12,
  };
  const remember = f.remember.checked;

  try {
    const report = await api.connect(opts, remember);
    savePrefs({
      url: f.url.value.trim(),
      username: opts.username, remember,
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
  let host = "";
  try { host = new URL(f.url.value.trim()).hostname; } catch {}
  const user = f.username.value.trim();
  if (!host || !user) { toast("请先填路由器地址和用户名", "warn"); return; }
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
  $("#topbar-actions").hidden = true;
  $("#status-line").textContent = "未连接";
  $("#status-line").hidden = true;
}

async function enterMain() {
  $("#view-login").hidden = true;
  $("#view-main").hidden = false;
  $("#btn-disconnect").hidden = false;
  $("#topbar-actions").hidden = false;
  $("#status-line").hidden = false;
  setStatus();
  renderBanner();
  try { await ctx.refresh(); } catch (e) { toast(`拉取概览失败：${formatError(e)}`, "warn"); }
  renderTabBar($("#tabbar"), $("#tabview"), ctx);
  // 路由器侧若残留 uci changes（之前会话或 LuCI 手动改了没 commit），点亮“未应用”指示器
  try {
    const changes = await api.pwChanges(ctx.config);
    if (changes && changes.length) {
      markDirty(ctx.config);
      toast(`路由器有 ${changes.length} 条未应用的 uci 暂存：点顶栏「保存并应用」让其生效，或「复位」丢弃`, "info");
    } else {
      clearDirty(ctx.config);
    }
  } catch {}
}

function setStatus() {
  const p = loadPrefs();
  const r = ctx.report;
  const hostShown = (() => { try { return new URL(p.url || "").hostname; } catch { return p.host || ""; } })();
  $("#status-line").textContent = `${p.username}@${hostShown} · ${r?.hostname || "?"} · ${r?.openwrt_release || "?"}`;
}

function renderBanner() {
  const r = ctx.report;
  const banner = $("#banner");
  // 紧凑模式：只在“未识别 PassWall”这种异常时展示，正常情况完全隐藏，不占空间
  let bad = !(r && (r.variant === "v1" || r.variant === "v2" || r.variant === "both"));
  if (!bad) { banner.hidden = true; banner.textContent = ""; return; }
  banner.hidden = false;
  banner.className = "banner banner-compact bad";
  banner.textContent = "未识别到 PassWall：请确认路由器已安装 luci-app-passwall / passwall2";
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

// 三按钮模型（LuCI footer.htm 复刻；详见 直连kwrt进行资料查询和处理.md §5.5/§6）：
//   保存       = uci commit <cfg>，仅持久化，不重启服务
//   保存并应用 = uci commit + /etc/init.d/<cfg> reload，让运行的进程重读
//   复位       = uci revert <cfg>，丢弃 staging，并重新从磁盘拉取 overview
//
// 关键原则：所有状态转变结束后都调 reconcileDirty()——以路由器侧 `uci changes` 为唯一事实来源，
// 不依赖本地乐观 markDirty/clearDirty，避免 “ACL 拒绝 revert但 indicator 被乐观清除” 这种状态反转 bug。
async function reconcileDirty() {
  if (!ctx.config) return [];
  try {
    const changes = await api.pwChanges(ctx.config);
    if (changes && changes.length) markDirty(ctx.config);
    else clearDirty(ctx.config);
    return changes || [];
  } catch (e) {
    // 获取状态失败不静默，告诉用户，不乱动 indicator
    toast("检查 uci changes 失败", "warn", { detail: formatError(e) });
    return [];
  }
}

async function onSave() {
  if (!ctx.config) return;
  const btn = $("#btn-save"); const old = btn.textContent; btn.disabled = true; btn.textContent = "保存中…";
  try {
    await api.pwCommit(ctx.config);
    // commit 后 staging 清空，但 “未重载” 仍是事实；dirty 本质上表示“进程未重读”
    // 这里按事实来源重新核实：uci changes 应为空了，但服务还没 reload——indicator 仍保留提示
    const changes = await reconcileDirty();
    // 手动指示“需要 reload”：即使 changes 为空，也强制 dirty，提醒用户下一步是「保存并应用」
    if (!changes.length) markDirty(ctx.config);
    toast(`✓ 已 commit ${ctx.config}（未重载服务，进程仍跑旧配置）`);
  } catch (e) { toast("保存失败", "warn", { detail: formatError(e) }); }
  finally { btn.disabled = false; btn.textContent = old; }
}

async function onSaveApply() {
  if (!ctx.config) return;
  const btn = $("#btn-save-apply"); const old = btn.textContent; btn.disabled = true; btn.textContent = "应用中…";
  try {
    await api.pwCommit(ctx.config);
    await api.pwReload(ctx.config);
    clearDirty(ctx.config);
    toast(`✓ 已保存并应用到 ${ctx.report?.init_script || ctx.config}`);
  } catch (e) { toast("保存并应用失败", "warn", { detail: formatError(e) }); }
  finally { btn.disabled = false; btn.textContent = old; }
}

async function onRevert() {
  if (!ctx.config) return;
  if (!confirm("复位将丢弃所有尚未 commit 的 staging，并重新从磁盘加载配置；运行中的服务不变。继续？")) return;
  const btn = $("#btn-revert"); const old = btn.textContent; btn.disabled = true; btn.textContent = "复位中…";
  try {
    await api.pwRevert(ctx.config);
    await ctx.refreshAndRedraw();
    // 路由器侧事实核实：revert 是否真的清了 staging？
    const changes = await reconcileDirty();
    if (changes.length) {
      // ACL 拒绝 / uci revert 未生效——不掩盖问题
      toast(`⚠ 复位后路由器侧仍有 ${changes.length} 条未提交变更，复位可能未生效`, "warn", { detail: changes.join("\n") });
    } else {
      toast(`✓ 已复位 ${ctx.config}（staging 已清、表单从磁盘重新加载）`);
    }
  } catch (e) { toast("复位失败", "warn", { detail: formatError(e) }); }
  finally { btn.disabled = false; btn.textContent = old; }
}

// 暴露给 view 内部统一刷新
export { ctx };

// 「未应用」徽标的悬浮：显示 uci changes 列表，方便用户决定保存还是放弃
function bindDirtyHover() {
  const ind = document.getElementById("dirty-indicator");
  const pop = document.getElementById("dirty-popover");
  if (!ind || !pop) return;
  let hideTimer = null;
  let lastFetchAt = 0;
  async function show() {
    if (ind.hidden) return;
    pop.hidden = false;
    pop.textContent = "正在读取 uci changes…";
    // 定位：徽标下方靠右
    const r = ind.getBoundingClientRect();
    pop.style.top = `${Math.round(r.bottom + 6)}px`;
    pop.style.right = `${Math.max(8, Math.round(window.innerWidth - r.right))}px`;
    // 缓存 1s，避免连续抖动多次请求
    if (Date.now() - lastFetchAt < 1000 && pop.dataset.cached) {
      pop.innerHTML = pop.dataset.cached;
      return;
    }
    try {
      const changes = await api.pwChanges(ctx.config);
      lastFetchAt = Date.now();
      let html;
      if (!changes.length) {
        html = `<div class="dp-empty">无未提交变更（uci changes 为空）</div>`;
      } else {
        const lines = changes.map(l => `<div class="dp-line">${escapeHtml(l)}</div>`).join("");
        html = `<div class="dp-head">${changes.length} 条未提交变更（${escapeHtml(ctx.config)}）</div>${lines}` +
               `<div class="dp-foot">点顶栏「保存并应用」让其生效，或「复位」丢弃</div>`;
      }
      pop.innerHTML = html;
      pop.dataset.cached = html;
    } catch (e) {
      pop.textContent = "读取失败：" + formatError(e);
    }
  }
  function hide() {
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => { pop.hidden = true; }, 200);
  }
  ind.addEventListener("mouseenter", show);
  ind.addEventListener("mouseleave", hide);
  pop.addEventListener("mouseenter", () => clearTimeout(hideTimer));
  pop.addEventListener("mouseleave", hide);
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}

// 把用户输入的"路由器地址"解析为 {scheme, host, port}。
// 接受：http://10.0.0.1  http://10.0.0.1:8080  https://router.lan:443  10.0.0.1（默认 http）
function parseRouterUrl(input) {
  let s = (input || "").trim();
  if (!s) throw new Error("地址为空");
  if (!/^https?:\/\//i.test(s)) s = "http://" + s;
  let u;
  try { u = new URL(s); } catch { throw new Error("无法解析"); }
  if (u.protocol !== "http:" && u.protocol !== "https:") throw new Error("仅支持 http / https");
  if (!u.hostname) throw new Error("缺少主机名");
  const port = u.port ? Number(u.port) : null;
  return { scheme: u.protocol.replace(":", ""), host: u.hostname, port };
}
function composeUrl(scheme, host, port) {
  if (!host) return "";
  return `${scheme || "http"}://${host}${port ? `:${port}` : ""}`;
}
