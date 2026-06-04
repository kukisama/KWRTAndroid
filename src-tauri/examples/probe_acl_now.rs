// 检查 PassWall ACL 当前配置和短路 rule
use anyhow::Result;
use kwrt_controller_lib::{client::{ConnectOptions, LuciClient}, creds};

#[tokio::main]
async fn main() -> Result<()> {
    let host = "10.0.0.1"; let user = "root";
    let pass = creds::load_password(host, user)?.ok_or_else(|| anyhow::anyhow!("no creds"))?;
    let c = LuciClient::connect(ConnectOptions {
        host: host.into(), scheme: Some("http".into()), port: None,
        username: user.into(), password: pass,
        accept_invalid_certs: false, timeout_secs: Some(30),
    }).await?;
    let probes: &[(&str, &str)] = &[
        ("acl-enable", "uci get passwall.@global[0].acl_enable 2>/dev/null"),
        ("acl-rules", "uci show passwall 2>/dev/null | grep acl_rule"),
        ("nft-passwall", "nft list table inet passwall 2>/dev/null | head -80"),
        ("kwrt-disabled", "nft list table inet passwall 2>/dev/null | grep -i 'kwrt-disabled'"),
        ("dnsmasq-leases", "cat /tmp/dhcp.leases 2>/dev/null | head -20"),
    ];
    for (l, s) in probes {
        println!("\n== {l} ==");
        match c.shell(s).await {
            Ok((code, out, err)) => { println!("[exit={code}]"); print!("{}", out); if !err.trim().is_empty() { eprintln!("--err--\n{}", err); } }
            Err(e) => eprintln!("[err] {e:#}"),
        }
    }
    Ok(())
}
