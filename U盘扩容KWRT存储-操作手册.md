# U 盘扩容 KWRT 存储 —— 操作手册

> 适用场景：KWRT 的 `/overlay`（可写覆盖层）容量不足，导致 PassWall 等 LuCI 应用安装/更新二进制（sing-box、xray、hysteria 等）时报 `/usr/bin 空间不足` 或 `No space left on device`。
>
> 适用设备：本仓库实测机型 **Netgear R6800**（mt7621 / mipsel），KWRT 25.12-SNAPSHOT。其它机型原理相同，分区号可能不一样。
>
> 适用前提：路由器已能 SSH/LuCI 登录；本仓库的 `LuciClient`（[`src-tauri/src/client.rs`](src-tauri/src/client.rs)）已能正常连接 `10.0.0.1`。

---

## 0. 为什么会反复需要做这件事

R6800 的 NAND 共 128 MiB，但 KWRT 实际可写的 `/overlay` 只有约 **9 MiB**（mtd3 UBI 内的 ubi0_1）。一旦：

- **刷了新固件**（sysupgrade 不勾选"保留配置"，或者整盘重刷）→ overlay 被清空、fstab 配置丢失
- **升级了 KWRT 大版本** → 部分内核模块版本变了，需要重新触发挂载
- **换了 U 盘** → UUID 变了，旧 fstab 条目失效

都会让"U 盘扩容"这一步从头再做一遍。所以记录成可重复执行的流程很有必要。

---

## 1. 设计原则（为什么这么做，而不是别的方案）

1. **不动 `/overlay` 本身**（即不做 extroot）
   - extroot 把整个 `/overlay` 转移到 U 盘，固件配置全部依赖 U 盘 → 一旦 U 盘掉，路由配置全没。
   - 本方案只把 U 盘挂到 `/mnt/sda1`，让用户把"可选的大二进制"放过去。**U 盘坏了/拔了，最多只是 sing-box 起不来，路由本体零影响**。
2. **格式化为 ext4，且去掉 journal（`-O ^has_journal`）**
   - FAT32/exFAT 不保留 Linux 可执行位，二进制放上去运行不了 → 必须 ext4 / f2fs / btrfs。
   - sing-box 这种二进制是"写一次、读多次"，不需要 journal；关掉 journal 显著减少 U 盘写入量，**延长寿命**。
3. **用 UUID 而不是 `/dev/sda1` 挂载**
   - 换 U 盘插入顺序、插不同口都不会让设备名变化失败。
4. **`block-mount` + `/etc/config/fstab` 而不是直接写 `/etc/fstab`**
   - 这是 OpenWrt/KWRT 的标准做法，重启后会被正确处理；纯 fstab 在 KWRT 上会被覆盖。
5. **所有步骤通过 ubus + `file.exec` 走 LuCI 协议**
   - 复用现有 `LuciClient.shell()`，凭据走 Windows 凭据管理器（`creds::load_password`）。

---

## 2. 操作流程总览

```
[1] 检查 U 盘是否被识别       (probe_usb.rs)
        │
        ├── /dev/sdaN 存在？     ──否──► 检查 USB 模块 / 换口 / 换盘
        ├── kmod-usb-storage / kmod-fs-ext4 / e2fsprogs 是否齐全？
        │     └── KWRT 默认全有；若缺失：opkg update && opkg install ...
        │
[2] 与用户确认"清空 U 盘"     (因为格式化会丢数据)
        │
[3] 真正执行初始化            (init_usb.rs)
        ├── umount 已自动挂的 vfat
        ├── mkfs.ext4 -F -L KWRT_DATA -m 0 -O ^has_journal
        ├── 重新挂到 /mnt/sda1
        ├── 创建 /mnt/sda1/bin（专门放二进制）
        ├── 用 uci 写 fstab.@mount[N]（按 UUID）
        ├── /etc/init.d/fstab enable && restart
        └── 写一个临时 .sh 验证 exec 位（确认 ext4 没挂错选项）
        │
[4] 在 LuCI / PassWall 把程序路径改到 /mnt/sda1/bin/<name>
        └── 让 PassWall 自己下载，写入会落到 U 盘
```

---

## 3. 第一步：检查 U 盘 — [`src-tauri/examples/probe_usb.rs`](src-tauri/examples/probe_usb.rs)

执行：

```powershell
cd src-tauri
cargo run -p kwrt-controller --example probe_usb --quiet
```

**关注下面这几项输出，全过才能进入下一步：**

| 探测项 | 期望结果 | 不通过怎么办 |
|---|---|---|
| `usb-dev` | 看到 `/dev/sda` 或 `/dev/sdb`，以及对应 `*1` 分区 | 拔插重试 / 换 USB 3.0 口 / 检查 `lsmod` 是否有 `usb_storage` `xhci_*` |
| `dmesg-usb` | 看到 `usb-storage ... USB Mass Storage device detected`、`[sda] Attached SCSI removable disk` | 看 dmesg 里 `error -110/-71` 之类，多半是供电或 U 盘本身问题 |
| `block-info` | 列出 `/dev/sda1: ... TYPE="vfat"` 之类 | 没有就说明分区表损坏，需要先 `parted` 或 `fdisk` 重建分区 |
| `kmod-usb` 内核模块 | 含 `usb_storage`、`scsi_mod`、`ext4`、`xhci_*` | 缺哪个就 `opkg install kmod-usb-storage kmod-fs-ext4 kmod-usb3` |
| `opkg-installed-storage` | 含 `block-mount`、`e2fsprogs`、`kmod-fs-ext4` | 缺哪个补哪个；R6800 上 KWRT 默认全装 |
| `fstab` | 现有内容里**不能**已经有指向这块 U 盘 UUID 的 mount 段 | 若有旧的，先 `uci delete fstab.@mount[N]` 清掉再走下一步 |

---

## 4. 第二步：与用户确认（不要省略）

格式化会清空 U 盘。在 IM/聊天里务必让用户**确认**：

- U 盘里没有需要保留的数据
- 接受 ext4（Windows 默认读不了，但 Linux/路由读写无障碍）
- 已经准备好用品牌 U 盘并插在 USB 3.0 口

只有明确"是"才执行下一步。`init_usb.rs` 内部没做二次确认，是设计成"用户已经在外部同意"。

---

## 5. 第三步：执行初始化 — [`src-tauri/examples/init_usb.rs`](src-tauri/examples/init_usb.rs)

执行：

```powershell
cd src-tauri
cargo run -p kwrt-controller --example init_usb --quiet
```

脚本里每一步都是独立的 `shell()` 调用，**任何一步非零退出即中止**，便于排错。各步骤含义：

| 步骤 | 命令要点 | 为什么这么写 |
|---|---|---|
| `pre-umount` | `umount /mnt/sda1; umount /dev/sda1; true` | KWRT 默认自动挂载 vfat，必须先卸掉；`true` 兜底，未挂时不报错 |
| `pre-check` | `ls -l /dev/sda1 && blkid` | 防止设备消失或换号 |
| `mkfs` | `mkfs.ext4 -F -L KWRT_DATA -m 0 -O ^has_journal /dev/sda1` | `-F` 强制（已挂过）；`-m 0` 取消 5% root 预留；`^has_journal` 关 journal 省 U 盘寿命 |
| `uuid` | `blkid /dev/sda1` | 拿到 ext4 的新 UUID（**和原来 vfat 的 UUID 不一样**） |
| `mount` | `mkdir -p /mnt/sda1 && mount -t ext4 ...` | 立刻挂上，后面才能创建子目录 |
| `mkdirs` | `mkdir -p /mnt/sda1/bin /mnt/sda1/share` | `bin/` 专门放二进制，`share/` 留给 geoip/geosite 等数据文件 |
| `uci-fstab` | 删除现有 `@mount[*]` → 新增一个 UUID 挂载段 → `uci commit fstab` | 用 UUID 不用设备名；`noatime,nodiratime` 进一步减少写入 |
| `enable-fstab` | `/etc/init.d/fstab enable; restart` | 让 KWRT 的 block-mount 服务接管，重启后自动挂载 |
| `verify-exec` | 写一个 `.exectest` 脚本 `chmod +x` 后执行 | **关键**：如果 ext4 选项写错（比如 `noexec`），这步会失败，提早暴露问题 |
| `final-summary` | 汇报 `df -h`、`/etc/config/fstab`、`/mnt/sda1/bin` 内容 | 让人一眼看到结果 |

正常输出会以以下两行结尾：

```
[done] U 盘已格式化为 ext4 并自动挂载到 /mnt/sda1
[done] 把 PassWall 的 Sing-Box 程序路径改为: /mnt/sda1/bin/sing-box
```

---

## 6. 第四步：在 LuCI 上完成最后一步

1. 打开 `http://10.0.0.1` → 服务 → PassWall → **组件更新**
2. 把 **Sing-Box 程序路径** 改为：`/mnt/sda1/bin/sing-box`
3. 保存并应用
4. 点 **检查更新 / 点击更新** → 这次下载会落到 U 盘

> Xray / Hysteria / Geoview 想搬过去同理：把对应"程序路径"改成 `/mnt/sda1/bin/<name>`，再点更新。

---

## 7. 失败/异常恢复（按"破坏性"从小到大）

| 现象 | 原因 | 操作 |
|---|---|---|
| 重启后 `/mnt/sda1` 是空的 | block-mount 没起来 / U 盘没插稳 | `/etc/init.d/fstab restart && mount \| grep sda` |
| `df` 里看不到 `/dev/sda1` | U 盘掉线 | `dmesg \| tail -30` 看 usb 错误；换 USB 口 |
| PassWall 报"sing-box 未找到" | 文件不在 `/mnt/sda1/bin/sing-box` 或者 U 盘没挂上 | 先确认 `ls -la /mnt/sda1/bin/sing-box`；再确认 PassWall 路径配置 |
| 想完全回到出厂 | — | `uci delete fstab.@mount[0]; uci commit fstab; /etc/init.d/fstab restart`，拔 U 盘即可 |
| 升级 KWRT 后想再来一遍 | overlay 被清掉、fstab 没了 | 直接重新跑 `cargo run --example probe_usb` 然后 `init_usb`；U 盘里的 sing-box 二进制如果还在，挂上后 PassWall 也能直接用 |

---

## 8. 稳定性与边界（必须告诉用户）

- **路由器本体不依赖 U 盘**。rootfs（`/rom` squashfs）和 overlay 都在 NAND（mtd3 UBI）上。
- 最坏情况：U 盘坏了/拔了 → **只是 PassWall 启动 sing-box 失败**，路由、Wi-Fi、LuCI、其它 PassWall 后端（xray/hysteria 如果还在 `/usr/bin`）全部正常。
- 严禁**热拔 U 盘**：会让 sing-box 进程立刻崩溃。
- 优先 **USB 3.0 + 品牌 U 盘**；劣质 U 盘容易掉线、写入很快坏。
- 不把 swap 指到 U 盘（系统已经有 zram swap）；不把日志指到 U 盘（无意义磨损）。

---

## 9. 关键事实备忘（避免下次又踩坑）

> 这些事实已写入 `/memories/repo/`，本文档作为人类可读补充。

- R6800 NAND 128 MiB：实际给 KWRT 用的只有 mtd3（40 MiB UBI），其它都是 Netgear 原厂残留分区（ML1–11 / Nvram / reserved0–5），**reserved5 单块就有 33.5 MiB 闲置**。如果想"不插 U 盘也扩容"，路径是重编固件合并分区，**风险远大于 U 盘方案**。
- KWRT 25.12 已默认安装好 `block-mount` / `e2fsprogs` / `kmod-fs-ext4` / `kmod-usb3` / `kmod-usb-xhci-mtk` 等所有必要模块，**不需要再 opkg install**。
- PassWall "组件更新" 的程序路径字段对应 uci：`passwall.@global_app[0].xxx_file`（例如 `singbox_file`、`xray_file`、`hysteria_file`）。直接改路径文本框是最直观的方式，无须知道 uci key。
- `/etc/config/fstab` 用 `uci show fstab` 查看，结构是 `@global[0]` + 一或多个 `@mount[N]` + 可选 `@swap[N]`。
