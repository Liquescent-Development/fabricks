//! Service management for the Fabricks daemon.
//!
//! This module provides the core service management functionality:
//!
//! - [`ServiceManager`] - Coordinates service lifecycle and mortar deployments
//! - [`ServiceHandle`] - Manages individual service instances
//! - [`ServiceConfig`] - Configuration for creating services
//! - [`ServiceState`] - Persisted service state
//! - [`ServiceInfo`] - Summary information for API responses
//! - [`ServiceDetail`] - Detailed information including instances
//!
//! # Example
//!
//! ```ignore
//! use fabricksd::service::{ServiceManager, ServiceConfig};
//!
//! // Create a service from configuration
//! let id = manager.create_service(config).await?;
//!
//! // Start the service
//! manager.start_service(&id).await?;
//!
//! // Scale to 3 replicas
//! manager.scale_service(&id, 3).await?;
//!
//! // Stop and delete
//! manager.stop_service(&id).await?;
//! manager.delete_service(&id).await?;
//! ```
//!
//! # Mortar Projects
//!
//! The service manager also handles mortar project deployments:
//!
//! ```ignore
//! // Deploy a mortar project
//! let (project_name, service_ids) = manager.deploy_mortar(path).await?;
//!
//! // List services in the project
//! let services = manager.list_mortar_services(&project_name).await?;
//!
//! // Tear down the project
//! manager.teardown_mortar(&project_name).await?;
//! ```

pub mod dependency;
pub mod handle;
pub mod manager;
pub mod types;

// Re-export main types
pub use handle::{CapabilityOutboundHandler, ServiceHandle};
pub use manager::ServiceManager;
pub use types::{
    generate_instance_id, generate_service_id, Instance, InstanceState, ReplicaState,
    ServiceConfig, ServiceDetail, ServiceInfo, ServiceState, State,
};
