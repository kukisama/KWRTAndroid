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

        // 1) 先 ubus 登录（最稳；不依赖 LuCI 主题/表单字段）
        client.login_ubus(&opts.username, &opts.password).await?;
        // 2) 再尝试 form login 拿 sysauth cookie（用于 WebView 直接访问 LuCI 页面）；
        //    失败不阻断 —— 真正的业务调用全走 ubus。
        if let Err(e) = client.login_form(&opts.username, &opts.password).await {
            log::warn!("form login 失败（不影响 ubus 调用）：{e:#}");
        }
        Ok(client)
    }

    async fn login_form(&mut self, user: &str, pass: &str) -> Result<()> {
        let url = self.base_url.join("cgi-bin/luci/")?;
        // 不同 LuCI 主题/版本字段名不同：
        //   - 经典 LuCI:    luci_username / luci_password
        //   - 新版 LuCI:    username / password
        // 两套都试一遍，谁先拿到 sysauth 就用谁。
        let candidates: [&[(&str, &str)]; 2] = [
            &[("luci_username", user), ("luci_password", pass)],
            &[("username", user), ("password", pass)],
        ];

        let mut last_status: Option<u16> = None;
        let mut last_body_head: Option<String> = None;

        for form in candidates.iter() {
            log::debug!("form login POST {url} fields={:?}", form.iter().map(|(k,_)| k).collect::<Vec<_>>());
            let resp = self
                .http
                .post(url.clone())
                .form(form)
                .send()
                .await
                .with_context(|| format!("无法访问 {url}（路由器是否可达？端口是否正确？）"))?;
            let status = resp.status();
            let cookies: Vec<(String, String)> = resp
                .cookies()
                .map(|c| (c.name().to_string(), c.value().to_string()))
                .collect();
            log::debug!("form login resp status={status} cookies={cookies:?} url={}", resp.url());

            for (name, value) in &cookies {
                if name.starts_with("sysauth") {
                    self.sysauth = Some(format!("{name}={value}"));
                    return Ok(());
                }
            }

            last_status = Some(status.as_u16());
            let body = resp.text().await.unwrap_or_default();
            let head: String = body.chars().take(400).collect();
            log::debug!("form login body head: {head}");
            last_body_head = Some(head);

            // 再拿一次根路径，看 cookie_store 是否在 302 之后才落
            if let Ok(probe) = self.http.get(self.base_url.clone()).send().await {
                for cookie in probe.cookies() {
                    if cookie.name().starts_with("sysauth") {
                        self.sysauth = Some(format!("{}={}", cookie.name(), cookie.value()));
                        return Ok(());
                    }
                }
            }
        }

        // 兜底：如果 ubus 已经登录成功，直接把 ubus session 当作 sysauth cookie 注入
        if let Some(sess) = &self.ubus_session {
            log::warn!("form login 没拿到 sysauth，用 ubus session 兜底注入 sysauth cookie");
            self.sysauth = Some(format!("sysauth_http={sess}"));
            return Ok(());
        }

        Err(anyhow!(
            "表单登录失败：未拿到 sysauth Cookie（HTTP {status:?}，body 前 400 字: {body:?}）。\
             可能原因：1) LuCI 表单字段名与本程序不匹配；2) 登录页要求 CSRF token；3) 账号/密码错误；\
             4) 该 LuCI 主题禁用了表单登录。",
            status = last_status,
            body = last_body_head
        ))
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

    /// uci set：写入若干 option（不会自动 commit）。
    pub async fn uci_set(
        &self,
        config: &str,
        section: &str,
        values: Value,
    ) -> Result<Value> {
        self.ubus_call(
            "uci",
            "set",
            json!({ "config": config, "section": section, "values": values }),
        )
        .await
    }

    /// uci add：新增匿名 section，可附带 values。返回 result 里通常带 `section` = 新名。
    pub async fn uci_add(
        &self,
        config: &str,
        section_type: &str,
        name: Option<&str>,
        values: Value,
    ) -> Result<Value> {
        let mut p = serde_json::Map::new();
        p.insert("config".into(), json!(config));
        p.insert("type".into(), json!(section_type));
        if let Some(n) = name {
            p.insert("name".into(), json!(n));
        }
        p.insert("values".into(), values);
        self.ubus_call("uci", "add", Value::Object(p)).await
    }

    /// uci delete section 或 option。
    pub async fn uci_delete(
        &self,
        config: &str,
        section: &str,
        option: Option<&str>,
    ) -> Result<Value> {
        let mut p = serde_json::Map::new();
        p.insert("config".into(), json!(config));
        p.insert("section".into(), json!(section));
        if let Some(o) = option {
            p.insert("option".into(), json!(o));
        }
        self.ubus_call("uci", "delete", Value::Object(p)).await
    }

    /// uci commit：写盘。
    pub async fn uci_commit(&self, config: &str) -> Result<Value> {
        self.ubus_call("uci", "commit", json!({ "config": config }))
            .await
    }

    /// 通过 file.exec 调用 shell 命令；某些固件可能禁用，调用方需容错。
    pub async fn file_exec(&self, command: &str, params: Vec<&str>) -> Result<Value> {
        self.ubus_call(
            "file",
            "exec",
            json!({ "command": command, "params": params }),
        )
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
