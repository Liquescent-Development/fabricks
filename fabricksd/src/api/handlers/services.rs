//! Service management API handlers.

use std::path::PathBuf;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::response::ApiResponse;
use crate::error::DaemonError;
use crate::service::{ServiceConfig, ServiceDetail, ServiceInfo};
use crate::state::AppState;

/// Request to create a service.
#[derive(Debug, Deserialize)]
pub struct CreateServiceRequest {
    /// Service name.
    pub name: String,

    /// Service version.
    pub version: String,

    /// Path to the WASM module.
    pub wasm_path: PathBuf,

    /// Environment variables (optional).
    #[serde(default)]
    pub environment: std::collections::HashMap<String, String>,

    /// Command-line arguments (optional).
    #[serde(default)]
    pub args: Vec<String>,
}

/// Response after creating a service.
#[derive(Debug, Serialize)]
pub struct CreateServiceResponse {
    /// Created service ID.
    pub id: String,

    /// Service name.
    pub name: String,
}

/// Request to run a Fabrickfile.
#[derive(Debug, Deserialize)]
pub struct RunFabrickfileRequest {
    /// Path to the Fabrickfile.
    pub fabrickfile_path: PathBuf,

    /// Optional path to pre-built WASM.
    #[serde(default)]
    pub wasm_path: Option<PathBuf>,
}

/// Request to scale a service.
#[derive(Debug, Deserialize)]
pub struct ScaleServiceRequest {
    /// Target number of replicas.
    pub replicas: usize,
}

/// Response with list of services.
#[derive(Debug, Serialize)]
pub struct ListServicesResponse {
    /// List of services.
    pub services: Vec<ServiceInfo>,

    /// Total count.
    pub total: usize,
}

/// POST `/v1/services`
///
/// Creates a new service from a configuration.
pub async fn create_service(
    State(state): State<AppState>,
    Json(req): Json<CreateServiceRequest>,
) -> Json<ApiResponse<CreateServiceResponse>> {
    // Compute digest
    let Ok(wasm_bytes) = tokio::fs::read(&req.wasm_path).await else {
        return Json(error_response(&DaemonError::WasmModuleNotFound {
            path: req.wasm_path.display().to_string(),
        }));
    };

    let digest = compute_digest(&wasm_bytes);

    let config = ServiceConfig::new(
        req.name.clone(),
        req.version,
        req.wasm_path,
        digest,
    );

    let manager = state.service_manager.write().await;

    match manager.create_service(config).await {
        Ok(id) => Json(ApiResponse::success(CreateServiceResponse {
            id,
            name: req.name,
        })),
        Err(e) => Json(error_response(&e)),
    }
}

/// POST `/v1/services/run`
///
/// Runs a Fabrickfile (creates and starts a service).
pub async fn run_fabrickfile(
    State(state): State<AppState>,
    Json(req): Json<RunFabrickfileRequest>,
) -> Json<ApiResponse<CreateServiceResponse>> {
    let manager = state.service_manager.write().await;

    match manager
        .run_fabrickfile(&req.fabrickfile_path, req.wasm_path.as_deref())
        .await
    {
        Ok(id) => {
            // Get service info to return name
            match manager.get_service(&id).await {
                Ok(detail) => Json(ApiResponse::success(CreateServiceResponse {
                    id,
                    name: detail.name,
                })),
                Err(_) => Json(ApiResponse::success(CreateServiceResponse {
                    id,
                    name: "unknown".to_string(),
                })),
            }
        }
        Err(e) => Json(error_response(&e)),
    }
}

/// GET `/v1/services`
///
/// Lists all services.
pub async fn list_services(State(state): State<AppState>) -> Json<ApiResponse<ListServicesResponse>> {
    let manager = state.service_manager.read().await;
    let services = manager.list_services().await;
    let total = services.len();

    Json(ApiResponse::success(ListServicesResponse { services, total }))
}

/// GET `/v1/services/:id`
///
/// Gets detailed information about a service.
/// The path parameter can be either a service ID or name.
pub async fn get_service(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Json<ApiResponse<ServiceDetail>> {
    let manager = state.service_manager.read().await;

    match manager.get_service_by_id_or_name(&id_or_name).await {
        Ok(detail) => Json(ApiResponse::success(detail)),
        Err(e) => Json(error_response(&e)),
    }
}

/// POST `/v1/services/:id/start`
///
/// Starts a service.
pub async fn start_service(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let manager = state.service_manager.write().await;

    match manager.start_service(&id).await {
        Ok(()) => Json(ApiResponse::Success { data: () }),
        Err(e) => Json(error_response(&e)),
    }
}

/// POST `/v1/services/:id/stop`
///
/// Stops a service.
pub async fn stop_service(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let manager = state.service_manager.write().await;

    match manager.stop_service(&id).await {
        Ok(()) => Json(ApiResponse::Success { data: () }),
        Err(e) => Json(error_response(&e)),
    }
}

/// POST `/v1/services/:id/scale`
///
/// Scales a service to a target number of replicas.
pub async fn scale_service(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ScaleServiceRequest>,
) -> Json<ApiResponse<()>> {
    let manager = state.service_manager.write().await;

    match manager.scale_service(&id, req.replicas).await {
        Ok(()) => Json(ApiResponse::Success { data: () }),
        Err(e) => Json(error_response(&e)),
    }
}

/// DELETE `/v1/services/:id`
///
/// Deletes a service.
pub async fn delete_service(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let manager = state.service_manager.write().await;

    match manager.delete_service(&id).await {
        Ok(()) => Json(ApiResponse::Success { data: () }),
        Err(e) => Json(error_response(&e)),
    }
}

/// Computes SHA256 digest of bytes.
fn compute_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    format!("sha256:{}", hex::encode(result))
}

/// Converts a `DaemonError` to an API error response.
fn error_response<T: serde::Serialize>(err: &DaemonError) -> ApiResponse<T> {
    let (code, message) = match err {
        DaemonError::ServiceNotFound { id } => ("SERVICE_NOT_FOUND", format!("Service not found: {id}")),
        DaemonError::ServiceAlreadyExists { name } => {
            ("SERVICE_EXISTS", format!("Service already exists: {name}"))
        }
        DaemonError::ServiceNotRunning { id } => {
            ("SERVICE_NOT_RUNNING", format!("Service is not running: {id}"))
        }
        DaemonError::InvalidStateTransition { from, to } => (
            "INVALID_STATE",
            format!("Cannot transition from '{from}' to '{to}'"),
        ),
        DaemonError::WasmModuleNotFound { path } => {
            ("WASM_NOT_FOUND", format!("WASM module not found: {path}"))
        }
        DaemonError::FabrickfileParseError(msg) => ("PARSE_ERROR", format!("Parse error: {msg}")),
        DaemonError::CircularDependency => (
            "CIRCULAR_DEPENDENCY",
            "Circular dependency detected".to_string(),
        ),
        DaemonError::DependencyNotFound {
            service,
            dependency,
        } => (
            "DEPENDENCY_NOT_FOUND",
            format!("Service '{service}' depends on '{dependency}' which does not exist"),
        ),
        DaemonError::RuntimeError(e) => ("RUNTIME_ERROR", format!("Runtime error: {e}")),
        DaemonError::BuildError(msg) => ("BUILD_ERROR", format!("Build error: {msg}")),
        _ => ("INTERNAL_ERROR", err.to_string()),
    };

    ApiResponse::Error {
        error: crate::api::response::ApiError {
            code: code.to_string(),
            message,
            details: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_digest() {
        let bytes = b"hello world";
        let digest = compute_digest(bytes);
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), 7 + 64); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_error_response_service_not_found() {
        let err = DaemonError::ServiceNotFound {
            id: "svc-123".to_string(),
        };
        let response: ApiResponse<()> = error_response(&err);

        match response {
            ApiResponse::Error { error } => {
                assert_eq!(error.code, "SERVICE_NOT_FOUND");
                assert!(error.message.contains("svc-123"));
            }
            _ => panic!("Expected error response"),
        }
    }

    #[test]
    fn test_error_response_circular_dependency() {
        let err = DaemonError::CircularDependency;
        let response: ApiResponse<()> = error_response(&err);

        match response {
            ApiResponse::Error { error } => {
                assert_eq!(error.code, "CIRCULAR_DEPENDENCY");
            }
            _ => panic!("Expected error response"),
        }
    }
}
