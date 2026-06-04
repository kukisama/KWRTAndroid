use anyhow::Result;
use kwrt_controller_lib::{client::{ConnectOptions, LuciClient}, creds};

#[tokio::main]
async fn main() -> Result<()> {
    let host = "10.0.0.1"; let user = "root";
    let pass = creds::load_password(host, user)?.unwrap();
    let c = LuciClient::connect(ConnectOptions {
        host: host.into(), scheme: Some("http".into()), port: None,
        username: user.into(), password: pass,
        accept_invalid_certs: false, timeout_secs: Some(45),
    }).await?;

    let script = r##"
set -e
TS=$(date +%s)
# 备份当前可能存在的同名文件
if [ -f /etc/sysctl.d/99-net-tuning.conf ]; then
  cp /etc/sysctl.d/99-net-tuning.conf /etc/sysctl.d/99-net-tuning.conf.bak.$TS
fi
# 备份当前运行时值（出问题时可对照）
{
  echo "# saved at $TS"
  sysctl net.core.rmem_max net.core.wmem_max \
         net.ipv4.tcp_rmem net.ipv4.tcp_wmem 2>/dev/null
} > /etc/sysctl.d/99-net-tuning.conf.previous.$TS

cat > /etc/sysctl.d/99-net-tuning.conf <<'EOF'
# KWRT 4K 跨境优化：放大 TCP 缓冲窗口
# 加这个的原因：默认 rmem/wmem_max 只有 180KB，跨境高 RTT 上 BBR 撑不开窗口，
# 4K 视频偶尔卡。把上限抬到 4MB，按需分配，国内/局域网不受影响。
# 想恢复默认：删本文件再 reboot，或参考 .previous.<ts> 备份用 sysctl -w 还原。
net.core.rmem_max = 4194304
net.core.wmem_max = 4194304
net.ipv4.tcp_rmem = 4096 87380 4194304
net.ipv4.tcp_wmem = 4096 16384 4194304
EOF

# 立刻生效
sysctl -p /etc/sysctl.d/99-net-tuning.conf
echo --- now ---
sysctl net.core.rmem_max net.core.wmem_max \
       net.ipv4.tcp_rmem net.ipv4.tcp_wmem
echo "backup_tag=$TS"
"##;

    let (code, stdout, stderr) = c.shell(script).await?;
    eprintln!("[exit={code}]");
    if !stdout.is_empty() { println!("{stdout}"); }
    if !stderr.trim().is_empty() { eprintln!("--stderr--\n{stderr}"); }
    Ok(())
}
