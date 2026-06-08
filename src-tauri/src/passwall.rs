//! PassWall 高层操作：基于 uci_get/uci_set/uci_commit/rc init 的业务封装。
//! 之所以单列一个模块，是因为 lib.rs 只负责把 Tauri command 转给这里，
//! 真正的业务/数据组装都收敛在这里，方便单元定位。

use crate::client::LuciClient;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// PassWall 顶层概览 —— 一次调用拿到所有 UI 需要展示的核心数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Overview {
    /// 主配置名（passwall 或 passwall2）
    pub config: String,
    pub init_name: String,

    /// 主开关 / Socks / 当前 tcp/udp 节点等
    pub global: GlobalSummary,

    /// 节点：[{name, remarks, protocol, address, port, type, ...}]
    pub nodes: Vec<Map<String, Value>>,
    /// 分流规则
    pub shunt_rules: Vec<Map<String, Value>>,
    /// 订阅
    pub subscribes: Vec<Map<String, Value>>,

    /// 其它一组 global_* section，原样返回，前端按 type 渲染。
    /// key = section_type（global_forwarding / global_xray / ...），value = 原始 section。
    pub globals: Map<String, Value>,

    /// 原始 full dump，前端"原始配置"tab 直接展示。
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalSummary {
    pub enabled: bool,
    pub socks_enabled: bool,
    pub tcp_node: Option<String>,
    pub udp_node: Option<String>,
    pub tcp_node_socks_port: Option<String>,
    pub dns_mode: Option<String>,
    pub remote_dns: Option<String>,
    pub dns_shunt: Option<String>,
    pub filter_proxy_ipv6: Option<String>,
    /// global section 的真实 .name（写 uci_set 用）
    pub section_name: Option<String>,
}

pub async fn overview(client: &LuciClient, config: &str) -> Result<Overview> {
    let raw = client.uci_get(config, None, None).await?;
    let values = raw
        .get("values")
        .and_then(|v| v.as_object())
        .cloned()
        .ok_or_else(|| anyhow!("uci.get {config} 缺少 values 字段"))?;

    let mut nodes = vec![];
    let mut shunt_rules = vec![];
    let mut subscribes = vec![];
    let mut globals: Map<String, Value> = Map::new();
    let mut global_summary = GlobalSummary::default();

    for (_name, sec_val) in values.iter() {
        let sec = match sec_val.as_object() {
            Some(o) => o.clone(),
            None => continue,
        };
        let ty = sec
            .get(".type")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        match ty.as_str() {
            "nodes" => nodes.push(sec),
            "shunt_rules" => shunt_rules.push(sec),
            "subscribe_list" => subscribes.push(sec),
            "global" => {
                let s = &sec;
                global_summary.section_name = s
                    .get(".name")
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string());
                global_summary.enabled = s.get("enabled").and_then(|x| x.as_str()).unwrap_or("0")
                    == "1";
                global_summary.socks_enabled = s
                    .get("socks_enabled")
                    .and_then(|x| x.as_str())
                    .unwrap_or("0")
                    == "1";
                global_summary.tcp_node = s
                    .get("tcp_node")
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string());
                global_summary.udp_node = s
                    .get("udp_node")
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string());
                global_summary.tcp_node_socks_port = s
                    .get("tcp_node_socks_port")
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string());
                global_summary.dns_mode = s
                    .get("dns_mode")
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string());
                global_summary.remote_dns = s
                    .get("remote_dns")
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string());
                global_summary.dns_shunt = s
                    .get("dns_shunt")
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string());
                global_summary.filter_proxy_ipv6 = s
                    .get("filter_proxy_ipv6")
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string());
                // 同时把 global 本身放进 globals 里方便前端"raw 之外的总览"展示
                globals.insert("global".into(), Value::Object(sec));
            }
            other if other.starts_with("global_") => {
                globals.insert(other.to_string(), Value::Object(sec));
            }
            _ => {
                // 其它 type 放 globals.other
                let key = format!("other:{}", ty);
                globals
                    .entry(key)
                    .or_insert_with(|| Value::Array(vec![]))
                    .as_array_mut()
                    .unwrap()
                    .push(Value::Object(sec));
            }
        }
    }

    // 按 .index 排序，UI 稳定
    nodes.sort_by_key(|s| s.get(".index").and_then(|x| x.as_i64()).unwrap_or(0));
    shunt_rules.sort_by_key(|s| s.get(".index").and_then(|x| x.as_i64()).unwrap_or(0));
    subscribes.sort_by_key(|s| s.get(".index").and_then(|x| x.as_i64()).unwrap_or(0));

    Ok(Overview {
        config: config.to_string(),
        init_name: config.to_string(), // passwall / passwall2 同名 init
        global: global_summary,
        nodes,
        shunt_rules,
        subscribes,
        globals,
        raw,
    })
}

fn global_section(o: &Overview) -> Result<&str> {
    o.global
        .section_name
        .as_deref()
        .ok_or_else(|| anyhow!("找不到 global section 名称"))
}

/// 把任意字符串安全嵌进单引号包裹的 shell 字面量。'  ->  '\''。
fn sh_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// 把 patch 里的字段（必须是 Object）转成一组 `uci set config.section.opt='val'` 语句。
fn patch_to_uci_set(config: &str, section: &str, patch: &Value) -> Result<Vec<String>> {
    let map = patch
        .as_object()
        .ok_or_else(|| anyhow!("patch 必须是 JSON 对象，收到：{patch}"))?;
    let mut out = Vec::with_capacity(map.len());
    for (k, v) in map {
        // 简单字段名校验：只允许 [A-Za-z0-9_]
        if !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(anyhow!("非法字段名：{k}"));
        }
        let val_str = match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => (if *b { "1" } else { "0" }).to_string(),
            Value::Null => String::new(),
            // list 类型 → 先 delete 再 add_list
            Value::Array(arr) => {
                let mut cmds = vec![format!(
                    "uci -q delete {}.{}.{}",
                    config, section, k
                )];
                for item in arr {
                    let s = item.as_str().map(|x| x.to_string()).unwrap_or_else(|| item.to_string());
                    cmds.push(format!(
                        "uci add_list {}.{}.{}={}",
                        config, section, k, sh_quote(&s)
                    ));
                }
                out.extend(cmds);
                continue;
            }
            other => other.to_string(),
        };
        out.push(format!(
            "uci -q set {}.{}.{}={}",
            config, section, k, sh_quote(&val_str)
        ));
    }
    Ok(out)
}

/// 修改若干 global 字段。`commit=true` 时立即 `uci commit`（默认）；
/// `commit=false` 时只 staging 写入 `/tmp/.uci/<config>`，等待显式 `uci_commit` 提交。
/// 三按钮模型中：保存=commit;保存并应用=commit+reload;复位=revert（丢弃 staging）。
pub async fn patch_global(
    client: &LuciClient,
    config: &str,
    patch: Value,
    commit: bool,
) -> Result<()> {
    let ov = overview(client, config).await?;
    let section = global_section(&ov)?.to_string();
    let mut cmds = patch_to_uci_set(config, &section, &patch)?;
    if commit { cmds.push(format!("uci commit {config}")); }
    let script = cmds.join(" && ");
    client.shell(&script).await?;
    Ok(())
}

/// 修改一个非 global 的命名 section 的若干字段。
pub async fn patch_section(
    client: &LuciClient,
    config: &str,
    section: &str,
    patch: Value,
    commit: bool,
) -> Result<()> {
    let mut cmds = patch_to_uci_set(config, section, &patch)?;
    if commit { cmds.push(format!("uci commit {config}")); }
    let script = cmds.join(" && ");
    client.shell(&script).await?;
    Ok(())
}

/// 删除一个 section（用于删节点/订阅/分流规则）。
pub async fn delete_section(
    client: &LuciClient,
    config: &str,
    section: &str,
    commit: bool,
) -> Result<()> {
    let script = if commit {
        format!("uci -q delete {config}.{section} && uci commit {config}")
    } else {
        format!("uci -q delete {config}.{section}")
    };
    client.shell(&script).await?;
    Ok(())
}

/// 新增一个 section。type 一般是 nodes/shunt_rules/subscribe_list。
/// 成功后返回新 section 名（uci add 的输出就是新名）。
pub async fn add_section(
    client: &LuciClient,
    config: &str,
    section_type: &str,
    values: Value,
    commit: bool,
) -> Result<String> {
    // 1) uci add config type  ->  stdout 里是新 section 名
    let add_script = format!("uci add {config} {section_type}");
    let (_c, stdout, _e) = client.shell(&add_script).await?;
    let new_name = stdout.trim().to_string();
    if new_name.is_empty() {
        return Err(anyhow!("uci add 没返回新 section 名"));
    }
    // 2) 写入字段
    let mut cmds = patch_to_uci_set(config, &new_name, &values)?;
    if commit { cmds.push(format!("uci commit {config}")); }
    let script = cmds.join(" && ");
    client.shell(&script).await?;
    Ok(new_name)
}

/// 三按钮模型：仅 commit 当前 staging（保存）。无 reload。
pub async fn uci_commit(client: &LuciClient, config: &str) -> Result<()> {
    let script = format!("uci -q commit {config}");
    client.shell(&script).await?;
    Ok(())
}

/// 三按钮模型：丢弃当前 staging（复位）。等价 LuCI footer.htm 里 `cbi-button-reset`
/// 的浏览器刷新——只清掉 `/tmp/.uci/<config>` 里未提交的修改，不动 `/etc/config/<config>`，
/// 不动正在跑的服务。
pub async fn uci_revert(client: &LuciClient, config: &str) -> Result<()> {
    let script = format!("uci -q revert {config}");
    client.shell(&script).await?;
    Ok(())
}

/// 返回当前 `uci changes <config>` 的原始行（每行一条暂存变更）。
/// 空数组 = 没有未应用的 staging。
pub async fn uci_changes(client: &LuciClient, config: &str) -> Result<Vec<String>> {
    let (_c, stdout, _e) = client.shell(&format!("uci changes {config}")).await?;
    Ok(stdout.lines().filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect())
}

/// 异步重启/重载某个服务。返回结果**仅供日志**，前端调用应该 fire-and-forget。
/// 用极短超时去 fire init.d reload；ubus 拿不到 EOF 时也不阻塞 UI（最多等 2 秒）。
pub async fn reload_config(client: &LuciClient, config: &str) -> Result<()> {
    // setsid + 完全分离 IO 描述符 + 父进程立即 exit。即便 file.exec 仍 wait 子 sh，
    // 子 sh 在启动 setsid 后毫秒级退出，rpcd 这一侧通常 <100 ms 返回。
    let script = format!(
        "setsid sh -c '/etc/init.d/{config} reload >/dev/null 2>&1 </dev/null' >/dev/null 2>&1 </dev/null &\nexit 0\n"
    );
    // 给独立的 2 秒超时——即便 ubus 真卡住也最多损失 2 秒，绝不会把 reqwest 连接池占满。
    let fut = client.shell(&script);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), fut).await;
    Ok(())
}

/// 读 PassWall 日志（容错：file.exec 不一定可用，回退到读已知日志文件）。
pub async fn read_log(client: &LuciClient, config: &str, lines: usize) -> Result<String> {
    let candidates = [
        format!("/tmp/log/{config}.log"),
        format!("/var/log/{config}.log"),
    ];
    for p in &candidates {
        if let Ok(s) = client.file_read(p).await {
            if !s.is_empty() {
                let v: Vec<&str> = s.lines().collect();
                let take = v.len().saturating_sub(lines);
                return Ok(v[take..].join("\n"));
            }
        }
    }
    // logread -e <config>
    match client
        .file_exec("/sbin/logread", vec!["-e", config])
        .await
    {
        Ok(v) => Ok(v
            .get("stdout")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()),
        Err(e) => Err(anyhow!("无法获取日志：{e}")),
    }
}

/// 备份完整配置文件。
pub async fn backup(client: &LuciClient, config: &str) -> Result<String> {
    let path = format!("/etc/config/{config}");
    client.file_read(&path).await
}

/// 还原配置文件（覆盖写）。不做 commit（uci 文件是文本，重启 init 即生效）。
pub async fn restore(client: &LuciClient, config: &str, content: &str) -> Result<()> {
    let path = format!("/etc/config/{config}");
    client.file_write(&path, content).await?;
    let _ = client.rc_init(config, "reload").await;
    Ok(())
}

/// 触发订阅更新：尝试 file.exec 调 PassWall 的订阅脚本。
pub async fn update_subscribe(client: &LuciClient, config: &str, section: &str) -> Result<String> {
    // PassWall v1 的脚本路径
    let script = format!("/usr/share/{config}/subscribe.lua");
    match client
        .file_exec("/usr/bin/lua", vec![&script, "start", section])
        .await
    {
        Ok(v) => Ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
        Err(e) => Err(anyhow!(
            "调用 {script} 失败（file.exec 可能被 ACL 禁用）：{e}"
        )),
    }
}

/// 触发规则更新：调用 PassWall 自带规则更新脚本。
pub async fn update_rules(client: &LuciClient, config: &str) -> Result<String> {
    let script = format!("/usr/share/{config}/rule_update.lua");
    match client
        .file_exec("/usr/bin/lua", vec![&script, "log"])
        .await
    {
        Ok(v) => Ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
        Err(e) => Err(anyhow!("调用 {script} 失败：{e}")),
    }
}

/// === 节点测试：直接复用 LuCI controller 的 JSON 端点 ===
/// 路径：/cgi-bin/luci/admin/services/passwall/{ping_node,urltest_node}
/// 已登录的 LuciClient.http 自带 sysauth cookie，可直接 GET。

/// 共用：调 ping_node 端点。type = "icmp" | "tcping"。
/// 返回 { ok, latency_ms, kind, summary, raw }
async fn http_ping(client: &LuciClient, kind: &str, address: &str, port: u16) -> Result<Value> {
    let path = format!(
        "cgi-bin/luci/admin/services/passwall/ping_node?type={}&address={}&port={}",
        kind,
        urlencoding(address),
        port
    );
    let url = client.base_url.join(&path)?;
    let resp = client.http.get(url.clone()).send().await
        .with_context(|| format!("调用 {} 失败", url))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let ping_str = v.get("ping").and_then(|x| x.as_str()).unwrap_or("").trim();
    let latency_ms: Option<f64> = ping_str.parse().ok().filter(|x: &f64| *x > 0.0);
    let ok = latency_ms.is_some();
    let summary = if ok {
        format!("{:.0} ms", latency_ms.unwrap())
    } else if !status.is_success() {
        format!("HTTP {}", status.as_u16())
    } else {
        "超时".into()
    };
    Ok(json!({
        "ok": ok, "latency_ms": latency_ms, "kind": kind,
        "summary": summary, "raw": text
    }))
}

/// ICMP Ping。
pub async fn test_icmp(client: &LuciClient, address: &str, port: u16) -> Result<Value> {
    http_ping(client, "icmp", address, port).await
}

/// TCPing：路由器有 tcping 命令，握手延迟单位 ms。
pub async fn test_tcping(client: &LuciClient, address: &str, port: u16) -> Result<Value> {
    http_ping(client, "tcping", address, port).await
}

/// URL 真代理测试：调 LuCI urltest_node 端点。
/// 路由器侧会用 uci `global_other.url_test_url`（默认 google generate_204）走该节点的 socks5 出口，
/// 返回 use_time（ms）。我们只关心 section id。
pub async fn test_url(client: &LuciClient, section: &str, _url_unused: &str) -> Result<Value> {
    let path = format!(
        "cgi-bin/luci/admin/services/passwall/urltest_node?id={}",
        urlencoding(section)
    );
    let url = client.base_url.join(&path)?;
    let resp = client.http.get(url.clone()).send().await
        .with_context(|| format!("调用 {} 失败", url))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let ms_str = v.get("use_time").and_then(|x| x.as_str()).unwrap_or("").trim();
    let latency_ms: Option<f64> = ms_str.parse().ok().filter(|x: &f64| *x > 0.0);
    let ok = latency_ms.is_some();
    let summary = if ok {
        format!("{:.0} ms", latency_ms.unwrap())
    } else if !status.is_success() {
        format!("HTTP {}", status.as_u16())
    } else {
        "失败".into()
    };
    Ok(json!({
        "ok": ok, "latency_ms": latency_ms, "supported": true,
        "summary": summary, "raw": text
    }))
}

/// 旧入口：兼容当前前端"测连通"按钮——内部用 TCPing。
pub async fn ping_node(client: &LuciClient, address: &str, port: u16) -> Result<Value> {
    let tcp = test_tcping(client, address, port).await?;
    Ok(json!({
        "ok": tcp["ok"],
        "method": "tcp",
        "latency_ms": tcp["latency_ms"],
        "summary": format!("TCP {}:{}  {}", address, port, tcp["summary"].as_str().unwrap_or("")),
        "raw": tcp
    }))
}

/// 最小 percent-encode：把非保留字符外的字符转义；address/section 用得到。
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}



/// 抓 PassWall 相关二进制的版本与路径。只读，不做更新动作。
pub async fn components_info(client: &LuciClient, config: &str) -> Result<Value> {
    // 从 uci 取路径，缺失就给默认。
    let bins = [
        ("xray",      "xray_file",      "/usr/bin/xray",       "version 2>/dev/null | head -1"),
        ("sing-box",  "singbox_file",   "/usr/bin/sing-box",   "version 2>/dev/null | head -1"),
        ("hysteria",  "hysteria_file",  "/usr/bin/hysteria",   "version 2>/dev/null | head -1"),
        ("geoview",   "geoview_file",   "/usr/bin/geoview",    "-v 2>/dev/null | head -1"),
        ("chinadns",  "chinadns_ng_file", "/usr/bin/chinadns-ng", "-V 2>/dev/null | head -1"),
    ];
    // 一次 shell 拉全部，省 RTT
    let mut script = String::new();
    script.push_str(&format!(
        "echo PASSWALL_VER=$(opkg status luci-app-passwall 2>/dev/null | awk -F': ' '/^Version/{{print $2; exit}}')\n"
    ));
    for (key, _, def, _) in &bins {
        // 读 uci，没有就 def
        script.push_str(&format!(
            "p=$(uci -q get {cfg}.@global[0].{ukey} 2>/dev/null); [ -z \"$p\" ] && p={def}; echo PATH_{k}=$p\n",
            cfg = config, ukey = bins.iter().find(|x| x.0 == *key).unwrap().1, def = def, k = key
        ));
    }
    for (key, _, _, vcmd) in &bins {
        script.push_str(&format!(
            "p=$(uci -q get {cfg}.@global[0].{ukey} 2>/dev/null); [ -z \"$p\" ] && p={def}; \
             if [ -x \"$p\" ]; then v=$(\"$p\" {vcmd}); else v='(not installed)'; fi; \
             sz=$(ls -l \"$p\" 2>/dev/null | awk '{{print $5}}'); \
             echo VER_{k}=$v; echo SIZE_{k}=$sz\n",
            cfg = config, ukey = bins.iter().find(|x| x.0 == *key).unwrap().1,
            def = bins.iter().find(|x| x.0 == *key).unwrap().2,
            vcmd = vcmd, k = key
        ));
    }
    let (_c, stdout, _e) = client.shell(&script).await?;

    let mut passwall_ver = String::new();
    let mut map: std::collections::HashMap<String, serde_json::Map<String, Value>> = Default::default();
    for (k, _, _, _) in &bins { map.insert((*k).into(), Default::default()); }

    for line in stdout.lines() {
        if let Some(v) = line.strip_prefix("PASSWALL_VER=") { passwall_ver = v.trim().to_string(); continue; }
        for prefix in ["PATH_", "VER_", "SIZE_"] {
            if let Some(rest) = line.strip_prefix(prefix) {
                if let Some(eq) = rest.find('=') {
                    let k = &rest[..eq]; let v = rest[eq+1..].trim().to_string();
                    if let Some(m) = map.get_mut(k) {
                        let field = match prefix { "PATH_" => "path", "VER_" => "version", _ => "size_bytes" };
                        if field == "size_bytes" {
                            m.insert(field.into(), v.parse::<u64>().map(Value::from).unwrap_or(Value::Null));
                        } else {
                            m.insert(field.into(), Value::String(v));
                        }
                    }
                }
            }
        }
    }
    let comps: serde_json::Map<String, Value> = map.into_iter()
        .map(|(k, v)| (k, Value::Object(v)))
        .collect();
    Ok(json!({
        "passwall_version": passwall_ver,
        "components": comps,
        "raw": stdout
    }))
}
