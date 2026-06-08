use anyhow::Result;
use kwrt_controller_lib::{client::{ConnectOptions, LuciClient}, creds};

#[tokio::main]
async fn main() -> Result<()> {
    let host = "10.0.0.1"; let user = "root";
    let pass = creds::load_password(host, user)?.unwrap();
    let c = LuciClient::connect(ConnectOptions {
        host: host.into(), scheme: Some("http".into()), port: None,
        username: user.into(), password: pass,
        accept_invalid_certs: false, timeout_secs: Some(45),
    }).await?;

    let probes: &[(&str, &str)] = &[
        ("current-node-detail",
         "uci show passwall.xv8eGFbE 2>/dev/null"),
        ("xray-config-summary",
         "cfg=/tmp/etc/passwall/acl/default/TCP_UDP_SOCKS.json; \
          ls -la $cfg; \
          grep -oE '\"protocol\"[ :]*\"[a-zA-Z_-]+\"' $cfg | sort -u; \
          grep -oE '\"network\"[ :]*\"[a-zA-Z_-]+\"' $cfg | sort -u; \
          grep -oE '\"security\"[ :]*\"[a-zA-Z0-9_-]+\"' $cfg | sort -u; \
          grep -oE '\"flow\"[ :]*\"[a-zA-Z0-9_-]+\"' $cfg | sort -u; \
          grep -oE '\"sniffing\"|\"destOverride\"|\"routeOnly\"' $cfg | sort | uniq -c; \
          grep -ciE '\"mux\"|\"concurrency\"' $cfg"),
        ("xray-threads",
         "ls /proc/32341/task 2>/dev/null | wc -l; \
          cat /proc/32341/status 2>/dev/null | grep -E '^(Threads|VmRSS|VmSize|voluntary_ctxt|nonvoluntary)'"),
        ("rps-xps",
         "for q in /sys/class/net/eth0/queues/rx-*/rps_cpus; do echo $q=$(cat $q); done; \
          for q in /sys/class/net/wan/queues/rx-*/rps_cpus 2>/dev/null; do echo $q=$(cat $q); done; \
          echo ---rfs---; sysctl net.core.rps_sock_flow_entries"),
        ("dns-and-sniff",
         "uci show passwall | grep -iE 'sniff|domain_strategy|fragment|mux'; \
          echo ---chinadns---; ps w | grep chinadns | grep -v grep"),
        ("connections",
         "wc -l /proc/net/nf_conntrack; \
          awk '{print $3}' /proc/net/nf_conntrack | sort | uniq -c | sort -rn | head"),
        ("xray-cpu-1s",
         "p1=$(awk '{print $14+$15}' /proc/32341/stat); sleep 1; \
          p2=$(awk '{print $14+$15}' /proc/32341/stat); \
          echo \"xray jiffies/s: $((p2-p1))   (HZ usually 250 on this kernel)\"; \
          grep ^CONFIG_HZ /proc/config.gz 2>/dev/null; \
          getconf CLK_TCK"),
        ("kwrt-build",
         "cat /etc/openwrt_release | grep -i revision; \
          opkg list-installed | grep -iE 'xray|sing-box|passwall' | head"),
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
