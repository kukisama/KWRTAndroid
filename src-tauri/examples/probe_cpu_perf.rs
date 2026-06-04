// 一次性诊断：路由器 CPU/网络/PassWall 性能瓶颈。
// 运行：cargo run -p kwrt-controller --example probe_cpu_perf --quiet
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
        timeout_secs: Some(45),
    }).await?;
    eprintln!("[ok] logged in");

    let probes: &[(&str, &str)] = &[
        ("hw-id",
         "cat /etc/openwrt_release 2>/dev/null; echo ---; uname -a; echo ---; \
          cat /proc/cpuinfo | head -30; echo ---mem---; free; echo ---load---; cat /proc/loadavg; \
          echo ---uptime---; uptime"),
        ("top-cpu",
         "top -bn1 2>/dev/null | head -40"),
        // 按 CPU 排序，picky busybox top 不一定支持；试备用
        ("top-sorted",
         "top -bn1 -o %CPU 2>/dev/null | head -30 || top -bn1 | head -30"),
        // 进程级 CPU：通过 /proc/*/stat 采样 1 秒计算
        ("ps-snapshot",
         "ps w 2>/dev/null | head -80"),
        ("cpu-sample-2s",
         "awk '{print $1,$2,$3,$4}' /proc/stat | head -1; sleep 2; \
          awk '{print $1,$2,$3,$4}' /proc/stat | head -1; echo ---irq---; cat /proc/interrupts | head -30"),
        // 软中断 + 网络驱动负载
        ("softirq",
         "cat /proc/softirqs | head -20"),
        // PassWall 关键进程
        ("passwall-procs",
         "ps w | grep -iE 'xray|sing-box|sing_box|v2ray|hysteria|naive|trojan|brook|chinadns|dns2|smartdns|haproxy|passwall' | grep -v grep"),
        ("passwall-status",
         "/etc/init.d/passwall status 2>&1 | head -20; echo ---; \
          ls /tmp/etc/passwall 2>/dev/null | head; echo ---; \
          ls /var/etc/passwall 2>/dev/null | head"),
        // PassWall 全局配置 + 当前节点
        ("uci-global",
         "uci show passwall.@global[0] 2>/dev/null"),
        ("uci-global-forwarding",
         "uci show passwall.@global_forwarding[0] 2>/dev/null; echo ---haproxy---; \
          uci show passwall.@global_haproxy[0] 2>/dev/null; echo ---xray---; \
          uci show passwall.@global_xray[0] 2>/dev/null; echo ---dns---; \
          uci show passwall.@global_subscribe[0] 2>/dev/null"),
        ("current-node",
         "node=$(uci get passwall.@global[0].node 2>/dev/null); echo node_id=$node; \
          [ -n \"$node\" ] && uci show passwall.$node 2>/dev/null"),
        // xray/sing-box 实际配置文件路径
        ("running-config",
         "ls -la /tmp/etc/passwall/ 2>/dev/null; echo ---; \
          ls -la /var/etc/passwall/ 2>/dev/null"),
        // 流量统计 / 网卡
        ("net-stats",
         "cat /proc/net/dev | head -20; echo ---; \
          ip -s link 2>/dev/null | head -40"),
        // 转发模式 / TPROXY / Redir / TUN
        ("ipt-nat",
         "iptables -t nat -L PSW -n -v 2>/dev/null | head -40; echo ---mangle---; \
          iptables -t mangle -L PSW -n -v 2>/dev/null | head -40"),
        ("nf-conntrack",
         "cat /proc/sys/net/netfilter/nf_conntrack_count 2>/dev/null; \
          cat /proc/sys/net/netfilter/nf_conntrack_max 2>/dev/null; \
          echo ---offload---; \
          cat /sys/module/nf_conntrack/parameters/* 2>/dev/null | head; \
          echo ---flow---; \
          cat /etc/config/firewall 2>/dev/null | grep -iE 'offload|flow|software|hardware'"),
        // 硬件加速 (mt7621 HNAT)
        ("hnat",
         "ls /sys/kernel/debug/hnat 2>/dev/null | head; \
          cat /etc/config/network 2>/dev/null | grep -iE 'hwnat|hnat'; \
          echo ---kmod---; lsmod | grep -iE 'hnat|hwnat|fast_classifier|shortcut'"),
        // sing-box / xray 日志末尾
        ("logs",
         "logread -e passwall 2>/dev/null | tail -40; echo ---xray---; \
          tail -30 /tmp/log/passwall/*.log 2>/dev/null; \
          tail -30 /var/log/passwall/*.log 2>/dev/null"),
        // 速度类相关 sysctl
        ("sysctl-net",
         "sysctl net.ipv4.tcp_congestion_control net.ipv4.tcp_available_congestion_control \
                 net.core.default_qdisc net.ipv4.tcp_fastopen \
                 net.core.rmem_max net.core.wmem_max 2>/dev/null"),
    ];

    for (label, script) in probes {
        eprintln!("\n========== {label} ==========");
        match c.shell(script).await {
            Ok((code, stdout, stderr)) => {
                eprintln!("[exit={code}]");
                if !stdout.is_empty() { print!("{}", stdout); if !stdout.ends_with('\n') { println!(); } }
                if !stderr.trim().is_empty() { eprintln!("--stderr--\n{}", stderr); }
            }
            Err(e) => eprintln!("[err] {e:#}"),
        }
    }
    Ok(())
}
