//! Service type definitions.
//!
//! Defines the core types for service management including service state,
//! configuration, and instance tracking.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use fabricks_common::models::capability::Capabilities;
use fabricks_common::models::common::{Replicas, Resources};
use fabricks_common::models::health_check::HealthCheck;

use crate::volume::VolumeMount;

// Re-export ServiceType from common crate - the single source of truth
pub use fabricks_common::models::fabrickfile::ServiceType;

/// Service lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Service is being created.
    Creating,
    /// Service is created but not running.
    Stopped,
    /// Service is starting up.
    Starting,
    /// Service is running.
    Running,
    /// Service is stopping.
    Stopping,
    /// Service has failed.
    Failed,
    /// Service is being deleted.
    Deleting,
}

impl State {
    /// Returns whether the service can be started from this state.
    #[must_use]
    pub const fn can_start(&self) -> bool {
        matches!(self, Self::Stopped | Self::Failed)
    }

    /// Returns whether the service can be stopped from this state.
    #[must_use]
    pub const fn can_stop(&self) -> bool {
        matches!(self, Self::Running | Self::Starting)
    }

    /// Returns whether the service can be deleted from this state.
    #[must_use]
    pub const fn can_delete(&self) -> bool {
        matches!(self, Self::Stopped | Self::Failed | Self::Creating)
    }

    /// Returns whether the service is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped | Self::Failed)
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Creating => write!(f, "creating"),
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Failed => write!(f, "failed"),
            Self::Deleting => write!(f, "deleting"),
        }
    }
}

/// Instance lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceState {
    /// Instance is starting.
    Starting,
    /// Instance is running.
    Running,
    /// Instance is stopping.
    Stopping,
    /// Instance has stopped.
    Stopped,
    /// Instance has failed.
    Failed,
}

impl std::fmt::Display for InstanceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Configuration for creating a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name.
    pub name: String,

    /// Service version.
    pub version: String,

    /// Service type (command, http, tcp).
    #[serde(default)]
    pub service_type: ServiceType,

    /// Path to the WASM module.
    pub wasm_path: PathBuf,

    /// SHA256 digest of the WASM module.
    pub wasm_digest: String,

    /// Capabilities granted to the service.
    pub capabilities: Capabilities,

    /// Environment variables.
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// Command-line arguments.
    #[serde(default)]
    pub args: Vec<String>,

    /// Resource limits.
    #[serde(default)]
    pub resources: Option<Resources>,

    /// Replica configuration.
    #[serde(default)]
    pub replicas: Replicas,

    /// Health check configuration.
    #[serde(default)]
    pub health_check: Option<HealthCheck>,

    /// Service dependencies (other service names).
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Networks this service belongs to.
    #[serde(default)]
    pub networks: Vec<String>,

    /// Volume mounts for persistent storage.
    #[serde(default)]
    pub volumes: Vec<VolumeMount>,

    /// Optional mortar project this service belongs to.
    #[serde(default)]
    pub mortar_project: Option<String>,
}

impl ServiceConfig {
    /// Creates a new service configuration.
    #[must_use]
    pub fn new(name: String, version: String, wasm_path: PathBuf, wasm_digest: String) -> Self {
        Self {
            name,
            version,
            service_type: ServiceType::default(),
            wasm_path,
            wasm_digest,
            capabilities: Capabilities::default(),
            environment: HashMap::new(),
            args: Vec::new(),
            resources: None,
            replicas: Replicas::default(),
            health_check: None,
            depends_on: Vec::new(),
            networks: Vec::new(),
            volumes: Vec::new(),
            mortar_project: None,
        }
    }
}

/// Persisted service state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    /// Unique service ID.
    pub id: String,

    /// Service name.
    pub name: String,

    /// Service version.
    pub version: String,

    /// Current service state.
    pub state: State,

    /// Replica state.
    pub replicas: ReplicaState,

    /// Service configuration.
    pub config: ServiceConfig,

    /// When the service was created.
    pub created_at: DateTime<Utc>,

    /// When the service was last updated.
    pub updated_at: DateTime<Utc>,

    /// Last error message if in failed state.
    #[serde(default)]
    pub last_error: Option<String>,

    /// Optional mortar project this service belongs to.
    #[serde(default)]
    pub mortar_project: Option<String>,
}

impl ServiceState {
    /// Creates a new service state.
    #[must_use]
    pub fn new(config: ServiceConfig) -> Self {
        let now = Utc::now();
        let id = generate_service_id();
        Self {
            id,
            name: config.name.clone(),
            version: config.version.clone(),
            state: State::Creating,
            replicas: ReplicaState::default(),
            config,
            created_at: now,
            updated_at: now,
            last_error: None,
            mortar_project: None,
        }
    }

    /// Updates the service state.
    pub fn set_state(&mut self, state: State) {
        self.state = state;
        self.updated_at = Utc::now();
    }

    /// Sets the error message.
    pub fn set_error(&mut self, error: String) {
        self.last_error = Some(error);
        self.updated_at = Utc::now();
    }
}

/// Current replica state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplicaState {
    /// Number of desired replicas.
    pub desired: usize,

    /// Number of ready replicas.
    pub ready: usize,

    /// Number of running replicas.
    pub running: usize,

    /// Number of failed replicas.
    pub failed: usize,
}

/// Individual instance state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    /// Unique instance ID.
    pub id: String,

    /// Parent service ID.
    pub service_id: String,

    /// Instance state.
    pub state: InstanceState,

    /// When the instance was started.
    pub started_at: DateTime<Utc>,

    /// When the instance was last updated.
    pub updated_at: DateTime<Utc>,

    /// Exit code if instance has stopped.
    #[serde(default)]
    pub exit_code: Option<i32>,

    /// Error message if instance failed.
    #[serde(default)]
    pub error: Option<String>,
}

impl Instance {
    /// Creates a new instance.
    #[must_use]
    pub fn new(service_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: generate_instance_id(),
            service_id,
            state: InstanceState::Starting,
            started_at: now,
            updated_at: now,
            exit_code: None,
            error: None,
        }
    }

    /// Updates the instance state.
    pub fn set_state(&mut self, state: InstanceState) {
        self.state = state;
        self.updated_at = Utc::now();
    }

    /// Sets the exit code and marks as stopped.
    pub fn set_exit(&mut self, code: i32) {
        self.exit_code = Some(code);
        self.state = if code == 0 {
            InstanceState::Stopped
        } else {
            InstanceState::Failed
        };
        self.updated_at = Utc::now();
    }

    /// Sets the error and marks as failed.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.state = InstanceState::Failed;
        self.updated_at = Utc::now();
    }
}

/// Information about a service for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Service ID.
    pub id: String,

    /// Service name.
    pub name: String,

    /// Service version.
    pub version: String,

    /// Current state.
    pub state: State,

    /// Replica information.
    pub replicas: ReplicaState,

    /// When the service was created.
    pub created_at: DateTime<Utc>,

    /// Optional mortar project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mortar_project: Option<String>,
}

impl From<&ServiceState> for ServiceInfo {
    fn from(state: &ServiceState) -> Self {
        Self {
            id: state.id.clone(),
            name: state.name.clone(),
            version: state.version.clone(),
            state: state.state,
            replicas: state.replicas.clone(),
            created_at: state.created_at,
            mortar_project: state.mortar_project.clone(),
        }
    }
}

/// Detailed service information for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDetail {
    /// Service ID.
    pub id: String,

    /// Service name.
    pub name: String,

    /// Service version.
    pub version: String,

    /// Current state.
    pub state: State,

    /// Replica information.
    pub replicas: ReplicaState,

    /// Service configuration.
    pub config: ServiceConfig,

    /// When the service was created.
    pub created_at: DateTime<Utc>,

    /// When the service was last updated.
    pub updated_at: DateTime<Utc>,

    /// Last error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,

    /// Running instances.
    pub instances: Vec<Instance>,

    /// Optional mortar project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mortar_project: Option<String>,

    /// Bound ports (populated from proxy server).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<u16>,
}

/// Generates a unique service ID.
#[must_use]
pub fn generate_service_id() -> String {
    let uuid = Uuid::new_v4();
    format!("svc-{}", &uuid.to_string()[..8])
}

/// Generates a unique instance ID.
#[must_use]
pub fn generate_instance_id() -> String {
    let uuid = Uuid::new_v4();
    format!("inst-{}", &uuid.to_string()[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        assert!(State::Stopped.can_start());
        assert!(State::Failed.can_start());
        assert!(!State::Running.can_start());
        assert!(!State::Starting.can_start());

        assert!(State::Running.can_stop());
        assert!(State::Starting.can_stop());
        assert!(!State::Stopped.can_stop());

        assert!(State::Stopped.can_delete());
        assert!(State::Failed.can_delete());
        assert!(!State::Running.can_delete());
    }

    #[test]
    fn test_state_display() {
        assert_eq!(State::Running.to_string(), "running");
        assert_eq!(State::Stopped.to_string(), "stopped");
        assert_eq!(State::Failed.to_string(), "failed");
    }

    #[test]
    fn test_service_id_generation() {
        let id1 = generate_service_id();
        let id2 = generate_service_id();

        assert!(id1.starts_with("svc-"));
        assert!(id2.starts_with("svc-"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_instance_id_generation() {
        let id1 = generate_instance_id();
        let id2 = generate_instance_id();

        assert!(id1.starts_with("inst-"));
        assert!(id2.starts_with("inst-"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_service_state_creation() {
        let config = ServiceConfig::new(
            "test-service".to_string(),
            "1.0.0".to_string(),
            PathBuf::from("/tmp/test.wasm"),
            "sha256:abc123".to_string(),
        );

        let state = ServiceState::new(config);

        assert!(state.id.starts_with("svc-"));
        assert_eq!(state.name, "test-service");
        assert_eq!(state.version, "1.0.0");
        assert_eq!(state.state, State::Creating);
    }

    #[test]
    fn test_instance_state_transitions() {
        let mut instance = Instance::new("svc-123".to_string());
        assert_eq!(instance.state, InstanceState::Starting);

        instance.set_state(InstanceState::Running);
        assert_eq!(instance.state, InstanceState::Running);

        instance.set_exit(0);
        assert_eq!(instance.state, InstanceState::Stopped);

        let mut instance2 = Instance::new("svc-456".to_string());
        instance2.set_exit(1);
        assert_eq!(instance2.state, InstanceState::Failed);
    }

    #[test]
    fn test_service_info_from_state() {
        let config = ServiceConfig::new(
            "my-service".to_string(),
            "2.0.0".to_string(),
            PathBuf::from("/wasm/module.wasm"),
            "sha256:def456".to_string(),
        );

        let state = ServiceState::new(config);
        let info = ServiceInfo::from(&state);

        assert_eq!(info.id, state.id);
        assert_eq!(info.name, "my-service");
        assert_eq!(info.version, "2.0.0");
    }
}
