# 直连 KWRT 进行资料查询和处理

> 适用对象：本仓库后续维护者。
> 目的：当你不确定 KWRT / luci-app-passwall 在路由器上的真实行为（字段名、脚本路径、init.d 行为、ACL 限制……），**不要靠记忆和猜测**，按本文方法在 10 分钟内造一个独立 Rust 探针程序，**复用 App 已经保存的登录凭据**直连真机拿事实，再写代码。

---

## 0. 总原则

- **任何关于"路由器侧 PassWall/LuCI 是如何工作的"问题，第一步都是写一个 `probe_*.rs` 实测**，不要再凭经验/上游 GitHub 文档猜。本仓库已有的 4 个探针文件就是模板：
  - [src-tauri/examples/probe_passwall.rs](src-tauri/examples/probe_passwall.rs) —— 整体结构、test.sh、二进制
  - [src-tauri/examples/probe_passwall_2.rs](src-tauri/examples/probe_passwall_2.rs) —— 详细脚本内容
  - [src-tauri/examples/probe_passwall_3.rs](src-tauri/examples/probe_passwall_3.rs) —— HTTP 端点直连
  - [src-tauri/examples/probe_save_apply.rs](src-tauri/examples/probe_save_apply.rs) —— ubus 能力 + uci changes
  - [src-tauri/examples/probe_apply_semantics.rs](src-tauri/examples/probe_apply_semantics.rs) —— save/apply/reset 三按钮语义
- 探针只用于读取与验证，**不要在探针里做破坏性操作**（写 uci、重启服务）；要写也写到临时 section，写完立刻 `uci revert`。

---

## 1. 凭据复用机制

App 在用户首次登录并勾选"记住密码"时，会把密码写到 Windows 凭据管理器。条目命名约定见 [src-tauri/src/creds.rs](src-tauri/src/creds.rs#L19)：

```
target = LegacyGeneric:target=root.kwrt-controller::<host>::<user>
```

例：`LegacyGeneric:target=root.kwrt-controller::10.0.0.1::root`

确认凭据已存：

```powershell
cmdkey /list | Select-String "kwrt"
```

预期输出：
```
目标: LegacyGeneric:target=root.kwrt-controller::10.0.0.1::root
```

探针直接调 `creds::load_password(host, user)` 即可拿到明文密码，**无需用户再输**。条目没有就报错退出。

---

## 2. 写探针的最小模板

新建 `src-tauri/examples/probe_<topic>.rs`：

```rust
use anyhow::Result;
use kwrt_controller_lib::{client::{ConnectOptions, LuciClient}, creds};

#[tokio::main]
async fn main() -> Result<()> {
    let host = "10.0.0.1";
    let user = "root";
    let pass = creds::load_password(host, user)?
        .ok_or_else(|| anyhow::anyhow!("Windows 凭据里没有 {host}/{user}"))?;

    // connect 已经做了 ubus 登录 + form 登录，拿到 sysauth cookie。
    let c = LuciClient::connect(ConnectOptions {
        host: host.into(),
        scheme: Some("http".into()),
        port: None,
        username: user.into(),
        password: pass,
        accept_invalid_certs: false,
        timeout_secs: Some(60),
    }).await?;

    // 一次跑多个脚本，结果按 label 分组打印
    for (label, script) in [
        ("hello", "echo 'hi from router'; uname -a"),
        ("uci-snapshot", "uci show passwall | head -20"),
    ] {
        eprintln!("\n========== {label} ==========");
        match c.shell(script).await {
            Ok((code, out, err)) => {
                eprintln!("[exit={code}]");
                print!("{out}");
                if !out.ends_with('\n') { println!(); }
                if !err.trim().is_empty() { eprintln!("--stderr--\n{err}"); }
            }
            Err(e) => eprintln!("[err] {e:#}"),
        }
    }
    Ok(())
}
```

运行：

```powershell
cd c:\Scripts\projects\KWRTAndroid\src-tauri
cargo run --quiet --example probe_<topic>
```

`--quiet` 抑制 cargo 进度噪音，只保留 warning 和你自己的 print。

> 要使 `kwrt_controller_lib::client` / `creds` 在 example 里能访问，[src-tauri/src/lib.rs](src-tauri/src/lib.rs#L1) 头部已经把 `client`/`creds` 改成 `pub mod`，**保持这两个声明为 `pub mod` 不要回退**。

---

## 3. LuciClient 上能用的 API

[src-tauri/src/client.rs](src-tauri/src/client.rs)：

| 方法 | 用法 | 说明 |
|---|---|---|
| `c.shell(script: &str)` | 通过 `ubus call file exec sh -c <script>` 在路由器侧 sh 执行任意一段脚本 | 返回 `(exit_code, stdout, stderr)`。多行脚本用 raw string `r#"..."#` |
| `c.file_exec(cmd, args)` | 直接 exec 一个具体命令，参数数组形式 | 返回 `Value`，含 `code/stdout/stderr` |
| `c.http.get/post(...)` | 已有 sysauth cookie 的 `reqwest::Client` | 直接打 LuCI 的任何 endpoint，比如 `/cgi-bin/luci/admin/services/passwall/ping_node?...` |
| `c.base_url` | 形如 `http://10.0.0.1/` 的 `url::Url` | 用 `.join(path)` 拼相对路径 |

> `file.exec` 的 ACL 在 KWRT 上是允许的，`uci.commit` 也是允许的（实测）。我们应用代码里很多写操作历史上走 `file.exec sh -c "uci set ...; uci commit"` 的 CLI 路径，是为了规避某些固件 ACL 把 uci.commit 禁掉的情况——稳定，无需改。

---

## 4. 探针应当探的"常见问题清单"

| 想知道什么 | 推荐脚本 |
|---|---|
| **PassWall 安装情况** | `opkg list-installed \| grep passwall` |
| **PassWall 文件结构** | `ls -la /usr/share/passwall`，`ls -la /usr/lib/lua/luci/{controller,model/cbi,view}/passwall* 2>/dev/null` |
| **某个 UCI 字段的真实名字** | `uci show passwall \| grep -i <猜测>`；然后看 CBI 模型 `grep -RInE '<候选字段名>' /usr/lib/lua/luci/model/cbi/passwall` |
| **LuCI 某个按钮调的端点** | 先在 chrome 里点一下记录请求，或者 `grep -RInE 'entry.*<功能名>\|/admin/services/passwall/<...>' /usr/lib/lua/luci/controller/passwall.lua` |
| **某个端点的源码** | `cat /usr/lib/lua/luci/controller/passwall.lua \| sed -n '<start>,<end>p'` |
| **某个 init.d 脚本的 reload 行为** | `head -80 /etc/init.d/<svc>`，`grep -nE 'reload\|restart\|trigger' /etc/init.d/<svc>` |
| **ubus 上有哪些可调对象** | `ubus list`；某个对象的方法签名 `ubus -v list uci` |
| **uci 是否有 staging 未提交** | `uci changes`；按 config `uci changes passwall`；目录 `ls -la /tmp/.uci` |
| **某次写入到底改了什么** | 写之前 `cp /etc/config/passwall /tmp/before; ...; diff -u /tmp/before /etc/config/passwall` |
| **节点测试用什么命令** | `command -v tcping; command -v nc; busybox nc 2>&1 \| head -3` |
| **busybox 的 nc 支不支持某选项** | `busybox nc 2>&1 \| head -3` —— **KWRT busybox 1.37 的 nc 只接 `nc IP PORT`，不支持 `-z/-w`！** |

---

## 5. 已经验证的关键事实（写代码前可直接引用）

> 这些是 2026/6 在 10.0.0.1 上跑探针得到的事实，**不是规范**。换台路由器要重新验证。

### 5.1 设备
- Kwrt 25.12-SNAPSHOT，mt7621 / mipsel
- luci-app-passwall = `26.4.15-r92`
- 路径：`/usr/share/passwall/`、`/usr/lib/lua/luci/{controller,model/cbi/passwall,view/passwall}`
- 二进制都在 `/usr/bin/`：`xray` `chinadns-ng` `tcping`（busybox 提供 `nc` `ping`）

### 5.2 节点测试 HTTP 端点（直接 GET 即可，已带 sysauth cookie）

| URL | 返回 | 注意 |
|---|---|---|
| `/cgi-bin/luci/admin/services/passwall/ping_node?type=icmp&address=&port=` | `{"ping":"95"}`（ms，字符串） | port 即便不用也要带 |
| `/cgi-bin/luci/admin/services/passwall/ping_node?type=tcping&address=&port=` | 同上，但用 `/usr/bin/tcping` | 路由器必须装 tcping，否则退化成 ICMP |
| `/cgi-bin/luci/admin/services/passwall/urltest_node?id=<section>` | `{"use_time":"1315.78"}`（ms，字符串） | 目标 URL 由 `uci passwall.@global_other[0].url_test_url` 决定（默认 `https://www.google.com/generate_204`） |

→ 见 [src-tauri/src/passwall.rs `http_ping`](src-tauri/src/passwall.rs)。

### 5.3 订阅 section 字段（最容易踩的坑）

`passwall.@subscribe_list[X]` 上**有两个字段都翻译成"备注"**：

| 字段 | 写入方 | 用途 |
|---|---|---|
| `remark` | 用户在 LuCI 表单里手填 | 表单显示；去重校验（不允许同名 / `default`） |
| `remarks` | `subscribe.lua` 拉到订阅后自动写入（订阅源的标题） | 卡片/日志展示用 |

UI 上的"备注"输入框，**对应官方字段是 `remark`，不是 `remarks`**。

### 5.4 节点 ping/tcping 命令
- `tcping` 已安装：`/usr/bin/tcping`
- **busybox nc 不支持 `-z` `-w`**，不要再用 `nc -zw2` 拼 TCP 握手测试

### 5.5 save / apply / reset 三按钮真正的语义

参 `/usr/lib/lua/luci/view/cbi/footer.htm`（[源码片段见 §6](#6-保存保存并应用复位的-luci-原意)）：

| 按钮 | onclick | 后端行为 |
|---|---|---|
| **保存** (`cbi-button-save`) | `type=submit` 普通提交 | 把表单写入 `/tmp/.uci/<config>`（**staging**），不 commit、不 reload |
| **保存并应用** (`cbi-button-apply`) | `cbi_submit(this, 'cbi.apply')` | staging + `uci commit` + 触发 init.d `<config>` reload |
| **复位** (`cbi-button-reset`) | `location.href = REQUEST_URI` | **只是重新 GET 当前页**——把表单上的本地修改全部丢弃，回到磁盘里的值；**不会撤销已 commit 的 uci changes** |

**注意**：上游 LuCI 新版本里"保存"也写到 staging 后即时 commit，是不是 reload 取决于页面是否走 `apply_unattended` 流程。PassWall 的 CBI 用 `api.set_apply_on_parse(map)` 在每次表单提交后异步 `/etc/init.d/passwall reload &`，所以 PassWall 表单的"保存"实际等同于"保存并应用"。**我们要在 UI 里复刻 KWRT 上"保存/保存并应用/复位"三按钮的语义，必须自己显式区分，不要依赖 PassWall hook**。

### 5.6 PassWall 自家提交辅助（`/usr/lib/lua/luci/passwall/api.lua`）

```lua
function uci_save(cursor, config, commit, apply)
  -- 新 LuCI 分支：commit=true 时一定 commit；apply=true 时走 cursor:commit，
  -- apply=false 时走 sh_uci_commit（CLI）
  -- 旧 LuCI 分支：cursor:save → cursor:commit → /etc/init.d/<config> reload &
end
function sh_uci_set(config, section, option, val, commit)
function sh_uci_del(config, section, option, commit)
function sh_uci_add_list(config, section, option, val, commit)
function sh_uci_commit(config)  -- 内部 = uci -q commit <config>
```

可直接复用这些 helper，但当前我们 App 走 `file.exec sh -c` CLI 路径同样能达到效果。

### 5.7 ubus 上 uci/service 完整签名

```
uci:     get/state/add/set/delete/rename/order/changes/revert/commit
service: apply{rollback,timeout}/confirm/rollback/reload_config/...
```

- 安全模式 apply：`service apply {rollback:true, timeout:30}` → 30 秒内不 `service confirm` 就自动 rollback。LuCI 主题用它防失联。
- 我们 PassWall 改的是代理而不是基础网络，不需要 rollback 模式，commit + reload 足够。

### 5.8 uci 写入 → 服务生效 的三阶段（再贴一次）

```
内存表单 ──set──▶ /tmp/.uci/<config>  ──commit──▶ /etc/config/<config>  ──apply/reload──▶ init.d 服务重读
            (uci changes 可见)        (持久化)                            (运行的进程感知到变更)
```

- 只 set 不 commit：仅 staging，重启路由器/换 ubus session 就丢
- commit 不 reload：磁盘已变，但守护进程仍跑旧配置
- reload 不 commit：reload 读的是 `/etc/config/`，相当于没改

---

## 6. "保存/保存并应用/复位"的 LuCI 原意

`/usr/lib/lua/luci/view/cbi/footer.htm`：

```html
<%- if display_apply then -%>
  <input class="btn cbi-button cbi-button-apply" type="button" value="Save & Apply"
         onclick="cbi_submit(this, 'cbi.apply')" />
<%- end -%>
<%- if display_save then -%>
  <input class="btn cbi-button cbi-button-save"  type="submit"  value="Save" />
<%- end -%>
<%- if display_reset then -%>
  <input class="btn cbi-button cbi-button-reset" type="button" value="Reset"
         onclick="location.href='<%=REQUEST_URI%>'" />
<%- end -%>
```

行为：
- **保存** = 表单 POST，dispatcher 跑 CBI 处理：所有 `o:option(...)` 写入 staging，不 commit
- **保存并应用** = 表单 POST 时带 `cbi.apply` flag，处理后 commit + 触发 reload（PassWall 这里走 `api.set_apply_on_parse`）
- **复位** = 浏览器直接 `location.href = 当前 URL`，重新 GET 表单，所有未提交编辑作废；不动磁盘和服务

> 用户的"复位"心智很容易误以为"把刚才 commit 的也回滚掉"。**真不是**。LuCI 自带的复位等同浏览器刷新。如果要"撤销已 commit 但未 reload 的修改"，对应是 `uci revert <config>`，但这只在还有 changes 时有效；commit 之后 changes 是空的，revert 也撤不回去。所以我们 App 的"复位"按钮也按 LuCI 原意来——**重新 GET 一遍 overview 把表单恢复到磁盘里的值**，不要瞎搞 `uci revert`。

---

## 7. 写探针时的常见坑

1. **路径**：探针在 `src-tauri/examples/` 下，运行必须 `cd src-tauri`（cargo 找不到外层的 Cargo.toml）。
2. **多行脚本里的换行**：用 raw string `r#"..."#`，避免转义 `\n`；脚本内部要的换行就直接换行。
3. **stdout 中文乱码**：路由器侧很多 lua 源文件是 UTF-8 中文注释，我们用 latin1 推断可能显示乱码（探针里看到的 `鑺傜偣` 这种）。这不影响业务正确性，要看注释就 `xxd` 或 `iconv` 一下。
4. **busybox 工具集有限**：`timeout` 不存在；`date +%s%N` 在某些版本会返回带 `N` 的字面值；`awk` 是 mawk/busybox awk，没有 gnu 扩展。优先用 `expr / sh 语法 / sed -n`。
5. **uci show 一次拉很多**：grep/head 截一下，免得回包巨大触发我们前端 60s timeout。

---

## 8. 探针 → 生产代码 的接力

- 探针确认某个 HTTP/ubus 端点行为后，**生产代码尽量直接调那个端点**，少自己 sh 拼装。如 §5.2 的三个测试端点，我们就直接 `client.http.get(...)`。
- 字段名以探针拿到的为准，写一段 comment 标注探针来源：

```rust
// 字段名按 /usr/lib/lua/luci/model/cbi/passwall/client/node_subscribe.lua:152
// 实测 uci show passwall.@subscribe_list[0] 见 examples/probe_save_apply.rs 输出
let patch = json!({ "remark": new_name });
```

这样后人维护时一眼能知道事实来源，而不是去猜。
