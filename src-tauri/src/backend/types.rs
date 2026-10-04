use crate::domain::{
    models::TaskProgress,
    protocol::{GameStatus, InstallSource},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct BackendStatus {
    pub data_directory: String,
    pub schema_version: u32,
    pub portable: bool,
    pub last_scan_task_id: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportGameRequest {
    pub directory: String,
    pub title: String,
    pub game_id: Option<String>,
    #[serde(default)]
    pub skip_metadata: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PreviewImportRequest {
    pub roots: Vec<String>,
    #[serde(deserialize_with = "crate::domain::requests::require_false")]
    pub follow_symlinks: bool,
    #[serde(default)]
    pub single_executable: Option<String>,
    #[serde(default)]
    pub single_directory: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewCandidate {
    pub directory: String,
    pub folder_name: String,
    pub search_name: String,
    pub executables: Vec<ExecutableCandidate>,
    pub selected_executable: Option<String>,
    pub existing_game_id: Option<String>,
    pub duplicate_reason: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportPreviewReport {
    pub items: Vec<ImportPreviewCandidate>,
    pub scanned_directories: u64,
    pub skipped_directories: u64,
    pub issue_count: u64,
    pub issues: Vec<ScanIssue>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateGameRequest {
    pub game_id: String,
    pub title: String,
    pub status: GameStatus,
    pub favorite: bool,
    pub hidden: bool,
    pub user_rating: Option<f64>,
}
#[derive(Debug, Deserialize)]
pub struct ScreenshotQuery {
    pub game_id: String,
    #[serde(default)]
    pub include_spoilers: bool,
}
#[derive(Debug, Deserialize)]
pub struct ScanScreenshotsRequest {
    pub install_id: String,
}
#[derive(Debug, Deserialize)]
pub struct UpdateScreenshotRequest {
    pub game_id: String,
    pub screenshot_id: String,
    pub title: String,
    pub captured_at: Option<String>,
    pub is_spoiler: bool,
}
#[derive(Debug, Deserialize)]
pub struct NoteRequest {
    pub id: Option<String>,
    pub game_id: String,
    pub title: String,
    pub content_markdown: String,
    pub is_spoiler: bool,
}
#[derive(Debug, Deserialize)]
pub struct DeleteNoteRequest {
    pub id: String,
    #[serde(deserialize_with = "crate::domain::requests::require_true")]
    pub confirmed: bool,
}
#[derive(Debug, Deserialize)]
pub struct ReplaceGameTagsRequest {
    pub game_id: String,
    pub tag_names: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveRecordRequest {
    pub id: String,
    #[serde(deserialize_with = "crate::domain::requests::require_true")]
    pub confirmed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutableCandidate {
    pub path: String,
    pub product_name: Option<String>,
    pub file_description: Option<String>,
    pub company_name: Option<String>,
    pub fingerprint: String,
    #[serde(default)]
    pub title_evidence: Vec<TitleEvidence>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleEvidence {
    pub source: String,
    pub value: String,
    pub weight: u8,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct InstallationDetails {
    pub id: String,
    pub game_id: String,
    pub absolute_path: String,
    pub executable_path: Option<String>,
    pub source: InstallSource,
    pub steam_app_id: Option<String>,
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    pub environment: BTreeMap<String, String>,
    pub candidates: Vec<ExecutableCandidate>,
    pub main_process_name: Option<String>,
    pub track_after_launcher_exit: bool,
    #[serde(default)]
    pub idle_timeout_minutes: Option<u16>,
    #[serde(default)]
    pub use_locale_emulator: Option<bool>,
    #[serde(default)]
    pub use_magpie: Option<bool>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigureInstallationRequest {
    pub install_id: String,
    pub executable_path: String,
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub steam_app_id: Option<String>,
    #[serde(default)]
    pub main_process_name: Option<String>,
    #[serde(default = "default_track_after_launcher_exit")]
    pub track_after_launcher_exit: bool,
    #[serde(default)]
    pub idle_timeout_minutes: Option<u16>,
    #[serde(default)]
    pub use_locale_emulator: Option<bool>,
    #[serde(default)]
    pub use_magpie: Option<bool>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExternalSourcesRequest {
    pub game_id: String,
}
#[derive(Debug, Deserialize)]
pub struct BindSourceRequest {
    pub game_id: String,
    pub install_id: Option<String>,
    pub provider: String,
    pub value: String,
}
#[derive(Debug, Deserialize)]
pub struct OpenSourceRequest {
    pub game_id: String,
    pub url: String,
}
fn default_track_after_launcher_exit() -> bool {
    true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanIssue {
    pub path: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    #[serde(flatten)]
    pub progress: TaskProgress,
    pub imported: u64,
    pub unchanged: u64,
    pub issue_count: u64,
    pub issues: Vec<ScanIssue>,
    pub truncated: bool,
}
#[derive(Debug, Deserialize)]
pub struct InstallIdRequest {
    pub install_id: String,
}
#[derive(Debug, Deserialize)]
pub struct CollectionIdRequest {
    pub collection_id: String,
}
#[derive(Debug, Deserialize)]
pub struct EmptyRequest {}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanAction {
    Pause,
    Resume,
    Cancel,
}
#[derive(Debug, Deserialize)]
pub struct ControlScanRequest {
    pub task_id: String,
    pub action: ScanAction,
}
