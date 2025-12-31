//! Fabricks Common Library
//!
//! Shared data models, parsers, and utilities for Fabricks.
//!
//! # Modules
//!
//! - [`models`] - Data structures for [`Fabrickfile`](models::Fabrickfile) and [`MortarFile`](models::MortarFile)
//! - [`error`] - Error types for validation and parsing
//! - [`parser`] - File parsing functions for configuration files
//! - [`validation`] - Validation logic and traits
//!
//! # Example
//!
//! ```
//! use fabricks_common::parser::parse_fabrickfile_str;
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
//! let fabrickfile = parse_fabrickfile_str(toml)?;
//! assert_eq!(fabrickfile.info.name, "my-service");
//! # Ok::<(), fabricks_common::ParseError>(())
//! ```

pub mod error;
pub mod models;
pub mod parser;
pub mod validation;

// Re-export commonly used types at crate root for convenience.
pub use error::{ParseError, ValidationError};
pub use models::{Capabilities, Fabrickfile, MortarFile};
pub use parser::{
    parse_fabrickfile, parse_fabrickfile_str, parse_mortar_file, parse_mortar_file_str,
};
pub use validation::Validate;
