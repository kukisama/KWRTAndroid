// 探测 U 盘状态与已装的 USB/存储相关模块。
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
        accept_invalid_certs: false, timeout_secs: Some(30),
    }).await?;
    eprintln!("[ok] logged in");

    let probes: &[(&str, &str)] = &[
        ("usb-dev", "ls -la /dev/sd* /dev/sda* 2>/dev/null; echo ---; ls /sys/block/ 2>/dev/null"),
        ("dmesg-usb", "dmesg 2>/dev/null | grep -iE 'usb|sd[a-z]|scsi' | tail -40"),
        ("block-info", "block info 2>/dev/null"),
        ("lsusb", "lsusb 2>/dev/null"),
        ("kmod-usb", "lsmod 2>/dev/null | grep -iE 'usb|scsi|ehci|xhci|ohci|storage|ext4|vfat|fuse|exfat|ntfs'"),
        ("opkg-installed-storage", "opkg list-installed 2>/dev/null | grep -iE 'block-mount|kmod-usb|kmod-fs|e2fsprogs|kmod-scsi|usbutils|f2fs|exfat|ntfs'"),
        ("fstab", "cat /etc/config/fstab 2>/dev/null"),
        ("partitions", "cat /proc/partitions 2>/dev/null"),
        ("mounts", "mount | grep -v 'cgroup\\|proc\\|sysfs\\|tmpfs\\|debugfs\\|bpffs\\|devpts'"),
        ("usb-sys", "ls /sys/bus/usb/devices/ 2>/dev/null"),
        ("free-overlay", "df -h /overlay /tmp /"),
    ];

    for (label, script) in probes {
        println!("\n========== {label} ==========");
        match c.shell(script).await {
            Ok((code, stdout, stderr)) => {
                println!("[exit={code}]");
                if !stdout.is_empty() { print!("{}", stdout); if !stdout.ends_with('\n') { println!(); } }
                if !stderr.trim().is_empty() { eprintln!("--stderr--\n{}", stderr); }
            }
            Err(e) => eprintln!("[err] {e:#}"),
        }
    }
    Ok(())
}
