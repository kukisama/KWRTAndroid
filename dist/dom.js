// 公用 DOM 工具
export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));

export function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    if (k === "class") node.className = v;
    else if (k === "html") node.innerHTML = v;
    else if (k === "text") node.textContent = v;
    else if (k === "tip") node.setAttribute("data-tip", v);
    else if (k.startsWith("on")) node.addEventListener(k.slice(2).toLowerCase(), v);
    else if (k === "style" && typeof v === "object") Object.assign(node.style, v);
    else if (typeof v === "boolean") { if (v) node.setAttribute(k, ""); }
    else node.setAttribute(k, v);
  }
  for (const c of [].concat(children)) {
    if (c == null || c === false) continue;
    node.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return node;
}

export function clear(node) { while (node.firstChild) node.removeChild(node.firstChild); }

export function toast(text, kind = "ok") {
  const t = el("div", { class: kind === "ok" ? "msg" : "msg warn", text });
  Object.assign(t.style, {
    position: "fixed", right: "16px", bottom: "16px", zIndex: 9999,
    maxWidth: "360px", boxShadow: "0 6px 24px rgba(0,0,0,0.4)",
  });
  document.body.append(t);
  setTimeout(() => t.remove(), 3500);
}

export function confirmAct(msg) { return window.confirm(msg); }
