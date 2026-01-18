//! Connection validation for network access control.
//!
//! Validates connections based on:
//! 1. Ingress (external to service): Network access mode (`external` vs `internal`)
//! 2. Egress capabilities (can the service connect to this target?)
//! 3. Network membership (do both services share a network?)
//! 4. Policy rules (deny, require, warn rules from mortar policies)

use std::sync::Arc;

use tracing::debug;

use fabricks_common::models::capability::Capabilities;

use crate::policy::{PolicyDecision, PolicyManager};

use super::manager::NetworkManager;

/// Result of connection validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionDecision {
    /// Allow connection to an internal service.
    AllowInternal {
        /// The resolved service ID.
        service_id: String,
    },

    /// Allow connection to an external host.
    AllowExternal,

    /// Deny due to missing capability grant.
    DenyCapability {
        /// Reason for denial.
        reason: String,
    },

    /// Deny due to network isolation.
    DenyNetwork {
        /// Reason for denial.
        reason: String,
    },

    /// Deny due to policy violation.
    DenyPolicy {
        /// Reason for denial.
        reason: String,
    },
}

impl ConnectionDecision {
    /// Returns true if the connection is allowed.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::AllowInternal { .. } | Self::AllowExternal)
    }

    /// Returns the denial reason if this is a denial decision.
    #[must_use]
    pub fn denial_reason(&self) -> Option<&str> {
        match self {
            Self::DenyCapability { reason }
            | Self::DenyNetwork { reason }
            | Self::DenyPolicy { reason } => Some(reason),
            Self::AllowInternal { .. } | Self::AllowExternal => None,
        }
    }
}

/// Validates an outbound connection from a service.
///
/// Checks in order:
/// 1. Does the service's capabilities allow connecting to this target?
/// 2. If internal: do both services share a network?
/// 3. Policy rules from mortar configuration
///
/// # Arguments
///
/// * `from_service_id` - The service initiating the connection
/// * `capabilities` - The service's capability grants
/// * `target_host` - The target hostname (may include port)
/// * `target_port` - The target port number
/// * `network_manager` - The network manager for service resolution
/// * `policy_manager` - Optional policy manager for policy evaluation
///
/// # Returns
///
/// A `ConnectionDecision` indicating whether the connection should be allowed.
pub async fn validate_connection(
    from_service_id: &str,
    capabilities: &Option<Capabilities>,
    target_host: &str,
    target_port: u16,
    network_manager: &Arc<NetworkManager>,
    policy_manager: Option<&PolicyManager>,
) -> ConnectionDecision {
    // Step 1: Check capability grant
    let target_with_port = format!("{target_host}:{target_port}");

    if !check_capability_allows(capabilities.as_ref(), &target_with_port) {
        return ConnectionDecision::DenyCapability {
            reason: format!("Service does not have capability to connect to '{target_with_port}'"),
        };
    }

    // Step 2: Check if target is an internal service
    if let Some(target_service_id) = network_manager.resolve_service(target_host).await {
        debug!(
            from = %from_service_id,
            to = %target_service_id,
            host = %target_host,
            "Checking internal service connection"
        );

        // Both services must share a network
        if !network_manager
            .services_share_network(from_service_id, &target_service_id)
            .await
        {
            return ConnectionDecision::DenyNetwork {
                reason: format!(
                    "Services '{from_service_id}' and '{target_service_id}' do not share a network"
                ),
            };
        }

        // Step 3: Check policies for internal connections
        if let Some(pm) = policy_manager {
            let decision = pm
                .evaluate_connection(from_service_id, &target_service_id)
                .await;

            match decision {
                PolicyDecision::Deny { reason, .. } => {
                    return ConnectionDecision::DenyPolicy { reason };
                }
                PolicyDecision::Warn { .. } => {
                    // Warning already logged by engine, continue to allow
                }
                PolicyDecision::Allow => {}
            }
        }

        return ConnectionDecision::AllowInternal {
            service_id: target_service_id,
        };
    }

    // Step 4: External connection - check policies
    if let Some(pm) = policy_manager {
        let decision = pm
            .evaluate_connection(from_service_id, &target_with_port)
            .await;

        match decision {
            PolicyDecision::Deny { reason, .. } => {
                return ConnectionDecision::DenyPolicy { reason };
            }
            PolicyDecision::Warn { .. } => {
                // Warning already logged by engine, continue to allow
            }
            PolicyDecision::Allow => {}
        }
    }

    debug!(
        from = %from_service_id,
        host = %target_host,
        port = %target_port,
        "Allowing external connection"
    );

    ConnectionDecision::AllowExternal
}

/// Checks if capabilities grant access to connect to a target.
fn check_capability_allows(capabilities: Option<&Capabilities>, target: &str) -> bool {
    match capabilities {
        None => false, // No capabilities = deny all
        Some(caps) => caps.can_connect(target),
    }
}

/// Validates that a service can listen on a given port.
///
/// Checks that the service's capabilities include the port in listen grants.
#[must_use]
pub fn validate_listen_port(capabilities: &Option<Capabilities>, port: u16) -> bool {
    match capabilities {
        None => false,
        Some(caps) => caps.can_listen(port),
    }
}

/// Result of ingress validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngressDecision {
    /// Allow the incoming connection.
    Allow,
    /// Deny: service only allows internal access.
    DenyInternal {
        /// The service ID that rejected external access.
        service_id: String,
    },
}

impl IngressDecision {
    /// Returns true if the ingress is allowed.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

/// Validates an incoming (ingress) connection to a service.
///
/// Checks if the target service allows external access based on its network
/// membership. Services on only `internal` networks will reject external requests.
///
/// # Arguments
///
/// * `service_id` - The service receiving the connection
/// * `network_manager` - The network manager for access control
///
/// # Returns
///
/// An `IngressDecision` indicating whether the connection should be allowed.
pub async fn validate_ingress(
    service_id: &str,
    network_manager: &Arc<NetworkManager>,
) -> IngressDecision {
    if network_manager
        .service_allows_external_access(service_id)
        .await
    {
        debug!(
            service_id = %service_id,
            "Allowing external ingress"
        );
        IngressDecision::Allow
    } else {
        debug!(
            service_id = %service_id,
            "Denying external ingress: service only allows internal access"
        );
        IngressDecision::DenyInternal {
            service_id: service_id.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::NetworkConfig;
    use crate::network::registry::ServiceRegistry;
    use crate::store::StateStore;
    use fabricks_common::models::capability::NetworkCapabilities;
    use std::sync::Arc;
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

    fn capabilities_for_listen(ports: Vec<u16>) -> Option<Capabilities> {
        Some(Capabilities {
            env: None,
            network: Some(NetworkCapabilities {
                connect: None,
                listen: Some(ports),
                allow_all_outbound: None,
            }),
            filesystem: None,
            wasm: None,
        })
    }

    fn capabilities_allow_all_outbound() -> Option<Capabilities> {
        Some(Capabilities {
            env: None,
            network: Some(NetworkCapabilities {
                connect: None,
                listen: None,
                allow_all_outbound: Some(true),
            }),
            filesystem: None,
            wasm: None,
        })
    }

    #[tokio::test]
    async fn test_deny_no_capability() {
        let manager = create_test_network_manager();

        let decision =
            validate_connection("svc-1", &None, "api.example.com", 443, &manager, None).await;

        assert!(matches!(
            decision,
            ConnectionDecision::DenyCapability { .. }
        ));
        assert!(!decision.is_allowed());
    }

    #[tokio::test]
    async fn test_deny_capability_not_granted() {
        let manager = create_test_network_manager();
        let cap = capabilities_for_connect(vec!["other.example.com:443"]);

        let decision =
            validate_connection("svc-1", &cap, "api.example.com", 443, &manager, None).await;

        assert!(matches!(
            decision,
            ConnectionDecision::DenyCapability { .. }
        ));
    }

    #[tokio::test]
    async fn test_allow_external_with_capability() {
        let manager = create_test_network_manager();
        let cap = capabilities_for_connect(vec!["api.example.com:443"]);

        let decision =
            validate_connection("svc-1", &cap, "api.example.com", 443, &manager, None).await;

        assert_eq!(decision, ConnectionDecision::AllowExternal);
        assert!(decision.is_allowed());
    }

    #[tokio::test]
    async fn test_allow_external_with_allow_all_outbound() {
        let manager = create_test_network_manager();
        let cap = capabilities_allow_all_outbound();

        let decision =
            validate_connection("svc-1", &cap, "any.host.com", 8080, &manager, None).await;

        assert_eq!(decision, ConnectionDecision::AllowExternal);
    }

    #[tokio::test]
    async fn test_deny_internal_no_shared_network() {
        let manager = create_test_network_manager();

        // Create two separate networks
        let net1_config = NetworkConfig::new("net-1".to_string());
        let net1_id = manager.create_network(net1_config).await.unwrap();

        let net2_config = NetworkConfig::new("net-2".to_string());
        let net2_id = manager.create_network(net2_config).await.unwrap();

        // Add services to different networks
        manager
            .add_service(&net1_id, "svc-a", "service-a")
            .await
            .unwrap();
        manager
            .add_service(&net2_id, "svc-b", "service-b")
            .await
            .unwrap();

        let cap = capabilities_for_connect(vec!["service-b:8080"]);

        let decision =
            validate_connection("svc-a", &cap, "service-b", 8080, &manager, None).await;

        assert!(matches!(decision, ConnectionDecision::DenyNetwork { .. }));
    }

    #[tokio::test]
    async fn test_allow_internal_shared_network() {
        let manager = create_test_network_manager();

        // Create a shared network
        let config = NetworkConfig::new("shared-net".to_string());
        let net_id = manager.create_network(config).await.unwrap();

        // Add both services to the same network
        manager
            .add_service(&net_id, "svc-a", "service-a")
            .await
            .unwrap();
        manager
            .add_service(&net_id, "svc-b", "service-b")
            .await
            .unwrap();

        let cap = capabilities_for_connect(vec!["service-b:8080"]);

        let decision =
            validate_connection("svc-a", &cap, "service-b", 8080, &manager, None).await;

        match decision {
            ConnectionDecision::AllowInternal { service_id } => {
                assert_eq!(service_id, "svc-b");
            }
            other => panic!("Expected AllowInternal, got {other:?}"),
        }
    }

    #[test]
    fn test_validate_listen_port_allowed() {
        let cap = capabilities_for_listen(vec![8080, 9090]);
        assert!(validate_listen_port(&cap, 8080));
        assert!(validate_listen_port(&cap, 9090));
    }

    #[test]
    fn test_validate_listen_port_denied() {
        let cap = capabilities_for_listen(vec![8080]);
        assert!(!validate_listen_port(&cap, 9090));
    }

    #[test]
    fn test_validate_listen_port_no_capability() {
        assert!(!validate_listen_port(&None, 8080));
    }

    #[test]
    fn test_decision_denial_reason() {
        let allow = ConnectionDecision::AllowExternal;
        assert!(allow.denial_reason().is_none());

        let deny = ConnectionDecision::DenyCapability {
            reason: "test reason".to_string(),
        };
        assert_eq!(deny.denial_reason(), Some("test reason"));
    }

    // ========== Ingress Validation Tests ==========

    #[tokio::test]
    async fn test_ingress_deny_no_network() {
        let manager = create_test_network_manager();

        // Service not on any network should be denied external access
        let decision = validate_ingress("svc-orphan", &manager).await;

        assert!(!decision.is_allowed());
        assert!(matches!(decision, IngressDecision::DenyInternal { .. }));
    }

    #[tokio::test]
    async fn test_ingress_allow_external_network() {
        let manager = create_test_network_manager();

        // Create an external network (default)
        let config = NetworkConfig::new("external-net".to_string());
        let net_id = manager.create_network(config).await.unwrap();

        // Add service to external network
        manager
            .add_service(&net_id, "svc-public", "public-service")
            .await
            .unwrap();

        // Service should be allowed external access
        let decision = validate_ingress("svc-public", &manager).await;

        assert!(decision.is_allowed());
        assert_eq!(decision, IngressDecision::Allow);
    }

    #[tokio::test]
    async fn test_ingress_deny_internal_only_network() {
        use crate::network::{NetworkAccess, NetworkOptions};

        let manager = create_test_network_manager();

        // Create an internal-only network
        let mut options = NetworkOptions::default();
        options.access = NetworkAccess::Internal;
        let config = NetworkConfig::with_options("internal-net".to_string(), options);
        let net_id = manager.create_network(config).await.unwrap();

        // Add service to internal network
        manager
            .add_service(&net_id, "svc-private", "private-service")
            .await
            .unwrap();

        // Service should be denied external access
        let decision = validate_ingress("svc-private", &manager).await;

        assert!(!decision.is_allowed());
        assert!(matches!(decision, IngressDecision::DenyInternal { .. }));
    }

    #[tokio::test]
    async fn test_ingress_allow_mixed_networks() {
        use crate::network::{NetworkAccess, NetworkOptions};

        let manager = create_test_network_manager();

        // Create an internal network
        let mut internal_opts = NetworkOptions::default();
        internal_opts.access = NetworkAccess::Internal;
        let internal_config =
            NetworkConfig::with_options("internal-net".to_string(), internal_opts);
        let internal_net_id = manager.create_network(internal_config).await.unwrap();

        // Create an external network
        let external_config = NetworkConfig::new("external-net".to_string());
        let external_net_id = manager.create_network(external_config).await.unwrap();

        // Add service to BOTH networks
        manager
            .add_service(&internal_net_id, "svc-mixed", "mixed-service")
            .await
            .unwrap();
        manager
            .add_service(&external_net_id, "svc-mixed", "mixed-service")
            .await
            .unwrap();

        // Service should be allowed because it's on at least one external network
        let decision = validate_ingress("svc-mixed", &manager).await;

        assert!(decision.is_allowed());
        assert_eq!(decision, IngressDecision::Allow);
    }

    #[test]
    fn test_ingress_decision_is_allowed() {
        let allow = IngressDecision::Allow;
        assert!(allow.is_allowed());

        let deny = IngressDecision::DenyInternal {
            service_id: "svc-1".to_string(),
        };
        assert!(!deny.is_allowed());
    }
}
