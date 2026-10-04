//! Read-only UTC activity snapshots. Duration is always saved playtime, never wall-clock elapsed time.
use super::playtime::PlaySession;
use crate::domain::models::Paginated;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityRange {
    Week,
    Days30,
    Month,
    Year,
    All,
}
#[derive(Debug, Deserialize)]
pub struct ActivityQuery {
    pub range: ActivityRange,
    pub calendar_year: Option<i32>,
    pub session_date: Option<String>,
    pub page: u32,
    pub page_size: u32,
}
#[derive(Debug, Serialize, Default)]
pub struct ActivitySummary {
    pub total_seconds: u64,
    pub played_count: u64,
    pub completed_count: u64,
    pub active_days: u64,
    pub session_count: u64,
    pub active_session_count: u64,
}
#[derive(Debug, Serialize, Clone)]
pub struct ActivityDay {
    pub date: String,
    pub duration_seconds: u64,
    pub session_count: u64,
}
#[derive(Debug, Serialize)]
pub struct ActivityTrend {
    pub date: String,
    pub end_date: String,
    pub duration_seconds: u64,
}
#[derive(Debug, Serialize)]
pub struct ActivityGame {
    pub game_id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub duration_seconds: u64,
}
#[derive(Debug, Serialize)]
pub struct ActivityHour {
    pub hour: u32,
    pub duration_seconds: u64,
}
#[derive(Debug, Serialize)]
pub struct ActivitySnapshot {
    pub as_of: String,
    pub range: ActivityRange,
    pub range_start: String,
    pub range_end: String,
    pub summary: ActivitySummary,
    pub daily: Vec<ActivityDay>,
    pub trend_granularity: String,
    pub trend: Vec<ActivityTrend>,
    pub games: Vec<ActivityGame>,
    pub hours: Vec<ActivityHour>,
    pub calendar_year: i32,
    pub calendar_years: Vec<i32>,
    pub sessions: Paginated<PlaySession>,
    pub session_date: Option<String>,
}
