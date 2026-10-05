use super::*;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::time::Duration;

const SEARCH_API: &str = "https://api.bgm.tv/v0/search/subjects?limit=10&offset=0";
const USER_AGENT: &str = "GalgameManager/0.1 (Bangumi metadata; local desktop app)";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Subject {
    id: String,
    name: String,
    name_cn: Option<String>,
    cover_url: Option<String>,
    has_chinese_description: Option<bool>,
}

pub(super) fn client() -> Result<Client> {
    super::network::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| {
            ServiceError(
                ErrorCode::NetworkUnavailable,
                "无法初始化 Bangumi 网络请求。",
            )
        })
}

pub(super) fn request_json(response: reqwest::blocking::Response) -> Result<Value> {
    if !response.status().is_success() {
        if matches!(response.status().as_u16(), 401 | 403) {
            return Err(ServiceError(
                ErrorCode::PermissionDenied,
                "Bangumi 授权已过期或权限不足，请重新登录。",
            ));
        }
        return Err(ServiceError(
            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                ErrorCode::RateLimited
            } else {
                ErrorCode::NetworkUnavailable
            },
            "Bangumi 暂时无法访问，请稍后再试。",
        ));
    }
    response
        .json::<Value>()
        .map_err(|_| ServiceError(ErrorCode::InvalidResponse, "Bangumi 返回了无法解析的资料。"))
}

pub(super) fn request_json_with_retry<F>(build: F) -> Result<Value>
where
    F: FnMut() -> reqwest::blocking::RequestBuilder,
{
    request_json(super::http_retry::send(build)?)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn subject_titles(subject: &Subject) -> Vec<(String, String)> {
    let mut fields = vec![("title".into(), subject.name.clone())];
    if let Some(name_cn) = &subject.name_cn {
        fields.push(("title_zh".into(), name_cn.clone()));
    }
    fields
}

#[cfg(test)]
fn sync_subject_titles(backend: &Backend, game_id: &str, subject: &Subject) -> Result<()> {
    backend.database()?.apply_remote_fields(
        game_id,
        "bangumi",
        &subject.id,
        &subject_titles(subject),
        &[(subject.name.clone(), "und".into())],
        &now(),
        false,
    )?;
    Ok(())
}

fn infobox_value(value: &Value, keys: &[&str]) -> Option<String> {
    let infobox = value.get("infobox").and_then(Value::as_array)?;
    // Key priority must be independent of the source's infobox ordering.
    keys.iter().find_map(|key| {
        let mut names = Vec::new();
        for item in infobox
            .iter()
            .filter(|item| string_field(item, "key").as_deref() == Some(*key))
        {
            let Some(value) = item.get("value") else {
                continue;
            };
            let values = value
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_else(|| std::slice::from_ref(value));
            for value in values {
                // Bangumi list entries are commonly {"k": "region", "v": "name"}.
                let name = value
                    .as_str()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .or_else(|| string_field(value, "v"));
                if let Some(name) = name.filter(|name| !names.contains(name)) {
                    names.push(name);
                }
            }
        }
        (!names.is_empty()).then(|| names.join("、"))
    })
}

fn subject_publisher(value: &Value) -> Option<String> {
    infobox_value(
        value,
        &[
            "发行商",
            "发行",
            "发行公司",
            "发行方",
            "出版社",
            "出版商",
            "出版",
            "発売元",
            "販売元",
        ],
    )
}

fn subject_details(value: &Value) -> Vec<(String, String)> {
    let publisher = subject_publisher(value);
    let rating = value
        .get("rating")
        .and_then(|rating| rating.get("score"))
        .and_then(Value::as_f64)
        .filter(|rating| rating.is_finite() && (0.0..=10.0).contains(rating))
        .map(|rating| format!("{rating:.2}"));
    let tags = value
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| string_field(tag, "name"))
                .take(24)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|tags| !tags.is_empty());
    let mut fields: Vec<_> = [
        ("publisher", publisher),
        ("developer", infobox_value(value, &["开发商", "制作公司"])),
        ("release_date", string_field(value, "date")),
        ("source_rating", rating),
        ("source_tags", tags),
    ]
    .into_iter()
    .filter_map(|(field, value)| value.map(|value| (field.to_owned(), value)))
    .collect();
    fields.extend(super::metadata_text::description_fields(string_field(
        value, "summary",
    )));
    fields
}

fn cover_url(value: &Value) -> Option<String> {
    let images = value.get("images")?;
    ["large", "common", "medium", "small", "grid"]
        .iter()
        .filter_map(|key| string_field(images, key))
        .find(|url| trusted_image(url) && url.contains("/pic/cover/"))
}

pub(super) fn trusted_image(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("lain.bgm.tv")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port_or_known_default() == Some(443)
    })
}

pub(super) fn trusted_vndb_image(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https"
            && matches!(url.host_str(), Some("t.vndb.org") | Some("s.vndb.org"))
            && url.username().is_empty()
            && url.password().is_none()
            && url.port_or_known_default() == Some(443)
    })
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CoverStatus {
    pub status: String,
    pub message: String,
    pub updated_at: Option<String>,
    pub remote_id: Option<String>,
}
#[derive(Deserialize)]
pub struct RetryRequest {
    pub game_id: String,
    pub query: Option<String>,
}
fn save_status(
    backend: &Backend,
    game_id: &str,
    status: &str,
    message: &str,
    remote_id: Option<String>,
) -> Result<CoverStatus> {
    let value = CoverStatus {
        status: status.to_owned(),
        message: message.to_owned(),
        updated_at: Some(now()),
        remote_id,
    };
    backend
        .database()?
        .put_setting(&format!("bangumi.cover.{game_id}"), &value)?;
    Ok(value)
}
pub fn cover_status(backend: &Backend, game_id: &str) -> Result<CoverStatus> {
    let db = backend.database()?;
    let game = db.get_game(game_id)?;
    if let Some(mut value) = db.setting::<CoverStatus>(&format!("bangumi.cover.{game_id}"))? {
        if value.status == "running"
            && value
                .updated_at
                .as_deref()
                .and_then(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
                .is_none_or(|time| {
                    (chrono::Utc::now() - time.with_timezone(&chrono::Utc)).num_seconds() > 120
                })
        {
            value.status = "incomplete".to_owned();
            value.message = "封面刮削尚未完成；若上次中断可点击重试。".to_owned();
        }
        if value.status == "success"
            && game
                .summary
                .cover_url
                .as_deref()
                .is_none_or(|path| !backend.data_directory.join(path).is_file())
        {
            value.status = "failed".to_owned();
            value.message = "本地封面缓存丢失，请重新刮削。".to_owned();
        }
        return Ok(value);
    }
    Ok(CoverStatus {
        status: "idle".to_owned(),
        message: if game.summary.cover_url.is_some() {
            "已有封面；可重新刮削并缓存图片。"
        } else {
            "尚未获得封面，请刮削 Bangumi。"
        }
        .to_owned(),
        updated_at: None,
        remote_id: None,
    })
}
fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("webp")
    } else {
        None
    }
}
fn cache_image(backend: &Backend, url: &str) -> Result<String> {
    cache_image_for(backend, "bangumi", url)
}

pub(crate) fn cache_image_for(backend: &Backend, provider: &str, url: &str) -> Result<String> {
    let trusted = match provider {
        "bangumi" => trusted_image(url),
        "vndb" => trusted_vndb_image(url),
        "hikarifield" => super::hikarifield::trusted_image(url),
        "hikarinagi" => super::hikarinagi::trusted_image(url),
        _ => false,
    };
    if !trusted {
        return Err(invalid("封面图片地址不受信任。"));
    }
    let preferences = super::app_settings::get(backend)?;
    let use_mirror = match provider {
        "bangumi" => preferences.bangumi_cover_source == "hikarinagi",
        "vndb" => preferences.vndb_cover_source == "hikarinagi",
        _ => false,
    };
    let resolved = if use_mirror {
        format!("https://imagesp.yurari.moe/{provider}/{url}")
    } else {
        url.to_owned()
    };
    let response = client()?.get(&resolved).send().map_err(|_| {
        ServiceError(
            ErrorCode::NetworkUnavailable,
            "封面下载失败，请检查网络后重试。",
        )
    })?;
    if !response.status().is_success() {
        return Err(ServiceError(
            ErrorCode::NetworkUnavailable,
            "封面服务器返回错误，请重试。",
        ));
    }
    let final_url = response.url().to_string();
    let final_trusted = if use_mirror {
        final_url == resolved
    } else {
        match provider {
            "bangumi" => trusted_image(&final_url),
            "vndb" => trusted_vndb_image(&final_url),
            "hikarifield" => super::hikarifield::trusted_image(&final_url),
            "hikarinagi" => super::hikarinagi::trusted_image(&final_url),
            _ => false,
        }
    };
    if !final_trusted {
        return Err(invalid("封面重定向地址不受信任。"));
    }
    let mut bytes = Vec::new();
    let mut stream = response.take(8 * 1024 * 1024 + 1);
    let mut chunk = [0u8; 64 * 1024];
    loop {
        if let Some(id) = &backend.import_batch {
            super::import_metadata::batch_config(backend, id)?;
        }
        let count = stream
            .read(&mut chunk)
            .map_err(|_| ServiceError(ErrorCode::NetworkUnavailable, "封面下载中断，请重试。"))?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    persist_image(backend, &bytes)
}
fn persist_image(backend: &Backend, bytes: &[u8]) -> Result<String> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(ServiceError(
            ErrorCode::InvalidResponse,
            "封面图片超过 8 MB 限制。",
        ));
    }
    let extension = image_extension(bytes).ok_or(ServiceError(
        ErrorCode::InvalidResponse,
        "封面服务器未返回有效图片。",
    ))?;
    let directory = backend.data_directory.join("covers");
    std::fs::create_dir_all(&directory)
        .map_err(|_| ServiceError(ErrorCode::PermissionDenied, "无法创建封面缓存目录。"))?;
    let filename = format!("{:x}.{extension}", Sha256::digest(bytes));
    let path = directory.join(&filename);
    super::import_metadata::persist_cover(backend, &path, bytes, &format!("covers/{filename}"))?;
    Ok(format!("covers/{filename}"))
}

fn parse_subject(value: &Value) -> Option<Subject> {
    if value.get("type").and_then(Value::as_i64) != Some(4) {
        return None;
    }
    let id = value.get("id")?.as_u64()?.to_string();
    let name = string_field(value, "name")?;
    Some(Subject {
        id,
        name,
        name_cn: string_field(value, "name_cn").filter(|name| !name.trim().is_empty()),
        cover_url: cover_url(value),
        has_chinese_description: super::metadata_text::has_chinese_description(
            string_field(value, "summary").as_deref(),
        ),
    })
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '_' | ':' | '：'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn score(query: &str, subject: &Subject) -> f64 {
    let query = normalize(query);
    let names = [Some(subject.name.as_str()), subject.name_cn.as_deref()];
    let mut best: f64 = 0.0;
    for name in names.into_iter().flatten() {
        let name = normalize(name);
        if name == query {
            best = best.max(1.0);
        } else if !query.is_empty() && (name.contains(&query) || query.contains(&name)) {
            best = best.max(0.86);
        }
    }
    best
}

#[cfg(test)]
fn select_subject(query: &str, value: &Value) -> Result<Option<Subject>> {
    let subjects = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "Bangumi 搜索结果结构无效。",
        ))?;
    let mut exact = subjects
        .iter()
        .filter_map(parse_subject)
        .filter(|subject| score(query, subject) == 1.0);
    let first = exact.next();
    if exact.next().is_some() {
        return Ok(None);
    }
    Ok(first.filter(|subject| subject.cover_url.is_some()))
}

pub fn search_metadata(
    backend: &Backend,
    request: &crate::domain::requests::SearchMetadataRequest,
) -> Result<Vec<crate::domain::models::MetadataCandidate>> {
    search_metadata_with(backend, request, |query| {
        if cfg!(test) {
            return Ok(None);
        }
        let http = client()?;
        request_json(super::http_retry::send_checked(
            || {
                let mut request = http
                    .clone()
                    .post(SEARCH_API)
                    .json(&json!({"keyword": query, "filter": {"type": [4]}, "sort": "match"}));
                if let Some(token) = super::bangumi_account::token(backend).ok().flatten() {
                    request = request.bearer_auth(token);
                }
                request
            },
            || backend.ensure_search_active(),
        )?)
        .map(Some)
    })
}

fn search_metadata_with(
    backend: &Backend,
    request: &crate::domain::requests::SearchMetadataRequest,
    fetch: impl FnOnce(&str) -> Result<Option<Value>>,
) -> Result<Vec<crate::domain::models::MetadataCandidate>> {
    let query = request.query.trim();
    if query.is_empty() || query.chars().count() > 200 {
        return Err(invalid("资料搜索关键词需要 1～200 个字符。"));
    }
    let provider = "bangumi";
    let fetched_at = now();
    // In Rust 2021 an if-let temporary lives through the else branch. Read in
    // a separate statement so a cache miss cannot retain the database mutex
    // across a network request or the subsequent cache write.
    let cached = if request.cache {
        backend
            .database()?
            .metadata_cache_response(provider, query)?
    } else {
        None
    };
    let value = if let Some((response, _)) = cached {
        serde_json::from_str::<Value>(&response)
            .map_err(|_| ServiceError(ErrorCode::InvalidResponse, "本地 Bangumi 缓存已损坏。"))?
    } else {
        let Some(value) = fetch(query)? else {
            return Ok(Vec::new());
        };
        if request.cache {
            backend.database()?.save_metadata_cache(
                provider,
                query,
                Some(&value.to_string()),
                "success",
                None,
                &fetched_at,
            )?;
        }
        value
    };
    let subjects = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "Bangumi 搜索结果结构无效。",
        ))?;
    Ok(subjects
        .iter()
        .filter_map(parse_subject)
        .take(20)
        .map(|subject| {
            let confidence = score(query, &subject);
            let title = subject
                .name_cn
                .clone()
                .unwrap_or_else(|| subject.name.clone());
            crate::domain::models::MetadataCandidate {
                provider: provider.into(),
                remote_id: subject.id,
                title,
                subtitle: Some(subject.name),
                cover_url: subject.cover_url,
                has_chinese_description: subject.has_chinese_description,
                confidence,
                matched_fields: vec!["name".into(), "name_cn".into()],
                explanation: if confidence >= 1.0 {
                    "中文名或原名完全匹配".into()
                } else {
                    "Bangumi 搜索结果，需人工确认".into()
                },
                fetched_at: fetched_at.clone(),
                cached: true,
            }
        })
        .collect())
}
pub fn confirm_metadata(
    backend: &Backend,
    request: &crate::domain::requests::ConfirmMetadataMatchRequest,
) -> Result<crate::domain::models::MatchResult> {
    backend
        .database()?
        .ensure_metadata_unlocked(&request.game_id)?;
    if request.provider != "bangumi" || !request.remote_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalid("Bangumi 远程标识无效。"));
    }
    let game_id = request.game_id.clone();
    backend.database()?.get_game(&game_id)?;
    if cfg!(test) {
        return Err(ServiceError(
            ErrorCode::NetworkUnavailable,
            "测试环境不调用 Bangumi 网络。",
        ));
    }
    let http = client()?;
    let value = request_json_with_retry(|| {
        let mut request = http.clone().get(format!(
            "https://api.bgm.tv/v0/subjects/{}",
            request.remote_id
        ));
        if let Some(token) = super::bangumi_account::token(backend).ok().flatten() {
            request = request.bearer_auth(token);
        }
        request
    })?;
    let subject = parse_subject(&value).ok_or_else(missing)?;
    let fetched_at = now();
    let mut fields = subject_titles(&subject);
    fields.extend(subject_details(&value));
    if let Some(url) = &subject.cover_url {
        fields.push(("cover_url".into(), url.clone()));
    }
    // Bind publishing facts from this exact subject without depending on a
    // later title search or a successful cover download.
    if !fields.iter().any(|(field, _)| field == "publisher") {
        fields.extend(subject_publisher(&value).map(|publisher| ("publisher".into(), publisher)));
    }
    let result = backend.database()?.apply_remote_fields(
        &game_id,
        "bangumi",
        &request.remote_id,
        &fields,
        &[],
        &fetched_at,
        true,
    )?;
    if !super::metadata_sources::needs_cover(backend, &game_id, "bangumi")? {
        return Ok(result);
    }
    // Cache the cover for the subject explicitly confirmed by the user. A
    // second title search could select a different subject or fail needlessly.
    let _guard = backend
        .cover_lock
        .lock()
        .map_err(|_| ServiceError(ErrorCode::InternalError, "封面服务需要重新启动。"))?;
    save_status(
        backend,
        &game_id,
        "running",
        "正在下载 Bangumi 封面…",
        Some(subject.id.clone()),
    )?;
    let cover = subject
        .cover_url
        .as_deref()
        .ok_or(ServiceError(
            ErrorCode::InvalidResponse,
            "资料已匹配，但没有可用封面；不计入刮削成功。",
        ))
        .and_then(|url| {
            let cached = cache_image(backend, url)?;
            backend.database()?.apply_cached_bangumi_cover(
                &game_id,
                &subject.id,
                url,
                &cached,
                &now(),
            )?;
            Ok(())
        });
    if let Err(error) = cover {
        save_status(backend, &game_id, "failed", error.1, Some(subject.id))?;
        return Err(error);
    }
    save_status(
        backend,
        &game_id,
        "success",
        "封面已同步并保存到本地缓存。",
        Some(subject.id),
    )?;
    Ok(result)
}

pub fn retry(backend: &Backend, request: RetryRequest) -> Result<CoverStatus> {
    backend
        .database()?
        .ensure_metadata_unlocked(&request.game_id)?;
    let game = backend.database()?.get_game(&request.game_id)?;
    let query = request
        .query
        .as_deref()
        .filter(|q| !q.trim().is_empty())
        .unwrap_or(&game.summary.title);
    if query.chars().count() > 200 {
        return Err(invalid("搜索关键词过长。"));
    }
    let mut stage = backend.clone();
    stage.metadata_snapshot = Some(super::metadata_sources::get(backend)?);
    save_status(
        &stage,
        &request.game_id,
        "running",
        "正在按元数据设置匹配资料与封面…",
        None,
    )?;
    let mut failure = None;
    for provider in super::metadata_sources::get(&stage)?.enabled() {
        let found = super::metadata_sources::search(
            &stage,
            &crate::domain::requests::SearchMetadataRequest {
                manual: false,
                batch_id: None,
                query: query.into(),
                providers: vec![provider],
                cache: false,
            },
        );
        let mut candidates = match found {
            Ok(items) => items,
            Err(e) => {
                failure = Some(e);
                continue;
            }
        };
        candidates.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        let Some(best) = candidates.first().filter(|c| {
            c.confidence >= 0.95
                && candidates
                    .get(1)
                    .is_none_or(|next| c.confidence - next.confidence >= 0.05)
        }) else {
            continue;
        };
        match super::metadata_sources::confirm(
            &stage,
            &crate::domain::requests::ConfirmMetadataMatchRequest {
                manual: false,
                title_hint: Some(best.title.clone()),
                game_id: request.game_id.clone(),
                provider: best.provider.clone(),
                remote_id: best.remote_id.clone(),
            },
        ) {
            Ok(result) => {
                let cover = stage
                    .database()?
                    .get_game(&request.game_id)?
                    .summary
                    .cover_url;
                if cover
                    .as_deref()
                    .is_some_and(|p| super::import_metadata::validate_cover(&stage, p).is_ok())
                {
                    save_status(
                        &stage,
                        &request.game_id,
                        "success",
                        "资料与封面已按来源设置同步。",
                        Some(result.remote_id),
                    )?;
                    return cover_status(&stage, &request.game_id);
                }
            }
            Err(e) => failure = Some(e),
        }
    }
    if let Some(e) = failure {
        save_status(&stage, &request.game_id, "failed", e.1, None)?;
    } else {
        save_status(
            &stage,
            &request.game_id,
            "no_match",
            "未找到唯一的同作品匹配，请手动选择资料。",
            None,
        )?;
    }
    cover_status(&stage, &request.game_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaving_a_slow_search_keeps_local_pages_usable_and_discards_late_cache() {
        use std::{sync::mpsc, thread};
        let root = std::env::temp_dir().join(format!("gm-slow-search-{}", id()));
        let backend = Backend::open(root.clone()).unwrap();
        let game_id = backend
            .database()
            .unwrap()
            .import_installation(
                "/fixture/slow-search",
                "离页作品",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        let scope = super::super::metadata_search::begin(&backend, "slow-search").unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            search_metadata_with(
                &scope.backend,
                &crate::domain::requests::SearchMetadataRequest {
                    manual: true,
                    batch_id: None,
                    query: "未缓存作品".into(),
                    providers: vec!["bangumi".into()],
                    cache: true,
                },
                |_| {
                    started_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
                    Ok(Some(
                        json!({"data": [{"id":123,"type":4,"name":"迟到作品"}]}),
                    ))
                },
            )
        });
        started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(
            backend.db.try_lock().is_ok(),
            "network search retained the database mutex"
        );
        assert_eq!(
            backend
                .database()
                .unwrap()
                .get_game(&game_id)
                .unwrap()
                .summary
                .title,
            "离页作品"
        );
        let query = super::super::activity::ActivityQuery {
            range: super::super::activity::ActivityRange::Week,
            calendar_year: None,
            session_date: None,
            page: 1,
            page_size: 20,
        };
        assert_eq!(
            backend
                .database()
                .unwrap()
                .activity_snapshot(&query)
                .unwrap()
                .summary
                .total_seconds,
            0
        );
        backend
            .database()
            .unwrap()
            .put_setting("search-regression", &true)
            .unwrap();
        super::super::metadata_search::cancel(
            &backend,
            super::super::metadata_search::CancelRequest {
                search_request_id: "slow-search".into(),
            },
        )
        .unwrap();
        release_tx.send(()).unwrap();
        assert_eq!(worker.join().unwrap().unwrap_err().0, ErrorCode::Cancelled);
        assert!(backend
            .database()
            .unwrap()
            .metadata_cache_response("bangumi", "未缓存作品")
            .unwrap()
            .is_none());
        assert_eq!(
            backend
                .database()
                .unwrap()
                .setting::<bool>("search-regression")
                .unwrap(),
            Some(true)
        );
        drop(backend);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn search_cache_miss_allows_group_and_source_saves_during_fetch() {
        let root = std::env::temp_dir().join(format!("gm-search-lock-{}", id()));
        let backend = Backend::open(root.clone()).unwrap();
        let request = crate::domain::requests::SearchMetadataRequest {
            manual: false,
            batch_id: None,
            query: "测试作品".into(),
            providers: vec!["bangumi".into()],
            cache: true,
        };
        let items = search_metadata_with(&backend, &request, |query| {
            // Assert before re-entering the database so a regression fails
            // immediately instead of hanging the test on a second lock.
            assert!(
                backend.db.try_lock().is_ok(),
                "search retained the database lock"
            );
            let saved = backend.database()?.save_collection(
                &crate::domain::models::SaveCollectionRequest {
                    id: None,
                    name: "搜索期间创建的分组".into(),
                    kind: crate::domain::models::CollectionKind::Normal,
                    icon: None,
                    color: None,
                    cover_url: None,
                    position: 0,
                    hidden: false,
                    query: None,
                    member_ids: Some(vec![]),
                },
            )?;
            assert_eq!(saved.summary.name, "搜索期间创建的分组");
            let mut config = super::super::metadata_sources::Config::default();
            config.sources[1].enabled = true;
            config.sources.swap(0, 1);
            config.sources[2].enabled = false;
            super::super::metadata_sources::save(&backend, config)?;
            Ok(Some(
                json!({"data": [{"id": 123, "type": 4, "name": query}]}),
            ))
        })
        .unwrap();
        assert_eq!(items.len(), 1);
        assert!(backend.db.try_lock().is_ok());
        let cached =
            search_metadata_with(&backend, &request, |_| panic!("cache hit fetched again"))
                .unwrap();
        assert_eq!(cached[0].remote_id, "123");
        assert_eq!(
            backend
                .database()
                .unwrap()
                .list_collections()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            super::super::metadata_sources::get(&backend)
                .unwrap()
                .enabled(),
            ["bangumi", "hikarinagi"]
        );
        drop(backend);
        let reopened = Backend::open(root.clone()).unwrap();
        assert_eq!(
            reopened
                .database()
                .unwrap()
                .list_collections()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            super::super::metadata_sources::get(&reopened)
                .unwrap()
                .enabled(),
            ["bangumi", "hikarinagi"]
        );
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_search_releases_database_and_does_not_cache_failure() {
        let root = std::env::temp_dir().join(format!("gm-search-error-{}", id()));
        let backend = Backend::open(root.clone()).unwrap();
        let request = crate::domain::requests::SearchMetadataRequest {
            manual: false,
            batch_id: None,
            query: "网络失败".into(),
            providers: vec!["bangumi".into()],
            cache: true,
        };
        let error = search_metadata_with(&backend, &request, |_| {
            assert!(backend.db.try_lock().is_ok());
            Err(ServiceError(
                ErrorCode::NetworkUnavailable,
                "测试网络失败。",
            ))
        })
        .unwrap_err();
        assert_eq!(error.0, ErrorCode::NetworkUnavailable);
        assert!(backend.db.try_lock().is_ok());
        assert!(backend
            .database()
            .unwrap()
            .metadata_cache_response("bangumi", &request.query)
            .unwrap()
            .is_none());
        let retried =
            search_metadata_with(&backend, &request, |_| Ok(Some(json!({"data": []})))).unwrap();
        assert!(retried.is_empty());
        assert!(backend
            .database()
            .unwrap()
            .metadata_cache_response("bangumi", &request.query)
            .unwrap()
            .is_some());
        drop(backend);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn uncached_search_ignores_existing_cache_and_never_replaces_it() {
        let root = std::env::temp_dir().join(format!("gm-search-fresh-{}", id()));
        let backend = Backend::open(root.clone()).unwrap();
        let response = json!({"data": [{"id": 123, "type": 4, "name": "旧结果"}]}).to_string();
        backend
            .database()
            .unwrap()
            .save_metadata_cache(
                "bangumi",
                "测试作品",
                Some(&response),
                "success",
                None,
                &now(),
            )
            .unwrap();
        let request = crate::domain::requests::SearchMetadataRequest {
            manual: false,
            batch_id: None,
            query: "测试作品".into(),
            providers: vec!["bangumi".into()],
            cache: false,
        };
        let items = search_metadata_with(&backend, &request, |_| {
            assert!(backend.db.try_lock().is_ok());
            Ok(Some(
                json!({"data": [{"id": 456, "type": 4, "name": "新结果"}]}),
            ))
        })
        .unwrap();
        assert_eq!(items[0].remote_id, "456");
        assert_eq!(
            backend
                .database()
                .unwrap()
                .metadata_cache_response("bangumi", &request.query)
                .unwrap()
                .unwrap()
                .0,
            response
        );
        drop(backend);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn publisher_infobox_handles_object_lists_and_prioritizes_publishing_keys() {
        let value = json!({"infobox": [
            {"key": "开发商", "value": "Developer Studio"},
            {"key": "发行", "value": "Fallback Publisher"},
            {"key": "发行商", "value": [
                {"k": "日本", "v": " Publisher A "},
                {"k": "中国", "v": "Publisher B"},
                {"k": "重复", "v": "Publisher A"},
                {"k": "空项", "v": "  "},
                {"k": "无名称"}, null
            ]}
        ]});
        assert_eq!(
            infobox_value(&value, &["发行商", "发行"]).as_deref(),
            Some("Publisher A、Publisher B")
        );
    }

    #[test]
    fn publisher_infobox_skips_empty_entries_and_accepts_string_lists() {
        let value = json!({"infobox": [
            {"key": "发行商", "value": []},
            {"key": "发行商", "value": "  "},
            {"key": "发行", "value": [" Publisher A ", "", "Publisher B", "Publisher A"]}
        ]});
        assert_eq!(
            infobox_value(&value, &["发行商", "发行"]).as_deref(),
            Some("Publisher A、Publisher B")
        );
        assert!(infobox_value(
            &json!({"infobox": [{"key": "开发商", "value": "Developer Studio"}]}),
            &["发行商", "发行"]
        )
        .is_none());
    }

    #[test]
    fn publisher_mapping_recognizes_bangumi_atri_release_key_without_using_developer() {
        // Bangumi subject 297264 uses 开发 / 发行, rather than 开发商 / 发行商.
        let value = json!({"infobox": [
            {"key": "开发", "value": "Front Wing × 枕"},
            {"key": "发行", "value": "ANIPLEX.EXE"}
        ]});
        assert_eq!(subject_publisher(&value).as_deref(), Some("ANIPLEX.EXE"));
        assert!(subject_publisher(&json!({"infobox": [
            {"key": "开发商", "value": "Developer Studio"}
        ]}))
        .is_none());
    }

    #[test]
    fn subject_details_save_chinese_summary_names_and_source_facts() {
        let summary = "女仆小春来到宿舍，柳诗音与柳花音和主人公共度夏天。";
        let fields = subject_details(&json!({
            "summary": summary,
            "date": "2026-01-01",
            "infobox": [{"key":"发行商","value":"发行商甲"}],
            "rating": {"score":8.3},
            "tags": [{"name":"Romance"}]
        }));
        assert!(fields.contains(&("description_zh".into(), summary.into())));
        assert!(fields.contains(&("description".into(), summary.into())));
        assert!(fields.contains(&("publisher".into(), "发行商甲".into())));
        assert!(fields.contains(&("release_date".into(), "2026-01-01".into())));
        assert!(fields.contains(&("source_rating".into(), "8.30".into())));
        assert!(fields.contains(&("source_tags".into(), "Romance".into())));
        let japanese = subject_details(&json!({"summary":"主人公は少女たちと暮らす。"}));
        assert_eq!(
            japanese,
            vec![("description".into(), "主人公は少女たちと暮らす。".into())]
        );
    }

    #[test]
    fn automatic_cover_match_syncs_subject_names_without_a_cover_download() {
        let root = std::env::temp_dir().join(format!("gm-titles-{}", id()));
        let backend = Backend::open(root.clone()).unwrap();
        let game = backend
            .database()
            .unwrap()
            .import_installation(
                "/fixture/Folder [Patch]",
                "Folder [Patch]",
                None,
                crate::domain::protocol::InstallSource::Local,
                &[],
                "fixture",
            )
            .unwrap();
        let subject = parse_subject(&json!({
            "id":123,"type":4,"name":"日本語の作品名","name_cn":"中文作品名"
        }))
        .unwrap();
        sync_subject_titles(&backend, &game, &subject).unwrap();
        let detail = backend.database().unwrap().get_game(&game).unwrap();
        assert_eq!(detail.summary.title, "日本語の作品名");
        assert_eq!(detail.summary.title_ja.as_deref(), Some("日本語の作品名"));
        assert!(detail.summary.cover_url.is_none());
        assert!(detail
            .metadata
            .iter()
            .any(|field| field.provider == "bangumi" && field.field == "title_zh"));
        drop(backend);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accepts_resized_official_cover_and_rejects_redirect_targets() {
        let subject = parse_subject(&json!({"id":1,"type":4,"name":"test","images":{"large":"https://lain.bgm.tv/r/800/pic/cover/l/test.jpg"}})).unwrap();
        assert!(subject.cover_url.is_some());
        for url in [
            "http://lain.bgm.tv/pic/cover/a.jpg",
            "https://lain.bgm.tv.evil.test/pic/cover/a.jpg",
            "https://user@lain.bgm.tv/pic/cover/a.jpg",
            "https://lain.bgm.tv:444/pic/cover/a.jpg",
        ] {
            assert!(!trusted_image(url));
        }
        assert!(trusted_vndb_image("https://t.vndb.org/cv/06/109906.jpg"));
        assert!(!trusted_vndb_image("https://example.invalid/cover.jpg"));
    }

    #[test]
    fn cached_image_is_portable_and_invalid_download_never_persists() {
        let root = std::env::temp_dir().join(format!("gm-cover-{}", id()));
        let backend = Backend::open(root.clone()).unwrap();
        assert!(persist_image(&backend, b"<html>error</html>").is_err());
        let png = b"\x89PNG\r\n\x1a\nfixture";
        let path = persist_image(&backend, png).unwrap();
        assert!(path.starts_with("covers/") && !Path::new(&path).is_absolute());
        assert_eq!(
            std::fs::read(backend.data_directory.join(&path)).unwrap(),
            png
        );
        assert_eq!(persist_image(&backend, png).unwrap(), path);
        assert_eq!(
            std::fs::read_dir(backend.data_directory.join("covers"))
                .unwrap()
                .count(),
            1
        );
        drop(backend);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parses_https_cover_and_prefers_exact_name() {
        let value = json!({
            "id": 123,
            "type": 4,
            "name": "CLANNAD",
            "name_cn": "CLANNAD",
            "images": {"large": "https://lain.bgm.tv/pic/cover/l/a.jpg"}
        });
        let subject = parse_subject(&value).unwrap();
        assert_eq!(subject.id, "123");
        assert_eq!(score("CLANNAD", &subject), 1.0);
        assert_eq!(
            subject.cover_url.as_deref(),
            Some("https://lain.bgm.tv/pic/cover/l/a.jpg")
        );
    }

    #[test]
    fn rejects_non_game_or_untrusted_cover_candidates() {
        let value = json!({
            "id": 123,
            "type": 4,
            "name": "CLANNAD",
            "images": {"large": "http://example.test/a.jpg"}
        });
        let subject = parse_subject(&value).unwrap();
        assert!(subject.cover_url.is_none());
        assert!(score("Other", &subject) < 0.86);
    }

    #[test]
    fn ambiguous_fuzzy_and_non_game_matches_never_bind() {
        let game = json!({"id": 123, "type": 4, "name": "CLANNAD", "images": {"large": "https://lain.bgm.tv/pic/cover/l/a.jpg"}});
        assert!(select_subject("CLANNAD", &json!({"data": [game.clone()]}))
            .unwrap()
            .is_some());
        assert!(select_subject(
            "CLANNAD",
            &json!({"data": [game.clone(), {"id": 124, "type": 4, "name": "CLANNAD"}]})
        )
        .unwrap()
        .is_none());
        assert!(select_subject(
            "CLANNAD",
            &json!({"data": [{"id": 124, "type": 4, "name": "CLANNAD Side Story"}]})
        )
        .unwrap()
        .is_none());
        assert!(parse_subject(&json!({"id": 125, "type": 2, "name": "CLANNAD"})).is_none());
    }
}
