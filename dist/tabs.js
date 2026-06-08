// Tab 注册表 + 路由
import { clear, el } from "./dom.js";

const tabs = [];

export function register(id, label, mount, tip = "") {
  tabs.push({ id, label, mount, tip });
}

let current = null;
let activeCleanup = null;
// 单调递增的"挂载代"。每次 activate 会 +1 并写到 viewEl 上，
// 慢异步 mount（如 dashboard 里的 await api.sysInfo()）入口先快照本 gen，
// 在 append 之前对比，发现已经被新的 activate 顶掉就跳过 append，
// 避免出现"切到 B 之后 A 的剩余 DOM 又被追加到 B 视图里"的串台 bug。
let mountGen = 0;
export function isStillMounted(viewEl, snapshot) {
  return viewEl.__mountGen === snapshot;
}

export function renderTabBar(barEl, viewEl, ctx) {
  clear(barEl);
  for (const t of tabs) {
    const btn = el("button", { tip: t.tip, onclick: () => activate(t.id, barEl, viewEl, ctx) }, t.label);
    btn.dataset.id = t.id;
    barEl.append(btn);
  }
  // 默认激活第一个
  if (!current && tabs[0]) activate(tabs[0].id, barEl, viewEl, ctx);
  else if (current) activate(current, barEl, viewEl, ctx);
}

export async function activate(id, barEl, viewEl, ctx) {
  current = id;
  for (const b of barEl.querySelectorAll("button")) b.classList.toggle("active", b.dataset.id === id);
  if (typeof activeCleanup === "function") {
    try { activeCleanup(); } catch {}
    activeCleanup = null;
  }
  clear(viewEl);
  // 关键：每次切 tab 都换一代，让前一个 tab 还在跑的 async mount 在 append 前自知过期
  viewEl.__mountGen = ++mountGen;
  const myGen = viewEl.__mountGen;
  const t = tabs.find((x) => x.id === id);
  if (!t) return;
  try {
    const cleanup = await t.mount(viewEl, ctx);
    // 只有当前还在本代时才挂 cleanup，避免被后续 activate 误调
    if (viewEl.__mountGen === myGen) activeCleanup = cleanup;
  } catch (e) {
    if (viewEl.__mountGen === myGen) {
      viewEl.append(el("div", { class: "error", text: String(e?.message || e) }));
    }
  }
}

export function refreshCurrent(barEl, viewEl, ctx) {
  if (current) activate(current, barEl, viewEl, ctx);
}
