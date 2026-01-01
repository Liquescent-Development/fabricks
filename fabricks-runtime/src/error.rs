//! Error types for WASM runtime operations.

use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur during runtime operations.
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// Failed to compile WASM module.
    #[error("failed to compile WASM module: {0}")]
    CompileError(#[from] wasmtime::Error),

    /// Failed to instantiate WASM module.
    #[error("failed to instantiate module: {reason}")]
    InstantiationError {
        /// Reason for the failure.
        reason: String,
    },

    /// Module execution failed.
    #[error("module execution failed: {reason}")]
    ExecutionError {
        /// Reason for the failure.
        reason: String,
    },

    /// A requested capability was denied.
    #[error("capability denied: {capability}")]
    CapabilityDenied {
        /// The capability that was denied.
        capability: String,
    },

    /// Environment variable access was denied.
    #[error("access to environment variable '{name}' denied")]
    EnvAccessDenied {
        /// The environment variable name.
        name: String,
    },

    /// Network connection was denied.
    #[error("network connection to '{target}' denied")]
    NetworkDenied {
        /// The target host:port.
        target: String,
    },

    /// Filesystem access was denied.
    #[error("filesystem access to '{path}' denied (operation: {operation})")]
    FilesystemDenied {
        /// The path that was denied.
        path: PathBuf,
        /// The operation that was denied (read, write).
        operation: String,
    },

    /// IO error during runtime operations.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Invalid WASM module.
    #[error("invalid WASM module: {reason}")]
    InvalidModule {
        /// Why the module is invalid.
        reason: String,
    },

    /// Module missing required export.
    #[error("module missing required export: {export}")]
    MissingExport {
        /// The missing export name.
        export: String,
    },

    /// Runtime pool exhausted.
    #[error("runtime pool exhausted (max: {max_size})")]
    PoolExhausted {
        /// Maximum pool size.
        max_size: usize,
    },
}

/// Result type for runtime operations.
pub type Result<T> = std::result::Result<T, RuntimeError>;
