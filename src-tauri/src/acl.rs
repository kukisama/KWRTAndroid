//! PassWall ACL（按源 IP/MAC 决定哪些设备走哪个节点）
//!
//! 事实依据（实地探测 + 阅读 /usr/lib/lua/luci/model/cbi/passwall/client/acl_config.lua）：
//!   - 段类型：`config acl_rule`（匿名 section）
//!   - 全局总开关：`passwall.@global[0].acl_enable`
//!   - 字段（按官方 CBI 顺序）：
//!       enabled              Flag,  默认 1
//!       remarks              Value, 备注
//!       interface            Value, 源接口（设备名，如 br-lan），"" = All
//!       sources              Value, 空格分隔；IP / CIDR / IP 范围 / MAC / ipset:NAME
//!       tcp_no_redir_ports   "" / "disable" / "1:65535" / 端口列表
//!       udp_no_redir_ports   同上
//!       tcp_node             节点 section id 或 "nil"
//!       udp_node             节点 section id / "nil" / "tcp"（跟随 TCP）
//!
//! 与 rule_list 的关系：ACL 选"哪台机用哪个节点"；rule_list（/usr/share/passwall/rules/）
//! 决定"目标域名/IP 走代理/直连/屏蔽"。两者并行：先匹 ACL（按源）选节点，再匹 rule_list（按目标）。
//! 配置上互不调用，但共享同一份 chnlist/gfwlist 数据。
//!
//! 写入策略：uci set + commit + **后台** reload。
//!   `/etc/init.d/passwall reload` 可能跑 10–60 秒，ubus file.exec 同步等会触发 60s 超时
//!   （uci 其实早写成功）。所以 commit 完立刻返回，reload 用 setsid + 后台 fork 异步执行。

use crate::client::LuciClient;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    #[serde(default)]
    pub section: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub remarks: String,
    /// 源接口（kernel 设备名，如 br-lan）；"" = 全部
    #[serde(default)]
    pub interface: String,
    /// 空格分隔的源标识：MAC / IP / CIDR / IP-Range / ipset:NAME
    #[serde(default)]
    pub sources: String,
    /// 节点 section id；"nil" = 不使用（直连）
    #[serde(default = "default_nil")]
    pub tcp_node: String,
    /// 节点 section id；"nil" = 不使用；"tcp" = 跟随 TCP
    #[serde(default = "default_nil")]
    pub udp_node: String,
    /// TCP 不转发端口：""=全局 / "disable" / "1:65535" / "80,443" 等
    #[serde(default)]
    pub tcp_no_redir_ports: String,
    #[serde(default)]
    pub udp_no_redir_ports: String,

    // “使用 5 个全局列表”开关（来自 LuCI acl_config.lua）：
    //   use_direct_list / use_proxy_list / use_block_list / use_gfw_list （Flag, LuCI 默认 1）
    //   chn_list （ListValue: "0" 关 / "direct" 直连 / "proxy" 走代理; LuCI 默认 "direct"）
    // 本应用面向 NAS-bypass 场景，新增默认全部关闭（前端给 draft 赋 false / "0"）。
    // 读取时若 UCI 上该字段不存在则视为 LuCI 默认（1 / "direct"），保持与 LuCI 督受一致。
    #[serde(default = "default_true")]
    pub use_direct_list: bool,
    #[serde(default = "default_true")]
    pub use_proxy_list: bool,
    #[serde(default = "default_true")]
    pub use_block_list: bool,
    #[serde(default = "default_true")]
    pub use_gfw_list: bool,
    #[serde(default = "default_chn_list")]
    pub chn_list: String,
}
fn default_true() -> bool { true }
fn default_nil() -> String { "nil".to_string() }
fn default_chn_list() -> String { "direct".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclSnapshot {
    pub acl_enable: bool,
    pub rules: Vec<AclRule>,
}

pub async fn read(client: &LuciClient, config: &str) -> Result<AclSnapshot> {
    let (code, out, err) = client.shell(&format!("uci show {config}")).await?;
    if code != 0 {
        return Err(anyhow!("uci show {config} 失败 (exit={code}) stderr={err}"));
    }

    use std::collections::BTreeMap;
    let mut secs: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let prefix = format!("{config}.");
    for line in out.lines() {
        let l = line.trim();
        let body = match l.strip_prefix(&prefix) { Some(b) => b, None => continue };
        if let Some(idx) = body.find('=') {
            let left = &body[..idx];
            let right = body[idx + 1..].trim_matches('\'').to_string();
            if let Some(dot) = left.find('.') {
                let sec = left[..dot].to_string();
                let key = left[dot + 1..].to_string();
                secs.entry(sec).or_default().insert(key, right);
            } else {
                let sec = left.to_string();
                secs.entry(sec).or_default().insert(".type".into(), right);
            }
        }
    }

    let mut acl_enable = false;
    let mut rules: Vec<AclRule> = Vec::new();
    for (name, fields) in secs {
        match fields.get(".type").map(String::as_str) {
            Some("global") => {
                acl_enable = fields.get("acl_enable").map(|v| v == "1" || v == "true").unwrap_or(false);
            }
            Some("acl_rule") => {
                rules.push(AclRule {
                    section: name,
                    enabled: fields.get("enabled").map(|v| v == "1" || v == "true").unwrap_or(true),
                    remarks: fields.get("remarks").cloned().unwrap_or_default(),
                    interface: fields.get("interface").cloned().unwrap_or_default(),
                    sources: fields.get("sources").cloned().unwrap_or_default(),
                    tcp_node: fields.get("tcp_node").cloned().unwrap_or_else(|| "nil".into()),
                    udp_node: fields.get("udp_node").cloned().unwrap_or_else(|| "nil".into()),
                    tcp_no_redir_ports: fields.get("tcp_no_redir_ports").cloned().unwrap_or_default(),
                    udp_no_redir_ports: fields.get("udp_no_redir_ports").cloned().unwrap_or_default(),
                    use_direct_list: fields.get("use_direct_list").map(|v| v == "1" || v == "true").unwrap_or(true),
                    use_proxy_list:  fields.get("use_proxy_list").map(|v| v == "1" || v == "true").unwrap_or(true),
                    use_block_list:  fields.get("use_block_list").map(|v| v == "1" || v == "true").unwrap_or(true),
                    use_gfw_list:    fields.get("use_gfw_list").map(|v| v == "1" || v == "true").unwrap_or(true),
                    chn_list: fields.get("chn_list").cloned().unwrap_or_else(|| "direct".into()),
                });
            }
            _ => {}
        }
    }
    Ok(AclSnapshot { acl_enable, rules })
}

pub async fn set_global_enable(client: &LuciClient, config: &str, on: bool) -> Result<()> {
    let val = if on { "1" } else { "0" };
    // 仅 commit，不 reload；由前端"应用配置"按钮统一触发 reload，避免每次都等 10-60s。
    let script = format!(
        "uci set {c}.@global[0].acl_enable='{v}' && uci commit {c} && echo OK",
        c = config, v = val
    );
    let (code, out, err) = client.shell(&script).await?;
    if code != 0 || !out.contains("OK") {
        return Err(anyhow!("set acl_enable 失败 (exit={code}) stderr={err}"));
    }
    Ok(())
}

pub async fn add(client: &LuciClient, config: &str, rule: &AclRule) -> Result<String> {
    validate(rule)?;
    let mut sets = String::new();
    push_set(&mut sets, config, "${sec}", "enabled", if rule.enabled { "1" } else { "0" });
    push_set(&mut sets, config, "${sec}", "remarks", &rule.remarks);
    push_set(&mut sets, config, "${sec}", "interface", &rule.interface);
    push_set(&mut sets, config, "${sec}", "sources", &rule.sources);
    push_set(&mut sets, config, "${sec}", "tcp_node", &rule.tcp_node);
    push_set(&mut sets, config, "${sec}", "udp_node", &rule.udp_node);
    push_set(&mut sets, config, "${sec}", "tcp_no_redir_ports", &rule.tcp_no_redir_ports);
    push_set(&mut sets, config, "${sec}", "udp_no_redir_ports", &rule.udp_no_redir_ports);
    push_set(&mut sets, config, "${sec}", "use_direct_list", if rule.use_direct_list { "1" } else { "0" });
    push_set(&mut sets, config, "${sec}", "use_proxy_list",  if rule.use_proxy_list  { "1" } else { "0" });
    push_set(&mut sets, config, "${sec}", "use_block_list",  if rule.use_block_list  { "1" } else { "0" });
    push_set(&mut sets, config, "${sec}", "use_gfw_list",    if rule.use_gfw_list    { "1" } else { "0" });
    push_set(&mut sets, config, "${sec}", "chn_list", &rule.chn_list);

    let script = format!(
        "set -e\nsec=$(uci add {c} acl_rule)\n{sets}uci commit {c}\necho \"SEC=${{sec}}\"\n",
        c = config, sets = sets
    );
    let (code, out, err) = client.shell(&script).await?;
    if code != 0 {
        return Err(anyhow!("add acl 失败 (exit={code}) stderr={err} stdout={out}"));
    }
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("SEC=") {
            let s = rest.trim();
            if !s.is_empty() { return Ok(s.to_string()); }
        }
    }
    Err(anyhow!("add acl 返回 section 未识别: {out}"))
}

pub async fn update(client: &LuciClient, config: &str, rule: &AclRule) -> Result<()> {
    if rule.section.is_empty() {
        return Err(anyhow!("update acl: section 名为空"));
    }
    validate(rule)?;
    let mut sets = String::new();
    push_set(&mut sets, config, &rule.section, "enabled", if rule.enabled { "1" } else { "0" });
    push_set(&mut sets, config, &rule.section, "remarks", &rule.remarks);
    push_set(&mut sets, config, &rule.section, "interface", &rule.interface);
    push_set(&mut sets, config, &rule.section, "sources", &rule.sources);
    push_set(&mut sets, config, &rule.section, "tcp_node", &rule.tcp_node);
    push_set(&mut sets, config, &rule.section, "udp_node", &rule.udp_node);
    push_set(&mut sets, config, &rule.section, "tcp_no_redir_ports", &rule.tcp_no_redir_ports);
    push_set(&mut sets, config, &rule.section, "udp_no_redir_ports", &rule.udp_no_redir_ports);
    push_set(&mut sets, config, &rule.section, "use_direct_list", if rule.use_direct_list { "1" } else { "0" });
    push_set(&mut sets, config, &rule.section, "use_proxy_list",  if rule.use_proxy_list  { "1" } else { "0" });
    push_set(&mut sets, config, &rule.section, "use_block_list",  if rule.use_block_list  { "1" } else { "0" });
    push_set(&mut sets, config, &rule.section, "use_gfw_list",    if rule.use_gfw_list    { "1" } else { "0" });
    push_set(&mut sets, config, &rule.section, "chn_list", &rule.chn_list);

    let script = format!(
        "set -e\n{sets}uci commit {c}\necho OK\n",
        c = config, sets = sets
    );
    let (code, out, err) = client.shell(&script).await?;
    if code != 0 || !out.contains("OK") {
        return Err(anyhow!("update acl 失败 (exit={code}) stderr={err} stdout={out}"));
    }
    Ok(())
}

pub async fn delete(client: &LuciClient, config: &str, section: &str) -> Result<()> {
    if section.is_empty() {
        return Err(anyhow!("delete acl: section 名为空"));
    }
    let script = format!(
        "uci delete {c}.{s} && uci commit {c} && echo OK",
        c = config, s = section
    );
    let (code, out, err) = client.shell(&script).await?;
    if code != 0 || !out.contains("OK") {
        return Err(anyhow!("delete acl 失败 (exit={code}) stderr={err}"));
    }
    Ok(())
}

/// 显式触发 reload（前端"应用配置"按钮）。后台 fork 异步执行，立即返回。
pub async fn reload(client: &LuciClient, config: &str) -> Result<()> {
    // 用 setsid + & 让 reload 完全脱离父进程，避免 ubus file.exec 60s 超时。
    let script = format!(
        "( setsid /etc/init.d/{c} reload </dev/null >/dev/null 2>&1 & ) >/dev/null 2>&1 && echo OK",
        c = config
    );
    let (code, out, err) = client.shell(&script).await?;
    if code != 0 || !out.contains("OK") {
        return Err(anyhow!("reload {config} 失败 (exit={code}) stderr={err}"));
    }
    Ok(())
}

#[allow(dead_code)]
fn _bg_reload_unused(config: &str) -> String {
    format!(
        "( setsid /etc/init.d/{c} reload </dev/null >/dev/null 2>&1 & ) >/dev/null 2>&1",
        c = config
    )
}

fn push_set(buf: &mut String, config: &str, sec: &str, key: &str, val: &str) {
    if val.is_empty() {
        // 显式清空：避免编辑时旧值残留
        buf.push_str(&format!(
            "uci -q delete {c}.{s}.{k} 2>/dev/null || true\n",
            c = config, s = sec, k = key
        ));
        return;
    }
    buf.push_str(&format!("uci set {c}.{s}.{k}='{v}'\n", c = config, s = sec, k = key, v = val));
}

fn validate(rule: &AclRule) -> Result<()> {
    if rule.sources.trim().is_empty() {
        return Err(anyhow!("源地址不能为空（填 MAC / IP / CIDR / IP 段，多个用空格分隔）"));
    }
    for f in [
        &rule.remarks, &rule.sources, &rule.tcp_node, &rule.udp_node,
        &rule.interface, &rule.tcp_no_redir_ports, &rule.udp_no_redir_ports,
        &rule.chn_list,
    ] {
        if f.contains('\'') {
            return Err(anyhow!("字段不允许包含单引号 '"));
        }
    }
    Ok(())
}
