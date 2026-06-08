//! 简易远端文件浏览器：列目录 + 读小文件 head。
//!
//! 只读、不做写入；用于让用户"下探看分区里有什么"。
//! 读文件大小硬上限 64KB，避免把大文件灌满内存。

use crate::client::LuciClient;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

const MAX_PREVIEW_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsEntry {
    pub name: String,
    /// "dir" | "file" | "link" | "other"
    pub kind: String,
    pub size: u64,
    /// 链接目标（kind=link 时）
    #[serde(default)]
    pub link_target: String,
    /// rwxrwxrwx 字符串（best-effort）
    #[serde(default)]
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsListing {
    pub path: String,
    pub entries: Vec<FsEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPreview {
    pub path: String,
    pub size: u64,
    /// 是否被截断（> MAX_PREVIEW_BYTES）
    pub truncated: bool,
    /// 文本内容（若非可打印则给十六进制头）
    pub text: String,
    pub is_binary: bool,
}

pub async fn list_dir(client: &LuciClient, path: &str) -> Result<FsListing> {
    if !path.starts_with('/') {
        return Err(anyhow!("路径必须以 / 开头"));
    }
    // 用 ls -la 解析；OpenWrt busybox ls 输出列：mode links user group size date time name (-> target)
    // 注意：BusyBox ls 不支持 --color=never，给了会直接报 unrecognized option 然后退出。
    let script = format!(
        "P='{p}'; if [ -d \"$P\" ]; then ls -la \"$P\" 2>/dev/null; else echo NOT_A_DIR; fi",
        p = path.replace('\'', "")
    );
    let (code, out, err) = client.shell(&script).await?;
    if code != 0 {
        return Err(anyhow!("ls 失败 (exit={code}) stderr={err}"));
    }
    if out.contains("NOT_A_DIR") {
        return Err(anyhow!("不是目录: {path}"));
    }
    let mut entries: Vec<FsEntry> = Vec::new();
    for line in out.lines() {
        if line.starts_with("total ") { continue; }
        // BusyBox ls -la 字段间是多空格，必须用 split_whitespace 才能正确切；
        // splitn(N, char::is_whitespace) 会把每个空白都当切点，连续空格直接吞光分段配额。
        let mut it = line.split_whitespace();
        let mode = match it.next() { Some(s) => s.to_string(), None => continue };
        let _links = match it.next() { Some(s) => s, None => continue };
        let _user  = match it.next() { Some(s) => s, None => continue };
        let _group = match it.next() { Some(s) => s, None => continue };
        let size: u64 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        // 接下来 3 段是 month day year-or-time（busybox 不保证恰好 3 段，但常见）
        let _t1 = it.next(); let _t2 = it.next(); let _t3 = it.next();
        // 剩余部分拼回去作为 name；可能含 " -> target"
        let rest: Vec<&str> = it.collect();
        if rest.is_empty() { continue; }
        let name_part = rest.join(" ");
        let (name, link_target) = if let Some((a, b)) = name_part.split_once(" -> ") {
            (a.to_string(), b.to_string())
        } else {
            (name_part, String::new())
        };
        if name == "." || name == ".." { continue; }
        let kind = match mode.chars().next().unwrap_or('-') {
            'd' => "dir",
            'l' => "link",
            '-' => "file",
            _ => "other",
        }.to_string();
        entries.push(FsEntry { name, kind, size, link_target, mode });
    }
    entries.sort_by(|a, b| {
        // 目录优先 + 名字
        let oa = if a.kind == "dir" { 0 } else { 1 };
        let ob = if b.kind == "dir" { 0 } else { 1 };
        oa.cmp(&ob).then(a.name.cmp(&b.name))
    });
    Ok(FsListing { path: path.into(), entries })
}

pub async fn preview(client: &LuciClient, path: &str) -> Result<FsPreview> {
    if !path.starts_with('/') {
        return Err(anyhow!("路径必须以 / 开头"));
    }
    // 单独取大小：BusyBox 上 file 命令往往缺失，不能和 wc 用 && 串，否则 file 失败会让整体非零。
    let (sz_code, sz_out, _sz_err) = client.shell(&format!(
        "wc -c < '{p}' 2>/dev/null || echo 0",
        p = path.replace('\'', "")
    )).await?;
    if sz_code != 0 {
        return Err(anyhow!("读取大小失败 (exit={sz_code})"));
    }
    let size: u64 = sz_out.trim().lines().next().unwrap_or("0").trim().parse().unwrap_or(0);
    // 启发判断二进制：head 出 NUL 字节即视为二进制（file 命令在 BusyBox 通常缺失，不依赖）
    let (_pk_code, peek, _e) = client.shell(&format!(
        "head -c 4096 '{p}' 2>/dev/null | tr -d '\\0' | wc -c",
        p = path.replace('\'', "")
    )).await?;
    let printable_bytes: u64 = peek.trim().parse().unwrap_or(0);
    let sample_size = size.min(4096);
    // 若可打印字节数 < 样本的 95%，认为是二进制
    let is_binary = sample_size > 0 && (printable_bytes as f64) < (sample_size as f64) * 0.95;

    let truncated = size as usize > MAX_PREVIEW_BYTES;
    let text = if is_binary {
        let (_c, out, _e) = client.shell(&format!(
            "head -c 256 '{p}' 2>/dev/null | od -An -tx1 | tr -s ' ' | head -16",
            p = path.replace('\'', "")
        )).await?;
        out
    } else {
        let cap = MAX_PREVIEW_BYTES;
        let (_c, out, _e) = client.shell(&format!(
            "head -c {cap} '{p}' 2>/dev/null",
            p = path.replace('\'', "")
        )).await?;
        out
    };
    Ok(FsPreview { path: path.into(), size, truncated, text, is_binary })
}
