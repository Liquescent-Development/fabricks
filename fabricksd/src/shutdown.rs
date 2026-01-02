//! Graceful shutdown handling.

use std::time::Duration;

use tokio::signal;
use tracing::{error, info};

use crate::events::{Event, EventType};
use crate::state::AppState;

/// Waits for shutdown signal and coordinates graceful shutdown.
///
/// This function listens for SIGTERM and SIGINT signals, publishes a
/// shutdown event, and signals all tasks to clean up before returning.
///
/// # Arguments
///
/// * `state` - The application state
/// * `timeout` - Maximum time to wait for cleanup before returning
pub async fn shutdown_signal(state: AppState, timeout: Duration) {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            error!("Failed to install Ctrl+C handler: {e}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(e) => {
                error!("Failed to install SIGTERM handler: {e}");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => info!("Received Ctrl+C, initiating shutdown"),
        () = terminate => info!("Received SIGTERM, initiating shutdown"),
    }

    // Publish shutdown event
    let event = Event::new(
        EventType::DaemonStopping,
        serde_json::json!({
            "reason": "signal"
        }),
    );
    state.event_bus.publish(event).await;

    // Signal all tasks to shut down
    state.shutdown();

    // Give tasks time to clean up
    info!("Waiting for tasks to complete (timeout: {timeout:?})");
    tokio::time::sleep(timeout).await;

    info!("Shutdown complete");
}
