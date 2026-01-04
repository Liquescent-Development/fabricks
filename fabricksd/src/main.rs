//! Fabricks Daemon
//!
//! Long-running orchestration service for Fabricks.
//!
//! The daemon manages:
//! - WASM module lifecycle (start, stop, restart)
//! - Health monitoring and automatic recovery
//! - Network proxying and service discovery
//! - Volume management
//! - Auto-scaling based on metrics
//!
//! # API
//!
//! The daemon exposes a REST API at `/v1/*` via Unix socket.
//! Default socket path is `~/.fabricks/fabricks.sock` for user mode
//! or `/var/run/fabricks.sock` for system mode.
//!
//! # Configuration
//!
//! Configuration is loaded from:
//! 1. `/etc/fabricksd/config.toml` (system-wide)
//! 2. `~/.fabricks/daemon.toml` (user-specific)
//! 3. Falls back to defaults if no config file found

use std::time::Duration;

use tokio::net::UnixListener;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use fabricksd::api::build_router;
use fabricksd::config::DaemonConfig;
use fabricksd::events::{Event, EventType};
use fabricksd::shutdown::shutdown_signal;
use fabricksd::state::AppState;

/// Shutdown timeout in seconds.
const SHUTDOWN_TIMEOUT_SECS: u64 = 30;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    init_tracing();

    info!("Starting fabricksd v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = DaemonConfig::load()?;
    info!(
        socket = %config.daemon.socket.display(),
        data_dir = %config.daemon.data_dir.display(),
        "Loaded configuration"
    );

    // Create application state
    let state = AppState::new(config.clone())?;
    state.initialize().await?;
    info!("Initialized state");

    // Publish startup event
    let startup_event = Event::new(
        EventType::DaemonStarted,
        serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "socket": config.daemon.socket.display().to_string(),
        }),
    );
    state.event_bus.publish(startup_event).await;

    // Remove existing socket file if present
    if config.daemon.socket.exists() {
        std::fs::remove_file(&config.daemon.socket)?;
        info!("Removed existing socket file");
    }

    // Ensure parent directory exists
    if let Some(parent) = config.daemon.socket.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create Unix socket listener
    let listener = UnixListener::bind(&config.daemon.socket).map_err(|e| {
        error!(path = %config.daemon.socket.display(), error = %e, "Failed to bind socket");
        fabricksd::DaemonError::SocketBindError {
            path: config.daemon.socket.clone(),
            source: e,
        }
    })?;
    info!(socket = %config.daemon.socket.display(), "Listening on Unix socket");

    // Build router
    let app = build_router(state.clone());

    // Run server with graceful shutdown
    let shutdown_timeout = Duration::from_secs(SHUTDOWN_TIMEOUT_SECS);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state, shutdown_timeout))
        .await?;

    info!("Daemon stopped");
    Ok(())
}

/// Initializes tracing with environment-based filtering.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}
