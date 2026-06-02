import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";
import { field, select, checkbox } from "./_form.js";

export default async function mount(root, ctx) {
  const ov = ctx.overview;
  if (!ov) { root.append(el("div", { class: "error", text: "尚未拿到 overview" })); return; }
  const g = ov.global || {};

  const nodeOptions = [{ value: "nil", label: "（不使用 / 直连）" }]
    .concat(ov.nodes.map(n => ({
      value: n[".name"],
      label: `${n.remarks || "(未命名)"}  [${n.protocol || n.type || "?"}]  ${n.address || ""}:${n.port || ""}`
    })));

  const card = el("section", { class: "card" }, [
    el("h3", {}, "运行状态"),
    el("div", { class: "kv" }, [
      el("div", { class: "k" }, "主配置"),     el("div", { class: "v mono" }, ov.config),
      el("div", { class: "k" }, "已识别节点"), el("div", { class: "v" }, String(ov.nodes.length)),
      el("div", { class: "k" }, "分流规则"),   el("div", { class: "v" }, String(ov.shunt_rules.length)),
      el("div", { class: "k" }, "订阅"),       el("div", { class: "v" }, String(ov.subscribes.length)),
    ]),
  ]);

  const form = el("form", { id: "form-dashboard" });
  form.addEventListener("submit", (e) => e.preventDefault());

  const enabledCb = checkbox("enabled", g.enabled, "PassWall 总开关：关闭后不会拦截/转发任何流量", "启用 PassWall 主开关");
  const socksCb = checkbox("socks_enabled", g.socks_enabled, "在路由器上同时暴露一个 Socks5 代理端口，供别的设备主动用", "启用 Socks 代理");

  const tcpSel = select("tcp_node", g.tcp_node, nodeOptions,
    "TCP 主出口节点：所有走代理的 TCP 流量出口");
  const udpSel = select("udp_node", g.udp_node, nodeOptions,
    "UDP 出口节点：填 'nil' 表示 UDP 不走代理，填 'tcp' 表示跟随 TCP 节点（v1 的特殊值）");

  form.append(
    el("div", { class: "row" }, [enabledCb, socksCb]),
    el("div", { class: "row" }, [
      field("TCP 出口节点", "改完按下方"保存"按钮即生效", tcpSel),
      field("UDP 出口节点", "可填特殊值 nil（不走代理）/ tcp（同 TCP）", udpSel),
    ]),
    el("div", { class: "row actions" }, [
      el("button", { class: "primary", tip: "把以上更改写入 /etc/config/passwall 并 reload 服务", onclick: async (e) => {
        e.preventDefault();
        const patch = {
          enabled: enabledCb.querySelector("input").checked ? "1" : "0",
          socks_enabled: socksCb.querySelector("input").checked ? "1" : "0",
          tcp_node: tcpSel.value,
          udp_node: udpSel.value,
        };
        try {
          await api.patchGlobal(ctx.config, patch);
          toast("已保存并 reload");
          await ctx.refreshAndRedraw();
        } catch (err) { toast(formatError(err), "warn"); }
      } }, "保存并应用"),
    ]),
  );

  const card2 = el("section", { class: "card" }, [
    el("h3", {}, "全局开关 / 当前节点"),
    form,
  ]);

  root.append(card, card2);
}
