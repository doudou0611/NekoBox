use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveProfile {
    pub id: String,
    pub game_id: String,
    pub install_id: String,
    pub source_path: String,
    pub backup_before_launch: bool,
    pub backup_after_exit: bool,
    pub retention_count: u32,
    pub source_available: bool,
    pub last_error: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct ConfigureProfile {
    pub id: Option<String>,
    pub install_id: String,
    pub source_path: String,
    pub backup_before_launch: bool,
    pub backup_after_exit: bool,
    pub retention_count: u32,
}
#[derive(Debug, Deserialize)]
pub struct ProfileRequest {
    pub profile_id: String,
}
#[derive(Debug, Deserialize)]
pub struct CreateSnapshot {
    pub profile_id: String,
    pub label: Option<String>,
    pub note: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct SnapshotRequest {
    pub snapshot_id: String,
}
#[derive(Debug, Deserialize)]
pub struct DeleteSnapshot {
    pub snapshot_id: String,
    pub confirmed: bool,
}
#[derive(Debug, Deserialize)]
pub struct DeleteProfile {
    pub profile_id: String,
    pub confirmed: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct RecycledProfile {
    pub profile: SaveProfile,
    pub snapshots: Vec<crate::domain::models::SaveSnapshot>,
    pub deleted_at: String,
    pub backup_directory: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileStamp {
    pub size_bytes: u64,
    pub sha256: String,
}
pub type Manifest = std::collections::BTreeMap<String, FileStamp>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePreview {
    pub preview_id: String,
    pub confirmation_token: String,
    pub snapshot_id: String,
    pub source_path: String,
    pub expires_at: String,
    pub added_files: u64,
    pub modified_files: u64,
    pub deleted_files: u64,
    pub preserved_files: u64,
    pub changes: Vec<FileChange>,
    pub truncated: bool,
}
pub(super) struct PendingPreview {
    pub preview: RestorePreview,
    pub profile: SaveProfile,
    pub current: Manifest,
    pub archive: Manifest,
    pub expires: std::time::Instant,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RestoreJournal {
    pub profile_id: String,
    pub transaction_id: String,
}
#[derive(Default)]
pub struct SaveManager {
    pub(super) previews: std::collections::HashMap<String, PendingPreview>,
}
