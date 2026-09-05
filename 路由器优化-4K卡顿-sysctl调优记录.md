# KWRT 路由器 4K 卡顿优化记录（2026-06-04）

## 背景
看 4K 视频卡顿，定位到路由器 CPU 被代理进程吃满。

- 设备：Netgear R6800 / MT7621 / MIPS 1004Kc 双核 4 VPE，880 MHz，**无 AES 硬件加速**
- 系统：Kwrt 25.12-SNAPSHOT（`05.31.2026` build）
- 代理：luci-app-passwall 26.6.2-r152
- 节点：`233boy-hysteria2-20.212.22.105`（Hysteria2 / QUIC / TLS）

## 已做的优化

### 优化 1：节点 type 从 Xray 切到 sing-box
**位置**：PassWall → 节点列表 → 编辑该节点 → 客户端类型选 `Sing-Box` 保存。
（或重拉订阅，订阅里已设 `hysteria2_type=sing-box`，重拉后新节点自动是 sing-box。）

**效果**：xray 进程从 ~61% CPU → sing-box ~46% CPU。

**回退**：编辑节点改回 Xray 保存。

### 优化 2：放大 TCP 缓冲区（sysctl）
**位置**：路由器 `/etc/sysctl.d/99-net-tuning.conf`（LuCI 无此 UI）。

**写入内容**：
```ini
net.core.rmem_max = 4194304
net.core.wmem_max = 4194304
net.ipv4.tcp_rmem = 4096 87380 4194304
net.ipv4.tcp_wmem = 4096 16384 4194304
```

**原值**（备份在 `/etc/sysctl.d/99-net-tuning.conf.previous.<timestamp>`）：
```
net.core.rmem_max = 180224
net.core.wmem_max = 180224
net.ipv4.tcp_rmem = 4096 87380 6291456    （默认上限其实就有 6MB，但被 core.rmem_max 截到 180KB）
net.ipv4.tcp_wmem = 4096 16384 4194304
```

**作用**：默认 `rmem/wmem_max` 只有 ~180KB，跨境 RTT 200ms+ 时 BBR 撑不开窗口（带宽×延迟乘积塞不下），4K 码率（25 Mbps）卡顿。抬到 4MB 后 BBR 能正常发挥。

**副作用**：实际为零。Linux 按需分配，国内/局域网小 RTT 仍按小值分配；只有"高吞吐 + 高 RTT"的少数连接才真的吃到大缓冲。本机内存 247MB，free 30MB+ cache 120MB+，conntrack 才 142 条，余量很大。

## 检查 / 验证命令

SSH 到 10.0.0.1（root）后：

```sh
# 看当前值（确认生效）
sysctl net.core.rmem_max net.core.wmem_max net.ipv4.tcp_rmem net.ipv4.tcp_wmem

# 看 CPU 占用大户
top -bn1 | head -25

# 看负载和内核态/中断比例（usr/sys/sirq）
uptime
top -bn1 | grep ^CPU

# 看 sing-box 实际线程数和内存
ps w | grep -iE 'xray|sing-box' | grep -v grep
PID=$(pidof sing-box); cat /proc/$PID/status | grep -E '^(VmRSS|VmSize|Threads)'

# 看 conntrack 连接数 / 上限
cat /proc/sys/net/netfilter/nf_conntrack_count
cat /proc/sys/net/netfilter/nf_conntrack_max

# 看 BBR 拥塞控制是否启用（应为 bbr）
sysctl net.ipv4.tcp_congestion_control net.core.default_qdisc

# 看备份
ls -la /etc/sysctl.d/99-net-tuning.conf*
```

期望状态：
- `rmem_max` / `wmem_max` = 4194304
- `tcp_congestion_control` = bbr（本机已开）
- sing-box `%CPU` 在普通看视频时 30–60%；播放 4K 跨境 60–100% 算正常（MIPS+软件 AES 的硬上限）

## 怎么改回来 / 怎么微调

### 完全恢复默认
SSH 到路由器：
```sh
rm /etc/sysctl.d/99-net-tuning.conf
# 立即生效
sysctl -w net.core.rmem_max=180224
sysctl -w net.core.wmem_max=180224
# 或直接 reboot
reboot
```

也可参考 `/etc/sysctl.d/99-net-tuning.conf.previous.<timestamp>` 里记录的原值。

### 调大/调小
直接编辑 `/etc/sysctl.d/99-net-tuning.conf` 改数字，然后：
```sh
sysctl -p /etc/sysctl.d/99-net-tuning.conf
```

经验值：
- 内存紧张的小路由：1–2 MB（`1048576` / `2097152`）就够
- 256 MB+ 内存、跨境高带宽：4–8 MB

## 还没做、但可考虑的优化（按收益）

1. **换非 Hysteria2 节点**（VLESS-Reality / SS chacha20）— MIPS 无 AES 硬件加速，ChaCha20 比 AES-GCM 快 2–3 倍。**这是真正能把 CPU 砍半的杀招**。
2. PassWall 「高级设置 → 转发配置 → UDP 转发端口」改成「不使用」— 减软中断负担。**副作用**：外服游戏 / Telegram 语音 / Zoom 外服会议会变直连，可能用不了。日常只看视频可以改。
3. PassWall 「高级设置 → 转发配置 → TCP 转发端口」收窄到 80,443,8080,8443 — 收益小，副作用是 SSH/邮件/git 不走代理。
4. DNS：把 `dns_mode` 从 tcp 改回 udp（少一些 TCP 握手开销）。

## 探针脚本（在仓库里）
- `src-tauri/examples/probe_cpu_perf.rs`   — 总体 CPU / 网络 / passwall 状态扫描
- `src-tauri/examples/probe_cpu_perf2.rs`  — 当前节点详情 / sing-box 配置摘要
- `src-tauri/examples/probe_cpu_perf3.rs`  — 切换后复查
- `src-tauri/examples/apply_sysctl_tuning.rs` — 应用 sysctl 调优（带备份）

复跑：
```pwsh
cd src-tauri
cargo run --example probe_cpu_perf --quiet
```
