//! Shared application state for the daemon.

use std::sync::Arc;
use std::time::Instant;

use sled::Db;
use tokio::sync::{RwLock, broadcast};

use tracing::info;

use crate::config::DaemonConfig;
use crate::error::Result;
use crate::events::EventBus;
use crate::health::{HealthMonitor, HealthMonitorConfig};
use crate::network::{NetworkManager, ServiceRegistry};
use crate::proxy::{EgressProxy, ProxyServer, RequestHandler, ServiceRouter, TcpConnectionHandler};
use crate::scaler::{AutoScaler, AutoScalerConfig, MetricsCollector, MetricsCollectorConfig};
use crate::service::ServiceManager;
use crate::store::StateStore;
use crate::volume::VolumeManager;

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

    /// Service registry for name-to-ID resolution.
    pub service_registry: Arc<ServiceRegistry>,

    /// Network manager for network isolation.
    pub network_manager: Arc<NetworkManager>,

    /// Proxy server for HTTP request routing.
    pub proxy_server: Arc<ProxyServer>,

    /// Egress proxy for outbound HTTP requests.
    pub egress_proxy: Arc<EgressProxy>,

    /// Health monitor for service health tracking.
    pub health_monitor: Arc<HealthMonitor>,

    /// Volume manager for persistent storage.
    pub volume_manager: Arc<VolumeManager>,

    /// Metrics collector for tracking service metrics.
    pub metrics_collector: Arc<MetricsCollector>,

    /// Auto-scaler for automatic scaling based on metrics.
    pub auto_scaler: Arc<AutoScaler>,

    /// Shutdown signal sender.
    shutdown_tx: broadcast::Sender<()>,
}

impl AppState {
    /// Creates a new application state.
    ///
    /// This initializes the database, state store, event bus, service manager,
    /// network manager, proxy server, egress proxy, and health monitor.
    ///
    /// # Errors
    ///
    /// Returns an error if the data directory cannot be created, the
    /// database cannot be opened, or any component fails to initialize.
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

        // Create service registry for name resolution
        let service_registry = Arc::new(ServiceRegistry::new());

        // Create network manager
        let network_manager = Arc::new(NetworkManager::new(
            Arc::clone(&store),
            Arc::clone(&service_registry),
        ));

        // Create service router for proxy
        let service_router = Arc::new(ServiceRouter::new());

        // Create proxy server
        let proxy_server = Arc::new(ProxyServer::new(
            Arc::clone(&service_router),
            Arc::clone(&network_manager),
        ));

        // Create egress proxy for outbound requests
        let egress_proxy = Arc::new(EgressProxy::new(Arc::clone(&network_manager))?);

        // Create health monitor
        let health_monitor_config = HealthMonitorConfig::default();
        let health_monitor = Arc::new(HealthMonitor::new(health_monitor_config)?);

        // Create volume manager
        let volumes_path = config.daemon.data_dir.join("volumes");
        let volume_manager = Arc::new(VolumeManager::new(Arc::clone(&store), volumes_path));

        // Create service manager with proxy server, network manager, and volume manager
        let service_manager = ServiceManager::new(
            Arc::clone(&store),
            Arc::clone(&event_bus),
            config.runtime.max_cached_modules,
            Some(Arc::clone(&proxy_server)),
            Some(Arc::clone(&network_manager)),
            Some(Arc::clone(&volume_manager)),
        )?;
        let service_manager = Arc::new(RwLock::new(service_manager));

        // Create metrics collector
        let metrics_collector_config = MetricsCollectorConfig::default();
        let metrics_collector = Arc::new(MetricsCollector::new(metrics_collector_config));

        // Create auto-scaler
        let auto_scaler_config = AutoScalerConfig::default();
        let auto_scaler = Arc::new(AutoScaler::new(
            auto_scaler_config,
            Arc::clone(&service_manager),
            Arc::clone(&metrics_collector),
            Arc::clone(&event_bus),
        ));

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config: Arc::new(config),
            started_at: Instant::now(),
            db,
            store,
            event_bus,
            service_manager,
            service_registry,
            network_manager,
            proxy_server,
            egress_proxy,
            health_monitor,
            volume_manager,
            metrics_collector,
            auto_scaler,
            shutdown_tx,
        })
    }

    /// Gets the daemon uptime.
    #[must_use]
    pub fn uptime(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Initializes async components after construction.
    ///
    /// This must be called after `new()` to set up the HTTP and TCP request routing
    /// between the proxy server and service manager.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub async fn initialize(&self) -> Result<()> {
        // Set up HTTP request routing from proxy server to service manager
        let service_manager = Arc::clone(&self.service_manager);

        let request_handler: RequestHandler = Arc::new(move |service_id, request| {
            let service_manager = Arc::clone(&service_manager);
            Box::pin(async move {
                let manager = service_manager.read().await;
                manager.route_http_request(&service_id, request).await
            })
        });

        self.proxy_server.set_request_handler(request_handler).await;
        info!("HTTP request handler configured");

        // Set up TCP connection routing from proxy server to service manager
        let service_manager = Arc::clone(&self.service_manager);

        let tcp_handler: TcpConnectionHandler = Arc::new(move |service_id, stream, peer_addr| {
            let service_manager = Arc::clone(&service_manager);
            Box::pin(async move {
                let manager = service_manager.read().await;
                manager
                    .route_tcp_connection(&service_id, stream, peer_addr)
                    .await
            })
        });

        self.proxy_server
            .set_tcp_connection_handler(tcp_handler)
            .await;
        info!("TCP connection handler configured");

        Ok(())
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
        assert!(Arc::ptr_eq(
            &state.service_registry,
            &cloned.service_registry
        ));
        assert!(Arc::ptr_eq(&state.network_manager, &cloned.network_manager));
        assert!(Arc::ptr_eq(&state.proxy_server, &cloned.proxy_server));
        assert!(Arc::ptr_eq(&state.egress_proxy, &cloned.egress_proxy));
        assert!(Arc::ptr_eq(&state.health_monitor, &cloned.health_monitor));
        assert!(Arc::ptr_eq(&state.volume_manager, &cloned.volume_manager));
        assert!(Arc::ptr_eq(
            &state.metrics_collector,
            &cloned.metrics_collector
        ));
        assert!(Arc::ptr_eq(&state.auto_scaler, &cloned.auto_scaler));
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
