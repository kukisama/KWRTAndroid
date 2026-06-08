import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

// 只读：展示 PassWall 用到的核心二进制版本/路径/大小。
// 故意不做更新操作——更新涉及覆写 /usr/bin/xxx，风险高。
export default async function mount(root, ctx) {
  const status = el("p", { class: "hint" }, "正在读取组件信息…");
  const tbody = el("tbody");
  const pwVer = el("span", { class: "mono" }, "—");

  const card = el("section", { class: "card" }, [
    el("h3", {}, "组件信息（只读）"),
    el("p", { class: "hint" },
      "PassWall 依赖的核心二进制版本与路径。仅展示，不在此处做更新——如需升级请走 opkg / 自带升级脚本，避免覆盖运行中进程。"),
    el("div", { class: "row" }, [
      el("span", {}, "PassWall 插件版本："), pwVer,
      el("button", {
        class: "ghost", style: { marginLeft: "12px" },
        tip: "重新读取一次",
        onclick: () => load(),
      }, "刷新"),
    ]),
    status,
    el("table", { class: "grid" }, [
      el("thead", {}, el("tr", {}, [
        el("th", {}, "组件"),
        el("th", {}, "路径"),
        el("th", {}, "大小"),
        el("th", {}, "版本"),
      ])),
      tbody,
    ]),
  ]);
  root.append(card);

  async function load() {
    status.textContent = "正在读取…";
    tbody.replaceChildren();
    try {
      const info = await api.components(ctx.config);
      pwVer.textContent = info.passwall_version || "（未识别）";
      const comps = info.components || {};
      const keys = Object.keys(comps).sort();
      for (const k of keys) {
        const c = comps[k] || {};
        const sz = c.size_bytes ? formatSize(c.size_bytes) : "—";
        const ver = c.version || "—";
        const path = c.path || "—";
        const exists = c.size_bytes != null;
        tbody.append(el("tr", {}, [
          el("td", {}, k),
          el("td", { class: "mono small" }, path),
          el("td", { class: "mono small" }, sz),
          el("td", { class: exists ? "" : "muted small" }, ver),
        ]));
      }
      status.textContent = "";
    } catch (e) {
      status.textContent = "";
      toast("读取组件信息失败", "warn", { detail: formatError(e) });
    }
  }
  load();
}

function formatSize(n) {
  if (n < 1024) return n + " B";
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + " KB";
  return (n / 1024 / 1024).toFixed(2) + " MB";
}
