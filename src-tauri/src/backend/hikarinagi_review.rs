//! Explicit user submissions using the user's native-vault authorization.
use super::{hikarinagi_account, hikarinagi_rates, invalid, Backend, Result, ServiceError};
use crate::domain::ErrorCode;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Mutex;

static SUBMISSION: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
pub struct GetRequest {
    pub game_id: String,
    pub account_id: u64,
}
#[derive(Deserialize)]
pub struct SubmitRequest {
    pub game_id: String,
    pub account_id: u64,
    pub remote_id: String,
    pub score: u8,
    pub comment: String,
    pub expected_updated_at: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Review {
    pub score: Option<f64>,
    pub comment: String,
    pub updated_at: String,
}
#[derive(Debug, Serialize)]
pub struct GetResponse {
    pub remote_id: String,
    pub review: Option<Review>,
}
#[derive(Debug, Serialize)]
pub struct SubmitResponse {
    pub review: Review,
    pub cache_warning: Option<&'static str>,
}
fn not_signed_in() -> ServiceError {
    ServiceError(
        ErrorCode::PermissionDenied,
        "尚未登录 Hikarinagi，请先在设置中登录。",
    )
}
fn account_matches(backend: &Backend, account_id: u64) -> Result<()> {
    let profile: Option<hikarinagi_account::Profile> =
        backend.database()?.setting("hikarinagi.profile")?;
    match profile {
        None => Err(not_signed_in()),
        Some(profile) if account_id > 0 && profile.id == account_id => Ok(()),
        _ => Err(ServiceError(
            ErrorCode::Conflict,
            "Hikarinagi 登录账户已变化，请重新打开评分窗口。",
        )),
    }
}
fn remote_id(backend: &Backend, game_id: &str) -> Result<String> {
    hikarinagi_rates::bound_id(backend, game_id)?
        .ok_or_else(|| invalid("此作品尚未匹配 Hikarinagi，请先匹配资料。"))
}
fn parse(value: &Value) -> Result<Review> {
    let bad = || {
        ServiceError(
            ErrorCode::InvalidResponse,
            "Hikarinagi 评分响应无效，请刷新后核对评分。未确认提交结果。",
        )
    };
    if value
        .get("id")
        .and_then(Value::as_u64)
        .is_none_or(|id| id == 0)
    {
        return Err(bad());
    }
    let score = match value.get("rate") {
        Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_f64()
                .filter(|score| score.is_finite() && (1.0..=10.0).contains(score))
                .ok_or_else(bad)?,
        ),
        None => return Err(bad()),
    };
    let comment = value
        .get("rate_content")
        .and_then(Value::as_str)
        .filter(|text| text.chars().count() <= 5000)
        .ok_or_else(bad)?;
    let updated_at = value
        .get("updated_at")
        .and_then(Value::as_str)
        .filter(|date| chrono::DateTime::parse_from_rfc3339(date).is_ok())
        .ok_or_else(bad)?;
    Ok(Review {
        score,
        comment: comment.into(),
        updated_at: updated_at.into(),
    })
}
fn payload(request: &SubmitRequest, previous: Option<&Value>) -> Value {
    let mut value = json!({"rate":request.score,"rate_content":request.comment.trim()});
    // Keep the user's existing remote status, visibility, spoilers and dimensions.
    if let Some(previous) = previous {
        for field in [
            "status",
            "status_private",
            "is_spoiler",
            "rate_scenario",
            "rate_direction",
            "rate_music",
            "rate_visual",
            "rate_character",
            "rate_system",
            "time_to_finish_minutes",
        ] {
            if let Some(existing) = previous.get(field) {
                value[field] = existing.clone();
            }
        }
    }
    value
}
fn url(id: &str) -> String {
    format!("{}/rates/galgames/{id}", hikarinagi_account::USER_API_BASE)
}
fn read_at(client: &Client, url: &str, token: &str) -> Result<Option<Value>> {
    let response = client.get(url).bearer_auth(token).send().map_err(|_| {
        ServiceError(
            ErrorCode::NetworkUnavailable,
            "无法加载 Hikarinagi 评分，请稍后重新打开。",
        )
    })?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    hikarinagi_account::response_json(response).map(Some)
}
fn write_at(client: &Client, url: &str, token: &str, payload: &Value) -> Result<Value> {
    // Do not retry writes: a lost response can mean the review was already accepted.
    let response = client
        .put(url)
        .bearer_auth(token)
        .json(payload)
        .send()
        .map_err(|_| {
            ServiceError(
                ErrorCode::NetworkUnavailable,
                "未能确认评论是否提交，请重新打开评分窗口核对。输入已保留。",
            )
        })?;
    hikarinagi_account::response_json(response)
}
pub fn get(backend: &Backend, request: GetRequest) -> Result<GetResponse> {
    get_with(
        backend,
        request,
        || hikarinagi_account::token(backend),
        |id, token| read_at(&hikarinagi_account::client()?, &url(id), token),
    )
}
fn get_with(
    backend: &Backend,
    request: GetRequest,
    token: impl FnOnce() -> Result<Option<String>>,
    read: impl FnOnce(&str, &str) -> Result<Option<Value>>,
) -> Result<GetResponse> {
    let token = token()?.ok_or_else(not_signed_in)?;
    account_matches(backend, request.account_id)?;
    let id = remote_id(backend, &request.game_id)?;
    let previous = read(&id, &token)?;
    account_matches(backend, request.account_id)?;
    if remote_id(backend, &request.game_id)? != id {
        return Err(ServiceError(
            ErrorCode::Cancelled,
            "作品绑定已变化，请重新打开评分窗口。",
        ));
    }
    Ok(GetResponse {
        remote_id: id,
        review: previous.as_ref().map(parse).transpose()?,
    })
}
pub fn submit(backend: &Backend, request: SubmitRequest) -> Result<SubmitResponse> {
    let _guard = SUBMISSION
        .try_lock()
        .map_err(|_| ServiceError(ErrorCode::Conflict, "评论正在提交，请勿重复操作。"))?;
    let client = hikarinagi_account::client()?;
    submit_with(
        backend,
        request,
        || hikarinagi_account::token(backend),
        |id, token| read_at(&client, &url(id), token),
        |id, token, data| write_at(&client, &url(id), token, data),
    )
}
fn submit_with(
    backend: &Backend,
    request: SubmitRequest,
    token: impl FnOnce() -> Result<Option<String>>,
    read: impl FnOnce(&str, &str) -> Result<Option<Value>>,
    write: impl FnOnce(&str, &str, &Value) -> Result<Value>,
) -> Result<SubmitResponse> {
    if !(1..=10).contains(&request.score)
        || request.comment.chars().count() > 5000
        || request.comment.contains('\0')
    {
        return Err(invalid("请选择 1～10 颗星，评论最多 5000 字。"));
    }
    let token = token()?.ok_or_else(not_signed_in)?;
    account_matches(backend, request.account_id)?;
    let id = remote_id(backend, &request.game_id)?;
    if id != request.remote_id {
        return Err(ServiceError(
            ErrorCode::Cancelled,
            "作品绑定已变化，请重新打开评分窗口。",
        ));
    }
    let previous = read(&id, &token)?;
    let old_review = previous.as_ref().map(parse).transpose()?;
    if old_review.as_ref().map(|review| review.updated_at.as_str())
        != request.expected_updated_at.as_deref()
    {
        return Err(ServiceError(
            ErrorCode::Conflict,
            "评分已在其他窗口更新，请重新打开核对后再提交。输入已保留。",
        ));
    }
    account_matches(backend, request.account_id)?;
    if remote_id(backend, &request.game_id)? != id {
        return Err(ServiceError(
            ErrorCode::Cancelled,
            "作品绑定已变化，请重新打开评分窗口。",
        ));
    }
    let result = write(&id, &token, &payload(&request, previous.as_ref()))?;
    let review = parse(&result)?;
    if review.score != Some(f64::from(request.score)) {
        return Err(ServiceError(
            ErrorCode::InvalidResponse,
            "网站返回的评分与提交不一致，请重新打开核对。未确认提交结果。",
        ));
    }
    // Community statistics are an explicit-refresh local snapshot.
    Ok(SubmitResponse {
        review,
        cache_warning: None,
    })
}

#[cfg(test)]
mod tests;
