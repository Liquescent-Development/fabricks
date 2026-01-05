//! Service registry for internal name resolution.
//!
//! Provides name-to-service-ID resolution for service-to-service communication.
//! The registry is authoritative - only explicitly registered service names
//! are considered internal services.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::debug;

/// Service registry for internal name resolution.
///
/// Maps service names to service IDs, allowing services to communicate
/// by name rather than by ID. The registry is the single source of truth
/// for what constitutes an "internal" service.
#[derive(Debug)]
pub struct ServiceRegistry {
    /// Map of service name to service ID.
    name_to_id: RwLock<HashMap<String, String>>,

    /// Map of service ID to service name (reverse lookup).
    id_to_name: RwLock<HashMap<String, String>>,
}

impl ServiceRegistry {
    /// Creates a new service registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name_to_id: RwLock::new(HashMap::new()),
            id_to_name: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a service name to ID mapping.
    pub async fn register(&self, name: String, id: String) {
        debug!(name = %name, id = %id, "Registering service");

        let mut name_to_id = self.name_to_id.write().await;
        let mut id_to_name = self.id_to_name.write().await;

        name_to_id.insert(name.clone(), id.clone());
        id_to_name.insert(id, name);
    }

    /// Unregisters a service by ID.
    pub async fn unregister_by_id(&self, id: &str) {
        let mut id_to_name = self.id_to_name.write().await;

        if let Some(name) = id_to_name.remove(id) {
            debug!(name = %name, id = %id, "Unregistering service");
            let mut name_to_id = self.name_to_id.write().await;
            name_to_id.remove(&name);
        }
    }

    /// Unregisters a service by name.
    pub async fn unregister_by_name(&self, name: &str) {
        let mut name_to_id = self.name_to_id.write().await;

        if let Some(id) = name_to_id.remove(name) {
            debug!(name = %name, id = %id, "Unregistering service");
            let mut id_to_name = self.id_to_name.write().await;
            id_to_name.remove(&id);
        }
    }

    /// Resolves a service name to its ID.
    ///
    /// Returns `None` if the service name is not registered.
    /// This is the authoritative check for whether a hostname is an internal service.
    pub async fn resolve(&self, name: &str) -> Option<String> {
        let name_to_id = self.name_to_id.read().await;
        name_to_id.get(name).cloned()
    }

    /// Looks up a service ID to get its name.
    ///
    /// Returns `None` if the service ID is not registered.
    pub async fn reverse_lookup(&self, id: &str) -> Option<String> {
        let id_to_name = self.id_to_name.read().await;
        id_to_name.get(id).cloned()
    }

    /// Checks if a service name is registered (i.e., is an internal service).
    pub async fn is_internal_service(&self, name: &str) -> bool {
        let name_to_id = self.name_to_id.read().await;
        name_to_id.contains_key(name)
    }

    /// Returns the number of registered services.
    pub async fn count(&self) -> usize {
        let name_to_id = self.name_to_id.read().await;
        name_to_id.len()
    }

    /// Returns all registered service names.
    pub async fn all_names(&self) -> Vec<String> {
        let name_to_id = self.name_to_id.read().await;
        name_to_id.keys().cloned().collect()
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared reference to a service registry.
pub type SharedServiceRegistry = Arc<ServiceRegistry>;

/// Extracts the service name from a host:port string.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(extract_service_name("api-service:8080"), "api-service");
/// assert_eq!(extract_service_name("api-service"), "api-service");
/// ```
#[must_use]
pub fn extract_service_name(host: &str) -> &str {
    host.split(':').next().unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_register_resolve() {
        let registry = ServiceRegistry::new();

        registry
            .register("api-service".to_string(), "svc-123".to_string())
            .await;

        let resolved = registry.resolve("api-service").await;
        assert_eq!(resolved, Some("svc-123".to_string()));

        let not_found = registry.resolve("unknown").await;
        assert_eq!(not_found, None);
    }

    #[tokio::test]
    async fn test_registry_reverse_lookup() {
        let registry = ServiceRegistry::new();

        registry
            .register("api-service".to_string(), "svc-123".to_string())
            .await;

        let name = registry.reverse_lookup("svc-123").await;
        assert_eq!(name, Some("api-service".to_string()));
    }

    #[tokio::test]
    async fn test_registry_unregister_by_id() {
        let registry = ServiceRegistry::new();

        registry
            .register("api-service".to_string(), "svc-123".to_string())
            .await;
        assert!(registry.is_internal_service("api-service").await);

        registry.unregister_by_id("svc-123").await;
        assert!(!registry.is_internal_service("api-service").await);
    }

    #[tokio::test]
    async fn test_registry_unregister_by_name() {
        let registry = ServiceRegistry::new();

        registry
            .register("api-service".to_string(), "svc-123".to_string())
            .await;

        registry.unregister_by_name("api-service").await;
        assert!(!registry.is_internal_service("api-service").await);
        assert!(registry.reverse_lookup("svc-123").await.is_none());
    }

    #[tokio::test]
    async fn test_registry_is_internal_service() {
        let registry = ServiceRegistry::new();

        // Not registered = not internal
        assert!(!registry.is_internal_service("api-service").await);

        // Registered = internal
        registry
            .register("api-service".to_string(), "svc-123".to_string())
            .await;
        assert!(registry.is_internal_service("api-service").await);

        // External hostnames are never internal (not registered)
        assert!(!registry.is_internal_service("api.example.com").await);
    }

    #[tokio::test]
    async fn test_registry_count() {
        let registry = ServiceRegistry::new();

        assert_eq!(registry.count().await, 0);

        registry
            .register("svc-a".to_string(), "id-a".to_string())
            .await;
        registry
            .register("svc-b".to_string(), "id-b".to_string())
            .await;

        assert_eq!(registry.count().await, 2);
    }

    #[test]
    fn test_extract_service_name() {
        assert_eq!(extract_service_name("api-service"), "api-service");
        assert_eq!(extract_service_name("api-service:8080"), "api-service");
        assert_eq!(extract_service_name("api-service:"), "api-service");
        assert_eq!(extract_service_name(""), "");
    }
}
