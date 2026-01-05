//! Health monitoring API handlers.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::api::response::{ApiError, ApiResponse};
use crate::health::{HealthCheckResult, HealthStatus, ServiceHealth};
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

/// Response with health status for all services.
#[derive(Debug, Serialize)]
pub struct AllHealthResponse {
    /// Health states by service ID.
    pub services: HashMap<String, ServiceHealth>,

    /// Summary counts.
    pub summary: HealthSummary,
}

/// Health status summary.
#[derive(Debug, Serialize)]
pub struct HealthSummary {
    /// Number of healthy services.
    pub healthy: usize,

    /// Number of unhealthy services.
    pub unhealthy: usize,

    /// Number of services with unknown status.
    pub unknown: usize,

    /// Number of services starting up.
    pub starting: usize,

    /// Total number of services being monitored.
    pub total: usize,
}

/// GET `/v1/health/services`
///
/// Gets health status for all monitored services.
pub async fn get_all_health(State(state): State<AppState>) -> Json<ApiResponse<AllHealthResponse>> {
    let services = state.health_monitor.get_all_health().await;

    // Calculate summary
    let mut healthy = 0;
    let mut unhealthy = 0;
    let mut unknown = 0;
    let mut starting = 0;

    for health in services.values() {
        match health.status {
            HealthStatus::Healthy => healthy += 1,
            HealthStatus::Unhealthy => unhealthy += 1,
            HealthStatus::Unknown => unknown += 1,
            HealthStatus::Starting => starting += 1,
        }
    }

    let total = services.len();

    Json(ApiResponse::success(AllHealthResponse {
        services,
        summary: HealthSummary {
            healthy,
            unhealthy,
            unknown,
            starting,
            total,
        },
    }))
}

/// GET `/v1/services/{id}/health`
///
/// Gets health status for a specific service.
pub async fn get_service_health(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<ApiResponse<ServiceHealth>>) {
    match state.health_monitor.get_health(&id).await {
        Some(health) => (StatusCode::OK, Json(ApiResponse::success(health))),
        None => (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "SERVICE_NOT_MONITORED",
                format!("Service '{id}' is not being monitored for health"),
            )),
        ),
    }
}

/// POST `/v1/services/{id}/health/check`
///
/// Triggers an immediate health check for a service.
pub async fn check_service_health(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<ApiResponse<HealthCheckResult>>) {
    match state.health_monitor.check_now(&id).await {
        Some(result) => (StatusCode::OK, Json(ApiResponse::success(result))),
        None => (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "SERVICE_NOT_MONITORED",
                format!("Service '{id}' is not being monitored for health"),
            )),
        ),
    }
}

/// Response with proxy binding information.
#[derive(Debug, Serialize)]
pub struct ProxyBindingsResponse {
    /// Port bindings.
    pub bindings: Vec<PortBinding>,

    /// Total count.
    pub total: usize,
}

/// A single port binding.
#[derive(Debug, Serialize)]
pub struct PortBinding {
    /// Bound port.
    pub port: u16,

    /// Service ID.
    pub service_id: String,

    /// Service name.
    pub service_name: String,
}

/// GET `/v1/proxy/bindings`
///
/// Lists all proxy port bindings.
pub async fn get_proxy_bindings(
    State(state): State<AppState>,
) -> Json<ApiResponse<ProxyBindingsResponse>> {
    let bindings = state.proxy_server.list_bindings().await;

    let bindings: Vec<PortBinding> = bindings
        .into_iter()
        .map(|b| PortBinding {
            port: b.port,
            service_id: b.service_id.clone(),
            service_name: b.service_name.clone(),
        })
        .collect();

    let total = bindings.len();

    Json(ApiResponse::success(ProxyBindingsResponse { bindings, total }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_summary_serialization() {
        let summary = HealthSummary {
            healthy: 5,
            unhealthy: 1,
            unknown: 2,
            starting: 0,
            total: 8,
        };

        let json = serde_json::to_string(&summary).expect("should serialize");
        assert!(json.contains("\"healthy\":5"));
        assert!(json.contains("\"unhealthy\":1"));
        assert!(json.contains("\"total\":8"));
    }
}
