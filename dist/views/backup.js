import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

export default async function mount(root, ctx) {
  const out = el("textarea", { tip: "点击右侧"读取当前配置"会把 /etc/config/passwall 整个内容拉进来", rows: 22, style: { width: "100%" } });

  async function doRead() {
    try { out.value = await api.backup(ctx.config); toast("已读取"); }
    catch (e) { toast(formatError(e), "warn"); }
  }
  async function doSaveFile() {
    const blob = new Blob([out.value], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = el("a", { href: url, download: `${ctx.config}.uci.bak` });
    document.body.append(a); a.click(); a.remove();
    URL.revokeObjectURL(url);
  }
  async function doRestore() {
    if (!out.value.trim()) { toast("内容为空", "warn"); return; }
    if (!confirm("确认覆盖路由器上的 /etc/config/" + ctx.config + "？将立即 reload。")) return;
    try { await api.restore(ctx.config, out.value); toast("已写回"); await ctx.refreshAndRedraw(); }
    catch (e) { toast(formatError(e), "warn"); }
  }

  root.append(el("section", { class: "card" }, [
    el("h3", {}, "备份 / 恢复整个 PassWall 配置"),
    el("p", { class: "hint" }, "整个 /etc/config/" + ctx.config + " 文件读写。强烈建议改大配置前先点"读取并保存为文件"做备份。"),
    el("div", { class: "row" }, [
      el("button", { class: "primary", tip: "从路由器读取当前完整配置", onclick: doRead }, "读取当前配置"),
      el("button", { class: "ghost", tip: "把上面的文本框存为本地文件", onclick: doSaveFile }, "保存为本地文件"),
      el("button", { class: "danger ghost", tip: "用文本框内容覆盖路由器上的配置", onclick: doRestore }, "写回路由器"),
    ]),
    out,
  ]));
}
