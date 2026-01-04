//! Egress proxy for outbound HTTP requests from WASM modules.
//!
//! This module handles all outbound HTTP connections from WASM services.
//! It validates connections against capabilities and network policies,
//! then either routes to internal services or executes external requests.

use std::sync::Arc;

use bytes::Bytes;
use reqwest::Client;
use tracing::{debug, error, warn};

use fabricks_common::models::capability::Capabilities;

use crate::error::{DaemonError, Result};
use crate::network::{validate_connection, ConnectionDecision, NetworkManager};

/// Request to be executed by the egress proxy.
#[derive(Debug, Clone)]
pub struct EgressRequest {
    /// HTTP method.
    pub method: String,
    /// Full URL (scheme://host:port/path).
    pub url: String,
    /// Request headers.
    pub headers: Vec<(String, String)>,
    /// Request body.
    pub body: Option<Bytes>,
}

impl EgressRequest {
    /// Creates a new egress request.
    #[must_use]
    pub fn new(method: String, url: String) -> Self {
        Self {
            method,
            url,
            headers: Vec::new(),
            body: None,
        }
    }

    /// Adds a header to the request.
    #[must_use]
    pub fn with_header(mut self, name: String, value: String) -> Self {
        self.headers.push((name, value));
        self
    }

    /// Sets the request body.
    #[must_use]
    pub fn with_body(mut self, body: Bytes) -> Self {
        self.body = Some(body);
        self
    }

    /// Extracts the host and port from the URL.
    ///
    /// Returns `None` if the URL cannot be parsed.
    #[must_use]
    pub fn host_port(&self) -> Option<(String, u16)> {
        let url = reqwest::Url::parse(&self.url).ok()?;
        let host = url.host_str()?.to_string();
        let port = url.port_or_known_default()?;
        Some((host, port))
    }
}

/// Response from the egress proxy.
#[derive(Debug, Clone)]
pub struct EgressResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers.
    pub headers: Vec<(String, String)>,
    /// Response body.
    pub body: Bytes,
}

impl EgressResponse {
    /// Creates a new egress response.
    #[must_use]
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Bytes::new(),
        }
    }

    /// Adds a header to the response.
    #[must_use]
    pub fn with_header(mut self, name: String, value: String) -> Self {
        self.headers.push((name, value));
        self
    }

    /// Sets the response body.
    #[must_use]
    pub fn with_body(mut self, body: Bytes) -> Self {
        self.body = body;
        self
    }

    /// Creates an error response with the given status and message.
    #[must_use]
    pub fn error(status: u16, message: &str) -> Self {
        Self {
            status,
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body: Bytes::from(message.to_string()),
        }
    }
}

/// Callback for routing requests to internal services.
///
/// When the egress proxy determines a request is for an internal service,
/// it uses this callback to route the request to the target service.
pub type InternalRouteHandler =
    Arc<dyn Fn(String, EgressRequest) -> InternalRouteFuture + Send + Sync>;

/// Future returned by the internal route handler.
pub type InternalRouteFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<EgressResponse>> + Send>>;

/// Egress proxy for handling outbound HTTP requests.
///
/// Validates and executes outbound HTTP connections from WASM services.
/// Internal service requests are routed through a callback; external
/// requests are executed via an HTTP client.
pub struct EgressProxy {
    /// HTTP client for external requests.
    client: Client,
    /// Network manager for connection validation.
    network_manager: Arc<NetworkManager>,
    /// Handler for routing to internal services.
    internal_route_handler: Option<InternalRouteHandler>,
}

impl EgressProxy {
    /// Creates a new egress proxy.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(network_manager: Arc<NetworkManager>) -> Result<Self> {
        let client = Client::builder()
            .build()
            .map_err(|e| DaemonError::IoError(std::io::Error::other(e.to_string())))?;

        Ok(Self {
            client,
            network_manager,
            internal_route_handler: None,
        })
    }

    /// Sets the handler for internal service routing.
    pub fn set_internal_route_handler(&mut self, handler: InternalRouteHandler) {
        self.internal_route_handler = Some(handler);
    }

    /// Executes an outbound HTTP request.
    ///
    /// Validates the connection against capabilities and network policies,
    /// then routes to an internal service or executes externally.
    ///
    /// # Arguments
    ///
    /// * `from_service_id` - The service making the request
    /// * `capabilities` - The service's capability grants
    /// * `request` - The request to execute
    ///
    /// # Returns
    ///
    /// The response from the target, or an error response if denied.
    pub async fn execute(
        &self,
        from_service_id: &str,
        capabilities: &Option<Capabilities>,
        request: EgressRequest,
    ) -> EgressResponse {
        // Extract host and port from URL
        let Some((host, port)) = request.host_port() else {
            warn!(url = %request.url, "Invalid URL in egress request");
            return EgressResponse::error(400, "Invalid URL");
        };

        debug!(
            from = %from_service_id,
            host = %host,
            port = %port,
            method = %request.method,
            "Processing egress request"
        );

        // Validate the connection
        let decision = validate_connection(
            from_service_id,
            capabilities,
            &host,
            port,
            &self.network_manager,
        )
        .await;

        match decision {
            ConnectionDecision::AllowInternal { service_id } => {
                self.route_internal(&service_id, request).await
            }
            ConnectionDecision::AllowExternal => self.execute_external(request).await,
            ConnectionDecision::DenyCapability { reason } => {
                warn!(
                    from = %from_service_id,
                    host = %host,
                    reason = %reason,
                    "Egress denied: capability"
                );
                EgressResponse::error(403, &format!("Forbidden: {reason}"))
            }
            ConnectionDecision::DenyNetwork { reason } => {
                warn!(
                    from = %from_service_id,
                    host = %host,
                    reason = %reason,
                    "Egress denied: network"
                );
                EgressResponse::error(403, &format!("Forbidden: {reason}"))
            }
            ConnectionDecision::DenyPolicy { reason } => {
                warn!(
                    from = %from_service_id,
                    host = %host,
                    reason = %reason,
                    "Egress denied: policy"
                );
                EgressResponse::error(403, &format!("Forbidden: {reason}"))
            }
        }
    }

    /// Routes a request to an internal service.
    async fn route_internal(&self, service_id: &str, request: EgressRequest) -> EgressResponse {
        debug!(
            service_id = %service_id,
            method = %request.method,
            url = %request.url,
            "Routing to internal service"
        );

        let Some(handler) = &self.internal_route_handler else {
            error!("Internal route handler not configured");
            return EgressResponse::error(503, "Internal routing not available");
        };

        match handler(service_id.to_string(), request).await {
            Ok(response) => response,
            Err(e) => {
                error!(error = %e, "Internal routing failed");
                EgressResponse::error(502, &format!("Bad Gateway: {e}"))
            }
        }
    }

    /// Executes an external HTTP request.
    async fn execute_external(&self, request: EgressRequest) -> EgressResponse {
        debug!(
            method = %request.method,
            url = %request.url,
            "Executing external request"
        );

        // Build the request
        let method = match request.method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            "HEAD" => reqwest::Method::HEAD,
            "OPTIONS" => reqwest::Method::OPTIONS,
            other => {
                warn!(method = %other, "Unsupported HTTP method");
                return EgressResponse::error(400, &format!("Unsupported method: {other}"));
            }
        };

        let mut req_builder = self.client.request(method, &request.url);

        // Add headers
        for (name, value) in &request.headers {
            req_builder = req_builder.header(name, value);
        }

        // Add body
        if let Some(body) = request.body {
            req_builder = req_builder.body(body);
        }

        // Execute the request
        let response = match req_builder.send().await {
            Ok(resp) => resp,
            Err(e) => {
                error!(error = %e, url = %request.url, "External request failed");
                return EgressResponse::error(502, &format!("Bad Gateway: {e}"));
            }
        };

        // Convert the response
        let status = response.status().as_u16();
        let mut headers = Vec::new();

        for (name, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.push((name.to_string(), value_str.to_string()));
            }
        }

        let body = match response.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!(error = %e, "Failed to read response body");
                return EgressResponse::error(502, &format!("Failed to read response: {e}"));
            }
        };

        EgressResponse {
            status,
            headers,
            body,
        }
    }
}

impl std::fmt::Debug for EgressProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EgressProxy")
            .field("has_internal_handler", &self.internal_route_handler.is_some())
            .finish_non_exhaustive()
    }
}

/// Shared reference to an egress proxy.
pub type SharedEgressProxy = Arc<EgressProxy>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::ServiceRegistry;
    use crate::network::NetworkConfig;
    use crate::store::StateStore;
    use fabricks_common::models::capability::NetworkCapabilities;
    use tempfile::tempdir;

    fn create_test_network_manager() -> Arc<NetworkManager> {
        let dir = tempdir().expect("should create temp dir");
        let db = sled::open(dir.path().join("test.db")).expect("should open db");
        let state_store = Arc::new(StateStore::new(Arc::new(db)));
        let registry = Arc::new(ServiceRegistry::new());
        Arc::new(NetworkManager::new(state_store, registry))
    }

    fn capabilities_for_connect(targets: Vec<&str>) -> Option<Capabilities> {
        Some(Capabilities {
            env: None,
            network: Some(NetworkCapabilities {
                connect: Some(targets.into_iter().map(String::from).collect()),
                listen: None,
                allow_all_outbound: None,
            }),
            filesystem: None,
            wasm: None,
        })
    }

    #[test]
    fn test_egress_request_host_port() {
        let req = EgressRequest::new("GET".to_string(), "https://api.example.com:443/v1".to_string());
        let (host, port) = req.host_port().unwrap();
        assert_eq!(host, "api.example.com");
        assert_eq!(port, 443);

        let req2 = EgressRequest::new("GET".to_string(), "http://localhost:8080/".to_string());
        let (host2, port2) = req2.host_port().unwrap();
        assert_eq!(host2, "localhost");
        assert_eq!(port2, 8080);
    }

    #[test]
    fn test_egress_request_default_port() {
        let req = EgressRequest::new("GET".to_string(), "https://api.example.com/v1".to_string());
        let (host, port) = req.host_port().unwrap();
        assert_eq!(host, "api.example.com");
        assert_eq!(port, 443);

        let req2 = EgressRequest::new("GET".to_string(), "http://example.com/".to_string());
        let (host2, port2) = req2.host_port().unwrap();
        assert_eq!(host2, "example.com");
        assert_eq!(port2, 80);
    }

    #[test]
    fn test_egress_request_builder() {
        let req = EgressRequest::new("POST".to_string(), "https://api.example.com/".to_string())
            .with_header("Content-Type".to_string(), "application/json".to_string())
            .with_body(Bytes::from(r#"{"key":"value"}"#));

        assert_eq!(req.method, "POST");
        assert_eq!(req.headers.len(), 1);
        assert!(req.body.is_some());
    }

    #[test]
    fn test_egress_response_builder() {
        let resp = EgressResponse::new(200)
            .with_header("Content-Type".to_string(), "application/json".to_string())
            .with_body(Bytes::from(r#"{"result":"ok"}"#));

        assert_eq!(resp.status, 200);
        assert_eq!(resp.headers.len(), 1);
        assert!(!resp.body.is_empty());
    }

    #[test]
    fn test_egress_response_error() {
        let resp = EgressResponse::error(403, "Access denied");
        assert_eq!(resp.status, 403);
        assert_eq!(resp.body, Bytes::from("Access denied"));
    }

    #[tokio::test]
    async fn test_egress_proxy_deny_no_capability() {
        let manager = create_test_network_manager();
        let proxy = EgressProxy::new(manager).unwrap();

        let request = EgressRequest::new(
            "GET".to_string(),
            "https://api.example.com/v1".to_string(),
        );

        let response = proxy.execute("svc-1", &None, request).await;

        assert_eq!(response.status, 403);
        assert!(String::from_utf8_lossy(&response.body).contains("Forbidden"));
    }

    #[tokio::test]
    async fn test_egress_proxy_deny_capability_not_granted() {
        let manager = create_test_network_manager();
        let proxy = EgressProxy::new(manager).unwrap();

        let caps = capabilities_for_connect(vec!["other.example.com:443"]);
        let request = EgressRequest::new(
            "GET".to_string(),
            "https://api.example.com/v1".to_string(),
        );

        let response = proxy.execute("svc-1", &caps, request).await;

        assert_eq!(response.status, 403);
    }

    #[tokio::test]
    async fn test_egress_proxy_internal_no_handler() {
        let manager = create_test_network_manager();

        // Create network and add services
        let config = NetworkConfig::new("test-net".to_string());
        let net_id = manager.create_network(config).await.unwrap();
        manager
            .add_service(&net_id, "svc-a", "service-a")
            .await
            .unwrap();
        manager
            .add_service(&net_id, "svc-b", "service-b")
            .await
            .unwrap();

        let proxy = EgressProxy::new(manager).unwrap();
        let caps = capabilities_for_connect(vec!["service-b:8080"]);

        let request = EgressRequest::new(
            "GET".to_string(),
            "http://service-b:8080/api".to_string(),
        );

        let response = proxy.execute("svc-a", &caps, request).await;

        // Should fail because internal handler not configured
        assert_eq!(response.status, 503);
        assert!(String::from_utf8_lossy(&response.body).contains("not available"));
    }

    #[tokio::test]
    async fn test_egress_proxy_internal_with_handler() {
        let manager = create_test_network_manager();

        // Create network and add services
        let config = NetworkConfig::new("test-net".to_string());
        let net_id = manager.create_network(config).await.unwrap();
        manager
            .add_service(&net_id, "svc-a", "service-a")
            .await
            .unwrap();
        manager
            .add_service(&net_id, "svc-b", "service-b")
            .await
            .unwrap();

        let mut proxy = EgressProxy::new(manager).unwrap();

        // Set up a mock internal handler
        let handler: InternalRouteHandler = Arc::new(|_service_id, _request| {
            Box::pin(async move {
                Ok(EgressResponse::new(200).with_body(Bytes::from("internal response")))
            })
        });
        proxy.set_internal_route_handler(handler);

        let caps = capabilities_for_connect(vec!["service-b:8080"]);
        let request = EgressRequest::new(
            "GET".to_string(),
            "http://service-b:8080/api".to_string(),
        );

        let response = proxy.execute("svc-a", &caps, request).await;

        assert_eq!(response.status, 200);
        assert_eq!(response.body, Bytes::from("internal response"));
    }
}
