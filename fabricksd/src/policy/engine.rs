//! Policy evaluation engine.
//!
//! Evaluates policies against connection requests, checking deny, require,
//! and warn rules in order.

use std::sync::Arc;

use fabricks_common::models::mortar::{DenyRule, RequireRule, WarnRule};
use tracing::{debug, warn};

use crate::events::{Event, EventBus, EventType};

use super::types::{EvaluatedPolicy, PolicyDecision, PolicyEvaluationContext};

/// Policy evaluation engine.
///
/// Evaluates policies against connection requests and emits audit events.
pub struct PolicyEngine {
    /// Event bus for audit logging.
    event_bus: Arc<EventBus>,
}

impl PolicyEngine {
    /// Creates a new policy engine.
    #[must_use]
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
    }

    /// Evaluates policies against a connection request.
    ///
    /// Rules are evaluated in this order:
    /// 1. Deny rules - if any match, the connection is denied
    /// 2. Require rules - if any requirements aren't met, connection is denied
    /// 3. Warn rules - if any match, a warning is logged but connection is allowed
    /// 4. If no rules match, the connection is allowed
    pub fn evaluate(
        &self,
        ctx: &PolicyEvaluationContext,
        policies: &[EvaluatedPolicy],
    ) -> PolicyDecision {
        debug!(
            from = %ctx.from_service,
            to = %ctx.to_target,
            policies = policies.len(),
            "Evaluating policies for connection"
        );

        for policy in policies {
            // Check deny rules first
            if let Some(ref deny_rules) = policy.policy.deny {
                for (idx, rule) in deny_rules.iter().enumerate() {
                    if self.matches_deny_rule(rule, ctx, policy) {
                        let description = format!(
                            "deny rule {} in {}",
                            idx + 1,
                            policy.mortar_id
                        );
                        let reason = rule
                            .reason
                            .clone()
                            .unwrap_or_else(|| "denied by policy".to_string());

                        let decision = PolicyDecision::deny(&description, &reason);
                        self.emit_policy_event(ctx, &decision);
                        return decision;
                    }
                }
            }

            // Check require rules
            if let Some(ref require_rules) = policy.policy.require {
                for (idx, rule) in require_rules.iter().enumerate() {
                    if let Some(reason) = self.check_require_rule(rule, ctx, policy) {
                        let description = format!(
                            "require rule {} in {}",
                            idx + 1,
                            policy.mortar_id
                        );

                        let decision = PolicyDecision::deny(&description, &reason);
                        self.emit_policy_event(ctx, &decision);
                        return decision;
                    }
                }
            }

            // Check warn rules
            if let Some(ref warn_rules) = policy.policy.warn {
                for (idx, rule) in warn_rules.iter().enumerate() {
                    if let Some(reason) = self.matches_warn_rule(rule, ctx, policy) {
                        let description = format!(
                            "warn rule {} in {}",
                            idx + 1,
                            policy.mortar_id
                        );

                        warn!(
                            from = %ctx.from_service,
                            to = %ctx.to_target,
                            rule = %description,
                            reason = %reason,
                            "Policy warning triggered"
                        );

                        let decision = PolicyDecision::warn(&description, &reason);
                        self.emit_policy_event(ctx, &decision);
                        return decision;
                    }
                }
            }
        }

        // No rules matched, allow
        let decision = PolicyDecision::Allow;
        self.emit_policy_event(ctx, &decision);
        decision
    }

    /// Checks if a deny rule matches the connection.
    fn matches_deny_rule(
        &self,
        rule: &DenyRule,
        ctx: &PolicyEvaluationContext,
        policy: &EvaluatedPolicy,
    ) -> bool {
        // Check if source matches "from" (if specified)
        let from_matches = match &rule.from {
            Some(froms) => self.matches_service_list(froms, &ctx.from_service, policy),
            None => true, // No "from" means all sources
        };

        if !from_matches {
            return false;
        }

        // Check if target matches "to" (if specified)
        let to_matches = match &rule.to {
            Some(tos) => self.matches_target_list(tos, &ctx.to_target, policy),
            None => true, // No "to" means all targets
        };

        if !to_matches {
            return false;
        }

        // Check exceptions
        if let Some(ref exceptions) = rule.except {
            // If either source or target is in exceptions, don't match
            if self.matches_service_list(exceptions, &ctx.from_service, policy) {
                return false;
            }
            if self.matches_target_list(exceptions, &ctx.to_target, policy) {
                return false;
            }
        }

        true
    }

    /// Checks if a require rule is violated and returns the reason if so.
    fn check_require_rule(
        &self,
        rule: &RequireRule,
        ctx: &PolicyEvaluationContext,
        policy: &EvaluatedPolicy,
    ) -> Option<String> {
        // Check if this rule applies to the source service
        let applies = match &rule.services {
            Some(services) => self.matches_service_list(services, &ctx.from_service, policy),
            None => true, // Applies to all services in the mortar
        };

        if !applies {
            return None;
        }

        // For now, we can check audit requirement (logging is always on)
        // TLS and encryption would need connection metadata we don't have here
        // These could be enforced at connection time with additional context

        if rule.tls == Some(true) {
            // We can't enforce TLS at policy level without connection context
            // This would be checked during actual connection establishment
            debug!("TLS requirement noted but cannot be enforced at policy level");
        }

        if rule.encryption == Some(true) {
            // Similar to TLS
            debug!("Encryption requirement noted but cannot be enforced at policy level");
        }

        // Audit is always enabled when policy is evaluated
        // rule.audit is implicitly satisfied

        None
    }

    /// Checks if a warn rule matches and returns the warning reason if so.
    fn matches_warn_rule(
        &self,
        rule: &WarnRule,
        ctx: &PolicyEvaluationContext,
        policy: &EvaluatedPolicy,
    ) -> Option<String> {
        // Check for cross-network warning
        if rule.cross_network == Some(true) {
            // A connection is cross-network if:
            // - Target is not in the same mortar project
            // - OR target is an external address

            let target_in_mortar = self.is_target_in_mortar(&ctx.to_target, policy);
            let is_external = self.is_external_target(&ctx.to_target);

            if !target_in_mortar || is_external {
                // Check exceptions
                if let Some(ref exceptions) = rule.except {
                    if self.matches_target_list(exceptions, &ctx.to_target, policy) {
                        return None;
                    }
                }

                return Some("cross-network communication detected".to_string());
            }
        }

        None
    }

    /// Checks if a service ID matches any entry in the list.
    fn matches_service_list(
        &self,
        list: &[String],
        service_id: &str,
        policy: &EvaluatedPolicy,
    ) -> bool {
        // Get the service name for this ID
        let service_name = policy.service_name(service_id);

        for entry in list {
            // Direct service ID match
            if entry == service_id {
                return true;
            }

            // Service name match
            if let Some(name) = service_name {
                if entry == name {
                    return true;
                }
            }

            // Wildcard match
            if entry == "*" {
                return true;
            }
        }

        false
    }

    /// Checks if a target matches any entry in the list.
    fn matches_target_list(
        &self,
        list: &[String],
        target: &str,
        policy: &EvaluatedPolicy,
    ) -> bool {
        for entry in list {
            // Direct target match
            if entry == target {
                return true;
            }

            // Check if target is a service name in this mortar
            if let Some(target_id) = policy.service_id(entry) {
                if target_id == target {
                    return true;
                }
            }

            // Wildcard match
            if entry == "*" {
                return true;
            }

            // Check if it's a service name and the target matches the ID
            if policy.service_id(target).is_some() {
                // Target is a service name, check if entry matches its ID
                if let Some(target_id) = policy.service_id(target) {
                    if entry == target_id {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Checks if a target is within the same mortar project.
    fn is_target_in_mortar(&self, target: &str, policy: &EvaluatedPolicy) -> bool {
        // Check if target is a service ID in this mortar
        if policy.contains_service(target) {
            return true;
        }

        // Check if target is a service name in this mortar
        if policy.service_id(target).is_some() {
            return true;
        }

        false
    }

    /// Checks if a target is an external address (not a service).
    fn is_external_target(&self, target: &str) -> bool {
        // External targets typically contain:
        // - URLs (http://, https://)
        // - IP addresses
        // - Domain names with ports

        target.contains("://")
            || target.contains(':')
            || target.contains('.')
            || target.starts_with("http")
    }

    /// Emits a policy evaluation event.
    fn emit_policy_event(&self, ctx: &PolicyEvaluationContext, decision: &PolicyDecision) {
        let event = Event::new(
            EventType::PolicyEvaluated,
            serde_json::json!({
                "from_service": ctx.from_service,
                "to_target": ctx.to_target,
                "mortar_id": ctx.mortar_id,
                "decision": decision.decision_type(),
                "rule_description": decision.rule_description(),
                "reason": decision.reason()
            }),
        );

        // Spawn task to publish event without blocking
        let event_bus = Arc::clone(&self.event_bus);
        tokio::spawn(async move {
            event_bus.publish(event).await;
        });

        // For denials, also emit a violation event
        if decision.is_denied() {
            let violation_event = Event::new(
                EventType::PolicyViolation,
                serde_json::json!({
                    "from_service": ctx.from_service,
                    "to_target": ctx.to_target,
                    "mortar_id": ctx.mortar_id,
                    "rule_description": decision.rule_description(),
                    "reason": decision.reason()
                }),
            );

            let event_bus = Arc::clone(&self.event_bus);
            tokio::spawn(async move {
                event_bus.publish(violation_event).await;
            });
        }
    }
}

impl std::fmt::Debug for PolicyEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyEngine").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use fabricks_common::models::mortar::Policy;

    use crate::events::EventBus;

    use super::*;

    fn test_engine() -> PolicyEngine {
        let event_bus = Arc::new(EventBus::new(100, 100));
        PolicyEngine::new(event_bus)
    }

    fn test_policy_with_deny(from: Option<Vec<&str>>, to: Option<Vec<&str>>) -> EvaluatedPolicy {
        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-api-1".to_string());
        mappings.insert("db".to_string(), "svc-db-1".to_string());
        mappings.insert("cache".to_string(), "svc-cache-1".to_string());

        let policy = Policy {
            description: Some("Test policy".to_string()),
            deny: Some(vec![DenyRule {
                from: from.map(|v| v.into_iter().map(String::from).collect()),
                to: to.map(|v| v.into_iter().map(String::from).collect()),
                except: None,
                reason: Some("Test denial".to_string()),
            }]),
            require: None,
            warn: None,
        };

        EvaluatedPolicy::new("mortar-1", policy, mappings)
    }

    #[tokio::test]
    async fn test_allow_when_no_rules() {
        let engine = test_engine();
        let ctx = PolicyEvaluationContext::new("svc-api-1", "svc-db-1");
        let policies = vec![];

        let decision = engine.evaluate(&ctx, &policies);
        assert!(decision.is_allowed());
    }

    #[tokio::test]
    async fn test_deny_all_to_all() {
        let engine = test_engine();
        let policy = test_policy_with_deny(None, None);
        let ctx = PolicyEvaluationContext::new("svc-api-1", "svc-db-1");

        let decision = engine.evaluate(&ctx, &[policy]);
        assert!(decision.is_denied());
    }

    #[tokio::test]
    async fn test_deny_specific_from() {
        let engine = test_engine();
        let policy = test_policy_with_deny(Some(vec!["api"]), None);

        // Connection from api should be denied
        let ctx = PolicyEvaluationContext::new("svc-api-1", "svc-db-1");
        let decision = engine.evaluate(&ctx, &[policy.clone()]);
        assert!(decision.is_denied());

        // Connection from cache should be allowed
        let ctx = PolicyEvaluationContext::new("svc-cache-1", "svc-db-1");
        let decision = engine.evaluate(&ctx, &[policy]);
        assert!(decision.is_allowed());
    }

    #[tokio::test]
    async fn test_deny_specific_to() {
        let engine = test_engine();
        let policy = test_policy_with_deny(None, Some(vec!["db"]));

        // Connection to db should be denied
        let ctx = PolicyEvaluationContext::new("svc-api-1", "svc-db-1");
        let decision = engine.evaluate(&ctx, &[policy.clone()]);
        assert!(decision.is_denied());

        // Connection to cache should be allowed
        let ctx = PolicyEvaluationContext::new("svc-api-1", "svc-cache-1");
        let decision = engine.evaluate(&ctx, &[policy]);
        assert!(decision.is_allowed());
    }

    #[tokio::test]
    async fn test_deny_from_to_combination() {
        let engine = test_engine();
        let policy = test_policy_with_deny(Some(vec!["api"]), Some(vec!["db"]));

        // api -> db should be denied
        let ctx = PolicyEvaluationContext::new("svc-api-1", "svc-db-1");
        let decision = engine.evaluate(&ctx, &[policy.clone()]);
        assert!(decision.is_denied());

        // api -> cache should be allowed
        let ctx = PolicyEvaluationContext::new("svc-api-1", "svc-cache-1");
        let decision = engine.evaluate(&ctx, &[policy.clone()]);
        assert!(decision.is_allowed());

        // cache -> db should be allowed
        let ctx = PolicyEvaluationContext::new("svc-cache-1", "svc-db-1");
        let decision = engine.evaluate(&ctx, &[policy]);
        assert!(decision.is_allowed());
    }

    #[tokio::test]
    async fn test_deny_with_exception() {
        let engine = test_engine();

        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-api-1".to_string());
        mappings.insert("db".to_string(), "svc-db-1".to_string());
        mappings.insert("admin".to_string(), "svc-admin-1".to_string());

        let policy = Policy {
            description: None,
            deny: Some(vec![DenyRule {
                from: None,
                to: Some(vec!["db".to_string()]),
                except: Some(vec!["admin".to_string()]),
                reason: Some("Only admin can access db".to_string()),
            }]),
            require: None,
            warn: None,
        };

        let ep = EvaluatedPolicy::new("mortar-1", policy, mappings);

        // api -> db should be denied
        let ctx = PolicyEvaluationContext::new("svc-api-1", "svc-db-1");
        let decision = engine.evaluate(&ctx, &[ep.clone()]);
        assert!(decision.is_denied());

        // admin -> db should be allowed (exception)
        let ctx = PolicyEvaluationContext::new("svc-admin-1", "svc-db-1");
        let decision = engine.evaluate(&ctx, &[ep]);
        assert!(decision.is_allowed());
    }

    #[tokio::test]
    async fn test_warn_cross_network() {
        let engine = test_engine();

        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-api-1".to_string());

        let policy = Policy {
            description: None,
            deny: None,
            require: None,
            warn: Some(vec![WarnRule {
                cross_network: Some(true),
                except: None,
            }]),
        };

        let ep = EvaluatedPolicy::new("mortar-1", policy, mappings);

        // Connection to external should warn
        let ctx = PolicyEvaluationContext::new("svc-api-1", "https://api.example.com");
        let decision = engine.evaluate(&ctx, &[ep.clone()]);
        assert!(matches!(decision, PolicyDecision::Warn { .. }));

        // Connection within mortar should allow
        let ctx = PolicyEvaluationContext::new("svc-api-1", "svc-api-1");
        let decision = engine.evaluate(&ctx, &[ep]);
        assert!(decision.is_allowed());
    }

    #[tokio::test]
    async fn test_warn_with_exception() {
        let engine = test_engine();

        let mut mappings = HashMap::new();
        mappings.insert("api".to_string(), "svc-api-1".to_string());

        let policy = Policy {
            description: None,
            deny: None,
            require: None,
            warn: Some(vec![WarnRule {
                cross_network: Some(true),
                except: Some(vec!["https://allowed.example.com".to_string()]),
            }]),
        };

        let ep = EvaluatedPolicy::new("mortar-1", policy, mappings);

        // Connection to non-excepted external should warn
        let ctx = PolicyEvaluationContext::new("svc-api-1", "https://api.example.com");
        let decision = engine.evaluate(&ctx, &[ep.clone()]);
        assert!(matches!(decision, PolicyDecision::Warn { .. }));

        // Connection to excepted external should allow
        let ctx = PolicyEvaluationContext::new("svc-api-1", "https://allowed.example.com");
        let decision = engine.evaluate(&ctx, &[ep]);
        assert!(decision.is_allowed());
    }
}
