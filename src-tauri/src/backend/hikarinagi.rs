//! Hikarinagi official v3 metadata API adapter.
//!
//! User-authorized application identity is bundled. Personal account credentials
//! live in the native vault and are never exposed in metadata responses.
use super::*;
use reqwest::{blocking::Client, header};
use serde_json::Value;
use std::time::Duration;

const API_BASE: &str = "https://api.hikarinagi.org/v3";
const TOKEN_URL: &str = "https://id.hikarinagi.org/oidc/token";
const USER_AGENT: &str = "GalgameManager/0.1 (Hikarinagi metadata; local desktop app)";

fn oauth_error(status: u16, code: Option<&str>) -> ServiceError {
    ServiceError(
        if status == 429 { ErrorCode::RateLimited } else { ErrorCode::PermissionDenied },
        match code {
            Some("invalid_scope") => "Hikarinagi 应用尚未允许 catalog:full 权限，请在开发者控制台的应用权限中启用并保存。",
            Some("unsupported_grant_type" | "unauthorized_client") => "Hikarinagi 应用不支持应用级授权，请使用服务端应用及 client_credentials 授权。",
            _ => "Hikarinagi 应用授权失败，请检查开发者控制台的应用权限和授权状态。",
        },
    )
}

fn client() -> Result<Client> {
    super::network::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| {
            ServiceError(
                ErrorCode::NetworkUnavailable,
                "无法初始化 Hikarinagi 网络请求。",
            )
        })
}

fn application_token(backend: &Backend, http: &Client) -> Result<String> {
    let (client_id, secret) = match super::hikarinagi_settings::authorization(backend)? {
        super::hikarinagi_settings::Authorization::AccessToken(token) => return Ok(token),
        super::hikarinagi_settings::Authorization::ClientCredentials(id, secret) => (id, secret),
    };
    let response = super::http_retry::send_checked(
        || {
            http.post(TOKEN_URL)
                .basic_auth(&client_id, Some(&secret))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ACCEPT, "application/json")
                .body("grant_type=client_credentials&scope=catalog%3Afull")
        },
        || backend.ensure_search_active(),
    )?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let value = response.json::<Value>().unwrap_or(Value::Null);
        return Err(oauth_error(
            status,
            value.get("error").and_then(Value::as_str),
        ));
    }
    let value = response.json::<Value>().map_err(|_| {
        ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi OAuth 返回格式无效。",
        )
    })?;
    value
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi OAuth 没有返回访问令牌。",
        ))
}

fn get_json(backend: &Backend, http: &Client, token: &str, url: &str) -> Result<Value> {
    let response = super::http_retry::send_checked(
        || {
            http.get(url)
                .bearer_auth(token)
                .header(header::ACCEPT, "application/json")
        },
        || backend.ensure_search_active(),
    )?;
    if !response.status().is_success() {
        return Err(ServiceError(
            match response.status().as_u16() {
                401 | 403 => ErrorCode::PermissionDenied,
                429 => ErrorCode::RateLimited,
                _ => ErrorCode::NetworkUnavailable,
            },
            if response.status().as_u16() == 401 {
                "Hikarinagi 账户授权已失效。"
            } else {
                "Hikarinagi 资料请求失败，请稍后重试。"
            },
        ));
    }
    let value = response
        .json::<Value>()
        .map_err(|_| ServiceError(ErrorCode::InvalidResponse, "Hikarinagi 返回资料格式无效。"))?;
    if value.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi 未能返回资料，请检查授权或稍后重试。",
        ));
    }
    Ok(value)
}

// Only an expired/revoked credential retries with the application identity.
// A valid account's content preferences (403) must remain effective.
fn metadata_json(backend: &Backend, http: &Client, url: &str) -> Result<Value> {
    backend.ensure_search_active()?;
    if let Ok(Some(token)) = super::hikarinagi_account::token(backend) {
        match get_json(backend, http, &token, url) {
            Err(error) if error.1 == "Hikarinagi 账户授权已失效。" => {}
            result => return result,
        }
    }
    let token = application_token(backend, http)?;
    get_json(backend, http, &token, url)
}

pub fn test_connection(backend: &Backend) -> Result<bool> {
    let http = client()?;
    let value = metadata_json(
        backend,
        &http,
        &format!("{API_BASE}/search?q=ATRI&types=galgame&page=1&page_size=1"),
    )?;
    if value
        .get("data")
        .and_then(|data| data.get("items"))
        .and_then(Value::as_array)
        .is_none()
    {
        return Err(ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi 搜索接口返回格式无效。",
        ));
    }
    Ok(true)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn cover(value: &Value) -> Option<String> {
    value
        .get("covers")
        .and_then(Value::as_array)
        .and_then(|covers| covers.iter().find_map(|item| text(item, "url")))
        .or_else(|| value.get("cover").and_then(|item| text(item, "url")))
}

fn date(value: &Value, key: &str) -> Option<String> {
    text(value, key).map(|value| value.chars().take(10).collect())
}

pub(super) fn trusted_image(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port_or_known_default() == Some(443)
            && url.host_str().is_some_and(|host| {
                host == "images.yurari.moe"
                    || host == "imagesp.yurari.moe"
                    || host == "hikarinagi.org"
                    || host.ends_with(".hikarinagi.org")
            })
    })
}

fn confidence(query: &str, title: &str, subtitle: Option<&str>) -> f64 {
    let normalize = |value: &str| {
        value
            .chars()
            .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '_' | ':' | '：'))
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let query = normalize(query);
    [Some(title), subtitle]
        .into_iter()
        .flatten()
        .map(normalize)
        .find_map(|candidate| {
            if candidate == query {
                Some(1.0)
            } else if !candidate.is_empty()
                && (candidate.contains(&query) || query.contains(&candidate))
            {
                Some(0.9)
            } else {
                None
            }
        })
        .unwrap_or(0.55)
}

pub fn search(
    backend: &Backend,
    request: &crate::domain::requests::SearchMetadataRequest,
) -> Result<Vec<crate::domain::models::MetadataCandidate>> {
    let query = request.query.trim();
    if query.is_empty() || query.chars().count() > 200 {
        return Err(invalid("资料搜索关键词需要 1～200 个字符。"));
    }
    if cfg!(test) {
        return Ok(Vec::new());
    }
    let http = client()?;
    let url = reqwest::Url::parse_with_params(
        &format!("{API_BASE}/search"),
        &[
            ("q", query),
            ("types", "galgame"),
            ("page", "1"),
            ("page_size", "20"),
        ],
    )
    .map_err(|_| ServiceError(ErrorCode::InvalidRequest, "Hikarinagi 搜索关键词无效。"))?;
    let value = metadata_json(backend, &http, url.as_str())?;
    let fetched_at = now();
    let items = value
        .get("data")
        .and_then(|data| data.get("items"))
        .and_then(Value::as_array)
        .ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi 搜索结果结构无效。",
        ))?;
    Ok(items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("galgame"))
        .filter_map(|item| {
            let remote_id = item.get("id")?.as_i64()?.to_string();
            let title = text(item, "title")?;
            let subtitle = text(item, "subtitle");
            let cover_url = item.get("cover").and_then(|cover| text(cover, "url"));
            Some(crate::domain::models::MetadataCandidate {
                provider: "hikarinagi".into(),
                remote_id,
                title: title.clone(),
                subtitle,
                cover_url,
                has_chinese_description: translated_intro_hint(item),
                confidence: confidence(query, &title, text(item, "subtitle").as_deref()),
                matched_fields: vec!["title".into()],
                explanation: "Hikarinagi 搜索结果，需人工确认".into(),
                fetched_at: fetched_at.clone(),
                cached: false,
            })
        })
        .take(20)
        .collect())
}

pub fn confirm(
    backend: &Backend,
    request: &crate::domain::requests::ConfirmMetadataMatchRequest,
) -> Result<crate::domain::models::MatchResult> {
    backend
        .database()?
        .ensure_metadata_unlocked(&request.game_id)?;
    if request.provider != "hikarinagi" || !request.remote_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalid("Hikarinagi 远程标识无效。"));
    }
    backend.database()?.get_game(&request.game_id)?;
    if cfg!(test) {
        return Err(ServiceError(
            ErrorCode::NetworkUnavailable,
            "测试环境不调用 Hikarinagi 网络。",
        ));
    }
    let http = client()?;
    let value = metadata_json(
        backend,
        &http,
        &format!("{API_BASE}/galgames/{}", request.remote_id),
    )?;
    let data = value.get("data").unwrap_or(&value);
    let title_original = text(data, "origin_title");
    let title_translated = text(data, "trans_title");
    let title = title_translated
        .clone()
        .or_else(|| title_original.clone())
        .ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi 资料缺少作品标题。",
        ))?;
    let rating = data
        .get("rating")
        .and_then(|rating| rating.get("score"))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && (0.0..=10.0).contains(value))
        .map(|value| format!("{value:.2}"));
    let tags = data
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| text(tag, "name"))
                .take(24)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|value| !value.is_empty());
    let cover_url = cover(data);
    let mut fields = [
        ("title", Some(title.as_str())),
        ("title_zh", title_translated.as_deref()),
        ("title_ja", title_original.as_deref()),
        ("developer", text(data, "developer").as_deref()),
        ("release_date", date(data, "release_date").as_deref()),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.map(|value| (name.to_owned(), value.to_owned())))
    .chain(rating.map(|value| ("source_rating".into(), value)))
    .chain(tags.map(|value| ("source_tags".into(), value)))
    .chain(cover_url.clone().map(|url| ("cover_url".into(), url)))
    .collect::<Vec<_>>();
    fields.extend(intro_fields(data));
    let fetched_at = now();
    let result = backend.database()?.apply_remote_fields(
        &request.game_id,
        "hikarinagi",
        &request.remote_id,
        &fields,
        &[],
        &fetched_at,
        true,
    )?;
    if let Some(url) = cover_url.filter(|_| {
        super::metadata_sources::needs_cover(backend, &request.game_id, "hikarinagi")
            .unwrap_or(true)
    }) {
        let cached = crate::backend::bangumi::cache_image_for(backend, "hikarinagi", &url)?;
        backend.database()?.apply_remote_fields(
            &request.game_id,
            "hikarinagi",
            &request.remote_id,
            &[("cover_path".into(), cached)],
            &[],
            &fetched_at,
            false,
        )?;
    }
    Ok(result)
}

fn translated_intro_hint(data: &Value) -> Option<bool> {
    // Search payloads may omit intros. Omission is unknown, not absence.
    data.get("trans_intro")
        .map(|_| text(data, "trans_intro").is_some())
}

fn intro_fields(data: &Value) -> Vec<(String, String)> {
    let mut fields = super::metadata_text::description_fields(text(data, "origin_intro"));
    if let Some(translated) = text(data, "trans_intro") {
        // Match the official site's trans_intro || origin_intro rule. Names,
        // quotations and punctuation never change the authoritative field choice.
        let translated = translated.replace("\r\n", "\n");
        fields.retain(|(field, _)| field != "description_zh");
        if !fields.iter().any(|(field, _)| field == "description") {
            fields.push(("description".into(), translated.clone()));
        }
        fields.push(("description_zh".into(), translated));
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_intro_matches_the_website_without_language_heuristics() {
        for translated in [
            "主人公遇见アトリ，一起踏上旅途。",
            "中文简介引用了台词「ただいま」。",
            "私立赞咲良 （さんさら）的故事。",
            "私立赞咲良〔さんさら〕的故事。",
            "私立赞咲良（さんさら ）的故事。",
            "主角・莲佛雪之进的故事。",
            "Brain-machine Interface，简称BMI，是连接大脑和机器的技术。",
            "Scarlet Ikaruga Wisteria 的故事。",
            "😀✨",
            "Official translated field in English.",
            "公式翻訳フィールドの文章。",
            "第一段中文。\r\n第二段中文与日文名アトリ。",
        ] {
            let data = serde_json::json!({
                "origin_intro": "主人公は高校生。",
                "trans_intro": translated,
            });
            assert_eq!(translated_intro_hint(&data), Some(true));
            let expected = translated.replace("\r\n", "\n");
            let fields = intro_fields(&data);
            assert!(fields.contains(&("description_zh".into(), expected.clone())));
            assert!(fields.contains(&("description".into(), "主人公は高校生。".into())));

            // The database previously repeated language detection, so parsing alone
            // could not guarantee that the selected source prose reached the UI.
            let mut db = crate::database::Database::in_memory().unwrap();
            let game = db
                .import_installation(
                    "/fixture/hikari-mixed-intro",
                    "作品",
                    None,
                    crate::domain::protocol::InstallSource::Manual,
                    &[],
                    "fixture",
                )
                .unwrap();
            db.apply_remote_fields(&game, "hikarinagi", "456", &fields, &[], &now(), true)
                .unwrap();
            assert_eq!(
                db.get_game(&game).unwrap().description.as_deref(),
                Some(expected.as_str())
            );
            db.save_translation_config(&super::super::translation::Settings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
            db.apply_translation_fields(
                &game,
                &[(
                    "hikarinagi".into(),
                    "description".into(),
                    "description_zh_translation".into(),
                    "旧机器译文。".into(),
                )],
            )
            .unwrap();
            assert_eq!(
                db.get_game(&game).unwrap().description.as_deref(),
                Some(expected.as_str())
            );
        }
    }

    #[test]
    fn absent_translated_intro_has_an_explicit_original_fallback() {
        assert_eq!(translated_intro_hint(&serde_json::json!({})), None);
        for translated in [
            Value::Null,
            serde_json::json!(""),
            serde_json::json!(" \r\n "),
            serde_json::json!(123),
        ] {
            let data =
                serde_json::json!({"origin_intro": "主人公は高校生。", "trans_intro": translated});
            assert_eq!(translated_intro_hint(&data), Some(false));
            assert_eq!(
                intro_fields(&data),
                vec![("description".into(), "主人公は高校生。".into())]
            );
        }
        let fields = intro_fields(&serde_json::json!({"trans_intro": "中文简介中的アトリ。"}));
        assert!(fields.contains(&("description".into(), "中文简介中的アトリ。".into())));
        assert!(fields.contains(&("description_zh".into(), "中文简介中的アトリ。".into())));
        assert!(intro_fields(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn real_bilingual_intros_keep_chinese_readings_on_import_and_refresh() {
        // Public v3 intro fields captured on 2026-10-04: a reading (793),
        // a Japanese middle dot (794), Latin terminology (785), and plain Chinese (789).
        for fixture in [
            include_str!("fixtures/hikarinagi-793-intro.json"),
            include_str!("fixtures/hikarinagi-789-intro.json"),
            include_str!("fixtures/hikarinagi-794-intro.json"),
            include_str!("fixtures/hikarinagi-785-intro.json"),
        ] {
            let data: Value = serde_json::from_str(fixture).unwrap();
            let remote_id = data["id"].as_i64().unwrap().to_string();
            let translated = text(&data, "trans_intro").unwrap();
            assert_eq!(
                translated_intro_hint(&data),
                Some(true),
                "Chinese search hint for Hikarinagi {remote_id}"
            );
            let fields = intro_fields(&data);
            let mut db = crate::database::Database::in_memory().unwrap();
            let game = db
                .import_installation(
                    &format!("/fixture/hikari-{remote_id}"),
                    "作品",
                    None,
                    crate::domain::protocol::InstallSource::Manual,
                    &[],
                    "fixture",
                )
                .unwrap();
            db.apply_remote_fields(&game, "hikarinagi", &remote_id, &fields, &[], &now(), true)
                .unwrap();
            assert_eq!(
                db.get_game(&game).unwrap().description.as_deref(),
                Some(translated.as_str()),
                "Imported description for Hikarinagi {remote_id}"
            );

            // Reproduce the old parser's stored Japanese selection, then refresh.
            let old_fields = [("description".into(), text(&data, "origin_intro").unwrap())];
            db.apply_remote_fields(
                &game,
                "hikarinagi",
                &remote_id,
                &old_fields,
                &[],
                &now(),
                true,
            )
            .unwrap();
            assert_ne!(
                db.get_game(&game).unwrap().description.as_deref(),
                Some(translated.as_str())
            );
            db.apply_remote_fields(&game, "hikarinagi", &remote_id, &fields, &[], &now(), true)
                .unwrap();
            assert_eq!(
                db.get_game(&game).unwrap().description.as_deref(),
                Some(translated.as_str()),
                "Refreshed description for Hikarinagi {remote_id}"
            );
        }
    }

    #[test]
    fn official_cdn_covers_are_allowed_but_credentials_and_lookalikes_are_rejected() {
        assert!(trusted_image(
            "https://images.yurari.moe/galgame/fixture.jpg"
        ));
        assert!(trusted_image(
            "https://imagesp.yurari.moe/galgame/fixture.jpg"
        ));
        for url in [
            "http://images.yurari.moe/a.jpg",
            "https://images.yurari.moe.evil.invalid/a.jpg",
            "https://user:secret@images.yurari.moe/a.jpg",
            "https://images.yurari.moe:444/a.jpg",
            "https://127.0.0.1/a.jpg",
            "https://other.yurari.moe/a.jpg",
        ] {
            assert!(!trusted_image(url));
        }
    }
    #[test]
    fn oauth_errors_explain_permission_without_returning_remote_secrets() {
        assert!(oauth_error(400, Some("invalid_scope"))
            .1
            .contains("catalog:full"));
        assert!(oauth_error(400, Some("unauthorized_client"))
            .1
            .contains("服务端应用"));
        assert_eq!(oauth_error(429, None).0, ErrorCode::RateLimited);
        assert!(!oauth_error(400, Some("fixture-private-secret"))
            .1
            .contains("fixture-private-secret"));
    }

    #[test]
    fn prefers_source_chinese_intro_without_mislabeling_original_japanese() {
        let fields = intro_fields(&serde_json::json!({
            "origin_intro": "主人公は女の子たちと暮らす。",
            "trans_intro": "主人公与小春、小夏共同生活。"
        }));
        assert!(fields.contains(&(
            "description_zh".into(),
            "主人公与小春、小夏共同生活。".into()
        )));
        assert!(fields.contains(&("description".into(), "主人公は女の子たちと暮らす。".into())));
        // Exercise the same fields-to-database projection used by confirmation,
        // prepared import, and refresh, rather than checking parsing alone.
        let mut db = crate::database::Database::in_memory().unwrap();
        let game = db
            .import_installation(
                "/fixture/hikari-intro",
                "作品",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        db.apply_remote_fields(&game, "hikarinagi", "456", &fields, &[], &now(), true)
            .unwrap();
        assert_eq!(
            db.get_game(&game).unwrap().description.as_deref(),
            Some("主人公与小春、小夏共同生活。")
        );

        let untranslated =
            intro_fields(&serde_json::json!({"origin_intro":"彼女は主人公の同級生。"}));
        assert_eq!(untranslated.len(), 1);
        assert_eq!(untranslated[0].0, "description");
    }
}
