# KWRT Controller (Windows · Tauri 2 + Rust)

> 基于 PassWall 的快速控制器。详细可行性分析见 [调研-Windows端可行性.md](调研-Windows端可行性.md)。

## 编译

```pwsh
cd src-tauri
cargo build --release
```

产物：`src-tauri/target/release/kwrt-controller.exe`（无 MSI/NSIS）。

## 运行

直接双击 exe，或：

```pwsh
Start-Process .\src-tauri\target\release\kwrt-controller.exe
```

## 功能

- 表单登录 + ubus session.login 双通道认证
- 登录后自动诊断：
  - 系统信息（hostname / OpenWrt release）
  - 列出全部 UCI 配置
  - 识别 **PassWall v1 / v2 / Both / 无**
  - 探测直连列表文件 (`/usr/share/passwall[2]/rules/direct_ip` 等)
  - 检测 init 脚本是否注册（`rc.list`）
  - 试读直连列表，统计条目数
- 快捷动作：
  - 加入 IP / CIDR / 域名到直连列表（去重 + 字符白名单）
  - 删除单条
  - reload / restart PassWall
- 凭据保存：勾选"记住密码"后写入 **Windows 凭据管理器**（keyring crate，DPAPI）；不会写入任何明文文件
- 自签 HTTPS：默认拒绝；用户主动勾选后才放行

## 安全

- HTTP 客户端走 Rust 端（绕开 WebView CORS）
- 默认拒绝无效证书；凭据加密存储；删除按钮一键擦除
- 写入直连列表前对条目做字符白名单校验，仅允许 `[a-zA-Z0-9._:/-]`
- 重启 / 重载 PassWall 等高风险动作有二次确认

## 兼容性

- OpenWrt / KWRT / ImmortalWrt 等任何带 `luci-app-passwall` 或 `luci-app-passwall2` 的固件
- 自动选 primary 配置（v1 优先），所有路径在登录后实时探测
