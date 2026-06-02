import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

export default async function mount(root, ctx) {
  const ov = ctx.overview;
  const cur = ov.global || {};

  // 导入栏
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
    } catch (e) { toast(formatError(e), "warn"); }
  }

  const importCard = el("section", { class: "card" }, [
    el("h3", {}, "导入节点链接"),
    el("p", { class: "hint" }, "粘贴一条订阅链接，会被解析后通过 uci add 写入 PassWall 节点列表。"),
    el("div", { class: "row" }, [
      importInput,
      el("button", { class: "primary", tip: "解析并加入节点池", onclick: doImport }, "导入"),
    ]),
  ]);

  // 表格
  const tbody = el("tbody");
  for (const n of ov.nodes) {
    const name = n[".name"];
    const isTcp = cur.tcp_node === name;
    const isUdp = cur.udp_node === name;
    const tr = el("tr", { class: (isTcp || isUdp) ? "current" : "" }, [
      el("td", { tip: "PassWall 内部 section 名（写 uci 用）" }, name),
      el("td", { tip: "节点备注名" }, n.remarks || ""),
      el("td", { tip: "协议（vless / vmess / hysteria2 …）" }, n.protocol || n.type || ""),
      el("td", { class: "mono", tip: "地址" }, n.address || ""),
      el("td", { class: "mono", tip: "端口" }, n.port || ""),
      el("td", {}, [
        isTcp ? el("span", { class: "muted small", tip: "已是 TCP 出口" }, "TCP出口") : "",
        " ",
        isUdp ? el("span", { class: "muted small", tip: "已是 UDP 出口" }, "UDP出口") : "",
      ]),
      el("td", {}, [
        actionBtn("设为TCP", `把"${n.remarks || name}"设为 TCP 主出口节点`, () => setNode("tcp_node", name)),
        actionBtn("设为UDP", "把它设为 UDP 出口节点", () => setNode("udp_node", name)),
        actionBtn("测连通", "在路由器侧 nc/ping 测试节点 address:port 是否可达", async () => {
          try {
            const r = await api.pingNode(n.address, Number(n.port || 0));
            toast(`连通测试：${JSON.stringify(r).slice(0, 200)}`);
          } catch (e) { toast(formatError(e), "warn"); }
        }),
        actionBtn("删除", "从 PassWall 删除该节点（uci delete + commit + reload）", async () => {
          if (!confirm(`确认删除节点 ${n.remarks || name}？`)) return;
          try { await api.deleteSection(ctx.config, name); await ctx.refreshAndRedraw(); toast("已删除"); }
          catch (e) { toast(formatError(e), "warn"); }
        }, "danger ghost"),
      ]),
    ]);
    tbody.append(tr);
  }
  if (!ov.nodes.length) {
    tbody.append(el("tr", {}, el("td", { colspan: 7, class: "muted", style: { textAlign: "center", padding: "12px" } }, "暂无节点。可在上方"导入节点链接"添加。")));
  }

  const card = el("section", { class: "card" }, [
    el("h3", {}, `节点列表（共 ${ov.nodes.length} 个）`),
    el("table", { class: "grid" }, [
      el("thead", {}, el("tr", {}, [
        el("th", { tip: "uci section 名" }, "section"),
        el("th", {}, "备注"),
        el("th", {}, "协议"),
        el("th", {}, "地址"),
        el("th", {}, "端口"),
        el("th", {}, "标记"),
        el("th", {}, "操作"),
      ])),
      tbody,
    ]),
  ]);

  root.append(importCard, card);

  async function setNode(kind, name) {
    try { await api.patchGlobal(ctx.config, { [kind]: name }); toast("已设置"); await ctx.refreshAndRedraw(); }
    catch (e) { toast(formatError(e), "warn"); }
  }
}

function actionBtn(text, tip, on, cls = "ghost") {
  return el("button", { class: cls, tip, onclick: on, style: { marginRight: "4px" } }, text);
}
