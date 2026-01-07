//! Network manager for network lifecycle and membership.
//!
//! Provides CRUD operations for networks and manages service membership
//! within networks. The manager is responsible for enforcing network
//! isolation policies.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::{DaemonError, Result};
use crate::store::StateStore;

use super::registry::{extract_service_name, SharedServiceRegistry};
use super::types::{NetworkConfig, NetworkDetail, NetworkInfo, NetworkState};

/// Network manager for network lifecycle and membership.
///
/// Manages the creation, deletion, and membership of networks.
/// Networks provide isolation between services - services can only
/// communicate if they share at least one network.
#[derive(Debug)]
pub struct NetworkManager {
    /// Active networks indexed by ID.
    networks: RwLock<HashMap<String, NetworkState>>,

    /// Service to networks mapping (`service_id` -> `network_ids`).
    service_networks: RwLock<HashMap<String, Vec<String>>>,

    /// Service registry for name resolution.
    registry: SharedServiceRegistry,

    /// Persistent state store.
    state_store: Arc<StateStore>,
}

impl NetworkManager {
    /// Creates a new network manager.
    #[must_use]
    pub fn new(state_store: Arc<StateStore>, registry: SharedServiceRegistry) -> Self {
        Self {
            networks: RwLock::new(HashMap::new()),
            service_networks: RwLock::new(HashMap::new()),
            registry,
            state_store,
        }
    }

    /// Creates a new network.
    ///
    /// # Errors
    ///
    /// Returns an error if a network with the same name already exists.
    pub async fn create_network(&self, config: NetworkConfig) -> Result<String> {
        let mut networks = self.networks.write().await;

        // Check for duplicate name
        for existing in networks.values() {
            if existing.name == config.name {
                return Err(DaemonError::NetworkExists(config.name));
            }
        }

        let state = NetworkState::new(config);
        let id = state.id.clone();

        info!(id = %id, name = %state.name, "Creating network");

        // Persist to state store
        self.persist_network(&state)?;

        networks.insert(id.clone(), state);

        Ok(id)
    }

    /// Deletes a network by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the network does not exist or has members.
    pub async fn delete_network(&self, id: &str) -> Result<()> {
        let mut networks = self.networks.write().await;

        let network = networks
            .get(id)
            .ok_or_else(|| DaemonError::NetworkNotFound(id.to_string()))?;

        // Don't allow deletion of networks with members
        if !network.members.is_empty() {
            return Err(DaemonError::NetworkHasMembers(id.to_string()));
        }

        info!(id = %id, name = %network.name, "Deleting network");

        // Remove from state store
        self.remove_network_state(id)?;

        networks.remove(id);

        Ok(())
    }

    /// Gets a network by ID.
    pub async fn get_network(&self, id: &str) -> Option<NetworkDetail> {
        let networks = self.networks.read().await;
        networks.get(id).map(NetworkDetail::from)
    }

    /// Gets a network by name.
    pub async fn get_network_by_name(&self, name: &str) -> Option<NetworkDetail> {
        let networks = self.networks.read().await;
        networks
            .values()
            .find(|n| n.name == name)
            .map(NetworkDetail::from)
    }

    /// Gets a network by ID or name.
    ///
    /// First tries to match by ID, then by name.
    pub async fn get_network_by_id_or_name(&self, id_or_name: &str) -> Option<NetworkDetail> {
        let networks = self.networks.read().await;

        // Try ID lookup first (exact match)
        if let Some(network) = networks.get(id_or_name) {
            return Some(NetworkDetail::from(network));
        }

        // Fall back to name lookup
        networks
            .values()
            .find(|n| n.name == id_or_name)
            .map(NetworkDetail::from)
    }

    /// Resolves a network ID or name to an ID.
    ///
    /// Returns the network ID if found, or None if not found.
    pub async fn resolve_network_id(&self, id_or_name: &str) -> Option<String> {
        let networks = self.networks.read().await;

        // Try ID lookup first (exact match)
        if networks.contains_key(id_or_name) {
            return Some(id_or_name.to_string());
        }

        // Fall back to name lookup
        networks
            .values()
            .find(|n| n.name == id_or_name)
            .map(|n| n.id.clone())
    }

    /// Lists all networks.
    pub async fn list_networks(&self) -> Vec<NetworkInfo> {
        let networks = self.networks.read().await;
        networks.values().map(NetworkInfo::from).collect()
    }

    /// Adds a service to a network.
    ///
    /// # Errors
    ///
    /// Returns an error if the network or service does not exist,
    /// or if the service is already a member.
    pub async fn add_service(
        &self,
        network_id: &str,
        service_id: &str,
        service_name: &str,
    ) -> Result<()> {
        let mut networks = self.networks.write().await;

        let network = networks
            .get_mut(network_id)
            .ok_or_else(|| DaemonError::NetworkNotFound(network_id.to_string()))?;

        if network.has_member(service_id) {
            debug!(
                network_id = %network_id,
                service_id = %service_id,
                "Service already in network"
            );
            return Ok(());
        }

        info!(
            network_id = %network_id,
            service_id = %service_id,
            service_name = %service_name,
            "Adding service to network"
        );

        network.add_member(service_id.to_string());

        // Update service -> networks mapping
        let mut service_networks = self.service_networks.write().await;
        service_networks
            .entry(service_id.to_string())
            .or_default()
            .push(network_id.to_string());

        // Register service name for discovery
        self.registry
            .register(service_name.to_string(), service_id.to_string())
            .await;

        // Persist updated network state
        self.persist_network(network)?;

        Ok(())
    }

    /// Removes a service from a network.
    ///
    /// # Errors
    ///
    /// Returns an error if the network does not exist.
    pub async fn remove_service(&self, network_id: &str, service_id: &str) -> Result<()> {
        let mut networks = self.networks.write().await;

        let network = networks
            .get_mut(network_id)
            .ok_or_else(|| DaemonError::NetworkNotFound(network_id.to_string()))?;

        if !network.has_member(service_id) {
            debug!(
                network_id = %network_id,
                service_id = %service_id,
                "Service not in network"
            );
            return Ok(());
        }

        info!(
            network_id = %network_id,
            service_id = %service_id,
            "Removing service from network"
        );

        network.remove_member(service_id);

        // Update service -> networks mapping
        let mut service_networks = self.service_networks.write().await;
        if let Some(networks_list) = service_networks.get_mut(service_id) {
            networks_list.retain(|id| id != network_id);
            if networks_list.is_empty() {
                service_networks.remove(service_id);
            }
        }

        // Persist updated network state
        self.persist_network(network)?;

        Ok(())
    }

    /// Removes a service from all networks.
    ///
    /// Called when a service is deleted.
    pub async fn remove_service_from_all(&self, service_id: &str) {
        let network_ids: Vec<String> = {
            let service_networks = self.service_networks.read().await;
            service_networks
                .get(service_id)
                .cloned()
                .unwrap_or_default()
        };

        for network_id in network_ids {
            if let Err(e) = self.remove_service(&network_id, service_id).await {
                warn!(
                    network_id = %network_id,
                    service_id = %service_id,
                    error = %e,
                    "Failed to remove service from network"
                );
            }
        }

        // Unregister from service registry
        self.registry.unregister_by_id(service_id).await;
    }

    /// Gets the networks a service belongs to.
    pub async fn get_service_networks(&self, service_id: &str) -> Vec<String> {
        let service_networks = self.service_networks.read().await;
        service_networks.get(service_id).cloned().unwrap_or_default()
    }

    /// Checks if two services share a network.
    ///
    /// Returns true if both services are members of at least one common network.
    pub async fn services_share_network(&self, service_a: &str, service_b: &str) -> bool {
        let service_networks = self.service_networks.read().await;

        let Some(networks_a) = service_networks.get(service_a) else {
            return false;
        };

        let Some(networks_b) = service_networks.get(service_b) else {
            return false;
        };

        // Check for any intersection
        networks_a.iter().any(|n| networks_b.contains(n))
    }

    /// Checks if a service allows external (ingress) access.
    ///
    /// Returns true only if the service is a member of at least one network
    /// with `access: external`. Services not on any network or only on
    /// `internal` networks cannot be accessed externally.
    pub async fn service_allows_external_access(&self, service_id: &str) -> bool {
        let service_networks = self.service_networks.read().await;

        // If service is not on any network, deny external access
        let Some(network_ids) = service_networks.get(service_id) else {
            return false;
        };

        if network_ids.is_empty() {
            return false;
        }

        // Check if any network allows external access
        let networks = self.networks.read().await;
        for network_id in network_ids {
            if let Some(network) = networks.get(network_id) {
                if !network.options.access.is_internal() {
                    return true;
                }
            }
        }

        // All networks are internal
        false
    }

    /// Resolves a service name to its ID.
    ///
    /// Uses the service registry to look up the service ID for a given name.
    /// Returns None if the name is not registered (external hostname).
    pub async fn resolve_service(&self, target_host: &str) -> Option<String> {
        let service_name = extract_service_name(target_host);
        self.registry.resolve(service_name).await
    }

    /// Checks if a hostname refers to an internal service.
    ///
    /// Returns true only if the hostname is explicitly registered in the
    /// service registry. External hostnames always return false.
    pub async fn is_internal_service(&self, host: &str) -> bool {
        let service_name = extract_service_name(host);
        self.registry.is_internal_service(service_name).await
    }

    /// Gets a reference to the service registry.
    #[must_use]
    pub fn registry(&self) -> &SharedServiceRegistry {
        &self.registry
    }

    /// Loads persisted network state from the state store.
    ///
    /// # Errors
    ///
    /// Returns an error if state cannot be read.
    pub async fn load_state(&self) -> Result<()> {
        let states = self.state_store.list_networks()?;

        let mut networks = self.networks.write().await;
        let mut service_networks = self.service_networks.write().await;

        for state in states {
            info!(id = %state.id, name = %state.name, "Loading network from state");

            // Rebuild service -> networks mapping
            for service_id in &state.members {
                service_networks
                    .entry(service_id.clone())
                    .or_default()
                    .push(state.id.clone());
            }

            networks.insert(state.id.clone(), state);
        }

        info!(count = networks.len(), "Loaded networks from state");

        Ok(())
    }

    /// Persists a network to the state store.
    fn persist_network(&self, network: &NetworkState) -> Result<()> {
        self.state_store.save_network(network)?;
        Ok(())
    }

    /// Removes a network from the state store.
    fn remove_network_state(&self, id: &str) -> Result<()> {
        self.state_store.delete_network(id)?;
        Ok(())
    }
}

/// Shared reference to a network manager.
pub type SharedNetworkManager = Arc<NetworkManager>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::registry::ServiceRegistry;
    use crate::network::{NetworkAccess, NetworkOptions};
    use std::sync::Arc;
    use tempfile::tempdir;

    fn create_test_manager() -> NetworkManager {
        let dir = tempdir().expect("should create temp dir");
        let db = sled::open(dir.path().join("test.db")).expect("should open db");
        let state_store = Arc::new(StateStore::new(Arc::new(db)));
        let registry = Arc::new(ServiceRegistry::new());
        NetworkManager::new(state_store, registry)
    }

    #[tokio::test]
    async fn test_create_network() {
        let manager = create_test_manager();

        let config = NetworkConfig::new("test-network".to_string());
        let id = manager.create_network(config).await.unwrap();

        assert!(id.starts_with("net-"));

        let network = manager.get_network(&id).await.unwrap();
        assert_eq!(network.name, "test-network");
        assert!(!network.options.access.is_internal());
    }

    #[tokio::test]
    async fn test_create_duplicate_network_name() {
        let manager = create_test_manager();

        let config = NetworkConfig::new("my-network".to_string());
        manager.create_network(config).await.unwrap();

        let config2 = NetworkConfig::new("my-network".to_string());
        let result = manager.create_network(config2).await;

        assert!(matches!(result, Err(DaemonError::NetworkExists(_))));
    }

    #[tokio::test]
    async fn test_delete_network() {
        let manager = create_test_manager();

        let config = NetworkConfig::new("deletable".to_string());
        let id = manager.create_network(config).await.unwrap();

        manager.delete_network(&id).await.unwrap();

        assert!(manager.get_network(&id).await.is_none());
    }

    #[tokio::test]
    async fn test_delete_network_with_members() {
        let manager = create_test_manager();

        let config = NetworkConfig::new("with-members".to_string());
        let id = manager.create_network(config).await.unwrap();

        manager
            .add_service(&id, "svc-1", "my-service")
            .await
            .unwrap();

        let result = manager.delete_network(&id).await;
        assert!(matches!(result, Err(DaemonError::NetworkHasMembers(_))));
    }

    #[tokio::test]
    async fn test_add_remove_service() {
        let manager = create_test_manager();

        let config = NetworkConfig::new("test-net".to_string());
        let id = manager.create_network(config).await.unwrap();

        // Add service
        manager
            .add_service(&id, "svc-123", "api-service")
            .await
            .unwrap();

        let network = manager.get_network(&id).await.unwrap();
        assert!(network.members.contains(&"svc-123".to_string()));

        // Service should be resolvable
        let resolved = manager.resolve_service("api-service").await;
        assert_eq!(resolved, Some("svc-123".to_string()));

        // Remove service
        manager.remove_service(&id, "svc-123").await.unwrap();

        let network = manager.get_network(&id).await.unwrap();
        assert!(!network.members.contains(&"svc-123".to_string()));
    }

    #[tokio::test]
    async fn test_services_share_network() {
        let manager = create_test_manager();

        let config = NetworkConfig::new("shared-net".to_string());
        let id = manager.create_network(config).await.unwrap();

        manager
            .add_service(&id, "svc-a", "service-a")
            .await
            .unwrap();
        manager
            .add_service(&id, "svc-b", "service-b")
            .await
            .unwrap();

        assert!(manager.services_share_network("svc-a", "svc-b").await);
        assert!(!manager.services_share_network("svc-a", "svc-c").await);
    }

    #[tokio::test]
    async fn test_get_service_networks() {
        let manager = create_test_manager();

        let config1 = NetworkConfig::new("net-1".to_string());
        let id1 = manager.create_network(config1).await.unwrap();

        let config2 = NetworkConfig::new("net-2".to_string());
        let id2 = manager.create_network(config2).await.unwrap();

        manager
            .add_service(&id1, "svc-1", "my-service")
            .await
            .unwrap();
        manager
            .add_service(&id2, "svc-1", "my-service")
            .await
            .unwrap();

        let networks = manager.get_service_networks("svc-1").await;
        assert_eq!(networks.len(), 2);
        assert!(networks.contains(&id1));
        assert!(networks.contains(&id2));
    }

    #[tokio::test]
    async fn test_remove_service_from_all() {
        let manager = create_test_manager();

        let config1 = NetworkConfig::new("net-a".to_string());
        let id1 = manager.create_network(config1).await.unwrap();

        let config2 = NetworkConfig::new("net-b".to_string());
        let id2 = manager.create_network(config2).await.unwrap();

        manager
            .add_service(&id1, "svc-1", "my-svc")
            .await
            .unwrap();
        manager
            .add_service(&id2, "svc-1", "my-svc")
            .await
            .unwrap();

        manager.remove_service_from_all("svc-1").await;

        let networks = manager.get_service_networks("svc-1").await;
        assert!(networks.is_empty());

        // Service should no longer be resolvable
        assert!(manager.resolve_service("my-svc").await.is_none());
    }

    #[tokio::test]
    async fn test_is_internal_service() {
        let manager = create_test_manager();

        let config = NetworkConfig::new("test-net".to_string());
        let id = manager.create_network(config).await.unwrap();

        manager
            .add_service(&id, "svc-1", "internal-api")
            .await
            .unwrap();

        // Registered service is internal
        assert!(manager.is_internal_service("internal-api").await);
        assert!(manager.is_internal_service("internal-api:8080").await);

        // Unregistered hostnames are external
        assert!(!manager.is_internal_service("external.com").await);
        assert!(!manager.is_internal_service("api.example.com:443").await);
    }

    #[tokio::test]
    async fn test_get_network_by_name() {
        let manager = create_test_manager();

        let config = NetworkConfig::new("named-network".to_string());
        let id = manager.create_network(config).await.unwrap();

        let network = manager.get_network_by_name("named-network").await.unwrap();
        assert_eq!(network.id, id);
        assert_eq!(network.name, "named-network");

        assert!(manager.get_network_by_name("nonexistent").await.is_none());
    }

    // ========== External Access Tests ==========

    #[tokio::test]
    async fn test_external_access_no_network() {
        let manager = create_test_manager();

        // Service not on any network should not allow external access
        assert!(!manager.service_allows_external_access("orphan-svc").await);
    }

    #[tokio::test]
    async fn test_external_access_on_external_network() {
        let manager = create_test_manager();

        // Create an external network (default)
        let config = NetworkConfig::new("external-net".to_string());
        let id = manager.create_network(config).await.unwrap();

        manager
            .add_service(&id, "svc-1", "my-service")
            .await
            .unwrap();

        // Service on external network should allow external access
        assert!(manager.service_allows_external_access("svc-1").await);
    }

    #[tokio::test]
    async fn test_external_access_on_internal_network() {
        let manager = create_test_manager();

        // Create an internal network
        let mut options = NetworkOptions::default();
        options.access = NetworkAccess::Internal;
        let config = NetworkConfig::with_options("internal-net".to_string(), options);
        let id = manager.create_network(config).await.unwrap();

        manager
            .add_service(&id, "svc-1", "internal-svc")
            .await
            .unwrap();

        // Service on internal-only network should not allow external access
        assert!(!manager.service_allows_external_access("svc-1").await);
    }

    #[tokio::test]
    async fn test_external_access_on_mixed_networks() {
        let manager = create_test_manager();

        // Create an internal network
        let mut internal_opts = NetworkOptions::default();
        internal_opts.access = NetworkAccess::Internal;
        let internal_config = NetworkConfig::with_options("internal".to_string(), internal_opts);
        let internal_id = manager.create_network(internal_config).await.unwrap();

        // Create an external network
        let external_config = NetworkConfig::new("external".to_string());
        let external_id = manager.create_network(external_config).await.unwrap();

        // Add service to both networks
        manager
            .add_service(&internal_id, "svc-1", "mixed-svc")
            .await
            .unwrap();
        manager
            .add_service(&external_id, "svc-1", "mixed-svc")
            .await
            .unwrap();

        // Service on at least one external network should allow external access
        assert!(manager.service_allows_external_access("svc-1").await);
    }

    #[tokio::test]
    async fn test_external_access_multiple_internal_networks() {
        let manager = create_test_manager();

        // Create two internal networks
        let mut opts1 = NetworkOptions::default();
        opts1.access = NetworkAccess::Internal;
        let config1 = NetworkConfig::with_options("internal-1".to_string(), opts1);
        let id1 = manager.create_network(config1).await.unwrap();

        let mut opts2 = NetworkOptions::default();
        opts2.access = NetworkAccess::Internal;
        let config2 = NetworkConfig::with_options("internal-2".to_string(), opts2);
        let id2 = manager.create_network(config2).await.unwrap();

        // Add service to both internal networks
        manager
            .add_service(&id1, "svc-1", "fully-internal")
            .await
            .unwrap();
        manager
            .add_service(&id2, "svc-1", "fully-internal")
            .await
            .unwrap();

        // Service only on internal networks should not allow external access
        assert!(!manager.service_allows_external_access("svc-1").await);
    }
}
