import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";
import { field, select } from "./_form.js";

export default async function mount(root, ctx) {
  const g = ctx.overview.global || {};
  const form = el("form");
  form.addEventListener("submit", e => e.preventDefault());

  const modeSel = select("dns_mode", g.dns_mode, [
    { value: "dns2socks", label: "dns2socks（默认推荐）" },
    { value: "udp", label: "udp" },
    { value: "tcp", label: "tcp" },
    { value: "doh", label: "doh / DoH" },
    { value: "xray", label: "xray" },
    { value: "sing-box", label: "sing-box" },
  ], "PassWall 远程 DNS 查询方式；dns2socks 即把 DNS 走当前代理。");

  const remote = el("input", { name: "remote_dns", value: g.remote_dns || "", tip: "远程 DNS 服务器（默认 1.1.1.1:53），DoH 时填 https://...）", style: { width: "100%" } });

  const shuntSel = select("dns_shunt", g.dns_shunt, [
    { value: "dnsmasq", label: "dnsmasq（OpenWrt 默认）" },
    { value: "chinadns-ng", label: "chinadns-ng" },
    { value: "smartdns", label: "smartdns" },
  ], "DNS 分流前端：决定查询先经过谁");

  const v6Sel = select("filter_proxy_ipv6", g.filter_proxy_ipv6, [
    { value: "0", label: "0 - 允许代理 IPv6" },
    { value: "1", label: "1 - 不解析代理域名的 AAAA（避免 IPv6 泄漏）" },
  ], "为代理域名屏蔽 IPv6 解析；多数家庭网用 1 更稳");

  form.append(
    el("div", { class: "row" }, [
      field("DNS 模式", "远程 DNS 查询方式", modeSel),
      field("DNS 前端", "dnsmasq / chinadns-ng / smartdns", shuntSel),
    ]),
    el("div", { class: "row" }, [field("远程 DNS 地址", "支持 ip:port 或 https:// 形式", remote)]),
    el("div", { class: "row" }, [field("过滤代理 IPv6", "对代理域名的 AAAA 解析行为", v6Sel)]),
    el("div", { class: "row actions" }, [
      el("button", { class: "primary", tip: "保存到 uci 并 reload PassWall", onclick: async () => {
        try {
          await api.patchGlobal(ctx.config, {
            dns_mode: modeSel.value, remote_dns: remote.value,
            dns_shunt: shuntSel.value, filter_proxy_ipv6: v6Sel.value,
          });
          toast("DNS 设置已保存"); await ctx.refreshAndRedraw();
        } catch (e) { toast(formatError(e), "warn"); }
      } }, "保存并应用"),
    ]),
  );

  root.append(el("section", { class: "card" }, [el("h3", {}, "DNS 设置"), form]));
}
