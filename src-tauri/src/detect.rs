//! 登录后的 PassWall 能力探测：识别 v1/v2、确认列表文件路径、检测 init 脚本与读写权限。
use crate::client::LuciClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PassWallVariant {
    None,
    V1,
    V2,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckItem {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionReport {
    pub variant: PassWallVariant,
    /// 当 variant 为 V1/V2/Both 时，建议作为主要操作目标的 UCI 配置名（passwall 或 passwall2）。
    pub primary_config: Option<String>,
    /// 直连列表文件路径（已确认存在）。可能为 None（即使存在 PassWall 也可能用其它列表机制）。
    pub direct_ip_path: Option<String>,
    pub proxy_ip_path: Option<String>,
    pub block_ip_path: Option<String>,
    pub init_script: Option<String>,
    pub openwrt_release: Option<String>,
    pub hostname: Option<String>,
    pub checks: Vec<CheckItem>,
    pub uci_configs: Vec<String>,
    /// 直连列表当前条目数（如果能读到）。
    pub direct_ip_count: Option<usize>,
}

impl DetectionReport {
    fn add(&mut self, name: &str, ok: bool, detail: impl Into<String>) {
        self.checks.push(CheckItem {
            name: name.into(),
            ok,
            detail: detail.into(),
        });
    }
}

const V1_DIRECT_PATHS: &[&str] = &[
    "/usr/share/passwall/rules/direct_ip",
    "/usr/share/passwall/rules/direct_host",
];
const V2_DIRECT_PATHS: &[&str] = &[
    "/usr/share/passwall2/rules/direct_ip",
    "/usr/share/passwall2/rules/direct_host",
];

pub async fn run_detection(client: &LuciClient) -> Result<DetectionReport> {
    let mut report = DetectionReport {
        variant: PassWallVariant::None,
        primary_config: None,
        direct_ip_path: None,
        proxy_ip_path: None,
        block_ip_path: None,
        init_script: None,
        openwrt_release: None,
        hostname: None,
        checks: vec![],
        uci_configs: vec![],
        direct_ip_count: None,
    };

    // 1) 系统信息（顺便验证 ubus 可达）
    match client.system_board().await {
        Ok(v) => {
            report.hostname = v
                .get("hostname")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            report.openwrt_release = v
                .get("release")
                .and_then(|r| r.get("description"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            report.add(
                "system.board",
                true,
                format!(
                    "{} / {}",
                    report.hostname.clone().unwrap_or_else(|| "?".into()),
                    report
                        .openwrt_release
                        .clone()
                        .unwrap_or_else(|| "?".into())
                ),
            );
        }
        Err(e) => report.add("system.board", false, e.to_string()),
    }

    // 2) 列出 UCI 配置；用来判断有没有 passwall / passwall2
    //    注意：很多 LuCI 用户 ACL 不允许 `uci configs`，但允许 `uci get <name>`，
    //    所以这里两条腿走路：先试枚举，再对候选名逐个 `uci get` 兜底。
    let (configs, configs_err) = match client.uci_configs().await {
        Ok(v) => (v, None),
        Err(e) => (vec![], Some(e.to_string())),
    };
    report.uci_configs = configs.clone();

    let candidates = ["passwall", "passwall2"];
    let mut probe_results: Vec<(String, bool, String)> = vec![];
    let mut has_v1 = configs.iter().any(|c| c == "passwall");
    let mut has_v2 = configs.iter().any(|c| c == "passwall2");

    for name in candidates {
        // 跳过已经通过枚举确认的
        let already = (name == "passwall" && has_v1) || (name == "passwall2" && has_v2);
        if already {
            probe_results.push((name.into(), true, "uci.configs 已枚举到".into()));
            continue;
        }
        match client.uci_get(name, None, None).await {
            Ok(v) => {
                let n = v
                    .get("values")
                    .and_then(|x| x.as_object())
                    .map(|m| m.len())
                    .unwrap_or(0);
                if name == "passwall" { has_v1 = true; }
                if name == "passwall2" { has_v2 = true; }
                if !report.uci_configs.iter().any(|c| c == name) {
                    report.uci_configs.push(name.to_string());
                }
                probe_results.push((name.into(), true, format!("uci get {name} 成功，{n} 个 section")));
            }
            Err(e) => {
                probe_results.push((name.into(), false, format!("uci get {name} 失败：{e}")));
            }
        }
    }

    report.variant = match (has_v1, has_v2) {
        (true, true) => PassWallVariant::Both,
        (true, false) => PassWallVariant::V1,
        (false, true) => PassWallVariant::V2,
        _ => PassWallVariant::None,
    };
    let configs_summary = if let Some(err) = &configs_err {
        format!(
            "uci.configs 调用失败（{err}）；改用直接 uci get 探测：passwall={}, passwall2={}",
            has_v1, has_v2
        )
    } else {
        format!(
            "uci.configs 枚举 {} 份；直接探测：passwall={}, passwall2={}",
            configs.len(), has_v1, has_v2
        )
    };
    report.add("uci.configs", configs_err.is_none(), configs_summary);
    for (name, ok, detail) in probe_results {
        report.add(&format!("uci.get({name})"), ok, detail);
    }

    // 3) 选 primary：优先 v1（你截图就是 v1 风格），其次 v2
    let primary = if has_v1 {
        Some("passwall".to_string())
    } else if has_v2 {
        Some("passwall2".to_string())
    } else {
        None
    };
    report.primary_config = primary.clone();

    // 4) 读 primary 配置（首层 section 即可，确认读权限）
    if let Some(cfg) = primary.as_deref() {
        match client.uci_get(cfg, None, None).await {
            Ok(v) => {
                let n_sections = v
                    .get("values")
                    .and_then(|x| x.as_object())
                    .map(|m| m.len())
                    .unwrap_or(0);
                // 把整份 UCI 配置 dump 出来，方便规划下一步功能
                log::info!("===== FULL UCI DUMP: {cfg} (sections={n_sections}) =====");
                log::info!("{}", serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()));
                log::info!("===== END UCI DUMP: {cfg} =====");
                report.add(
                    "uci.get(primary)",
                    true,
                    format!("可读 {cfg}，共 {n_sections} 个 section"),
                );
            }
            Err(e) => report.add("uci.get(primary)", false, format!("读 {cfg} 失败：{e}")),
        }
    } else {
        report.add(
            "passwall",
            false,
            "未检测到 passwall / passwall2 UCI 配置；请确认路由器已安装 luci-app-passwall",
        );
    }

    // 5) 探测列表文件
    let (direct_paths, proxy_paths, block_paths, init_name) = match primary.as_deref() {
        Some("passwall2") => (
            V2_DIRECT_PATHS,
            &[
                "/usr/share/passwall2/rules/proxy_ip",
                "/usr/share/passwall2/rules/proxy_host",
            ][..],
            &[
                "/usr/share/passwall2/rules/block_ip",
                "/usr/share/passwall2/rules/block_host",
            ][..],
            "passwall2",
        ),
        Some("passwall") | _ => (
            V1_DIRECT_PATHS,
            &[
                "/usr/share/passwall/rules/proxy_ip",
                "/usr/share/passwall/rules/proxy_host",
            ][..],
            &[
                "/usr/share/passwall/rules/block_ip",
                "/usr/share/passwall/rules/block_host",
            ][..],
            "passwall",
        ),
    };
    report.init_script = Some(init_name.to_string());

    for p in direct_paths {
        if client.file_stat(p).await?.is_some() {
            report.direct_ip_path = Some(p.to_string());
            report.add("file.stat(direct)", true, format!("找到 {p}"));
            break;
        }
    }
    if report.direct_ip_path.is_none() {
        report.add(
            "file.stat(direct)",
            false,
            "未找到 direct_ip / direct_host 列表文件",
        );
    }

    for p in proxy_paths {
        if client.file_stat(p).await?.is_some() {
            report.proxy_ip_path = Some(p.to_string());
            break;
        }
    }
    for p in block_paths {
        if client.file_stat(p).await?.is_some() {
            report.block_ip_path = Some(p.to_string());
            break;
        }
    }

    // 6) 读直连文件，统计条目数（验证 file.read 权限 + ACL）
    if let Some(path) = report.direct_ip_path.clone() {
        match client.file_read(&path).await {
            Ok(data) => {
                let count = data.lines().filter(|l| {
                    let t = l.trim();
                    !t.is_empty() && !t.starts_with('#')
                }).count();
                report.direct_ip_count = Some(count);
                report.add("file.read(direct)", true, format!("当前 {count} 条直连规则"));
            }
            Err(e) => report.add("file.read(direct)", false, format!("读取失败：{e}")),
        }
    }

    // 7) 探测 init 脚本是否注册（ubus rc 对象 list 方法）
    match client
        .ubus_call("rc", "list", json!({ "name": init_name }))
        .await
    {
        Ok(v) => {
            let enabled = v
                .get(init_name)
                .and_then(|x: &Value| x.get("enabled"))
                .and_then(|b| b.as_bool());
            report.add(
                "rc.list",
                true,
                format!(
                    "init 脚本 {init_name} 已注册（enabled={}）",
                    enabled
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "?".into())
                ),
            );
        }
        Err(e) => report.add("rc.list", false, format!("未找到 init 脚本 {init_name}：{e}")),
    }

    Ok(report)
}
