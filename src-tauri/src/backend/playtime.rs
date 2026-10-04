use super::{invalid, Result};
use serde::{Deserialize, Serialize};

pub const MAX_DURATION: u64 = 315_360_000;

#[derive(Debug, Deserialize)]
pub struct SessionQuery {
    pub game_id: Option<String>,
    pub page: u32,
    pub page_size: u32,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PlaySession {
    pub id: String,
    pub game_id: String,
    pub game_title: String,
    pub install_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: u64,
    pub process_name: Option<String>,
    pub end_reason: Option<String>,
    pub corrected: bool,
    pub correction_reason: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct CorrectSessionRequest {
    pub session_id: String,
    pub expected_duration_seconds: u64,
    pub duration_seconds: u64,
    pub reason: String,
    #[serde(deserialize_with = "crate::domain::requests::require_true")]
    pub confirmed: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Correction {
    pub previous_seconds: u64,
    pub duration_seconds: u64,
    pub reason: String,
    pub corrected_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub occurred_at: String,
    pub duration_seconds: u64,
}
#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub game_id: Option<String>,
    pub days: u32,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DailyPlaytime {
    pub date: String,
    pub duration_seconds: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct GamePlaytime {
    pub game_id: String,
    pub title: String,
    pub duration_seconds: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PlaytimeStats {
    pub total_seconds: u64,
    pub period_seconds: u64,
    pub session_count: u64,
    pub active_session_count: u64,
    pub daily: Vec<DailyPlaytime>,
    pub games: Vec<GamePlaytime>,
}
pub fn validate_correction(r: &CorrectSessionRequest) -> Result<()> {
    if !r.confirmed
        || r.duration_seconds > MAX_DURATION
        || r.reason.trim().is_empty()
        || r.reason.chars().count() > 500
    {
        return Err(invalid(
            "请确认修正并填写原因；时长不能超过十年，原因不能超过 500 字。",
        ));
    }
    Ok(())
}
