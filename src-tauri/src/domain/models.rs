//! Wire DTOs only. They are not database rows or proof of command implementation.
use super::protocol::{GameStatus, InstallSource, MetadataStatus, TaskStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GameInstallation {
    pub id: String,
    pub game_id: String,
    pub absolute_path: String,
    pub executable_path: Option<String>,
    pub source: InstallSource,
    pub steam_app_id: Option<String>,
    pub path_valid: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct GameSummary {
    pub id: String,
    pub title: String,
    pub title_zh: Option<String>,
    pub title_ja: Option<String>,
    pub title_en: Option<String>,
    pub cover_url: Option<String>,
    pub developer: Option<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub source_rating: Option<f64>,
    #[serde(default)]
    pub source_tags: Vec<String>,
    pub status: GameStatus,
    pub favorite: bool,
    pub hidden: bool,
    pub user_rating: Option<f64>,
    pub total_playtime_seconds: u64,
    pub last_played_at: Option<String>,
    pub added_at: String,
    pub metadata_status: MetadataStatus,
    pub installations: Vec<GameInstallation>,
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub has_save_backup: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Screenshot {
    pub id: String,
    pub game_id: String,
    pub install_id: Option<String>,
    pub image_url: String,
    pub thumbnail_url: String,
    pub title: Option<String>,
    pub is_spoiler: bool,
    pub captured_at: Option<String>,
    pub created_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub game_id: String,
    pub title: String,
    pub content_markdown: String,
    pub is_spoiler: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExternalSource {
    pub kind: String,
    pub label: String,
    pub provider: String,
    pub url: String,
    pub remote_id: Option<String>,
    pub official: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcedField {
    #[serde(default)]
    pub remote_id: Option<String>,
    pub field: String,
    pub value: String,
    pub provider: String,
    pub fetched_at: Option<String>,
    pub cached: bool,
    pub manually_edited: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct GameDetail {
    #[serde(default)]
    pub metadata_locked: bool,
    #[serde(flatten)]
    pub summary: GameSummary,
    pub description: Option<String>,
    pub metadata: Vec<SourcedField>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSort {
    Title,
    AddedAt,
    LastPlayedAt,
    Playtime,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GameFilters {
    pub developer: Option<String>,
    pub release_year: Option<u16>,
    pub min_playtime_seconds: Option<u64>,
    pub max_playtime_seconds: Option<u64>,
    pub played_after: Option<String>,
    pub metadata_pending: Option<bool>,
    pub has_save_backup: Option<bool>,
    pub multiple_installations: Option<bool>,
    pub metadata_incomplete: Option<bool>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct GameQuery {
    pub page: u32,
    pub page_size: u32,
    pub search: String,
    pub statuses: Vec<GameStatus>,
    pub sources: Vec<InstallSource>,
    pub tag_ids: Vec<String>,
    pub favorite: Option<bool>,
    pub collection_id: Option<String>,
    pub sort: GameSort,
    pub direction: SortDirection,
    #[serde(default)]
    pub filters: GameFilters,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionKind {
    Normal,
    Smart,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionSummary {
    pub id: String,
    pub name: String,
    pub kind: CollectionKind,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub cover_url: Option<String>,
    pub position: u32,
    pub hidden: bool,
    pub game_count: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionDetail {
    #[serde(flatten)]
    pub summary: CollectionSummary,
    pub query: Option<GameQuery>,
    pub member_ids: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SaveCollectionRequest {
    pub id: Option<String>,
    pub name: String,
    pub kind: CollectionKind,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub cover_url: Option<String>,
    pub position: u32,
    pub hidden: bool,
    pub query: Option<GameQuery>,
    #[serde(default)]
    pub member_ids: Option<Vec<String>>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationSource {
    Local,
    External,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationAction {
    Launch,
    ViewDetails,
    OfficialPage,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Recommendation {
    pub game_id: String,
    pub reason: String,
    pub source: RecommendationSource,
    pub is_installed: bool,
    pub action: RecommendationAction,
    pub official_url: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct HomeSummary {
    #[serde(default)]
    pub dashboard: Option<super::home::HomeDashboard>,
    pub game_count: u64,
    pub playing_count: u64,
    pub completed_count: u64,
    pub week_playtime_seconds: u64,
    pub pending_match_count: u64,
    pub save_issue_count: u64,
    pub continue_game_ids: Vec<String>,
    pub recent_game_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    pub task_id: String,
    pub status: TaskStatus,
    pub phase: String,
    pub processed: u64,
    pub total: Option<u64>,
    pub message: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ScanRootsRequest {
    pub roots: Vec<String>,
    #[serde(deserialize_with = "super::requests::require_false")]
    pub follow_symlinks: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ScanTask {
    pub task_id: String,
    pub status: TaskStatus,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchSession {
    pub session_id: String,
    pub game_id: String,
    pub install_id: String,
    pub started_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SaveSnapshot {
    pub id: String,
    pub save_profile_id: String,
    pub game_id: String,
    pub install_id: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub label: Option<String>,
    pub note: Option<String>,
    pub file_count: u64,
    pub creation_reason: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct RestoreResult {
    pub snapshot_id: String,
    pub safety_backup_id: String,
    pub added_files: u64,
    pub modified_files: u64,
    pub deleted_files: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct MetadataCandidate {
    pub provider: String,
    pub remote_id: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_chinese_description: Option<bool>,
    pub confidence: f64,
    pub matched_fields: Vec<String>,
    pub explanation: String,
    pub fetched_at: String,
    pub cached: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct MatchResult {
    pub game_id: String,
    pub provider: String,
    pub remote_id: String,
    pub matched_at: String,
    #[serde(default)]
    pub translation_message: Option<String>,
    #[serde(default)]
    pub supplementation_message: Option<String>,
    #[serde(default)]
    pub cover_message: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationPreference {
    NotInterested,
    Snoozed,
    None,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PreferenceResult {
    pub game_id: String,
    pub preference: RecommendationPreference,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecommendationPreferenceEntry {
    #[serde(flatten)]
    pub preference: PreferenceResult,
    pub game_title: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_metadata_candidates_remain_compatible_with_description_language_hint() {
        let original = serde_json::json!({
            "provider":"bangumi", "remote_id":"123", "title":"中文作品",
            "subtitle":null, "cover_url":null, "confidence":1.0,
            "matched_fields":["name_cn"], "explanation":"fixture",
            "fetched_at":"2026-10-02T00:00:00Z", "cached":true
        });
        let candidate: MetadataCandidate = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(candidate.has_chinese_description, None);
        assert_eq!(serde_json::to_value(candidate).unwrap(), original);
        for has_chinese in [true, false] {
            let mut with_hint = original.clone();
            with_hint["has_chinese_description"] = serde_json::json!(has_chinese);
            let candidate: MetadataCandidate = serde_json::from_value(with_hint.clone()).unwrap();
            assert_eq!(candidate.has_chinese_description, Some(has_chinese));
            assert_eq!(serde_json::to_value(candidate).unwrap(), with_hint);
        }
    }

    #[test]
    fn game_fixture_round_trips_without_field_drift() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../../../shared/game-fixture.json")).unwrap();
        let game: GameDetail = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(game).unwrap(), value);
    }
}
