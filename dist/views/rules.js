import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";
import { field } from "./_form.js";

// PassWall 规则源在 globals 里通常是 global_rules section
export default async function mount(root, ctx) {
  const gs = ctx.overview.globals || {};
  const rules = gs.global_rules || pickByType(ctx.overview.raw?.values, "global_rules") || {};
  const name = rules[".name"];

  const form = el("form");
  form.addEventListener("submit", e => e.preventDefault());

  const FIELDS = [
    ["gfwlist_url", "GFWList 源", "GFW 列表（被墙域名）的下载 URL，多行可拼"],
    ["chnlist_url", "ChnList 源", "中国大陆域名列表"],
    ["chnroute_url", "ChnRoute 源", "中国大陆 IPv4 路由表"],
    ["chnroute6_url", "ChnRoute6 源", "中国大陆 IPv6 路由表"],
    ["geosite_url", "Geosite 源", "Xray 的 geosite.dat 下载地址"],
    ["geoip_url", "GeoIP 源", "Xray 的 geoip.dat 下载地址"],
    ["v2ray_location_asset", "规则资源目录", "Xray 规则文件存放路径，一般 /usr/share/v2ray/"],
  ];

  const inputs = {};
  for (const [k, label, tip] of FIELDS) {
    const v = Array.isArray(rules[k]) ? rules[k].join("\n") : rules[k] || "";
    const ctrl = k === "v2ray_location_asset"
      ? el("input", { name: k, value: v, tip })
      : (() => { const t = el("textarea", { name: k, tip, rows: 2 }); t.value = v; return t; })();
    inputs[k] = ctrl;
    form.append(el("div", { class: "row" }, [field(label, tip, ctrl)]));
  }

  form.append(el("div", { class: "row actions" }, [
    el("button", { class: "primary", tip: "保存源地址（不会自动下载）", onclick: async () => {
      if (!name) { toast("找不到 global_rules section", "warn"); return; }
      const patch = {};
      for (const [k] of FIELDS) patch[k] = inputs[k].value;
      try { await api.patchSection(ctx.config, name, patch); toast("已保存"); await ctx.refreshAndRedraw(); }
      catch (e) { toast("操作失败", "warn", { detail: formatError(e) }); }
    } }, "保存"),
    el("button", { class: "ghost", tip: "调用 rule_update.lua 立即下载并更新规则文件", onclick: async () => {
      try { const out = await api.updateRules(ctx.config); toast("已触发：" + out.slice(0, 120)); }
      catch (e) { toast("操作失败", "warn", { detail: formatError(e) }); }
    } }, "立即更新规则"),
  ]));

  root.append(el("section", { class: "card" }, [el("h3", {}, `规则源 (${name || "找不到 global_rules"})`), form]));
}

function pickByType(values, t) {
  if (!values) return null;
  for (const v of Object.values(values)) if (v && v[".type"] === t) return v;
  return null;
}
