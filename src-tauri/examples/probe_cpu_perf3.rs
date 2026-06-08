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
        ("top-now",
         "uptime; echo ---; top -bn1 2>/dev/null | head -25"),
        ("current-node",
         "uci get passwall.@global[0].tcp_node; \
          uci get passwall.@global[0].udp_node; \
          n=$(uci get passwall.@global[0].tcp_node); \
          echo ---tcp_node detail---; uci show passwall.$n"),
        ("running-bin",
         "ls -la /tmp/etc/passwall/bin/ 2>/dev/null; \
          echo ---procs---; \
          ps w | grep -iE 'xray|sing-box|sing_box|hysteria|chinadns|trojan' | grep -v grep"),
        ("running-cfg-files",
         "ls -la /tmp/etc/passwall/acl/default/ 2>/dev/null"),
        ("sing-cfg-summary",
         "for f in /tmp/etc/passwall/acl/default/*.json; do \
            echo ===$f===; \
            grep -oE '\"type\"[ :]*\"[a-zA-Z0-9_-]+\"' $f | sort | uniq -c; \
          done"),
        ("uci-redir",
         "uci get passwall.@global_forwarding[0].udp_redir_ports; \
          uci get passwall.@global_forwarding[0].tcp_redir_ports; \
          uci get passwall.@global_forwarding[0].tcp_proxy_way"),
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
