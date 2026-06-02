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
        // LuCI 处理"保存/保存并应用/复位"的核心源
        ("dispatcher-cbi-actions",
         "grep -nE 'cbi.*save|cbi.*apply|cbi.*reset|action_apply|apply_unattended|apply_xhr|rollback' /usr/lib/lua/luci/dispatcher.lua | head -40"),
        ("cbi-lua-buttons",
         "grep -nE 'cbi-button-save|cbi-button-apply|cbi-button-reset|reset|apply|save' /usr/lib/lua/luci/view/cbi/footer.htm 2>/dev/null"),
        ("cbi-footer-files",
         "find /usr/lib/lua/luci/view/cbi -maxdepth 2 -name 'footer*' -o -name 'apply*' 2>/dev/null"),
        ("apply-controllers",
         "grep -RInE 'apply_rollback|apply_unattended|action_apply|/admin/uci/apply' /usr/lib/lua/luci/controller/admin 2>/dev/null | head -40"),
        ("luci-uci-controller-tail",
         "wc -l /usr/lib/lua/luci/controller/admin/uci.lua 2>/dev/null; head -200 /usr/lib/lua/luci/controller/admin/uci.lua 2>/dev/null"),
        ("apply-from-tools",
         "grep -RInE 'apply_unattended|reload_config|service\\.apply' /usr/lib/lua/luci 2>/dev/null | head -40"),
        // PassWall 自带的 api.uci_save / sh_uci_commit
        ("passwall-api-lua",
         "grep -nE '^function (api|M)\\.|uci_save|sh_uci_commit|sh_uci_set|sh_uci_add|sh_uci_del|init\\.d|reload|apply' /usr/lib/lua/luci/passwall/api.lua | head -80"),
        ("passwall-uci_save-body",
         "awk '/function .*uci_save/,/^end/' /usr/lib/lua/luci/passwall/api.lua | head -80"),
        ("passwall-sh-commit-body",
         "awk '/function .*sh_uci_commit/,/^end/' /usr/lib/lua/luci/passwall/api.lua | head -40"),
        // ubus service apply 的 rollback/timeout 用法
        ("rcS-apply-handler",
         "ls /sbin/rc /sbin/apply 2>/dev/null; cat /usr/sbin/luci-reload 2>/dev/null | head -40"),
        // 看一下 ucitrack 哪些 init.d 是 passwall reload 的
        ("ucitrack-passwall",
         "uci show ucitrack 2>/dev/null | grep -i passwall; echo ---; uci show ucitrack 2>/dev/null | head -30"),
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
