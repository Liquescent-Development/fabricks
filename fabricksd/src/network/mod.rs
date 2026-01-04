//! Network management for service isolation and communication.
//!
//! This module provides network isolation between services. Services can only
//! communicate if they share at least one network. The network manager handles:
//!
//! - Network lifecycle (create, delete, list)
//! - Service membership (join, leave networks)
//! - Service discovery (name-to-ID resolution)
//! - Connection validation (shared network requirement)

mod manager;
mod registry;
mod types;
mod validation;

pub use manager::{NetworkManager, SharedNetworkManager};
pub use registry::{extract_service_name, ServiceRegistry, SharedServiceRegistry};
pub use types::{
    NetworkAccess, NetworkAudit, NetworkConfig, NetworkDetail, NetworkEncryption, NetworkInfo,
    NetworkIsolation, NetworkOptions, NetworkState,
};
pub use validation::{validate_connection, validate_listen_port, ConnectionDecision};
