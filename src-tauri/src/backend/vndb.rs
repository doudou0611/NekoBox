use super::*;
use crate::{
    database::Database,
    domain::{models::*, requests::*, ErrorCode},
};
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://api.vndb.org/kana/vn";
const RELEASE_API: &str = "https://api.vndb.org/kana/release";

#[derive(Debug, Clone)]
struct Entry {
    id: String,
    title: String,
    title_zh: Option<String>,
    title_en: Option<String>,
    alt_title: Option<String>,
    cover_url: Option<String>,
    developer: Option<String>,
    release_date: Option<String>,
    rating: Option<f64>,
    tags: Vec<String>,
    description: Option<String>,
}

fn network_error(status: reqwest::StatusCode) -> ServiceError {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        ServiceError(ErrorCode::RateLimited, "VNDB 请求过于频繁，请稍后再试。")
    } else {
        ServiceError(
            ErrorCode::NetworkUnavailable,
            "VNDB 暂时无法访问，请稍后再试。",
        )
    }
}

fn request_json(backend: &Backend, payload: Value) -> Result<Value> {
    request_json_at(backend, API, payload)
}

fn request_json_at(backend: &Backend, api: &str, payload: Value) -> Result<Value> {
    let client = super::network::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("GalgameManager/0.1 (local metadata search)")
        .build()
        .map_err(|_| ServiceError(ErrorCode::NetworkUnavailable, "无法初始化 VNDB 网络请求。"))?;
    let token = super::vndb_settings::token(backend).unwrap_or(None);
    let mut response = super::http_retry::send_checked(
        || {
            let request = client.post(api).json(&payload);
            if let Some(token) = &token {
                request.header("Authorization", format!("Token {token}"))
            } else {
                request
            }
        },
        || backend.ensure_search_active(),
    )?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED && token.is_some() {
        response = super::http_retry::send_checked(
            || client.post(api).json(&payload),
            || backend.ensure_search_active(),
        )?;
    }
    if !response.status().is_success() {
        return Err(network_error(response.status()));
    }
    response
        .json::<Value>()
        .map_err(|_| ServiceError(ErrorCode::InvalidResponse, "VNDB 返回了无法解析的资料。"))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn publishers_from_releases(value: &Value, remote_id: &str) -> Result<Option<String>> {
    let releases = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "VNDB 发行资料结构无效。",
        ))?;
    let mut names = Vec::new();
    for release in releases {
        let complete = release
            .get("vns")
            .and_then(Value::as_array)
            .is_some_and(|vns| {
                vns.iter().any(|vn| {
                    string_field(vn, "id").as_deref() == Some(remote_id)
                        && string_field(vn, "rtype").as_deref() == Some("complete")
                })
            });
        if !complete
            || release.get("official").and_then(Value::as_bool) != Some(true)
            || release.get("patch").and_then(Value::as_bool) != Some(false)
        {
            continue;
        }
        for producer in release
            .get("producers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if producer.get("publisher").and_then(Value::as_bool) != Some(true) {
                continue;
            }
            if let Some(name) = string_field(producer, "name")
                .or_else(|| string_field(producer, "original"))
                .filter(|name| !names.contains(name))
            {
                names.push(name);
            }
        }
    }
    Ok((!names.is_empty()).then(|| names.join("、")))
}

fn publisher_for(backend: &Backend, remote_id: &str) -> Result<Option<String>> {
    let query = format!("publishers:{remote_id}");
    if let Some((response, _)) = backend
        .database()?
        .metadata_cache_response("vndb", &query)?
    {
        let value: Value = serde_json::from_str(&response).map_err(|_| {
            ServiceError(ErrorCode::InvalidResponse, "本地 VNDB 发行商缓存已损坏。")
        })?;
        if let Some(publisher) = string_field(&value, "publisher") {
            return Ok(Some(publisher));
        }
    }
    if cfg!(test) {
        return Ok(None);
    }
    let mut releases = Vec::new();
    for page in 1..=10 {
        let value = request_json_at(
            backend,
            RELEASE_API,
            json!({
                "filters": ["and",
                    ["vn", "=", ["id", "=", remote_id]],
                    ["official", "=", 1], ["patch", "!=", 1], ["rtype", "=", "complete"]
                ],
                "fields": "id,official,patch,vns{id,rtype},producers{name,original,publisher}",
                "sort": "released", "results": 100, "page": page
            }),
        )?;
        releases.extend(
            value
                .get("results")
                .and_then(Value::as_array)
                .ok_or(ServiceError(
                    ErrorCode::InvalidResponse,
                    "VNDB 发行资料结构无效。",
                ))?
                .iter()
                .cloned(),
        );
        let more = value
            .get("more")
            .and_then(Value::as_bool)
            .ok_or(ServiceError(
                ErrorCode::InvalidResponse,
                "VNDB 发行资料分页无效。",
            ))?;
        if !more {
            let publisher = publishers_from_releases(&json!({"results": releases}), remote_id)?;
            if let Some(publisher) = &publisher {
                // Keep this separate from VN search responses. There is no VN
                // id inside the cache JSON, so it cannot be mistaken for a VN entry.
                let response = json!({"publisher": publisher}).to_string();
                backend.database()?.save_metadata_cache(
                    "vndb",
                    &query,
                    Some(&response),
                    "success",
                    None,
                    &now(),
                )?;
            }
            return Ok(publisher);
        }
    }
    Err(ServiceError(
        ErrorCode::InvalidResponse,
        "VNDB 发行版本过多，暂时无法补全发行商。",
    ))
}

fn title_in_language(value: &Value, prefix: &str) -> Option<String> {
    let titles = value.get("titles").and_then(Value::as_array)?;
    let mut fallback = None;
    for title in titles {
        let language = title
            .get("lang")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !language.starts_with(prefix) {
            continue;
        }
        let value = string_field(title, "title");
        if (language == "zh-Hans" || language == "zh-CN") && value.is_some() {
            return value;
        }
        fallback = value.or(fallback);
    }
    fallback
}

fn parse_entry(value: &Value) -> Result<Entry> {
    let id = string_field(value, "id").ok_or(ServiceError(
        ErrorCode::InvalidResponse,
        "VNDB 资料缺少作品标识。",
    ))?;
    let title = string_field(value, "title").ok_or(ServiceError(
        ErrorCode::InvalidResponse,
        "VNDB 资料缺少作品标题。",
    ))?;
    let developer = value
        .get("developers")
        .and_then(Value::as_array)
        .map(|developers| {
            developers
                .iter()
                .filter_map(|developer| string_field(developer, "name"))
                .collect::<Vec<_>>()
                .join("、")
        })
        .filter(|value| !value.is_empty());
    Ok(Entry {
        id,
        title,
        title_zh: title_in_language(value, "zh"),
        title_en: title_in_language(value, "en"),
        alt_title: title_in_language(value, "ja").or_else(|| string_field(value, "alttitle")),
        cover_url: value
            .get("image")
            .and_then(|image| string_field(image, "url"))
            .filter(|url| crate::backend::bangumi::trusted_vndb_image(url)),
        developer,
        release_date: string_field(value, "released"),
        rating: value
            .get("rating")
            .and_then(Value::as_f64)
            .map(|rating| rating / 10.0)
            .filter(|rating| rating.is_finite() && (0.0..=10.0).contains(rating)),
        tags: value
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| string_field(tag, "name"))
                    .take(24)
                    .collect()
            })
            .unwrap_or_default(),
        description: string_field(value, "description"),
    })
}

fn entries_from_response(value: &Value) -> Result<Vec<Entry>> {
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "VNDB 返回结果结构无效。",
        ))?;
    results.iter().map(parse_entry).collect()
}

fn normalized(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '_' | ':' | '：'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_release_tag(value: &str) -> bool {
    let value = value.to_lowercase();
    [
        "汉化",
        "中文",
        "简体",
        "繁体",
        "steam",
        "patch",
        "edition",
        "demo",
        "trial",
        "版本",
        "修正",
        "破解",
        "免安装",
        "v1.",
        "ver.",
        "beta",
    ]
    .iter()
    .any(|tag| value.contains(tag))
}

/// Keep the local title as the source of truth while removing common release tags
/// that would otherwise prevent an exact VNDB title match.
pub(super) fn automatic_query(value: &str) -> String {
    let chars: Vec<char> = value.trim().chars().collect();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        let Some(close) = (match chars[index] {
            '[' => Some(']'),
            '【' => Some('】'),
            '(' => Some(')'),
            _ => None,
        }) else {
            output.push(chars[index]);
            index += 1;
            continue;
        };
        let start = index;
        let Some(end) = chars[index + 1..].iter().position(|c| *c == close) else {
            output.extend(chars[index..].iter().copied());
            break;
        };
        let end = index + 1 + end;
        let inner: String = chars[index + 1..end].iter().collect();
        if !is_release_tag(&inner) {
            output.extend(chars[start..=end].iter().copied());
        }
        index = end + 1;
    }
    let query = output.trim();
    if query.is_empty() {
        value.trim().to_owned()
    } else {
        query.to_owned()
    }
}

fn clean_vndb_description(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '[' {
            if let Some(offset) = chars[index..].iter().position(|c| *c == ']') {
                let end = index + offset;
                let tag: String = chars[index + 1..end].iter().collect();
                if tag.trim().eq_ignore_ascii_case("br") {
                    output.push('\n');
                }
                index = end + 1;
                continue;
            }
        }
        if chars[index] != '\r' {
            output.push(chars[index]);
        }
        index += 1;
    }
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn localized_description(value: &str) -> (String, Option<String>) {
    let cleaned = clean_vndb_description(value);
    let translated = super::metadata_text::description_fields(Some(cleaned.clone()))
        .into_iter()
        .find_map(|(field, value)| (field == "description_zh").then_some(value));
    (cleaned, translated)
}

fn confidence(query: &str, entry: &Entry) -> (f64, Vec<String>, String) {
    let query = normalized(query);
    let titles = [
        ("title", Some(entry.title.as_str())),
        ("title_zh", entry.title_zh.as_deref()),
        ("title_en", entry.title_en.as_deref()),
        ("alternative_title", entry.alt_title.as_deref()),
    ];
    for (field, value) in titles
        .iter()
        .filter_map(|(field, value)| value.map(|value| (*field, normalized(value))))
    {
        if !value.is_empty() && query == value {
            return (1.0, vec![field.into()], "标题完全匹配".into());
        }
    }
    for (field, value) in titles
        .iter()
        .filter_map(|(field, value)| value.map(|value| (*field, normalized(value))))
    {
        if !value.is_empty() && (value.contains(&query) || query.contains(&value)) {
            return (0.9, vec![field.into()], "标题包含匹配".into());
        }
    }
    (
        0.55,
        vec!["search".into()],
        "VNDB 搜索结果，需人工确认".into(),
    )
}

fn cache_response(
    db: &mut Database,
    query: &str,
    response: &Value,
    fetched_at: &str,
) -> Result<()> {
    let response = serde_json::to_string(response)
        .map_err(|_| ServiceError(ErrorCode::InvalidResponse, "VNDB 返回结果无法缓存。"))?;
    db.save_metadata_cache("vndb", query, Some(&response), "success", None, fetched_at)
}

pub fn search(
    backend: &Backend,
    request: &SearchMetadataRequest,
) -> Result<Vec<MetadataCandidate>> {
    let query = request.query.trim();
    if query.is_empty() || query.chars().count() > 200 {
        return Err(invalid("资料搜索关键词需要 1～200 个字符。"));
    }
    if request.providers.iter().any(|provider| provider != "vndb") {
        return Err(invalid("VNDB 搜索只接受 VNDB 资料源。"));
    }
    let fetched_at = now();
    let (response, cached) = {
        let db = backend.database()?;
        if request.cache {
            if let Some((response, _)) = db.metadata_cache_response("vndb", query)? {
                (
                    serde_json::from_str::<Value>(&response).map_err(|_| {
                        ServiceError(ErrorCode::InvalidResponse, "本地 VNDB 缓存已损坏。")
                    })?,
                    true,
                )
            } else {
                drop(db);
                let response = request_json(
                    backend,
                    json!({
                        "filters": ["search", "=", query],
                        "fields": "id,title,alttitle,titles{lang,title},image.url,developers{name},released,rating,tags{name},description",
                        "sort": "searchrank",
                        "results": 20
                    }),
                )?;
                let mut db = backend.database()?;
                cache_response(&mut db, query, &response, &fetched_at)?;
                (response, false)
            }
        } else {
            drop(db);
            let response = request_json(
                backend,
                json!({
                    "filters": ["search", "=", query],
                    "fields": "id,title,alttitle,titles{lang,title},image.url,developers{name},released,rating,tags{name},description",
                    "sort": "searchrank",
                    "results": 20
                }),
            )?;
            (response, false)
        }
    };
    entries_from_response(&response)?
        .into_iter()
        .map(|entry| {
            let (confidence, matched_fields, explanation) = confidence(query, &entry);
            Ok(MetadataCandidate {
                provider: "vndb".into(),
                remote_id: entry.id,
                title: entry
                    .title_zh
                    .clone()
                    .or_else(|| entry.title_en.clone())
                    .unwrap_or(entry.title),
                subtitle: entry.alt_title,
                cover_url: entry.cover_url,
                has_chinese_description: entry.description.as_deref().and_then(|description| {
                    super::metadata_text::has_chinese_description(Some(&clean_vndb_description(
                        description,
                    )))
                }),
                confidence,
                matched_fields,
                explanation,
                fetched_at: fetched_at.clone(),
                cached,
            })
        })
        .collect()
}

pub fn confirm(backend: &Backend, request: &ConfirmMetadataMatchRequest) -> Result<MatchResult> {
    backend
        .database()?
        .ensure_metadata_unlocked(&request.game_id)?;
    if request.provider != "vndb" || !request.remote_id.starts_with('v') {
        return Err(invalid("资料匹配来源或远程标识无效。"));
    }
    let fetched_at = now();
    let response = {
        let db = backend.database()?;
        db.latest_vndb_cache_for_remote_id(&request.remote_id)?
            .map(|(response, _)| response)
    };
    let (entry, response_json) = if let Some(response) = response {
        let value = serde_json::from_str::<Value>(&response)
            .map_err(|_| ServiceError(ErrorCode::InvalidResponse, "本地 VNDB 缓存已损坏。"))?;
        let entry = entries_from_response(&value)?
            .into_iter()
            .find(|entry| entry.id == request.remote_id)
            .ok_or_else(missing)?;
        if entry.cover_url.is_some() {
            (entry, None)
        } else {
            // Older cached VNDB responses did not request image.url. Refresh
            // those records once so importing an existing library can still
            // populate its local cover without waiting for cache expiry.
            match request_json(
                backend,
                json!({
                    "filters": ["id", "=", request.remote_id],
                    "fields": "id,title,alttitle,titles{lang,title},image.url,developers{name},released,rating,tags{name},description",
                    "results": 1
                }),
            ) {
                Ok(value) => {
                    let entry = entries_from_response(&value)?
                        .into_iter()
                        .find(|entry| entry.id == request.remote_id)
                        .ok_or_else(missing)?;
                    let response_json = serde_json::to_string(&value).map_err(|_| {
                        ServiceError(ErrorCode::InvalidResponse, "VNDB 返回结果无法缓存。")
                    })?;
                    (entry, Some(response_json))
                }
                // Keep an older metadata record usable when the network is
                // temporarily unavailable. Cover sync will simply be retried
                // from the detail page once the source is reachable again.
                Err(_) => (entry, None),
            }
        }
    } else {
        let value = request_json(
            backend,
            json!({
                "filters": ["id", "=", request.remote_id],
                "fields": "id,title,alttitle,titles{lang,title},image.url,developers{name},released,rating,tags{name},description",
                "results": 1
            }),
        )?;
        let entry = entries_from_response(&value)?
            .into_iter()
            .find(|entry| entry.id == request.remote_id)
            .ok_or_else(missing)?;
        let response_json = serde_json::to_string(&value)
            .map_err(|_| ServiceError(ErrorCode::InvalidResponse, "VNDB 返回结果无法缓存。"))?;
        (entry, Some(response_json))
    };
    if let Some(response_json) = response_json {
        let mut db = backend.database()?;
        db.save_metadata_cache(
            "vndb",
            &format!("id:{}", request.remote_id),
            Some(&response_json),
            "success",
            None,
            &fetched_at,
        )?;
    }
    let localized = entry.description.as_deref().map(localized_description);
    let description = localized.as_ref().map(|value| value.0.as_str());
    let description_zh = localized.as_ref().and_then(|value| value.1.as_deref());
    // Publisher enrichment is optional: an unavailable release endpoint must
    // not discard a valid title match or overwrite a hand-edited publisher.
    let publisher = publisher_for(backend, &entry.id).ok().flatten();
    let mut db = backend.database()?;
    let result = db.apply_vndb_match_with_details(
        &request.game_id,
        &entry.id,
        &entry.title,
        entry.title_zh.as_deref(),
        entry.title_en.as_deref(),
        entry.alt_title.as_deref(),
        entry.developer.as_deref(),
        publisher.as_deref(),
        entry.release_date.as_deref(),
        entry.rating,
        &entry.tags,
        description,
        description_zh,
        &fetched_at,
    )?;
    db.delete_setting(&format!("metadata.candidates.{}", request.game_id))?;
    drop(db);
    if !cfg!(test) && super::metadata_sources::needs_cover(backend, &request.game_id, "vndb")? {
        if let Some(url) = entry.cover_url.as_deref() {
            let path = crate::backend::bangumi::cache_image_for(backend, "vndb", url)?;
            backend.database()?.apply_remote_fields(
                &request.game_id,
                "vndb",
                &entry.id,
                &[("cover_path".into(), path)],
                &[],
                &fetched_at,
                false,
            )?;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishers_only_include_explicit_publishers_of_official_complete_releases() {
        let release = json!({
            "official": true, "patch": false,
            "vns": [{"id": "v17", "rtype": "complete"}],
            "producers": [
                {"name": "Developer Studio", "publisher": false},
                {"name": " Publisher A ", "publisher": true},
                {"name": "Publisher B", "publisher": true},
                {"name": "", "original": "日本語の発売元", "publisher": true},
                {"name": "Unspecified Role"}
            ]
        });
        let mut unofficial = release.clone();
        unofficial["official"] = json!(false);
        unofficial["producers"] = json!([{"name": "Fan Group", "publisher": true}]);
        let mut patch = unofficial.clone();
        patch["official"] = json!(true);
        patch["patch"] = json!(true);
        let mut partial_bundle = unofficial.clone();
        partial_bundle["official"] = json!(true);
        partial_bundle["vns"] = json!([
            {"id": "v17", "rtype": "partial"},
            {"id": "v18", "rtype": "complete"}
        ]);
        let mut unrelated = unofficial.clone();
        unrelated["official"] = json!(true);
        unrelated["vns"] = json!([{"id": "v18", "rtype": "complete"}]);
        let value = json!({"results": [
            release.clone(), release, unofficial, patch, partial_bundle, unrelated
        ]});
        assert_eq!(
            publishers_from_releases(&value, "v17").unwrap().as_deref(),
            Some("Publisher A、Publisher B、日本語の発売元")
        );
        assert!(publishers_from_releases(&json!({"results": []}), "v17")
            .unwrap()
            .is_none());
        assert!(publishers_from_releases(&json!({}), "v17").is_err());
    }

    #[test]
    fn publisher_enrichment_works_with_legacy_vn_cache_and_reaches_game_summary() {
        let root = std::env::temp_dir().join(format!("gm-publisher-{}", id()));
        let backend = Backend::open(root.clone()).unwrap();
        let game = {
            let mut db = backend.database().unwrap();
            let game = db
                .import_installation(
                    "/fixture/publisher",
                    "旧缓存作品",
                    None,
                    crate::domain::protocol::InstallSource::Local,
                    &[],
                    "fixture",
                )
                .unwrap();
            let response = json!({"results": [{
                "id": "v17", "title": "Cached Title",
                "developers": [{"name": "Developer Studio"}],
                "image": {"url": "https://t.vndb.org/cv/00/000017.jpg"}
            }]})
            .to_string();
            db.save_metadata_cache(
                "vndb",
                "legacy-title",
                Some(&response),
                "success",
                None,
                &now(),
            )
            .unwrap();
            let publisher_cache = json!({"publisher": "Publisher A、Publisher B"}).to_string();
            db.save_metadata_cache(
                "vndb",
                "publishers:v17",
                Some(&publisher_cache),
                "success",
                None,
                &now(),
            )
            .unwrap();
            // The extra cache must never be read as a VN search response.
            assert_eq!(
                db.latest_vndb_cache_for_remote_id("v17")
                    .unwrap()
                    .unwrap()
                    .0,
                response
            );
            game
        };
        confirm(
            &backend,
            &ConfirmMetadataMatchRequest {
                manual: false,
                title_hint: None,
                game_id: game.clone(),
                provider: "vndb".into(),
                remote_id: "v17".into(),
            },
        )
        .unwrap();
        let detail = backend.database().unwrap().get_game(&game).unwrap();
        assert_eq!(
            detail.summary.publisher.as_deref(),
            Some("Publisher A、Publisher B")
        );
        assert_eq!(
            detail.summary.developer.as_deref(),
            Some("Developer Studio")
        );
        assert!(detail.metadata.iter().any(|field| {
            field.field == "publisher" && field.provider == "vndb" && !field.manually_edited
        }));
        drop(backend);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parses_and_scores_vndb_fixture() {
        let value = serde_json::json!({
            "id": "v17",
            "title": "Kanon",
            "alttitle": "カノン",
            "titles": [{"lang": "zh-Hans", "title": "Kanon 中文名"}],
            "developers": [{"name": "Key"}],
            "released": "1999-06-04",
            "rating": 80.8,
            "tags": [{"name": "校园"}, {"name": "恋爱"}],
            "description": "fixture",
            "image": {"url": "https://t.vndb.org/cv/00/000017.jpg"}
        });
        let entry = parse_entry(&value).unwrap();
        assert_eq!(entry.id, "v17");
        assert_eq!(confidence("kanon", &entry).0, 1.0);
        assert_eq!(confidence("カノン", &entry).0, 1.0);
        assert_eq!(entry.developer.as_deref(), Some("Key"));
        assert_eq!(entry.title_zh.as_deref(), Some("Kanon 中文名"));
        assert!(entry
            .rating
            .is_some_and(|rating| (rating - 8.08).abs() < 0.001));
        assert_eq!(entry.tags, vec!["校园", "恋爱"]);
        assert_eq!(
            entry.cover_url.as_deref(),
            Some("https://t.vndb.org/cv/00/000017.jpg")
        );
    }

    #[test]
    fn localized_titles_prefer_actual_english_and_japanese_and_skip_blank_chinese() {
        let entry = parse_entry(&json!({
            "id":"v17", "title":"Romanized Title", "alttitle":"Other Alternative",
            "titles":[
                {"lang":"zh-Hans","title":"  "},
                {"lang":"zh-Hant","title":" 中文作品名 "},
                {"lang":"en","title":" English Title "},
                {"lang":"ja","title":" 日本語の作品名 "}
            ]
        }))
        .unwrap();
        assert_eq!(entry.title_zh.as_deref(), Some("中文作品名"));
        assert_eq!(entry.title_en.as_deref(), Some("English Title"));
        assert_eq!(entry.alt_title.as_deref(), Some("日本語の作品名"));
    }

    #[test]
    fn scraped_english_tags_use_chinese_display_labels() {
        assert_eq!(
            crate::backend::localized_source_tag("High School Student Protagonist"),
            Some("高中生主角".into())
        );
        assert_eq!(
            crate::backend::localized_source_tag("Musical Environment"),
            Some("音乐环境".into())
        );
        assert_eq!(
            crate::backend::localized_source_tag("校园"),
            Some("校园".into())
        );
        assert_eq!(
            crate::backend::localized_source_tag("Unlisted Provider Tag"),
            None
        );
    }

    #[test]
    fn automatic_query_removes_release_tags_but_keeps_title_parentheses() {
        assert_eq!(automatic_query("Kanon [简体中文]"), "Kanon");
        assert_eq!(automatic_query("CLANNAD (クラナド)"), "CLANNAD (クラナド)");
    }

    #[test]
    fn cleans_vndb_markup_and_keeps_chinese_description() {
        let (description, translated) =
            localized_description("[url=/c123]角色名称[/url]\n\n[b]故事简介[/b]");
        assert_eq!(description, "角色名称\n\n故事简介");
        assert_eq!(translated.as_deref(), Some("角色名称\n\n故事简介"));
    }
}
