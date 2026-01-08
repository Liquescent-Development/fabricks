//! Network management API handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::response::{ApiError, ApiResponse};
use crate::network::{NetworkConfig, NetworkDetail, NetworkInfo, NetworkOptions};
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

/// Request to create a network.
#[derive(Debug, Deserialize)]
pub struct CreateNetworkRequest {
    /// Network name (must be unique).
    pub name: String,

    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,

    /// Network options (access, isolation, encryption, audit).
    #[serde(flatten, default)]
    pub options: NetworkOptions,
}

/// Response after creating a network.
#[derive(Debug, Serialize)]
pub struct CreateNetworkResponse {
    /// Created network ID.
    pub id: String,

    /// Network name.
    pub name: String,
}

/// Response with list of networks.
#[derive(Debug, Serialize)]
pub struct ListNetworksResponse {
    /// List of networks.
    pub networks: Vec<NetworkInfo>,

    /// Total count.
    pub total: usize,
}

/// Request to add a service to a network.
#[derive(Debug, Deserialize)]
pub struct JoinNetworkRequest {
    /// Service ID to add.
    pub service_id: String,

    /// Service name for DNS resolution.
    pub service_name: String,
}

/// POST `/v1/networks`
///
/// Creates a new network.
pub async fn create_network(
    State(state): State<AppState>,
    Json(req): Json<CreateNetworkRequest>,
) -> (StatusCode, Json<ApiResponse<CreateNetworkResponse>>) {
    let config = if let Some(desc) = req.description {
        NetworkConfig::with_description(req.name.clone(), desc, req.options)
    } else {
        NetworkConfig::with_options(req.name.clone(), req.options)
    };

    match state.network_manager.create_network(config).await {
        Ok(id) => (
            StatusCode::CREATED,
            Json(ApiResponse::success(CreateNetworkResponse {
                id,
                name: req.name,
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(typed_error("NETWORK_CREATE_FAILED", e.to_string())),
        ),
    }
}

/// GET `/v1/networks`
///
/// Lists all networks.
pub async fn list_networks(
    State(state): State<AppState>,
) -> Json<ApiResponse<ListNetworksResponse>> {
    let networks = state.network_manager.list_networks().await;
    let total = networks.len();

    Json(ApiResponse::success(ListNetworksResponse { networks, total }))
}

/// GET `/v1/networks/{id}`
///
/// Gets details about a specific network.
/// The path parameter can be either a network ID or name.
pub async fn get_network(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> (StatusCode, Json<ApiResponse<NetworkDetail>>) {
    match state.network_manager.get_network_by_id_or_name(&id_or_name).await {
        Some(detail) => (StatusCode::OK, Json(ApiResponse::success(detail))),
        None => (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "NETWORK_NOT_FOUND",
                format!("Network '{id_or_name}' not found"),
            )),
        ),
    }
}

/// DELETE `/v1/networks/{id}`
///
/// Deletes a network.
/// The path parameter can be either a network ID or name.
pub async fn delete_network(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    // Resolve ID or name to ID
    let Some(network_id) = state.network_manager.resolve_network_id(&id_or_name).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "NETWORK_NOT_FOUND",
                format!("Network '{id_or_name}' not found"),
            )),
        );
    };

    match state.network_manager.delete_network(&network_id).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::success(()))),
        Err(e) => {
            let code = if e.to_string().contains("has members") {
                "NETWORK_HAS_MEMBERS"
            } else {
                "NETWORK_DELETE_FAILED"
            };
            (
                StatusCode::BAD_REQUEST,
                Json(typed_error(code, e.to_string())),
            )
        }
    }
}

/// POST `/v1/networks/{id}/join`
///
/// Adds a service to a network.
/// The path parameter can be either a network ID or name.
pub async fn join_network(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
    Json(req): Json<JoinNetworkRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    // Resolve ID or name to ID
    let Some(network_id) = state.network_manager.resolve_network_id(&id_or_name).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "NETWORK_NOT_FOUND",
                format!("Network '{id_or_name}' not found"),
            )),
        );
    };

    match state
        .network_manager
        .add_service(&network_id, &req.service_id, &req.service_name)
        .await
    {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::success(()))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(typed_error("NETWORK_JOIN_FAILED", e.to_string())),
        ),
    }
}

/// POST `/v1/networks/{id}/leave`
///
/// Removes a service from a network.
/// The path parameter can be either a network ID or name.
pub async fn leave_network(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
    Json(req): Json<LeaveNetworkRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    // Resolve ID or name to ID
    let Some(network_id) = state.network_manager.resolve_network_id(&id_or_name).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "NETWORK_NOT_FOUND",
                format!("Network '{id_or_name}' not found"),
            )),
        );
    };

    match state
        .network_manager
        .remove_service(&network_id, &req.service_id)
        .await
    {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::success(()))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(typed_error("NETWORK_LEAVE_FAILED", e.to_string())),
        ),
    }
}

/// Request to remove a service from a network.
#[derive(Debug, Deserialize)]
pub struct LeaveNetworkRequest {
    /// Service ID to remove.
    pub service_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{NetworkAccess, NetworkAudit, NetworkEncryption, NetworkIsolation};

    #[test]
    fn test_create_network_request_defaults() {
        let json = r#"{"name": "test-net"}"#;
        let req: CreateNetworkRequest = serde_json::from_str(json).expect("should parse");

        assert_eq!(req.name, "test-net");
        assert!(req.description.is_none());
        // Default options
        assert_eq!(req.options.access, NetworkAccess::External);
        assert_eq!(req.options.isolation, NetworkIsolation::Connected);
        assert_eq!(req.options.encryption, NetworkEncryption::Optional);
        assert_eq!(req.options.audit, NetworkAudit::Disabled);
    }

    #[test]
    fn test_create_network_request_full() {
        let json = r#"{
            "name": "secure-net",
            "description": "A secure network",
            "access": "internal",
            "isolation": "isolated",
            "encryption": "required",
            "audit": "enabled"
        }"#;
        let req: CreateNetworkRequest = serde_json::from_str(json).expect("should parse");

        assert_eq!(req.name, "secure-net");
        assert_eq!(req.description, Some("A secure network".to_string()));
        assert_eq!(req.options.access, NetworkAccess::Internal);
        assert_eq!(req.options.isolation, NetworkIsolation::Isolated);
        assert_eq!(req.options.encryption, NetworkEncryption::Required);
        assert_eq!(req.options.audit, NetworkAudit::Enabled);
    }
}
