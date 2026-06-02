import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";
import { field, select } from "./_form.js";

export default async function mount(root, ctx) {
  const gs = ctx.overview.globals || {};
  const fwd = gs.global_forwarding || pickByType(ctx.overview.raw?.values, "global_forwarding") || {};
  const name = fwd[".name"];

  const form = el("form");
  form.addEventListener("submit", e => e.preventDefault());

  const tcpRedir = el("input", { name: "tcp_redir_ports", value: fwd.tcp_redir_ports || "", tip: "需要重定向到代理的 TCP 端口列表，默认 1:65535（全部）" });
  const udpRedir = el("input", { name: "udp_redir_ports", value: fwd.udp_redir_ports || "", tip: "需要重定向到代理的 UDP 端口列表" });
  const udpDrop  = el("input", { name: "udp_proxy_drop_ports", value: fwd.udp_proxy_drop_ports || "", tip: "代理 UDP 时丢弃的端口（常用：80,443 来禁用 QUIC）" });

  const useNftSel = select("use_nft", fwd.use_nft, [
    { value: "0", label: "0 - iptables" },
    { value: "1", label: "1 - nftables" },
  ], "底层防火墙：现代 OpenWrt 用 nftables，老固件用 iptables");

  const tcpWaySel = select("tcp_proxy_way", fwd.tcp_proxy_way, [
    { value: "redirect", label: "redirect（默认）" },
    { value: "tproxy", label: "tproxy" },
  ], "TCP 流量重定向方式：tproxy 兼容性更好但要求内核支持");

  form.append(
    el("div", { class: "row" }, [
      field("TCP 重定向端口", "默认 1:65535", tcpRedir),
      field("UDP 重定向端口", "默认 1:65535", udpRedir),
    ]),
    el("div", { class: "row" }, [field("UDP 代理丢弃端口", "建议填 80,443 禁用 QUIC，避免 YouTube 直连", udpDrop)]),
    el("div", { class: "row" }, [
      field("防火墙后端", "iptables 或 nftables", useNftSel),
      field("TCP 代理方式", "redirect 或 tproxy", tcpWaySel),
    ]),
    el("div", { class: "row actions" }, [
      el("button", { class: "primary", tip: "保存并 reload PassWall（规则会重新装载）", onclick: async () => {
        if (!name) { toast("找不到 global_forwarding section", "warn"); return; }
        try {
          await api.patchSection(ctx.config, name, {
            tcp_redir_ports: tcpRedir.value,
            udp_redir_ports: udpRedir.value,
            udp_proxy_drop_ports: udpDrop.value,
            use_nft: useNftSel.value,
            tcp_proxy_way: tcpWaySel.value,
          });
          toast("已保存"); await ctx.refreshAndRedraw();
        } catch (e) { toast("操作失败", "warn", { detail: formatError(e) }); }
      } }, "保存并应用"),
    ]),
  );

  root.append(el("section", { class: "card" }, [el("h3", {}, `端口 / 转发 (${name || "未找到 global_forwarding"})`), form]));
}

function pickByType(values, t) {
  if (!values) return null;
  for (const v of Object.values(values)) if (v && v[".type"] === t) return v;
  return null;
}
