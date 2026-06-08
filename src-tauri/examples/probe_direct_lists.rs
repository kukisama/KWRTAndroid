// 探针：搞清楚 KWRT 上"直连列表"到底是几个文件、字段名、LuCI 怎么呈现
// 用法：cd src-tauri; cargo run --quiet --example probe_direct_lists
use anyhow::Result;
use kwrt_controller_lib::{client::{ConnectOptions, LuciClient}, creds};

#[tokio::main]
async fn main() -> Result<()> {
    let host = "10.0.0.1";
    let user = "root";
    let pass = creds::load_password(host, user)?
        .ok_or_else(|| anyhow::anyhow!("Windows 凭据里没有 {host}/{user}"))?;

    let c = LuciClient::connect(ConnectOptions {
        host: host.into(), scheme: Some("http".into()), port: None,
        username: user.into(), password: pass,
        accept_invalid_certs: false, timeout_secs: Some(60),
    }).await?;

    let scripts: &[(&str, &str)] = &[
        ("rules-dir-listing", "ls -la /usr/share/passwall/rules/ 2>&1"),
        ("rules-dir-listing-pw2", "ls -la /usr/share/passwall2/rules/ 2>/dev/null || echo '(no passwall2)'"),
        ("direct_ip-head", "echo '--- direct_ip ---'; head -40 /usr/share/passwall/rules/direct_ip 2>&1; echo; wc -l /usr/share/passwall/rules/direct_ip"),
        ("direct_host-head", "echo '--- direct_host ---'; head -40 /usr/share/passwall/rules/direct_host 2>&1; echo; wc -l /usr/share/passwall/rules/direct_host"),
        ("proxy_ip-head",  "echo '--- proxy_ip ---'; head -10 /usr/share/passwall/rules/proxy_ip 2>&1; wc -l /usr/share/passwall/rules/proxy_ip"),
        ("proxy_host-head","echo '--- proxy_host ---'; head -10 /usr/share/passwall/rules/proxy_host 2>&1; wc -l /usr/share/passwall/rules/proxy_host"),
        ("block_ip-head",  "head -10 /usr/share/passwall/rules/block_ip 2>&1; wc -l /usr/share/passwall/rules/block_ip"),
        ("block_host-head","head -10 /usr/share/passwall/rules/block_host 2>&1; wc -l /usr/share/passwall/rules/block_host"),
        // LuCI 直连列表页 CBI / view，确认 "二合一 or 分开"
        ("cbi-rule-list-files", "ls -la /usr/lib/lua/luci/model/cbi/passwall/client/ 2>&1 | grep -iE 'rule|direct|list'"),
        ("controller-rule-grep", "grep -nE 'direct_ip|direct_host|rule_list|/rule_list' /usr/lib/lua/luci/controller/passwall.lua 2>&1 | head -30"),
        ("cbi-rule_list-head", "ls /usr/lib/lua/luci/model/cbi/passwall/client/rule_list* 2>/dev/null; for f in /usr/lib/lua/luci/model/cbi/passwall/client/rule_list*; do echo \"=== $f ===\"; head -120 \"$f\"; done"),
        ("view-rule_list", "ls /usr/lib/lua/luci/view/passwall/rule_list/ 2>/dev/null; for f in /usr/lib/lua/luci/view/passwall/rule_list/*.htm; do echo \"=== $f ===\"; head -80 \"$f\"; done"),
    ];

    for (label, script) in scripts {
        eprintln!("\n========== {label} ==========");
        match c.shell(script).await {
            Ok((code, out, err)) => {
                eprintln!("[exit={code}]");
                print!("{out}");
                if !out.ends_with('\n') { println!(); }
                if !err.trim().is_empty() { eprintln!("--stderr--\n{err}"); }
            }
            Err(e) => eprintln!("[err] {e:#}"),
        }
    }
    Ok(())
}
