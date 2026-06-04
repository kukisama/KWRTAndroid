// 初始化 U 盘：umount -> mkfs.ext4 -> 挂载 -> 写 fstab -> 启动 fstab 服务
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
    eprintln!("[ok] logged in");

    // 顺序执行；用单独的 shell() 调用，便于看清每一步状态
    let steps: &[(&str, &str)] = &[
        // 1. 卸掉自动挂载（如果在用）
        ("pre-umount",
         "umount /mnt/sda1 2>&1; umount /dev/sda1 2>&1; true"),
        // 2. 二次确认设备存在
        ("pre-check",
         "ls -l /dev/sda1 && blkid /dev/sda1 2>/dev/null || true"),
        // 3. 格式化 ext4（-F 不交互；-L 设卷标；-m 0 不预留 root 空间）
        ("mkfs",
         "mkfs.ext4 -F -L KWRT_DATA -m 0 -O ^has_journal /dev/sda1 2>&1 | tail -20"),
        // 说明：去掉 journal 可显著降 U 盘写入量、延长寿命；只放二进制不需要 journal
        // 4. 拿 UUID
        ("uuid",
         "blkid /dev/sda1"),
        // 5. 挂载
        ("mount",
         "mkdir -p /mnt/sda1 && mount -t ext4 /dev/sda1 /mnt/sda1 && df -h /mnt/sda1"),
        // 6. 建目录
        ("mkdirs",
         "mkdir -p /mnt/sda1/bin /mnt/sda1/share && ls -la /mnt/sda1"),
        // 7. 写 fstab：清掉旧的同 UUID 条目再加一条
        ("uci-fstab",
         "UUID=$(blkid /dev/sda1 | sed -n 's/.* UUID=\"\\([^\"]*\\)\".*/\\1/p'); \
          echo \"UUID=$UUID\"; \
          # 删除所有 mount 段（我们当前 fstab 是空的，安全）\n\
          while uci -q delete fstab.@mount[0]; do :; done; \
          uci add fstab mount >/dev/null; \
          uci set fstab.@mount[-1].uuid=\"$UUID\"; \
          uci set fstab.@mount[-1].target='/mnt/sda1'; \
          uci set fstab.@mount[-1].fstype='ext4'; \
          uci set fstab.@mount[-1].options='rw,noatime,nodiratime'; \
          uci set fstab.@mount[-1].enabled='1'; \
          uci commit fstab; \
          uci show fstab"),
        // 8. 启用 fstab 服务并立刻刷新一次
        ("enable-fstab",
         "/etc/init.d/fstab enable; /etc/init.d/fstab restart 2>&1; sleep 1; mount | grep sda"),
        // 9. 验证：写一个测试可执行文件，确认 ext4 上 exec 位能保留
        ("verify-exec",
         "cat >/mnt/sda1/bin/.exectest <<'EOF'\n#!/bin/sh\necho ok-from-usb\nEOF\n\
          chmod +x /mnt/sda1/bin/.exectest && \
          /mnt/sda1/bin/.exectest && \
          rm -f /mnt/sda1/bin/.exectest"),
        // 10. 最终汇报
        ("final-summary",
         "echo '--- df ---'; df -h /overlay /mnt/sda1; \
          echo '--- fstab ---'; cat /etc/config/fstab; \
          echo '--- target dir ---'; ls -la /mnt/sda1/bin"),
    ];

    for (label, script) in steps {
        println!("\n========== {label} ==========");
        match c.shell(script).await {
            Ok((code, stdout, stderr)) => {
                println!("[exit={code}]");
                if !stdout.is_empty() { print!("{}", stdout); if !stdout.ends_with('\n') { println!(); } }
                if !stderr.trim().is_empty() { eprintln!("--stderr--\n{}", stderr); }
                if code != 0 {
                    eprintln!("[abort] step '{label}' failed, stopping.");
                    return Ok(());
                }
            }
            Err(e) => { eprintln!("[err] {e:#}"); return Ok(()); }
        }
    }
    println!("\n[done] U 盘已格式化为 ext4 并自动挂载到 /mnt/sda1");
    println!("[done] 把 PassWall 的 Sing-Box 程序路径改为: /mnt/sda1/bin/sing-box");
    Ok(())
}
