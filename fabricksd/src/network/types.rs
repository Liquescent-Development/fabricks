//! Network type definitions.
//!
//! Defines the core types for network management including network configuration,
//! state, and membership tracking.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Network access mode determining external connectivity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkAccess {
    /// Network allows external access (default).
    #[default]
    External,
    /// Internal network with no external access.
    Internal,
}

impl NetworkAccess {
    /// Returns true if this is an internal network.
    #[must_use]
    pub const fn is_internal(self) -> bool {
        matches!(self, Self::Internal)
    }
}

/// Network isolation mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkIsolation {
    /// Network can communicate with other networks (default).
    #[default]
    Connected,
    /// Network is completely isolated from other networks.
    Isolated,
}

impl NetworkIsolation {
    /// Returns true if this network is isolated.
    #[must_use]
    pub const fn is_isolated(self) -> bool {
        matches!(self, Self::Isolated)
    }
}

/// Network encryption policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkEncryption {
    /// Encryption is optional (default).
    #[default]
    Optional,
    /// Encryption is required for all traffic.
    Required,
}

impl NetworkEncryption {
    /// Returns true if encryption is required.
    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }
}

/// Network audit policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkAudit {
    /// Auditing is disabled (default).
    #[default]
    Disabled,
    /// All traffic on this network is audited.
    Enabled,
}

impl NetworkAudit {
    /// Returns true if auditing is enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Network options controlling behavior and security policies.
///
/// Groups related network configuration into a single cohesive type.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkOptions {
    /// Network access mode.
    #[serde(default)]
    pub access: NetworkAccess,

    /// Network isolation mode.
    #[serde(default)]
    pub isolation: NetworkIsolation,

    /// Network encryption policy.
    #[serde(default)]
    pub encryption: NetworkEncryption,

    /// Network audit policy.
    #[serde(default)]
    pub audit: NetworkAudit,
}

impl NetworkOptions {
    /// Creates new network options with explicit settings.
    #[must_use]
    pub const fn new(
        access: NetworkAccess,
        isolation: NetworkIsolation,
        encryption: NetworkEncryption,
        audit: NetworkAudit,
    ) -> Self {
        Self {
            access,
            isolation,
            encryption,
            audit,
        }
    }

    /// Creates options for an internal network.
    #[must_use]
    pub const fn internal() -> Self {
        Self {
            access: NetworkAccess::Internal,
            isolation: NetworkIsolation::Connected,
            encryption: NetworkEncryption::Optional,
            audit: NetworkAudit::Disabled,
        }
    }

    /// Creates options for an isolated network.
    #[must_use]
    pub const fn isolated() -> Self {
        Self {
            access: NetworkAccess::External,
            isolation: NetworkIsolation::Isolated,
            encryption: NetworkEncryption::Optional,
            audit: NetworkAudit::Disabled,
        }
    }
}

/// Network configuration for creating a network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Network name.
    pub name: String,

    /// Network description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Network options.
    #[serde(flatten, default)]
    pub options: NetworkOptions,
}

impl NetworkConfig {
    /// Creates a new network configuration with the given name.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            options: NetworkOptions::default(),
        }
    }

    /// Creates a network configuration with custom options.
    #[must_use]
    pub fn with_options(name: String, options: NetworkOptions) -> Self {
        Self {
            name,
            description: None,
            options,
        }
    }

    /// Creates a network configuration with description and options.
    #[must_use]
    pub fn with_description(name: String, description: String, options: NetworkOptions) -> Self {
        Self {
            name,
            description: Some(description),
            options,
        }
    }

    /// Creates an internal network configuration.
    #[must_use]
    pub fn internal(name: String) -> Self {
        Self {
            name,
            description: None,
            options: NetworkOptions::internal(),
        }
    }
}

/// Persisted network state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkState {
    /// Unique network ID.
    pub id: String,

    /// Network name.
    pub name: String,

    /// Network description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Network options.
    #[serde(flatten)]
    pub options: NetworkOptions,

    /// Services that are members of this network.
    pub members: HashSet<String>,

    /// When the network was created.
    pub created_at: DateTime<Utc>,

    /// When the network was last updated.
    pub updated_at: DateTime<Utc>,
}

impl NetworkState {
    /// Creates a new network state from configuration.
    #[must_use]
    pub fn new(config: NetworkConfig) -> Self {
        let now = Utc::now();
        Self {
            id: generate_network_id(),
            name: config.name,
            description: config.description,
            options: config.options,
            members: HashSet::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Adds a service to this network.
    pub fn add_member(&mut self, service_id: String) {
        self.members.insert(service_id);
        self.updated_at = Utc::now();
    }

    /// Removes a service from this network.
    pub fn remove_member(&mut self, service_id: &str) {
        self.members.remove(service_id);
        self.updated_at = Utc::now();
    }

    /// Checks if a service is a member of this network.
    #[must_use]
    pub fn has_member(&self, service_id: &str) -> bool {
        self.members.contains(service_id)
    }

    /// Returns the number of members in this network.
    #[must_use]
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

/// Information about a network for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    /// Network ID.
    pub id: String,

    /// Network name.
    pub name: String,

    /// Network description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Network access mode.
    pub access: NetworkAccess,

    /// Number of service members.
    pub member_count: usize,

    /// When the network was created.
    pub created_at: DateTime<Utc>,
}

impl From<&NetworkState> for NetworkInfo {
    fn from(state: &NetworkState) -> Self {
        Self {
            id: state.id.clone(),
            name: state.name.clone(),
            description: state.description.clone(),
            access: state.options.access,
            member_count: state.members.len(),
            created_at: state.created_at,
        }
    }
}

/// Detailed network information for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDetail {
    /// Network ID.
    pub id: String,

    /// Network name.
    pub name: String,

    /// Network description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Network options.
    #[serde(flatten)]
    pub options: NetworkOptions,

    /// Service IDs that are members.
    pub members: Vec<String>,

    /// When the network was created.
    pub created_at: DateTime<Utc>,

    /// When the network was last updated.
    pub updated_at: DateTime<Utc>,
}

impl From<&NetworkState> for NetworkDetail {
    fn from(state: &NetworkState) -> Self {
        Self {
            id: state.id.clone(),
            name: state.name.clone(),
            description: state.description.clone(),
            options: state.options,
            members: state.members.iter().cloned().collect(),
            created_at: state.created_at,
            updated_at: state.updated_at,
        }
    }
}

/// Generates a unique network ID.
#[must_use]
pub fn generate_network_id() -> String {
    let uuid = Uuid::new_v4();
    format!("net-{}", &uuid.to_string()[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_options_default() {
        let opts = NetworkOptions::default();
        assert!(!opts.access.is_internal());
        assert!(!opts.isolation.is_isolated());
        assert!(!opts.encryption.is_required());
        assert!(!opts.audit.is_enabled());
    }

    #[test]
    fn test_network_options_internal() {
        let opts = NetworkOptions::internal();
        assert!(opts.access.is_internal());
        assert!(!opts.isolation.is_isolated());
    }

    #[test]
    fn test_network_options_isolated() {
        let opts = NetworkOptions::isolated();
        assert!(!opts.access.is_internal());
        assert!(opts.isolation.is_isolated());
    }

    #[test]
    fn test_network_config_new() {
        let config = NetworkConfig::new("test-network".to_string());
        assert_eq!(config.name, "test-network");
        assert!(!config.options.access.is_internal());
        assert!(!config.options.isolation.is_isolated());
    }

    #[test]
    fn test_network_config_internal() {
        let config = NetworkConfig::internal("internal-net".to_string());
        assert_eq!(config.name, "internal-net");
        assert!(config.options.access.is_internal());
    }

    #[test]
    fn test_network_state_membership() {
        let config = NetworkConfig::new("test".to_string());
        let mut state = NetworkState::new(config);

        assert_eq!(state.member_count(), 0);
        assert!(!state.has_member("svc-1"));

        state.add_member("svc-1".to_string());
        assert_eq!(state.member_count(), 1);
        assert!(state.has_member("svc-1"));

        state.add_member("svc-2".to_string());
        assert_eq!(state.member_count(), 2);

        state.remove_member("svc-1");
        assert_eq!(state.member_count(), 1);
        assert!(!state.has_member("svc-1"));
        assert!(state.has_member("svc-2"));
    }

    #[test]
    fn test_network_id_generation() {
        let id1 = generate_network_id();
        let id2 = generate_network_id();

        assert!(id1.starts_with("net-"));
        assert!(id2.starts_with("net-"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_network_info_from_state() {
        let config = NetworkConfig::new("my-network".to_string());
        let mut state = NetworkState::new(config);
        state.add_member("svc-1".to_string());
        state.add_member("svc-2".to_string());

        let info = NetworkInfo::from(&state);
        assert_eq!(info.name, "my-network");
        assert_eq!(info.member_count, 2);
    }
}
