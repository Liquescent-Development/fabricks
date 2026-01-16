//! Proxy server for routing HTTP requests and TCP connections to WASM services.
//!
//! The `ProxyServer` binds TCP listeners on configured ports and routes
//! incoming connections to the appropriate WASM service:
//! - HTTP ports parse requests and route to HTTP handlers
//! - TCP ports pass raw connections to TCP handlers (inetd model)

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument, warn};

use crate::error::{DaemonError, Result};
use crate::network::{NetworkManager, validate_ingress};

use super::router::{BindingProtocol, SharedServiceRouter};

/// Handle to a running listener.
#[derive(Debug)]
struct ListenerHandle {
    /// Port being listened on.
    port: u16,

    /// Handle to the listener task.
    task: JoinHandle<()>,

    /// Channel to signal shutdown.
    shutdown_tx: broadcast::Sender<()>,
}

impl ListenerHandle {
    /// Signals the listener to stop and waits for it to finish.
    async fn shutdown(self) {
        // Send shutdown signal (ignore error if receiver dropped)
        let _ = self.shutdown_tx.send(());

        // Wait for task to complete
        if let Err(e) = self.task.await {
            warn!(port = self.port, "Listener task panicked: {}", e);
        }
    }
}

/// Callback for handling requests.
///
/// The proxy server calls this callback with the service ID and request,
/// and expects an HTTP response. This allows the daemon to inject its own
/// logic for routing requests to WASM runtimes.
pub type RequestHandler =
    Arc<dyn Fn(String, fabricks_runtime::HttpRequest) -> RequestFuture + Send + Sync>;

/// Future returned by the request handler.
pub type RequestFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<fabricks_runtime::HttpResponse>> + Send>,
>;

/// Callback for handling TCP connections.
///
/// The proxy server calls this callback with the service ID, TCP stream, and
/// peer address. The callback should route the connection to the appropriate
/// WASM runtime (inetd model - stdin/stdout connected to the stream).
pub type TcpConnectionHandler =
    Arc<dyn Fn(String, TcpStream, SocketAddr) -> TcpConnectionFuture + Send + Sync>;

/// Future returned by the TCP connection handler.
pub type TcpConnectionFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>;

/// Proxy server that manages port listeners and routes requests.
///
/// Supports both HTTP and TCP protocols:
/// - HTTP ports parse requests and route to HTTP handlers
/// - TCP ports pass raw connections to TCP handlers (inetd model)
pub struct ProxyServer {
    /// Port to listener handle map.
    listeners: RwLock<HashMap<u16, ListenerHandle>>,

    /// Service router for port-to-service mapping.
    router: SharedServiceRouter,

    /// Network manager for access control.
    network_manager: Arc<NetworkManager>,

    /// HTTP request handler callback.
    request_handler: RwLock<Option<RequestHandler>>,

    /// TCP connection handler callback.
    tcp_connection_handler: RwLock<Option<TcpConnectionHandler>>,
}

impl ProxyServer {
    /// Creates a new proxy server.
    #[must_use]
    pub fn new(router: SharedServiceRouter, network_manager: Arc<NetworkManager>) -> Self {
        Self {
            listeners: RwLock::new(HashMap::new()),
            router,
            network_manager,
            request_handler: RwLock::new(None),
            tcp_connection_handler: RwLock::new(None),
        }
    }

    /// Sets the HTTP request handler callback.
    ///
    /// This callback is invoked for each incoming HTTP request with the service ID
    /// and request details. The callback should route the request to the
    /// appropriate WASM runtime and return the response.
    pub async fn set_request_handler(&self, handler: RequestHandler) {
        let mut guard = self.request_handler.write().await;
        *guard = Some(handler);
    }

    /// Sets the TCP connection handler callback.
    ///
    /// This callback is invoked for each incoming TCP connection with the service ID,
    /// TCP stream, and peer address. The callback should route the connection to the
    /// appropriate WASM runtime (inetd model - stdin/stdout connected to the stream).
    pub async fn set_tcp_connection_handler(&self, handler: TcpConnectionHandler) {
        let mut guard = self.tcp_connection_handler.write().await;
        *guard = Some(handler);
    }

    /// Gets the router.
    #[must_use]
    pub fn router(&self) -> &SharedServiceRouter {
        &self.router
    }

    /// Binds a port for a service and starts listening.
    ///
    /// # Arguments
    ///
    /// * `port` - The port to bind (use 0 for OS-assigned port)
    /// * `service_id` - The service ID to route requests to
    /// * `service_name` - The service name for display and lookup
    ///
    /// # Returns
    ///
    /// Returns the actual bound port (useful when port 0 was requested).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The port is already bound to another service
    /// - The port cannot be bound (permission denied, in use, etc.)
    #[instrument(skip(self), fields(port, service_id, service_name))]
    pub async fn bind_port(
        &self,
        port: u16,
        service_id: String,
        service_name: String,
    ) -> Result<u16> {
        // Try to bind the TCP listener first to get actual port
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| DaemonError::PortBindError {
                port,
                reason: e.to_string(),
            })?;

        // Get the actual bound port (important when port 0 was requested)
        let actual_port = listener
            .local_addr()
            .map_err(|e| DaemonError::PortBindError {
                port,
                reason: e.to_string(),
            })?
            .port();

        // Register with router (checks for conflicts)
        // If this fails, listener will be dropped automatically
        self.router
            .bind(actual_port, service_id.clone(), service_name.clone())
            .await?;

        info!(port = actual_port, %service_id, %service_name, "Bound port for service");

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        // Clone what we need for the listener task
        let router = Arc::clone(&self.router);
        let network_manager = Arc::clone(&self.network_manager);
        let handler_lock = Arc::new(RwLock::new(None::<RequestHandler>));

        // Copy current handler reference
        {
            let current = self.request_handler.read().await;
            if let Some(ref h) = *current {
                *handler_lock.write().await = Some(Arc::clone(h));
            }
        }

        // Spawn listener task
        let task = tokio::spawn(Self::listener_task(
            actual_port,
            listener,
            router,
            network_manager,
            handler_lock,
            shutdown_rx,
        ));

        // Store handle
        let handle = ListenerHandle {
            port: actual_port,
            task,
            shutdown_tx,
        };

        let mut listeners = self.listeners.write().await;
        listeners.insert(actual_port, handle);

        Ok(actual_port)
    }

    /// Binds a TCP port for a service and starts listening.
    ///
    /// Unlike `bind_port`, this binds for raw TCP connections (not HTTP).
    /// Incoming connections are passed directly to the TCP connection handler.
    ///
    /// # Arguments
    ///
    /// * `port` - The port to bind (use 0 for OS-assigned port)
    /// * `service_id` - The service ID to route connections to
    /// * `service_name` - The service name for display and lookup
    ///
    /// # Returns
    ///
    /// Returns the actual bound port (useful when port 0 was requested).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The port is already bound to another service
    /// - The port cannot be bound (permission denied, in use, etc.)
    #[instrument(skip(self), fields(port, service_id, service_name))]
    pub async fn bind_tcp_port(
        &self,
        port: u16,
        service_id: String,
        service_name: String,
    ) -> Result<u16> {
        // Try to bind the TCP listener first to get actual port
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| DaemonError::PortBindError {
                port,
                reason: e.to_string(),
            })?;

        // Get the actual bound port (important when port 0 was requested)
        let actual_port = listener
            .local_addr()
            .map_err(|e| DaemonError::PortBindError {
                port,
                reason: e.to_string(),
            })?
            .port();

        // Register with router using TCP protocol (checks for conflicts)
        // If this fails, listener will be dropped automatically
        self.router
            .bind_with_protocol(
                actual_port,
                service_id.clone(),
                service_name.clone(),
                BindingProtocol::Tcp,
            )
            .await?;

        info!(port = actual_port, %service_id, %service_name, protocol = "tcp", "Bound TCP port for service");

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        // Clone what we need for the listener task
        let router = Arc::clone(&self.router);
        let network_manager = Arc::clone(&self.network_manager);
        let handler_lock = Arc::new(RwLock::new(None::<TcpConnectionHandler>));

        // Copy current handler reference
        {
            let current = self.tcp_connection_handler.read().await;
            if let Some(ref h) = *current {
                *handler_lock.write().await = Some(Arc::clone(h));
            }
        }

        // Spawn TCP listener task
        let task = tokio::spawn(Self::tcp_listener_task(
            actual_port,
            listener,
            router,
            network_manager,
            handler_lock,
            shutdown_rx,
        ));

        // Store handle
        let handle = ListenerHandle {
            port: actual_port,
            task,
            shutdown_tx,
        };

        let mut listeners = self.listeners.write().await;
        listeners.insert(actual_port, handle);

        Ok(actual_port)
    }

    /// Unbinds a port and stops listening.
    ///
    /// # Errors
    ///
    /// Returns an error if the port is not bound.
    #[instrument(skip(self), fields(port))]
    pub async fn unbind_port(&self, port: u16) -> Result<()> {
        // Remove from router first
        self.router.unbind(port).await?;

        // Stop the listener
        let mut listeners = self.listeners.write().await;
        if let Some(handle) = listeners.remove(&port) {
            handle.shutdown().await;
            info!(port, "Unbound port");
        }

        Ok(())
    }

    /// Unbinds all ports for a service.
    ///
    /// Returns the list of ports that were unbound.
    pub async fn unbind_service(&self, service_id: &str) -> Vec<u16> {
        let ports = self.router.unbind_service(service_id).await;

        let mut listeners = self.listeners.write().await;
        for port in &ports {
            if let Some(handle) = listeners.remove(port) {
                handle.shutdown().await;
            }
        }

        if !ports.is_empty() {
            info!(%service_id, ports = ?ports, "Unbound all ports for service");
        }

        ports
    }

    /// Returns list of currently bound ports.
    pub async fn bound_ports(&self) -> Vec<u16> {
        let listeners = self.listeners.read().await;
        listeners.keys().copied().collect()
    }

    /// Checks if a port is currently bound.
    pub async fn is_bound(&self, port: u16) -> bool {
        let listeners = self.listeners.read().await;
        listeners.contains_key(&port)
    }

    /// Returns all current port bindings.
    pub async fn list_bindings(&self) -> Vec<super::router::ServiceBinding> {
        self.router.list_bindings().await
    }

    /// Shuts down all listeners.
    pub async fn shutdown(&self) {
        let mut listeners = self.listeners.write().await;
        let handles: Vec<_> = listeners.drain().map(|(_, h)| h).collect();

        for handle in handles {
            handle.shutdown().await;
        }

        info!("All proxy listeners shut down");
    }

    /// The listener task that accepts connections and handles requests.
    async fn listener_task(
        port: u16,
        listener: TcpListener,
        router: SharedServiceRouter,
        network_manager: Arc<NetworkManager>,
        handler: Arc<RwLock<Option<RequestHandler>>>,
        mut shutdown: broadcast::Receiver<()>,
    ) {
        debug!(port, "Listener task started");

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            debug!(port, %addr, "Accepted connection");

                            let router = Arc::clone(&router);
                            let network_manager = Arc::clone(&network_manager);
                            let handler = Arc::clone(&handler);

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_connection(
                                    port,
                                    stream,
                                    router,
                                    network_manager,
                                    handler,
                                ).await {
                                    warn!(port, %addr, "Connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!(port, "Accept error: {}", e);
                        }
                    }
                }
                _ = shutdown.recv() => {
                    debug!(port, "Listener received shutdown signal");
                    break;
                }
            }
        }

        debug!(port, "Listener task stopped");
    }

    /// The TCP listener task that accepts connections and routes to TCP handlers.
    ///
    /// Unlike the HTTP listener task, this passes raw TCP streams to the handler
    /// without parsing them as HTTP (inetd model).
    async fn tcp_listener_task(
        port: u16,
        listener: TcpListener,
        router: SharedServiceRouter,
        network_manager: Arc<NetworkManager>,
        handler: Arc<RwLock<Option<TcpConnectionHandler>>>,
        mut shutdown: broadcast::Receiver<()>,
    ) {
        debug!(port, protocol = "tcp", "TCP listener task started");

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            debug!(port, %addr, protocol = "tcp", "Accepted TCP connection");

                            let router = Arc::clone(&router);
                            let network_manager = Arc::clone(&network_manager);
                            let handler = Arc::clone(&handler);

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_tcp_connection(
                                    port,
                                    stream,
                                    addr,
                                    router,
                                    network_manager,
                                    handler,
                                ).await {
                                    warn!(port, %addr, "TCP connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!(port, "TCP accept error: {}", e);
                        }
                    }
                }
                _ = shutdown.recv() => {
                    debug!(port, "TCP listener received shutdown signal");
                    break;
                }
            }
        }

        debug!(port, "TCP listener task stopped");
    }

    /// Handles a single TCP connection by routing to the TCP connection handler.
    async fn handle_tcp_connection(
        port: u16,
        stream: TcpStream,
        peer_addr: SocketAddr,
        router: SharedServiceRouter,
        network_manager: Arc<NetworkManager>,
        handler: Arc<RwLock<Option<TcpConnectionHandler>>>,
    ) -> Result<()> {
        // Look up the service for this port
        let Some(binding) = router.lookup(port).await else {
            warn!(port, "No service bound to TCP port");
            return Err(DaemonError::PortNotBound { port });
        };

        // Validate ingress - check if service allows external access
        let ingress = validate_ingress(&binding.service_id, &network_manager).await;
        if !ingress.is_allowed() {
            warn!(
                port,
                %peer_addr,
                service_id = %binding.service_id,
                "Ingress denied: service only allows internal access"
            );
            return Err(DaemonError::NetworkAccessDenied {
                service_id: binding.service_id.clone(),
                reason: "Service only allows internal access".to_string(),
            });
        }

        // Get the handler
        let handler_opt = {
            let guard = handler.read().await;
            guard.clone()
        };

        let Some(tcp_handler) = handler_opt else {
            warn!("No TCP connection handler configured");
            return Err(DaemonError::IoError(std::io::Error::other(
                "No TCP connection handler configured",
            )));
        };

        // Call the handler with the stream
        debug!(port, %peer_addr, service_id = %binding.service_id, "Routing TCP connection to service");
        tcp_handler(binding.service_id.clone(), stream, peer_addr).await
    }

    /// Handles a single HTTP connection.
    async fn handle_connection(
        port: u16,
        stream: tokio::net::TcpStream,
        router: SharedServiceRouter,
        network_manager: Arc<NetworkManager>,
        handler: Arc<RwLock<Option<RequestHandler>>>,
    ) -> Result<()> {
        let io = TokioIo::new(stream);

        let service = service_fn(move |req: Request<hyper::body::Incoming>| {
            let router = Arc::clone(&router);
            let network_manager = Arc::clone(&network_manager);
            let handler = Arc::clone(&handler);

            async move { Self::handle_request(port, req, router, network_manager, handler).await }
        });

        http1::Builder::new()
            .serve_connection(io, service)
            .await
            .map_err(|e| DaemonError::IoError(std::io::Error::other(e.to_string())))?;

        Ok(())
    }

    /// Creates an error response with the given status and message.
    ///
    /// This helper avoids `expect()` by using a fallback empty body if builder fails.
    fn error_response(status: u16, message: &str) -> Response<Full<Bytes>> {
        Response::builder()
            .status(status)
            .body(Full::new(Bytes::from(message.to_string())))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("Internal error"))))
    }

    /// Handles a single HTTP request.
    async fn handle_request(
        port: u16,
        req: Request<hyper::body::Incoming>,
        router: SharedServiceRouter,
        network_manager: Arc<NetworkManager>,
        handler: Arc<RwLock<Option<RequestHandler>>>,
    ) -> std::result::Result<Response<Full<Bytes>>, hyper::Error> {
        // Look up the service for this port
        let Some(binding) = router.lookup(port).await else {
            warn!(port, "No service bound to port");
            return Ok(Self::error_response(503, "Service unavailable"));
        };

        // Validate ingress - check if service allows external access
        let ingress = validate_ingress(&binding.service_id, &network_manager).await;
        if !ingress.is_allowed() {
            warn!(
                port,
                service_id = %binding.service_id,
                "Ingress denied: service only allows internal access"
            );
            return Ok(Self::error_response(
                403,
                "Forbidden: Service only allows internal access",
            ));
        }

        // Get the handler
        let handler_opt = {
            let guard = handler.read().await;
            guard.clone()
        };

        let Some(request_handler) = handler_opt else {
            warn!("No request handler configured");
            return Ok(Self::error_response(503, "Service unavailable"));
        };

        // Convert hyper request to our request type
        let method = req.method().to_string();
        let uri = req.uri().to_string();
        let mut headers = std::collections::HashMap::new();

        for (name, value) in req.headers() {
            if let Ok(v) = value.to_str() {
                headers.insert(name.to_string(), v.to_string());
            }
        }

        // Collect body
        let body = match req.into_body().collect().await {
            Ok(b) => b.to_bytes(),
            Err(e) => {
                error!("Failed to read request body: {}", e);
                return Ok(Self::error_response(400, "Bad request"));
            }
        };

        let http_request = fabricks_runtime::HttpRequest {
            method,
            uri,
            headers,
            body: Bytes::from(body.to_vec()),
            scheme: fabricks_runtime::Scheme::Http,
            authority: None,
        };

        // Call the handler
        match request_handler(binding.service_id.clone(), http_request).await {
            Ok(response) => {
                let mut builder = Response::builder().status(response.status);

                for (name, value) in &response.headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }

                Ok(builder
                    .body(Full::new(response.body))
                    .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))))
            }
            Err(e) => {
                error!(service_id = %binding.service_id, "Handler error: {}", e);
                Ok(Self::error_response(500, "Internal server error"))
            }
        }
    }
}

impl std::fmt::Debug for ProxyServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyServer")
            .field("router", &self.router)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::ServiceRegistry;
    use crate::proxy::router::ServiceRouter;
    use crate::store::StateStore;
    use tempfile::tempdir;

    fn create_test_network_manager() -> Arc<NetworkManager> {
        let dir = tempdir().expect("should create temp dir");
        let db = sled::open(dir.path().join("test.db")).expect("should open db");
        let state_store = Arc::new(StateStore::new(Arc::new(db)));
        let registry = Arc::new(ServiceRegistry::new());
        Arc::new(NetworkManager::new(state_store, registry))
    }

    fn create_test_server() -> ProxyServer {
        let router = Arc::new(ServiceRouter::new());
        let network_manager = create_test_network_manager();
        ProxyServer::new(router, network_manager)
    }

    #[tokio::test]
    async fn test_proxy_server_creation() {
        let server = create_test_server();

        assert!(server.bound_ports().await.is_empty());
    }

    #[tokio::test]
    async fn test_bind_unbind_port() {
        let server = create_test_server();

        // Bind port (returns actual port)
        let port = server
            .bind_port(0, "svc-123".to_string(), "my-service".to_string())
            .await
            .expect("should bind");

        // The port should be non-zero (OS assigned)
        assert!(port > 0);

        // There should be one bound port
        let ports = server.bound_ports().await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0], port);

        // Unbind
        server.unbind_port(port).await.expect("should unbind");

        assert!(server.bound_ports().await.is_empty());
    }

    #[tokio::test]
    async fn test_unbind_service() {
        let server = create_test_server();

        // Bind multiple ports for same service
        let port1 = server
            .bind_port(0, "svc-1".to_string(), "service-one".to_string())
            .await
            .expect("should bind first");
        let port2 = server
            .bind_port(0, "svc-1".to_string(), "service-one".to_string())
            .await
            .expect("should bind second");

        // Both should be different ports
        assert_ne!(port1, port2);
        assert_eq!(server.bound_ports().await.len(), 2);

        // Unbind service
        let unbound = server.unbind_service("svc-1").await;
        assert_eq!(unbound.len(), 2);

        assert!(server.bound_ports().await.is_empty());
    }

    #[tokio::test]
    async fn test_shutdown() {
        let server = create_test_server();

        let _port1 = server
            .bind_port(0, "svc-1".to_string(), "service-one".to_string())
            .await
            .expect("should bind first");
        let _port2 = server
            .bind_port(0, "svc-2".to_string(), "service-two".to_string())
            .await
            .expect("should bind second");

        assert_eq!(server.bound_ports().await.len(), 2);

        server.shutdown().await;

        assert!(server.bound_ports().await.is_empty());
    }

    #[tokio::test]
    async fn test_bind_tcp_port() {
        let server = create_test_server();

        // Bind TCP port (returns actual port)
        let port = server
            .bind_tcp_port(0, "tcp-svc-123".to_string(), "my-tcp-service".to_string())
            .await
            .expect("should bind TCP port");

        // The port should be non-zero (OS assigned)
        assert!(port > 0);

        // There should be one bound port
        let ports = server.bound_ports().await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0], port);

        // Check that router has the binding with TCP protocol
        let bindings = server.list_bindings().await;
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].protocol, BindingProtocol::Tcp);
        assert_eq!(bindings[0].service_id, "tcp-svc-123");

        // Unbind
        server.unbind_port(port).await.expect("should unbind");

        assert!(server.bound_ports().await.is_empty());
    }

    #[tokio::test]
    async fn test_mixed_http_tcp_ports() {
        let server = create_test_server();

        // Bind HTTP port
        let http_port = server
            .bind_port(0, "http-svc".to_string(), "http-service".to_string())
            .await
            .expect("should bind HTTP port");

        // Bind TCP port
        let tcp_port = server
            .bind_tcp_port(0, "tcp-svc".to_string(), "tcp-service".to_string())
            .await
            .expect("should bind TCP port");

        // Both ports should be bound
        let ports = server.bound_ports().await;
        assert_eq!(ports.len(), 2);

        // Check bindings have correct protocols
        let bindings = server.list_bindings().await;
        assert_eq!(bindings.len(), 2);

        let http_binding = bindings.iter().find(|b| b.port == http_port);
        let tcp_binding = bindings.iter().find(|b| b.port == tcp_port);

        assert!(http_binding.is_some());
        assert!(tcp_binding.is_some());
        assert_eq!(
            http_binding.expect("has http binding").protocol,
            BindingProtocol::Http
        );
        assert_eq!(
            tcp_binding.expect("has tcp binding").protocol,
            BindingProtocol::Tcp
        );

        // Shutdown
        server.shutdown().await;
        assert!(server.bound_ports().await.is_empty());
    }
}
