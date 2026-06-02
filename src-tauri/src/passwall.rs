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

/// 修改若干 global 字段并 commit + reload。
pub async fn patch_global(client: &LuciClient, config: &str, patch: Value) -> Result<()> {
    let ov = overview(client, config).await?;
    let section = global_section(&ov)?.to_string();
    client.uci_set(config, &section, patch).await?;
    client.uci_commit(config).await?;
    // reload 比 restart 更轻
    let _ = client.rc_init(config, "reload").await;
    Ok(())
}

/// 修改一个非 global 的命名 section 的若干字段。
pub async fn patch_section(
    client: &LuciClient,
    config: &str,
    section: &str,
    patch: Value,
    reload: bool,
) -> Result<()> {
    client.uci_set(config, section, patch).await?;
    client.uci_commit(config).await?;
    if reload {
        let _ = client.rc_init(config, "reload").await;
    }
    Ok(())
}

/// 删除一个 section（用于删节点/订阅/分流规则）。
pub async fn delete_section(
    client: &LuciClient,
    config: &str,
    section: &str,
    reload: bool,
) -> Result<()> {
    client.uci_delete(config, section, None).await?;
    client.uci_commit(config).await?;
    if reload {
        let _ = client.rc_init(config, "reload").await;
    }
    Ok(())
}

/// 新增一个 section。type 一般是 nodes/shunt_rules/subscribe_list。
/// 成功后返回新 section 名（uci.add 的返回字段名是 `section`）。
pub async fn add_section(
    client: &LuciClient,
    config: &str,
    section_type: &str,
    values: Value,
    reload: bool,
) -> Result<String> {
    let v = client
        .uci_add(config, section_type, None, values)
        .await
        .context("uci add 失败")?;
    let new_name = v
        .get("section")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("uci.add 响应缺少 section 字段: {v}"))?
        .to_string();
    client.uci_commit(config).await?;
    if reload {
        let _ = client.rc_init(config, "reload").await;
    }
    Ok(new_name)
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

/// 简单包装：用 ping/tcping 测试节点的 address:port 是否可达（在路由器侧测）。
pub async fn ping_node(
    client: &LuciClient,
    address: &str,
    port: u16,
) -> Result<Value> {
    // 优先 nc -zw3 host port；不行则退到 ping -c1 -W2 host
    let port_s = port.to_string();
    if let Ok(v) = client
        .file_exec("/bin/nc", vec!["-zw3", address, &port_s])
        .await
    {
        return Ok(json!({"method":"nc","result":v}));
    }
    let v = client
        .file_exec("/bin/ping", vec!["-c", "1", "-W", "2", address])
        .await?;
    Ok(json!({"method":"ping","result":v}))
}

/// 工具：在 patch 用的 Value 上把 bool 转成 "0"/"1"（UCI 都是字符串）。
pub fn bool_str(b: bool) -> &'static str {
    if b { "1" } else { "0" }
}
