//! API response types.

use serde::Serialize;

/// Standard API response wrapper.
///
/// All API responses are wrapped in this type to provide a consistent
/// structure with status and either data or error information.
#[derive(Debug, Serialize)]
#[serde(tag = "status")]
pub enum ApiResponse<T: Serialize> {
    /// Successful response.
    #[serde(rename = "success")]
    Success {
        /// Response data.
        data: T,
    },

    /// Error response.
    #[serde(rename = "error")]
    Error {
        /// Error details.
        error: ApiError,
    },
}

/// API error details.
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// Error code (e.g., `SERVICE_NOT_FOUND`).
    pub code: String,

    /// Human-readable error message.
    pub message: String,

    /// Additional error details (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Creates a success response with the given data.
    pub fn success(data: T) -> Self {
        Self::Success { data }
    }
}

impl ApiResponse<()> {
    /// Creates an error response.
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            error: ApiError {
                code: code.into(),
                message: message.into(),
                details: None,
            },
        }
    }

    /// Creates an error response with additional details.
    pub fn error_with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self::Error {
            error: ApiError {
                code: code.into(),
                message: message.into(),
                details: Some(details),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize)]
    struct TestData {
        value: String,
    }

    #[test]
    fn test_success_response_serialization() {
        let response = ApiResponse::success(TestData {
            value: "test".to_string(),
        });

        let json = serde_json::to_string(&response).expect("should serialize");
        assert!(json.contains("\"status\":\"success\""));
        assert!(json.contains("\"value\":\"test\""));
    }

    #[test]
    fn test_error_response_serialization() {
        let response: ApiResponse<()> =
            ApiResponse::error("TEST_ERROR", "Something went wrong");

        let json = serde_json::to_string(&response).expect("should serialize");
        assert!(json.contains("\"status\":\"error\""));
        assert!(json.contains("\"code\":\"TEST_ERROR\""));
        assert!(json.contains("\"message\":\"Something went wrong\""));
    }

    #[test]
    fn test_error_response_with_details() {
        let response: ApiResponse<()> = ApiResponse::error_with_details(
            "VALIDATION_ERROR",
            "Invalid input",
            serde_json::json!({
                "field": "name",
                "reason": "too short"
            }),
        );

        let json = serde_json::to_string(&response).expect("should serialize");
        assert!(json.contains("\"details\""));
        assert!(json.contains("\"field\":\"name\""));
    }

    #[test]
    fn test_error_response_without_details() {
        let response: ApiResponse<()> = ApiResponse::error("ERROR", "message");

        let json = serde_json::to_string(&response).expect("should serialize");
        // details should be omitted entirely
        assert!(!json.contains("\"details\""));
    }
}
