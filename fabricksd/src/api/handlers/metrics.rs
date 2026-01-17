//! Metrics API handlers.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;

use crate::api::response::{ApiError, ApiResponse};
use crate::scaler::{MetricsSummary, ServiceMetrics};
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

/// Response with metrics for all services.
#[derive(Debug, Serialize)]
pub struct AllMetricsResponse {
    /// Metrics for all services.
    pub summary: MetricsSummary,

    /// Total number of services.
    pub total: usize,
}

/// GET `/v1/metrics`
///
/// Gets metrics for all monitored services.
pub async fn get_all_metrics(State(state): State<AppState>) -> Json<ApiResponse<AllMetricsResponse>> {
    let services = state.metrics_collector.get_all_metrics().await;
    let total = services.len();

    let summary = MetricsSummary::new(services);

    Json(ApiResponse::success(AllMetricsResponse { summary, total }))
}

/// GET `/v1/services/{id}/metrics`
///
/// Gets metrics for a specific service.
pub async fn get_service_metrics(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<ApiResponse<ServiceMetrics>>) {
    match state.metrics_collector.get_metrics(&id).await {
        Some(metrics) => (StatusCode::OK, Json(ApiResponse::success(metrics))),
        None => (
            StatusCode::NOT_FOUND,
            Json(typed_error(
                "SERVICE_NOT_FOUND",
                format!("No metrics available for service '{id}'"),
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_metrics_response_serialization() {
        let response = AllMetricsResponse {
            summary: MetricsSummary::new(vec![]),
            total: 0,
        };

        let json = serde_json::to_string(&response).expect("should serialize");
        assert!(json.contains("\"total\":0"));
    }
}
