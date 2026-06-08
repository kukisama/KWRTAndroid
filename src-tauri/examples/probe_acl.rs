use anyhow::Result;
use kwrt_controller_lib::{client::{ConnectOptions, LuciClient}, creds};

#[tokio::main]
async fn main() -> Result<()> {
    let host = "10.0.0.1"; let user = "root";
    let pass = creds::load_password(host, user)?.unwrap();
    let c = LuciClient::connect(ConnectOptions {
        host: host.into(), scheme: Some("http".into()), port: None,
        username: user.into(), password: pass,
        accept_invalid_certs: false, timeout_secs: Some(60),
    }).await?;

    for (label, script) in [
        ("acl_rule_sections",
         "uci show passwall | grep -E 'acl_rule' | head -200"),
        ("global_other_lan",
         "uci -q show passwall.@global[0] 2>/dev/null | head -200; echo ---; \
          uci -q show passwall.@global_haproxy[0] 2>/dev/null | head -20; echo ---; \
          uci -q show passwall.@global_app[0] 2>/dev/null | head -20"),
        ("nodes_brief",
         "uci show passwall | awk -F'=' '/=nodes/{print $1}' | head -50"),
        ("lan_devices",
         "cat /tmp/dhcp.leases 2>/dev/null | head -30; echo ---; \
          ip neigh show 2>/dev/null | grep -v FAILED | head -30"),
        ("cpu_stat_sources",
         "head -2 /proc/stat; echo ---; \
          cat /proc/loadavg; echo ---; \
          uptime; echo ---; \
          top -bn1 2>/dev/null | head -10"),
        ("storage_facts",
         "df -h 2>/dev/null; echo ---; \
          mount | head -30; echo ---; \
          ls -la /rom 2>/dev/null | head -10"),
        ("luci_acl_model_lua",
         "sed -n '1,80p' /usr/lib/lua/luci/model/cbi/passwall/client/acl/index.lua 2>/dev/null; echo ---END---"),
        ("luci_acl_interface_lua",
         "ls /usr/lib/lua/luci/model/cbi/passwall/client/acl/ 2>/dev/null"),
    ] {
        eprintln!("\n========== {label} ==========");
        match c.shell(script).await {
            Ok((code, out, err)) => {
                eprintln!("[exit={code}]");
                print!("{out}"); if !out.ends_with('\n') { println!(); }
                if !err.trim().is_empty() { eprintln!("--stderr--\n{err}"); }
            }
            Err(e) => eprintln!("[err] {e:#}"),
        }
    }
    Ok(())
}
