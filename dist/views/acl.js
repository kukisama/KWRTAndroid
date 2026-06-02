// 客户端例外（PassWall ACL，按源 IP/MAC）
//
// 真实事实（实地探测 + 阅读 /usr/lib/lua/luci/model/cbi/passwall/client/acl_config.lua）：
//   - 段类型 acl_rule（匿名），全局总开关 passwall.@global[0].acl_enable
//   - 字段：enabled / remarks / interface / sources / tcp_node / udp_node
//          / tcp_no_redir_ports / udp_no_redir_ports
//   - sources 空格分隔，支持 IP / CIDR / IP-Range / MAC / ipset:NAME
//   - tcp_node="nil" 或 udp_node="nil" → 该来源对应方向不走代理（直连）
//   - udp_node="tcp" → 跟随 TCP 节点
//
// 与"规则列表"的关系（重要）：
//   规则列表是按 **目标域名/IP** 维度的（直连/代理/屏蔽/GFW/中国列表都按目标匹）。
//   若 ACL 里 tcp_node=nil + udp_node=nil（NAS 用法）→ 整机流量根本不进 PassWall 转发链
//     → 任何列表都不会被检查 → 一定不会"覆盖"这条 ACL。完全安全。
//   若 ACL 里指定了真实节点 → 流量进 PassWall → 4 个列表按目标维度叠加生效（设计如此）。
//
// 写入策略（本轮改造）：
//   所有 add/update/delete/setGlobalEnable 后端**只 commit 不 reload**，单次 <1s；
//   "应用配置"按钮负责后台 reload PassWall（10-30s 路由侧异步执行，前端立即返回）。
//   这样可批量改 N 条，只用 reload 一次。
import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

export default async function mount(root, ctx) {
  let snap = { acl_enable: false, rules: [] };
  // 自上次"应用配置"后是否有待生效改动
  let pendingApply = false;

  const nodeOpts = () => {
    const ov = ctx.overview || {};
    const arr = [{ value: "nil", label: "（不使用 / 直连）" }];
    for (const n of (ov.nodes || [])) {
      arr.push({
        value: n[".name"],
        label: `${n.remarks || "(未命名)"}  [${n.protocol || n.type || "?"}]  ${n.address || ""}:${n.port || ""}`,
      });
    }
    return arr;
  };
  const udpOpts = () => [
    { value: "nil", label: "（不使用 / 直连）" },
    { value: "tcp", label: "（跟随 TCP）" },
    ...nodeOpts().filter(o => o.value !== "nil"),
  ];
  const portOpts = () => [
    { value: "", label: "（使用全局配置）" },
    { value: "disable", label: "禁用 / 不使用" },
    { value: "1:65535", label: "所有端口 1:65535" },
    { value: "80,443", label: "80,443" },
    { value: "53", label: "53" },
  ];

  const loading = el("div", { class: "hint" }, "正在读取 ACL…");
  root.append(loading);
  try { snap = await api.aclRead(ctx.config); }
  catch (e) { loading.replaceWith(el("div", { class: "error" }, ["读取 ACL 失败：", el("pre", { class: "mono" }, formatError(e))])); return; }
  loading.remove();

  // ===== 总开关 =====
  const enableCb = el("input", { type: "checkbox" });
  enableCb.checked = !!snap.acl_enable;
  enableCb.addEventListener("change", async () => {
    const on = enableCb.checked;
    enableCb.disabled = true;
    try {
      await api.aclSetGlobalEnable(ctx.config, on);
      markPending();
      toast(on ? "✓ 已暂存：开启 ACL 总开关" : "✓ 已暂存：关闭 ACL 总开关");
    } catch (err) {
      enableCb.checked = !on;
      toast("切换 ACL 总开关失败", "warn", { detail: formatError(err) });
    } finally { enableCb.disabled = false; }
  });

  // 顶部"应用配置"按钮（pendingApply 时变高亮 + 角标）
  const applyBtn = el("button", { class: "primary", tip: "把暂存改动应用到 PassWall（后台 reload，约 10–30 秒生效）" }, "✓ 应用配置");
  const pendingBadge = el("span", { class: "pill pill-warn", hidden: true, style: { marginLeft: "6px" } }, "有未应用改动");
  function markPending() {
    pendingApply = true;
    pendingBadge.hidden = false;
    applyBtn.classList.add("primary");
  }
  function clearPending() {
    pendingApply = false;
    pendingBadge.hidden = true;
  }
  applyBtn.addEventListener("click", async () => {
    applyBtn.disabled = true; const old = applyBtn.textContent; applyBtn.textContent = "应用中…";
    try {
      await api.aclReload(ctx.config);
      clearPending();
      toast("✓ 已触发后台 reload PassWall（约 10–30 秒生效）");
    } catch (err) {
      toast("应用配置失败", "warn", { detail: formatError(err) });
    } finally { applyBtn.disabled = false; applyBtn.textContent = old; }
  });

  // ===== 规则表格 =====
  const tbody = el("tbody", {});
  function rerender() {
    tbody.replaceChildren(...snap.rules.map(rowEl));
    if (!snap.rules.length) {
      tbody.append(el("tr", {}, el("td", { colspan: "6", class: "hint", style: { textAlign: "center", padding: "16px" } },
        "暂无规则。点「+ 新增规则」或「快速添加：NAS 直连」预填一条。")));
    }
  }
  function nodeLabel(id, opts) {
    const f = opts.find(o => o.value === id);
    return f ? f.label : (id || "-");
  }

  // 行内 enabled 列：点一下即写 UCI（不 reload），等用户点"应用配置"统一生效
  function makeRowToggle(r) {
    const cb = el("input", { type: "checkbox" });
    cb.checked = !!r.enabled;
    cb.addEventListener("change", async () => {
      const on = cb.checked;
      cb.disabled = true;
      try {
        await api.aclUpdate(ctx.config, { ...r, enabled: on });
        r.enabled = on;
        // 更新行 class
        const tr = cb.closest("tr");
        if (tr) tr.classList.toggle("row-disabled", !on);
        markPending();
        toast(on ? `✓ 已暂存：启用 ${r.sources}` : `✓ 已暂存：停用 ${r.sources}`);
      } catch (err) {
        cb.checked = !on;
        toast("切换启用失败", "warn", { detail: formatError(err) });
      } finally { cb.disabled = false; }
    });
    return cb;
  }

  function rowEl(r) {
    const optsT = nodeOpts(), optsU = udpOpts();
    return el("tr", { class: r.enabled ? "" : "row-disabled" }, [
      el("td", { style: { textAlign: "center" } }, makeRowToggle(r)),
      el("td", { class: "mono" }, r.sources || "-"),
      el("td", {}, [
        r.remarks || el("span", { class: "muted" }, "(无备注)"),
        r.interface ? el("div", { class: "small muted" }, "接口: " + r.interface) : null,
      ]),
      el("td", { class: "small" }, nodeLabel(r.tcp_node, optsT)),
      el("td", { class: "small" }, nodeLabel(r.udp_node, optsU)),
      el("td", { style: { whiteSpace: "nowrap" } }, [
        el("button", { class: "ghost", tip: "编辑这条 ACL", onclick: () => openEditor(r) }, "编辑"),
        " ",
        el("button", { class: "danger ghost", tip: "删除这条 ACL", onclick: async () => {
          if (!confirm(`删除 ACL：${r.sources}  备注 "${r.remarks}"？`)) return;
          try {
            await api.aclDelete(ctx.config, r.section);
            snap = await api.aclRead(ctx.config);
            rerender();
            markPending();
            toast("✓ 已暂存：删除该条");
          } catch (e) { toast("删除失败", "warn", { detail: formatError(e) }); }
        } }, "✕"),
      ]),
    ]);
  }

  function openEditor(existing) {
    const isNew = !existing;
    const draft = existing ? { ...existing } : {
      section: "", enabled: true, remarks: "", interface: "",
      sources: "", tcp_node: "nil", udp_node: "nil",
      tcp_no_redir_ports: "", udp_no_redir_ports: "",
      // 默认 5 个全局列表全关（本 App 面向 NAS-bypass 场景）
      use_direct_list: false, use_proxy_list: false, use_block_list: false,
      use_gfw_list: false, chn_list: "0",
    };

    const enCb = el("input", { type: "checkbox" }); enCb.checked = !!draft.enabled;
    const rmIn = el("input", { type: "text", value: draft.remarks, placeholder: "例：NAS 整机直连", style: { width: "100%" } });
    const ifIn = el("input", { type: "text", value: draft.interface, placeholder: "留空 = 全部；常见值 br-lan", style: { width: "100%" } });
    const srcIn = el("input", { type: "text", value: draft.sources, placeholder: "10.0.0.3   或   aa:bb:cc:dd:ee:ff 10.0.0.0/24", style: { width: "100%" } });
    const tcpSel = mkSelect(draft.tcp_node, nodeOpts());
    const udpSel = mkSelect(draft.udp_node, udpOpts());
    const tcpPortSel = mkPortInput(draft.tcp_no_redir_ports);
    const udpPortSel = mkPortInput(draft.udp_no_redir_ports);

    // “全局列表” 5 个控件
    const useDirectCb = el("input", { type: "checkbox" }); useDirectCb.checked = !!draft.use_direct_list;
    const useProxyCb  = el("input", { type: "checkbox" }); useProxyCb.checked  = !!draft.use_proxy_list;
    const useBlockCb  = el("input", { type: "checkbox" }); useBlockCb.checked  = !!draft.use_block_list;
    const useGfwCb    = el("input", { type: "checkbox" }); useGfwCb.checked    = !!draft.use_gfw_list;
    const chnSel = mkSelect(draft.chn_list || "0", [
      { value: "0", label: "关闭（不使用中国列表）" },
      { value: "direct", label: "直连（中国列表内的目标走直连）" },
      { value: "proxy",  label: "代理（中国列表内的目标走代理）" },
    ]);
    const listsCard = el("details", { style: { marginTop: "10px" }, open: isNew ? false : false }, [
      el("summary", { style: { cursor: "pointer", color: "var(--muted)" } },
        "使用 PassWall 全局规则列表（默认全关）"),
      el("div", { class: "hint small", style: { margin: "6px 0 8px" } },
        "这些列表是与本条 ACL 绑定的独立开关（不是 LuCI “规则列表”页面的全局开关）。 "
        + "勾选 = 本条 ACL 的流量会按对应列表重新类；不勾选 = 完全交给节点处理，不走列表。"),
      el("div", { class: "kv", style: { gridTemplateColumns: "160px 1fr" } }, [
        el("div", { class: "k" }, "直连列表"), el("div", { class: "v" }, [useDirectCb,
          el("span", { class: "hint small", style: { marginLeft: "6px" } }, "命中则本条流量直连出局")]),
        el("div", { class: "k" }, "代理列表"), el("div", { class: "v" }, [useProxyCb,
          el("span", { class: "hint small", style: { marginLeft: "6px" } }, "命中则强制走节点")]),
        el("div", { class: "k" }, "屏蔽列表"), el("div", { class: "v" }, [useBlockCb,
          el("span", { class: "hint small", style: { marginLeft: "6px" } }, "命中则丢弃")]),
        el("div", { class: "k" }, "GFW 列表"), el("div", { class: "v" }, [useGfwCb,
          el("span", { class: "hint small", style: { marginLeft: "6px" } }, "命中则强制走代理")]),
        el("div", { class: "k" }, "中国列表"), el("div", { class: "v" }, [chnSel,
          el("div", { class: "hint small" }, "chnlist / chnroute；三态，默认关")]),
      ]),
    ]);

    const advCard = el("details", { style: { marginTop: "10px" } }, [
      el("summary", { style: { cursor: "pointer", color: "var(--muted)" } }, "高级选项（端口）"),
      el("div", { class: "kv", style: { gridTemplateColumns: "120px 1fr", marginTop: "8px" } }, [
        el("div", { class: "k" }, "TCP 不转发端口"), el("div", { class: "v" }, [tcpPortSel,
          el("div", { class: "hint small" }, "命中的设备这些 TCP 端口直接放行，不进 PassWall")]),
        el("div", { class: "k" }, "UDP 不转发端口"), el("div", { class: "v" }, [udpPortSel,
          el("div", { class: "hint small" }, "同上，作用于 UDP")]),
      ]),
    ]);

    const modal = el("div", { class: "modal-bg" }, [
      el("div", { class: "modal-card", style: { width: "620px", maxWidth: "92vw", maxHeight: "85vh", overflow: "auto" } }, [
        el("h3", {}, isNew ? "新增 ACL 规则" : `编辑 ACL 规则 · ${existing.section}`),
        el("div", { class: "kv", style: { gridTemplateColumns: "120px 1fr" } }, [
          el("div", { class: "k" }, "启用"),    el("div", { class: "v" }, enCb),
          el("div", { class: "k" }, "源接口"),  el("div", { class: "v" }, [ifIn,
            el("div", { class: "hint small" }, "kernel 设备名（不是 UCI 接口名）；如 br-lan / eth0；留空 = 所有接口")]),
          el("div", { class: "k" }, "源地址"),  el("div", { class: "v" }, [srcIn,
            el("div", { class: "hint small" }, "空格分隔；支持 IP / CIDR / IP 范围 / MAC / ipset:NAME。例：10.0.0.3   192.168.1.0/24   aa:bb:cc:dd:ee:ff")]),
          el("div", { class: "k" }, "备注"),    el("div", { class: "v" }, rmIn),
          el("div", { class: "k" }, "TCP 节点"), el("div", { class: "v" }, [tcpSel,
            el("div", { class: "hint small" }, "「不使用 / 直连」= 此来源 TCP 完全不走 PassWall（任何「直连/代理/屏蔽列表」都不会作用）")]),
          el("div", { class: "k" }, "UDP 节点"), el("div", { class: "v" }, [udpSel,
            el("div", { class: "hint small" }, "「跟随 TCP」= 与上面同一节点；「不使用 / 直连」= UDP 不走 PassWall")]),
        ]),
        advCard,
        listsCard,
        el("div", { class: "row actions", style: { marginTop: "14px", justifyContent: "flex-end" } }, [
          el("button", { class: "ghost", onclick: () => modal.remove() }, "取消"),
          el("button", { class: "primary", onclick: async (e) => {
            const btn = e.currentTarget; btn.disabled = true; const old = btn.textContent; btn.textContent = "保存中…";
            const rule = {
              section: draft.section,
              enabled: enCb.checked,
              remarks: rmIn.value.trim(),
              interface: ifIn.value.trim(),
              sources: srcIn.value.trim(),
              tcp_node: tcpSel.value,
              udp_node: udpSel.value,
              tcp_no_redir_ports: tcpPortSel.value.trim(),
              udp_no_redir_ports: udpPortSel.value.trim(),
              use_direct_list: useDirectCb.checked,
              use_proxy_list: useProxyCb.checked,
              use_block_list: useBlockCb.checked,
              use_gfw_list: useGfwCb.checked,
              chn_list: chnSel.value,
            };
            if (!rule.sources) { toast("源地址不能为空", "warn"); btn.disabled = false; btn.textContent = old; return; }
            try {
              if (isNew) await api.aclAdd(ctx.config, rule);
              else await api.aclUpdate(ctx.config, rule);
              snap = await api.aclRead(ctx.config);
              rerender();
              modal.remove();
              markPending();
              toast("✓ 已暂存到 UCI；记得点顶部「应用配置」让 PassWall 重载生效");
            } catch (err) {
              toast("保存失败", "warn", { detail: formatError(err) });
            } finally { btn.disabled = false; btn.textContent = old; }
          } }, "暂存（不立即应用）"),
        ]),
      ]),
    ]);
    modal.addEventListener("click", e => { if (e.target === modal) modal.remove(); });
    document.body.append(modal);
  }

  function mkSelect(current, opts) {
    const s = el("select", { style: { width: "100%" } });
    for (const o of opts) {
      const opt = el("option", { value: o.value }, o.label);
      if (o.value === current) opt.selected = true;
      s.append(opt);
    }
    return s;
  }
  function mkPortInput(current) {
    const wrap = el("div", { class: "row", style: { gap: "6px", alignItems: "center" } });
    const inList = portOpts().some(o => o.value === current);
    const sel = mkSelect(inList ? current : "__custom", [
      ...portOpts(),
      { value: "__custom", label: "（自定义…）" },
    ]);
    const txt = el("input", { type: "text", value: current, placeholder: "如 80,443 或 1:1024",
      style: { width: "180px" }, hidden: inList });
    sel.addEventListener("change", () => {
      if (sel.value === "__custom") { txt.hidden = false; txt.focus(); }
      else { txt.hidden = true; txt.value = sel.value; }
    });
    wrap.append(sel, txt);
    Object.defineProperty(wrap, "value", {
      get() { return sel.value === "__custom" ? txt.value : sel.value; },
    });
    return wrap;
  }

  async function quickAddNas() {
    const ip = prompt("NAS 直连：输入 NAS 的 IP（TCP/UDP 都直连，不走代理）", "10.0.0.3");
    if (!ip) return;
    try {
      await api.aclAdd(ctx.config, {
        section: "", enabled: true, remarks: `NAS 直连 ${ip}`, interface: "",
        sources: ip.trim(), tcp_node: "nil", udp_node: "nil",
        tcp_no_redir_ports: "", udp_no_redir_ports: "",
        use_direct_list: false, use_proxy_list: false, use_block_list: false,
        use_gfw_list: false, chn_list: "0",
      });
      snap = await api.aclRead(ctx.config);
      rerender();
      markPending();
      toast(`✓ 已暂存：${ip} → 直连；记得点「应用配置」`);
    } catch (e) { toast("添加失败", "warn", { detail: formatError(e) }); }
  }

  root.append(
    el("section", { class: "card" }, [
      el("h3", {}, "客户端例外 / 访问控制（ACL）"),
      el("p", { class: "hint" }, [
        "按 ", el("b", {}, "源 IP / MAC"), " 决定哪些设备走代理；TCP/UDP 节点都选「不使用 / 直连」= 这些设备整机绕过 PassWall。",
        el("br", {}),
        el("b", {}, "工作流"), "：所有改动只 commit、不立即重启；改完一批后点顶部 ", el("b", {}, "「应用配置」"), " 才让 PassWall 重载生效（一次重载约 10–30 秒）。",
      ]),
      el("details", {}, [
        el("summary", { style: { cursor: "pointer", color: "var(--muted)", marginBottom: "6px" } }, "ACL 和 PassWall 「规则列表 / 直连列表」的关系？"),
        el("div", { class: "hint", style: { padding: "8px 10px", border: "1px solid var(--line)", borderRadius: "6px", background: "var(--card-2)" } }, [
          el("p", {}, [el("b", {}, "ACL（本页）"), "：按 ", el("b", {}, "源 IP/MAC"), " 决定 「这台设备走哪个节点」（tcp_node/udp_node）。"]),
          el("p", {}, [el("b", {}, "规则列表"), "（直连/代理/屏蔽/局域网IP/路由Hosts/GFW/中国域名/中国IP，共 8 类，位于 ", el("code", {}, "/usr/share/passwall/rules/"), " 的纯文本文件如 ", el("code", {}, "direct_host / direct_ip / proxy_host / block_host / lanlist / gfwlist / chnlist / chnroute"), "）：按 ", el("b", {}, "目标域名/IP"), " 决定 「这个目标走代理 / 直连 / 屏蔽」，是 ", el("b", {}, "全局生效"), " 的文件，不通过 UCI section 关联到具体 ACL 条目。"]),
          el("p", {}, [el("b", {}, "会不会"), "覆盖？分两种情况："]),
          el("ul", { style: { margin: "4px 0", paddingLeft: "20px" } }, [
            el("li", {}, [el("b", {}, "ACL 选「不使用 / 直连」"), "（即 NAS 用法 tcp/udp 都 nil）→ 该源流量根本不进 PassWall 转发链 → 任何列表都 ", el("b", { style: { color: "var(--ok)" } }, "一定不会作用"), " ✓ 完全安全。"]),
            el("li", {}, [el("b", {}, "ACL 指定了真实节点"), " → 流量进 PassWall → 4 个列表按目标维度叠加生效：例如「直连列表」里的域名仍直连，「屏蔽列表」里的域名仍被屏蔽。", el("b", {}, "不是覆盖单条 ACL"), "，而是节点内部的目标分流。"]),
          ]),
          el("p", { style: { marginBottom: 0 } }, "结论：你给 NAS 加 tcp_node=nil + udp_node=nil 的 ACL，4 个列表勾不勾都不会影响它，可放心。"),
        ]),
      ]),
      el("div", { class: "row", style: { gap: "12px", alignItems: "center", marginTop: "12px", flexWrap: "wrap" } }, [
        el("label", {}, [enableCb, " 启用 ACL 总开关（关闭后下面所有规则都不生效）"]),
        el("span", { class: "grow" }),
        el("button", { class: "ghost", tip: "快速预填一条 NAS 整机直连", onclick: quickAddNas }, "快速添加：NAS 直连"),
        el("button", { class: "ghost", tip: "新增一条按源 IP/MAC 的规则", onclick: () => openEditor(null) }, "+ 新增规则"),
        el("button", { class: "ghost", tip: "重新从路由器拉取（放弃未保存的本地修改）", onclick: async () => {
          try { snap = await api.aclRead(ctx.config); enableCb.checked = !!snap.acl_enable; rerender(); }
          catch (e) { toast("刷新失败", "warn", { detail: formatError(e) }); }
        } }, "↻ 刷新"),
        applyBtn, pendingBadge,
      ]),
      el("p", { class: "hint small", style: { marginTop: "6px" } }, [
        "💡 ", el("b", {}, "工作流"), "：本页所有改动（启用/停用、新增、编辑、删除、总开关）只暂存到 UCI，",
        "改完点 ", el("b", {}, "「✓ 应用配置」"), " 一次性 reload PassWall。这样改 10 条只用等 1 次重载。",
      ]),
      el("table", { class: "sys-table", style: { marginTop: "12px" } }, [
        el("thead", {}, el("tr", {}, [
          el("th", { style: { width: "60px" } }, "启用"),
          el("th", {}, "源地址（IP / MAC）"),
          el("th", {}, "备注 / 接口"),
          el("th", {}, "TCP 节点"),
          el("th", {}, "UDP 节点"),
          el("th", {}, ""),
        ])),
        tbody,
      ]),
    ]),
  );
  rerender();
}
