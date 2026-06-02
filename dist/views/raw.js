import { el } from "../dom.js";

// 把整个 ctx.overview.raw.values 按 type 分组、可折叠展示，让用户能看到所有字段。
export default async function mount(root, ctx) {
  const values = ctx.overview?.raw?.values || {};
  const all = Object.entries(values);
  // 按 type 分组
  const byType = new Map();
  for (const [name, sec] of all) {
    const t = sec?.[".type"] || "(?)";
    if (!byType.has(t)) byType.set(t, []);
    byType.get(t).push([name, sec]);
  }

  const filter = el("input", { placeholder: "过滤 type 或 section 名（输入即筛选）", tip: "支持按 type 名（如 nodes / shunt_rules）或 section 名搜索",
    style: { width: "100%", marginBottom: "10px" } });

  const list = el("div");

  function render() {
    const q = filter.value.trim().toLowerCase();
    list.replaceChildren();
    for (const [t, items] of byType) {
      const filtered = items.filter(([name]) => !q || t.toLowerCase().includes(q) || name.toLowerCase().includes(q));
      if (!filtered.length) continue;
      const groupDetails = el("details", { class: "raw-section", open: t !== "nodes" }); // nodes 默认收起，太多
      groupDetails.append(el("summary", { tip: `类型 ${t}：${items.length} 条` }, `${t}  (${items.length})`));
      const body = el("div");
      for (const [name, sec] of filtered) {
        const d = el("details", { class: "raw-section", style: { margin: "4px 8px 4px" } });
        d.append(el("summary", { tip: `section ${name}，type=${t}` }, `${name}`));
        d.append(el("pre", {}, JSON.stringify(sec, null, 2)));
        body.append(d);
      }
      groupDetails.append(body);
      list.append(groupDetails);
    }
  }
  filter.addEventListener("input", render);
  render();

  root.append(el("section", { class: "card" }, [
    el("h3", {}, `原始 UCI 配置 (${all.length} 个 section)`),
    el("p", { class: "hint" }, "这里把 ubus uci.get passwall 拉到的全部 section 原样展示。任何前几个 tab 没覆盖的字段，都能在这里看到。"),
    filter,
    list,
  ]));
}
