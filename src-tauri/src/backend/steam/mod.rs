//! Task-local Steam metadata. Community bindings never project catalog fields.
pub mod scan;
use super::*;
use crate::domain::{
    models::GameDetail,
    requests::{ConfirmMetadataMatchRequest, SearchMetadataRequest},
};
use reqwest::{blocking::Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io::Read, time::Duration};

#[derive(Deserialize)]
pub struct PrepareRequest {
    pub app_id: String,
    pub directory: String,
    pub batch_id: String,
    #[serde(default = "default_true")]
    pub match_hikarinagi: bool,
    #[serde(default)]
    pub hikarinagi_remote_id: Option<String>,
}
fn default_true() -> bool {
    true
}
#[derive(Deserialize)]
pub struct ImportRequest {
    pub app_id: String,
    pub directory: String,
    pub preparation_id: Option<String>,
}
#[derive(Serialize)]
pub struct ImportResult {
    pub game: GameDetail,
}

pub fn prepare(b: &Backend, q: PrepareRequest) -> Result<import_metadata::Preparation> {
    let local = scan::verify(&q.directory, &q.app_id)?;
    import_metadata::prepare_with(
        b,
        import_metadata::PrepareRequest {
            manual: false,
            single_source: true,
            title_hint: Some(local.name),
            directory: local.directory,
            batch_id: Some(q.batch_id),
            provider: "steam".into(),
            remote_id: q.app_id,
        },
        |stage, request| {
            confirm(
                stage,
                request,
                q.hikarinagi_remote_id.as_deref(),
                q.match_hikarinagi,
            )
        },
    )
}
pub fn import(b: &Backend, q: ImportRequest) -> Result<ImportResult> {
    let local = scan::verify(&q.directory, &q.app_id)?;
    let game = import_metadata::commit_steam(
        b,
        import_metadata::CommitRequest {
            directory: local.directory,
            title: local.name,
            executable_path: None,
            preparation_id: q.preparation_id,
        },
        &q.app_id,
    )?;
    Ok(ImportResult { game })
}
fn active(b: &Backend) -> Result<()> {
    b.ensure_search_active()?;
    if let Some(id) = &b.import_batch {
        import_metadata::batch_config(b, id)?;
    }
    Ok(())
}
fn client() -> Result<Client> {
    network::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(format!("NekoBox/{}", env!("CARGO_PKG_VERSION")))
        .redirect(Policy::none())
        .build()
        .map_err(|_| ServiceError(ErrorCode::NetworkUnavailable, "无法初始化 Steam 网络请求。"))
}
fn json(b: &Backend, http: &Client, url: &str) -> Result<Value> {
    active(b)?;
    let response = http_retry::send_checked(
        || http.get(url).header("Accept", "application/json"),
        || active(b),
    )?;
    if !response.status().is_success() {
        return Err(ServiceError(
            if response.status().as_u16() == 429 {
                ErrorCode::RateLimited
            } else {
                ErrorCode::NetworkUnavailable
            },
            "Steam 资料服务暂时不可用，请稍后重试。",
        ));
    }
    let mut bytes = vec![];
    response
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ServiceError(ErrorCode::NetworkUnavailable, "Steam 资料读取中断。"))?;
    active(b)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(invalid("Steam 资料超过大小限制。"));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| ServiceError(ErrorCode::InvalidResponse, "Steam 资料返回格式无效。"))
}
fn details<'a>(value: &'a Value, app_id: &str) -> Result<&'a Value> {
    let entry = value
        .get(app_id)
        .ok_or_else(|| invalid("Steam 没有返回该游戏资料。"))?;
    let data = entry
        .get("data")
        .ok_or_else(|| invalid("Steam 游戏资料为空。"))?;
    if entry.get("success").and_then(Value::as_bool) != Some(true)
        || data.get("steam_appid").and_then(Value::as_u64) != app_id.parse::<u64>().ok()
        || text(data, "name").is_none()
    {
        return Err(ServiceError(
            ErrorCode::InvalidResponse,
            "Steam 游戏身份或资料无效。",
        ));
    }
    Ok(data)
}
fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}
fn plain_text(html: &str) -> String {
    let html = html
        .replace("<br>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n");
    let mut inside = false;
    html.chars()
        .filter(|c| match c {
            '<' => {
                inside = true;
                false
            }
            '>' => {
                inside = false;
                false
            }
            _ => !inside,
        })
        .collect::<String>()
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .trim()
        .to_owned()
}
fn date(value: &str) -> Option<String> {
    let normalized = value.replace(['年', '月'], "-").replace(['日', ' '], "");
    for format in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&normalized, format) {
            return Some(date.to_string());
        }
    }
    for format in ["%b %d, %Y", "%d %b, %Y", "%B %d, %Y", "%d %B, %Y"] {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(value.trim(), format) {
            return Some(date.to_string());
        }
    }
    None
}
fn fields(data: &Value) -> Vec<(String, String)> {
    let list = |key| {
        data.get(key).and_then(Value::as_array).map(|v| {
            v.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、")
        })
    };
    let tags = data.get("genres").and_then(Value::as_array).map(|v| {
        v.iter()
            .filter_map(|v| text(v, "description"))
            .filter_map(|tag| localized_source_tag(&tag))
            .collect::<Vec<_>>()
            .join("\n")
    });
    let rating = data
        .pointer("/metacritic/score")
        .and_then(Value::as_f64)
        .filter(|v| *v > 0.0 && *v <= 100.0)
        .map(|v| format!("{:.2}", v / 10.0));
    [
        ("title", text(data, "name")),
        ("developer", list("developers")),
        ("publisher", list("publishers")),
        (
            "description",
            text(data, "short_description").map(|s| plain_text(&s)),
        ),
        (
            "release_date",
            data.pointer("/release_date/date")
                .and_then(Value::as_str)
                .and_then(date),
        ),
        ("source_tags", tags),
        ("source_rating", rating),
    ]
    .into_iter()
    .filter_map(|(key, v)| v.filter(|v| !v.is_empty()).map(|v| (key.into(), v)))
    .collect()
}
pub(crate) fn trusted_steam_image(url: &str) -> bool {
    let Ok(value) = reqwest::Url::parse(url) else {
        return false;
    };
    value.scheme() == "https"
        && value.username().is_empty()
        && value.password().is_none()
        && value.port().is_none()
        && matches!(
            value.host_str(),
            Some(
                "cdn.akamai.steamstatic.com"
                    | "shared.akamai.steamstatic.com"
                    | "cdn.cloudflare.steamstatic.com"
                    | "shared.cloudflare.steamstatic.com"
                    | "steamcdn-a.akamaihd.net"
            )
        )
        && (value.path().starts_with("/steam/apps/")
            || value.path().starts_with("/store_item_assets/steam/apps/"))
}
fn asset_covers(item: &Value, id: &str) -> Vec<String> {
    let assets = &item["assets"];
    let format = text(assets, "asset_url_format")
        .unwrap_or_else(|| format!("steam/apps/{id}/${{FILENAME}}"));
    ["library_capsule_2x", "library_capsule"]
        .into_iter()
        .filter_map(|key| text(assets, key))
        .filter_map(|path| {
            let url = if path.starts_with("https://") {
                path
            } else if format.contains("${FILENAME}") {
                let url = format.replace("${FILENAME}", path.trim_start_matches('/'));
                if url.starts_with("https://") {
                    url
                } else {
                    format!(
                        "https://shared.akamai.steamstatic.com/store_item_assets/{}",
                        url.trim_start_matches('/')
                    )
                }
            } else {
                return None;
            };
            trusted_steam_image(&url).then_some(url)
        })
        .collect()
}
fn modern_covers(b: &Backend, http: &Client, id: &str) -> Result<Vec<String>> {
    let input = serde_json::json!({"ids":[{"appid":scan::app_id(id)?}],"context":{"language":"schinese","country_code":"CN","steam_realm":1},"data_request":{"include_assets":true,"include_basic_info":true}});
    let mut url =
        reqwest::Url::parse("https://api.steampowered.com/IStoreBrowseService/GetItems/v1/")
            .map_err(|_| invalid("Steam 资料地址无效。"))?;
    url.query_pairs_mut()
        .append_pair("input_json", &input.to_string());
    let payload = json(b, http, url.as_str())?;
    Ok(payload
        .pointer("/response/store_items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| {
            item["appid"].as_u64() == id.parse().ok() && item["success"].as_u64() == Some(1)
        })
        .flat_map(|item| asset_covers(item, id))
        .collect())
}
fn exact(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn unique_community_id(
    aliases: &[String],
    candidates: &[crate::domain::models::MetadataCandidate],
) -> Option<String> {
    let mut ids = candidates
        .iter()
        .filter(|c| {
            aliases.iter().any(|a| {
                let key = exact(a);
                !key.is_empty()
                    && (key == exact(&c.title)
                        || c.subtitle.as_ref().is_some_and(|s| key == exact(s)))
            })
        })
        .map(|c| c.remote_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    (ids.len() == 1).then(|| ids[0].clone())
}
pub(crate) fn bind_community(b: &Backend, game_id: &str, remote_id: &str) -> Result<()> {
    if remote_id.is_empty()
        || remote_id.len() > 20
        || !remote_id.bytes().all(|c| c.is_ascii_digit())
        || !remote_id.bytes().any(|c| c != b'0')
    {
        return Err(invalid("Hikarinagi 安利墙标识无效。"));
    }
    b.database()?.apply_remote_fields(
        game_id,
        "hikarinagi",
        remote_id,
        &[("community_binding".into(), remote_id.into())],
        &[],
        &now(),
        true,
    )?;
    Ok(())
}
/// The caller owns an isolated repository; the real library has no writes here.
pub(crate) fn confirm(
    b: &Backend,
    q: &ConfirmMetadataMatchRequest,
    community_id: Option<&str>,
    supplement: bool,
) -> Result<Option<String>> {
    scan::app_id(&q.remote_id)?;
    let http = client()?;
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}&l=schinese",
        q.remote_id
    );
    let payload = json(b, &http, &url)?;
    let data = details(&payload, &q.remote_id)?;
    let title = text(data, "name").ok_or_else(|| invalid("Steam 游戏标题为空。"))?;
    b.database()?
        .set_metadata_priority(&q.game_id, &["steam".into()])?;
    let mut fields = fields(data);
    let fetched = now();
    let mut cover = None;
    let mut cover_failure = None;
    let mut candidates = vec![
        Some(format!(
            "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900_schinese.jpg",
            q.remote_id
        )),
        Some(format!(
            "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900.jpg",
            q.remote_id
        )),
        text(data, "header_image"),
    ];
    match modern_covers(b, &http, &q.remote_id) {
        Ok(urls) => {
            candidates.splice(0..0, urls.into_iter().map(Some));
        }
        Err(e) => {
            if e.0 == ErrorCode::Cancelled {
                return Err(e);
            }
        }
    }
    for url in candidates
        .into_iter()
        .flatten()
        .filter(|u| trusted_steam_image(u))
    {
        active(b)?;
        match bangumi::cache_image_for(b, "steam", &url) {
            Ok(path) => {
                cover = Some(path);
                fields.push(("cover_url".into(), url));
                break;
            }
            Err(e) => {
                if e.0 == ErrorCode::Cancelled {
                    return Err(e);
                }
                cover_failure = Some(e);
            }
        }
    }
    let cover = cover
        .ok_or_else(|| cover_failure.unwrap_or(invalid("Steam 封面未能保存，请重试刮削。")))?;
    fields.push(("cover_path".into(), cover));
    let cap = app_settings::get(b)?.tag_limit;
    b.database()?.apply_remote_fields(
        &q.game_id,
        "steam",
        &q.remote_id,
        &fields,
        &[],
        &fetched,
        true,
    )?;
    b.database()?.limit_source_tags(&q.game_id, cap)?;
    if !supplement {
        return Ok(Some("已使用 Steam 资料；安利墙自动关联已关闭。".into()));
    }
    active(b)?;
    let binding = if let Some(id) = community_id {
        Some(id.to_owned())
    } else {
        let mut aliases = vec![title];
        if let Some(hint) = &q.title_hint {
            if !aliases.contains(hint) {
                aliases.push(hint.clone());
            }
        }
        let mut candidates = vec![];
        for alias in &aliases {
            match hikarinagi::search(
                b,
                &SearchMetadataRequest {
                    manual: false,
                    batch_id: None,
                    query: alias.clone(),
                    providers: vec!["hikarinagi".into()],
                    cache: false,
                },
            ) {
                Ok(items) => candidates.extend(items),
                Err(e) => {
                    active(b)?;
                    return Ok(Some(format!(
                        "Steam 资料已准备；安利墙关联暂不可用：{}",
                        e.1
                    )));
                }
            }
        }
        unique_community_id(&aliases, &candidates)
    };
    active(b)?;
    match binding {
        Some(id) => {
            bind_community(b, &q.game_id, &id)?;
            Ok(Some(format!("已关联 Hikarinagi 安利墙 · #{id}")))
        }
        None => Ok(Some(
            "Steam 资料已准备；未找到唯一的安利墙关联，可手动选择作品。".into(),
        )),
    }
}

#[cfg(test)]
mod tests;
