//! LuCI / ubus 客户端：表单登录拿 sysauth Cookie，并通过 /ubus 调用 RPC。
use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, COOKIE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use url::Url;

const ANON_SESSION: &str = "00000000000000000000000000000000";

/// 一个已登录的 LuCI 会话。线程安全；可被多个命令并发使用（reqwest::Client 本身是 Arc）。
#[derive(Clone)]
pub struct LuciClient {
    pub base_url: Url,
    pub http: Client,
    pub sysauth: Option<String>,
    pub ubus_session: Option<String>,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectOptions {
    pub host: String,
    /// http 或 https；若为空默认 http
    pub scheme: Option<String>,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    /// 仅当用户主动允许时才允许自签证书
    pub accept_invalid_certs: bool,
    pub timeout_secs: Option<u64>,
}

impl LuciClient {
    pub fn build_base(opts: &ConnectOptions) -> Result<Url> {
        let scheme = opts.scheme.as_deref().unwrap_or("http");
        if scheme != "http" && scheme != "https" {
            return Err(anyhow!("scheme 仅支持 http 或 https"));
        }
        let host = opts.host.trim();
        if host.is_empty() {
            return Err(anyhow!("host 不能为空"));
        }
        // 防止用户把整段 URL 粘进来
        if host.contains("://") || host.contains('/') {
            return Err(anyhow!("host 只填 IP 或域名，不要带 http:// 或路径"));
        }
        let url_str = match opts.port {
            Some(p) => format!("{}://{}:{}/", scheme, host, p),
            None => format!("{}://{}/", scheme, host),
        };
        Url::parse(&url_str).context("base URL 解析失败")
    }

    pub async fn connect(opts: ConnectOptions) -> Result<Self> {
        let base = Self::build_base(&opts)?;
        let timeout = Duration::from_secs(opts.timeout_secs.unwrap_or(10));

        let mut builder = Client::builder()
            .cookie_store(true)
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(5))
            .user_agent("KWRT-Controller/0.1");
        if opts.accept_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let http = builder.build().context("构建 HTTP 客户端失败")?;

        let mut client = LuciClient {
            base_url: base,
            http,
            sysauth: None,
            ubus_session: None,
            username: opts.username.clone(),
        };

        // 1) 表单登录拿 sysauth（用于 /cgi-bin/luci 的所有访问）
        client.login_form(&opts.username, &opts.password).await?;
        // 2) ubus 会话登录（用于程序化调用）
        client.login_ubus(&opts.username, &opts.password).await?;
        Ok(client)
    }

    async fn login_form(&mut self, user: &str, pass: &str) -> Result<()> {
        let url = self.base_url.join("cgi-bin/luci/")?;
        // LuCI 默认登录表单字段：luci_username / luci_password
        let form = [("luci_username", user), ("luci_password", pass)];
        let resp = self
            .http
            .post(url.clone())
            .form(&form)
            .send()
            .await
            .with_context(|| format!("无法访问 {url}（路由器是否可达？端口是否正确？）"))?;
        // 成功标志：响应头里能拿到 sysauth*=… Cookie，或重定向到 /admin
        let mut found: Option<String> = None;
        for cookie in resp.cookies() {
            let name = cookie.name();
            if name.starts_with("sysauth") {
                found = Some(format!("{}={}", name, cookie.value()));
                break;
            }
        }
        if found.is_none() {
            // 个别主题会在 302 之后才下发；reqwest 默认会跟随重定向，cookie 仍会写入 store。
            // 这里再用 cookie_store 查一次。
            // reqwest 0.12 没有公开 cookie_store；通过对根路径再发一次请求来探测。
            let probe = self.http.get(self.base_url.clone()).send().await?;
            for cookie in probe.cookies() {
                let name = cookie.name();
                if name.starts_with("sysauth") {
                    found = Some(format!("{}={}", name, cookie.value()));
                    break;
                }
            }
        }
        let cookie =
            found.ok_or_else(|| anyhow!("表单登录失败：未拿到 sysauth Cookie（密码是否正确？）"))?;
        self.sysauth = Some(cookie);
        Ok(())
    }

    async fn login_ubus(&mut self, user: &str, pass: &str) -> Result<()> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "call",
            "params": [
                ANON_SESSION, "session", "login",
                { "username": user, "password": pass, "timeout": 0 }
            ]
        });
        let url = self.base_url.join("ubus")?;
        let resp = self.http.post(url).json(&body).send().await?;
        let v: Value = resp
            .json()
            .await
            .context("ubus 响应不是 JSON（可能 /ubus 未启用：检查 uhttpd 配置）")?;
        if let Some(err) = v.get("error") {
            return Err(anyhow!("ubus 登录错误：{}", err));
        }
        // result = [status_code, { ubus_rpc_session: "..." }]
        let result = v
            .get("result")
            .and_then(|r| r.as_array())
            .ok_or_else(|| anyhow!("ubus 响应缺少 result：{v}"))?;
        let code = result.first().and_then(|x| x.as_i64()).unwrap_or(-1);
        if code != 0 {
            return Err(anyhow!(
                "ubus session.login 失败 (code={code})：账号密码错误或 ACL 不允许登录。原始响应：{v}"
            ));
        }
        let session = result
            .get(1)
            .and_then(|d| d.get("ubus_rpc_session"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow!("ubus 响应缺少 ubus_rpc_session：{v}"))?;
        self.ubus_session = Some(session.to_string());
        Ok(())
    }

    /// 通用 ubus 调用。返回 result 数组里第二项（payload）。
    pub async fn ubus_call(&self, object: &str, method: &str, params: Value) -> Result<Value> {
        let session = self
            .ubus_session
            .as_deref()
            .ok_or_else(|| anyhow!("未登录 ubus"))?;
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "call",
            "params": [session, object, method, params]
        });
        let url = self.base_url.join("ubus")?;
        let resp = self.http.post(url).json(&body).send().await?;
        let v: Value = resp.json().await?;
        if let Some(err) = v.get("error") {
            return Err(anyhow!("ubus 调用 {object}.{method} 出错：{err}"));
        }
        let result = v
            .get("result")
            .and_then(|r| r.as_array())
            .ok_or_else(|| anyhow!("ubus 响应缺少 result：{v}"))?;
        let code = result.first().and_then(|x| x.as_i64()).unwrap_or(-1);
        if code != 0 {
            // ubus 状态码：
            // 0 = OK, 2 = INVALID_COMMAND, 4 = NOT_FOUND, 5 = NO_DATA, 6 = PERMISSION_DENIED, 7 = TIMEOUT
            return Err(anyhow!(
                "ubus {object}.{method} 返回非 0 状态码 {code}（4=对象不存在，6=权限拒绝）"
            ));
        }
        Ok(result.get(1).cloned().unwrap_or(Value::Null))
    }

    /// 读 UCI 配置（整份 / 单 section / 单 option）。
    pub async fn uci_get(
        &self,
        config: &str,
        section: Option<&str>,
        option: Option<&str>,
    ) -> Result<Value> {
        let mut p = serde_json::Map::new();
        p.insert("config".into(), json!(config));
        if let Some(s) = section {
            p.insert("section".into(), json!(s));
        }
        if let Some(o) = option {
            p.insert("option".into(), json!(o));
        }
        self.ubus_call("uci", "get", Value::Object(p)).await
    }

    /// 列出系统所有 UCI 配置名。
    pub async fn uci_configs(&self) -> Result<Vec<String>> {
        let v = self.ubus_call("uci", "configs", json!({})).await?;
        Ok(v.get("configs")
            .and_then(|c| c.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn file_read(&self, path: &str) -> Result<String> {
        let v = self
            .ubus_call("file", "read", json!({ "path": path }))
            .await?;
        Ok(v.get("data")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default())
    }

    pub async fn file_write(&self, path: &str, data: &str) -> Result<()> {
        // file.write 期望 { path, data, [mode], [append], [base64] }
        self.ubus_call(
            "file",
            "write",
            json!({ "path": path, "data": data, "mode": 0o644 }),
        )
        .await?;
        Ok(())
    }

    pub async fn file_stat(&self, path: &str) -> Result<Option<Value>> {
        match self
            .ubus_call("file", "stat", json!({ "path": path }))
            .await
        {
            Ok(v) => Ok(Some(v)),
            Err(_) => Ok(None),
        }
    }

    /// 调用 init 脚本动作（start/stop/restart/reload/enable/disable）。
    pub async fn rc_init(&self, name: &str, action: &str) -> Result<Value> {
        self.ubus_call("rc", "init", json!({ "name": name, "action": action }))
            .await
    }

    pub async fn system_board(&self) -> Result<Value> {
        self.ubus_call("system", "board", json!({})).await
    }

    /// 在不依赖 reqwest 内部 cookie store 的情况下，按需手动构造一次带 sysauth 的请求头。
    /// 当前未直接使用，预留给后续 WebView Cookie 注入或老 JSON-RPC 兜底。
    #[allow(dead_code)]
    pub fn manual_cookie_header(&self) -> Option<HeaderMap> {
        self.sysauth.as_ref().and_then(|c| {
            let mut h = HeaderMap::new();
            HeaderValue::from_str(c).ok().map(|v| {
                h.insert(COOKIE, v);
                h
            })
        })
    }
}

/// 包装成 Arc，方便存到 Tauri State。
pub type SharedClient = Arc<tokio::sync::RwLock<Option<LuciClient>>>;
