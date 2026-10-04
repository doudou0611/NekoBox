use super::{
    credential::{NativeVault, Vault},
    *,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
pub struct Settings {
    pub has_api_token: bool,
}
#[derive(Deserialize)]
pub struct SaveRequest {
    pub api_token: Option<String>,
    #[serde(default)]
    pub clear_api_token: bool,
}
#[derive(Serialize)]
pub struct Connection {
    pub username: String,
    pub permissions: Vec<String>,
}
fn vault(backend: &Backend) -> Result<NativeVault> {
    NativeVault::new(backend, "dev.galgame.manager.vndb")
}
pub fn token(backend: &Backend) -> Result<Option<String>> {
    if cfg!(test) {
        return Ok(None);
    }
    let _guard = backend
        .vndb_lock
        .lock()
        .map_err(|_| invalid("VNDB 授权服务需要重新启动。"))?;
    vault(backend)?.read()
}
pub fn get(backend: &Backend) -> Result<Settings> {
    Ok(Settings {
        has_api_token: token(backend)?.is_some(),
    })
}
fn save_with(vault: &impl Vault, request: SaveRequest) -> Result<Settings> {
    let token = request
        .api_token
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    if request.clear_api_token && token.is_some() {
        return Err(invalid("清除 VNDB Token 时不能同时填写新 Token。"));
    }
    if request.clear_api_token {
        vault.delete()?;
    } else if let Some(token) = token {
        vault.write(super::credential::validate(token)?)?;
    }
    Ok(Settings {
        has_api_token: vault.read()?.is_some(),
    })
}
pub fn save(backend: &Backend, request: SaveRequest) -> Result<Settings> {
    let _guard = backend
        .vndb_lock
        .lock()
        .map_err(|_| invalid("VNDB 授权服务需要重新启动。"))?;
    save_with(&vault(backend)?, request)
}
pub fn test(backend: &Backend) -> Result<Connection> {
    let token = token(backend)?.ok_or_else(|| invalid("请先填写并保存自己的 VNDB API Token。"))?;
    let response = super::network::builder()
        .timeout(std::time::Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| invalid("无法创建 VNDB 请求。"))?
        .get("https://api.vndb.org/kana/authinfo")
        .header("Authorization", format!("Token {token}"))
        .send()
        .map_err(|_| ServiceError(ErrorCode::NetworkUnavailable, "VNDB 暂时无法连接。"))?;
    if !response.status().is_success() {
        return Err(ServiceError(
            match response.status().as_u16() {
                401 | 403 => ErrorCode::PermissionDenied,
                429 => ErrorCode::RateLimited,
                _ => ErrorCode::NetworkUnavailable,
            },
            "VNDB 授权验证失败，请检查 Token、网络或稍后重试。",
        ));
    }
    let v: serde_json::Value = response
        .json()
        .map_err(|_| invalid("VNDB 授权响应无效。"))?;
    Ok(Connection {
        username: v
            .get("username")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid("VNDB 账户响应缺少用户名。"))?
            .into(),
        permissions: v
            .get("permissions")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid("VNDB 账户响应缺少权限。"))?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blank_preserves_clear_removes_and_response_never_returns_token() {
        let vault = crate::backend::credential::MemoryVault::default();
        let result = save_with(
            &vault,
            SaveRequest {
                api_token: Some("fixture-private-token".into()),
                clear_api_token: false,
            },
        )
        .unwrap();
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("fixture-private-token"));
        assert!(
            save_with(
                &vault,
                SaveRequest {
                    api_token: None,
                    clear_api_token: false
                }
            )
            .unwrap()
            .has_api_token
        );
        assert!(save_with(
            &vault,
            SaveRequest {
                api_token: Some("bad\ntoken".into()),
                clear_api_token: false
            }
        )
        .is_err());
        assert!(save_with(
            &vault,
            SaveRequest {
                api_token: Some("new-token".into()),
                clear_api_token: true
            }
        )
        .is_err());
        assert!(
            !save_with(
                &vault,
                SaveRequest {
                    api_token: None,
                    clear_api_token: true
                }
            )
            .unwrap()
            .has_api_token
        );
    }
}
