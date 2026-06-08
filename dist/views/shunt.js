import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";
import { field, textarea, select } from "./_form.js";

export default async function mount(root, ctx) {
  const ov = ctx.overview;
  const rules = ov.shunt_rules || [];
  const nodeOptions = [
    { value: "", label: "（继承全局）" },
    { value: "_default_", label: "_default_（按主出口）" },
    { value: "_direct_", label: "_direct_（直连）" },
    { value: "_blackhole_", label: "_blackhole_（黑洞 / 丢弃）" },
  ].concat(ov.nodes.map(n => ({
    value: n[".name"],
    label: `${n.remarks || "(未命名)"}  ${n.address || ""}`,
  })));

  root.append(el("section", { class: "card" }, [
    el("h3", {}, "分流规则"),
    el("p", { class: "hint" }, "对应 PassWall 分流 页：为不同站点 / 网段指定不同节点（或直接直连）。每条规则保存后自动 reload。"),
  ]));

  if (!rules.length) {
    root.append(el("div", { class: "msg warn" }, "没有 shunt_rules section。是否安装的不是标准 PassWall？"));
  }

  for (const r of rules) {
    root.append(renderRule(r, ctx, nodeOptions));
  }
}

function renderRule(r, ctx, nodeOptions) {
  const name = r[".name"];
  const form = el("form");
  form.addEventListener("submit", (e) => e.preventDefault());

  const remarks = el("input", { name: "remarks", value: r.remarks || "", tip: "规则备注名，仅展示用" });
  const domains = textarea("domain_list", normalizeList(r.domain_list), "命中这些域名的流量走指定节点；一行一条，支持 regex:/.../ 和 geosite:cn 等 PassWall 语法", { rows: 4 });
  const ips = textarea("ip_list", normalizeList(r.ip_list), "命中这些 IP / CIDR 的流量走指定节点；一行一条；支持 geoip:cn", { rows: 4 });
  const nodeSel = select("node", r.node || "", nodeOptions, "命中后使用的出口节点，_direct_ = 直连，_blackhole_ = 丢弃");

  form.append(
    el("div", { class: "row" }, [
      field(`规则名 [${name}]`, "uci section 名（修改备注不会改它）", remarks),
      field("出口节点", "命中后流量走哪", nodeSel),
    ]),
    el("div", { class: "row" }, [
      field("Domain 列表", "一行一条；可用 regex:/foo/、geosite:cn、domain:example.com", domains),
      field("IP 列表", "一行一条；可用 geoip:cn 或 CIDR", ips),
    ]),
    el("div", { class: "row actions" }, [
      el("button", { class: "primary", tip: "把上面修改写入 uci 并 reload PassWall", onclick: async () => {
        const patch = {
          remarks: remarks.value,
          domain_list: domains.value,
          ip_list: ips.value,
          node: nodeSel.value,
        };
        try { await api.patchSection(ctx.config, name, patch); toast(`已保存：${name}`); await ctx.refreshAndRedraw(); }
        catch (e) { toast("操作失败", "warn", { detail: formatError(e) }); }
      } }, "保存"),
      el("button", { class: "danger ghost", tip: "删除整条分流规则", onclick: async () => {
        if (!confirm(`确认删除规则 ${name} (${r.remarks || ""})?`)) return;
        try { await api.deleteSection(ctx.config, name); await ctx.refreshAndRedraw(); toast("已删除"); }
        catch (e) { toast("操作失败", "warn", { detail: formatError(e) }); }
      } }, "删除"),
    ]),
  );

  return el("section", { class: "card" }, [
    el("h4", { style: { margin: "0 0 8px" } }, `▸ ${r.remarks || name}`),
    form,
  ]);
}

function normalizeList(v) {
  if (Array.isArray(v)) return v.join("\n");
  return v ?? "";
}
