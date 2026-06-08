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

    // 删除 probe 留下的两条
    let s = "for S in cfg1c30ec cfg1d30ec; do \
             if uci -q get passwall.$S >/dev/null 2>&1; then \
               echo deleting $S; uci delete passwall.$S; \
             fi; done; uci commit passwall; \
             echo === remaining acl_rule ===; \
             uci show passwall | grep acl_rule";
    let (code, out, err) = c.shell(s).await?;
    println!("[exit={code}]\n{out}");
    if !err.trim().is_empty() { eprintln!("--stderr--\n{err}"); }
    Ok(())
}
