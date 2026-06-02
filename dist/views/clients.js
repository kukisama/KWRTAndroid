// 客户端例外（简版只读视图）
//
// 设计目的：放在「总览」旁边，只用来日常点几下启用/停用，不做任何编辑/新增/删除。
// 改 ACL 详细字段去「客户端例外」完整页（views/acl.js）。
//
// 字段映射（来源 acl.rs / passwall.acl_rule）：
//   enabled / sources / remarks / interface / tcp_node / udp_node
//
// 交互：
//   - 表格：[启用按钮] [源 IP/MAC] [备注 / 接口] [TCP 节点] [UDP 节点]
//   - 启用按钮：绿色=启用 / 灰色=停用，点一下立即 uci set（暂存），不 reload
//   - 顶部：[刷新] [应用配置]
//   - 应用配置 = 后台 reload PassWall，让本次所有暂存的启用切换生效
import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

export default async function mount(root, ctx) {
  let snap = { acl_enable: false, rules: [] };
  let pending = false;
  let tbody, pendingBadge, applyBtn, refreshBtn, countEl;

  // 节点 id → 显示名映射，沿用 overview 数据，缺失就直接显示 id
  function nodeLabel(id) {
    if (!id || id === "nil") return el("span", { class: "muted small" }, "直连");
    if (id === "tcp") return el("span", { class: "muted small" }, "跟随 TCP");
    const n = (ctx.overview?.nodes || []).find(x => x[".name"] === id);
    if (!n) return el("span", { class: "mono small" }, id);
    return el("span", { class: "small", tip: `${n.protocol || n.type || "?"} · ${n.address || ""}:${n.port || ""}` },
      n.remarks || id);
  }

  function markPending() {
    pending = true;
    if (pendingBadge) pendingBadge.hidden = false;
  }
  function clearPending() {
    pending = false;
    if (pendingBadge) pendingBadge.hidden = true;
  }

  function enableBtn(r) {
    // 视觉：on = 绿色实心 / off = 灰色幽灵；带 tip
    const btn = el("button", {
      class: "pill-btn",
      tip: r.enabled ? "点击停用此规则（暂存，不立即生效）" : "点击启用此规则（暂存，不立即生效）",
    });
    function paint() {
      btn.textContent = r.enabled ? "● 启用" : "○ 停用";
      // 主题变量依赖 style.v4.css 定义：--ok / --card-2 / --line / --muted
      // 避免写死色值，深色主题下也能自动反转
      btn.style.background = r.enabled ? "var(--ok)" : "var(--card-2)";
      btn.style.color = r.enabled ? "#fff" : "var(--muted)";
      btn.style.borderColor = r.enabled ? "var(--ok)" : "var(--line)";
      btn.style.fontWeight = "600";
      btn.style.minWidth = "72px";
    }
    paint();
    btn.addEventListener("click", async () => {
      const next = !r.enabled;
      btn.disabled = true;
      try {
        await api.aclUpdate(ctx.config, { ...r, enabled: next });
        r.enabled = next;
        paint();
        const tr = btn.closest("tr");
        if (tr) tr.classList.toggle("row-disabled", !next);
        markPending();
      } catch (e) {
        toast("切换启用失败", "warn", { detail: formatError(e) });
      } finally { btn.disabled = false; }
    });
    return btn;
  }

  function rowEl(r) {
    return el("tr", { class: r.enabled ? "" : "row-disabled" }, [
      el("td", { style: { textAlign: "center", width: "92px" } }, enableBtn(r)),
      el("td", { class: "mono", style: { whiteSpace: "nowrap" } }, r.sources || "-"),
      el("td", {}, [
        r.remarks || el("span", { class: "muted" }, "(无备注)"),
        r.interface ? el("div", { class: "small muted" }, "接口: " + r.interface) : null,
      ]),
      el("td", {}, nodeLabel(r.tcp_node)),
      el("td", {}, nodeLabel(r.udp_node)),
    ]);
  }

  function rerender() {
    if (!tbody) return;
    tbody.replaceChildren(...snap.rules.map(rowEl));
    if (!snap.rules.length) {
      tbody.append(el("tr", {}, el("td", { colspan: "5", class: "hint",
        style: { textAlign: "center", padding: "16px" } },
        "暂无规则。如需新增/编辑，去「客户端例外」完整页。")));
    }
    if (countEl) {
      const on = snap.rules.filter(x => x.enabled).length;
      countEl.textContent = `${snap.rules.length} 条（启用 ${on}）`;
    }
  }

  async function load() {
    try { snap = await api.aclRead(ctx.config); }
    catch (e) {
      tbody.replaceChildren(el("tr", {}, el("td", { colspan: "5", class: "error" },
        ["读取 ACL 失败：", formatError(e)])));
      return;
    }
    rerender();
  }

  refreshBtn = el("button", { class: "ghost", tip: "重新从路由器拉取最新 ACL（不影响未应用的本地切换）" }, "↻ 刷新");
  refreshBtn.addEventListener("click", async () => {
    if (pending && !confirm("还有未应用的启用切换，刷新会拉到服务端最新状态，确认继续？")) return;
    refreshBtn.disabled = true;
    try { await load(); clearPending(); } finally { refreshBtn.disabled = false; }
  });

  applyBtn = el("button", { class: "primary", tip: "把上面那些启用/停用切换实际应用到 PassWall（后台 reload，约 10–30 秒生效）" }, "✓ 应用配置");
  applyBtn.addEventListener("click", async () => {
    applyBtn.disabled = true; const old = applyBtn.textContent; applyBtn.textContent = "应用中…";
    try {
      await api.aclReload(ctx.config);
      clearPending();
      toast("✓ 已触发后台 reload PassWall（约 10–30 秒生效）");
    } catch (e) {
      toast("应用配置失败", "warn", { detail: formatError(e) });
    } finally { applyBtn.disabled = false; applyBtn.textContent = old; }
  });

  pendingBadge = el("span", { class: "pill pill-warn", hidden: true, style: { marginLeft: "6px" } }, "有未应用改动");
  countEl = el("span", { class: "mono small muted" }, "—");

  tbody = el("tbody", {});
  const table = el("table", { class: "sys-table" }, [
    el("thead", {}, el("tr", {}, [
      el("th", { style: { width: "92px" } }, "启用"),
      el("th", {}, "源地址（IP / MAC）"),
      el("th", {}, "备注 / 接口"),
      el("th", {}, "TCP 节点"),
      el("th", {}, "UDP 节点"),
    ])),
    tbody,
  ]);

  root.append(
    el("section", { class: "card" }, [
      el("h3", {}, "客户端例外 · 快捷开关"),
      el("p", { class: "hint" }, [
        "只用来快速 ", el("b", {}, "启用 / 停用"), " 已有 ACL 规则，不编辑也不删除。",
        "新增 / 修改字段请去 ", el("b", {}, "「客户端例外」"), " 完整页。",
      ]),
      el("div", { class: "row" }, [
        refreshBtn,
        el("span", { class: "grow" }),
        countEl,
        pendingBadge,
        applyBtn,
      ]),
      table,
    ]),
  );

  await load();
}
