// 直连列表（PassWall direct_ip + direct_host）
//
// 设计依据：KWRT/PassWall 的 LuCI 「直连列表」页本身就是 **两段独立 textarea**：
//   - /usr/share/passwall/rules/direct_ip   一行一个 IP / CIDR (v4/v6)
//   - /usr/share/passwall/rules/direct_host 一行一个域名
// 两个文件结构、加载、生效机制完全独立。所以本页也按 LuCI 的视觉分段呈现：
// 上面一张「IP / CIDR」子表，下面一张「域名」子表，但共用顶部一个「提交并应用」
// （写两次 + 一次 reload）。这样既忠于 KWRT 原始模型，也避免来回切两个页面。
//
// 行为：
//   - 「启用」开关：关 = 该行被写成注释（行首加 #），PassWall 加载时忽略，不丢数据
//   - 「提交并应用」：分别把两张表回写到对应文件，再触发后台 reload PassWall
//   - 「重新加载」：丢弃本地未提交改动，重新从路由器拉两份文件
import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

export default async function mount(root, ctx) {
  const ipPath = ctx.report?.direct_ip_path;
  if (!ipPath) {
    root.append(el("div", { class: "error" }, "未检测到 direct_ip 路径。先在 总览 重新检测。"));
    return;
  }
  // detect.rs 只暴露 direct_ip_path；direct_host 与之同目录、改文件名即可
  const hostPath = ipPath.replace(/direct_ip$/, "direct_host");

  // 两个独立的表数据（互不影响）
  const groups = {
    ip:   { path: ipPath,   entries: [], extras: [], tbody: null, countEl: null, title: "IP / CIDR",
            placeholder: "1.2.3.4 / 1.2.3.0/24 / 2001:db8::/32",
            tip: "支持 IPv4 / IPv4-CIDR / IPv6 / IPv6-CIDR" },
    host: { path: hostPath, entries: [], extras: [], tbody: null, countEl: null, title: "域名",
            placeholder: "example.com / .cdn.example.com",
            tip: "一行一个域名，PassWall 会做后缀匹配" },
  };
  let dirty = false;
  let dirtyTag, submitBtn;

  function markDirty() {
    dirty = true;
    if (dirtyTag) dirtyTag.hidden = false;
    if (submitBtn) submitBtn.disabled = false;
  }

  function rowEl(g, entry, idx) {
    return el("tr", { class: entry.enabled ? "" : "row-disabled" }, [
      el("td", { style: { width: "60px" } }, [
        el("label", { class: "switch", tip: "关闭 = 行首加 #，PassWall 不加载，但不丢数据" }, [
          (() => {
            const cb = el("input", { type: "checkbox" });
            cb.checked = !!entry.enabled;
            cb.addEventListener("change", () => {
              entry.enabled = cb.checked;
              markDirty();
              render(g);
            });
            return cb;
          })(),
          el("span", { class: "switch-slider" }),
        ]),
      ]),
      el("td", {}, [
        (() => {
          const inp = el("input", { type: "text", value: entry.value,
            placeholder: g.placeholder, tip: g.tip, style: { width: "100%" } });
          inp.addEventListener("input", () => { entry.value = inp.value.trim(); markDirty(); });
          return inp;
        })(),
      ]),
      el("td", {}, [
        (() => {
          const inp = el("input", { type: "text", value: entry.comment,
            placeholder: "备注（可选）", tip: "写明用途方便日后回顾", style: { width: "100%" } });
          inp.addEventListener("input", () => { entry.comment = inp.value; markDirty(); });
          return inp;
        })(),
      ]),
      el("td", { style: { width: "60px", textAlign: "center" } }, [
        el("button", { class: "danger ghost", tip: "删除此行", onclick: () => {
          g.entries.splice(idx, 1);
          markDirty();
          render(g);
        } }, "✕"),
      ]),
    ]);
  }

  function render(g) {
    if (!g.tbody) return;
    g.tbody.replaceChildren(...g.entries.map((e, i) => rowEl(g, e, i)));
    if (!g.entries.length) {
      g.tbody.append(el("tr", {}, el("td", { colspan: "4", class: "hint",
        style: { textAlign: "center", padding: "12px" } }, "（空，点 + 新增一行）")));
    }
    if (g.countEl) {
      const total = g.entries.length;
      const on = g.entries.filter(e => e.enabled).length;
      g.countEl.textContent = `${total} 条 / 启用 ${on}`;
    }
  }

  async function loadOne(g) {
    try {
      const data = await api.directListStructured(g.path);
      g.entries = (data.entries || []).map(e => ({ ...e }));
      g.extras  = data.extras || [];
    } catch (e) {
      // 文件不存在算空表，不抛
      g.entries = [];
      g.extras = [];
      console.warn(`load ${g.path} failed`, e);
    }
    render(g);
  }
  async function loadAll() {
    await Promise.all([loadOne(groups.ip), loadOne(groups.host)]);
    dirty = false;
    if (dirtyTag) dirtyTag.hidden = true;
    if (submitBtn) submitBtn.disabled = true;
  }

  function cleanOne(g) {
    const cleaned = g.entries
      .map(e => ({ value: (e.value || "").trim(),
                   comment: (e.comment || "").trim(),
                   enabled: !!e.enabled }))
      .filter(e => e.value.length > 0);
    const seen = new Set(); const dup = [];
    for (const e of cleaned) { if (seen.has(e.value)) dup.push(e.value); seen.add(e.value); }
    return { cleaned, dup };
  }

  async function submit() {
    const a = cleanOne(groups.ip);
    const b = cleanOne(groups.host);
    if (a.dup.length || b.dup.length) {
      toast("存在重复条目：" + [...a.dup, ...b.dup].join(", "), "warn");
      return;
    }
    if (!confirm(`将写入：\n  ${a.cleaned.length} 条 → ${groups.ip.path}\n  ${b.cleaned.length} 条 → ${groups.host.path}\n然后 reload PassWall。继续？`)) return;

    const btn = submitBtn; const old = btn.textContent;
    btn.disabled = true; btn.textContent = "提交中…";
    try {
      await api.directSaveStructured(groups.ip.path,   a.cleaned, groups.ip.extras);
      await api.directSaveStructured(groups.host.path, b.cleaned, groups.host.extras);
      try { await api.reloadService(ctx.report?.init_script || ctx.config); }
      catch (e) { toast("写入成功但 reload 失败", "warn", { detail: formatError(e) }); }
      toast(`✓ 已写入 IP ${a.cleaned.length} + 域名 ${b.cleaned.length} 并 reload`);
      groups.ip.entries = a.cleaned;
      groups.host.entries = b.cleaned;
      render(groups.ip); render(groups.host);
      dirty = false;
      if (dirtyTag) dirtyTag.hidden = true;
    } catch (e) {
      toast("提交失败", "warn", { detail: formatError(e) });
    } finally {
      btn.disabled = false; btn.textContent = old;
    }
  }

  function buildSubTable(g, valueColTitle) {
    g.tbody = el("tbody", {});
    g.countEl = el("span", { class: "mono small muted" }, "—");
    const addBtn = el("button", { class: "ghost", tip: `在 ${g.title} 表底加一行`, onclick: () => {
      g.entries.push({ value: "", comment: "", enabled: true });
      markDirty(); render(g);
    } }, "+ 新增一行");
    const reloadBtn = el("button", { class: "ghost", tip: `只重新加载 ${g.title} 子表（丢弃本地未提交修改）`,
      onclick: async () => {
        if (dirty && !confirm(`有未提交改动，确认丢弃并重新拉取 ${g.title}？`)) return;
        await loadOne(g);
      } }, "↻");

    return el("section", { class: "card", style: { marginTop: "12px" } }, [
      el("div", { class: "row", style: { alignItems: "center" } }, [
        el("h4", { style: { margin: 0 } }, g.title),
        el("span", { class: "small muted" }, "· " + g.path),
        el("span", { class: "grow" }),
        g.countEl, addBtn, reloadBtn,
      ]),
      el("table", { class: "sys-table direct-table" }, [
        el("thead", {}, el("tr", {}, [
          el("th", { style: { width: "60px" } }, "启用"),
          el("th", {}, valueColTitle),
          el("th", {}, "备注"),
          el("th", {}, ""),
        ])),
        g.tbody,
      ]),
    ]);
  }

  dirtyTag = el("span", { class: "dirty-indicator", hidden: true }, "● 有未提交改动");
  submitBtn = el("button", { class: "primary", disabled: true,
    tip: "把两张子表分别写回 direct_ip / direct_host，并 reload PassWall", onclick: submit }, "提交并应用");

  root.append(
    el("section", { class: "card" }, [
      el("h3", {}, "直连列表（按目标 · 绕过代理）"),
      el("p", { class: "hint" }, [
        "命中此列表的 ", el("b", {}, "目标地址"), " 会 ",
        el("b", {}, "绕过 PassWall 走直连出口"), "。",
        "对应 LuCI「直连列表」页的两段 textarea，KWRT 原本就把 ",
        el("code", {}, "direct_ip"), " 和 ", el("code", {}, "direct_host"),
        " 拆成两个独立文件 —— 本页保持这种拆分，但用一个「提交并应用」一次写两份并触发一次 reload。",
      ]),
      el("div", { class: "row" }, [
        el("button", { class: "ghost", tip: "重新拉取两份文件，丢弃本地未提交修改",
          onclick: async () => {
            if (dirty && !confirm("有未提交的修改，确认丢弃并重新拉取？")) return;
            await loadAll();
          } }, "↻ 重新加载全部"),
        el("span", { class: "grow" }),
        dirtyTag, submitBtn,
      ]),
    ]),
    buildSubTable(groups.ip,   "IP / CIDR"),
    buildSubTable(groups.host, "域名"),
  );

  await loadAll();
}
