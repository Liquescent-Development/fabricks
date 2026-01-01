//! Error types for OCI registry operations.

use thiserror::Error;

/// Errors that can occur during OCI operations.
#[derive(Debug, Error)]
pub enum OciError {
    /// The underlying OCI client returned an error.
    #[error("OCI client error: {0}")]
    ClientError(#[from] oci_client::errors::OciDistributionError),

    /// JSON serialization/deserialization failed.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// IO operation failed.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Invalid image reference.
    #[error("invalid image reference '{reference}': {reason}")]
    InvalidReference {
        /// The invalid reference.
        reference: String,
        /// Reason it's invalid.
        reason: String,
    },

    /// Digest mismatch during verification.
    #[error("digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch {
        /// Expected digest.
        expected: String,
        /// Actual digest.
        actual: String,
    },

    /// Module not found.
    #[error("module not found: {reference}")]
    ModuleNotFound {
        /// The reference that wasn't found.
        reference: String,
    },

    /// Unsupported media type in manifest.
    #[error("unsupported media type: {media_type}")]
    UnsupportedMediaType {
        /// The unsupported media type.
        media_type: String,
    },

    /// Invalid module: missing required layer.
    #[error("invalid module: {reason}")]
    InvalidModule {
        /// Why the module is invalid.
        reason: String,
    },

    /// Configuration parsing failed.
    #[error("failed to parse config: {reason}")]
    ConfigParseError {
        /// Why parsing failed.
        reason: String,
    },

    /// Local storage operation failed.
    #[error("storage error: {reason}")]
    StorageError {
        /// Why the storage operation failed.
        reason: String,
    },
}

/// Result type for OCI operations.
pub type Result<T> = std::result::Result<T, OciError>;
