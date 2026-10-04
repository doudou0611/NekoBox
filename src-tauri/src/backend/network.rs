//! The same proxy policy is applied to every native HTTP service.
use super::app_settings::Settings;
use std::sync::{OnceLock, RwLock};

static POLICY: OnceLock<RwLock<Settings>> = OnceLock::new();

pub fn configure(settings: Settings) {
    if let Ok(mut policy) = POLICY
        .get_or_init(|| RwLock::new(Settings::default()))
        .write()
    {
        *policy = settings;
    }
}

pub fn builder() -> reqwest::blocking::ClientBuilder {
    let settings = POLICY
        .get_or_init(|| RwLock::new(Settings::default()))
        .read()
        .map(|p| p.clone())
        .unwrap_or_default();
    builder_with(&settings)
}

pub fn builder_with(settings: &Settings) -> reqwest::blocking::ClientBuilder {
    let builder = reqwest::blocking::Client::builder();
    if !settings.proxy_enabled {
        return builder.no_proxy();
    }
    if settings.proxy_mode == "system" {
        return builder;
    }
    match reqwest::Proxy::all(&settings.proxy_url) {
        Ok(proxy) => builder.no_proxy().proxy(proxy),
        // Persisted settings are validated before use. Never use system proxy
        // as a silent substitute for an invalid manual configuration.
        Err(_) => builder.no_proxy(),
    }
}
/// Read the same policy for each updater request, including download retries.
pub(crate) fn configure_async(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    let settings = POLICY
        .get_or_init(|| RwLock::new(Settings::default()))
        .read()
        .map(|p| p.clone())
        .unwrap_or_default();
    let builder = builder
        .connect_timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 || attempt.url().scheme() != "https" {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }));
    if !settings.proxy_enabled {
        return builder.no_proxy();
    }
    if settings.proxy_mode == "system" {
        return builder;
    }
    match reqwest::Proxy::all(&settings.proxy_url) {
        Ok(proxy) => builder.no_proxy().proxy(proxy),
        Err(_) => builder.no_proxy(),
    }
}
#[derive(serde::Deserialize)]
pub struct ImageRequest {
    pub url: String,
    #[serde(default)]
    pub avatar: bool,
}
pub fn cache_image(b: &super::Backend, q: ImageRequest) -> super::Result<String> {
    let provider = if super::bangumi::trusted_image(&q.url) {
        "bangumi"
    } else if super::bangumi::trusted_vndb_image(&q.url) {
        "vndb"
    } else if super::hikarinagi::trusted_image(&q.url) {
        "hikarinagi"
    } else {
        return Err(super::invalid("远程图片地址不受信任。"));
    };
    let mut snapshot = b.clone();
    if q.avatar {
        let mut settings = super::app_settings::get(b)?;
        settings.bangumi_cover_source = "original".into();
        settings.vndb_cover_source = "original".into();
        snapshot.app_settings_snapshot = Some(settings);
    }
    let path = super::bangumi::cache_image_for(&snapshot, provider, &q.url)?;
    Ok(path)
}
