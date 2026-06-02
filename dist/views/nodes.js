import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

// 节点测试三件套——全部走 LuCI 现成 JSON 端点：
//  Ping:   /admin/services/passwall/ping_node?type=icmp&address=...&port=...
//  TCPing: 同上 type=tcping
//  URL:    /admin/services/passwall/urltest_node?id=<section>
//  URL 测试用的目标 URL 由路由器侧 uci global_other.url_test_url 控制（默认 google 204）。

export default async function mount(root, ctx) {
  const ov = ctx.overview;
  const cur = ov.global || {};

  // 结果缓存：name -> { icmp?, tcp?, url? }
  const results = new Map();
  let chosenMethod = "tcp";

  // ── 导入栏 ──
  const importInput = el("input", {
    placeholder: "粘贴 vless:// / vmess:// / hy2:// / trojan:// / ss:// 链接",
    tip: "支持单条链接，回车或点按钮直接导入到 PassWall 节点池",
    style: { width: "100%" },
  });
  importInput.addEventListener("keydown", (e) => { if (e.key === "Enter") doImport(); });
  async function doImport() {
    const link = importInput.value.trim();
    if (!link) return;
    try {
      const name = await api.importNode(ctx.config, link);
      toast(`已导入节点 ${name}`);
      importInput.value = "";
      await ctx.refreshAndRedraw();
    } catch (e) { toast("操作失败", "warn", { detail: formatError(e) }); }
  }

  const importCard = el("section", { class: "card" }, [
    el("h3", {}, "导入节点链接"),
    el("p", { class: "hint" }, "粘贴一条订阅链接，会被解析后通过 uci add 写入 PassWall 节点列表。"),
    el("div", { class: "row" }, [
      importInput,
      el("button", { class: "primary", tip: "解析并加入节点池", onclick: doImport }, "导入"),
    ]),
  ]);

  // ── 工具栏：批量测试 + 排序 ──
  const methodSel = el("select", {
    tip: "批量测试时用哪种方法",
    onchange: (e) => { chosenMethod = e.target.value; },
  }, [
    el("option", { value: "tcp" },  "TCP"),
    el("option", { value: "icmp" }, "Ping"),
    el("option", { value: "url" },  "URL"),
  ]);

  const btnAll  = el("button", { class: "primary small", tip: "对全部节点跑一遍当前方法（并发 3）", onclick: () => testAll() }, "测全部");
  const btnStop = el("button", { class: "ghost small",   tip: "中断批量测试", disabled: true, onclick: () => { stopAll = true; } }, "停止");
  const btnSort = el("button", { class: "ghost small",   tip: "按当前方法延迟排序（未测/超时排后）", onclick: () => sortByLatency() }, "排序");
  const progress = el("span", { class: "muted small" }, "");

  const toolbar = el("div", { class: "row test-toolbar" }, [
    methodSel, btnAll, btnStop, btnSort, progress,
    el("span", { class: "muted small", style: { marginLeft: "auto" } },
      "URL 测试目标走路由器 uci global_other.url_test_url（默认 Google 204）"),
  ]);

  // ── 表格 ──
  const tbody = el("tbody");
  // node -> tr / cells，方便单点更新
  const rowMap = new Map(); // name -> { tr, cells: { ping, tcp, url } }

  // 按当前数据顺序构建
  let nodes = ov.nodes.slice();
  renderRows();

  function renderRows() {
    tbody.replaceChildren();
    rowMap.clear();
    for (const n of nodes) {
      const name = n[".name"];
      const isTcp = cur.tcp_node === name;
      const isUdp = cur.udp_node === name;

      const cells = {
        icmp: el("span", { class: "chip-cell" }, "—"),
        tcp:  el("span", { class: "chip-cell" }, "—"),
        url:  el("span", { class: "chip-cell" }, "—"),
      };

      const mkChip = (label, method, tip) => el("span", {
        class: "test-chip", tip,
        onclick: () => runOne(n, method),
      }, label);

      const tr = el("tr", { class: (isTcp || isUdp) ? "current" : "" }, [
        el("td", { tip: "PassWall 内部 section 名（写 uci 用）" }, name),
        el("td", { tip: "节点备注名" }, n.remarks || ""),
        el("td", { tip: "协议" }, n.protocol || n.type || ""),
        el("td", { class: "mono", tip: "地址" }, n.address || ""),
        el("td", { class: "mono", tip: "端口" }, n.port || ""),
        el("td", { tip: "已是 TCP/UDP 出口的标记" }, [
          isTcp ? el("span", { class: "tag tag-tcp" }, "T") : "",
          isUdp ? el("span", { class: "tag tag-udp" }, "U") : "",
        ]),
        el("td", { class: "test-col" }, [
          el("div", { class: "test-line" }, [ mkChip("P", "icmp", "Ping（ICMP）"),  cells.icmp ]),
          el("div", { class: "test-line" }, [ mkChip("T", "tcp",  "TCPing（握手）"), cells.tcp  ]),
          el("div", { class: "test-line" }, [ mkChip("U", "url",  "URL 真代理"),     cells.url  ]),
        ]),
        el("td", { class: "op-col" }, [
          actionBtn("TCP出口", `设为 TCP 主出口`, () => setNode("tcp_node", name)),
          actionBtn("UDP出口", `设为 UDP 出口`,   () => setNode("udp_node", name)),
          actionBtn("删除",    "从 PassWall 删除该节点", async () => {
            if (!confirm(`确认删除节点 ${n.remarks || name}？`)) return;
            try { await api.deleteSection(ctx.config, name); await ctx.refreshAndRedraw(); toast(`✓ 已删除 ${n.remarks || name}`); }
            catch (e) { toast("删除失败", "warn", { detail: formatError(e) }); }
          }, "danger ghost"),
        ]),
      ]);
      rowMap.set(name, { tr, cells });
      tbody.append(tr);

      // 回填已缓存的结果
      const r = results.get(name);
      if (r) {
        if (r.icmp) renderCell(cells.icmp, r.icmp);
        if (r.tcp)  renderCell(cells.tcp,  r.tcp);
        if (r.url)  renderCell(cells.url,  r.url);
      }
    }
    if (!nodes.length) {
      tbody.append(el("tr", {}, el("td", {
        colspan: 8, class: "muted",
        style: { textAlign: "center", padding: "12px" }
      }, "暂无节点。可在上方 导入节点链接 添加。")));
    }
  }

  function renderCell(cellEl, r) {
    cellEl.replaceChildren();
    if (!r) { cellEl.textContent = "—"; return; }
    if (r.ok && r.latency_ms != null) {
      const ms = Math.round(r.latency_ms);
      const cls = ms < 150 ? "ok" : ms < 400 ? "warn" : "bad";
      cellEl.className = "chip-cell badge-" + cls;
      cellEl.textContent = `${ms}ms`;
    } else {
      cellEl.className = "chip-cell muted";
      cellEl.textContent = r.summary && r.summary.length <= 8 ? r.summary : "✗";
    }
    cellEl.title = r.summary || "";
    cellEl.style.cursor = "pointer";
    cellEl.onclick = () => toast(r.summary || "", r.ok ? "ok" : "warn", { detail: r });
  }

  async function runOne(n, method) {
    const name = n[".name"];
    const cells = rowMap.get(name)?.cells;
    const cell = cells?.[method];
    if (cell) { cell.textContent = "…"; cell.className = "chip-cell muted"; }
    try {
      let r;
      if (method === "icmp")     r = await api.testIcmp(n.address, Number(n.port || 0));
      else if (method === "tcp") r = await api.testTcping(n.address, Number(n.port || 0));
      else if (method === "url") r = await api.testUrl(name);
      const cur = results.get(name) || {};
      cur[method] = r;
      results.set(name, cur);
      if (cell) renderCell(cell, r);
    } catch (e) {
      if (cell) { cell.textContent = "出错"; cell.className = "chip-cell muted"; cell.onclick = () => toast("测试失败", "warn", { detail: formatError(e) }); }
    }
  }

  // ── 批量测试（并发 3） ──
  let stopAll = false;
  async function testAll() {
    if (!nodes.length) return;
    stopAll = false;
    btnAll.disabled = true; btnStop.disabled = false;
    const total = nodes.length; let done = 0;
    const queue = nodes.slice();
    const workers = Array.from({ length: 3 }, async () => {
      while (queue.length && !stopAll) {
        const n = queue.shift();
        await runOne(n, chosenMethod);
        done++;
        progress.textContent = `进度 ${done}/${total}`;
      }
    });
    await Promise.all(workers);
    btnAll.disabled = false; btnStop.disabled = true;
    progress.textContent = stopAll ? `已停止（${done}/${total}）` : `完成 ${done}/${total}`;
  }

  function sortByLatency() {
    const m = chosenMethod;
    nodes.sort((a, b) => {
      const ra = (results.get(a[".name"]) || {})[m];
      const rb = (results.get(b[".name"]) || {})[m];
      const va = (ra && ra.ok && ra.latency_ms != null) ? ra.latency_ms : Infinity;
      const vb = (rb && rb.ok && rb.latency_ms != null) ? rb.latency_ms : Infinity;
      return va - vb;
    });
    renderRows();
    toast(`已按 ${m === "icmp" ? "Ping" : m === "tcp" ? "TCPing" : "URL"} 延迟排序`, "info");
  }

  const card = el("section", { class: "card" }, [
    el("h3", {}, `节点列表（共 ${ov.nodes.length} 个）`),
    toolbar,
    el("table", { class: "grid" }, [
      el("thead", {}, el("tr", {}, [
        el("th", { tip: "uci section 名" }, "section"),
        el("th", {}, "备注"),
        el("th", {}, "协议"),
        el("th", {}, "地址"),
        el("th", {}, "端口"),
        el("th", { tip: "T=已是 TCP 出口  U=已是 UDP 出口" }, "出口"),
        el("th", { tip: "P=Ping  T=TCPing  U=URL。点字母测试，点结果看详情" }, "测试"),
        el("th", {}, "操作"),
      ])),
      tbody,
    ]),
  ]);

  root.append(importCard, card);

  async function setNode(kind, name) {
    try {
      await api.patchGlobal(ctx.config, { [kind]: name });
      toast(`✓ 已设为 ${kind === "tcp_node" ? "TCP" : "UDP"} 出口`);
      await ctx.refreshAndRedraw();
    } catch (e) { toast("设置出口节点失败", "warn", { detail: formatError(e) }); }
  }
}

function actionBtn(text, tip, on, cls = "ghost") {
  return el("button", { class: cls, tip, onclick: on, style: { marginRight: "4px" } }, text);
}
