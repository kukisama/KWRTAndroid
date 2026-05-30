//! Windows 凭据管理器封装（keyring crate → Credential Manager）。
//! 仅在用户主动勾选"记住密码"时写入；删除时彻底擦除。
use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE_PREFIX: &str = "kwrt-controller";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCredential {
    pub host: String,
    pub scheme: String,
    pub port: Option<u16>,
    pub username: String,
    pub accept_invalid_certs: bool,
}

/// 凭据 key：把 host+user 拼进 service 名，避免多账号串号。
fn service_name(host: &str, user: &str) -> String {
    format!("{SERVICE_PREFIX}::{host}::{user}")
}

pub fn save_password(host: &str, user: &str, password: &str) -> Result<()> {
    let entry = Entry::new(&service_name(host, user), user).context("创建凭据条目失败")?;
    entry.set_password(password).context("写入 Windows 凭据失败")
}

pub fn load_password(host: &str, user: &str) -> Result<Option<String>> {
    let entry = Entry::new(&service_name(host, user), user)?;
    match entry.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_password(host: &str, user: &str) -> Result<()> {
    let entry = Entry::new(&service_name(host, user), user)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
