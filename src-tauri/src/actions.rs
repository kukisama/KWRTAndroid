//! 快捷动作：读/写直连列表、重启 PassWall。
use crate::client::LuciClient;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectListResult {
    pub path: String,
    pub entries: Vec<String>,
    /// 原文（包含注释行），便于"原样回写"避免破坏用户已有注释。
    pub raw: String,
}

pub async fn read_list(client: &LuciClient, path: &str) -> Result<DirectListResult> {
    let raw = client.file_read(path).await?;
    let entries: Vec<String> = raw
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    Ok(DirectListResult {
        path: path.to_string(),
        entries,
        raw,
    })
}

/// 简单校验：IPv4 / IPv4-CIDR / IPv6 / 域名。返回归一化字符串。
pub fn normalize_entry(input: &str) -> Result<String> {
    let s = input.trim();
    if s.is_empty() {
        return Err(anyhow!("条目为空"));
    }
    // 简单白名单：仅允许 [a-zA-Z0-9._:/-]
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '-'))
    {
        return Err(anyhow!("非法字符（仅允许字母数字 . _ : / -）"));
    }
    if s.len() > 253 {
        return Err(anyhow!("条目过长"));
    }
    Ok(s.to_string())
}

/// 追加一条 IP / 域名到直连列表，去重；返回是否实际写入。
pub async fn add_entry(client: &LuciClient, path: &str, entry: &str) -> Result<bool> {
    let entry = normalize_entry(entry)?;
    let raw = client.file_read(path).await.unwrap_or_default();
    // 判重：去掉注释和空白后再比较
    let already = raw.lines().any(|l| l.trim() == entry);
    if already {
        return Ok(false);
    }
    let mut new_raw = raw;
    if !new_raw.is_empty() && !new_raw.ends_with('\n') {
        new_raw.push('\n');
    }
    new_raw.push_str(&entry);
    new_raw.push('\n');
    client.file_write(path, &new_raw).await?;
    Ok(true)
}

/// 删除一条
pub async fn remove_entry(client: &LuciClient, path: &str, entry: &str) -> Result<bool> {
    let target = normalize_entry(entry)?;
    let raw = client.file_read(path).await.unwrap_or_default();
    let mut changed = false;
    let new_raw: String = raw
        .lines()
        .filter(|l| {
            let keep = l.trim() != target;
            if !keep {
                changed = true;
            }
            keep
        })
        .collect::<Vec<_>>()
        .join("\n");
    if changed {
        let mut s = new_raw;
        if !s.is_empty() && !s.ends_with('\n') {
            s.push('\n');
        }
        client.file_write(path, &s).await?;
    }
    Ok(changed)
}

pub async fn restart_passwall(client: &LuciClient, init_name: &str) -> Result<()> {
    client.rc_init(init_name, "restart").await?;
    Ok(())
}

pub async fn reload_passwall(client: &LuciClient, init_name: &str) -> Result<()> {
    client.rc_init(init_name, "reload").await?;
    Ok(())
}
