//! 硬件 / 系统总览：通过一次 shell 调用拉回 /proc/* + ubus system info + ip addr，
//! 前端解析展示。所有字段都是 best-effort，缺失则给空字符串/0。

use crate::client::LuciClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SysInfo {
    pub hostname: String,
    pub model: String,
    pub board_name: String,
    pub openwrt_release: String,
    pub kernel: String,
    pub uptime_seconds: u64,
    pub load: [f64; 3],
    pub cpu_model: String,
    pub cpu_cores: u32,
    /// 整机 CPU 占用率 0-100；通过 /proc/stat 间隔 ~300ms 双采样计算。
    pub cpu_busy_percent: u32,
    pub mem_total_kb: u64,
    pub mem_free_kb: u64,
    pub mem_available_kb: u64,
    pub mem_buffers_kb: u64,
    pub mem_cached_kb: u64,
    /// 磁盘/分区列表：(mount, fs_type, total_kb, used_kb, avail_kb)
    pub mounts: Vec<MountInfo>,
    /// 网口列表
    pub interfaces: Vec<IfaceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub mount: String,
    pub fs: String,
    pub total_kb: u64,
    pub used_kb: u64,
    pub avail_kb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfaceInfo {
    pub name: String,
    pub state: String,
    pub mac: String,
    pub addrs: Vec<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

pub async fn collect(client: &LuciClient) -> Result<SysInfo> {
    // 拆成两条并发 ubus shell 调用，让 CPU 双采样的 0.3s sleep 跟其它 cat/df/ip 重叠跑：
    //   - cpu_script: 只做 /proc/stat 两次采样 + sleep 0.3
    //   - meta_script: hostname/model/board/release/kernel/uptime/load/cpuinfo/meminfo/df/ip/link/netdev
    // 两段都用同一套 ===KWRT-SECTION=== 分块，stdout 直接拼起来交给原 parse() 处理。
    let cpu_script = r#"
set +e
S="===KWRT-SECTION==="
echo "$S cpustat1"; head -1 /proc/stat
sleep 0.3 2>/dev/null || sleep 1
echo "$S cpustat2"; head -1 /proc/stat
echo "$S end"
"#;
    let meta_script = r#"
set +e
S="===KWRT-SECTION==="
echo "$S hostname"; uci -q get system.@system[0].hostname || cat /proc/sys/kernel/hostname
echo "$S model"; cat /tmp/sysinfo/model 2>/dev/null
echo "$S board"; cat /tmp/sysinfo/board_name 2>/dev/null
echo "$S release"; cat /etc/openwrt_release 2>/dev/null | grep -E '^DISTRIB_DESCRIPTION' | sed -E "s/DISTRIB_DESCRIPTION='?([^']*)'?/\1/"
echo "$S kernel"; uname -r
echo "$S uptime"; cat /proc/uptime
echo "$S loadavg"; cat /proc/loadavg
echo "$S cpuinfo"; cat /proc/cpuinfo
echo "$S meminfo"; cat /proc/meminfo
echo "$S df"; df -kP 2>/dev/null
echo "$S ip"; ip -o addr 2>/dev/null
echo "$S link"; ip -o link 2>/dev/null
echo "$S netdev"; cat /proc/net/dev
echo "$S end"
"#;
    // tokio::try_join! 让两条 HTTP/ubus 请求真正并发；reqwest 的连接池会按需开第二条 TCP。
    let (cpu_res, meta_res) = tokio::try_join!(
        client.shell(cpu_script),
        client.shell(meta_script),
    )?;
    let mut combined = String::with_capacity(cpu_res.1.len() + meta_res.1.len() + 2);
    combined.push_str(&cpu_res.1);
    if !combined.ends_with('\n') { combined.push('\n'); }
    combined.push_str(&meta_res.1);
    Ok(parse(&combined))
}

fn parse(text: &str) -> SysInfo {
    let mut info = SysInfo::default();
    let mut current = String::new();
    let mut buf: Vec<&str> = Vec::new();
    let mut sections: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("===KWRT-SECTION=== ") {
            // flush 上一段
            if !current.is_empty() {
                sections.insert(current.clone(), buf.join("\n"));
            }
            current = rest.trim().to_string();
            buf.clear();
        } else {
            buf.push(line);
        }
    }
    if !current.is_empty() {
        sections.insert(current, buf.join("\n"));
    }
    let get = |k: &str| sections.get(k).map(|s| s.trim().to_string()).unwrap_or_default();

    info.hostname = get("hostname");
    info.model = get("model");
    info.board_name = get("board");
    info.openwrt_release = get("release");
    info.kernel = get("kernel");
    // uptime: "12345.67 9876.54" 取第一项整数秒
    info.uptime_seconds = get("uptime")
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f as u64)
        .unwrap_or(0);
    // loadavg: "0.12 0.34 0.56 1/123 4567"
    let la: Vec<f64> = get("loadavg")
        .split_whitespace()
        .take(3)
        .filter_map(|s| s.parse().ok())
        .collect();
    if la.len() == 3 {
        info.load = [la[0], la[1], la[2]];
    }

    // cpuinfo
    let cpu = sections.get("cpuinfo").cloned().unwrap_or_default();
    let mut cores: u32 = 0;
    for line in cpu.lines() {
        if let Some(v) = line.strip_prefix("model name") {
            if info.cpu_model.is_empty() {
                info.cpu_model = v.trim_start_matches(':').trim().to_string();
            }
        } else if let Some(v) = line.strip_prefix("Hardware") {
            if info.cpu_model.is_empty() {
                info.cpu_model = v.trim_start_matches(':').trim().to_string();
            }
        } else if let Some(v) = line.strip_prefix("system type") {
            if info.cpu_model.is_empty() {
                info.cpu_model = v.trim_start_matches(':').trim().to_string();
            }
        } else if line.starts_with("processor") {
            cores += 1;
        }
    }
    info.cpu_cores = cores.max(1);

    // CPU busy% from two samples of /proc/stat first line: "cpu user nice system idle iowait irq softirq steal ..."
    info.cpu_busy_percent = parse_cpu_busy(
        sections.get("cpustat1").map(String::as_str).unwrap_or(""),
        sections.get("cpustat2").map(String::as_str).unwrap_or(""),
    );

    // meminfo
    let mem = sections.get("meminfo").cloned().unwrap_or_default();
    for line in mem.lines() {
        let (k, v) = match line.split_once(':') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => continue,
        };
        let n: u64 = v.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0);
        match k {
            "MemTotal" => info.mem_total_kb = n,
            "MemFree" => info.mem_free_kb = n,
            "MemAvailable" => info.mem_available_kb = n,
            "Buffers" => info.mem_buffers_kb = n,
            "Cached" => info.mem_cached_kb = n,
            _ => {}
        }
    }

    // df
    let df = sections.get("df").cloned().unwrap_or_default();
    for (idx, line) in df.lines().enumerate() {
        if idx == 0 { continue; } // 表头
        // FS  1K-blocks  Used  Avail  Use%  MountedOn
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 6 { continue; }
        let mount = cols[cols.len() - 1].to_string();
        // 过滤掉常见伪 FS
        if mount.starts_with("/dev") || mount.starts_with("/sys") || mount.starts_with("/proc") { continue; }
        if cols[0] == "tmpfs" && mount == "/dev" { continue; }
        let total: u64 = cols[1].parse().unwrap_or(0);
        let used: u64 = cols[2].parse().unwrap_or(0);
        let avail: u64 = cols[3].parse().unwrap_or(0);
        info.mounts.push(MountInfo {
            mount,
            fs: cols[0].to_string(),
            total_kb: total,
            used_kb: used,
            avail_kb: avail,
        });
    }

    // interfaces：ip -o link 拿 state/MAC，ip -o addr 拿 IP，/proc/net/dev 拿统计
    let mut ifmap: std::collections::BTreeMap<String, IfaceInfo> = std::collections::BTreeMap::new();
    for line in sections.get("link").cloned().unwrap_or_default().lines() {
        // "1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 ... link/loopback 00:00:00:00:00:00 brd ..."
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }
        let name = parts[1].trim_end_matches(':').to_string();
        if name.is_empty() { continue; }
        let state = if line.contains("state UP") { "up".into() }
                    else if line.contains("state DOWN") { "down".into() }
                    else if line.contains(",UP,") || line.contains("<UP,") { "up".into() }
                    else { "unknown".into() };
        let mac = parts.iter().enumerate().find_map(|(i, &p)| {
            if (p == "link/ether" || p == "link/loopback") && i + 1 < parts.len() {
                Some(parts[i + 1].to_string())
            } else { None }
        }).unwrap_or_default();
        ifmap.entry(name.clone()).or_insert(IfaceInfo {
            name, state, mac, addrs: vec![], rx_bytes: 0, tx_bytes: 0,
        });
    }
    for line in sections.get("ip").cloned().unwrap_or_default().lines() {
        // "2: eth0    inet 10.0.0.1/24 brd ... scope global eth0"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 { continue; }
        let name = parts[1].trim_end_matches(':').to_string();
        let fam = parts[2];
        let addr = parts[3].to_string();
        if fam != "inet" && fam != "inet6" { continue; }
        if let Some(ifi) = ifmap.get_mut(&name) {
            ifi.addrs.push(addr);
        } else {
            ifmap.insert(name.clone(), IfaceInfo {
                name, state: "unknown".into(), mac: String::new(),
                addrs: vec![addr], rx_bytes: 0, tx_bytes: 0,
            });
        }
    }
    for (i, line) in sections.get("netdev").cloned().unwrap_or_default().lines().enumerate() {
        if i < 2 { continue; }
        // "  eth0: 12345  ...   67890  ..."
        if let Some(idx) = line.find(':') {
            let name = line[..idx].trim().to_string();
            let nums: Vec<u64> = line[idx + 1..].split_whitespace().filter_map(|s| s.parse().ok()).collect();
            // 第 1 个=rx_bytes, 第 9 个=tx_bytes
            let rx = nums.first().copied().unwrap_or(0);
            let tx = nums.get(8).copied().unwrap_or(0);
            if let Some(ifi) = ifmap.get_mut(&name) {
                ifi.rx_bytes = rx;
                ifi.tx_bytes = tx;
            }
        }
    }
    info.interfaces = ifmap.into_values().collect();
    info
}

/// 从两次 /proc/stat 首行采样算 CPU busy%。
/// 行格式：`cpu user nice system idle iowait irq softirq steal guest guest_nice`
fn parse_cpu_busy(a: &str, b: &str) -> u32 {
    fn parse(s: &str) -> Option<(u64, u64)> {
        let line = s.lines().next()?.trim();
        let rest = line.strip_prefix("cpu")?.trim();
        let nums: Vec<u64> = rest.split_whitespace().filter_map(|t| t.parse().ok()).collect();
        if nums.len() < 4 { return None; }
        let idle = nums[3] + nums.get(4).copied().unwrap_or(0); // idle + iowait
        let total: u64 = nums.iter().sum();
        Some((idle, total))
    }
    let (i1, t1) = match parse(a) { Some(x) => x, None => return 0 };
    let (i2, t2) = match parse(b) { Some(x) => x, None => return 0 };
    let dt = t2.saturating_sub(t1);
    let di = i2.saturating_sub(i1);
    if dt == 0 { return 0; }
    let busy = dt.saturating_sub(di) as f64 * 100.0 / dt as f64;
    busy.round().clamp(0.0, 100.0) as u32
}
