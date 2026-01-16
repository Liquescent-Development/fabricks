//! Volume management API handlers.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::api::response::{ApiError, ApiResponse};
use crate::state::AppState;
use crate::volume::{VolumeConfig, VolumeDetail, VolumeInfo};

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

/// Request to create a volume.
#[derive(Debug, Deserialize)]
pub struct CreateVolumeRequest {
    /// Volume name (must be unique).
    pub name: String,

    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,

    /// Optional size limit (e.g., "1Gi", "500Mi").
    #[serde(default)]
    pub size: Option<String>,
}

/// Response after creating a volume.
#[derive(Debug, Serialize)]
pub struct CreateVolumeResponse {
    /// Created volume ID.
    pub id: String,

    /// Volume name.
    pub name: String,
}

/// Response with list of volumes.
#[derive(Debug, Serialize)]
pub struct ListVolumesResponse {
    /// List of volumes.
    pub volumes: Vec<VolumeInfo>,

    /// Total count.
    pub total: usize,
}

/// POST `/v1/volumes`
///
/// Creates a new volume.
pub async fn create_volume(
    State(state): State<AppState>,
    Json(req): Json<CreateVolumeRequest>,
) -> (StatusCode, Json<ApiResponse<CreateVolumeResponse>>) {
    let mut config = VolumeConfig::new(req.name.clone());
    config.description = req.description;
    config.size = req.size;

    match state.volume_manager.create_volume(config).await {
        Ok(id) => (
            StatusCode::CREATED,
            Json(ApiResponse::success(CreateVolumeResponse {
                id,
                name: req.name,
            })),
        ),
        Err(e) => {
            let code = if e.to_string().contains("already exists") {
                "VOLUME_EXISTS"
            } else {
                "VOLUME_CREATE_FAILED"
            };
            (
                StatusCode::BAD_REQUEST,
                Json(typed_error(code, e.to_string())),
            )
        }
    }
}

/// GET `/v1/volumes`
///
/// Lists all volumes.
pub async fn list_volumes(State(state): State<AppState>) -> Json<ApiResponse<ListVolumesResponse>> {
    let volumes = state.volume_manager.list_volumes().await;
    let total = volumes.len();

    Json(ApiResponse::success(ListVolumesResponse { volumes, total }))
}

/// GET `/v1/volumes/{id}`
///
/// Gets details about a specific volume.
/// The path parameter can be either a volume ID or name.
pub async fn get_volume(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> (StatusCode, Json<ApiResponse<VolumeDetail>>) {
    match state
        .volume_manager
        .get_volume_by_id_or_name(&id_or_name)
        .await
    {
        Some(detail) => (StatusCode::OK, Json(ApiResponse::success(detail))),
        None => (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "VOLUME_NOT_FOUND",
                format!("Volume '{id_or_name}' not found"),
            )),
        ),
    }
}

/// DELETE `/v1/volumes/{id}`
///
/// Deletes a volume.
/// The path parameter can be either a volume ID or name.
pub async fn delete_volume(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    // Resolve ID or name to ID
    let Some(volume_id) = state.volume_manager.resolve_volume_id(&id_or_name).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "VOLUME_NOT_FOUND",
                format!("Volume '{id_or_name}' not found"),
            )),
        );
    };

    match state.volume_manager.delete_volume(&volume_id).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::success(()))),
        Err(e) => {
            let code = if e.to_string().contains("mounted") {
                "VOLUME_MOUNTED"
            } else {
                "VOLUME_DELETE_FAILED"
            };
            (
                StatusCode::BAD_REQUEST,
                Json(typed_error(code, e.to_string())),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_volume_request_minimal() {
        let json = r#"{"name": "test-vol"}"#;
        let req: CreateVolumeRequest = serde_json::from_str(json).expect("should parse");

        assert_eq!(req.name, "test-vol");
        assert!(req.description.is_none());
        assert!(req.size.is_none());
    }

    #[test]
    fn test_create_volume_request_full() {
        let json = r#"{
            "name": "data-vol",
            "description": "Database storage volume",
            "size": "10Gi"
        }"#;
        let req: CreateVolumeRequest = serde_json::from_str(json).expect("should parse");

        assert_eq!(req.name, "data-vol");
        assert_eq!(req.description, Some("Database storage volume".to_string()));
        assert_eq!(req.size, Some("10Gi".to_string()));
    }
}
