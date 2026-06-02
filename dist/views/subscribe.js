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

    // 注意：LuCI 表单字段是 `remark`（用户编辑），`remarks` 是 subscribe.lua
    // 拉取后回填的订阅源标题，仅显示用。详见 直连kwrt进行资料查询和处理.md §5.3
    const displayName = s.remark || s.remarks || name;
    // 统计该订阅当前贡献的节点数量：节点 section 上的 `add_from` 字段会被 subscribe.lua
    // 写为订阅 remark；老版本 PassWall 也可能写成 add_mode='2' 表示来自订阅。
    const allNodes = (ctx.overview && ctx.overview.nodes) || [];
    const nodeCount = allNodes.filter(n => {
      const af = n.add_from || n.add_mode || "";
      return af === (s.remark || "") || af === name;
    }).length;
    const remark = el("input", { name: "remark", value: s.remark || "", tip: "订阅备注名（必填、不能叫 default、不能与其它订阅重名）" });
    const url = el("input", { name: "url", value: s.url || "", tip: "订阅链接（http/https）", style: { width: "100%" } });
    const keyword = el("input", { name: "filter_keyword", value: arrJoin(s.filter_keyword), tip: "节点名包含这些关键字才保留（逗号分隔）；留空不过滤" });
    const discard = el("input", { name: "discard_keyword", value: arrJoin(s.discard_keyword), tip: "节点名包含这些关键字会被丢弃" });

    form.append(
      el("div", { class: "row" }, [
        field(`备注 [${name}]`, "改名不影响内部 uci section", remark),
      ]),
      el("div", { class: "row" }, [field("订阅 URL", "从这里拉节点列表", url)]),
      el("div", { class: "row" }, [
        field("保留关键字", "节点名包含才保留，多个用逗号", keyword),
        field("丢弃关键字", "节点名包含则丢弃", discard),
      ]),
      el("div", { class: "row actions" }, [
        el("button", { class: "primary", tip: "保存这条订阅的配置（不会立刻拉取节点）", onclick: async (ev) => {
          const btn = ev.currentTarget;
          const r = remark.value.trim();
          const u = url.value.trim();
          // LuCI: 备注必填、不可与其他订阅重名、不可叫 default
          if (!r) { toast("备注不能为空", "warn"); return; }
          if (r === "default") { toast("备注不能叫 default", "warn"); return; }
          const dup = subs.find(x => x[".name"] !== name && (x.remark || "").trim() === r);
          if (dup) { toast(`备注 "${r}" 已被订阅 [${dup[".name"]}] 占用`, "warn"); return; }
          if (!u) { toast("订阅 URL 不能为空", "warn"); return; }
          if (!/^https?:\/\//i.test(u)) { toast("订阅 URL 必须以 http:// 或 https:// 开头", "warn"); return; }
          const oldText = btn.textContent; btn.disabled = true; btn.textContent = "保存中…";
          try {
            await api.patchSection(ctx.config, name, {
              remark: r,
              url: u,
              filter_keyword: keyword.value.trim(),
              discard_keyword: discard.value.trim(),
            });
            toast(`✓ 已保存订阅：${r}（顶栏点"保存并应用"让 PassWall 真正生效）`);
            await ctx.refreshAndRedraw();
          } catch (e) {
            toast("保存订阅失败", "warn", { detail: formatError(e) });
          } finally { btn.disabled = false; btn.textContent = oldText; }
        } }, "保存"),
        el("button", { class: "ghost", tip: "立即触发订阅更新（调用 subscribe.lua start <section>）", onclick: async (ev) => {
          const btn = ev.currentTarget; const oldText = btn.textContent; btn.disabled = true; btn.textContent = "更新中…";
          try { const out = await api.updateSubscribe(ctx.config, name); toast(`✓ 已触发订阅更新：${name}`, "ok", { detail: out }); }
          catch (e) { toast("更新订阅失败", "warn", { detail: formatError(e) }); }
          finally { btn.disabled = false; btn.textContent = oldText; }
        } }, "立即更新"),
        el("button", { class: "danger ghost", tip: "删除这条订阅", onclick: async (ev) => {
          if (!confirm(`删除订阅 ${displayName}?`)) return;
          const btn = ev.currentTarget; const oldText = btn.textContent; btn.disabled = true; btn.textContent = "删除中…";
          try { await api.deleteSection(ctx.config, name); await ctx.refreshAndRedraw(); toast(`✓ 已删除订阅 ${displayName}`); }
          catch (e) { toast("删除订阅失败", "warn", { detail: formatError(e) }); }
          finally { btn.disabled = false; btn.textContent = oldText; }
        } }, "删除"),
      ]),
    );

    root.append(el("section", { class: "card" }, [
      el("h4", { style: { margin: "0 0 8px", display: "flex", alignItems: "center", gap: "8px" } }, [
        `▸ ${displayName}`,
        el("span", { class: "pill pill-muted small" }, `节点 ${nodeCount}`),
      ]),
      form,
    ]));
  }
}

function arrJoin(v) {
  if (Array.isArray(v)) return v.join(",");
  return v ?? "";
}
