use super::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: u64,
    pub username: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
}
#[derive(Serialize)]
pub struct Account {
    pub status: &'static str,
    pub profile: Option<Profile>,
    pub message: &'static str,
}
// Deliberately not Debug/Serialize: the credential must never enter a response or log.
#[derive(Deserialize)]
pub struct LoginRequest {
    pub access_token: String,
}
fn vault_error() -> ServiceError {
    ServiceError(
        ErrorCode::PermissionDenied,
        "无法访问系统凭据库，请检查系统权限后重试。",
    )
}
trait Vault {
    fn read(&self) -> Result<Option<String>>;
    fn write(&self, token: &str) -> Result<()>;
    fn delete(&self) -> Result<()>;
}
struct NativeVault(keyring::Entry);
impl NativeVault {
    fn new(backend: &Backend) -> Result<Self> {
        let scope = format!(
            "{:x}",
            Sha256::digest(path_text(&backend.data_directory)?.as_bytes())
        );
        keyring::Entry::new("dev.galgame.manager.bangumi", &scope)
            .map(Self)
            .map_err(|_| vault_error())
    }
}
impl Vault for NativeVault {
    fn read(&self) -> Result<Option<String>> {
        match self.0.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(vault_error()),
        }
    }
    fn write(&self, token: &str) -> Result<()> {
        self.0.set_password(token).map_err(|_| vault_error())
    }
    fn delete(&self) -> Result<()> {
        match self.0.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(vault_error()),
        }
    }
}
fn validate_token(token: &str) -> Result<&str> {
    let token = token.trim();
    if token.is_empty() || token.len() > 4096 || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(invalid("请输入有效的 Bangumi Access Token。"));
    }
    Ok(token)
}
fn parse_profile(value: &Value) -> Result<Profile> {
    let text = |key| value.get(key).and_then(Value::as_str).map(str::to_owned);
    Ok(Profile {
        id: value.get("id").and_then(Value::as_u64).ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "Bangumi 账号资料无效。",
        ))?,
        username: text("username").ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "Bangumi 账号资料无效。",
        ))?,
        nickname: text("nickname").unwrap_or_default(),
        avatar_url: value
            .get("avatar")
            .and_then(|v| {
                ["large", "medium", "small"]
                    .into_iter()
                    .filter_map(|k| v.get(k).and_then(Value::as_str))
                    .find(|url| super::bangumi::trusted_image(url))
            })
            .map(str::to_owned),
    })
}
fn fetch_profile(token: &str) -> Result<Profile> {
    let response = super::bangumi::client()?
        .get("https://api.bgm.tv/v0/me")
        .bearer_auth(token)
        .send()
        .map_err(|_| {
            ServiceError(
                ErrorCode::NetworkUnavailable,
                "暂时无法连接 Bangumi，请检查网络。",
            )
        })?;
    parse_profile(&super::bangumi::request_json(response)?)
}
pub fn token(backend: &Backend) -> Result<Option<String>> {
    if cfg!(test) {
        return Ok(None);
    }
    let _guard = backend.bangumi_lock.lock().map_err(|_| vault_error())?;
    NativeVault::new(backend)?.read()
}
fn login_with(
    backend: &Backend,
    vault: &impl Vault,
    token: &str,
    fetch: impl FnOnce(&str) -> Result<Profile>,
) -> Result<Account> {
    let token = validate_token(token)?;
    let profile = fetch(token)?;
    vault.write(token)?;
    backend
        .database()?
        .put_setting("bangumi.profile", &profile)?;
    Ok(Account {
        status: "authenticated",
        profile: Some(profile),
        message: "已登录，后续刮削将使用此账号的授权。",
    })
}
pub fn login(backend: &Backend, request: LoginRequest) -> Result<Account> {
    let _guard = backend.bangumi_lock.lock().map_err(|_| vault_error())?;
    login_with(
        backend,
        &NativeVault::new(backend)?,
        &request.access_token,
        fetch_profile,
    )
}
pub fn account(backend: &Backend) -> Result<Account> {
    let _guard = backend.bangumi_lock.lock().map_err(|_| vault_error())?;
    let Some(token) = NativeVault::new(backend)?.read()? else {
        return Ok(Account {
            status: "signed_out",
            profile: None,
            message: "尚未登录 Bangumi。",
        });
    };
    match fetch_profile(&token) {
        Ok(profile) => {
            backend
                .database()?
                .put_setting("bangumi.profile", &profile)?;
            Ok(Account {
                status: "authenticated",
                profile: Some(profile),
                message: "账号授权有效，刮削使用此账号。",
            })
        }
        Err(e) if matches!(e.0, ErrorCode::NetworkUnavailable | ErrorCode::RateLimited) => {
            Ok(Account {
                status: "offline",
                profile: backend.database()?.setting("bangumi.profile")?,
                message: "暂时无法验证授权，显示上次账号信息；凭证仍已保存。",
            })
        }
        Err(e) if e.0 == ErrorCode::PermissionDenied => Ok(Account {
            status: "expired",
            profile: backend.database()?.setting("bangumi.profile")?,
            message: "授权已过期或权限不足，请重新生成 Token 登录。",
        }),
        Err(e) => Err(e),
    }
}
fn logout_with(backend: &Backend, vault: &impl Vault) -> Result<Account> {
    vault.delete()?;
    backend.database()?.delete_setting("bangumi.profile")?;
    Ok(Account {
        status: "signed_out",
        profile: None,
        message: "已退出登录并删除本机保存的凭证。",
    })
}
pub fn logout(backend: &Backend) -> Result<Account> {
    let _guard = backend.bangumi_lock.lock().map_err(|_| vault_error())?;
    logout_with(backend, &NativeVault::new(backend)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[derive(Default)]
    struct TestVault(RefCell<Option<String>>);
    impl Vault for TestVault {
        fn read(&self) -> Result<Option<String>> {
            Ok(self.0.borrow().clone())
        }
        fn write(&self, token: &str) -> Result<()> {
            *self.0.borrow_mut() = Some(token.to_owned());
            Ok(())
        }
        fn delete(&self) -> Result<()> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }
    #[test]
    fn login_saves_only_after_validation_and_logout_clears_credential() {
        let root = std::env::temp_dir().join(format!("gm-account-{}", id()));
        let b = Backend::open(root.clone()).unwrap();
        let vault = TestVault::default();
        let profile = || {
            parse_profile(
                &serde_json::json!({"id": 1,"username":"tester","nickname":"测试","email":"private@example.test","avatar":{"large":"https://lain.bgm.tv/pic/user/l/test.jpg"}}),
            )
        };
        assert!(login_with(&b, &vault, "test-token", |_| Err(ServiceError(
            ErrorCode::PermissionDenied,
            "拒绝"
        )))
        .is_err());
        assert!(vault.read().unwrap().is_none());
        let result = login_with(&b, &vault, " test-token ", |_| profile()).unwrap();
        let response = serde_json::to_string(&result).unwrap();
        assert!(!response.contains("test-token") && !response.contains("private@example"));
        assert_eq!(vault.read().unwrap().as_deref(), Some("test-token"));
        assert!(b
            .database()
            .unwrap()
            .setting::<Profile>("bangumi.profile")
            .unwrap()
            .is_some());
        logout_with(&b, &vault).unwrap();
        assert!(vault.read().unwrap().is_none());
        assert!(b
            .database()
            .unwrap()
            .setting::<Profile>("bangumi.profile")
            .unwrap()
            .is_none());
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_tokens_and_untrusted_avatars_are_rejected() {
        for token in ["", " ", "a\nb", "中文"] {
            assert!(validate_token(token).is_err());
        }
        let profile = parse_profile(&serde_json::json!({"id":1,"username":"t","avatar":{"large":"https://example.test/user.jpg"}})).unwrap();
        assert!(profile.avatar_url.is_none());
    }
}
