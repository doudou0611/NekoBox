//! Optional source authorization, scoped to the portable data directory.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SETTING: &str = "metadata.hikarinagi";
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    #[default]
    ClientCredentials,
    AccessToken,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub method: Method,
    pub client_id: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            method: Method::default(),
            client_id: String::new(),
        }
    }
}
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Secrets {
    client_secret: Option<String>,
    access_token: Option<String>,
}
#[derive(Serialize)]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    pub has_client_secret: bool,
    pub has_access_token: bool,
    pub can_search: bool,
}
#[derive(Deserialize)]
pub struct SaveRequest {
    #[serde(flatten)]
    pub settings: Settings,
    pub client_secret: Option<String>,
    pub access_token: Option<String>,
    #[serde(default)]
    pub clear_credentials: bool,
}
pub enum Authorization {
    ClientCredentials(String, String),
    AccessToken(String),
}
trait Vault {
    fn read(&self) -> Result<Secrets>;
    fn write(&self, secrets: &Secrets) -> Result<()>;
}
struct NativeVault(keyring::Entry);
fn vault_error() -> ServiceError {
    ServiceError(
        ErrorCode::PermissionDenied,
        "无法访问 Hikarinagi 授权的系统凭据库。",
    )
}
impl NativeVault {
    fn new(backend: &Backend) -> Result<Self> {
        let scope = format!(
            "{:x}",
            Sha256::digest(path_text(&backend.data_directory)?.as_bytes())
        );
        keyring::Entry::new("dev.galgame.manager.hikarinagi", &scope)
            .map(Self)
            .map_err(|_| vault_error())
    }
}
impl Vault for NativeVault {
    fn read(&self) -> Result<Secrets> {
        match self.0.get_password() {
            Ok(value) => serde_json::from_str(&value).map_err(|_| vault_error()),
            Err(keyring::Error::NoEntry) => Ok(Secrets::default()),
            Err(_) => Err(vault_error()),
        }
    }
    fn write(&self, secrets: &Secrets) -> Result<()> {
        if secrets.client_secret.is_none() && secrets.access_token.is_none() {
            match self.0.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err(vault_error()),
            }
        } else {
            let value = serde_json::to_string(secrets).map_err(|_| vault_error())?;
            self.0.set_password(&value).map_err(|_| vault_error())
        }
    }
}
fn environment(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
fn settings(backend: &Backend) -> Result<Settings> {
    Ok(backend
        .database()?
        .setting(SETTING)?
        .unwrap_or_else(|| Settings {
            client_id: environment("GALGAME_MANAGER_HIKARINAGI_CLIENT_ID")
                .unwrap_or_else(|| super::hikarinagi_app::CLIENT_ID.into()),
            method: if environment("GALGAME_MANAGER_HIKARINAGI_ACCESS_TOKEN").is_some() {
                Method::AccessToken
            } else {
                Method::ClientCredentials
            },
            ..Settings::default()
        }))
}
fn effective_secrets(mut secrets: Secrets, settings: &Settings) -> Secrets {
    secrets.client_secret = secrets
        .client_secret
        .or_else(|| environment("GALGAME_MANAGER_HIKARINAGI_CLIENT_SECRET"));
    secrets.access_token = secrets
        .access_token
        .or_else(|| environment("GALGAME_MANAGER_HIKARINAGI_ACCESS_TOKEN"));
    if settings.client_id == super::hikarinagi_app::CLIENT_ID && secrets.client_secret.is_none() {
        secrets.client_secret = Some(super::hikarinagi_app::CLIENT_SECRET.into());
    }
    secrets
}
fn view(settings: Settings, secrets: &Secrets) -> SettingsView {
    let has_client_secret = secrets.client_secret.is_some();
    let has_access_token = secrets.access_token.is_some();
    let can_search = settings.enabled
        && match settings.method {
            Method::ClientCredentials => !settings.client_id.is_empty() && has_client_secret,
            Method::AccessToken => has_access_token,
        };
    SettingsView {
        settings,
        has_client_secret,
        has_access_token,
        can_search,
    }
}
pub fn get_settings(backend: &Backend) -> Result<SettingsView> {
    let _guard = backend.hikarinagi_lock.lock().map_err(|_| vault_error())?;
    let mut config = settings(backend)?;
    if config.client_id.is_empty() {
        config.client_id = super::hikarinagi_app::CLIENT_ID.into();
    }
    let secrets = effective_secrets(
        NativeVault::new(backend)?.read().unwrap_or_default(),
        &config,
    );
    Ok(view(config, &secrets))
}
fn secret(value: Option<String>) -> Result<Option<String>> {
    let value = value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if value.as_ref().is_some_and(|value| {
        value.len() > 8192 || !value.bytes().all(|byte| byte.is_ascii_graphic())
    }) {
        return Err(invalid("Hikarinagi 授权内容无效，请检查输入。"));
    }
    Ok(value)
}
fn save_with(
    backend: &Backend,
    vault: &impl Vault,
    mut request: SaveRequest,
) -> Result<SettingsView> {
    request.settings.client_id = request.settings.client_id.trim().into();
    if request.settings.client_id.len() > 256
        || !request
            .settings
            .client_id
            .bytes()
            .all(|byte| byte.is_ascii_graphic())
    {
        return Err(invalid("Hikarinagi Client ID 无效。"));
    }
    let previous = settings(backend)?;
    let old = vault.read()?;
    let client_secret = secret(request.client_secret)?;
    let access_token = secret(request.access_token)?;
    if request.clear_credentials && (client_secret.is_some() || access_token.is_some()) {
        return Err(invalid("清除授权时不能同时填写新授权。"));
    }
    if previous.client_id != request.settings.client_id
        && old.client_secret.is_some()
        && client_secret.is_none()
        && !request.clear_credentials
    {
        return Err(invalid(
            "更换 Client ID 时，请填写对应的新密钥或清除旧授权。",
        ));
    }
    let next = if request.clear_credentials {
        Secrets::default()
    } else {
        Secrets {
            client_secret: client_secret.or_else(|| old.client_secret.clone()),
            access_token: access_token.or_else(|| old.access_token.clone()),
        }
    };
    vault.write(&next)?;
    if let Err(error) = backend
        .database()
        .and_then(|db| db.put_setting(SETTING, &request.settings))
    {
        vault.write(&old)?;
        return Err(error);
    }
    let secrets = effective_secrets(next, &request.settings);
    Ok(view(request.settings, &secrets))
}
pub fn save_settings(backend: &Backend, request: SaveRequest) -> Result<SettingsView> {
    let _guard = backend.hikarinagi_lock.lock().map_err(|_| vault_error())?;
    save_with(backend, &NativeVault::new(backend)?, request)
}
pub fn authorization(backend: &Backend) -> Result<Authorization> {
    let _guard = backend.hikarinagi_lock.lock().map_err(|_| vault_error())?;
    let mut settings = settings(backend)?;
    if let Some(snapshot) = &backend.metadata_snapshot {
        settings.enabled = snapshot
            .sources
            .iter()
            .any(|s| s.provider == "hikarinagi" && s.enabled);
    }
    if settings.client_id.is_empty() {
        settings.client_id = super::hikarinagi_app::CLIENT_ID.into();
    }
    let secrets = effective_secrets(
        NativeVault::new(backend)?.read().unwrap_or_default(),
        &settings,
    );
    if !view(settings.clone(), &secrets).can_search {
        return Err(ServiceError(
            ErrorCode::PermissionDenied,
            "Hikarinagi 尚未授权，请在设置的“刮削来源”中配置；也可使用 Bangumi 或 VNDB。",
        ));
    }
    Ok(match settings.method {
        Method::AccessToken => Authorization::AccessToken(secrets.access_token.unwrap()),
        Method::ClientCredentials => {
            Authorization::ClientCredentials(settings.client_id, secrets.client_secret.unwrap())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[derive(Default)]
    struct MemoryVault(RefCell<Secrets>);
    impl Vault for MemoryVault {
        fn read(&self) -> Result<Secrets> {
            Ok(self.0.borrow().clone())
        }
        fn write(&self, secrets: &Secrets) -> Result<()> {
            *self.0.borrow_mut() = secrets.clone();
            Ok(())
        }
    }
    #[test]
    fn bundled_application_is_available_without_entering_credentials() {
        let config = Settings {
            client_id: super::super::hikarinagi_app::CLIENT_ID.into(),
            ..Settings::default()
        };
        let credentials = effective_secrets(Secrets::default(), &config);
        assert!(view(config, &credentials).can_search);
        assert!(credentials.client_secret.is_some());
    }
    #[test]
    fn availability_requires_matching_authorization_and_enabled_source() {
        let mut settings = Settings::default();
        let secrets = Secrets {
            client_secret: Some("fixture-secret".into()),
            access_token: None,
        };
        assert!(!view(settings.clone(), &secrets).can_search);
        settings.client_id = "own-app".into();
        assert!(view(settings.clone(), &secrets).can_search);
        settings.enabled = false;
        assert!(!view(settings.clone(), &secrets).can_search);
        settings.enabled = true;
        settings.method = Method::AccessToken;
        assert!(!view(settings.clone(), &secrets).can_search);
        assert!(
            view(
                settings,
                &Secrets {
                    access_token: Some("fixture-token".into()),
                    ..Secrets::default()
                }
            )
            .can_search
        );
    }
    #[test]
    fn save_keeps_secrets_out_of_database_and_response_and_preserves_on_blank() {
        let root = std::env::temp_dir().join(format!("gm-hikari-settings-{}", id()));
        let backend = Backend::open(root.join("data")).unwrap();
        let vault = MemoryVault::default();
        let request = || SaveRequest {
            settings: Settings {
                client_id: "own-app".into(),
                ..Settings::default()
            },
            client_secret: None,
            access_token: None,
            clear_credentials: false,
        };
        let mut first = request();
        first.client_secret = Some("fixture-private-secret".into());
        let first_view = save_with(&backend, &vault, first).unwrap();
        assert!(first_view.can_search);
        assert!(!serde_json::to_string(&first_view)
            .unwrap()
            .contains("fixture-private-secret"));
        let db_settings: serde_json::Value = backend
            .database()
            .unwrap()
            .setting(SETTING)
            .unwrap()
            .unwrap();
        assert!(db_settings.get("client_secret").is_none());
        assert!(db_settings.get("access_token").is_none());
        assert!(
            save_with(&backend, &vault, request())
                .unwrap()
                .has_client_secret
        );
        let mut changed = request();
        changed.settings.client_id = "other-app".into();
        assert!(save_with(&backend, &vault, changed).is_err());
        let mut clear = request();
        clear.clear_credentials = true;
        save_with(&backend, &vault, clear).unwrap();
        assert!(vault.read().unwrap().client_secret.is_none());
        drop(backend);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_secret_is_rejected_without_echoing_it() {
        assert!(secret(Some("bad\nsecret".into())).is_err());
        assert!(secret(Some("x".repeat(8193))).is_err());
    }
}
