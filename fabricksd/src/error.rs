//! Error types for the Fabricks daemon.

use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur during daemon operations.
#[derive(Debug, Error)]
pub enum DaemonError {
    /// Failed to load configuration file.
    #[error("failed to load configuration from '{path}': {reason}")]
    ConfigLoadError {
        /// Configuration file path.
        path: PathBuf,
        /// Reason for the failure.
        reason: String,
    },

    /// Failed to parse configuration.
    #[error("invalid configuration: {0}")]
    ConfigParseError(#[from] toml::de::Error),

    /// Failed to create or bind Unix socket.
    #[error("failed to bind Unix socket at '{path}': {source}")]
    SocketBindError {
        /// Socket path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to open or access database.
    #[error("database error: {0}")]
    DatabaseError(#[from] sled::Error),

    /// JSON serialization/deserialization error.
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// IO error.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Service not found.
    #[error("service not found: {id}")]
    ServiceNotFound {
        /// Service ID.
        id: String,
    },

    /// Network not found.
    #[error("network not found: {id}")]
    NetworkNotFound {
        /// Network ID.
        id: String,
    },

    /// Volume not found.
    #[error("volume not found: {id}")]
    VolumeNotFound {
        /// Volume ID.
        id: String,
    },

    /// Invalid state transition.
    #[error("invalid state transition from '{from}' to '{to}'")]
    InvalidStateTransition {
        /// Current state.
        from: String,
        /// Requested state.
        to: String,
    },

    /// Shutdown in progress.
    #[error("daemon is shutting down")]
    ShuttingDown,

    /// Event bus send error.
    #[error("failed to publish event: channel closed")]
    EventBusClosed,
}

/// Result type for daemon operations.
pub type Result<T> = std::result::Result<T, DaemonError>;
