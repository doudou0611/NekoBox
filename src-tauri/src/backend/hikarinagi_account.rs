//! Browser consent with PKCE and a bounded loopback callback. User credentials
//! are independent from the built-in application's catalog authorization.
use super::{
    credential::{NativeVault, Vault},
    hikarinagi_app as app, *,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

const PROFILE: &str = "hikarinagi.profile";
pub(super) const USER_API_BASE: &str = "https://api.hikarinagi.org/api/v3/open/user/me";
const SCOPES: &str =
    "openid profile offline_access catalog:full user:read status:read status:write";
#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: u64,
    pub username: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct Account {
    pub status: &'static str,
    pub profile: Option<Profile>,
    pub message: &'static str,
}
#[derive(Deserialize)]
pub struct FlowRequest {
    pub flow_id: String,
}
#[derive(Serialize)]
pub struct Begin {
    pub flow_id: String,
    pub authorization_url: String,
    pub expires_in_seconds: u32,
}
#[derive(Clone, Default, Serialize)]
pub struct LoginFlow {
    pub flow_id: String,
    pub status: String,
    pub message: String,
    pub account: Option<Account>,
}
// Only serialized into the native vault, never a command result or log.
#[derive(Serialize, Deserialize)]
struct Credential {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: i64,
    scope: String,
}
fn vault(backend: &Backend) -> Result<NativeVault> {
    NativeVault::new(backend, "dev.galgame.manager.hikarinagi.account")
}
pub(super) fn client() -> Result<reqwest::blocking::Client> {
    super::network::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("GalgameManager/0.1 (account and play data)")
        .build()
        .map_err(|_| ServiceError(ErrorCode::NetworkUnavailable, "无法创建账户网络请求。"))
}
pub(super) fn response_json(response: reqwest::blocking::Response) -> Result<Value> {
    if !response.status().is_success() {
        return Err(response_error(response.status().as_u16()));
    }
    let value: Value = response
        .json()
        .map_err(|_| ServiceError(ErrorCode::InvalidResponse, "Hikarinagi 账户响应无效。"))?;
    if value.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi 未能处理账户请求。",
        ));
    }
    Ok(value.get("data").cloned().unwrap_or(value))
}
fn response_error(status: u16) -> ServiceError {
    match status {
        401 => ServiceError(
            ErrorCode::PermissionDenied,
            "Hikarinagi 账户授权无效，请重新登录。",
        ),
        403 => ServiceError(
            ErrorCode::PermissionDenied,
            "Hikarinagi 账户或应用缺少所需权限。",
        ),
        404 => ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi 账户接口不存在，请更新软件后重试。",
        ),
        429 => ServiceError(
            ErrorCode::RateLimited,
            "Hikarinagi 请求过于频繁，请稍后重试。",
        ),
        500..=599 => ServiceError(
            ErrorCode::NetworkUnavailable,
            "Hikarinagi 服务暂时不可用，请稍后重试。",
        ),
        _ => ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi 未接受此账户请求，请稍后重试。",
        ),
    }
}
fn parse_profile(value: &Value) -> Result<Profile> {
    let username = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("Hikarinagi 账户缺少用户名。"))?;
    Ok(Profile {
        id: value
            .get("id")
            .and_then(Value::as_u64)
            .filter(|v| *v > 0)
            .ok_or_else(|| invalid("Hikarinagi 账户标识无效。"))?,
        username: username.into(),
        nickname: value
            .get("nickname")
            .and_then(Value::as_str)
            .unwrap_or(username)
            .into(),
        avatar_url: value
            .get("avatar")
            .and_then(|v| v.get("src"))
            .and_then(Value::as_str)
            .filter(|url| super::hikarinagi::trusted_image(url))
            .map(str::to_owned),
    })
}
fn fetch_profile(token: &str) -> Result<Profile> {
    fetch_profile_at(USER_API_BASE, token)
}
fn fetch_profile_at(url: &str, token: &str) -> Result<Profile> {
    let response =
        client()?.get(url).bearer_auth(token).send().map_err(|_| {
            ServiceError(ErrorCode::NetworkUnavailable, "暂时无法连接 Hikarinagi。")
        })?;
    parse_profile(&response_json(response)?)
}
fn parse_credential(value: &Value, previous_refresh: Option<String>) -> Result<Credential> {
    let access = value
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("账户授权没有返回访问令牌。"))?;
    let access = super::credential::validate(access)?;
    let expires = value
        .get("expires_in")
        .and_then(Value::as_i64)
        .filter(|v| *v > 0 && *v <= 31_536_000)
        .ok_or_else(|| invalid("账户授权的有效期无效。"))?;
    let refresh = value
        .get("refresh_token")
        .and_then(Value::as_str)
        .map(super::credential::validate)
        .transpose()?
        .map(str::to_owned)
        .or(previous_refresh);
    Ok(Credential {
        access_token: access.into(),
        refresh_token: refresh,
        expires_at: chrono::Utc::now().timestamp() + expires,
        scope: value
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or(SCOPES)
            .into(),
    })
}
fn exchange(form: &[(&str, &str)], previous: Option<String>) -> Result<Credential> {
    let mut encoded = reqwest::Url::parse(app::TOKEN_URL).map_err(|_| invalid("授权地址无效。"))?;
    encoded.query_pairs_mut().extend_pairs(form.iter().copied());
    let response = client()?
        .post(app::TOKEN_URL)
        .basic_auth(app::CLIENT_ID, Some(app::CLIENT_SECRET))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(encoded.query().unwrap_or("").to_owned())
        .send()
        .map_err(|_| {
            ServiceError(
                ErrorCode::NetworkUnavailable,
                "账户授权暂时无法连接，请重新登录。",
            )
        })?;
    if !response.status().is_success() {
        let value = response.json::<Value>().unwrap_or(Value::Null);
        return Err(ServiceError(ErrorCode::PermissionDenied,match value.get("error").and_then(Value::as_str) {
            Some("invalid_scope")=>"应用尚未允许账户与同步权限，请在开发者控制台启用登录、user:read、status:read/status:write 与 offline_access。",
            Some("invalid_grant")=>"Hikarinagi 授权已失效，请重新登录。",
            _=>"Hikarinagi 账户授权失败，请检查应用回调地址与账户权限。",
        }));
    }
    parse_credential(
        &response.json().map_err(|_| invalid("账户授权响应无效。"))?,
        previous,
    )
}
fn save_with(
    backend: &Backend,
    vault: &impl Vault,
    credential: &Credential,
    profile: &Profile,
) -> Result<()> {
    let previous = vault.read()?;
    vault.write(&serde_json::to_string(credential).map_err(|_| invalid("账户授权无法保存。"))?)?;
    if let Err(e) = backend
        .database()
        .and_then(|db| db.put_setting(PROFILE, profile))
    {
        if let Some(old) = previous {
            vault.write(&old)?;
        } else {
            vault.delete()?;
        }
        return Err(e);
    }
    Ok(())
}
fn read_token(backend: &Backend) -> Result<Option<String>> {
    let store = vault(backend)?;
    let Some(raw) = store.read()? else {
        return Ok(None);
    };
    let mut credential: Credential = serde_json::from_str(&raw)
        .map_err(|_| invalid("保存的 Hikarinagi 授权无法读取，请重新登录。"))?;
    super::credential::validate(&credential.access_token)?;
    if credential.expires_at <= chrono::Utc::now().timestamp() + 30 {
        let refresh = credential.refresh_token.as_deref().ok_or(ServiceError(
            ErrorCode::PermissionDenied,
            "Hikarinagi 登录已过期，请重新登录。",
        ))?;
        credential = exchange(
            &[("grant_type", "refresh_token"), ("refresh_token", refresh)],
            credential.refresh_token.clone(),
        )?;
        store.write(
            &serde_json::to_string(&credential).map_err(|_| invalid("账户授权无法保存。"))?,
        )?;
    }
    Ok(Some(credential.access_token))
}
pub fn token(backend: &Backend) -> Result<Option<String>> {
    if cfg!(test) {
        return Ok(None);
    }
    let _guard = backend
        .hikarinagi_account_lock
        .lock()
        .map_err(|_| invalid("账户服务需要重启。"))?;
    read_token(backend)
}
fn signed_out() -> Account {
    Account {
        status: "signed_out",
        profile: None,
        message: "登录自己的 Hikarinagi 账户以同步游玩数据；未登录也可刮削。",
    }
}
pub fn account(backend: &Backend) -> Result<Account> {
    let _guard = backend
        .hikarinagi_account_lock
        .lock()
        .map_err(|_| invalid("账户服务需要重启。"))?;
    let result =
        read_token(backend).and_then(|token| token.map(|token| fetch_profile(&token)).transpose());
    match result {
        Ok(Some(profile)) => {
            backend.database()?.put_setting(PROFILE, &profile)?;
            Ok(Account {
                status: "authenticated",
                profile: Some(profile),
                message: "已登录，元数据请求使用此账户，可同步游玩数据。",
            })
        }
        Ok(None) => Ok(signed_out()),
        Err(e)
            if matches!(
                e.0,
                ErrorCode::NetworkUnavailable
                    | ErrorCode::RateLimited
                    | ErrorCode::PermissionDenied
            ) =>
        {
            Ok(Account {
                status: if e.0 == ErrorCode::PermissionDenied {
                    "expired"
                } else {
                    "offline"
                },
                profile: backend.database()?.setting(PROFILE)?,
                message: if e.0 == ErrorCode::PermissionDenied {
                    "账户授权已过期或缺少权限，请重新登录。"
                } else {
                    "网络暂不可用，显示上次账户信息。"
                },
            })
        }
        Err(e) => Err(e),
    }
}
pub fn logout(backend: &Backend) -> Result<Account> {
    let _guard = backend
        .hikarinagi_account_lock
        .lock()
        .map_err(|_| invalid("账户服务需要重启。"))?;
    let mut flow = backend
        .hikarinagi_login
        .lock()
        .map_err(|_| invalid("登录服务需要重启。"))?;
    flow.status = "cancelled".into();
    vault(backend)?.delete()?;
    backend.database()?.delete_setting(PROFILE)?;
    Ok(signed_out())
}
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..chunk.len() + 1 {
            result.push(ALPHABET[((n >> (18 - index * 6)) & 63) as usize] as char);
        }
    }
    result
}
fn authorization_url(state: &str, verifier: &str) -> Result<String> {
    let mut url = reqwest::Url::parse("https://id.hikarinagi.org/oidc/auth")
        .map_err(|_| invalid("授权地址无效。"))?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", app::CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", app::REDIRECT_URI),
        ("scope", SCOPES),
        ("state", state),
        ("code_challenge_method", "S256"),
        (
            "code_challenge",
            &base64url(&Sha256::digest(verifier.as_bytes())),
        ),
    ]);
    Ok(url.into())
}
fn callback_target(target: &str, state: &str) -> Result<Option<String>> {
    let url = reqwest::Url::parse(&format!("http://127.0.0.1:17893{target}"))
        .map_err(|_| invalid("登录回调无效。"))?;
    let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    if url.host_str() != Some("127.0.0.1")
        || url.port() != Some(17893)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/callback"
        || pairs.get("state").map(String::as_str) != Some(state)
    {
        return Err(invalid("登录回调不匹配。"));
    }
    if pairs.contains_key("error") {
        return Ok(None);
    }
    let code = pairs
        .get("code")
        .filter(|v| !v.is_empty() && v.len() <= 4096)
        .ok_or_else(|| invalid("登录回调缺少授权码。"))?;
    Ok(Some(code.clone()))
}
pub fn begin(backend: &Backend) -> Result<Begin> {
    let mut flow = backend
        .hikarinagi_login
        .lock()
        .map_err(|_| invalid("登录服务需要重启。"))?;
    if flow.status == "pending" {
        return Err(ServiceError(
            ErrorCode::Conflict,
            "已有 Hikarinagi 登录窗口，请完成或取消后再试。",
        ));
    }
    let listener = TcpListener::bind("127.0.0.1:17893").map_err(|_| {
        ServiceError(
            ErrorCode::Conflict,
            "登录回调端口 17893 被占用，请关闭其他登录窗口后重试。",
        )
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|_| invalid("无法启动登录回调。"))?;
    let flow_id = id();
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let url = authorization_url(&flow_id, &verifier)?;
    *flow = LoginFlow {
        flow_id: flow_id.clone(),
        status: "pending".into(),
        message: "请在官方登录窗口确认授权。".into(),
        account: None,
    };
    let state = flow_id.clone();
    let backend = backend.clone();
    std::thread::spawn(move || listen_callback(&backend, listener, &state, &verifier));
    Ok(Begin {
        flow_id,
        authorization_url: url,
        expires_in_seconds: 180,
    })
}
fn finish(backend: &Backend, state: &str, status: &str, message: &str, account: Option<Account>) {
    if let Ok(mut flow) = backend.hikarinagi_login.lock() {
        if flow.flow_id == state && flow.status == "pending" {
            flow.status = status.into();
            flow.message = message.into();
            flow.account = account;
        }
    }
}
fn listen_callback(backend: &Backend, listener: TcpListener, state: &str, verifier: &str) {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(180) {
        if !backend
            .hikarinagi_login
            .lock()
            .is_ok_and(|flow| flow.flow_id == state && flow.status == "pending")
        {
            return;
        }
        let (mut stream, _) = match listener.accept() {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(_) => break,
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut bytes = Vec::new();
        let mut buffer = [0; 1024];
        while bytes.len() < 8192 && !bytes.windows(4).any(|v| v == b"\r\n\r\n") {
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            }
        }
        let request = String::from_utf8_lossy(&bytes);
        let first = request.lines().next().unwrap_or("");
        let parts: Vec<_> = first.split_whitespace().collect();
        let host_valid = request
            .lines()
            .any(|line| line.eq_ignore_ascii_case("Host: 127.0.0.1:17893"));
        let parsed = if parts.first() == Some(&"GET") && host_valid && bytes.len() < 8192 {
            parts
                .get(1)
                .and_then(|target| callback_target(target, state).ok())
        } else {
            None
        };
        let (status, body) = if parsed.is_some() {
            (
                "200 OK",
                "Authorization received. You can return to NekoBox.",
            )
        } else {
            (
                "400 Bad Request",
                "This callback is not for the current sign-in.",
            )
        };
        let _=write!(stream,"HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",body.len());
        let Some(code) = parsed else {
            continue;
        };
        let Some(code) = code else {
            finish(backend, state, "cancelled", "已取消官方授权。", None);
            return;
        };
        let result = (|| {
            let credential = exchange(
                &[
                    ("grant_type", "authorization_code"),
                    ("code", &code),
                    ("redirect_uri", app::REDIRECT_URI),
                    ("code_verifier", verifier),
                ],
                None,
            )?;
            let profile = fetch_profile(&credential.access_token)?;
            let _account_guard = backend
                .hikarinagi_account_lock
                .lock()
                .map_err(|_| invalid("账户服务需要重启。"))?;
            let mut flow = backend
                .hikarinagi_login
                .lock()
                .map_err(|_| invalid("登录服务需要重启。"))?;
            if flow.flow_id != state || flow.status != "pending" {
                return Err(ServiceError(ErrorCode::Cancelled, "登录已取消。"));
            }
            save_with(backend, &vault(backend)?, &credential, &profile)?;
            flow.status = "completed".into();
            flow.message = "登录成功。".into();
            flow.account = Some(Account {
                status: "authenticated",
                profile: Some(profile),
                message: "已登录，元数据与游玩同步使用此账户。",
            });
            Ok(())
        })();
        if let Err(e) = result {
            finish(backend, state, "failed", e.1, None);
        }
        return;
    }
    finish(
        backend,
        state,
        "failed",
        "登录超时，请重新打开官方授权。",
        None,
    );
}
pub fn poll(backend: &Backend, request: FlowRequest) -> Result<LoginFlow> {
    let flow = backend
        .hikarinagi_login
        .lock()
        .map_err(|_| invalid("登录服务需要重启。"))?;
    if flow.flow_id != request.flow_id {
        return Err(invalid("登录会话已失效。"));
    }
    Ok(flow.clone())
}
pub fn cancel(backend: &Backend, request: FlowRequest) -> Result<bool> {
    let mut flow = backend
        .hikarinagi_login
        .lock()
        .map_err(|_| invalid("登录服务需要重启。"))?;
    if flow.flow_id != request.flow_id {
        return Err(invalid("登录会话已失效。"));
    }
    if flow.status == "pending" {
        flow.status = "cancelled".into();
        flow.message = "已取消登录。".into();
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_request_uses_official_path_and_does_not_call_missing_alias() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = [0; 8192];
            let n = stream.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..n]);
            let correct = request.starts_with("GET /api/v3/open/user/me HTTP/1.1\r\n")
                && request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer account-fixture\r\n");
            let status = if correct { "200 OK" } else { "404 Not Found" };
            let body = if correct {
                r#"{"success":true,"data":{"id":7,"name":"tester","nickname":"Test"}}"#
            } else {
                r#"{"success":false,"error":{"code":"COMMON_NOT_FOUND"}}"#
            };
            write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        });
        let official = reqwest::Url::parse(USER_API_BASE).unwrap();
        assert_eq!(official.host_str(), Some("api.hikarinagi.org"));
        let profile = fetch_profile_at(
            &format!("http://{address}{}", official.path()),
            "account-fixture",
        )
        .unwrap();
        assert_eq!(profile.username, "tester");
        worker.join().unwrap();
    }
    #[test]
    fn missing_api_is_not_reported_as_network_failure() {
        assert_eq!(response_error(404).0, ErrorCode::InvalidResponse);
        assert!(!response_error(404).1.contains("网络"));
        assert_eq!(response_error(401).0, ErrorCode::PermissionDenied);
        assert_eq!(response_error(403).0, ErrorCode::PermissionDenied);
        assert_eq!(response_error(429).0, ErrorCode::RateLimited);
        assert_eq!(response_error(503).0, ErrorCode::NetworkUnavailable);
    }
    #[test]
    fn pkce_matches_rfc7636_and_does_not_send_secret_or_verifier_to_browser() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            base64url(&Sha256::digest(verifier.as_bytes())),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let url = authorization_url("fixture-state", verifier).unwrap();
        assert!(!url.contains(app::CLIENT_SECRET));
        assert!(!url.contains(verifier));
        assert!(url.contains("code_challenge_method=S256"));
    }
    #[test]
    fn callback_requires_current_state_and_exact_path() {
        assert_eq!(
            callback_target("/callback?state=current&code=valid-code", "current").unwrap(),
            Some("valid-code".into())
        );
        assert!(callback_target("/callback?state=other&code=valid-code", "current").is_err());
        assert!(callback_target("/other?state=current&code=valid-code", "current").is_err());
        assert!(callback_target(
            "@external.example/callback?state=current&code=fake",
            "current"
        )
        .is_err());
        assert!(
            callback_target("/callback?state=current&error=access_denied", "current")
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn profile_and_account_do_not_return_private_claims_or_credentials() {
        let v = serde_json::json!({"id":1,"name":"tester","nickname":"Test","email":"private@example.test","avatar":{"src":"https://images.yurari.moe/avatar.png"}});
        let profile = parse_profile(&v).unwrap();
        assert!(profile.avatar_url.is_some());
        let account = Account {
            status: "authenticated",
            profile: Some(profile),
            message: "ok",
        };
        assert!(!serde_json::to_string(&account)
            .unwrap()
            .contains("private@example"));
        let invalid = parse_credential(
            &serde_json::json!({"access_token":"bad\ntoken","expires_in":100}),
            None,
        );
        assert!(invalid.is_err());
    }
    #[test]
    fn personal_credentials_are_separate_from_database_and_command_response() {
        let root = std::env::temp_dir().join(format!("gm-hika-vault-{}", id()));
        let backend = Backend::open(root.clone()).unwrap();
        let store = super::super::credential::MemoryVault::default();
        let credential = parse_credential(
            &serde_json::json!({
                "access_token":"private-access-fixture", "refresh_token":"private-refresh-fixture",
                "expires_in":3600, "scope":SCOPES
            }),
            None,
        )
        .unwrap();
        let profile =
            parse_profile(&serde_json::json!({"id":7,"name":"fixture-user","nickname":"测试"}))
                .unwrap();
        save_with(&backend, &store, &credential, &profile).unwrap();
        let cached: Profile = backend
            .database()
            .unwrap()
            .setting(PROFILE)
            .unwrap()
            .unwrap();
        let response = serde_json::to_string(&Account {
            status: "authenticated",
            profile: Some(cached),
            message: "ok",
        })
        .unwrap();
        assert!(!response.contains("private-access-fixture"));
        assert!(!response.contains("private-refresh-fixture"));
        assert!(store
            .read()
            .unwrap()
            .unwrap()
            .contains("private-refresh-fixture"));
        // A rotated refresh response that omits refresh_token preserves the prior credential.
        let rotated = parse_credential(
            &serde_json::json!({"access_token":"rotated-fixture","expires_in":3600}),
            credential.refresh_token,
        )
        .unwrap();
        assert_eq!(
            rotated.refresh_token.as_deref(),
            Some("private-refresh-fixture")
        );
        drop(backend);
        let database = std::fs::read(root.join("galgame-manager.sqlite3")).unwrap();
        assert!(!database
            .windows(b"private-access-fixture".len())
            .any(|v| v == b"private-access-fixture"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancellation_and_stale_result_do_not_replace_current_login() {
        let root = std::env::temp_dir().join(format!("gm-hika-flow-{}", id()));
        let b = Backend::open(root.clone()).unwrap();
        *b.hikarinagi_login.lock().unwrap() = LoginFlow {
            flow_id: "current".into(),
            status: "pending".into(),
            ..LoginFlow::default()
        };
        assert!(cancel(
            &b,
            FlowRequest {
                flow_id: "old".into()
            }
        )
        .is_err());
        cancel(
            &b,
            FlowRequest {
                flow_id: "current".into(),
            },
        )
        .unwrap();
        finish(&b, "current", "completed", "wrong", None);
        assert_eq!(
            poll(
                &b,
                FlowRequest {
                    flow_id: "current".into()
                }
            )
            .unwrap()
            .status,
            "cancelled"
        );
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
}
