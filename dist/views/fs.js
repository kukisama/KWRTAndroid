// 远端文件浏览（只读）
//
// 目的：让用户能下探看路由器分区里到底有什么东西。
// 限制：只读；预览硬上限 64 KB（后端强制）；不渲染图片/二进制，仅显示头部 hex。
import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

const STARTERS = ["/", "/etc", "/etc/config", "/overlay", "/tmp", "/usr/share/passwall", "/usr/share/passwall/rules"];

export default async function mount(root, ctx) {
  let cwd = "/";
  // 排序状态：key ∈ name|kind|size|mode；dir ∈ 1(asc)|-1(desc)
  let sortKey = "name", sortDir = 1;
  let lastEntries = [];

  const pathIn = el("input", { type: "text", value: cwd, style: { width: "100%" }, placeholder: "/etc/config" });
  pathIn.addEventListener("keydown", e => { if (e.key === "Enter") go(pathIn.value.trim() || "/"); });
  const goBtn = el("button", { class: "ghost", onclick: () => go(pathIn.value.trim() || "/") }, "前往");
  const upBtn = el("button", { class: "ghost", tip: "上一级", onclick: () => {
    if (cwd === "/") return;
    const p = cwd.replace(/\/+$/, "");
    const parent = p.slice(0, p.lastIndexOf("/")) || "/";
    go(parent);
  } }, "↑ 上一级");

  const starters = el("div", { class: "row", style: { gap: "6px", flexWrap: "wrap" } },
    STARTERS.map(p => el("button", { class: "ghost small", onclick: () => go(p) }, p)));

  const list = el("table", { class: "sys-table fs-table" }, [
    el("thead", {}, el("tr", {}, [
      sortableTh("名称", "name"),
      sortableTh("类型", "kind"),
      sortableTh("大小", "size"),
      sortableTh("权限", "mode"),
      el("th", {}, ""),
    ])),
    el("tbody", {}),
  ]);

  function sortableTh(label, key) {
    const arrow = sortKey === key ? (sortDir > 0 ? " ▲" : " ▼") : "";
    return el("th", {
      style: { cursor: "pointer", userSelect: "none" },
      onclick: () => {
        if (sortKey === key) sortDir = -sortDir; else { sortKey = key; sortDir = 1; }
        renderTable();
        // 重新构造 thead 以更新箭头
        list.querySelector("thead").replaceWith(el("thead", {}, el("tr", {}, [
          sortableTh("名称", "name"), sortableTh("类型", "kind"),
          sortableTh("大小", "size"), sortableTh("权限", "mode"), el("th", {}, ""),
        ])));
      },
    }, label + arrow);
  }
  function renderTable() {
    const tbody = list.querySelector("tbody");
    if (!lastEntries.length) {
      tbody.replaceChildren(el("tr", {}, el("td", { colspan: "5", class: "hint" }, "（空目录）")));
      return;
    }
    // 目录始终优先
    const sorted = [...lastEntries].sort((a, b) => {
      const oa = a.kind === "dir" ? 0 : 1, ob = b.kind === "dir" ? 0 : 1;
      if (oa !== ob) return oa - ob;
      let va = a[sortKey], vb = b[sortKey];
      if (sortKey === "size") { va = +va || 0; vb = +vb || 0; return (va - vb) * sortDir; }
      return String(va).localeCompare(String(vb)) * sortDir;
    });
    tbody.replaceChildren(...sorted.map(rowEl));
  }

  const preview = el("section", { class: "card", hidden: true }, []);

  async function go(p) {
    if (!p.startsWith("/")) p = "/" + p;
    preview.hidden = true;
    preview.replaceChildren();
    const tbody = list.querySelector("tbody");
    tbody.replaceChildren(el("tr", {}, el("td", { colspan: "5", class: "hint" }, "正在读取…")));
    try {
      const data = await api.fsList(p);
      cwd = data.path;
      pathIn.value = cwd;
      lastEntries = data.entries || [];
      renderTable();
    } catch (e) {
      tbody.replaceChildren(el("tr", {}, el("td", { colspan: "5", class: "error" }, formatError(e))));
    }
  }
  function rowEl(it) {
    const isDir = it.kind === "dir";
    const isLink = it.kind === "link";
    const nameCell = isDir
      ? el("a", { href: "#", onclick: e => { e.preventDefault(); go(join(cwd, it.name)); } }, it.name + "/")
      : isLink
        ? el("span", {}, [it.name, el("span", { class: "muted small" }, "  → " + it.link_target)])
        : el("a", { href: "#", onclick: e => { e.preventDefault(); doPreview(join(cwd, it.name)); } }, it.name);
    return el("tr", {}, [
      el("td", { class: "mono" }, nameCell),
      el("td", {}, it.kind),
      el("td", { class: "mono" }, fmtSize(it.size)),
      el("td", { class: "mono small" }, it.mode),
      el("td", {}, isDir
        ? el("button", { class: "ghost small", onclick: () => go(join(cwd, it.name)) }, "进入")
        : isLink ? "" : el("button", { class: "ghost small", onclick: () => doPreview(join(cwd, it.name)) }, "预览")),
    ]);
  }

  async function doPreview(path) {
    preview.hidden = false;
    preview.replaceChildren(el("div", { class: "hint" }, "正在加载预览…"));
    try {
      const data = await api.fsPreview(path);
      preview.replaceChildren(
        el("h3", {}, ["预览：", el("code", {}, path)]),
        el("div", { class: "hint small" }, [
          `大小 ${fmtSize(data.size)}`,
          data.truncated ? "  ·  已截断到前 64 KB" : "",
          data.is_binary ? "  ·  二进制（仅显示前 256B 十六进制）" : "",
          "  ·  ",
          el("a", { href: "#", onclick: e => { e.preventDefault(); preview.hidden = true; } }, "关闭"),
        ]),
        el("pre", { class: "mono small", style: { maxHeight: "420px", overflow: "auto", whiteSpace: "pre", border: "1px solid var(--line)", padding: "8px", borderRadius: "6px" } }, data.text || "（空）"),
      );
    } catch (e) {
      preview.replaceChildren(el("div", { class: "error" }, formatError(e)));
    }
  }

  function join(base, name) {
    if (base.endsWith("/")) return base + name;
    return base + "/" + name;
  }
  function fmtSize(n) {
    if (!n && n !== 0) return "-";
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
    return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  root.append(
    el("section", { class: "card" }, [
      el("h3", {}, "文件浏览（只读）"),
      el("p", { class: "hint" }, "只读浏览路由器文件系统；点目录进入、点文件预览（小文本最多 64 KB）。请勿在不熟悉的目录乱删乱改——本面板不支持写入，但避免给自己留误操作隐患。"),
      el("div", { class: "row", style: { gap: "6px" } }, [upBtn, pathIn, goBtn]),
      el("div", { style: { marginTop: "6px" } }, [el("span", { class: "hint small" }, "快捷入口："), starters]),
      list,
    ]),
    preview,
  );

  await go("/");
}
