//! Fabricks Common Library
//!
//! Shared data models, parsers, and utilities for Fabricks.
//!
//! # Modules
//!
//! - [`models`] - Data structures for [`Fabrickfile`](models::Fabrickfile) and [`MortarFile`](models::MortarFile)
//! - [`error`] - Error types for validation and parsing
//! - [`validation`] - Validation logic and traits
//!
//! # Example
//!
//! ```
//! use fabricks_common::models::Fabrickfile;
//! use fabricks_common::validation::Validate;
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
//! fabrickfile.validate().expect("validation failed");
//! ```

pub mod error;
pub mod models;
pub mod validation;

// Re-export commonly used types at crate root for convenience.
pub use error::{ParseError, ValidationError};
pub use models::{Capabilities, Fabrickfile, MortarFile};
pub use validation::Validate;
