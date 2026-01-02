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
//! - [`shutdown`] - Graceful shutdown coordination

pub mod api;
pub mod config;
pub mod error;
pub mod events;
pub mod shutdown;
pub mod state;
pub mod store;

// Re-export commonly used types at crate root for convenience.
pub use config::DaemonConfig;
pub use error::{DaemonError, Result};
pub use events::{Event, EventBus, EventType};
pub use state::AppState;
pub use store::StateStore;
