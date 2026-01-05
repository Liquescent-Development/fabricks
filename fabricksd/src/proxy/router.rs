//! Service router for mapping ports to services and routing requests.
//!
//! The router maintains a mapping of bound ports to service IDs and handles
//! request routing with load balancing across service instances.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, instrument};

use crate::error::{DaemonError, Result};

use super::loadbalancer::LoadBalancer;

/// Protocol type for port binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingProtocol {
    /// HTTP protocol - requests are parsed as HTTP and routed to handlers.
    #[default]
    Http,

    /// Raw TCP protocol - connections are passed directly to handlers.
    Tcp,
}

impl std::fmt::Display for BindingProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http => write!(f, "http"),
            Self::Tcp => write!(f, "tcp"),
        }
    }
}

/// Binding information for a port.
#[derive(Debug, Clone)]
pub struct ServiceBinding {
    /// Service ID bound to this port.
    pub service_id: String,

    /// Service name for display.
    pub service_name: String,

    /// The port being listened on.
    pub port: u16,

    /// The protocol for this binding.
    pub protocol: BindingProtocol,
}

/// Router for directing requests to services.
///
/// Maintains a mapping of ports to service bindings and handles load
/// balancing across instances.
pub struct ServiceRouter {
    /// Port to service binding map.
    bindings: RwLock<HashMap<u16, ServiceBinding>>,

    /// Load balancer for instance selection.
    load_balancer: LoadBalancer,
}

impl ServiceRouter {
    /// Creates a new service router.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: RwLock::new(HashMap::new()),
            load_balancer: LoadBalancer::round_robin(),
        }
    }

    /// Creates a new router with a custom load balancer.
    #[must_use]
    pub fn with_load_balancer(load_balancer: LoadBalancer) -> Self {
        Self {
            bindings: RwLock::new(HashMap::new()),
            load_balancer,
        }
    }

    /// Registers a service binding for a port with the default HTTP protocol.
    ///
    /// # Errors
    ///
    /// Returns an error if the port is already bound to another service.
    #[instrument(skip(self), fields(port, service_id, service_name))]
    pub async fn bind(&self, port: u16, service_id: String, service_name: String) -> Result<()> {
        self.bind_with_protocol(port, service_id, service_name, BindingProtocol::Http)
            .await
    }

    /// Registers a service binding for a port with a specific protocol.
    ///
    /// # Errors
    ///
    /// Returns an error if the port is already bound to another service.
    #[instrument(skip(self), fields(port, service_id, service_name, %protocol))]
    pub async fn bind_with_protocol(
        &self,
        port: u16,
        service_id: String,
        service_name: String,
        protocol: BindingProtocol,
    ) -> Result<()> {
        let mut bindings = self.bindings.write().await;

        if let Some(existing) = bindings.get(&port) {
            return Err(DaemonError::PortAlreadyBound {
                port,
                service_id: existing.service_id.clone(),
            });
        }

        debug!(port, %service_id, %service_name, %protocol, "Binding port to service");

        bindings.insert(
            port,
            ServiceBinding {
                service_id,
                service_name,
                port,
                protocol,
            },
        );

        Ok(())
    }

    /// Removes a port binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the port is not bound.
    #[instrument(skip(self), fields(port))]
    pub async fn unbind(&self, port: u16) -> Result<()> {
        let mut bindings = self.bindings.write().await;

        if bindings.remove(&port).is_none() {
            return Err(DaemonError::PortNotBound { port });
        }

        debug!(port, "Unbound port");
        Ok(())
    }

    /// Looks up the service bound to a port.
    ///
    /// Returns `None` if no service is bound to the port.
    pub async fn lookup(&self, port: u16) -> Option<ServiceBinding> {
        let bindings = self.bindings.read().await;
        bindings.get(&port).cloned()
    }

    /// Returns all current bindings.
    pub async fn list_bindings(&self) -> Vec<ServiceBinding> {
        let bindings = self.bindings.read().await;
        bindings.values().cloned().collect()
    }

    /// Returns bindings for a specific service.
    pub async fn bindings_for_service(&self, service_id: &str) -> Vec<ServiceBinding> {
        let bindings = self.bindings.read().await;
        bindings
            .values()
            .filter(|b| b.service_id == service_id)
            .cloned()
            .collect()
    }

    /// Removes all bindings for a service.
    ///
    /// Returns the list of ports that were unbound.
    pub async fn unbind_service(&self, service_id: &str) -> Vec<u16> {
        let mut bindings = self.bindings.write().await;
        let ports: Vec<u16> = bindings
            .iter()
            .filter(|(_, b)| b.service_id == service_id)
            .map(|(p, _)| *p)
            .collect();

        for port in &ports {
            bindings.remove(port);
        }

        if !ports.is_empty() {
            debug!(%service_id, ports = ?ports, "Unbound all ports for service");
        }

        ports
    }

    /// Selects an instance index for load balancing.
    ///
    /// Returns the index of the instance to route to.
    #[must_use]
    pub fn select_instance(&self, instance_count: usize) -> Option<usize> {
        self.load_balancer.select(instance_count)
    }

    /// Checks if a port is currently bound.
    pub async fn is_bound(&self, port: u16) -> bool {
        let bindings = self.bindings.read().await;
        bindings.contains_key(&port)
    }
}

impl Default for ServiceRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ServiceRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceRouter")
            .field("load_balancer", &self.load_balancer)
            .finish_non_exhaustive()
    }
}

/// Shared service router.
pub type SharedServiceRouter = Arc<ServiceRouter>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bind_and_lookup() {
        let router = ServiceRouter::new();

        router
            .bind(8080, "svc-123".to_string(), "my-service".to_string())
            .await
            .expect("should bind");

        let binding = router.lookup(8080).await;
        assert!(binding.is_some());
        assert_eq!(binding.as_ref().expect("has binding").service_id, "svc-123");
        assert_eq!(binding.as_ref().expect("has binding").service_name, "my-service");
        assert_eq!(binding.expect("has binding").port, 8080);
    }

    #[tokio::test]
    async fn test_bind_duplicate_port() {
        let router = ServiceRouter::new();

        router
            .bind(8080, "svc-123".to_string(), "my-service".to_string())
            .await
            .expect("should bind");

        let result = router.bind(8080, "svc-456".to_string(), "other-service".to_string()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DaemonError::PortAlreadyBound { port: 8080, .. }
        ));
    }

    #[tokio::test]
    async fn test_unbind() {
        let router = ServiceRouter::new();

        router
            .bind(8080, "svc-123".to_string(), "my-service".to_string())
            .await
            .expect("should bind");

        router.unbind(8080).await.expect("should unbind");

        let binding = router.lookup(8080).await;
        assert!(binding.is_none());
    }

    #[tokio::test]
    async fn test_unbind_not_bound() {
        let router = ServiceRouter::new();

        let result = router.unbind(9999).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DaemonError::PortNotBound { port: 9999 }
        ));
    }

    #[tokio::test]
    async fn test_list_bindings() {
        let router = ServiceRouter::new();

        router
            .bind(8080, "svc-1".to_string(), "service-one".to_string())
            .await
            .expect("should bind");
        router
            .bind(8081, "svc-2".to_string(), "service-two".to_string())
            .await
            .expect("should bind");

        let bindings = router.list_bindings().await;
        assert_eq!(bindings.len(), 2);
    }

    #[tokio::test]
    async fn test_unbind_service() {
        let router = ServiceRouter::new();

        router
            .bind(8080, "svc-1".to_string(), "service-one".to_string())
            .await
            .expect("should bind");
        router
            .bind(8081, "svc-1".to_string(), "service-one".to_string())
            .await
            .expect("should bind");
        router
            .bind(8082, "svc-2".to_string(), "service-two".to_string())
            .await
            .expect("should bind");

        let unbound = router.unbind_service("svc-1").await;
        assert_eq!(unbound.len(), 2);

        let bindings = router.list_bindings().await;
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].service_id, "svc-2");
    }

    #[tokio::test]
    async fn test_bindings_for_service() {
        let router = ServiceRouter::new();

        router
            .bind(8080, "svc-1".to_string(), "service-one".to_string())
            .await
            .expect("should bind");
        router
            .bind(8081, "svc-1".to_string(), "service-one".to_string())
            .await
            .expect("should bind");
        router
            .bind(8082, "svc-2".to_string(), "service-two".to_string())
            .await
            .expect("should bind");

        let bindings = router.bindings_for_service("svc-1").await;
        assert_eq!(bindings.len(), 2);

        let bindings = router.bindings_for_service("svc-2").await;
        assert_eq!(bindings.len(), 1);

        let bindings = router.bindings_for_service("svc-nonexistent").await;
        assert!(bindings.is_empty());
    }

    #[tokio::test]
    async fn test_is_bound() {
        let router = ServiceRouter::new();

        assert!(!router.is_bound(8080).await);

        router
            .bind(8080, "svc-1".to_string(), "service-one".to_string())
            .await
            .expect("should bind");

        assert!(router.is_bound(8080).await);
    }
}
