//! End-to-end tests for policy engine functionality.
//!
//! These tests verify that policy evaluation works correctly
//! with deny, require, and warn rules.

use std::collections::HashMap;

use fabricks_common::models::mortar::{DenyRule, Policy, WarnRule};
use fabricksd::events::EventType;
use fabricksd::policy::{PolicyDecision, PolicyInfo};

/// Test that PolicyManager can load and unload policies.
#[tokio::test]
async fn test_policy_load_unload() {
    use fabricksd::events::EventBus;
    use fabricksd::policy::PolicyManager;
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new(100, 100));
    let manager = PolicyManager::new(event_bus);

    let mut mappings = HashMap::new();
    mappings.insert("api".to_string(), "svc-api-1".to_string());
    mappings.insert("db".to_string(), "svc-db-1".to_string());

    let policy = Policy {
        description: Some("Test policy".to_string()),
        deny: None,
        require: None,
        warn: None,
    };

    // Load policy
    manager
        .load_policies("mortar-test", policy, mappings)
        .await;

    // Verify loaded
    let policies = manager.list_policies().await;
    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0].mortar_id, "mortar-test");

    // Unload policy
    manager.unload_policies("mortar-test").await;

    // Verify unloaded
    let policies = manager.list_policies().await;
    assert!(policies.is_empty());
}

/// Test deny rule blocks connections.
#[tokio::test]
async fn test_deny_rule_blocks_connection() {
    use fabricksd::events::EventBus;
    use fabricksd::policy::PolicyManager;
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new(100, 100));
    let manager = PolicyManager::new(event_bus);

    let mut mappings = HashMap::new();
    mappings.insert("api".to_string(), "svc-api-1".to_string());
    mappings.insert("db".to_string(), "svc-db-1".to_string());

    let policy = Policy {
        description: Some("Block api from db".to_string()),
        deny: Some(vec![DenyRule {
            from: Some(vec!["api".to_string()]),
            to: Some(vec!["db".to_string()]),
            except: None,
            reason: Some("API cannot directly access database".to_string()),
        }]),
        require: None,
        warn: None,
    };

    manager
        .load_policies("mortar-test", policy, mappings)
        .await;

    // api -> db should be denied
    let decision = manager
        .evaluate_connection("svc-api-1", "svc-db-1")
        .await;

    assert!(decision.is_denied());
    assert!(decision.reason().unwrap().contains("API cannot directly access database"));
}

/// Test deny rule with exception allows excepted connections.
#[tokio::test]
async fn test_deny_rule_with_exception() {
    use fabricksd::events::EventBus;
    use fabricksd::policy::PolicyManager;
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new(100, 100));
    let manager = PolicyManager::new(event_bus);

    let mut mappings = HashMap::new();
    mappings.insert("api".to_string(), "svc-api-1".to_string());
    mappings.insert("admin".to_string(), "svc-admin-1".to_string());
    mappings.insert("db".to_string(), "svc-db-1".to_string());

    let policy = Policy {
        description: Some("Block all to db except admin".to_string()),
        deny: Some(vec![DenyRule {
            from: None,  // All sources
            to: Some(vec!["db".to_string()]),
            except: Some(vec!["admin".to_string()]),
            reason: Some("Only admin can access database".to_string()),
        }]),
        require: None,
        warn: None,
    };

    manager
        .load_policies("mortar-test", policy, mappings)
        .await;

    // api -> db should be denied
    let decision = manager
        .evaluate_connection("svc-api-1", "svc-db-1")
        .await;
    assert!(decision.is_denied());

    // admin -> db should be allowed (exception)
    let decision = manager
        .evaluate_connection("svc-admin-1", "svc-db-1")
        .await;
    assert!(decision.is_allowed());
}

/// Test warn rule allows but logs warning.
#[tokio::test]
async fn test_warn_rule_allows_with_warning() {
    use fabricksd::events::EventBus;
    use fabricksd::policy::PolicyManager;
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new(100, 100));
    let manager = PolicyManager::new(event_bus);

    let mut mappings = HashMap::new();
    mappings.insert("api".to_string(), "svc-api-1".to_string());

    let policy = Policy {
        description: Some("Warn on cross-network".to_string()),
        deny: None,
        require: None,
        warn: Some(vec![WarnRule {
            cross_network: Some(true),
            except: None,
        }]),
    };

    manager
        .load_policies("mortar-test", policy, mappings)
        .await;

    // Connection to external should warn
    let decision = manager
        .evaluate_connection("svc-api-1", "https://api.example.com")
        .await;

    assert!(decision.is_allowed());
    assert!(matches!(decision, PolicyDecision::Warn { .. }));
}

/// Test connections without policies are allowed.
#[tokio::test]
async fn test_no_policy_allows_connection() {
    use fabricksd::events::EventBus;
    use fabricksd::policy::PolicyManager;
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new(100, 100));
    let manager = PolicyManager::new(event_bus);

    // No policies loaded
    let decision = manager
        .evaluate_connection("svc-unknown", "svc-other")
        .await;

    assert!(decision.is_allowed());
    assert!(matches!(decision, PolicyDecision::Allow));
}

/// Test multiple deny rules - first match wins.
#[tokio::test]
async fn test_multiple_deny_rules() {
    use fabricksd::events::EventBus;
    use fabricksd::policy::PolicyManager;
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new(100, 100));
    let manager = PolicyManager::new(event_bus);

    let mut mappings = HashMap::new();
    mappings.insert("api".to_string(), "svc-api-1".to_string());
    mappings.insert("worker".to_string(), "svc-worker-1".to_string());
    mappings.insert("db".to_string(), "svc-db-1".to_string());

    let policy = Policy {
        description: Some("Multiple deny rules".to_string()),
        deny: Some(vec![
            DenyRule {
                from: Some(vec!["api".to_string()]),
                to: Some(vec!["db".to_string()]),
                except: None,
                reason: Some("Reason 1".to_string()),
            },
            DenyRule {
                from: Some(vec!["worker".to_string()]),
                to: Some(vec!["db".to_string()]),
                except: None,
                reason: Some("Reason 2".to_string()),
            },
        ]),
        require: None,
        warn: None,
    };

    manager
        .load_policies("mortar-test", policy, mappings)
        .await;

    // api -> db matches first rule
    let decision = manager
        .evaluate_connection("svc-api-1", "svc-db-1")
        .await;
    assert!(decision.is_denied());
    assert!(decision.reason().unwrap().contains("Reason 1"));

    // worker -> db matches second rule
    let decision = manager
        .evaluate_connection("svc-worker-1", "svc-db-1")
        .await;
    assert!(decision.is_denied());
    assert!(decision.reason().unwrap().contains("Reason 2"));
}

/// Test PolicyInfo serialization.
#[test]
fn test_policy_info_serialization() {
    let info = PolicyInfo {
        mortar_id: "mortar-1".to_string(),
        description: Some("Test".to_string()),
        deny_rules: 2,
        require_rules: 1,
        warn_rules: 0,
        services: vec!["api".to_string(), "db".to_string()],
    };

    let json = serde_json::to_string(&info).expect("should serialize");
    assert!(json.contains("\"mortar_id\":\"mortar-1\""));
    assert!(json.contains("\"deny_rules\":2"));
}

/// Test PolicyDecision serialization.
#[test]
fn test_policy_decision_serialization() {
    let allow = PolicyDecision::Allow;
    let json = serde_json::to_string(&allow).expect("should serialize");
    assert!(json.contains("\"decision\":\"allow\""));

    let deny = PolicyDecision::deny("rule 1", "not allowed");
    let json = serde_json::to_string(&deny).expect("should serialize");
    assert!(json.contains("\"decision\":\"deny\""));
    assert!(json.contains("\"reason\":\"not allowed\""));

    let warn = PolicyDecision::warn("rule 2", "suspicious");
    let json = serde_json::to_string(&warn).expect("should serialize");
    assert!(json.contains("\"decision\":\"warn\""));
}

/// Test policy events are emitted.
#[tokio::test]
async fn test_policy_events_emitted() {
    use fabricksd::events::EventBus;
    use fabricksd::policy::PolicyManager;
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new(100, 100));
    let manager = PolicyManager::new(Arc::clone(&event_bus));

    let mut mappings = HashMap::new();
    mappings.insert("api".to_string(), "svc-api-1".to_string());
    mappings.insert("db".to_string(), "svc-db-1".to_string());

    let policy = Policy {
        description: Some("Test policy".to_string()),
        deny: Some(vec![DenyRule {
            from: Some(vec!["api".to_string()]),
            to: Some(vec!["db".to_string()]),
            except: None,
            reason: Some("Denied".to_string()),
        }]),
        require: None,
        warn: None,
    };

    manager
        .load_policies("mortar-test", policy, mappings)
        .await;

    // Subscribe to events before triggering the connection
    let mut rx = event_bus.subscribe().await;

    // Trigger a denied connection
    let _ = manager
        .evaluate_connection("svc-api-1", "svc-db-1")
        .await;

    // Check events were emitted
    let mut found_evaluated = false;
    let mut found_violation = false;

    // Wait for events with timeout
    for _ in 0..10 {
        match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
            Ok(Some(event)) => {
                if event.event_type == EventType::PolicyEvaluated {
                    found_evaluated = true;
                }
                if event.event_type == EventType::PolicyViolation {
                    found_violation = true;
                }
                if found_evaluated && found_violation {
                    break;
                }
            }
            _ => break,
        }
    }

    assert!(found_evaluated, "Should have PolicyEvaluated event");
    assert!(found_violation, "Should have PolicyViolation event");
}

/// Test wildcard deny rule.
#[tokio::test]
async fn test_wildcard_deny_rule() {
    use fabricksd::events::EventBus;
    use fabricksd::policy::PolicyManager;
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new(100, 100));
    let manager = PolicyManager::new(event_bus);

    let mut mappings = HashMap::new();
    mappings.insert("api".to_string(), "svc-api-1".to_string());
    mappings.insert("db".to_string(), "svc-db-1".to_string());

    let policy = Policy {
        description: Some("Deny all".to_string()),
        deny: Some(vec![DenyRule {
            from: None,  // All sources (wildcard)
            to: None,    // All targets (wildcard)
            except: None,
            reason: Some("All connections denied".to_string()),
        }]),
        require: None,
        warn: None,
    };

    manager
        .load_policies("mortar-test", policy, mappings)
        .await;

    // All connections should be denied
    let decision = manager
        .evaluate_connection("svc-api-1", "svc-db-1")
        .await;
    assert!(decision.is_denied());

    let decision = manager
        .evaluate_connection("svc-db-1", "svc-api-1")
        .await;
    assert!(decision.is_denied());
}

/// Test isolated mortar projects don't affect each other.
#[tokio::test]
async fn test_mortar_isolation() {
    use fabricksd::events::EventBus;
    use fabricksd::policy::PolicyManager;
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new(100, 100));
    let manager = PolicyManager::new(event_bus);

    // Mortar 1: Has a deny rule
    let mut mappings1 = HashMap::new();
    mappings1.insert("api".to_string(), "svc-api-1".to_string());

    let policy1 = Policy {
        description: Some("Mortar 1 - restrictive".to_string()),
        deny: Some(vec![DenyRule {
            from: None,
            to: None,
            except: None,
            reason: Some("Deny all in mortar 1".to_string()),
        }]),
        require: None,
        warn: None,
    };

    // Mortar 2: No deny rules
    let mut mappings2 = HashMap::new();
    mappings2.insert("worker".to_string(), "svc-worker-1".to_string());

    let policy2 = Policy {
        description: Some("Mortar 2 - permissive".to_string()),
        deny: None,
        require: None,
        warn: None,
    };

    manager.load_policies("mortar-1", policy1, mappings1).await;
    manager.load_policies("mortar-2", policy2, mappings2).await;

    // Service in mortar-1 should be denied
    let decision = manager
        .evaluate_connection("svc-api-1", "external.com")
        .await;
    assert!(decision.is_denied());

    // Service in mortar-2 should be allowed
    let decision = manager
        .evaluate_connection("svc-worker-1", "external.com")
        .await;
    assert!(decision.is_allowed());
}
