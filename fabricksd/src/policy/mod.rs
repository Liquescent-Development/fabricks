//! Policy engine for enforcing security policies.
//!
//! This module provides policy evaluation for connections between services,
//! enforcing deny, require, and warn rules defined in mortar files.
//!
//! # Architecture
//!
//! - [`PolicyManager`] manages policy lifecycle (loading, unloading)
//! - [`PolicyEngine`] evaluates rules against connection requests
//! - [`PolicyDecision`] represents the outcome of policy evaluation
//!
//! # Usage
//!
//! Policies are automatically loaded when a mortar project is deployed and
//! unloaded when it's torn down. The policy manager is called during
//! connection validation to enforce policies.
//!
//! ```ignore
//! // Load policies for a mortar project
//! policy_manager.load_policies(mortar_id, policy, service_mappings).await;
//!
//! // Evaluate a connection
//! let decision = policy_manager.evaluate_connection(from_service, to_target).await;
//! match decision {
//!     PolicyDecision::Allow => { /* proceed */ }
//!     PolicyDecision::Deny { reason, .. } => { /* block connection */ }
//!     PolicyDecision::Warn { .. } => { /* log warning, proceed */ }
//! }
//! ```

mod engine;
mod manager;
mod types;

pub use engine::PolicyEngine;
pub use manager::{PolicyManager, SharedPolicyManager};
pub use types::{EvaluatedPolicy, PolicyDecision, PolicyEvaluationContext, PolicyInfo};
