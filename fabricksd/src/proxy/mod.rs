//! Proxy layer for HTTP and TCP service request routing.
//!
//! This module provides the proxy infrastructure that sits between external
//! traffic and WASM services. The daemon binds ports on behalf of services
//! (since WASM modules cannot directly bind sockets) and routes requests to
//! the appropriate service instances.
//!
//! # Supported Protocols
//!
//! - **HTTP** - Requests are parsed as HTTP and routed to `HttpRuntime`
//! - **TCP** - Raw connections are passed to `TcpRuntime` (inetd model)
//!
//! # Architecture
//!
//! ```text
//! External Request (HTTP or TCP)
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
//!   Handler callback (provided by daemon)
//!         │
//!         ├──▶ HttpRuntime (HTTP services)
//!         │
//!         └──▶ TcpRuntime (TCP services, inetd model)
//! ```
//!
//! # Components
//!
//! - [`ProxyServer`] - Manages TCP listeners and handles incoming connections
//! - [`ServiceRouter`] - Maps ports to services and tracks bindings
//! - [`LoadBalancer`] - Distributes requests across service instances
//! - [`BindingProtocol`] - Distinguishes HTTP vs TCP port bindings
//!
//! # Usage
//!
//! ```ignore
//! use fabricksd::proxy::{ProxyServer, ServiceRouter, BindingProtocol};
//! use std::sync::Arc;
//!
//! // Create router and server
//! let router = Arc::new(ServiceRouter::new());
//! let server = ProxyServer::new(router);
//!
//! // Set up HTTP request handler
//! server.set_request_handler(Arc::new(|service_id, request| {
//!     Box::pin(async move {
//!         // Route to WASM runtime
//!         Ok(response)
//!     })
//! })).await;
//!
//! // Set up TCP connection handler
//! server.set_tcp_connection_handler(Arc::new(|service_id, stream, peer_addr| {
//!     Box::pin(async move {
//!         // Route to TcpRuntime (inetd model)
//!         Ok(())
//!     })
//! })).await;
//!
//! // Bind HTTP port for a service
//! server.bind_port(8080, "my-http-service".to_string()).await?;
//!
//! // Bind TCP port for a service
//! server.bind_tcp_port(9000, "my-tcp-service".to_string()).await?;
//! ```

mod egress;
mod loadbalancer;
mod router;
mod server;

use std::sync::Arc;

pub use egress::{
    EgressProxy, EgressRequest, EgressResponse, InternalRouteFuture, InternalRouteHandler,
    SharedEgressProxy, TcpConnectRequest, TcpConnectResult,
};
pub use loadbalancer::{LoadBalancer, Strategy};
pub use router::{BindingProtocol, ServiceBinding, ServiceRouter, SharedServiceRouter};
pub use server::{
    ProxyServer, RequestFuture, RequestHandler, TcpConnectionFuture, TcpConnectionHandler,
};

/// Shared reference to a proxy server.
pub type SharedProxyServer = Arc<ProxyServer>;
