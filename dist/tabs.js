// Tab 注册表 + 路由
import { clear, el } from "./dom.js";

const tabs = [];

export function register(id, label, mount, tip = "") {
  tabs.push({ id, label, mount, tip });
}

let current = null;
let activeCleanup = null;

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
  const t = tabs.find((x) => x.id === id);
  if (!t) return;
  try {
    activeCleanup = await t.mount(viewEl, ctx);
  } catch (e) {
    viewEl.append(el("div", { class: "error", text: String(e?.message || e) }));
  }
}

export function refreshCurrent(barEl, viewEl, ctx) {
  if (current) activate(current, barEl, viewEl, ctx);
}
