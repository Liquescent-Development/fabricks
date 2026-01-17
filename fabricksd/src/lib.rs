//! Fabricks Daemon Library
//!
//! This crate provides the core functionality for the Fabricks daemon,
//! including the HTTP API server, state persistence, and event system.
//!
//! # Modules
//!
//! - [`error`] - Error types for daemon operations
//! - [`config`] - Configuration loading and management
//! - [`state`] - Application state container
//! - [`store`] - Persistent state storage using sled
//! - [`events`] - Event bus for pub/sub messaging
//! - [`api`] - HTTP API server and handlers
//! - [`service`] - Service lifecycle management
//! - [`network`] - Network isolation and service discovery
//! - [`volume`] - Persistent volume management
//! - [`proxy`] - HTTP proxy for routing to WASM services
//! - [`scaler`] - Metrics collection and auto-scaling
//! - [`shutdown`] - Graceful shutdown coordination

pub mod api;
pub mod config;
pub mod error;
pub mod events;
pub mod health;
pub mod network;
pub mod proxy;
pub mod scaler;
pub mod service;
pub mod shutdown;
pub mod state;
pub mod store;
pub mod volume;

// Re-export commonly used types at crate root for convenience.
pub use config::DaemonConfig;
pub use error::{DaemonError, Result};
pub use events::{Event, EventBus, EventType};
pub use health::{HealthMonitor, HealthMonitorConfig, HealthStatus, ServiceHealth};
pub use network::{NetworkConfig, NetworkManager, ServiceRegistry};
pub use proxy::{EgressProxy, EgressRequest, EgressResponse, ProxyServer, ServiceRouter};
pub use scaler::{AutoScaler, AutoScalerConfig, MetricsCollector, MetricsCollectorConfig, ServiceMetrics};
pub use service::{ServiceConfig, ServiceInfo, ServiceManager};
pub use state::AppState;
pub use store::StateStore;
pub use volume::{VolumeConfig, VolumeManager};
