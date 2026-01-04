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
//! - Network capability checking (enforced at daemon level)
//! - Instance pooling for performance
//!
//! # Security Model
//!
//! All capabilities must be explicitly granted in the Fabrickfile.
//! The runtime enforces these restrictions at the WASI layer.
//!
//! # Example
//!
//! ```no_run
//! use fabricks_runtime::{Runtime, RuntimeConfig};
//! use fabricks_common::Capabilities;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load WASM bytes
//! let wasm_bytes = std::fs::read("module.wasm")?;
//!
//! // Configure with capabilities
//! let config = RuntimeConfig {
//!     capabilities: Capabilities {
//!         env: Some(vec!["DATABASE_URL".to_string()]),
//!         ..Default::default()
//!     },
//!     ..Default::default()
//! };
//!
//! // Create and run
//! let runtime = Runtime::new(&wasm_bytes, config)?;
//! runtime.run()?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod http;
pub mod pool;
pub mod runtime;

// Re-export main types
pub use error::{Result, RuntimeError};
pub use http::{HttpRuntime, HttpRuntimeConfig, HttpRequest, HttpResponse, OutboundHandler, Scheme, WasiHttpState};
pub use pool::{RuntimePool, RuntimePoolBuilder};
pub use runtime::{Runtime, RuntimeConfig};
