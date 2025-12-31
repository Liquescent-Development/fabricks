//! Data models for Fabricks configuration files.
//!
//! This module contains the data structures for:
//! - [`Fabrickfile`] - Single WASM service definition
//! - [`MortarFile`] - Multi-service application composition
//!
//! # Example
//!
//! ```
//! use fabricks_common::models::fabrickfile::Fabrickfile;
//!
//! let toml = r#"
//!     fabrick_version = "1.0"
//!
//!     [info]
//!     name = "my-service"
//!     version = "1.0.0"
//!
//!     [capabilities.network]
//!     listen = [8080]
//! "#;
//!
//! let fabrickfile: Fabrickfile = toml::from_str(toml).unwrap();
//! assert_eq!(fabrickfile.info.name, "my-service");
//! ```

pub mod capability;
pub mod common;
pub mod fabrickfile;
pub mod health_check;
pub mod mortar;

// Re-export primary types for convenience.
pub use capability::Capabilities;
pub use common::{ByteSize, Duration, Labels, Replicas, Resources, RestartPolicy};
pub use fabrickfile::Fabrickfile;
pub use health_check::HealthCheck;
pub use mortar::MortarFile;
