// 检查路由器存储分区与剩余空间。
// 运行：cargo run -p kwrt-controller --example probe_disk --quiet
use anyhow::Result;
use kwrt_controller_lib::{client::{ConnectOptions, LuciClient}, creds};

#[tokio::main]
async fn main() -> Result<()> {
    let host = "10.0.0.1";
    let user = "root";
    let pass = creds::load_password(host, user)?
        .ok_or_else(|| anyhow::anyhow!("Windows 凭据里没有 {host}/{user}"))?;

    let mut c = LuciClient::connect(ConnectOptions {
        host: host.into(),
        scheme: Some("http".into()),
        port: None,
        username: user.into(),
        password: pass,
        accept_invalid_certs: false,
        timeout_secs: Some(30),
    }).await?;
    eprintln!("[ok] logged in");

    let probes: &[(&str, &str)] = &[
        ("df-h", "df -h"),
        ("df-i", "df -i 2>/dev/null"),
        ("mount", "mount"),
        ("mtd", "cat /proc/mtd 2>/dev/null"),
        ("partitions", "cat /proc/partitions 2>/dev/null"),
        ("board", "cat /tmp/sysinfo/board_name 2>/dev/null; echo ---; cat /tmp/sysinfo/model 2>/dev/null"),
        ("flash-overlay", "ls -la /overlay 2>/dev/null | head -20; echo ---; ls -la /overlay/upper 2>/dev/null | head"),
        ("opkg-conf", "cat /etc/opkg.conf 2>/dev/null; echo ---; ls /etc/opkg/ 2>/dev/null"),
        ("memory", "free -m 2>/dev/null; echo ---; cat /proc/meminfo 2>/dev/null | head -10"),
        ("biggest-overlay", "du -sh /overlay/upper/* 2>/dev/null | sort -hr | head -20"),
        ("biggest-root-usr", "du -sh /usr/* 2>/dev/null | sort -hr | head -15"),
        ("tmp-usage", "du -sh /tmp/* 2>/dev/null | sort -hr | head -10"),
        ("opkg-installed-count", "opkg list-installed 2>/dev/null | wc -l"),
        ("opkg-big-packages", "opkg list-installed 2>/dev/null | awk '{print $1}' | while read p; do sz=$(opkg status \"$p\" 2>/dev/null | awk '/Installed-Size/ {print $2}'); [ -n \"$sz\" ] && echo \"$sz $p\"; done | sort -nr | head -20"),
    ];

    for (label, script) in probes {
        println!("\n========== {label} ==========");
        match c.shell(script).await {
            Ok((code, stdout, stderr)) => {
                println!("[exit={code}]");
                if !stdout.is_empty() { print!("{}", stdout); if !stdout.ends_with('\n') { println!(); } }
                if !stderr.trim().is_empty() { eprintln!("--stderr--\n{}", stderr); }
            }
            Err(e) => eprintln!("[err] {e:#}"),
        }
    }
    Ok(())
}
