use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Default, Deserialize)]
pub struct HomeQuery {
    pub dashboard_window: Option<HomeWindow>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeRange {
    pub start_at: String,
    pub end_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeDay {
    pub date: String,
    pub start_at: String,
    pub end_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeWindow {
    pub time_zone: String,
    pub today: HomeRange,
    pub week: HomeRange,
    pub week_days: Vec<HomeDay>,
    pub memory_day: Option<HomeRange>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct HomeDuration {
    pub game_id: String,
    pub duration_seconds: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct HomeDaily {
    pub date: String,
    pub duration_seconds: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct HomeSession {
    pub id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct HomeMemory {
    pub game_id: String,
    pub duration_seconds: u64,
    pub sessions: Vec<HomeSession>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct HomeDashboard {
    pub status_counts: BTreeMap<String, u64>,
    pub active_game_ids: Vec<String>,
    pub recently_played_game_ids: Vec<String>,
    pub today_seconds: u64,
    pub week_seconds: u64,
    pub week_daily: Vec<HomeDaily>,
    pub week_games: Vec<HomeDuration>,
    pub other_seconds: u64,
    pub memory_games: Vec<HomeMemory>,
    pub queried_at: String,
    pub window: HomeWindow,
}
