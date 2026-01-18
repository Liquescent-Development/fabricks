//! Policy API handlers.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;

use crate::api::response::{ApiError, ApiResponse};
use crate::policy::PolicyInfo;
use crate::state::AppState;

/// Creates a typed error response.
fn typed_error<T: Serialize>(error_type: &str, message: String) -> ApiResponse<T> {
    ApiResponse::Error {
        error: ApiError {
            code: error_type.to_string(),
            message,
            details: None,
        },
    }
}

/// Response with all policies.
#[derive(Debug, Serialize)]
pub struct AllPoliciesResponse {
    /// All loaded policies.
    pub policies: Vec<PolicyInfo>,

    /// Total number of policies.
    pub total: usize,
}

/// GET `/v1/policies`
///
/// Lists all loaded policies.
pub async fn list_policies(State(state): State<AppState>) -> Json<ApiResponse<AllPoliciesResponse>> {
    let policies = state.policy_manager.list_policies().await;
    let total = policies.len();

    Json(ApiResponse::success(AllPoliciesResponse { policies, total }))
}

/// GET `/v1/policies/{mortar_id}`
///
/// Gets a specific policy by mortar ID.
pub async fn get_policy(
    State(state): State<AppState>,
    Path(mortar_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<PolicyInfo>>) {
    match state.policy_manager.get_policy(&mortar_id).await {
        Some(policy) => (StatusCode::OK, Json(ApiResponse::success(PolicyInfo::from(&policy)))),
        None => (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "POLICY_NOT_FOUND",
                format!("No policy found for mortar project '{mortar_id}'"),
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_policies_response_serialization() {
        let response = AllPoliciesResponse {
            policies: vec![],
            total: 0,
        };

        let json = serde_json::to_string(&response).expect("should serialize");
        assert!(json.contains("\"total\":0"));
        assert!(json.contains("\"policies\":[]"));
    }
}
