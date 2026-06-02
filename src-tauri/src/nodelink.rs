//! 解析常见节点分享链接：vless:// / vmess:// / hysteria2://（hy2://）
//! 输出 PassWall `nodes` section 用的 key=value（字符串值，符合 UCI）。
//!
//! 这一版只覆盖最常见字段；剩余高级字段（reality short_id 等）后端原样透传。

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use serde_json::{json, Map, Value};
use url::Url;

/// 解析入口：根据 scheme 分派。
pub fn parse(link: &str) -> Result<Map<String, Value>> {
    let link = link.trim();
    if let Some(rest) = link.strip_prefix("vmess://") {
        return parse_vmess(rest);
    }
    if link.starts_with("vless://") {
        return parse_vless(link);
    }
    if link.starts_with("hysteria2://") || link.starts_with("hy2://") {
        return parse_hysteria2(link);
    }
    if link.starts_with("trojan://") {
        return parse_trojan(link);
    }
    if link.starts_with("ss://") {
        return parse_ss(link);
    }
    Err(anyhow!(
        "暂不支持的链接 scheme，目前仅支持 vmess:// vless:// hysteria2:// trojan:// ss://"
    ))
}

fn ins(m: &mut Map<String, Value>, k: &str, v: impl Into<String>) {
    m.insert(k.to_string(), Value::String(v.into()));
}

/// vmess:// = base64(JSON)
fn parse_vmess(b64: &str) -> Result<Map<String, Value>> {
    let raw = decode_b64(b64).context("vmess 链接 base64 解码失败")?;
    let v: Value = serde_json::from_slice(&raw).context("vmess JSON 解析失败")?;
    let mut m = Map::new();
    ins(&mut m, ".type", "nodes");
    ins(&mut m, "type", "Xray");
    ins(&mut m, "protocol", "vmess");
    ins(&mut m, "add_mode", "1"); // 1=手动
    let get = |k: &str| {
        v.get(k)
            .map(|x| match x {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    };
    if !get("ps").is_empty() { ins(&mut m, "remarks", get("ps")); }
    if !get("add").is_empty() { ins(&mut m, "address", get("add")); }
    if !get("port").is_empty() { ins(&mut m, "port", get("port")); }
    if !get("id").is_empty() { ins(&mut m, "uuid", get("id")); }
    if !get("aid").is_empty() { ins(&mut m, "alter_id", get("aid")); }
    if !get("scy").is_empty() { ins(&mut m, "security", get("scy")); }
    if !get("tls").is_empty() { ins(&mut m, "tls", if get("tls") == "tls" { "1" } else { "0" }); }
    if !get("sni").is_empty() { ins(&mut m, "tls_serverName", get("sni")); }
    if !get("net").is_empty() { ins(&mut m, "transport", map_transport(&get("net"))); }
    ins(&mut m, "timeout", "60");
    Ok(m)
}

fn parse_vless(link: &str) -> Result<Map<String, Value>> {
    let url = Url::parse(link).context("vless URL 解析失败")?;
    let mut m = Map::new();
    ins(&mut m, ".type", "nodes");
    ins(&mut m, "type", "Xray");
    ins(&mut m, "protocol", "vless");
    ins(&mut m, "add_mode", "1");
    if let Some(u) = url.username().to_string().strip_prefix("") {
        if !u.is_empty() { ins(&mut m, "uuid", u); }
    }
    if let Some(h) = url.host_str() { ins(&mut m, "address", h); }
    if let Some(p) = url.port() { ins(&mut m, "port", p.to_string()); }
    let frag = url
        .fragment()
        .map(|f| percent_decode(f))
        .unwrap_or_default();
    if !frag.is_empty() { ins(&mut m, "remarks", frag); }

    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "type" => ins(&mut m, "transport", map_transport(&v)),
            "security" => match v.as_ref() {
                "reality" => { ins(&mut m, "tls", "1"); ins(&mut m, "reality", "1"); }
                "tls" => ins(&mut m, "tls", "1"),
                _ => {}
            },
            "sni" => ins(&mut m, "tls_serverName", v.to_string()),
            "pbk" => ins(&mut m, "reality_publicKey", v.to_string()),
            "sid" => ins(&mut m, "reality_shortId", v.to_string()),
            "fp" => ins(&mut m, "fingerprint", v.to_string()),
            "flow" => ins(&mut m, "flow", v.to_string()),
            "encryption" => ins(&mut m, "encryption", v.to_string()),
            _ => {}
        }
    }
    ins(&mut m, "timeout", "60");
    Ok(m)
}

fn parse_hysteria2(link: &str) -> Result<Map<String, Value>> {
    let url = Url::parse(link).context("hysteria2 URL 解析失败")?;
    let mut m = Map::new();
    ins(&mut m, ".type", "nodes");
    ins(&mut m, "type", "Xray");
    ins(&mut m, "protocol", "hysteria2");
    ins(&mut m, "add_mode", "1");
    let pwd = url.username().to_string();
    if !pwd.is_empty() {
        ins(&mut m, "hysteria2_auth_password", percent_decode(&pwd));
    }
    if let Some(h) = url.host_str() { ins(&mut m, "address", h); }
    if let Some(p) = url.port() { ins(&mut m, "port", p.to_string()); }
    let frag = url.fragment().map(percent_decode).unwrap_or_default();
    if !frag.is_empty() { ins(&mut m, "remarks", frag); }
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "sni" => ins(&mut m, "tls_serverName", v.to_string()),
            "insecure" => ins(&mut m, "tls_allowInsecure", if v == "1" { "1" } else { "0" }),
            _ => {}
        }
    }
    ins(&mut m, "timeout", "60");
    Ok(m)
}

fn parse_trojan(link: &str) -> Result<Map<String, Value>> {
    let url = Url::parse(link).context("trojan URL 解析失败")?;
    let mut m = Map::new();
    ins(&mut m, ".type", "nodes");
    ins(&mut m, "type", "Xray");
    ins(&mut m, "protocol", "trojan");
    ins(&mut m, "add_mode", "1");
    let pwd = url.username().to_string();
    if !pwd.is_empty() { ins(&mut m, "password", percent_decode(&pwd)); }
    if let Some(h) = url.host_str() { ins(&mut m, "address", h); }
    if let Some(p) = url.port() { ins(&mut m, "port", p.to_string()); }
    let frag = url.fragment().map(percent_decode).unwrap_or_default();
    if !frag.is_empty() { ins(&mut m, "remarks", frag); }
    ins(&mut m, "tls", "1");
    for (k, v) in url.query_pairs() {
        if k == "sni" { ins(&mut m, "tls_serverName", v.to_string()); }
    }
    ins(&mut m, "timeout", "60");
    Ok(m)
}

fn parse_ss(link: &str) -> Result<Map<String, Value>> {
    // ss://BASE64(method:password)@host:port#name
    let rest = link.strip_prefix("ss://").unwrap();
    let (head, frag) = match rest.find('#') {
        Some(i) => (&rest[..i], percent_decode(&rest[i + 1..])),
        None => (rest, String::new()),
    };
    let (user_b64, hostport) = head
        .split_once('@')
        .ok_or_else(|| anyhow!("ss 链接缺少 @"))?;
    let dec = decode_b64(user_b64).context("ss base64 解码失败")?;
    let creds = String::from_utf8_lossy(&dec).to_string();
    let (method, password) = creds
        .split_once(':')
        .ok_or_else(|| anyhow!("ss 凭据缺少 :"))?;
    let (host, port) = hostport
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("ss host:port 解析失败"))?;
    let mut m = Map::new();
    ins(&mut m, ".type", "nodes");
    ins(&mut m, "type", "SS");
    ins(&mut m, "protocol", "ss");
    ins(&mut m, "add_mode", "1");
    ins(&mut m, "method", method);
    ins(&mut m, "password", password);
    ins(&mut m, "address", host);
    ins(&mut m, "port", port);
    if !frag.is_empty() { ins(&mut m, "remarks", frag); }
    ins(&mut m, "timeout", "60");
    Ok(m)
}

fn map_transport(v: &str) -> &'static str {
    match v {
        "tcp" | "raw" => "raw",
        "ws" => "ws",
        "grpc" => "grpc",
        "h2" | "http" => "h2",
        "quic" => "quic",
        _ => "raw",
    }
}

fn decode_b64(s: &str) -> Result<Vec<u8>> {
    // 容错：URL-safe / 标准 / 无填充
    let s = s.replace('-', "+").replace('_', "/");
    let s = s.trim_end_matches('=');
    let padding_needed = (4 - s.len() % 4) % 4;
    let padded = format!("{s}{}", "=".repeat(padding_needed));
    base64::engine::general_purpose::STANDARD
        .decode(padded.as_bytes())
        .map_err(|e| anyhow!("base64 解码错误: {e}"))
}

fn percent_decode(s: &str) -> String {
    // 简单百分号解码（不依赖额外 crate）
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = (hex(bytes[i + 1]) << 4) | hex(bytes[i + 2]);
            out.push(h);
            i += 3;
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// 把解析结果中的 `.type` 之外的字段提取为 UCI add 的 values。
pub fn into_uci_values(mut parsed: Map<String, Value>) -> Value {
    parsed.remove(".type");
    Value::Object(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vless_reality() {
        let link = "vless://abcd-1234@1.2.3.4:443?security=reality&pbk=XX&sni=apple.com&type=tcp&fp=chrome&flow=xtls-rprx-vision#node1";
        let m = parse_vless(link).unwrap();
        assert_eq!(m["address"], "1.2.3.4");
        assert_eq!(m["port"], "443");
        assert_eq!(m["reality"], "1");
        assert_eq!(m["remarks"], "node1");
    }
}
