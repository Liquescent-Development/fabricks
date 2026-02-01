//! Daemon management API handlers.

use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tracing::info;

use crate::api::response::ApiResponse;
use crate::state::AppState;

/// Daemon information response.
#[derive(Debug, Serialize)]
pub struct DaemonInfo {
    /// Daemon version.
    pub version: String,

    /// API version.
    pub api_version: String,

    /// WASM runtime name and version.
    pub runtime: String,

    /// Platform identifier (os/arch).
    pub platform: String,

    /// When the daemon started (ISO 8601).
    pub started_at: DateTime<Utc>,

    /// Human-readable uptime.
    pub uptime: String,

    /// Relevant configuration values.
    pub config: DaemonConfigInfo,
}

/// Configuration info subset for API response.
#[derive(Debug, Serialize)]
pub struct DaemonConfigInfo {
    /// Socket path.
    pub socket: String,

    /// Data directory.
    pub data_dir: String,

    /// Maximum services.
    pub max_services: u32,
}

/// GET `/v1/daemon/info`
///
/// Returns information about the running daemon.
pub async fn daemon_info(State(state): State<AppState>) -> Json<ApiResponse<DaemonInfo>> {
    let uptime = state.uptime();
    let uptime_str = format_uptime(uptime);

    // Calculate started_at from current time minus uptime
    let started_at = Utc::now() - chrono::Duration::from_std(uptime).unwrap_or_default();

    let info = DaemonInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: state.config.api.version.clone(),
        runtime: "wasmtime".to_string(),
        platform: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
        started_at,
        uptime: uptime_str,
        config: DaemonConfigInfo {
            socket: state.config.daemon.socket.display().to_string(),
            data_dir: state.config.daemon.data_dir.display().to_string(),
            max_services: state.config.resources.max_services,
        },
    };

    Json(ApiResponse::success(info))
}

/// Shutdown response.
#[derive(Debug, Serialize)]
pub struct ShutdownResponse {
    /// Message confirming shutdown has been initiated.
    pub message: String,
}

/// POST `/v1/daemon/shutdown`
///
/// Initiates graceful daemon shutdown.
pub async fn shutdown(State(state): State<AppState>) -> Json<ApiResponse<ShutdownResponse>> {
    info!("Shutdown requested via API");

    // Send shutdown signal (this is non-blocking)
    state.shutdown();

    Json(ApiResponse::success(ShutdownResponse {
        message: "Shutdown initiated".to_string(),
    }))
}

/// Formats a duration as a human-readable string.
fn format_uptime(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if days > 0 {
        format!("{days}d {hours}h {mins}m {secs}s")
    } else if hours > 0 {
        format!("{hours}h {mins}m {secs}s")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_uptime_seconds() {
        assert_eq!(format_uptime(std::time::Duration::from_secs(45)), "45s");
    }

    #[test]
    fn test_format_uptime_minutes() {
        assert_eq!(format_uptime(std::time::Duration::from_secs(125)), "2m 5s");
    }

    #[test]
    fn test_format_uptime_hours() {
        assert_eq!(
            format_uptime(std::time::Duration::from_secs(3725)),
            "1h 2m 5s"
        );
    }

    #[test]
    fn test_format_uptime_days() {
        assert_eq!(
            format_uptime(std::time::Duration::from_secs(90125)),
            "1d 1h 2m 5s"
        );
    }
}
