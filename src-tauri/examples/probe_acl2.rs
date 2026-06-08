// 进一步定位 ACL CBI + 字段
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
        ("find_acl_cbi",
         "find /usr/lib/lua/luci/model/cbi/passwall -iname '*acl*' 2>/dev/null"),
        ("acl_index_lua",
         "F=$(find /usr/lib/lua/luci/model/cbi/passwall -iname '*acl*' 2>/dev/null | grep -i index | head -1); echo \"FILE=$F\"; sed -n '1,150p' \"$F\" 2>/dev/null"),
        ("acl_detail_lua",
         "F=$(find /usr/lib/lua/luci/model/cbi/passwall -iname '*acl*' 2>/dev/null | grep -v index | head -1); echo \"FILE=$F\"; sed -n '1,200p' \"$F\" 2>/dev/null"),
        ("controller_acl",
         "grep -rni 'acl' /usr/lib/lua/luci/controller/passwall.lua 2>/dev/null | head -20"),
        ("default_acl_section",
         "uci show passwall | grep -E '^passwall\\.[^.]+=acl' 2>/dev/null"),
        ("uci_add_dry",
         "echo 'will use: uci add passwall acl_rule; uci set ...sources/tcp_node/udp_node/enabled/remarks'"),
        ("cpu_proc_stat",
         "head -1 /proc/stat; cat /proc/cpuinfo | grep -E 'model name|processor|cpu MHz|BogoMIPS' | head -20"),
        ("disk_block_devices",
         "ls /sys/block/ 2>/dev/null; echo ---; \
          ls /dev/ubi* 2>/dev/null; echo ---; \
          cat /proc/mtd 2>/dev/null"),
        ("ls_root",
         "ls -la / 2>/dev/null | head -40"),
        ("ls_etc_config",
         "ls -la /etc/config 2>/dev/null"),
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
