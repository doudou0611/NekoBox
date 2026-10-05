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
    pub gallery_columns: usize,
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
    pub sidebar_game_order: std::collections::BTreeMap<String, Vec<String>>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "light".into(),
            palette: "wisteria".into(),
            glass_enabled: true,
            startup_page: "home".into(),
            gallery_columns: 5,
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
            sidebar_game_order: Default::default(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.sidebar_game_order.len() > 1024
            || self
                .sidebar_game_order
                .values()
                .map(Vec::len)
                .sum::<usize>()
                > 100_000
            || self.sidebar_game_order.iter().any(|(group, games)| {
                group.is_empty()
                    || group.len() > 200
                    || games.iter().any(|game| game.is_empty() || game.len() > 200)
                    || games.iter().collect::<std::collections::HashSet<_>>().len() != games.len()
            })
        {
            return Err(invalid("侧栏游戏排序无效。"));
        }
        if !["light", "dark"].contains(&self.theme.as_str())
            || ![
                "wisteria", "sea", "forest", "rose", "amber", "graphite", "black", "white", "jade",
                "coral",
            ]
            .contains(&self.palette.as_str())
            || !["home", "games"].contains(&self.startup_page.as_str())
            || !(3..=9).contains(&self.gallery_columns)
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
    fn gallery_columns_default_bounds_and_persistence() {
        let legacy: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(legacy.gallery_columns, 5);
        let mut settings = legacy;
        for columns in 3..=9 {
            settings.gallery_columns = columns;
            assert!(settings.validate().is_ok());
        }
        for columns in [0, 2, 10, usize::MAX] {
            settings.gallery_columns = columns;
            assert!(settings.validate().is_err());
        }
        let root = std::env::temp_dir().join(id());
        let b = Backend::open(root.join("data")).unwrap();
        settings.gallery_columns = 9;
        save(&b, settings).unwrap();
        drop(b);
        let reopened = Backend::open(root.join("data")).unwrap();
        let saved = get(&reopened).unwrap();
        assert_eq!(saved.gallery_columns, 9);
        assert_eq!(saved.theme, "dark");
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sidebar_order_defaults_and_persistence_preserve_other_preferences() {
        let legacy: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert!(legacy.sidebar_game_order.is_empty());
        let root = std::env::temp_dir().join(id());
        let b = Backend::open(root.join("data")).unwrap();
        let mut settings = legacy;
        settings
            .sidebar_game_order
            .insert("favorites".into(), vec!["game-b".into(), "game-a".into()]);
        save(&b, settings).unwrap();
        drop(b);
        let reopened = Backend::open(root.join("data")).unwrap();
        let saved = get(&reopened).unwrap();
        assert_eq!(saved.theme, "dark");
        assert_eq!(saved.sidebar_game_order["favorites"], ["game-b", "game-a"]);
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sidebar_order_rejects_duplicate_and_empty_ids() {
        let mut settings = Settings::default();
        for games in [vec!["game-a".into(), "game-a".into()], vec![String::new()]] {
            settings
                .sidebar_game_order
                .insert("favorites".into(), games);
            assert!(settings.validate().is_err());
        }
    }
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
