use crate::domain::ApiResponse;
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
pub struct HealthCheckRequest {
    pub request_id: String,
}
#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub version: String,
    pub platform: String,
}
#[tauri::command]
pub fn health_check(request: HealthCheckRequest) -> ApiResponse<HealthStatus> {
    if !crate::domain::protocol::valid_request_id(&request.request_id) {
        return ApiResponse::error(
            String::new(),
            crate::domain::ErrorCode::InvalidRequest,
            "请求标识无效。",
        );
    }
    ApiResponse::ok(
        request.request_id,
        HealthStatus {
            version: env!("CARGO_PKG_VERSION").into(),
            platform: std::env::consts::OS.into(),
        },
        "Rust 后端已连接",
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn health_rejects_unsafe_request_id() {
        let response = health_check(HealthCheckRequest {
            request_id: "unsafe\nvalue".into(),
        });
        assert!(!response.success);
        assert_eq!(
            response.error_code,
            Some(crate::domain::ErrorCode::InvalidRequest)
        );
        assert!(response.request_id.is_empty());
        assert!(response.data.is_none());
    }
    #[test]
    fn health_preserves_request_id() {
        let response = health_check(HealthCheckRequest {
            request_id: "test-request".into(),
        });
        assert!(response.success);
        assert_eq!(response.request_id, "test-request");
        assert!(response.error_code.is_none());
        assert_eq!(response.data.unwrap().version, env!("CARGO_PKG_VERSION"));
    }
}
mod local;
pub use local::*;

pub use crate::updates::*;
