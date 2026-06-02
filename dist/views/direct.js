import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

export default async function mount(root, ctx) {
  const path = ctx.report?.direct_ip_path;
  if (!path) {
    root.append(el("div", { class: "error" }, "未检测到 direct_ip 路径。先在"总览"重新检测。"));
    return;
  }

  const listEl = el("ul", { class: "entries" });
  const count = el("span", { tip: "当前直连规则条数" }, "0");
  const entryInput = el("input", { name: "entry", required: true, placeholder: "例如 1.2.3.4 或 1.2.3.0/24 或 example.com",
    tip: "支持 IPv4 / CIDR / 域名；加进来的目标将直连出局不走代理", style: { width: "100%" } });

  async function refresh() {
    try {
      const data = await api.listDirect(path);
      count.textContent = String(data.entries.length);
      listEl.replaceChildren(...data.entries.map(entry => el("li", {}, [
        el("span", { class: "mono", tip: "直连条目" }, entry),
        el("button", { class: "danger ghost", tip: "从直连列表删除该条", onclick: async () => {
          if (!confirm(`删除 ${entry}?`)) return;
          try { await api.removeDirect(path, entry); refresh(); }
          catch (e) { toast(formatError(e), "warn"); }
        } }, "删除"),
      ])));
    } catch (e) { toast(formatError(e), "warn"); }
  }

  root.append(
    el("section", { class: "card" }, [
      el("h3", {}, "直连列表"),
      el("p", { class: "hint" }, [
        "当前文件：", el("code", { tip: "PassWall 直连文件路径，加入这里的目标将直连不走代理" }, path),
      ]),
      el("form", { onsubmit: async (e) => {
        e.preventDefault();
        const v = entryInput.value.trim();
        if (!v) return;
        try { const added = await api.addDirect(path, v);
          toast(added ? `已加入：${v}` : `${v} 已存在`, added ? "ok" : "warn");
          entryInput.value = ""; refresh();
        } catch (err) { toast(formatError(err), "warn"); }
      } }, [
        el("div", { class: "row" }, [
          entryInput,
          el("button", { type: "submit", class: "primary", tip: "把上面输入追加到直连文件" }, "加入直连"),
          el("button", { type: "button", class: "ghost", tip: "重新读取直连文件", onclick: refresh }, "刷新"),
        ]),
      ]),
      el("h4", {}, ["当前条数：", count]),
      listEl,
    ]),
  );

  refresh();
}
