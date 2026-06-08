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
        ("controller-ping-and-url-blocks",
         "sed -n '420,500p' /usr/lib/lua/luci/controller/passwall.lua"),
        ("global_other-url",
         "uci -q get passwall.@global_other[0].url_test_url; echo ---; \
          uci show passwall.@global_other[0] 2>/dev/null | head -40"),
        ("test-http-ping-icmp-direct",
         "timeout 5 wget -qO- --header='Cookie: $(env|grep sysauth)' \
          'http://127.0.0.1/cgi-bin/luci/admin/services/passwall/ping_node?type=icmp&address=8.8.8.8&port=443&id=Pen9ygcU' 2>&1 | head -40"),
        ("test-curl-from-router-204",
         "curl -I -o /dev/null -skL --connect-timeout 3 --max-time 6 -w '%{http_code}\\n' https://www.google.com/generate_204"),
        ("which-tcping",
         "command -v tcping; ls /usr/bin/tcping 2>/dev/null"),
        ("nodes-ids",
         "uci show passwall | grep '=nodes' | sed 's/.*passwall\\.\\([^=]*\\)=.*/\\1/'"),
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

    // 直接走 HTTP 试两个 LuCI 端点（带 sysauth cookie）
    let urls = [
        ("http-ping-icmp",
         "cgi-bin/luci/admin/services/passwall/ping_node?type=icmp&address=1.1.1.1&port=443&id=Pen9ygcU"),
        ("http-ping-tcping",
         "cgi-bin/luci/admin/services/passwall/ping_node?type=tcping&address=1.1.1.1&port=443&id=Pen9ygcU"),
        ("http-urltest",
         "cgi-bin/luci/admin/services/passwall/urltest_node?id=Pen9ygcU"),
    ];
    for (label, path) in urls {
        eprintln!("\n========== {label} ==========");
        let url = c.base_url.join(path)?;
        let r = c.http.get(url.clone()).send().await;
        match r {
            Ok(resp) => {
                let s = resp.status();
                let body = resp.text().await.unwrap_or_default();
                eprintln!("[status={s}] url={url}");
                println!("{}", body.chars().take(1500).collect::<String>());
            }
            Err(e) => eprintln!("[err] {e:#}"),
        }
    }
    Ok(())
}
