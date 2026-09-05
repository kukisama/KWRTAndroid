# KWRT Controller — Android 端

最小化原生 Android 客户端。功能严格两项：
1. **登录**：填路由器 IP / 端口 / 账号 / 密码，勾选"记住"后下次进入自动登录。
2. **客户端开关**：登录后只显示一张列表，每行 = 备注 + 源 IP/MAC + 启用开关；底部"应用配置"按钮触发 PassWall 后台 reload。

不做新增 / 编辑 / 删除 / 节点切换等任何写操作（按你要求精简到只剩 toggle）。

## 与桌面端一致

- 直连 OpenWrt LuCI 的 `/ubus`，复用 `session.login` + `file.exec sh -c "<script>"`，路由器侧零改动。
- 色板（[ui/theme/Theme.kt](app/src/main/java/com/kwrt/controller/ui/theme/Theme.kt)）从 Tauri 端 [dist/style.v4.css](../dist/style.v4.css) 的 CSS 变量整套搬过来（`--bg / --card / --primary / --ok / --text / --muted / --line`），浅 / 深主题跟随系统。
- 全面屏：`enableEdgeToEdge` + `windowInsetsPadding(safeDrawing/navigationBars)`，登录页避开状态栏 + 输入法，列表页 TopAppBar 走 statusBars，底部按钮区贴手势条上方。

## 协议要点（对齐 Rust 端）

| 操作 | shell 脚本 | 来源 |
|---|---|---|
| 读 ACL | `uci show passwall` | [acl.rs](../src-tauri/src/acl.rs) `read()` |
| 切换启用 | `uci set passwall.SEC.enabled='1/0' && uci commit passwall && echo OK` | `update()` 子集 |
| 应用配置 | `( setsid /etc/init.d/passwall reload </dev/null >/dev/null 2>&1 & ) >/dev/null 2>&1 && echo OK` | `reload()` |

切换是乐观更新 + 失败回滚，不立即 reload；用户点底部"应用配置"才统一 reload（避免每次都等 10–60 s）。

## 构建

需要本机已装 JDK 17 + Android SDK（`ANDROID_HOME` 环境变量指向 SDK 根目录）。

**推荐**：直接用仓库根目录的脚本，自动检测依赖、缺啥提示啥：

```pwsh
pwsh c:\Scripts\projects\KWRTAndroid\scripts\build-android.ps1
# 产物：dist-android\app-debug.apk
# Release（未签名）：pwsh scripts/build-android.ps1 -Release
```

手工方式：

```pwsh
cd c:\Scripts\projects\KWRTAndroid\android
gradle wrapper --gradle-version 8.9      # 仅首次
.\gradlew.bat assembleDebug
# 产物：app/build/outputs/apk/debug/app-debug.apk
```

## 已知/取舍

- **凭据存储**：用应用私有 `SharedPreferences`（其它应用读不到）。如需更严谨，可换 `androidx.security:security-crypto` 的 `EncryptedSharedPreferences`。
- **HTTPS 自签**：OkHttp 关闭证书校验，等价 Rust 端 `accept_invalid_certs`。仅适合 LAN 路由器场景。
- **不做**：节点选择 / 详细字段编辑 / 总览页 / 直连列表 —— 这些都在桌面 Tauri 端。
