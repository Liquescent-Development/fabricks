//! Policy lifecycle management.
//!
//! Manages loading, unloading, and querying policies for mortar projects.

use std::collections::HashMap;
use std::sync::Arc;

use fabricks_common::models::mortar::Policy;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::events::EventBus;

use super::engine::PolicyEngine;
use super::types::{EvaluatedPolicy, PolicyDecision, PolicyEvaluationContext, PolicyInfo};

/// Manages policies for mortar projects.
///
/// Handles loading and unloading policies, and delegates evaluation
/// to the policy engine.
pub struct PolicyManager {
    /// Policy evaluation engine.
    engine: PolicyEngine,

    /// Loaded policies indexed by mortar ID.
    policies: RwLock<HashMap<String, EvaluatedPolicy>>,

    /// Mapping from service IDs to mortar IDs.
    service_to_mortar: RwLock<HashMap<String, String>>,
}

impl PolicyManager {
    /// Creates a new policy manager.
    #[must_use]
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            engine: PolicyEngine::new(event_bus),
            policies: RwLock::new(HashMap::new()),
            service_to_mortar: RwLock::new(HashMap::new()),
        }
    }

    /// Loads a policy for a mortar project.
    ///
    /// The service mappings map service names (as used in the mortar file)
    /// to service IDs (as used internally by the daemon).
    pub async fn load_policies(
        &self,
        mortar_id: &str,
        policy: Policy,
        service_mappings: HashMap<String, String>,
    ) {
        info!(
            mortar_id = %mortar_id,
            deny_rules = policy.deny.as_ref().map_or(0, Vec::len),
            require_rules = policy.require.as_ref().map_or(0, Vec::len),
            warn_rules = policy.warn.as_ref().map_or(0, Vec::len),
            services = service_mappings.len(),
            "Loading policy for mortar project"
        );

        // Create evaluated policy
        let evaluated = EvaluatedPolicy::new(mortar_id, policy, service_mappings.clone());

        // Store the policy
        {
            let mut policies = self.policies.write().await;
            policies.insert(mortar_id.to_string(), evaluated);
        }

        // Update service-to-mortar mapping
        {
            let mut s2m = self.service_to_mortar.write().await;
            for service_id in service_mappings.values() {
                s2m.insert(service_id.clone(), mortar_id.to_string());
            }
        }

        debug!(mortar_id = %mortar_id, "Policy loaded successfully");
    }

    /// Unloads a policy for a mortar project.
    pub async fn unload_policies(&self, mortar_id: &str) {
        info!(mortar_id = %mortar_id, "Unloading policy for mortar project");

        // Remove the policy
        let removed = {
            let mut policies = self.policies.write().await;
            policies.remove(mortar_id)
        };

        // Clean up service-to-mortar mappings
        if let Some(policy) = removed {
            let mut s2m = self.service_to_mortar.write().await;
            for service_id in policy.service_mappings.values() {
                s2m.remove(service_id);
            }
        }

        debug!(mortar_id = %mortar_id, "Policy unloaded successfully");
    }

    /// Evaluates a connection request against applicable policies.
    ///
    /// Returns the policy decision for the connection.
    pub async fn evaluate_connection(
        &self,
        from_service: &str,
        to_target: &str,
    ) -> PolicyDecision {
        // Find the mortar ID for this service
        let mortar_id = {
            let s2m = self.service_to_mortar.read().await;
            s2m.get(from_service).cloned()
        };

        // Build evaluation context
        let mut ctx = PolicyEvaluationContext::new(from_service, to_target);
        if let Some(ref mid) = mortar_id {
            ctx = ctx.with_mortar_id(mid);
        }

        // Get applicable policies
        let policies = self.get_applicable_policies(from_service).await;

        // If no policies apply, allow
        if policies.is_empty() {
            debug!(
                from = %from_service,
                to = %to_target,
                "No policies applicable, allowing connection"
            );
            return PolicyDecision::Allow;
        }

        // Evaluate against all applicable policies
        self.engine.evaluate(&ctx, &policies)
    }

    /// Gets policies applicable to a service.
    ///
    /// Currently, only the policy for the service's mortar project applies.
    async fn get_applicable_policies(&self, service_id: &str) -> Vec<EvaluatedPolicy> {
        // Find the mortar ID for this service
        let mortar_id = {
            let s2m = self.service_to_mortar.read().await;
            s2m.get(service_id).cloned()
        };

        // Get the policy for that mortar
        let Some(mortar_id) = mortar_id else {
            return vec![];
        };

        let policies = self.policies.read().await;
        policies.get(&mortar_id).cloned().into_iter().collect()
    }

    /// Gets a policy by mortar ID.
    pub async fn get_policy(&self, mortar_id: &str) -> Option<EvaluatedPolicy> {
        let policies = self.policies.read().await;
        policies.get(mortar_id).cloned()
    }

    /// Lists all loaded policies.
    pub async fn list_policies(&self) -> Vec<PolicyInfo> {
        let policies = self.policies.read().await;
        policies.values().map(PolicyInfo::from).collect()
    }

    /// Gets the mortar ID for a service.
    pub async fn get_mortar_id(&self, service_id: &str) -> Option<String> {
        let s2m = self.service_to_mortar.read().await;
        s2m.get(service_id).cloned()
    }

    /// Checks if a service has any policies that apply to it.
    pub async fn has_policies(&self, service_id: &str) -> bool {
        let s2m = self.service_to_mortar.read().await;
        if let Some(mortar_id) = s2m.get(service_id) {
            let policies = self.policies.read().await;
            return policies.contains_key(mortar_id);
        }
        false
    }
}

impl std::fmt::Debug for PolicyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyManager").finish_non_exhaustive()
    }
}

/// Shared reference to a policy manager.
pub type SharedPolicyManager = Arc<PolicyManager>;

#[cfg(test)]
mod tests {
    use fabricks_common::models::mortar::DenyRule;

    use crate::events::EventBus;

    use super::*;

    fn test_manager() -> PolicyManager {
        let event_bus = Arc::new(EventBus::new(100, 100));
        PolicyManager::new(event_bus)
    }

    #[tokio::test]
    async fn test_load_and_unload_policy() {
        let manager = test_manager();

        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-api-1".to_string());

        let policy = Policy {
            description: Some("Test".to_string()),
            deny: None,
            require: None,
            warn: None,
        };

        // Load policy
        manager
            .load_policies("mortar-1", policy, mappings)
            .await;

        // Verify loaded
        assert!(manager.get_policy("mortar-1").await.is_some());
        assert_eq!(manager.get_mortar_id("svc-api-1").await, Some("mortar-1".to_string()));

        // Unload policy
        manager.unload_policies("mortar-1").await;

        // Verify unloaded
        assert!(manager.get_policy("mortar-1").await.is_none());
        assert!(manager.get_mortar_id("svc-api-1").await.is_none());
    }

    #[tokio::test]
    async fn test_list_policies() {
        let manager = test_manager();

        // Initially empty
        assert!(manager.list_policies().await.is_empty());

        // Load a policy
        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-api-1".to_string());

        let policy = Policy {
            description: Some("Test".to_string()),
            deny: None,
            require: None,
            warn: None,
        };

        manager
            .load_policies("mortar-1", policy, mappings)
            .await;

        // Should have one policy
        let policies = manager.list_policies().await;
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].mortar_id, "mortar-1");
    }

    #[tokio::test]
    async fn test_evaluate_connection_no_policy() {
        let manager = test_manager();

        // No policies loaded, should allow
        let decision = manager
            .evaluate_connection("svc-unknown", "svc-other")
            .await;
        assert!(decision.is_allowed());
    }

    #[tokio::test]
    async fn test_evaluate_connection_with_deny() {
        let manager = test_manager();

        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-api-1".to_string());
        mappings.insert("db".to_string(), "svc-db-1".to_string());

        let policy = Policy {
            description: None,
            deny: Some(vec![DenyRule {
                from: Some(vec!["api".to_string()]),
                to: Some(vec!["db".to_string()]),
                except: None,
                reason: Some("API cannot directly access DB".to_string()),
            }]),
            require: None,
            warn: None,
        };

        manager
            .load_policies("mortar-1", policy, mappings)
            .await;

        // api -> db should be denied
        let decision = manager
            .evaluate_connection("svc-api-1", "svc-db-1")
            .await;
        assert!(decision.is_denied());
        assert!(decision.reason().unwrap().contains("API cannot directly access DB"));
    }

    #[tokio::test]
    async fn test_has_policies() {
        let manager = test_manager();

        // No policy initially
        assert!(!manager.has_policies("svc-api-1").await);

        // Load a policy
        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-api-1".to_string());

        let policy = Policy {
            description: None,
            deny: None,
            require: None,
            warn: None,
        };

        manager
            .load_policies("mortar-1", policy, mappings)
            .await;

        // Now has policy
        assert!(manager.has_policies("svc-api-1").await);
        assert!(!manager.has_policies("svc-unknown").await);
    }

    #[tokio::test]
    async fn test_multiple_mortars() {
        let manager = test_manager();

        // Load two mortars with different policies
        let mut mappings1 = HashMap::new();
        mappings1.insert("api".to_string(), "svc-api-1".to_string());

        let policy1 = Policy {
            description: Some("Mortar 1".to_string()),
            deny: None,
            require: None,
            warn: None,
        };

        let mut mappings2 = HashMap::new();
        mappings2.insert("worker".to_string(), "svc-worker-1".to_string());

        let policy2 = Policy {
            description: Some("Mortar 2".to_string()),
            deny: Some(vec![DenyRule {
                from: None,
                to: None,
                except: None,
                reason: Some("Deny all".to_string()),
            }]),
            require: None,
            warn: None,
        };

        manager.load_policies("mortar-1", policy1, mappings1).await;
        manager.load_policies("mortar-2", policy2, mappings2).await;

        // api (mortar-1) should be allowed to connect anywhere
        let decision = manager
            .evaluate_connection("svc-api-1", "external.com")
            .await;
        assert!(decision.is_allowed());

        // worker (mortar-2) should be denied
        let decision = manager
            .evaluate_connection("svc-worker-1", "external.com")
            .await;
        assert!(decision.is_denied());
    }
}
