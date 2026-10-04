//! Explicitly confirmed uploads; never upload paths or rewrite local sessions.
use super::*;
use crate::domain::{models::GameDetail, protocol::GameStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Deserialize)]
pub struct Request {
    pub provider: String,
    pub confirmed: bool,
}
#[derive(Serialize)]
pub struct Failure {
    pub title: String,
    pub message: &'static str,
}
#[derive(Serialize)]
pub struct Report {
    pub processed: usize,
    pub synced: usize,
    pub skipped: usize,
    pub failed: usize,
    pub failures: Vec<Failure>,
    pub message: &'static str,
}
fn payload(provider: &str, game: &GameDetail, previous: &Value) -> Result<Value> {
    let status = match game.summary.status {
        GameStatus::NotStarted => 1,
        GameStatus::Completed => 2,
        GameStatus::Playing => 3,
        GameStatus::Paused => 4,
        GameStatus::Dropped => 5,
        GameStatus::PendingConfirmation => return Err(invalid("待确认状态不上传。")),
    };
    let rating = game
        .summary
        .user_rating
        .filter(|v| v.is_finite() && *v >= 1.0 && *v <= 10.0);
    if provider == "bangumi" {
        let mut data = json!({"type":status});
        if let Some(rating) = rating {
            data["rate"] = json!(rating.round() as u64);
        }
        if previous.is_null() {
            data["private"] = json!(true);
        }
        return Ok(data);
    }
    let mut data = serde_json::Map::new();
    for field in [
        "rate",
        "rate_scenario",
        "rate_direction",
        "rate_music",
        "rate_visual",
        "rate_character",
        "rate_system",
        "rate_content",
        "is_spoiler",
        "status_private",
        "time_to_finish_minutes",
    ] {
        if let Some(value) = previous.get(field) {
            data.insert(field.into(), value.clone());
        }
    }
    if previous.is_null() {
        data.insert("status_private".into(), json!(true));
    }
    data.insert(
        "status".into(),
        json!(match status {
            1 => "PLAN",
            2 => "COMPLETED",
            3 => "GOING",
            4 => "ON_HOLD",
            _ => "DROPPED",
        }),
    );
    if let Some(rating) = rating {
        data.insert("rate".into(), json!(rating));
    }
    if game.summary.status == GameStatus::Completed && game.summary.total_playtime_seconds > 0 {
        let minutes = game.summary.total_playtime_seconds / 60;
        if minutes <= 525600 {
            data.insert("time_to_finish_minutes".into(), json!(minutes));
        }
    }
    Ok(Value::Object(data))
}
fn checked_id(id: &str) -> Result<u64> {
    id.parse::<u64>()
        .ok()
        .filter(|n| *n > 0 && id.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(|| invalid("作品的远端绑定无效，请重新匹配。"))
}
fn send_one(
    provider: &str,
    token: &str,
    username: &str,
    remote: &str,
    game: &GameDetail,
) -> Result<()> {
    let remote = checked_id(remote)?;
    let client = super::hikarinagi_account::client()?;
    if provider == "bangumi" {
        let mut url = reqwest::Url::parse("https://api.bgm.tv/v0/users/")
            .map_err(|_| invalid("同步地址无效。"))?;
        url.path_segments_mut()
            .map_err(|_| invalid("同步地址无效。"))?
            .pop_if_empty()
            .extend([username, "collections", &remote.to_string()]);
        let response = client.get(url).bearer_auth(token).send().map_err(|_| {
            ServiceError(ErrorCode::NetworkUnavailable, "Bangumi 同步暂时无法连接。")
        })?;
        let existing = response.status() != reqwest::StatusCode::NOT_FOUND;
        let previous = if existing {
            super::bangumi::request_json(response)?
        } else {
            Value::Null
        };
        let data = payload(provider, game, &previous)?;
        let url = format!("https://api.bgm.tv/v0/users/-/collections/{remote}");
        let request = if existing {
            client.patch(url)
        } else {
            client.post(url)
        };
        let response = request.bearer_auth(token).json(&data).send().map_err(|_| {
            ServiceError(ErrorCode::NetworkUnavailable, "Bangumi 同步暂时无法连接。")
        })?;
        if !response.status().is_success() {
            super::bangumi::request_json(response)?;
            return Err(ServiceError(
                ErrorCode::InvalidResponse,
                "Bangumi 未确认同步结果。",
            ));
        }
    } else {
        let url = format!(
            "{}/rates/galgames/{remote}",
            super::hikarinagi_account::USER_API_BASE
        );
        let response = client.get(&url).bearer_auth(token).send().map_err(|_| {
            ServiceError(
                ErrorCode::NetworkUnavailable,
                "Hikarinagi 同步暂时无法连接。",
            )
        })?;
        let previous = if response.status() == reqwest::StatusCode::NOT_FOUND {
            Value::Null
        } else {
            super::hikarinagi_account::response_json(response)?
        };
        let data = payload(provider, game, &previous)?;
        let response = client
            .put(url)
            .bearer_auth(token)
            .json(&data)
            .send()
            .map_err(|_| {
                ServiceError(
                    ErrorCode::NetworkUnavailable,
                    "Hikarinagi 同步暂时无法连接。",
                )
            })?;
        super::hikarinagi_account::response_json(response)?;
    }
    Ok(())
}
fn run_with(
    backend: &Backend,
    provider: &str,
    send: impl Fn(&str, &GameDetail) -> Result<()>,
) -> Result<Report> {
    let (games, total) = {
        let database = backend.database()?;
        (
            database.account_sync_games(provider)?,
            database.account_sync_game_count()?,
        )
    };
    let mut report = Report {
        processed: 0,
        synced: 0,
        skipped: total.saturating_sub(games.len()),
        failed: 0,
        failures: Vec::new(),
        message: if provider == "bangumi" {
            "本轮上传已结束；Bangumi 不支持游玩耗时字段，请查看成功、失败与跳过数量。"
        } else {
            "本轮上传已结束，请查看游玩状态、评分与通关耗时的成功、失败与跳过数量。"
        },
    };
    for (game, remote) in games {
        if game.summary.status == GameStatus::PendingConfirmation {
            report.skipped += 1;
            continue;
        }
        report.processed += 1;
        match send(&remote, &game) {
            Ok(()) => report.synced += 1,
            Err(e) => {
                report.failed += 1;
                if report.failures.len() < 100 {
                    report.failures.push(Failure {
                        title: game.summary.title.clone(),
                        message: e.1,
                    });
                }
            }
        }
    }
    Ok(report)
}
pub fn sync(backend: &Backend, request: Request) -> Result<Report> {
    if !request.confirmed || !matches!(request.provider.as_str(), "bangumi" | "hikarinagi") {
        return Err(invalid(
            "同步需要用户确认且来源必须是 Bangumi 或 Hikarinagi。",
        ));
    }
    let _guard = backend
        .account_sync_lock
        .try_lock()
        .map_err(|_| ServiceError(ErrorCode::Conflict, "已有账户同步正在进行。"))?;
    let (token, username) = if request.provider == "bangumi" {
        let account = super::bangumi_account::account(backend)?;
        if account.status != "authenticated" {
            return Err(ServiceError(
                ErrorCode::PermissionDenied,
                "请先登录有效的 Bangumi 账户。",
            ));
        }
        (
            super::bangumi_account::token(backend)?.ok_or_else(|| invalid("请先登录 Bangumi。"))?,
            account
                .profile
                .ok_or_else(|| invalid("Bangumi 账户信息缺失。"))?
                .username,
        )
    } else {
        (
            super::hikarinagi_account::token(backend)?
                .ok_or_else(|| invalid("请先登录自己的 Hikarinagi 账户。"))?,
            String::new(),
        )
    };
    run_with(backend, &request.provider, |remote, game| {
        send_one(&request.provider, &token, &username, remote, game)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation_and_source_are_required_before_network() {
        let root = std::env::temp_dir().join(format!("gm-account-sync-{}", id()));
        let b = Backend::open(root.clone()).unwrap();
        assert!(sync(
            &b,
            Request {
                provider: "bangumi".into(),
                confirmed: false
            }
        )
        .is_err());
        assert!(sync(
            &b,
            Request {
                provider: "vndb".into(),
                confirmed: true
            }
        )
        .is_err());
        assert!(checked_id("1/path").is_err());
        assert!(checked_id("0").is_err());
        assert_eq!(checked_id("123").unwrap(), 123);
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bound_games_preserve_reviews_and_report_actual_failures() {
        let root = std::env::temp_dir().join(format!("gm-account-sync-{}", id()));
        let b = Backend::open(root.clone()).unwrap();
        let gid = b
            .database()
            .unwrap()
            .import_installation(
                "C:/sync/game",
                "fixture",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "",
            )
            .unwrap();
        b.database()
            .unwrap()
            .apply_remote_fields(
                &gid,
                "hikarinagi",
                "123",
                &[("title".into(), "中文作品".into())],
                &[],
                &now(),
                true,
            )
            .unwrap();
        let mut game = b.database().unwrap().get_game(&gid).unwrap();
        b.database()
            .unwrap()
            .update_game(&crate::backend::types::UpdateGameRequest {
                game_id: gid.clone(),
                title: game.summary.title.clone(),
                status: GameStatus::NotStarted,
                favorite: false,
                hidden: false,
                user_rating: None,
            })
            .unwrap();
        game.summary.status = GameStatus::NotStarted;
        let previous = json!({"rate_content":"keep review","status_private":true,"rate_visual":9,"time_to_finish_minutes":500});
        let data = payload("hikarinagi", &game, &previous).unwrap();
        assert_eq!(data["rate_content"], "keep review");
        assert_eq!(data["rate_visual"], 9);
        assert_eq!(data["time_to_finish_minutes"], 500);
        assert_eq!(data["status"], "PLAN");
        assert!(data.get("title").is_none());
        assert!(data.get("directory").is_none());
        game.summary.status = GameStatus::Completed;
        game.summary.total_playtime_seconds = 3660;
        game.summary.user_rating = Some(8.4);
        assert_eq!(
            payload("hikarinagi", &game, &previous).unwrap()["time_to_finish_minutes"],
            61
        );
        let bgm = payload("bangumi", &game, &previous).unwrap();
        assert!(bgm.get("time_to_finish_minutes").is_none());
        assert_eq!(bgm["rate"], 8);
        let report = run_with(&b, "hikarinagi", |_, _| {
            Err(ServiceError(
                ErrorCode::NetworkUnavailable,
                "fixture failure",
            ))
        })
        .unwrap();
        assert_eq!(report.failed, 1);
        assert_eq!(report.synced, 0);
        assert_eq!(report.processed, 1);
        assert_eq!(
            b.database().unwrap().get_game(&gid).unwrap().summary.status,
            GameStatus::NotStarted
        );
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
}
