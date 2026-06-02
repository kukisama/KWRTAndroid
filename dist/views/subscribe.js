import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";
import { field, textarea } from "./_form.js";

export default async function mount(root, ctx) {
  const subs = ctx.overview.subscribes || [];

  root.append(el("section", { class: "card" }, [
    el("h3", {}, "节点订阅"),
    el("p", { class: "hint" }, "更新订阅会调用 PassWall 自带 subscribe.lua。若 ACL 禁用了 file.exec，会收到错误，可改为在路由器上手动跑订阅。"),
  ]));

  if (!subs.length) {
    root.append(el("div", { class: "msg warn" }, "暂无订阅 section。"));
    return;
  }

  for (const s of subs) {
    const name = s[".name"];
    const form = el("form");
    form.addEventListener("submit", e => e.preventDefault());

    const remarks = el("input", { name: "remarks", value: s.remarks || "", tip: "订阅备注名" });
    const url = el("input", { name: "url", value: s.url || "", tip: "订阅链接（http/https）", style: { width: "100%" } });
    const keyword = el("input", { name: "filter_keyword", value: arrJoin(s.filter_keyword), tip: "节点名包含这些关键字才保留（逗号分隔）；留空不过滤" });
    const discard = el("input", { name: "discard_keyword", value: arrJoin(s.discard_keyword), tip: "节点名包含这些关键字会被丢弃" });

    form.append(
      el("div", { class: "row" }, [
        field(`备注 [${name}]`, "改名不影响内部 uci section", remarks),
      ]),
      el("div", { class: "row" }, [field("订阅 URL", "从这里拉节点列表", url)]),
      el("div", { class: "row" }, [
        field("保留关键字", "节点名包含才保留，多个用逗号", keyword),
        field("丢弃关键字", "节点名包含则丢弃", discard),
      ]),
      el("div", { class: "row actions" }, [
        el("button", { class: "primary", tip: "保存这条订阅的配置（不会立刻拉取节点）", onclick: async () => {
          try {
            await api.patchSection(ctx.config, name, {
              remarks: remarks.value, url: url.value,
              filter_keyword: keyword.value, discard_keyword: discard.value,
            });
            toast("已保存"); await ctx.refreshAndRedraw();
          } catch (e) { toast(formatError(e), "warn"); }
        } }, "保存"),
        el("button", { class: "ghost", tip: "立即触发订阅更新（调用 subscribe.lua start <section>）", onclick: async () => {
          try { const out = await api.updateSubscribe(ctx.config, name); toast("已触发：" + out.slice(0, 120)); }
          catch (e) { toast(formatError(e), "warn"); }
        } }, "立即更新"),
        el("button", { class: "danger ghost", tip: "删除这条订阅", onclick: async () => {
          if (!confirm(`删除订阅 ${name}?`)) return;
          try { await api.deleteSection(ctx.config, name); await ctx.refreshAndRedraw(); toast("已删除"); }
          catch (e) { toast(formatError(e), "warn"); }
        } }, "删除"),
      ]),
    );

    root.append(el("section", { class: "card" }, [
      el("h4", { style: { margin: "0 0 8px" } }, `▸ ${s.remarks || name}`),
      form,
    ]));
  }
}

function arrJoin(v) {
  if (Array.isArray(v)) return v.join(",");
  return v ?? "";
}
