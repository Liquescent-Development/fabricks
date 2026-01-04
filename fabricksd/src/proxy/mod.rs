//! HTTP proxy layer for service request routing.
//!
//! This module provides the proxy infrastructure that sits between external
//! HTTP traffic and WASM services. The daemon binds ports on behalf of services
//! (since WASM modules cannot directly bind sockets) and routes requests to
//! the appropriate service instances.
//!
//! # Architecture
//!
//! ```text
//! External HTTP Request
//!         │
//!         ▼
//!    ProxyServer
//!         │
//!         ▼
//!   ServiceRouter (port → service mapping)
//!         │
//!         ▼
//!   LoadBalancer (instance selection)
//!         │
//!         ▼
//!   RequestHandler callback (provided by daemon)
//!         │
//!         ▼
//!   HttpRuntime (WASM execution)
//! ```
//!
//! # Components
//!
//! - [`ProxyServer`] - Manages TCP listeners and handles incoming connections
//! - [`ServiceRouter`] - Maps ports to services and tracks bindings
//! - [`LoadBalancer`] - Distributes requests across service instances
//!
//! # Usage
//!
//! ```ignore
//! use fabricksd::proxy::{ProxyServer, ServiceRouter};
//! use std::sync::Arc;
//!
//! // Create router and server
//! let router = Arc::new(ServiceRouter::new());
//! let server = ProxyServer::new(router);
//!
//! // Set up request handler
//! server.set_request_handler(Arc::new(|service_id, request| {
//!     Box::pin(async move {
//!         // Route to WASM runtime
//!         Ok(response)
//!     })
//! })).await;
//!
//! // Bind port for a service
//! server.bind_port(8080, "my-service".to_string()).await?;
//! ```

mod egress;
mod loadbalancer;
mod router;
mod server;

use std::sync::Arc;

pub use egress::{
    EgressProxy, EgressRequest, EgressResponse, InternalRouteFuture, InternalRouteHandler,
    SharedEgressProxy,
};
pub use loadbalancer::{LoadBalancer, Strategy};
pub use router::{ServiceBinding, ServiceRouter, SharedServiceRouter};
pub use server::{ProxyServer, RequestFuture, RequestHandler};

/// Shared reference to a proxy server.
pub type SharedProxyServer = Arc<ProxyServer>;
