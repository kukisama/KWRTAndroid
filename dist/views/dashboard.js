// 首页：KWRT 硬件 / 系统总览。
import { el, toast } from "../dom.js";
import { api, formatError, clearDirty } from "../api.js";
import { field, select, checkbox } from "./_form.js";
import { isStillMounted } from "../tabs.js";

function fmtBytes(n) {
  if (n == null) return "-";
  const u = ["B", "KB", "MB", "GB", "TB"];
  let v = Number(n); let i = 0;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(v >= 100 ? 0 : v >= 10 ? 1 : 2)} ${u[i]}`;
}
function fmtKB(kb) { return fmtBytes((kb || 0) * 1024); }
function fmtUptime(s) {
  if (!s) return "-";
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const parts = [];
  if (d) parts.push(`${d} 天`);
  if (h || d) parts.push(`${h} 时`);
  parts.push(`${m} 分`);
  return parts.join(" ");
}
function bar(used, total, opts = {}) {
  const pct = total > 0 ? Math.min(100, Math.round((used / total) * 100)) : 0;
  const color = pct > 90 ? "var(--bad)" : pct > 75 ? "#d68910" : "var(--ok)";
  return el("div", { class: "usage-bar", tip: opts.tip || `${pct}%` }, [
    el("div", { class: "usage-bar-fill", style: { width: `${pct}%`, background: color } }),
    el("span", { class: "usage-bar-label" }, opts.label || `${pct}%`),
  ]);
}

// 模块级缓存：sysInfo 后端要在路由器上执行一段 shell + 0.3s CPU 双采样，
// 单次往返大约 0.6~1.2s。来回切 tab 没必要每次都拉，缓存 30s 内复用，
// 用户点 ↻ 才强制刷新。
let _sysCache = null;        // { info, ts }
let _sysInflight = null;     // 去重并发请求
const SYS_TTL_MS = 30_000;
async function getSysInfo({ force = false } = {}) {
  const fresh = _sysCache && (Date.now() - _sysCache.ts) < SYS_TTL_MS;
  if (!force && fresh) return { info: _sysCache.info, fromCache: true };
  if (!_sysInflight) {
    _sysInflight = (async () => {
      try {
        const info = await api.sysInfo();
        _sysCache = { info, ts: Date.now() };
        return info;
      } finally { _sysInflight = null; }
    })();
  }
  const info = await _sysInflight;
  return { info, fromCache: false };
}

export default async function mount(root, ctx, opts = {}) {
  const r = ctx.report;
  // 快照当前 mount 代号；任何 await 之后向 root 追加 DOM 之前都要再校验，
  // 否则用户切走后异步 resolve 会把本 tab 的卡片塞到其他 tab 里（串台 bug）
  const myGen = root.__mountGen;
  const stale = () => !isStillMounted(root, myGen);

  // 顶部"刷新硬件"按钮：只刷 sys / cpu / 存储 / 网口几张卡，PassWall 全局不动
  const refreshBtn = el("button", { class: "ghost", tip: "重新读取 CPU 占用 / 内存 / 运行时间 / 网口流量" }, "↻ 刷新硬件");
  refreshBtn.addEventListener("click", async () => {
    refreshBtn.disabled = true; const old = refreshBtn.textContent; refreshBtn.textContent = "刷新中…";
    try { root.replaceChildren(); await mount(root, ctx, { force: true }); }
    finally { refreshBtn.disabled = false; refreshBtn.textContent = old; }
  });
  root.append(el("div", { class: "row", style: { justifyContent: "flex-end", marginBottom: "8px" } }, [refreshBtn]));

  // 仅在确实要发请求时才显示"正在拉取…"，命中缓存就直接渲染
  const willFetch = opts.force || !(_sysCache && (Date.now() - _sysCache.ts) < SYS_TTL_MS);
  let loadingEl = null;
  if (willFetch) {
    loadingEl = el("div", { class: "hint" }, "正在拉取硬件信息…");
    root.append(loadingEl);
  }

  let info;
  try {
    ({ info } = await getSysInfo({ force: !!opts.force }));
  } catch (e) {
    if (stale()) return;
    if (loadingEl) loadingEl.remove();
    root.append(el("div", { class: "error" }, [
      "拉取硬件信息失败：", el("pre", { class: "mono" }, formatError(e)),
    ]));
    return;
  }
  if (stale()) return;  // 用户已经切走，丢弃本次结果，避免追加到别的 tab
  if (loadingEl) loadingEl.remove();

  const sys = el("section", { class: "card" }, [
    el("h3", {}, "系统"),
    el("div", { class: "kv" }, [
      el("div", { class: "k" }, "主机名"),        el("div", { class: "v mono" }, info.hostname || r?.hostname || "-"),
      el("div", { class: "k" }, "型号"),          el("div", { class: "v" }, info.model || "-"),
      el("div", { class: "k" }, "Board"),         el("div", { class: "v mono" }, info.board_name || "-"),
      el("div", { class: "k" }, "OpenWrt"),       el("div", { class: "v" }, info.openwrt_release || r?.openwrt_release || "-"),
      el("div", { class: "k" }, "Kernel"),        el("div", { class: "v mono" }, info.kernel || "-"),
      el("div", { class: "k" }, "运行时间"),      el("div", { class: "v" }, fmtUptime(info.uptime_seconds)),
      el("div", { class: "k" }, "负载 (1/5/15)"), el("div", { class: "v mono" }, (info.load || [0,0,0]).map(x => Number(x).toFixed(2)).join(" / ")),
    ]),
  ]);

  const memUsed = (info.mem_total_kb || 0) - (info.mem_available_kb || info.mem_free_kb || 0);
  const cpuMem = el("section", { class: "card" }, [
    el("h3", {}, "CPU / 内存"),
    el("div", { class: "kv" }, [
      el("div", { class: "k" }, "CPU"),        el("div", { class: "v mono" }, `${info.cpu_model || "-"}  × ${info.cpu_cores}`),
      el("div", { class: "k" }, "CPU 占用"),   el("div", { class: "v" }, [
        el("div", { class: "mono", style: { marginBottom: "4px" } }, `${info.cpu_busy_percent ?? 0} %  ·  负载 ${(info.load || [0,0,0]).map(x => Number(x).toFixed(2)).join(" / ")}`),
        bar(info.cpu_busy_percent ?? 0, 100, { tip: "整机 CPU 占用（300ms 双采样 /proc/stat 估算）" }),
      ]),
      el("div", { class: "k" }, "内存总量"),    el("div", { class: "v" }, fmtKB(info.mem_total_kb)),
      el("div", { class: "k" }, "已用 / 可用"), el("div", { class: "v" }, [
        el("div", { class: "mono", style: { marginBottom: "4px" } },
          `${fmtKB(memUsed)} / ${fmtKB(info.mem_total_kb)}  ·  可用 ${fmtKB(info.mem_available_kb || info.mem_free_kb)}`),
        bar(memUsed, info.mem_total_kb, { tip: `Buffers ${fmtKB(info.mem_buffers_kb)} · Cached ${fmtKB(info.mem_cached_kb)}` }),
      ]),
    ]),
  ]);

  const disk = el("section", { class: "card" }, [
    el("h3", {}, "存储 / 分区"),
    el("p", { class: "hint" }, [
      "OpenWrt 标准布局：", el("code", {}, "/rom"), " 是只读固件 squashfs，", el("b", {}, "100% 是正常的，不是问题"),
      "；", el("code", {}, "/overlay"), " 才是用户可写空间，真正剩余看这一行。",
    ]),
    (info.mounts || []).length === 0
      ? el("p", { class: "hint" }, "未读取到分区信息")
      : el("table", { class: "sys-table" }, [
        el("thead", {}, el("tr", {}, [
          el("th", {}, "挂载点"), el("th", {}, "文件系统"), el("th", {}, "总量"), el("th", {}, "已用"), el("th", {}, "可用"), el("th", {}, "用量"),
        ])),
        el("tbody", {}, info.mounts.map(m => el("tr", {}, [
          el("td", { class: "mono" }, m.mount),
          el("td", { class: "mono" }, m.fs),
          el("td", {}, fmtKB(m.total_kb)),
          el("td", {}, fmtKB(m.used_kb)),
          el("td", {}, fmtKB(m.avail_kb)),
          el("td", { style: { minWidth: "140px" } }, bar(m.used_kb, m.total_kb)),
        ]))),
      ]),
  ]);

  const ifaces = el("section", { class: "card" }, [
    el("h3", {}, "网络接口"),
    (info.interfaces || []).length === 0
      ? el("p", { class: "hint" }, "未读取到网口信息")
      : el("table", { class: "sys-table" }, [
        el("thead", {}, el("tr", {}, [
          el("th", {}, "名称"), el("th", {}, "状态"), el("th", {}, "MAC"), el("th", {}, "IP 地址"), el("th", {}, "接收"), el("th", {}, "发送"),
        ])),
        el("tbody", {}, info.interfaces.map(it => el("tr", {}, [
          el("td", { class: "mono" }, it.name),
          el("td", {}, el("span", { class: `pill pill-${it.state === "up" ? "ok" : "muted"}` }, it.state || "?")),
          el("td", { class: "mono small" }, it.mac || "-"),
          el("td", { class: "mono small", style: { whiteSpace: "pre" } }, (it.addrs && it.addrs.length) ? it.addrs.join("\n") : "-"),
          el("td", {}, fmtBytes(it.rx_bytes)),
          el("td", {}, fmtBytes(it.tx_bytes)),
        ]))),
      ]),
  ]);

  const ov = ctx.overview;
  // 全局开关：保留可编辑表单，方便直接在首页改 TCP/UDP 出口
  let globalCard = null;
  if (ov) {
    const g = ov.global || {};
    const nodeOptions = [{ value: "nil", label: "（不使用 / 直连）" }]
      .concat(ov.nodes.map(n => ({
        value: n[".name"],
        label: `${n.remarks || "(未命名)"}  [${n.protocol || n.type || "?"}]  ${n.address || ""}:${n.port || ""}`
      })));
    const enabledCb = checkbox("enabled", g.enabled, "PassWall 总开关：关闭后不会拦截/转发任何流量", "启用 PassWall 主开关");
    const socksCb  = checkbox("socks_enabled", g.socks_enabled, "在路由器上同时暴露一个 Socks5 代理端口", "启用 Socks 代理");
    const tcpSel   = select("tcp_node", g.tcp_node, nodeOptions, "TCP 主出口节点");
    const udpSel   = select("udp_node", g.udp_node, nodeOptions, "UDP 出口节点：填 nil 不走代理，填 tcp 跟随 TCP");
    const form = el("form", {});
    form.addEventListener("submit", e => e.preventDefault());
    form.append(
      el("div", { class: "row" }, [enabledCb, socksCb]),
      el("div", { class: "row" }, [
        field("TCP 出口节点", "改完按下方 保存并应用 即生效", tcpSel),
        field("UDP 出口节点", "可填特殊值 nil（不走代理）/ tcp（同 TCP）", udpSel),
      ]),
      el("div", { class: "row actions" }, [
        el("button", { class: "primary", tip: "把以上更改写入 /etc/config/passwall 并 reload 服务", onclick: async (e) => {
          e.preventDefault();
          const btn = e.currentTarget; const oldText = btn.textContent;
          const patch = {
            enabled: enabledCb.querySelector("input").checked ? "1" : "0",
            socks_enabled: socksCb.querySelector("input").checked ? "1" : "0",
            tcp_node: tcpSel.value,
            udp_node: udpSel.value,
          };
          btn.disabled = true; btn.textContent = "应用中…";
          try {
            await api.patchGlobal(ctx.config, patch);
            await api.pwReload(ctx.config);
            clearDirty(ctx.config);
            toast("✓ 已保存并应用");
            await ctx.refreshAndRedraw();
          } catch (err) { toast("保存全局配置失败", "warn", { detail: formatError(err) }); }
          finally { btn.disabled = false; btn.textContent = oldText; }
        } }, "保存并应用"),
      ]),
    );
    globalCard = el("section", { class: "card" }, [
      el("h3", {}, "PassWall 全局开关"),
      el("div", { class: "kv" }, [
        el("div", { class: "k" }, "主配置"), el("div", { class: "v mono" }, ov.config),
        el("div", { class: "k" }, "节点 / 分流 / 订阅"), el("div", { class: "v mono" },
          `${ov.nodes.length} / ${ov.shunt_rules.length} / ${ov.subscribes.length}`),
      ]),
      form,
    ]);
  }

  root.append(sys, cpuMem, disk, ifaces);
  if (globalCard) root.append(globalCard);
}