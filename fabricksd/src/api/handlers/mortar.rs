//! Mortar project management API handlers.

use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};

use crate::api::response::ApiResponse;
use crate::error::DaemonError;
use crate::service::ServiceInfo;
use crate::state::AppState;

/// Request to deploy a mortar project.
#[derive(Debug, Deserialize)]
pub struct DeployMortarRequest {
    /// Path to the fabricks-mortar.toml file.
    pub mortar_path: PathBuf,
}

/// Response after deploying a mortar project.
#[derive(Debug, Serialize)]
pub struct DeployMortarResponse {
    /// Project name.
    pub project: String,

    /// List of created service IDs.
    pub service_ids: Vec<String>,

    /// Total number of services.
    pub total: usize,
}

/// Response with list of mortar projects.
#[derive(Debug, Serialize)]
pub struct ListProjectsResponse {
    /// List of project names.
    pub projects: Vec<String>,

    /// Total count.
    pub total: usize,
}

/// Response with list of services in a project.
#[derive(Debug, Serialize)]
pub struct ProjectServicesResponse {
    /// Project name.
    pub project: String,

    /// Services in the project.
    pub services: Vec<ServiceInfo>,

    /// Total count.
    pub total: usize,
}

/// POST `/v1/mortar/deploy`
///
/// Deploys a mortar project from a fabricks-mortar.toml file.
pub async fn deploy_mortar(
    State(state): State<AppState>,
    Json(req): Json<DeployMortarRequest>,
) -> Json<ApiResponse<DeployMortarResponse>> {
    let manager = state.service_manager.write().await;

    match manager.deploy_mortar(&req.mortar_path).await {
        Ok((project, service_ids)) => {
            let total = service_ids.len();
            Json(ApiResponse::success(DeployMortarResponse {
                project,
                service_ids,
                total,
            }))
        }
        Err(e) => Json(error_response(&e)),
    }
}

/// GET `/v1/mortar/projects`
///
/// Lists all mortar projects.
pub async fn list_projects(
    State(state): State<AppState>,
) -> Json<ApiResponse<ListProjectsResponse>> {
    let manager = state.service_manager.read().await;
    let projects = manager.list_mortar_projects().await;
    let total = projects.len();

    Json(ApiResponse::success(ListProjectsResponse {
        projects,
        total,
    }))
}

/// GET `/v1/mortar/projects/:name`
///
/// Gets services in a mortar project.
pub async fn get_project_services(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Json<ApiResponse<ProjectServicesResponse>> {
    let manager = state.service_manager.read().await;

    match manager.list_mortar_services(&name).await {
        Ok(services) => {
            let total = services.len();
            Json(ApiResponse::success(ProjectServicesResponse {
                project: name,
                services,
                total,
            }))
        }
        Err(e) => Json(error_response(&e)),
    }
}

/// DELETE `/v1/mortar/projects/:name`
///
/// Tears down a mortar project.
pub async fn teardown_mortar(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Json<ApiResponse<()>> {
    let manager = state.service_manager.write().await;

    match manager.teardown_mortar(&name).await {
        Ok(()) => Json(ApiResponse::Success { data: () }),
        Err(e) => Json(error_response(&e)),
    }
}

/// Converts a `DaemonError` to an API error response.
fn error_response<T: serde::Serialize>(err: &DaemonError) -> ApiResponse<T> {
    let (code, message) = match err {
        DaemonError::MortarProjectNotFound { name } => {
            ("PROJECT_NOT_FOUND", format!("Project not found: {name}"))
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
        DaemonError::WasmModuleNotFound { path } => {
            ("WASM_NOT_FOUND", format!("WASM module not found: {path}"))
        }
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
    fn test_error_response_project_not_found() {
        let err = DaemonError::MortarProjectNotFound {
            name: "my-app".to_string(),
        };
        let response: ApiResponse<()> = error_response(&err);

        match response {
            ApiResponse::Error { error } => {
                assert_eq!(error.code, "PROJECT_NOT_FOUND");
                assert!(error.message.contains("my-app"));
            }
            _ => panic!("Expected error response"),
        }
    }
}
