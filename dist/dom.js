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

/**
 * 友好的 toast。
 * @param text 主提示语（一句话）
 * @param kind "ok" | "warn" | "info"
 * @param opts.detail 可选，错误对象 / 字符串；点"详情"才展开
 * @param opts.duration 自动消失毫秒（默认 ok 3.5s, warn 不自动消失，需点关闭）
 */
export function toast(text, kind = "ok", opts = {}) {
  const colors = {
    ok:   { bg: "#e7f7ed", border: "#3aa05a", fg: "#0a5524" },
    warn: { bg: "#fdecea", border: "#d33b3b", fg: "#6a0f0f" },
    info: { bg: "#eaf3fd", border: "#2c7be5", fg: "#0d3a73" },
  }[kind] || { bg: "#fdecea", border: "#d33b3b", fg: "#6a0f0f" };

  const box = el("div", { class: "msg " + kind });
  Object.assign(box.style, {
    position: "fixed", right: "16px", bottom: "16px", zIndex: 9999,
    maxWidth: "420px", padding: "10px 12px",
    background: colors.bg, color: colors.fg,
    border: "1px solid " + colors.border, borderRadius: "6px",
    boxShadow: "0 6px 24px rgba(0,0,0,0.25)",
    fontSize: "13px", lineHeight: "1.4",
  });

  const head = el("div", { style: { display: "flex", alignItems: "flex-start", gap: "8px" } }, [
    el("div", { style: { flex: "1", whiteSpace: "pre-wrap" }, text }),
    el("button", {
      style: { border: "none", background: "transparent", color: colors.fg, cursor: "pointer", fontSize: "14px", padding: "0 4px" },
      onclick: () => box.remove(),
      text: "✕",
    }),
  ]);
  box.append(head);

  const detail = opts.detail;
  if (detail) {
    let expanded = false;
    const detailText = typeof detail === "string" ? detail
      : detail && detail.message ? detail.message
      : (() => { try { return JSON.stringify(detail, null, 2); } catch { return String(detail); } })();
    const pre = el("pre", {
      style: { margin: "8px 0 0", padding: "6px 8px", background: "rgba(0,0,0,0.06)", borderRadius: "4px", maxHeight: "180px", overflow: "auto", fontSize: "11px", whiteSpace: "pre-wrap", wordBreak: "break-all" },
      text: detailText,
    });
    pre.hidden = true;
    const toggle = el("a", {
      href: "#",
      style: { fontSize: "11px", color: colors.fg, opacity: 0.75, textDecoration: "underline", display: "inline-block", marginTop: "6px" },
      text: "查看详情",
      onclick: (ev) => { ev.preventDefault(); expanded = !expanded; pre.hidden = !expanded; toggle.textContent = expanded ? "收起" : "查看详情"; },
    });
    box.append(toggle, pre);
  }

  document.body.append(box);
  const dur = opts.duration != null ? opts.duration : (kind === "ok" ? 3500 : 0);
  if (dur > 0) setTimeout(() => box.remove(), dur);
  return box;
}

export function confirmAct(msg) { return window.confirm(msg); }
