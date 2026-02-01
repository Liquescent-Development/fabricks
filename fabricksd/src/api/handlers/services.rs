//! Service management API handlers.

use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};

use crate::api::response::ApiResponse;
use crate::error::DaemonError;
use crate::service::{NetworkAttachment, ServiceConfig, ServiceDetail, ServiceInfo};
use crate::state::AppState;
use crate::volume::VolumeMount;
use fabricks_common::Fabrickfile;

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

/// Request to run a module by tag.
#[derive(Debug, Deserialize)]
pub struct RunModuleRequest {
    /// Module reference (tag like "my-module:1.0.0").
    pub reference: String,

    /// Additional arguments to pass to the module.
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variable overrides.
    #[serde(default)]
    pub env_vars: Vec<(String, String)>,

    /// Whether to disable capability enforcement.
    #[serde(default)]
    pub no_capabilities: bool,

    /// Networks to join the service to (by name).
    #[serde(default)]
    pub networks: Vec<String>,
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

    let config = ServiceConfig::new(req.name.clone(), req.version, req.wasm_path, digest);

    let manager = state.service_manager.write().await;

    match manager.create_service(config).await {
        Ok(id) => Json(ApiResponse::success(CreateServiceResponse {
            id,
            name: req.name,
        })),
        Err(e) => Json(error_response(&e)),
    }
}

/// POST `/v1/services/run-module`
///
/// Runs a module from OCI storage by tag/reference.
/// This loads the module from local OCI storage, creates a service, and starts it.
pub async fn run_module(
    State(state): State<AppState>,
    Json(req): Json<RunModuleRequest>,
) -> Json<ApiResponse<CreateServiceResponse>> {
    // Load module from OCI storage
    let module = match state.oci_storage.load_module(&req.reference).await {
        Ok(module) => module,
        Err(e) => {
            return Json(error_response(&DaemonError::ModuleNotFound {
                reference: format!("{}: {e}", req.reference),
            }));
        }
    };

    // Get the config (Fabrickfile) from the module
    let fabrickfile = module.config();

    // Determine if this is an interpreted module (has runtime + source, no module layer)
    let is_interpreted = module.has_runtime_layer() && module.has_source_layers();

    // Get WASM bytes - for interpreted runtimes, use the runtime layer
    let wasm_bytes = if is_interpreted {
        match module.runtime_layer() {
            Some(layer) => layer.data.as_slice(),
            None => {
                return Json(error_response(&DaemonError::OciStorageError(
                    "interpreted module missing runtime layer".to_string(),
                )));
            }
        }
    } else {
        module.wasm_bytes()
    };

    // Create a temp directory for module files
    let temp_dir = std::env::temp_dir().join("fabricks-modules");
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        return Json(error_response(&DaemonError::IoError(e)));
    }

    let wasm_path = temp_dir.join(format!("{}.wasm", fabrickfile.info.name));
    if let Err(e) = tokio::fs::write(&wasm_path, wasm_bytes).await {
        return Json(error_response(&DaemonError::IoError(e)));
    }

    // For interpreted modules, extract source layers to /app mount point
    let source_mount_path = if is_interpreted {
        let app_dir = temp_dir.join(format!("{}-app", fabrickfile.info.name));
        if let Err(e) = tokio::fs::create_dir_all(&app_dir).await {
            return Json(error_response(&DaemonError::IoError(e)));
        }

        // Extract each source layer (they stack, later overrides earlier)
        for source_layer in module.source_layers() {
            if let Err(e) = extract_tar_gz(&source_layer.data, &app_dir) {
                return Json(error_response(&DaemonError::OciStorageError(format!(
                    "failed to extract source layer: {e}"
                ))));
            }
        }

        Some(app_dir)
    } else {
        None
    };

    // Build service config
    let config = build_service_config(
        fabrickfile,
        wasm_path,
        wasm_bytes,
        source_mount_path,
        &req,
    );

    let manager = state.service_manager.write().await;

    // Validate requested networks exist before creating the service
    if !req.networks.is_empty() {
        for network_name in &req.networks {
            if state
                .network_manager
                .get_network_by_name(network_name)
                .await
                .is_none()
            {
                return Json(error_response(&DaemonError::NetworkNotFound(
                    network_name.clone(),
                )));
            }
        }
    }

    // Create the service
    let service_name = fabrickfile.info.name.clone();
    match manager.create_service(config).await {
        Ok(id) => {
            // Join requested networks
            for network_name in &req.networks {
                if let Some(network) = state
                    .network_manager
                    .get_network_by_name(network_name)
                    .await
                    && let Err(e) = state
                        .network_manager
                        .add_service(&network.id, &id, &service_name)
                        .await
                {
                    return Json(error_response(&e));
                }
            }

            // Start the service
            if let Err(e) = manager.start_service(&id).await {
                return Json(error_response(&e));
            }

            Json(ApiResponse::success(CreateServiceResponse {
                id,
                name: service_name,
            }))
        }
        Err(e) => Json(error_response(&e)),
    }
}

/// GET `/v1/services`
///
/// Lists all services.
pub async fn list_services(
    State(state): State<AppState>,
) -> Json<ApiResponse<ListServicesResponse>> {
    let manager = state.service_manager.read().await;
    let services = manager.list_services().await;
    let total = services.len();

    Json(ApiResponse::success(ListServicesResponse {
        services,
        total,
    }))
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
        Ok(mut detail) => {
            // Populate port bindings from proxy server
            let bindings = state.proxy_server.list_bindings().await;
            detail.ports = bindings
                .into_iter()
                .filter(|b| b.service_id == detail.id)
                .map(|b| b.port)
                .collect();

            // Populate network attachments from network manager
            let network_ids = state.network_manager.get_service_networks(&detail.id).await;
            for network_id in network_ids {
                if let Some(network) = state.network_manager.get_network(&network_id).await {
                    detail.networks.push(NetworkAttachment {
                        id: network.id,
                        name: network.name,
                        internal: network.options.access.is_internal(),
                    });
                }
            }

            Json(ApiResponse::success(detail))
        }
        Err(e) => Json(error_response(&e)),
    }
}

/// POST `/v1/services/:id/start`
///
/// Starts a service. The path parameter can be either a service ID or name.
pub async fn start_service(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Json<ApiResponse<()>> {
    let manager = state.service_manager.write().await;

    // Resolve name to ID if needed
    let id = match resolve_service_id(&manager, &id_or_name).await {
        Ok(id) => id,
        Err(e) => return Json(error_response(&e)),
    };

    match manager.start_service(&id).await {
        Ok(()) => Json(ApiResponse::Success { data: () }),
        Err(e) => Json(error_response(&e)),
    }
}

/// POST `/v1/services/:id/stop`
///
/// Stops a service. The path parameter can be either a service ID or name.
pub async fn stop_service(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Json<ApiResponse<()>> {
    let manager = state.service_manager.write().await;

    // Resolve name to ID if needed
    let id = match resolve_service_id(&manager, &id_or_name).await {
        Ok(id) => id,
        Err(e) => return Json(error_response(&e)),
    };

    match manager.stop_service(&id).await {
        Ok(()) => Json(ApiResponse::Success { data: () }),
        Err(e) => Json(error_response(&e)),
    }
}

/// POST `/v1/services/:id/scale`
///
/// Scales a service to a target number of replicas.
/// The path parameter can be either a service ID or name.
pub async fn scale_service(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
    Json(req): Json<ScaleServiceRequest>,
) -> Json<ApiResponse<()>> {
    let manager = state.service_manager.write().await;

    // Resolve name to ID if needed
    let id = match resolve_service_id(&manager, &id_or_name).await {
        Ok(id) => id,
        Err(e) => return Json(error_response(&e)),
    };

    match manager.scale_service(&id, req.replicas).await {
        Ok(()) => Json(ApiResponse::Success { data: () }),
        Err(e) => Json(error_response(&e)),
    }
}

/// DELETE `/v1/services/:id`
///
/// Deletes a service. The path parameter can be either a service ID or name.
pub async fn delete_service(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Json<ApiResponse<()>> {
    let manager = state.service_manager.write().await;

    // Resolve name to ID if needed
    let id = match resolve_service_id(&manager, &id_or_name).await {
        Ok(id) => id,
        Err(e) => return Json(error_response(&e)),
    };

    match manager.delete_service(&id).await {
        Ok(()) => Json(ApiResponse::Success { data: () }),
        Err(e) => Json(error_response(&e)),
    }
}

/// Resolves a service ID or name to an ID.
async fn resolve_service_id(
    manager: &crate::service::ServiceManager,
    id_or_name: &str,
) -> Result<String, DaemonError> {
    // If it looks like an ID (starts with "svc-"), use it directly
    if id_or_name.starts_with("svc-") {
        return Ok(id_or_name.to_string());
    }

    // Otherwise, look up by name
    let detail = manager.get_service_by_id_or_name(id_or_name).await?;
    Ok(detail.id)
}

/// Computes SHA256 digest of bytes.
fn compute_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    format!("sha256:{}", hex::encode(result))
}

/// Extracts a gzipped tar archive to a directory.
fn extract_tar_gz(data: &[u8], dest: &std::path::Path) -> std::io::Result<()> {
    use std::io::Cursor;
    use flate2::read::GzDecoder;
    use tar::Archive;

    let cursor = Cursor::new(data);
    let decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    // Extract to destination directory
    archive.unpack(dest)?;

    Ok(())
}

/// Builds a service configuration from a Fabrickfile and module data.
fn build_service_config(
    fabrickfile: &Fabrickfile,
    wasm_path: PathBuf,
    wasm_bytes: &[u8],
    source_mount_path: Option<PathBuf>,
    req: &RunModuleRequest,
) -> ServiceConfig {
    let digest = compute_digest(wasm_bytes);

    // Merge environment variables from Fabrickfile and request
    let mut environment: std::collections::HashMap<String, String> = fabrickfile
        .config
        .as_ref()
        .and_then(|c| c.environment.clone())
        .unwrap_or_default();
    for (key, value) in &req.env_vars {
        environment.insert(key.clone(), value.clone());
    }

    // Add /app volume mount for interpreted modules (read-only source)
    let volumes = source_mount_path
        .map(|path| {
            vec![VolumeMount::read_only(
                "app-source".to_string(),
                "app-source".to_string(),
                path,
                "/app".to_string(),
            )]
        })
        .unwrap_or_default();

    ServiceConfig {
        name: fabrickfile.info.name.clone(),
        version: fabrickfile.info.version.clone(),
        service_type: fabrickfile.info.service_type,
        wasm_path,
        wasm_digest: digest,
        capabilities: if req.no_capabilities {
            fabricks_common::Capabilities::default()
        } else {
            fabrickfile.capabilities.clone()
        },
        environment,
        args: req.args.clone(),
        resources: fabrickfile.config.as_ref().and_then(|c| c.resources.clone()),
        replicas: fabricks_common::models::Replicas::default(),
        health_check: fabrickfile.health_check.clone(),
        depends_on: Vec::new(),
        networks: Vec::new(),
        volumes,
        mortar_project: None,
    }
}

/// Converts a `DaemonError` to an API error response.
fn error_response<T: serde::Serialize>(err: &DaemonError) -> ApiResponse<T> {
    let (code, message) = match err {
        DaemonError::ServiceNotFound { id } => {
            ("SERVICE_NOT_FOUND", format!("Service not found: {id}"))
        }
        DaemonError::ServiceAlreadyExists { name } => {
            ("SERVICE_EXISTS", format!("Service already exists: {name}"))
        }
        DaemonError::ServiceNotRunning { id } => (
            "SERVICE_NOT_RUNNING",
            format!("Service is not running: {id}"),
        ),
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
        DaemonError::ModuleNotFound { reference } => {
            ("MODULE_NOT_FOUND", format!("Module not found: {reference}"))
        }
        DaemonError::OciStorageError(msg) => ("STORAGE_ERROR", format!("Storage error: {msg}")),
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

/// Query parameters for the logs endpoint.
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// Return only the last N log entries.
    pub tail: Option<usize>,
}

/// Response from the logs endpoint.
#[derive(Debug, Serialize)]
pub struct ServiceLogsResponse {
    /// Service ID.
    pub id: String,
    /// Log entries.
    pub entries: Vec<crate::service::LogEntry>,
    /// Total number of entries returned.
    pub count: usize,
}

/// GET `/v1/services/{id}/logs`
///
/// Returns captured stdout/stderr log entries for a service.
pub async fn get_service_logs(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
    axum::extract::Query(query): axum::extract::Query<LogsQuery>,
) -> Json<ApiResponse<ServiceLogsResponse>> {
    let manager = state.service_manager.read().await;

    match manager.get_service_logs(&id_or_name, query.tail).await {
        Ok((id, entries)) => {
            let count = entries.len();
            Json(ApiResponse::success(ServiceLogsResponse {
                id,
                entries,
                count,
            }))
        }
        Err(e) => Json(error_response(&e)),
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
