//! HTTP proxy server for routing requests to WASM services.
//!
//! The `ProxyServer` binds TCP listeners on configured ports and routes
//! incoming HTTP requests to the appropriate WASM service via the HTTP runtime.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument, warn};

use crate::error::{DaemonError, Result};

use super::router::{ServiceRouter, SharedServiceRouter};

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
pub type RequestHandler = Arc<
    dyn Fn(String, fabricks_runtime::HttpRequest) -> RequestFuture + Send + Sync,
>;

/// Future returned by the request handler.
pub type RequestFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<fabricks_runtime::HttpResponse>> + Send>,
>;

/// HTTP proxy server that manages port listeners and routes requests.
pub struct ProxyServer {
    /// Port to listener handle map.
    listeners: RwLock<HashMap<u16, ListenerHandle>>,

    /// Service router for port-to-service mapping.
    router: SharedServiceRouter,

    /// Request handler callback.
    request_handler: RwLock<Option<RequestHandler>>,
}

impl ProxyServer {
    /// Creates a new proxy server.
    #[must_use]
    pub fn new(router: SharedServiceRouter) -> Self {
        Self {
            listeners: RwLock::new(HashMap::new()),
            router,
            request_handler: RwLock::new(None),
        }
    }

    /// Creates a new proxy server with a new router.
    #[must_use]
    pub fn with_new_router() -> Self {
        Self::new(Arc::new(ServiceRouter::new()))
    }

    /// Sets the request handler callback.
    ///
    /// This callback is invoked for each incoming request with the service ID
    /// and request details. The callback should route the request to the
    /// appropriate WASM runtime and return the response.
    pub async fn set_request_handler(&self, handler: RequestHandler) {
        let mut guard = self.request_handler.write().await;
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
    pub async fn bind_port(&self, port: u16, service_id: String, service_name: String) -> Result<u16> {
        // Try to bind the TCP listener first to get actual port
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            DaemonError::PortBindError {
                port,
                reason: e.to_string(),
            }
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
                            let handler = Arc::clone(&handler);

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_connection(
                                    port,
                                    stream,
                                    router,
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

    /// Handles a single connection.
    async fn handle_connection(
        port: u16,
        stream: tokio::net::TcpStream,
        router: SharedServiceRouter,
        handler: Arc<RwLock<Option<RequestHandler>>>,
    ) -> Result<()> {
        let io = TokioIo::new(stream);

        let service = service_fn(move |req: Request<hyper::body::Incoming>| {
            let router = Arc::clone(&router);
            let handler = Arc::clone(&handler);

            async move {
                Self::handle_request(port, req, router, handler).await
            }
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
            .unwrap_or_else(|_| {
                Response::new(Full::new(Bytes::from("Internal error")))
            })
    }

    /// Handles a single HTTP request.
    async fn handle_request(
        port: u16,
        req: Request<hyper::body::Incoming>,
        router: SharedServiceRouter,
        handler: Arc<RwLock<Option<RequestHandler>>>,
    ) -> std::result::Result<Response<Full<Bytes>>, hyper::Error> {
        // Look up the service for this port
        let Some(binding) = router.lookup(port).await else {
            warn!(port, "No service bound to port");
            return Ok(Self::error_response(503, "Service unavailable"));
        };

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

    #[tokio::test]
    async fn test_proxy_server_creation() {
        let router = Arc::new(ServiceRouter::new());
        let server = ProxyServer::new(router);

        assert!(server.bound_ports().await.is_empty());
    }

    #[tokio::test]
    async fn test_bind_unbind_port() {
        let server = ProxyServer::with_new_router();

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
        let server = ProxyServer::with_new_router();

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
        let server = ProxyServer::with_new_router();

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
}
