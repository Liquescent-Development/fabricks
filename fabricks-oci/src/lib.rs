//! Fabricks OCI Registry Client
//!
//! Client library for pushing and pulling Fabricks WASM modules to/from
//! OCI-compliant container registries.
//!
//! # Features
//!
//! - Push and pull WASM modules as OCI artifacts
//! - OAuth2/Bearer token authentication
//! - Content verification with SHA256 digests
//! - Support for major registries (Docker Hub, GHCR, ECR, GAR, ACR)
//!
//! # Example
//!
//! ```no_run
//! use fabricks_oci::client::FabricksClient;
//! use fabricks_oci::module::FabricksModule;
//! use oci_client::Reference;
//! use oci_client::secrets::RegistryAuth;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create client
//! let client = FabricksClient::new();
//!
//! // Parse reference
//! let reference: Reference = "ghcr.io/user/my-module:1.0.0".parse()?;
//!
//! // Pull a module
//! let pulled = client.pull(&reference, &RegistryAuth::Anonymous).await?;
//! println!("Pulled module: {}", pulled.module.name());
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod digest;
pub mod error;
pub mod media_types;
pub mod module;
pub mod storage;

// Re-export commonly used types
pub use client::{ClientConfig, FabricksClient};
pub use error::{OciError, Result};
pub use module::{FabricksModule, PulledModule};
pub use storage::LocalStorage;

// Re-export oci-client types for convenience
pub use oci_client::Reference;
pub use oci_client::secrets::RegistryAuth;
