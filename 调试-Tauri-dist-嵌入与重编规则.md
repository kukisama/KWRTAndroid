# Tauri 2 + 前端 dist 的"嵌入 vs 实时读盘"问题（KWRTAndroid）

> 适用对象：本仓库后续维护者。
> 目的：解释为什么有时候改了 [dist/](dist/) 的 JS/CSS/HTML，**App 看不到新内容**，到底该重启、该重编、还是该重新打 release。把每种情况写清楚，省得下次又踩同一个坑。

---

## 0. 一句话结论

> **改了 [dist/](dist/) 文件之后，最稳的做法是 `cargo build` 重编一次再启动。**
> 只重启不重编**有时**能看到新内容，**有时**不能；具体取决于 Tauri 2 当前会话是把 dist 走「实时读盘 dev-server」还是「编译期嵌入二进制」那条路径。release 产物**一定**是嵌入的，不重新打包绝对不可能看到新内容。

---

## 1. 仓库当前配置

[src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)：

```jsonc
{
  "build": {
    "frontendDist": "../dist"
    // 注意：没有 "devUrl"
  }
}
```

- 有 `frontendDist`、**没有** `devUrl` → Tauri 2 在构建阶段把 [dist/](dist/) 整个目录作为静态资源**嵌入到 exe**（通过 `tauri-build` 走 `include_dir!` 机制）。
- 没有 `devUrl` 意味着：**没有 webpack-dev-server / vite-dev-server 实时供给静态资源**。WebView 加载的是 `tauri://localhost`，背后是嵌入的字节而非磁盘文件。
- 这跟 React/Vite 模板用 `npm run tauri dev` 时不一样——那种是 `devUrl: "http://localhost:5173"`，WebView 走 HTTP 拉文件，dev-server 监听磁盘变化、热更新。

### 1.1 那为什么这次会话里我经常"只重启不重编也能看到新版"？

实测下来的现象：debug 二进制 `target\debug\kwrt-controller.exe` 启动时，**有些**版本的 Tauri 在 debug 模式下确实会按相对路径再次去磁盘读 [dist/](dist/)（兜底机制），所以 dist 改完重启也能生效；但这是**未在配置里声明**的隐式行为，不保证、不稳定、并且：

- 不同 Tauri 版本、不同 Cargo profile、不同启动方式（cmd 双击 vs `cargo run`）下结果不一致；
- 一旦 webview 命中了缓存版本的 JS（service worker、HTTP cache），新文件读到了也不会被加载；
- 用户报告"看不到新功能"时，**99% 是因为本次启动走了嵌入路径，而嵌入的还是上次 `cargo build` 时的旧 dist**。

所以才有了用户的那句忠告：「**打包 web 到程序，才能看到新的信息，要不可能就会让 debug 下显示不太正常**」。

---

## 2. 五种修改场景下的最稳操作

| 改了什么 | debug 验证（开发态） | 生成给别人用的 exe |
|---|---|---|
| 只改 [src-tauri/src/*.rs](src-tauri/src/) | `cargo build` 重编必走 | `scripts\build-release.ps1` |
| 只改 [dist/**/*.{js,css,html}](dist/) | **`cargo build` 重编**，然后重启 exe | `scripts\build-release.ps1` |
| 同时改 Rust + dist | `cargo build` 重编，重启 exe | `scripts\build-release.ps1` |
| 改 [tauri.conf.json](src-tauri/tauri.conf.json) | `cargo build` 必走 | `scripts\build-release.ps1` |
| 改 [Cargo.toml](src-tauri/Cargo.toml) | `cargo build` 必走 | `scripts\build-release.ps1 -Clean` 更稳 |

**为什么不推荐"重启 exe 但不重编"？** 见 §1.1。会话期间为了快可以试，但只要出现「明明 dist 改了，按钮还是老样子 / JS 报旧错」，**第一反应永远是 `cargo build` 重编一次**，别去怀疑代码写错了。

---

## 3. 操作流程标准化

### 3.1 改 dist/ 后的验证流程

```powershell
# 1. 确保 exe 不在跑（防止 windows 占用导致 link 失败）
Get-Process kwrt-controller -EA SilentlyContinue | Stop-Process -Force

# 2. 重编 debug
cd c:\Scripts\projects\KWRTAndroid\src-tauri
cargo build
# ~13 秒（增量编译；首次冷编要更久）

# 3. 启动
Start-Process .\target\debug\kwrt-controller.exe
```

> Tauri 在 `cargo build` 期间会把整个 [dist/](dist/) 重新打包进 exe（`include_dir!` 算 hash，dist 内容变了就重新嵌入）。如果**只**改了 dist 没改 Rust 源码，Rust 部分不会真正重编，整体很快。

### 3.2 改 Rust 后的验证流程

同上。Rust 改动一定会触发重编（增量），dist 顺便也会重新嵌入一遍。

### 3.3 出 release（给用户的最终 exe）

```powershell
c:\Scripts\projects\KWRTAndroid\scripts\build-release.ps1
# 可选：-Run 编完直接拉起来；-Clean 先 cargo clean 再编（怀疑增量出鬼时用）
```

详细行为见 [scripts/build-release.ps1](scripts/build-release.ps1)：
- 自动 `Stop-Process kwrt-controller`
- `cargo build --release`
- 收纳到 `release\<version>\kwrt-controller_v<ver>_<时间戳>.exe`
- 维护 `release\latest\kwrt-controller.exe` 软拷贝

**release exe 永远是嵌入式的**，复制给别人用只需要这一个 exe 文件，不要带 dist 目录。dist 一旦改了，必须重新跑这个脚本。

---

## 4. 常见症状速查

| 症状 | 大概率原因 | 处理 |
|---|---|---|
| dist 里加了新选项卡，App 看不到 | 嵌入的还是旧 dist | `cargo build` 后重启 |
| dist JS 报旧版本里的语法错 | webview 命中旧嵌入字节 | `cargo build` 后重启 |
| 改 CSS 颜色不生效 | 同上 | `cargo build` 后重启 |
| Rust `pub fn` 改了签名，前端 `invoke` 还是老报错 | exe 是旧的 | `cargo build` 后重启 |
| `cargo build` 报 `failed to remove ... kwrt-controller.exe (os error 32)` | exe 在跑，文件被锁 | `Stop-Process kwrt-controller` 再编 |
| release exe 复制到别处运行只显示空白 | 误把 release 当成需要 dist | release 是嵌入式，单 exe 即可；如果空白请查 `cargo build --release` 是否真的有跑 |
| 改了 [tauri.conf.json](src-tauri/tauri.conf.json) 但 App 行为不变 | 该文件由 `tauri-build` 在编译期烤进 exe | `cargo build` 必走 |

---

## 5. 为什么不直接配 `devUrl` 走"真正的 dev-server 热更新"

可以，但成本/收益不划算：
- 本仓库前端是纯静态 ES module（无 React/Vite），没必要拉个 dev-server；
- 加 `devUrl` 后开发时多一个 node 进程要管，启动顺序也要小心；
- 现状下 `cargo build` 增量编译 + 重启 ≈ 15 秒，开发节奏完全可接受。

> 如果将来真要迁，标准做法是：`tauri.conf.json` 加 `"devUrl": "http://localhost:5173"`，配合 vite/esbuild watch；同时保留 `frontendDist: "../dist"` 给 release。Tauri 会在 debug 模式优先用 `devUrl`，release 模式用嵌入资源。

---

## 6. TL;DR

1. 看不到新功能、报旧错？**先 `cargo build`**，别先怀疑代码。
2. 给别人 exe？**永远走 [scripts/build-release.ps1](scripts/build-release.ps1)**，单 exe 直接用，别带 dist。
3. exe 被锁导致 link 失败？**`Stop-Process kwrt-controller`** 再编。
4. 配置/Cargo.toml 改了？**重编必走**，没有捷径。
