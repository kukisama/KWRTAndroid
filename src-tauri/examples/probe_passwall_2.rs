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
        ("test-sh-full", "cat /usr/share/passwall/test.sh"),
        ("utils-snippet", "grep -nE '^[a-zA-Z_][a-zA-Z0-9_]*\\(\\)' /usr/share/passwall/utils.sh | head -40"),
        ("controller-passwall-test-routes",
         "grep -nE 'function|test|ping|url' /usr/lib/lua/luci/controller/passwall.lua | head -60"),
        ("nc-busybox-opts",
         "busybox nc 2>&1; echo '---'; busybox --help 2>&1 | grep -o 'nc[^,]*'"),
        ("try-real-url-test",
         ". /usr/share/passwall/test.sh; \
          ids=$(uci show passwall | grep \"=nodes\" | head -1 | sed 's/.*passwall\\.\\([^=]*\\)=.*/\\1/'); \
          echo using=$ids; \
          url_test_node \"$ids\" urltest_node 2>&1; echo rc=$?"),
        ("uci-global-urltest",
         "uci show passwall.@global[0] 2>/dev/null | grep -i -E 'url|test|timeout|node' | head"),
        ("init-d-passwall-show",
         "head -40 /etc/init.d/passwall"),
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
