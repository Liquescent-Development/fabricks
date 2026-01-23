//! Types for policy evaluation.
//!
//! Defines the types used for policy decisions, evaluation context,
//! and policy storage.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use fabricks_common::models::mortar::Policy;
use serde::{Deserialize, Serialize};

/// Result of evaluating a policy against a connection request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum PolicyDecision {
    /// Connection is allowed.
    Allow,
    /// Connection is denied by policy.
    Deny {
        /// Description of the rule that caused the denial.
        rule_description: String,
        /// Reason for the denial.
        reason: String,
    },
    /// Connection is allowed but a warning is logged.
    Warn {
        /// Description of the rule that triggered the warning.
        rule_description: String,
        /// Reason for the warning.
        reason: String,
    },
}

impl PolicyDecision {
    /// Creates a new deny decision.
    #[must_use]
    pub fn deny(rule_description: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Deny {
            rule_description: rule_description.into(),
            reason: reason.into(),
        }
    }

    /// Creates a new warn decision.
    #[must_use]
    pub fn warn(rule_description: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Warn {
            rule_description: rule_description.into(),
            reason: reason.into(),
        }
    }

    /// Returns true if this decision allows the connection.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow | Self::Warn { .. })
    }

    /// Returns true if this decision denies the connection.
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }

    /// Returns the decision type as a string.
    #[must_use]
    pub fn decision_type(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny { .. } => "deny",
            Self::Warn { .. } => "warn",
        }
    }

    /// Returns the rule description if this is a deny or warn decision.
    #[must_use]
    pub fn rule_description(&self) -> Option<&str> {
        match self {
            Self::Allow => None,
            Self::Deny {
                rule_description, ..
            }
            | Self::Warn {
                rule_description, ..
            } => Some(rule_description),
        }
    }

    /// Returns the reason if this is a deny or warn decision.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Allow => None,
            Self::Deny { reason, .. } | Self::Warn { reason, .. } => Some(reason),
        }
    }
}

/// Context for policy evaluation.
///
/// Contains all information needed to evaluate policies against a connection.
#[derive(Debug, Clone)]
pub struct PolicyEvaluationContext {
    /// Service ID initiating the connection.
    pub from_service: String,

    /// Target of the connection (service name, URL, or address).
    pub to_target: String,

    /// Mortar project ID the source service belongs to (if any).
    pub mortar_id: Option<String>,

    /// Timestamp of the evaluation.
    pub timestamp: DateTime<Utc>,
}

impl PolicyEvaluationContext {
    /// Creates a new evaluation context.
    #[must_use]
    pub fn new(from_service: impl Into<String>, to_target: impl Into<String>) -> Self {
        Self {
            from_service: from_service.into(),
            to_target: to_target.into(),
            mortar_id: None,
            timestamp: Utc::now(),
        }
    }

    /// Sets the mortar project ID.
    #[must_use]
    pub fn with_mortar_id(mut self, mortar_id: impl Into<String>) -> Self {
        self.mortar_id = Some(mortar_id.into());
        self
    }
}

/// A policy loaded from a mortar file with service mappings.
///
/// This stores the policy along with mappings from service names
/// (as used in the mortar file) to service IDs (as used internally).
#[derive(Debug, Clone)]
pub struct EvaluatedPolicy {
    /// Mortar project ID.
    pub mortar_id: String,

    /// The policy from the mortar file.
    pub policy: Policy,

    /// Mapping from service names to service IDs.
    pub service_mappings: HashMap<String, String>,

    /// Reverse mapping from service IDs to service names.
    pub id_to_name: HashMap<String, String>,
}

impl EvaluatedPolicy {
    /// Creates a new evaluated policy.
    #[must_use]
    pub fn new(
        mortar_id: impl Into<String>,
        policy: Policy,
        service_mappings: HashMap<String, String>,
    ) -> Self {
        // Build reverse mapping
        let id_to_name: HashMap<String, String> = service_mappings
            .iter()
            .map(|(name, id)| (id.clone(), name.clone()))
            .collect();

        Self {
            mortar_id: mortar_id.into(),
            policy,
            service_mappings,
            id_to_name,
        }
    }

    /// Gets the service name for a given service ID.
    #[must_use]
    pub fn service_name(&self, service_id: &str) -> Option<&str> {
        self.id_to_name.get(service_id).map(String::as_str)
    }

    /// Gets the service ID for a given service name.
    #[must_use]
    pub fn service_id(&self, service_name: &str) -> Option<&str> {
        self.service_mappings.get(service_name).map(String::as_str)
    }

    /// Checks if a service ID belongs to this mortar project.
    #[must_use]
    pub fn contains_service(&self, service_id: &str) -> bool {
        self.id_to_name.contains_key(service_id)
    }
}

/// API response for a policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyInfo {
    /// Mortar project ID.
    pub mortar_id: String,

    /// Policy description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Number of deny rules.
    pub deny_rules: usize,

    /// Number of require rules.
    pub require_rules: usize,

    /// Number of warn rules.
    pub warn_rules: usize,

    /// Services covered by this policy.
    pub services: Vec<String>,
}

impl From<&EvaluatedPolicy> for PolicyInfo {
    fn from(ep: &EvaluatedPolicy) -> Self {
        Self {
            mortar_id: ep.mortar_id.clone(),
            description: ep.policy.description.clone(),
            deny_rules: ep.policy.deny.as_ref().map_or(0, Vec::len),
            require_rules: ep.policy.require.as_ref().map_or(0, Vec::len),
            warn_rules: ep.policy.warn.as_ref().map_or(0, Vec::len),
            services: ep.service_mappings.keys().cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_decision_allow() {
        let decision = PolicyDecision::Allow;
        assert!(decision.is_allowed());
        assert!(!decision.is_denied());
        assert_eq!(decision.decision_type(), "allow");
        assert!(decision.rule_description().is_none());
        assert!(decision.reason().is_none());
    }

    #[test]
    fn test_policy_decision_deny() {
        let decision = PolicyDecision::deny("deny rule 1", "connection not allowed");
        assert!(!decision.is_allowed());
        assert!(decision.is_denied());
        assert_eq!(decision.decision_type(), "deny");
        assert_eq!(decision.rule_description(), Some("deny rule 1"));
        assert_eq!(decision.reason(), Some("connection not allowed"));
    }

    #[test]
    fn test_policy_decision_warn() {
        let decision = PolicyDecision::warn("warn rule 1", "cross-network communication");
        assert!(decision.is_allowed());
        assert!(!decision.is_denied());
        assert_eq!(decision.decision_type(), "warn");
        assert_eq!(decision.rule_description(), Some("warn rule 1"));
        assert_eq!(decision.reason(), Some("cross-network communication"));
    }

    #[test]
    fn test_evaluation_context() {
        let ctx = PolicyEvaluationContext::new("svc-1", "svc-2").with_mortar_id("mortar-1");

        assert_eq!(ctx.from_service, "svc-1");
        assert_eq!(ctx.to_target, "svc-2");
        assert_eq!(ctx.mortar_id, Some("mortar-1".to_string()));
    }

    #[test]
    fn test_evaluated_policy() {
        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-api-123".to_string());
        mappings.insert("db".to_string(), "svc-db-456".to_string());

        let policy = Policy {
            description: Some("Test policy".to_string()),
            deny: None,
            require: None,
            warn: None,
        };

        let ep = EvaluatedPolicy::new("mortar-1", policy, mappings);

        assert_eq!(ep.service_name("svc-api-123"), Some("api"));
        assert_eq!(ep.service_id("api"), Some("svc-api-123"));
        assert!(ep.contains_service("svc-api-123"));
        assert!(!ep.contains_service("svc-unknown"));
    }

    #[test]
    fn test_policy_info_from_evaluated_policy() {
        use fabricks_common::models::mortar::{DenyRule, RequireRule};

        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-1".to_string());

        let policy = Policy {
            description: Some("Test".to_string()),
            deny: Some(vec![
                DenyRule {
                    from: None,
                    to: None,
                    except: None,
                    reason: None,
                },
                DenyRule {
                    from: None,
                    to: None,
                    except: None,
                    reason: None,
                },
            ]),
            require: Some(vec![RequireRule {
                networks: None,
                services: None,
                tls: Some(true),
                audit: None,
                encryption: None,
            }]),
            warn: None,
        };

        let ep = EvaluatedPolicy::new("mortar-1", policy, mappings);
        let info = PolicyInfo::from(&ep);

        assert_eq!(info.mortar_id, "mortar-1");
        assert_eq!(info.description, Some("Test".to_string()));
        assert_eq!(info.deny_rules, 2);
        assert_eq!(info.require_rules, 1);
        assert_eq!(info.warn_rules, 0);
        assert_eq!(info.services.len(), 1);
    }
}
