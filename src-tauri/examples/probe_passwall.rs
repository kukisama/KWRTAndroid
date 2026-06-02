// 一次性探测：登录路由器并扫描 PassWall 在 KWRT 上的真实文件结构。
// 运行：cargo run -p kwrt-controller --example probe_passwall --quiet
// 注意：用 Windows 凭据管理器里 host=10.0.0.1 user=root 的条目。
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
    eprintln!("[ok] logged in. sysauth={:?} ubus={:?}", c.sysauth.is_some(), c.ubus_session.is_some());

    let probes: &[(&str, &str)] = &[
        // 1. 总览 PassWall 安装
        ("opkg-list-passwall",
         "opkg list-installed 2>/dev/null | grep -E '^(luci-app-passwall|passwall)' | head -20"),
        ("passwall-dirs",
         "ls -la /usr/share/passwall 2>/dev/null | head -60"),
        ("passwall-bin",
         "ls -la /usr/bin | grep -iE 'passwall|xray|sing-?box|hysteria|naive|trojan|brook|geo|chinadns' | head"),
        ("init-d",
         "ls -la /etc/init.d/passwall* 2>/dev/null"),
        // 2. test.sh —— 函数清单
        ("test-sh-head",
         "head -5 /usr/share/passwall/test.sh 2>/dev/null; echo '---'; \
          grep -nE '^[a-zA-Z_][a-zA-Z0-9_]*\\(\\)' /usr/share/passwall/test.sh 2>/dev/null"),
        ("test-sh-runnable",
         "[ -x /usr/share/passwall/test.sh ] && echo executable || echo not-executable; \
          file /usr/share/passwall/test.sh 2>/dev/null; \
          wc -l /usr/share/passwall/test.sh 2>/dev/null"),
        // 3. LuCI 里如何调用 test —— 看 controller / view
        ("luci-controller",
         "ls /usr/lib/lua/luci/controller/passwall* 2>/dev/null; \
          grep -nE 'url_test|test_url|test\\.sh|node_test' /usr/lib/lua/luci/controller/passwall.lua 2>/dev/null | head -40"),
        ("luci-view-test",
         "ls /usr/lib/lua/luci/view/passwall/node_list/ 2>/dev/null; \
          grep -RInE 'url_test|test_url|tcping|test\\.sh' /usr/lib/lua/luci/view/passwall 2>/dev/null | head -40"),
        ("luci-model",
         "ls /usr/lib/lua/luci/model/cbi/passwall 2>/dev/null | head; \
          grep -RInE 'test\\.sh|url_test|tcping' /usr/lib/lua/luci/model/cbi/passwall 2>/dev/null | head -40"),
        // 4. PassWall API 文件（PassWall2 / PassWall 都把通用函数放 api.lua / api.sh）
        ("api-files",
         "ls -la /usr/share/passwall/*.sh /usr/share/passwall/*.lua /usr/share/passwall/api/* 2>/dev/null | head -40"),
        // 5. 直接试着调三种最可能的函数名，看脚本里到底有谁
        ("source-and-type",
         ". /usr/share/passwall/test.sh 2>/dev/null; \
          for fn in url_test_node test_url_node test_url url_test node_test test_node; do \
            if type \"$fn\" >/dev/null 2>&1; then echo HAVE:$fn; else echo MISS:$fn; fi; \
          done"),
        // 6. 直接调一次 url_test_node 看到底要几个参数
        ("try-url-test-node-noargs",
         ". /usr/share/passwall/test.sh 2>/dev/null; \
          if type url_test_node >/dev/null 2>&1; then \
            out=$(url_test_node 2>&1); echo rc=$?; echo \"out=$out\" | head -10; \
          fi"),
        // 7. KWRT 是 luci2/argon？看是不是有不同布局
        ("kwrt-id",
         "cat /etc/openwrt_release 2>/dev/null; echo ---; \
          cat /etc/os-release 2>/dev/null | head -10; echo ---; \
          uname -a"),
        ("nc-binary",
         "which nc; ls -la /bin/nc /usr/bin/nc 2>/dev/null; nc --help 2>&1 | head -5"),
        ("uci-show-passwall-head",
         "uci show passwall 2>/dev/null | head -5; uci show passwall.@global[0] 2>/dev/null | head"),
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
