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
    #[error("network not found: {0}")]
    NetworkNotFound(String),

    /// Network already exists.
    #[error("network already exists with name: {0}")]
    NetworkExists(String),

    /// Network has members and cannot be deleted.
    #[error("network '{0}' has members and cannot be deleted")]
    NetworkHasMembers(String),

    /// Network access denied (service only allows internal access).
    #[error("network access denied for service '{service_id}': {reason}")]
    NetworkAccessDenied {
        /// Service ID.
        service_id: String,
        /// Reason for denial.
        reason: String,
    },

    /// Volume not found.
    #[error("volume not found: {id}")]
    VolumeNotFound {
        /// Volume ID.
        id: String,
    },

    /// Volume already exists.
    #[error("volume already exists with name: {0}")]
    VolumeExists(String),

    /// Volume is mounted and cannot be deleted.
    #[error("volume '{id}' is mounted by services: {services:?}")]
    VolumeMounted {
        /// Volume ID.
        id: String,
        /// Services that have the volume mounted.
        services: Vec<String>,
    },

    /// Failed to create volume directory.
    #[error("failed to create volume '{name}': {reason}")]
    VolumeCreateFailed {
        /// Volume name.
        name: String,
        /// Reason for the failure.
        reason: String,
    },

    /// Failed to delete volume directory.
    #[error("failed to delete volume '{id}': {reason}")]
    VolumeDeleteFailed {
        /// Volume ID.
        id: String,
        /// Reason for the failure.
        reason: String,
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

    /// Circular dependency detected.
    #[error("circular dependency detected in service graph")]
    CircularDependency,

    /// Runtime error.
    #[error("runtime error: {0}")]
    RuntimeError(#[from] fabricks_runtime::RuntimeError),

    /// Fabrickfile parse error.
    #[error("failed to parse Fabrickfile: {0}")]
    FabrickfileParseError(String),

    /// Build error.
    #[error("build failed: {0}")]
    BuildError(String),

    /// Service already exists.
    #[error("service already exists: {name}")]
    ServiceAlreadyExists {
        /// Service name.
        name: String,
    },

    /// Service is not running.
    #[error("service is not running: {id}")]
    ServiceNotRunning {
        /// Service ID.
        id: String,
    },

    /// Mortar project not found.
    #[error("mortar project not found: {name}")]
    MortarProjectNotFound {
        /// Project name.
        name: String,
    },

    /// Dependency not found.
    #[error(
        "dependency not found: service '{service}' depends on '{dependency}' which does not exist"
    )]
    DependencyNotFound {
        /// Service name.
        service: String,
        /// Missing dependency.
        dependency: String,
    },

    /// WASM module not found.
    #[error("WASM module not found at path: {path}")]
    WasmModuleNotFound {
        /// Path to the WASM file.
        path: String,
    },

    /// Port already bound.
    #[error("port {port} is already bound to service '{service_id}'")]
    PortAlreadyBound {
        /// Port number.
        port: u16,
        /// Service ID currently bound.
        service_id: String,
    },

    /// Port not bound.
    #[error("port {port} is not bound to any service")]
    PortNotBound {
        /// Port number.
        port: u16,
    },

    /// Failed to bind port.
    #[error("failed to bind port {port}: {reason}")]
    PortBindError {
        /// Port number.
        port: u16,
        /// Reason for the failure.
        reason: String,
    },

    /// No instances available for routing.
    #[error("no healthy instances available for service '{service_id}'")]
    NoInstancesAvailable {
        /// Service ID.
        service_id: String,
    },

    /// Generic service error.
    #[error("service '{id}' error: {reason}")]
    ServiceError {
        /// Service ID.
        id: String,
        /// Error reason.
        reason: String,
    },
}

/// Result type for daemon operations.
pub type Result<T> = std::result::Result<T, DaemonError>;
