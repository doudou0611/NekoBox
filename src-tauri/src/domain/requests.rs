//! Command payloads. Actual implementation status is tracked in shared/protocol.json.
use super::models::RecommendationPreference;
use serde::{Deserialize, Deserializer};

pub fn require_false<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    let value = bool::deserialize(deserializer)?;
    if value {
        Err(serde::de::Error::custom("follow_symlinks must be false"))
    } else {
        Ok(false)
    }
}
pub fn require_true<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    let value = bool::deserialize(deserializer)?;
    if value {
        Ok(true)
    } else {
        Err(serde::de::Error::custom("user initiation is required"))
    }
}
#[derive(Debug, Deserialize)]
pub struct TaskIdRequest {
    pub task_id: String,
}
#[derive(Debug, Deserialize)]
pub struct GameIdRequest {
    pub game_id: String,
}
#[derive(Debug, Deserialize)]
pub struct LaunchOptions {
    #[serde(deserialize_with = "require_true")]
    pub user_initiated: bool,
}
#[derive(Debug, Deserialize)]
pub struct LaunchGameRequest {
    pub install_id: String,
    pub options: LaunchOptions,
}
#[derive(Debug, Deserialize)]
pub struct RestoreSaveSnapshotRequest {
    pub snapshot_id: String,
    pub preview_id: String,
    pub confirmation_token: String,
}
#[derive(Debug, Deserialize)]
pub struct SearchMetadataRequest {
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub batch_id: Option<String>,
    pub query: String,
    pub providers: Vec<String>,
    #[serde(default = "default_true")]
    pub cache: bool,
}

fn default_true() -> bool {
    true
}
#[derive(Debug, Deserialize)]
pub struct ConfirmMetadataMatchRequest {
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub title_hint: Option<String>,
    pub game_id: String,
    pub provider: String,
    pub remote_id: String,
}
#[derive(Debug, Deserialize)]
pub struct UnbindMetadataRequest {
    pub game_id: String,
    pub provider: String,
    #[serde(deserialize_with = "require_true")]
    pub confirmed: bool,
}
#[derive(Debug, Deserialize)]
pub struct RecommendationQuery {
    pub limit: u32,
    pub excluded_game_ids: Vec<String>,
}
#[derive(Debug, Deserialize)]
pub struct SetCollectionMembersRequest {
    pub collection_id: String,
    pub game_ids: Vec<String>,
}
#[derive(Debug, Deserialize)]
pub struct SetRecommendationPreferenceRequest {
    pub game_id: String,
    pub preference: RecommendationPreference,
    pub expires_at: Option<String>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsafe_flags_are_rejected() {
        assert!(
            serde_json::from_str::<super::super::models::ScanRootsRequest>(
                r#"{"roots":[],"follow_symlinks":true}"#
            )
            .is_err()
        );
        assert!(serde_json::from_str::<LaunchOptions>(r#"{"user_initiated":false}"#).is_err());
        assert!(serde_json::from_str::<LaunchOptions>(r#"{"user_initiated":true}"#).is_ok());
    }
}

#[derive(Debug, Deserialize)]
pub struct ReorderCollectionsRequest {
    pub collection_ids: Vec<String>,
}
