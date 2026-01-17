//! Axum router configuration.

use axum::{
    Router,
    routing::{delete, get, post},
};
use tower_http::trace::TraceLayer;

use super::handlers;
use crate::state::AppState;

/// Builds the complete API router.
///
/// This configures all API routes and applies middleware.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Daemon management
        .route("/v1/daemon/info", get(handlers::daemon::daemon_info))
        // Simple health check for liveness probes
        .route("/v1/health", get(health_check))
        // Service management
        .route("/v1/services", get(handlers::services::list_services))
        .route("/v1/services", post(handlers::services::create_service))
        .route(
            "/v1/services/run",
            post(handlers::services::run_fabrickfile),
        )
        .route("/v1/services/{id}", get(handlers::services::get_service))
        .route(
            "/v1/services/{id}",
            delete(handlers::services::delete_service),
        )
        .route(
            "/v1/services/{id}/start",
            post(handlers::services::start_service),
        )
        .route(
            "/v1/services/{id}/stop",
            post(handlers::services::stop_service),
        )
        .route(
            "/v1/services/{id}/scale",
            post(handlers::services::scale_service),
        )
        // Mortar project management
        .route("/v1/mortar/deploy", post(handlers::mortar::deploy_mortar))
        .route("/v1/mortar/projects", get(handlers::mortar::list_projects))
        .route(
            "/v1/mortar/projects/{name}",
            get(handlers::mortar::get_project_services),
        )
        .route(
            "/v1/mortar/projects/{name}",
            delete(handlers::mortar::teardown_mortar),
        )
        // Network management
        .route("/v1/networks", post(handlers::networks::create_network))
        .route("/v1/networks", get(handlers::networks::list_networks))
        .route("/v1/networks/{id}", get(handlers::networks::get_network))
        .route(
            "/v1/networks/{id}",
            delete(handlers::networks::delete_network),
        )
        .route(
            "/v1/networks/{id}/join",
            post(handlers::networks::join_network),
        )
        .route(
            "/v1/networks/{id}/leave",
            post(handlers::networks::leave_network),
        )
        // Volume management
        .route("/v1/volumes", post(handlers::volumes::create_volume))
        .route("/v1/volumes", get(handlers::volumes::list_volumes))
        .route("/v1/volumes/{id}", get(handlers::volumes::get_volume))
        .route("/v1/volumes/{id}", delete(handlers::volumes::delete_volume))
        // Health monitoring
        .route("/v1/health/services", get(handlers::health::get_all_health))
        .route(
            "/v1/services/{id}/health",
            get(handlers::health::get_service_health),
        )
        .route(
            "/v1/services/{id}/health/check",
            post(handlers::health::check_service_health),
        )
        // Proxy information
        .route(
            "/v1/proxy/bindings",
            get(handlers::health::get_proxy_bindings),
        )
        // Metrics
        .route("/v1/metrics", get(handlers::metrics::get_all_metrics))
        .route(
            "/v1/services/{id}/metrics",
            get(handlers::metrics::get_service_metrics),
        )
        // Add state
        .with_state(state)
        // Add tracing layer for request/response logging
        .layer(TraceLayer::new_for_http())
}

/// Simple health check endpoint.
///
/// GET `/v1/health`
///
/// Returns "ok" if the daemon is running.
async fn health_check() -> &'static str {
    "ok"
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tempfile::tempdir;
    use tower::ServiceExt;

    use crate::config::DaemonConfig;

    fn create_test_state() -> AppState {
        let dir = tempdir().expect("should create temp dir");
        let mut config = DaemonConfig::default();
        config.daemon.data_dir = dir.keep();
        AppState::new(config).expect("should create state")
    }

    #[tokio::test]
    async fn test_health_check_endpoint() {
        let state = create_test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .expect("should build request"),
            )
            .await
            .expect("should get response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_daemon_info_endpoint() {
        let state = create_test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/daemon/info")
                    .body(Body::empty())
                    .expect("should build request"),
            )
            .await
            .expect("should get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("should read body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("should parse json");

        assert_eq!(json["status"], "success");
        assert!(json["data"]["version"].is_string());
        assert!(json["data"]["api_version"].is_string());
        assert!(json["data"]["runtime"].is_string());
        assert!(json["data"]["platform"].is_string());
        assert!(json["data"]["uptime"].is_string());
    }

    #[tokio::test]
    async fn test_not_found() {
        let state = create_test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/nonexistent")
                    .body(Body::empty())
                    .expect("should build request"),
            )
            .await
            .expect("should get response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
