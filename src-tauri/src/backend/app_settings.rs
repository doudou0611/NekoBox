//! Validated application preferences. Credentials have a separate native vault.
use super::*;
use serde::{Deserialize, Serialize};
pub const KEY: &str = "app.preferences";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub palette: String,
    pub glass_enabled: bool,
    pub startup_page: String,
    pub bangumi_cover_source: String,
    pub vndb_cover_source: String,
    pub tag_limit: usize,
    pub launch_wait_seconds: u64,
    pub ui_refresh_seconds: u64,
    pub locale_emulator_path: String,
    pub magpie_path: String,
    pub default_locale_emulator: bool,
    pub default_magpie: bool,
    pub proxy_enabled: bool,
    pub proxy_mode: String,
    pub proxy_url: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "light".into(),
            palette: "wisteria".into(),
            glass_enabled: true,
            startup_page: "home".into(),
            bangumi_cover_source: "hikarinagi".into(),
            vndb_cover_source: "hikarinagi".into(),
            tag_limit: 10,
            launch_wait_seconds: 15,
            ui_refresh_seconds: 15,
            locale_emulator_path: String::new(),
            magpie_path: String::new(),
            default_locale_emulator: false,
            default_magpie: false,
            proxy_enabled: false,
            proxy_mode: "system".into(),
            proxy_url: String::new(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !["light", "dark"].contains(&self.theme.as_str())
            || ![
                "wisteria", "sea", "forest", "rose", "amber", "graphite", "black", "white", "jade",
                "coral",
            ]
            .contains(&self.palette.as_str())
            || !["home", "games"].contains(&self.startup_page.as_str())
            || !["original", "hikarinagi"].contains(&self.bangumi_cover_source.as_str())
            || !["original", "hikarinagi"].contains(&self.vndb_cover_source.as_str())
            || !(1..=40).contains(&self.tag_limit)
            || !(1..=300).contains(&self.launch_wait_seconds)
            || !(1..=300).contains(&self.ui_refresh_seconds)
            || !["system", "manual"].contains(&self.proxy_mode.as_str())
        {
            return Err(invalid("设置值超出允许范围。"));
        }
        for path in [&self.locale_emulator_path, &self.magpie_path] {
            if !path.is_empty() && (!std::path::Path::new(path).is_absolute() || path.len() > 32768)
            {
                return Err(invalid("外部工具需要完整的文件路径。"));
            }
        }
        if !self.proxy_url.is_empty() || (self.proxy_enabled && self.proxy_mode == "manual") {
            let url =
                reqwest::Url::parse(&self.proxy_url).map_err(|_| invalid("代理地址无效。"))?;
            if !["http", "https", "socks5", "socks5h"].contains(&url.scheme())
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || url.path() != "/" && !url.path().is_empty()
            {
                return Err(invalid(
                    "代理只接受 HTTP、HTTPS 或 SOCKS5 主机和端口，不含账号。",
                ));
            }
        }
        Ok(())
    }
}
pub fn get(b: &Backend) -> Result<Settings> {
    if let Some(value) = &b.app_settings_snapshot {
        return Ok(value.clone());
    }
    let settings: Settings = b.database()?.setting(KEY)?.unwrap_or_default();
    settings.validate()?;
    Ok(settings)
}
pub fn save(b: &Backend, settings: Settings) -> Result<Settings> {
    settings.validate()?;
    b.database()?.put_setting(KEY, &settings)?;
    #[cfg(not(test))]
    super::network::configure(settings.clone());
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_credential_urls_and_invalid_limits() {
        let mut s = Settings {
            proxy_enabled: true,
            proxy_mode: "manual".into(),
            proxy_url: "http://user:secret@localhost:8080".into(),
            ..Settings::default()
        };
        assert!(s.validate().is_err());
        s.proxy_url = "socks5h://127.0.0.1:1080".into();
        assert!(s.validate().is_ok());
        s.tag_limit = 41;
        assert!(s.validate().is_err());
    }
}
