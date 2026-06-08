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

/// 结构化的一条直连规则。
/// 序列化到文件时的约定（与 PassWall direct_ip / direct_host 兼容）：
///   - enabled 行：`<value>    # <comment>`
///   - disabled 行：`# <value>    # <comment>`  （行首再加一个 # 把整行注释掉）
/// PassWall 视 `#` 开头为注释、忽略不加载，所以"禁用"就是把整行注释掉。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectEntry {
    pub value: String,
    #[serde(default)]
    pub comment: String,
    pub enabled: bool,
}

/// 解析现有文件为结构化列表。无法解析的行（不含 IP/CIDR/域名）当作"原始注释"丢回 `extras`，
/// 保存时会原样写回到文件头部，避免破坏用户已有的注释模板。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectListStructured {
    pub path: String,
    pub entries: Vec<DirectEntry>,
    /// 文件中既不是规则也不是 "# <规则>" 形式的纯注释/说明行，保留并原样写回到文件头部。
    pub extras: Vec<String>,
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

/// 看一行有没有 "本应是规则" 的可识别值；只在能解析出值时返回 Some((value, comment))。
fn parse_rule_line(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // 行内尾随注释：` # ...`
    let (head, comment) = match s.find('#') {
        Some(i) => (&s[..i], s[i + 1..].trim().to_string()),
        None => (s, String::new()),
    };
    let value = head.trim();
    if value.is_empty() {
        return None;
    }
    if normalize_entry(value).is_ok() {
        Some((value.to_string(), comment))
    } else {
        None
    }
}

pub async fn list_structured(client: &LuciClient, path: &str) -> Result<DirectListStructured> {
    let raw = client.file_read(path).await.unwrap_or_default();
    let mut entries = Vec::new();
    let mut extras = Vec::new();
    for line in raw.lines() {
        let l = line.trim_end_matches('\r');
        let trimmed = l.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        // 整行注释：尝试看是不是 "# <规则> [# 备注]" 形式 ⇒ disabled 规则
        if let Some(rest) = trimmed.strip_prefix('#') {
            let rest = rest.trim_start();
            if let Some((value, comment)) = parse_rule_line(rest) {
                entries.push(DirectEntry { value, comment, enabled: false });
                continue;
            }
            // 普通注释行原样保留
            extras.push(l.to_string());
            continue;
        }
        // 普通规则行
        if let Some((value, comment)) = parse_rule_line(trimmed) {
            entries.push(DirectEntry { value, comment, enabled: true });
        } else {
            // 解析不出来：保留为原样，避免丢数据
            extras.push(l.to_string());
        }
    }
    Ok(DirectListStructured { path: path.to_string(), entries, extras })
}

/// 整体覆盖写入直连列表。前端把全部条目（含禁用项）一次性传过来，后端拼成文件再 file_write。
pub async fn save_structured(
    client: &LuciClient,
    path: &str,
    entries: &[DirectEntry],
    extras: &[String],
) -> Result<usize> {
    let mut seen = std::collections::HashSet::new();
    let mut out = String::new();
    // 头部保留 extras（原始注释/模板）
    for e in extras {
        out.push_str(e);
        out.push('\n');
    }
    let mut written = 0usize;
    for entry in entries {
        let value = normalize_entry(&entry.value)?;
        if !seen.insert(value.clone()) {
            // 跳过重复
            continue;
        }
        let comment = entry.comment.trim();
        let line = match (entry.enabled, comment.is_empty()) {
            (true, true) => value,
            (true, false) => format!("{value}    # {comment}"),
            (false, true) => format!("# {value}"),
            (false, false) => format!("# {value}    # {comment}"),
        };
        out.push_str(&line);
        out.push('\n');
        written += 1;
    }
    client.file_write(path, &out).await?;
    Ok(written)
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
