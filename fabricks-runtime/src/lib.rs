//! Fabricks WASM Runtime
//!
//! Integration layer for executing WASM modules with capability-based security.
//!
//! # Features
//!
//! - Wasmtime-based execution
//! - Capability enforcement (deny-by-default)
//! - Environment variable filtering
//! - Filesystem preopening with path restrictions
//! - Network capability proxying
//! - Instance pooling for performance
//!
//! # Security Model
//!
//! All capabilities must be explicitly granted in the Fabrickfile.
//! The runtime enforces these restrictions at the WASI layer.

// Implementation will be added in Phase 4
