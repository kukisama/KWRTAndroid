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
        // 1) 现存订阅 section 全字段（看 remark vs 别的字段名）
        ("uci-subscribe-full",
         "uci show passwall | grep -E '@?subscribe_list|=subscribe' "),
        ("uci-show-by-section-name",
         "for s in $(uci show passwall | grep '=subscribe_list' | sed 's/.*\\.\\([^=]*\\)=.*/\\1/'); do echo '==='$s; uci -q show passwall.$s; done"),
        // 2) PassWall subscribe 的 CBI 模型，看官方表单怎么定义字段
        ("cbi-subscribe-list",
         "ls /usr/lib/lua/luci/model/cbi/passwall/client/ 2>/dev/null; \
          echo ---; \
          cat /usr/lib/lua/luci/model/cbi/passwall/client/node_subscribe.lua 2>/dev/null | head -200"),
        ("cbi-grep-remark",
         "grep -RInE 'remark|Remark|备注' /usr/lib/lua/luci/model/cbi/passwall/ 2>/dev/null | head -40"),
        // 3) subscribe.lua 自身怎么写入 remarks（它会用订阅地址 / 数字编号）
        ("subscribe-lua-write",
         "grep -nE 'remark|备注|node_num|name=|filename|节点数量' /usr/share/passwall/subscribe.lua 2>/dev/null | head -60"),
        // 4) LuCI 保存/应用流程：标准 cbi save vs apply（看一眼 luci-base 的入口）
        ("luci-apply-endpoint",
         "ls /usr/lib/lua/luci/dispatcher.lua 2>/dev/null; \
          grep -nE 'ubus.*apply|uci_apply|apply_unattended|rollback|reload_config|service' /usr/lib/lua/luci/dispatcher.lua 2>/dev/null | head -40"),
        ("luci-cbi-action",
         "grep -nE 'function action_apply|function action_save|apply_xhr|cbi-save' /usr/lib/lua/luci/controller/admin/uci.lua 2>/dev/null | head -40; \
          echo ---; \
          ls /usr/lib/lua/luci/controller/admin/ 2>/dev/null | head"),
        ("luci-uci-controller-full",
         "cat /usr/lib/lua/luci/controller/admin/uci.lua 2>/dev/null | head -180"),
        // 5) passwall controller 自己有没有覆盖 save / reload 行为
        ("passwall-controller-save-apply",
         "grep -nE 'apply|reload|restart|commit|save|init\\.d|service_start|sys\\.call' /usr/lib/lua/luci/controller/passwall.lua | head -80"),
        // 6) ubus uci 服务：apply / commit 操作
        ("ubus-uci-introspect",
         "ubus list | grep -E '^uci$|^service$|^system$'; echo ---; ubus -v list uci 2>&1 | head -60"),
        ("ubus-service-introspect",
         "ubus -v list service 2>&1 | head -40"),
        // 7) Active config 与 staged：/etc/config vs /tmp/.uci
        ("uci-staging-dir",
         "ls -la /tmp/.uci 2>/dev/null; echo ---; ls -la /etc/config/passwall*"),
        ("uci-changes-pending",
         "uci changes; echo rc=$?; echo ---; uci changes passwall"),
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
