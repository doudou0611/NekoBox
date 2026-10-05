//! Official HF client protocol; ownership is independent of scraping and installations.
mod download;
#[cfg(test)]
mod tests;
use super::{
    credential::{NativeVault, Vault},
    *,
};
pub use download::{cancel, downloads, start, Download, DownloadRequest, Manager, TaskRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::time::Duration;
const BASE: &str = "https://api.hikarifield.co.jp/v1/";
const PROFILE: &str = "hikarifield.profile";
const CONFIG: &str = "hikarifield.settings";
const JOBS: &str = "hikarifield.downloads";
#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: u64,
    pub name: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub root: Option<String>,
    #[serde(default)]
    pub uuid: String,
}
#[derive(Serialize)]
pub struct Account {
    pub status: &'static str,
    pub profile: Option<Profile>,
    pub message: &'static str,
}
#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}
#[derive(Deserialize)]
pub struct PathRequest {
    pub parent: String,
}
#[derive(Serialize)]
pub struct SyncReport {
    pub owned: usize,
    pub imported: usize,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct App {
    pub id: u64,
    pub tag: String,
    pub name: String,
    #[serde(default, deserialize_with = "nullable_default")]
    pub have: u8,
    #[serde(default, deserialize_with = "nullable_default")]
    pub released: u8,
    #[serde(default, deserialize_with = "nullable_default")]
    pub trial: u8,
    #[serde(default, deserialize_with = "nullable_default")]
    pub asmr: u8,
    #[serde(default, deserialize_with = "nullable_default")]
    pub build_id: u64,
    #[serde(default, deserialize_with = "nullable_default")]
    pub version: String,
    #[serde(default, deserialize_with = "nullable_default")]
    pub install_path: String,
    #[serde(default, deserialize_with = "nullable_default")]
    pub exec_file: String,
    #[serde(default, deserialize_with = "nullable_default")]
    pub depots: std::collections::BTreeMap<String, Depot>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Depot {
    pub build_id: u64,
    #[serde(default, deserialize_with = "nullable_default")]
    pub version: String,
    #[serde(default)]
    pub depot_name: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Ownership {
    pub owners: Vec<u64>,
    pub app: App,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub app_id: u64,
    pub released: bool,
}
#[derive(Serialize, Deserialize)]
struct Credential {
    token: String,
    account_id: u64,
    refreshed_at: i64,
}
fn vault(b: &Backend) -> Result<NativeVault> {
    NativeVault::new(b, "NekoBox.HikariField")
}
fn http() -> Result<reqwest::blocking::Client> {
    network::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(40))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| net())
}
fn net() -> ServiceError {
    ServiceError(
        ErrorCode::NetworkUnavailable,
        "HIKARI FIELD 连接失败，请检查网络或代理后重试。",
    )
}
fn bad() -> ServiceError {
    ServiceError(
        ErrorCode::InvalidResponse,
        "HIKARI FIELD 返回的数据格式不受支持。",
    )
}
fn response_error(status: u16) -> ServiceError {
    match status {
        401 => permission("HIKARI FIELD 登录已失效，请重新登录。"),
        403 => permission("HIKARI FIELD 拒绝此操作，请检查购买权限、设备数量和下载额度。"),
        422 => invalid("HIKARI FIELD 请求未通过验证，请检查账号信息或游戏版本。"),
        429 => ServiceError(
            ErrorCode::RateLimited,
            "HIKARI FIELD 请求过于频繁，请稍后重试。",
        ),
        _ => net(),
    }
}
fn send_at(
    base: &str,
    method: reqwest::Method,
    path: &str,
    body: Value,
    token: Option<&str>,
) -> Result<Value> {
    let mut r = http()?
        .request(method, format!("{base}{path}"))
        .header("Accept", "application/json")
        .json(&body);
    if let Some(t) = token {
        r = r.bearer_auth(credential::validate(t)?);
    }
    let response = r.send().map_err(|_| net())?;
    if !response.status().is_success() {
        return Err(response_error(response.status().as_u16()));
    }
    let mut bytes = Vec::new();
    response
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| net())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(bad());
    }
    serde_json::from_slice(&bytes).map_err(|_| bad())
}
fn send(method: reqwest::Method, path: &str, body: Value, token: Option<&str>) -> Result<Value> {
    send_at(BASE, method, path, body, token)
}
fn read_credential(b: &Backend) -> Result<Credential> {
    let value = vault(b)?
        .read()?
        .ok_or_else(|| permission("请先登录 HIKARI FIELD 账户。"))?;
    serde_json::from_str(&value).map_err(|_| permission("HIKARI FIELD 凭证损坏，请重新登录。"))
}
fn save_credential(b: &Backend, c: &Credential) -> Result<()> {
    vault(b)?.write(&serde_json::to_string(c).map_err(|_| bad())?)
}
fn token_from(v: &Value) -> Result<String> {
    Ok(credential::validate(v["access_token"].as_str().ok_or_else(bad)?)?.into())
}
fn rotate(b: &Backend, c: &mut Credential) -> Result<()> {
    let v = send(
        reqwest::Method::PUT,
        "auth/refresh",
        json!({}),
        Some(&c.token),
    )?;
    c.token = token_from(&v)?;
    c.refreshed_at = chrono::Utc::now().timestamp();
    save_credential(b, c)
}
fn request(b: &Backend, path: &str, body: Value) -> Result<Value> {
    Ok(request_identity(b, path, body)?.1)
}
fn request_identity(b: &Backend, path: &str, body: Value) -> Result<(u64, Value)> {
    let _guard = b.hikarifield_lock.lock().map_err(|_| bad())?;
    let mut c = read_credential(b)?;
    if chrono::Utc::now().timestamp() - c.refreshed_at >= 45 * 60 {
        rotate(b, &mut c)?;
    }
    let result = match send(reqwest::Method::POST, path, body.clone(), Some(&c.token)) {
        Err(e) if e.0 == ErrorCode::PermissionDenied && e.1.contains("登录已失效") => {
            rotate(b, &mut c)?;
            send(reqwest::Method::POST, path, body, Some(&c.token))
        }
        r => r,
    };
    Ok((c.account_id, result?))
}
pub fn account(b: &Backend) -> Result<Account> {
    let _guard = b.hikarifield_lock.lock().map_err(|_| bad())?;
    let profile = b.database()?.setting(PROFILE)?;
    let authenticated = vault(b)?.read()?.is_some();
    Ok(Account {
        status: if authenticated {
            "authenticated"
        } else {
            "signed_out"
        },
        profile: if authenticated { profile } else { None },
        message: if authenticated {
            "已购游戏已保存在本地游戏库。"
        } else {
            "登录后自动导入你已拥有的游戏。"
        },
    })
}
pub fn login(b: &Backend, q: LoginRequest) -> Result<Account> {
    let _guard = b.hikarifield_lock.lock().map_err(|_| bad())?;
    ensure_idle(b)?;
    if q.email.len() > 320
        || !q.email.contains('@')
        || q.password.is_empty()
        || q.password.len() > 4096
    {
        return Err(invalid("请输入有效邮箱和密码。"));
    }
    let v = send(
        reqwest::Method::POST,
        "auth/login",
        json!({"email":q.email.trim(),"password":q.password}),
        None,
    )
    .map_err(|e| {
        if e.1.contains("登录已失效") {
            permission("HIKARI FIELD 邮箱或密码错误。")
        } else {
            e
        }
    })?;
    let token = token_from(&v)?;
    let p = send(reqwest::Method::POST, "user/info", json!({}), Some(&token))?;
    let profile = Profile {
        id: p["id"].as_u64().filter(|v| *v > 0).ok_or_else(bad)?,
        name: p["name"].as_str().unwrap_or("HIKARI FIELD 玩家").into(),
    };
    save_credential(
        b,
        &Credential {
            token,
            account_id: profile.id,
            refreshed_at: chrono::Utc::now().timestamp(),
        },
    )?;
    b.database()?.put_setting(PROFILE, &profile)?;
    Ok(Account {
        status: "authenticated",
        profile: Some(profile),
        message: "登录成功，正在导入已购游戏。",
    })
}
pub fn logout(b: &Backend) -> Result<Account> {
    let _guard = b.hikarifield_lock.lock().map_err(|_| bad())?;
    ensure_idle(b)?;
    if let Ok(c) = read_credential(b) {
        let _ = send(
            reqwest::Method::DELETE,
            "auth/logout",
            json!({}),
            Some(&c.token),
        );
    }
    vault(b)?.delete()?;
    b.database()?.delete_setting(PROFILE)?;
    Ok(Account {
        status: "signed_out",
        profile: None,
        message: "已退出登录，已购游戏和已安装文件仍保留。",
    })
}
fn ensure_idle(b: &Backend) -> Result<()> {
    if b.hikarifield_downloads
        .lock()
        .map_err(|_| bad())?
        .tasks
        .iter()
        .any(|t| t.active())
    {
        return Err(invalid("请先完成或取消 HIKARI FIELD 下载任务。"));
    }
    Ok(())
}
pub fn settings(b: &Backend) -> Result<Settings> {
    Ok(b.database()?.setting(CONFIG)?.unwrap_or_default())
}
pub fn save_path(b: &Backend, q: PathRequest) -> Result<Settings> {
    let _guard = b.hikarifield_lock.lock().map_err(|_| bad())?;
    ensure_idle(b)?;
    let parent = absolute_directory(&q.parent)?;
    let root = if parent
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("HikariFieldGames"))
    {
        parent
    } else {
        parent.join("HikariFieldGames")
    };
    if std::fs::symlink_metadata(&root).is_ok_and(|m| scanner::linked(&m)) {
        return Err(invalid("安装目录不能是符号链接。"));
    }
    std::fs::create_dir_all(&root)
        .map_err(|_| permission("无法创建 HikariFieldGames，请选择可写目录。"))?;
    let check = root.join(format!(".nekobox-write-{}", id()));
    std::fs::write(&check, []).map_err(|_| permission("HikariFieldGames 不可写，请更换目录。"))?;
    let _ = std::fs::remove_file(check);
    let mut s = settings(b)?;
    s.root = Some(path_text(&root.canonicalize().map_err(|_| bad())?)?);
    if s.uuid.is_empty() {
        s.uuid = id();
    }
    b.database()?.put_setting(CONFIG, &s)?;
    Ok(s)
}
fn nullable_default<'de, T: Deserialize<'de> + Default, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}
fn parse_apps(value: Value) -> Result<Vec<App>> {
    let raw = value
        .as_array()
        .filter(|v| v.len() <= 10000)
        .ok_or_else(bad)?;
    let apps: Vec<App> = raw
        .iter()
        .filter(|a| {
            a["have"].as_u64() == Some(1)
                && a["trial"].as_u64() != Some(1)
                && a["asmr"].as_u64() != Some(1)
        })
        .map(|a| serde_json::from_value(a.clone()).map_err(|_| bad()))
        .collect::<Result<_>>()?;
    if apps.iter().any(|a| {
        a.id == 0
            || a.id > i64::MAX as u64
            || !safe_tag(&a.tag)
            || a.name.trim().is_empty()
            || a.name.len() > 1024
    }) {
        return Err(bad());
    }
    Ok(apps)
}
fn safe_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() < 200
        && tag
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}
pub fn sync(b: &Backend) -> Result<SyncReport> {
    let (account_id, value) = request_identity(b, "apps", json!({"category_id":3}))?;
    let apps = parse_apps(value)?;
    let _guard = b.hikarifield_lock.lock().map_err(|_| bad())?;
    let c = read_credential(b)?;
    if c.account_id != account_id {
        return Err(invalid("账户已变更，请重新同步。"));
    }
    // Account switch is serialized with import; ownership snapshots are account-scoped.
    let owned: Vec<App> = apps
        .into_iter()
        .filter(|a| a.have == 1 && a.trial == 0 && a.asmr == 0)
        .collect();
    let mut imported = 0;
    for app in &owned {
        if b.database()?.import_hf_app(c.account_id, app, None)? {
            imported += 1;
        }
    }
    drop(_guard);
    // Store covers separately; a failed image must never discard an owned game.
    for app in &owned {
        if b.database()?.hf_needs_cover(app.id)? {
            let tag = app.tag.strip_suffix("_trial").unwrap_or(&app.tag);
            let url = format!("https://static.hikarifield.co.jp/images/client/games/{tag}.jpg");
            if let Ok(cover) = bangumi::cache_image_for(b, "hikarifield", &url) {
                b.database()?.set_hf_cover(app.id, &cover)?;
            }
        }
    }
    Ok(SyncReport {
        owned: owned.len(),
        imported,
    })
}
pub(crate) fn trusted_image(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str() == Some("static.hikarifield.co.jp")
            && u.port_or_known_default() == Some(443)
            && u.username().is_empty()
            && u.password().is_none()
            && u.path().starts_with("/images/client/games/")
    })
}
