use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidRequest,
    NotFound,
    PermissionDenied,
    PathInvalid,
    Conflict,
    Cancelled,
    NetworkUnavailable,
    RateLimited,
    DatabaseError,
    InternalError,
    NotImplemented,
    DesktopUnavailable,
    InvalidResponse,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameStatus {
    NotStarted,
    Playing,
    Paused,
    Completed,
    Dropped,
    PendingConfirmation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallSource {
    Local,
    Steam,
    Manual,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataStatus {
    LocalOnly,
    Queued,
    Syncing,
    Synced,
    Failed,
    PendingConfirmation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

pub const EVENTS: [(&str, &str); 6] = [
    ("scan_progress", "scan:progress"),
    ("metadata_progress", "metadata:progress"),
    ("session_started", "playtime:session-started"),
    ("session_ended", "playtime:session-ended"),
    ("backup_created", "save:backup-created"),
    ("download_progress", "download:progress"),
];
pub const PLANNED_COMMANDS: [&str; 0] = [];
pub const IMPLEMENTED_COMMANDS: [&str; 112] = [
    "cache_remote_image",
    "open_bangumi_login",
    "start_metadata_refresh",
    "get_metadata_refresh",
    "cancel_metadata_refresh",
    "get_backup_settings",
    "save_backup_settings",
    "create_application_backup",
    "list_application_backups",
    "delete_application_backup",
    "preview_application_restore",
    "confirm_application_restore",
    "cancel_application_restore",
    "get_application_backup_status",
    "test_webdav",
    "list_webdav_backups",
    "upload_webdav_backup",
    "download_webdav_backup",
    "get_app_settings",
    "save_app_settings",
    "cancel_metadata_search",
    "hikarinagi_account",
    "begin_hikarinagi_login",
    "poll_hikarinagi_login",
    "cancel_hikarinagi_login",
    "logout_hikarinagi",
    "get_metadata_sources",
    "save_metadata_sources",
    "get_vndb_settings",
    "save_vndb_settings",
    "test_vndb_connection",
    "sync_account_play_data",
    "health_check",
    "scan_roots",
    "get_scan_task",
    "list_games",
    "get_game",
    "launch_game",
    "get_home_summary",
    "get_recommendations",
    "list_collections",
    "reorder_collections",
    "save_collection",
    "set_collection_members",
    "set_recommendation_preference",
    "list_recommendation_preferences",
    "backend_status",
    "export_database",
    "preview_database_import",
    "confirm_database_import",
    "cancel_database_import",
    "database_transfer_status",
    "import_game",
    "preview_import",
    "update_game",
    "update_game_metadata",
    "set_metadata_lock",
    "list_game_processes",
    "select_game_process",
    "remove_game",
    "get_installation",
    "configure_installation",
    "control_scan_task",
    "get_collection",
    "delete_collection",
    "search_metadata",
    "confirm_metadata_match",
    "begin_import_batch",
    "cancel_import_batch",
    "prepare_import_metadata",
    "discard_import_metadata",
    "import_prepared_game",
    "get_translation_settings",
    "get_hikarinagi_settings",
    "get_hikarinagi_rates",
    "get_hikarinagi_review",
    "submit_hikarinagi_review",
    "save_hikarinagi_settings",
    "test_hikarinagi_connection",
    "save_translation_settings",
    "test_translation",
    "unbind_metadata",
    "bangumi_account",
    "login_bangumi",
    "logout_bangumi",
    "get_bangumi_cover_status",
    "retry_bangumi_cover",
    "list_play_sessions",
    "correct_play_session",
    "get_playtime_stats",
    "get_activity_snapshot",
    "detect_save_paths",
    "list_save_profiles",
    "configure_save_profile",
    "delete_save_profile",
    "list_save_snapshots",
    "create_save_snapshot",
    "preview_save_restore",
    "restore_save_snapshot",
    "delete_save_snapshot",
    "export_screenshots",
    "delete_screenshots",
    "scan_screenshots",
    "list_screenshots",
    "update_screenshot",
    "list_notes",
    "save_note",
    "delete_note",
    "replace_game_tags",
    "list_external_sources",
    "bind_external_source",
    "open_external_source",
];
pub fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
#[cfg(test)]
mod tests {
    use super::*;
    fn protocol() -> serde_json::Value {
        serde_json::from_str(include_str!("../../../shared/protocol.json")).unwrap()
    }
    fn assert_values<T: Serialize>(key: &str, values: &[T]) {
        assert_eq!(serde_json::to_value(values).unwrap(), protocol()[key]);
    }
    #[test]
    fn enum_names_match_central_protocol() {
        assert_values(
            "error_codes",
            &[
                ErrorCode::InvalidRequest,
                ErrorCode::NotFound,
                ErrorCode::PermissionDenied,
                ErrorCode::PathInvalid,
                ErrorCode::Conflict,
                ErrorCode::Cancelled,
                ErrorCode::NetworkUnavailable,
                ErrorCode::RateLimited,
                ErrorCode::DatabaseError,
                ErrorCode::InternalError,
                ErrorCode::NotImplemented,
                ErrorCode::DesktopUnavailable,
                ErrorCode::InvalidResponse,
            ],
        );
        assert_values(
            "game_statuses",
            &[
                GameStatus::NotStarted,
                GameStatus::Playing,
                GameStatus::Paused,
                GameStatus::Completed,
                GameStatus::Dropped,
                GameStatus::PendingConfirmation,
            ],
        );
        assert_values(
            "install_sources",
            &[
                InstallSource::Local,
                InstallSource::Steam,
                InstallSource::Manual,
                InstallSource::Unknown,
            ],
        );
        assert_values(
            "metadata_statuses",
            &[
                MetadataStatus::LocalOnly,
                MetadataStatus::Queued,
                MetadataStatus::Syncing,
                MetadataStatus::Synced,
                MetadataStatus::Failed,
                MetadataStatus::PendingConfirmation,
            ],
        );
        assert_values(
            "task_statuses",
            &[
                TaskStatus::Queued,
                TaskStatus::Running,
                TaskStatus::Paused,
                TaskStatus::Completed,
                TaskStatus::Cancelled,
                TaskStatus::Failed,
            ],
        );
    }
    #[test]
    fn events_match_central_protocol() {
        let manifest = protocol();
        for (key, name) in EVENTS {
            assert_eq!(manifest["events"][key], name);
        }
        assert_eq!(manifest["events"].as_object().unwrap().len(), EVENTS.len());
    }
    #[test]
    fn registered_commands_match_implementation_manifest() {
        let manifest = protocol();
        for command in IMPLEMENTED_COMMANDS {
            assert_eq!(manifest["commands"][command], "implemented");
            assert!(include_str!("../lib.rs").contains(&format!("commands::{command}")));
        }
        for command in PLANNED_COMMANDS {
            assert_eq!(manifest["commands"][command], "planned");
        }
        assert_eq!(
            manifest["commands"].as_object().unwrap().len(),
            PLANNED_COMMANDS.len() + IMPLEMENTED_COMMANDS.len()
        );
    }
    #[test]
    fn rejects_unsafe_request_identifiers() {
        assert!(valid_request_id("request-1_test"));
        assert!(valid_request_id(&"a".repeat(128)));
        for invalid in ["", "a\nb", "用户", "../path", &"a".repeat(129)] {
            assert!(!valid_request_id(invalid));
        }
    }
}
