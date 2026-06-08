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
        // ===== 文件浏览相关 =====
        ("ls_root_raw",
         "ls -la / 2>&1 | head -40; echo ---END---"),
        ("ls_etc_config_raw",
         "ls -la /etc/config 2>&1 | head -40; echo ---END---"),
        ("ls_alt_print",
         "ls -la / 2>&1 | cat -A | head -20; echo ---END---"),
        ("busybox_ver",
         "busybox 2>&1 | head -3; echo ---; \
          ls --help 2>&1 | head -3; echo ---END---"),

        // ===== ACL 完整字段（acl_config.lua）=====
        ("acl_config_lua_full",
         "cat /usr/lib/lua/luci/model/cbi/passwall/client/acl_config.lua 2>/dev/null | head -400; echo ---END---"),
        ("acl_lua",
         "cat /usr/lib/lua/luci/model/cbi/passwall/client/acl.lua 2>/dev/null | head -200; echo ---END---"),

        // ===== rule_list 关系 =====
        ("rule_list_lua",
         "ls /usr/lib/lua/luci/model/cbi/passwall/client/ 2>/dev/null | grep -i rule; echo ---; \
          ls /usr/share/passwall/rules/ 2>/dev/null; echo ---; \
          cat /usr/share/passwall/rules/lanlist_ipv4 2>/dev/null | head -30; echo ---END---"),
        ("rule_list_config_relation",
         "uci show passwall | grep -E 'list|chnlist|gfw' | head -40; echo ---END---"),

        // ===== 排查保存超时 =====
        ("file_exec_test_short",
         "echo hello; uci show passwall.@global[0].acl_enable; echo done"),
        ("file_exec_test_uci_add",
         "echo 'TIMING'; date +%s; \
          T=$(uci add passwall acl_rule); echo \"SEC=$T\"; \
          uci set passwall.$T.enabled='0'; \
          uci set passwall.$T.remarks='probe_delete_me'; \
          uci set passwall.$T.sources='10.0.0.250'; \
          uci set passwall.$T.tcp_node='nil'; uci set passwall.$T.udp_node='nil'; \
          uci commit passwall; date +%s; echo OK"),
    ] {
        eprintln!("\n========== {label} ==========");
        match c.shell(script).await {
            Ok((code, out, err)) => {
                eprintln!("[exit={code} len_out={} len_err={}]", out.len(), err.len());
                print!("{out}"); if !out.ends_with('\n') { println!(); }
                if !err.trim().is_empty() { eprintln!("--stderr--\n{err}"); }
            }
            Err(e) => eprintln!("[err] {e:#}"),
        }
    }
    Ok(())
}
