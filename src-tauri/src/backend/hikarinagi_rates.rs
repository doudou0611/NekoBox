//! Public community statistics; no account cookies, credentials or user reviews.
use super::{now, Backend, Result, ServiceError};
use crate::domain::{models::ExternalSource, ErrorCode};
use reqwest::{blocking::Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io::Read, time::Duration};

const WEB_BASE: &str = "https://www.hikarinagi.org";

#[derive(Debug, Deserialize)]
pub struct Request {
    pub game_id: String,
    #[serde(default)]
    pub refresh: bool,
}
#[derive(Debug, Serialize)]
pub struct Response {
    pub wall: Option<Wall>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScoreCount {
    pub score: u8,
    pub count: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Keyword {
    pub word: String,
    pub count: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusCounts {
    pub completed: u64,
    pub going: u64,
    pub on_hold: u64,
    pub dropped: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Wall {
    pub remote_id: String,
    pub source_url: String,
    pub average: Option<f64>,
    pub rated_count: u64,
    pub distribution: Vec<ScoreCount>,
    pub status_counts: StatusCounts,
    pub keywords: Vec<Keyword>,
    pub fetched_at: String,
    pub cached: bool,
    pub stale: bool,
    pub message: Option<String>,
}

fn invalid_response() -> ServiceError {
    ServiceError(
        ErrorCode::InvalidResponse,
        "Hikarinagi 安利墙返回格式无效，请稍后重试。",
    )
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 20
        && id.bytes().all(|byte| byte.is_ascii_digit())
        && id.bytes().any(|byte| byte != b'0')
}
pub(super) fn bound_id(backend: &Backend, game_id: &str) -> Result<Option<String>> {
    let sources = backend.database()?.list_external_sources(game_id)?;
    let mut ids = sources
        .into_iter()
        .filter(|source| source.provider == "hikarinagi")
        .filter_map(|source| source.remote_id)
        .filter(|id| valid_id(id))
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    if ids.len() > 1 {
        return Err(ServiceError(
            ErrorCode::Conflict,
            "作品存在多个 Hikarinagi 绑定，请在资料与来源中重新匹配。",
        ));
    }
    Ok(ids.pop())
}
pub(super) fn allowed_rating_url(source: &ExternalSource, url: &str) -> bool {
    source.provider == "hikarinagi"
        && source.remote_id.as_deref().is_some_and(|id| {
            valid_id(id)
                && source.url == format!("{WEB_BASE}/galgames/{id}")
                && url == format!("{WEB_BASE}/galgames/{id}/rates")
        })
}
fn data(value: &Value) -> Result<&Value> {
    if value.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(invalid_response());
    }
    value
        .get("data")
        .filter(|value| value.is_object())
        .ok_or_else(invalid_response)
}
fn parse(remote_id: &str, statistics: &Value, keywords: &Value) -> Result<Wall> {
    let stats = data(statistics)?;
    let count = stats
        .get("rated_count")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_response)?;
    let average = match stats.get("average") {
        Some(Value::Null) if count == 0 => None,
        Some(value) => {
            let average = value
                .as_f64()
                .filter(|average| average.is_finite() && (0.0..=10.0).contains(average))
                .ok_or_else(invalid_response)?;
            (count > 0).then_some(average)
        }
        _ => return Err(invalid_response()),
    };
    let distribution: Vec<ScoreCount> = serde_json::from_value(
        stats
            .get("distribution")
            .cloned()
            .ok_or_else(invalid_response)?,
    )
    .map_err(|_| invalid_response())?;
    if distribution.len() > 10
        || distribution
            .iter()
            .any(|row| !(1..=10).contains(&row.score))
    {
        return Err(invalid_response());
    }
    let mut scores = std::collections::HashSet::new();
    if distribution.iter().any(|row| !scores.insert(row.score))
        || distribution
            .iter()
            .try_fold(0_u64, |sum, row| sum.checked_add(row.count))
            != Some(count)
    {
        return Err(invalid_response());
    }
    let status_counts: StatusCounts = serde_json::from_value(
        stats
            .get("status_counts")
            .cloned()
            .ok_or_else(invalid_response)?,
    )
    .map_err(|_| invalid_response())?;
    let keywords: Vec<Keyword> = serde_json::from_value(
        data(keywords)?
            .get("keywords")
            .cloned()
            .ok_or_else(invalid_response)?,
    )
    .map_err(|_| invalid_response())?;
    if keywords.len() > 100
        || keywords
            .iter()
            .any(|keyword| keyword.word.trim().is_empty() || keyword.word.chars().count() > 100)
    {
        return Err(invalid_response());
    }
    Ok(Wall {
        remote_id: remote_id.into(),
        source_url: format!("{WEB_BASE}/galgames/{remote_id}/rates"),
        average,
        rated_count: count,
        distribution,
        status_counts,
        keywords: keywords
            .into_iter()
            .filter(|keyword| keyword.count > 0)
            .take(24)
            .collect(),
        fetched_at: now(),
        cached: false,
        stale: false,
        message: None,
    })
}
fn fetch_json(http: &Client, url: &str) -> Result<Value> {
    let response = super::http_retry::send(|| {
        http.get(url)
            .header(reqwest::header::ACCEPT, "application/json")
    })?;
    if !response.status().is_success() {
        return Err(ServiceError(
            match response.status().as_u16() {
                401 | 403 => ErrorCode::PermissionDenied,
                404 => ErrorCode::NotFound,
                429 => ErrorCode::RateLimited,
                _ => ErrorCode::NetworkUnavailable,
            },
            "Hikarinagi 安利墙暂时无法获取，请稍后重试。",
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_response())?;
    if bytes.len() > 65536 {
        return Err(invalid_response());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid_response())
}
fn fetch(id: &str) -> Result<Wall> {
    let http = super::network::builder()
        .timeout(Duration::from_secs(8))
        .redirect(Policy::none())
        .user_agent("GalgameManager/0.1 (Hikarinagi public rating statistics)")
        .build()
        .map_err(|_| ServiceError(ErrorCode::NetworkUnavailable, "无法初始化安利墙网络请求。"))?;
    let statistics = fetch_json(
        &http,
        &format!("{WEB_BASE}/api/v3/galgames/{id}/rates/statistics"),
    )?;
    let keywords = fetch_json(
        &http,
        &format!("{WEB_BASE}/api/v3/galgames/{id}/rates/keywords"),
    )?;
    parse(id, &statistics, &keywords)
}
pub fn get(backend: &Backend, request: Request) -> Result<Response> {
    get_with(backend, request, fetch)
}
fn get_with(
    backend: &Backend,
    request: Request,
    fetch: impl FnOnce(&str) -> Result<Wall>,
) -> Result<Response> {
    let Some(id) = bound_id(backend, &request.game_id)? else {
        return Ok(Response { wall: None });
    };
    let key = format!("rates-wall:{id}");
    let cached = backend
        .database()?
        .metadata_cache_response("hikarinagi", &key)?
        .and_then(|(json, _)| serde_json::from_str::<Wall>(&json).ok())
        .filter(|wall| {
            wall.remote_id == id && wall.source_url == format!("{WEB_BASE}/galgames/{id}/rates")
        });
    if !request.refresh {
        if let Some(mut wall) = cached.clone() {
            wall.cached = true;
            return Ok(Response { wall: Some(wall) });
        }
    }
    let result = fetch(&id);
    // A rebind/unbind while the network is running must not show another work's statistics.
    if bound_id(backend, &request.game_id)?.as_deref() != Some(&id) {
        return Err(ServiceError(
            ErrorCode::Cancelled,
            "作品资料绑定已变化，请重新加载安利墙。",
        ));
    }
    match result {
        Ok(wall) => {
            let json = serde_json::to_string(&wall).map_err(|_| invalid_response())?;
            backend.database()?.save_metadata_cache(
                "hikarinagi",
                &key,
                Some(&json),
                "success",
                None,
                &wall.fetched_at,
            )?;
            Ok(Response { wall: Some(wall) })
        }
        Err(error) => match cached {
            Some(mut wall) => {
                wall.cached = true;
                wall.stale = true;
                wall.message = Some(format!("{} 正在显示上次获取的数据。", error.1));
                Ok(Response { wall: Some(wall) })
            }
            None => Err(error),
        },
    }
}

#[cfg(test)]
mod tests;
