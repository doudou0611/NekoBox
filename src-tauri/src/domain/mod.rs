pub mod home;
pub mod models;
pub mod protocol;
pub mod requests;
pub use protocol::ErrorCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub error_code: Option<ErrorCode>,
    pub message: String,
    pub request_id: String,
    pub data: Option<T>,
}
impl<T> ApiResponse<T> {
    pub fn ok(request_id: String, data: T, message: &str) -> Self {
        Self {
            success: true,
            error_code: None,
            message: message.into(),
            request_id,
            data: Some(data),
        }
    }
    pub fn error(request_id: String, code: ErrorCode, message: &str) -> Self {
        Self {
            success: false,
            error_code: Some(code),
            message: message.into(),
            request_id,
            data: None,
        }
    }
}
#[derive(Debug, Deserialize)]
pub struct ApiRequest<T> {
    pub request_id: String,
    pub payload: T,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<T> {
    pub request_id: String,
    pub occurred_at: String,
    pub payload: T,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_envelopes_are_consistent() {
        let success = serde_json::to_value(ApiResponse::ok("r1".into(), 1, "ok")).unwrap();
        assert_eq!(success["success"], true);
        assert!(success["error_code"].is_null());
        assert_eq!(success["data"], 1);
        let failure = serde_json::to_value(ApiResponse::<()>::error(
            "r2".into(),
            ErrorCode::NotFound,
            "not found",
        ))
        .unwrap();
        assert_eq!(failure["success"], false);
        assert_eq!(failure["error_code"], "NOT_FOUND");
        assert!(failure["data"].is_null());
    }
}
