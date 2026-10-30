//! 内嵌 SSO：浏览器登录（OIDC 授权码 + PKCE，Public client）→ 回环回调 →
//! 用 id_token 向我们的 provisioning 端点换一把 Key → 直接写进配置。
//!
//! 设计要点（SPEC §1.7）：
//! - **不导流到 cc.hsfp.cn**：登录、领 Key 全在本程序内完成；
//! - 凭据（client_secret / PG 密码）一律不进仓库、不进日志：这里是 **Public client + PKCE**，没有 secret；
//! - 回环回调固定 `http://127.0.0.1:17321/callback`（可被环境变量覆盖，方便自测）；
//! - 拿到的 Key 只回传前端、只写进隔离配置，**日志里只有掩码**；
//! - 失败给出可读原因，前端自动退回「手动粘贴 Key」。

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

/// 这些是**公开信息**（Public client / 公开端点），不是凭据；可用环境变量覆盖，便于联调与自托管。
fn issuer() -> String {
    std::env::var("AIQUICK_OIDC_ISSUER")
        .unwrap_or_else(|_| "https://auth.hsfp.cn/application/o/aiquick/".to_string())
}
fn client_id() -> String {
    std::env::var("AIQUICK_OIDC_CLIENT_ID").unwrap_or_else(|_| "aiquick-desktop".to_string())
}
fn provision_url() -> String {
    std::env::var("AIQUICK_PROVISION_URL")
        .unwrap_or_else(|_| "https://setup.hsfp.cn/api/provision-key".to_string())
}
fn redirect_uri() -> String {
    std::env::var("AIQUICK_REDIRECT_URI")
        .unwrap_or_else(|_| "http://127.0.0.1:17321/callback".to_string())
}
fn token_group() -> String {
    std::env::var("AIQUICK_TOKEN_GROUP").unwrap_or_else(|_| "半价".to_string())
}

fn rand_token(len: usize) -> String {
    let mut buf = vec![0u8; len];
    if getrandom::getrandom(&mut buf).is_err() {
        // 极端情况下退化为时间/pid 混合，仍然比固定值强；PKCE 主要防拦截而非保密
        let seed = format!(
            "{}{}{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            std::process::id(),
            len
        );
        for (i, b) in seed.bytes().enumerate() {
            buf[i % len] ^= b;
        }
    }
    URL_SAFE_NO_PAD.encode(buf)
}

/// PKCE：verifier（43~128 字符）与它的 S256 challenge。
pub fn pkce_pair() -> (String, String) {
    let verifier = rand_token(48); // 64 个 base64url 字符
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                out.push(u8::from_str_radix(hex, 16).unwrap_or(b'?'));
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// 构造授权 URL（独立成函数便于单测）。
pub fn build_authorize_url(base: &str, client: &str, redirect: &str, challenge: &str, state: &str) -> String {
    format!(
        "{base}{}client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        if base.contains('?') { "&" } else { "?" },
        urlencode(client),
        urlencode(redirect),
        urlencode("openid profile email"),
        urlencode(challenge),
        urlencode(state),
    )
}

/// 从回调请求行里取 `code` 与 `state`（独立成函数便于单测）。
pub fn parse_callback(request_line: &str) -> Result<(String, String), String> {
    // e.g. "GET /callback?code=abc&state=xyz HTTP/1.1"
    let path = request_line.split_whitespace().nth(1).ok_or("回调请求格式不对")?;
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut code = None;
    let mut state = None;
    let mut err = None;
    for kv in query.split('&') {
        let (k, v) = match kv.split_once('=') {
            Some(x) => x,
            None => continue,
        };
        match k {
            "code" => code = Some(urldecode(v)),
            "state" => state = Some(urldecode(v)),
            "error" => err = Some(urldecode(v)),
            "error_description" => err = Some(urldecode(v)),
            _ => {}
        }
    }
    if let Some(e) = err {
        return Err(format!("登录被拒绝：{e}"));
    }
    match (code, state) {
        (Some(c), Some(s)) if !c.is_empty() => Ok((c, s)),
        _ => Err("回调里没有 code/state".to_string()),
    }
}

fn curl(args: &[&str]) -> Result<(u16, String), String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sS", "-m", "30"]);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| format!("调用 curl 失败：{e}"))?;
    let body = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        return Err(format!("网络请求失败：{}", body.trim()));
    }
    Ok((200, body))
}

fn discover() -> Result<(String, String), String> {
    let base = issuer();
    let url = format!("{}/.well-known/openid-configuration", base.trim_end_matches('/'));
    let (_, body) = curl(&[&url])?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| "登录服务返回的不是 JSON，可能被网络拦截".to_string())?;
    let auth = v
        .get("authorization_endpoint")
        .and_then(|x| x.as_str())
        .ok_or("登录服务没给 authorization_endpoint")?
        .to_string();
    let token = v
        .get("token_endpoint")
        .and_then(|x| x.as_str())
        .ok_or("登录服务没给 token_endpoint")?
        .to_string();
    Ok((auth, token))
}

fn exchange_code(token_endpoint: &str, code: &str, verifier: &str) -> Result<String, String> {
    let form = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
        urlencode(code),
        urlencode(&redirect_uri()),
        urlencode(&client_id()),
        urlencode(verifier),
    );
    let (_, body) = curl(&[
        "-X",
        "POST",
        token_endpoint,
        "-H",
        "Content-Type: application/x-www-form-urlencoded",
        "--data-binary",
        &form,
    ])?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| format!("换取令牌失败：{}", body.trim()))?;
    if let Some(err) = v.get("error").and_then(|x| x.as_str()) {
        let desc = v
            .get("error_description")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        return Err(format!("登录失败（{err}）{desc}"));
    }
    v.get("id_token")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "登录服务没有返回 id_token".to_string())
}

fn provision(id_token: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "id_token": id_token,
        "group": token_group(),
        "name": "aiquick",
    })
    .to_string();
    let (_, resp) = curl(&[
        "-X",
        "POST",
        &provision_url(),
        "-H",
        "Content-Type: application/json",
        "--data-binary",
        &body,
    ])?;
    let v: serde_json::Value = serde_json::from_str(&resp)
        .map_err(|_| format!("领取 Key 失败（服务返回非 JSON）：{}", recut(&resp)))?;
    if let Some(key) = v.get("key").and_then(|x| x.as_str()) {
        if key.starts_with("sk-") {
            return Ok(key.to_string());
        }
    }
    let msg = v
        .get("message")
        .and_then(|x| x.as_str())
        .unwrap_or("服务没有返回可用的 Key");
    Err(format!("领取 Key 失败：{msg}"))
}

fn recut(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 200 {
        t.chars().take(200).collect::<String>() + "…"
    } else {
        t.to_string()
    }
}

/// 自检信息：真的去拉一次 discovery，证明登录服务可达、PKCE 参数齐。
pub fn debug_info() -> Result<serde_json::Value, String> {
    let (auth, token) = discover()?;
    let (verifier, challenge) = pkce_pair();
    Ok(serde_json::json!({
        "issuer": issuer(),
        "client_id": client_id(),
        "redirect_uri": redirect_uri(),
        "authorization_endpoint": auth,
        "token_endpoint": token,
        "provision_url": provision_url(),
        "token_group": token_group(),
        "pkce_verifier_len": verifier.chars().count(),
        "pkce_challenge_len": challenge.chars().count(),
        "authorize_url": build_authorize_url(&auth, &client_id(), &redirect_uri(), &challenge, "state-demo"),
    }))
}

/// 完整流程：起回环回调 → 打开浏览器 → 等 code → 换 id_token → 换 Key。
/// `open` 由调用方给（Tauri 用 opener 插件打开系统浏览器；单测可注入假实现）。
pub fn obtain_key(open: &dyn Fn(&str) -> Result<(), String>, timeout: Duration) -> Result<String, String> {
    let (verifier, challenge) = pkce_pair();
    let state = rand_token(24);
    let redirect = redirect_uri();
    let port: u16 = redirect
        .rsplit(':')
        .next()
        .and_then(|s| s.split('/').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(17321);

    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("本地回调端口 {port} 被占用或被拦截（{e}），可以关掉占用它的程序再试"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("设置回调监听失败：{e}"))?;

    let (auth_base, token_endpoint) = discover()?;
    let url = build_authorize_url(&auth_base, &client_id(), &redirect, &challenge, &state);
    open(&url)?;

    let start = Instant::now();
    let code = loop {
        if start.elapsed() > timeout {
            return Err("等了太久没收到登录结果，可以重试，或直接手动粘贴 Key".to_string());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let req = read_request(&mut stream);
                let line = req.lines().next().unwrap_or("");
                match parse_callback(line) {
                    Ok((code, got_state)) => {
                        // state 必须一致，防 CSRF
                        if got_state != state {
                            let _ = respond(&mut stream, false);
                            return Err("登录回调校验失败（state 不匹配），请重试".to_string());
                        }
                        let _ = respond(&mut stream, true);
                        break code;
                    }
                    Err(e) => {
                        let _ = respond(&mut stream, false);
                        // 浏览器预检/favicon 之类的请求：继续等真正的回调
                        if !line.starts_with("GET /callback") {
                            std::thread::sleep(Duration::from_millis(150));
                            continue;
                        }
                        return Err(e);
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(180));
            }
            Err(e) => return Err(format!("等待登录回调失败：{e}")),
        }
    };

    let id_token = exchange_code(&token_endpoint, &code, &verifier)?;
    provision(&id_token)
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut buf = [0u8; 2048];
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    match stream.read(&mut buf) {
        Ok(n) => String::from_utf8_lossy(&buf[..n]).to_string(),
        Err(_) => String::new(),
    }
}

fn respond(stream: &mut TcpStream, ok: bool) -> std::io::Result<()> {
    let (title, body) = if ok {
        ("登录成功", "已经拿到 Key，回到「AI 快速配置」即可。这个页面可以关掉了。")
    } else {
        ("登录没成功", "回到「AI 快速配置」重试，或手动粘贴 Key。")
    };
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>{title}</title><body style=\"font:16px system-ui;padding:40px;line-height:1.7\"><h2>{title}</h2><p>{body}</p></body>"
    );
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    stream.write_all(resp.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_shapes() {
        let (v, c) = pkce_pair();
        assert!((43..=128).contains(&v.chars().count()), "verifier 长度不合规");
        // base64url(sha256) 固定 43 字符
        assert_eq!(c.chars().count(), 43);
        assert!(!c.contains('='), "challenge 不能有 padding");
        let (v2, c2) = pkce_pair();
        assert_ne!(v, v2);
        assert_ne!(c, c2);
    }

    #[test]
    fn authorize_url_has_pkce() {
        let u = build_authorize_url(
            "https://auth.example/authorize",
            "aiquick-desktop",
            "http://127.0.0.1:17321/callback",
            "CHAL",
            "ST",
        );
        assert!(u.starts_with("https://auth.example/authorize?"));
        assert!(u.contains("response_type=code"));
        assert!(u.contains("code_challenge=CHAL"));
        assert!(u.contains("code_challenge_method=S256"));
        assert!(u.contains("state=ST"));
        assert!(u.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A17321%2Fcallback"));
    }

    #[test]
    fn parses_callback() {
        let r = parse_callback("GET /callback?code=abc123&state=xyz HTTP/1.1").unwrap();
        assert_eq!(r.0, "abc123");
        assert_eq!(r.1, "xyz");
        // percent 编码
        let r = parse_callback("GET /callback?code=a%2Bb&state=s%20t HTTP/1.1").unwrap();
        assert_eq!(r.0, "a+b");
        assert_eq!(r.1, "s t");
    }

    #[test]
    fn callback_errors_are_readable() {
        assert!(parse_callback("GET /callback?state=only HTTP/1.1").is_err());
        assert!(parse_callback("GET /favicon.ico HTTP/1.1").is_err());
        assert!(parse_callback("GET /callback?error=access_denied&error_description=nope HTTP/1.1")
            .unwrap_err()
            .contains("登录被拒绝"));
        assert!(parse_callback("garbage").is_err());
    }

    #[test]
    fn port_busy_is_friendly() {
        // 占用端口后应给出可读提示而不是 panic
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        std::env::set_var("AIQUICK_REDIRECT_URI", format!("http://127.0.0.1:{port}/callback"));
        let err = obtain_key(&|_| Ok(()), Duration::from_millis(10)).unwrap_err();
        std::env::remove_var("AIQUICK_REDIRECT_URI");
        assert!(err.contains("被占用") || err.contains("端口"), "{err}");
        assert!(!err.contains("sk-"));
    }
}
