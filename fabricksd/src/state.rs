//! Shared application state for the daemon.

use std::sync::Arc;
use std::time::Instant;

use sled::Db;
use tokio::sync::{broadcast, RwLock};

use crate::config::DaemonConfig;
use crate::error::Result;
use crate::events::EventBus;
use crate::service::ServiceManager;
use crate::store::StateStore;

/// Shared daemon state accessible from all handlers.
///
/// This struct is cheaply cloneable (all fields are `Arc`-wrapped) and
/// is passed to all API handlers via axum's state extraction.
#[derive(Clone)]
pub struct AppState {
    /// Daemon configuration.
    pub config: Arc<DaemonConfig>,

    /// Time when daemon started.
    started_at: Instant,

    /// Embedded database.
    pub db: Arc<Db>,

    /// Persistent state store.
    pub store: Arc<StateStore>,

    /// Event bus for publishing events.
    pub event_bus: Arc<EventBus>,

    /// Service manager for service lifecycle.
    pub service_manager: Arc<RwLock<ServiceManager>>,

    /// Shutdown signal sender.
    shutdown_tx: broadcast::Sender<()>,
}

impl AppState {
    /// Creates a new application state.
    ///
    /// This initializes the database, state store, event bus, and service manager.
    ///
    /// # Errors
    ///
    /// Returns an error if the data directory cannot be created or the
    /// database cannot be opened.
    pub fn new(config: DaemonConfig) -> Result<Self> {
        // Ensure data directory exists
        std::fs::create_dir_all(&config.daemon.data_dir)?;

        // Open sled database
        let db_path = config.daemon.data_dir.join("state.db");
        let db = sled::open(&db_path)?;
        let db = Arc::new(db);

        // Create state store
        let store = Arc::new(StateStore::new(Arc::clone(&db)));

        // Create event bus
        let event_bus = Arc::new(EventBus::new(
            config.events.buffer_size,
            config.events.history_size,
        ));

        // Create service manager
        let service_manager = ServiceManager::new(
            Arc::clone(&store),
            Arc::clone(&event_bus),
            config.runtime.max_cached_modules,
        )?;
        let service_manager = Arc::new(RwLock::new(service_manager));

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config: Arc::new(config),
            started_at: Instant::now(),
            db,
            store,
            event_bus,
            service_manager,
            shutdown_tx,
        })
    }

    /// Gets the daemon uptime.
    #[must_use]
    pub fn uptime(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Sends shutdown signal to all listeners.
    ///
    /// This notifies all tasks that have subscribed to the shutdown signal
    /// to begin their cleanup procedures.
    pub fn shutdown(&self) {
        // Ignore send errors - receivers may have already been dropped
        let _ = self.shutdown_tx.send(());
    }

    /// Subscribes to the shutdown signal.
    ///
    /// Returns a receiver that will receive a message when the daemon
    /// is shutting down.
    #[must_use]
    pub fn subscribe_shutdown(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    fn create_test_config() -> DaemonConfig {
        let dir = tempdir().expect("should create temp dir");
        let mut config = DaemonConfig::default();
        config.daemon.data_dir = dir.keep();
        config
    }

    #[test]
    fn test_app_state_creation() {
        let config = create_test_config();
        let state = AppState::new(config).expect("should create state");

        assert!(state.uptime() < Duration::from_secs(1));
    }

    #[test]
    fn test_app_state_clone() {
        let config = create_test_config();
        let state = AppState::new(config).expect("should create state");

        let cloned = state.clone();
        assert!(Arc::ptr_eq(&state.config, &cloned.config));
        assert!(Arc::ptr_eq(&state.db, &cloned.db));
        assert!(Arc::ptr_eq(&state.store, &cloned.store));
        assert!(Arc::ptr_eq(&state.event_bus, &cloned.event_bus));
        assert!(Arc::ptr_eq(&state.service_manager, &cloned.service_manager));
    }

    #[tokio::test]
    async fn test_shutdown_signal() {
        let config = create_test_config();
        let state = AppState::new(config).expect("should create state");

        let mut rx = state.subscribe_shutdown();

        // Shutdown should succeed
        state.shutdown();

        // Receiver should get the signal
        let result = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_ok());
    }
}
