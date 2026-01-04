//! `MortarFile` data model.
//!
//! A `fabricks-mortar.toml` file composes multiple Fabricks into a complete application,
//! defining services, networks, volumes, secrets, and policies.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::common::{ByteSize, Duration, Labels, Replicas, Resources, RestartPolicy};
use super::fabrickfile::ServiceType;
use super::health_check::HealthCheck;

/// The current supported mortar file format version.
pub const MORTAR_VERSION: &str = "1.0";

/// A complete `fabricks-mortar.toml` definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MortarFile {
    /// Mortar file format version (required).
    pub mortar_version: String,

    /// Project-level metadata (required).
    pub project: Project,

    /// Reusable variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable: Option<HashMap<String, Variable>>,

    /// Secret definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<HashMap<String, Secret>>,

    /// Network definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<HashMap<String, Network>>,

    /// External host definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_hosts: Option<HashMap<String, ExternalHosts>>,

    /// Service definitions (required).
    pub service: HashMap<String, Service>,

    /// Volume definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<HashMap<String, Volume>>,

    /// Security policy definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<HashMap<String, Policy>>,

    /// Validation rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validate: Option<MortarValidate>,

    /// Deployment profiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<HashMap<String, Profile>>,
}

/// Project-level metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    /// Project name.
    pub name: String,

    /// Project version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Project description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Project authors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
}

/// Variable definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Variable {
    /// Variable type.
    #[serde(rename = "type")]
    pub var_type: VariableType,

    /// Default value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<toml::Value>,

    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Allowed values (for enums).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_values: Option<Vec<toml::Value>>,
}

/// Variable types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VariableType {
    /// String variable.
    String,
    /// Numeric variable.
    Number,
    /// Boolean variable.
    Boolean,
}

/// Secret definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Secret {
    /// Secret provider.
    pub provider: SecretProvider,

    /// Path for vault/file providers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Key name for vault/env providers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Secret providers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SecretProvider {
    /// Hashicorp Vault.
    Vault,
    /// Environment variable.
    Env,
    /// File on disk.
    File,
}

/// Network definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Network {
    /// Network description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Internal network (no external access).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal: Option<bool>,

    /// Completely isolated from other networks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolated: Option<bool>,

    /// Who can connect into this network.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingress: Option<NetworkIngress>,

    /// What this network can connect to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub egress: Option<Vec<String>>,

    /// Can only receive connections, not initiate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingress_only: Option<bool>,

    /// Audit all traffic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_all: Option<bool>,

    /// Encryption requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encryption: Option<EncryptionRequirement>,
}

/// Network ingress specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum NetworkIngress {
    /// CIDR notation for public access.
    Cidr(String),
    /// List of network names.
    Networks(Vec<String>),
}

/// Encryption requirement levels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EncryptionRequirement {
    /// Encryption is required.
    Required,
    /// Encryption is optional.
    Optional,
}

/// External hosts definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExternalHosts {
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// List of allowed host:port endpoints.
    pub hosts: Vec<String>,

    /// Require TLS for connections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_required: Option<bool>,
}

/// Service definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Service {
    /// Build from local Fabrickfile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,

    /// Use pre-built image from registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Override fabrick name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Override fabrick version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Service type (command, http, tcp).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub service_type: Option<ServiceType>,

    /// Networks this service belongs to.
    pub networks: Vec<String>,

    /// Environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,

    /// Port mappings (host:container or just port).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,

    /// Resource limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,

    /// Scaling configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicas: Option<Replicas>,

    /// Volume mounts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volumes: Option<HashMap<String, String>>,

    /// File mounts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,

    /// Health check configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheck>,

    /// Restart policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<RestartPolicy>,

    /// Service dependencies (start order).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,

    /// Component Model imports from other services.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imports: Option<HashMap<String, ServiceImport>>,

    /// Component Model exports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exports: Option<ServiceExports>,

    /// Persistence configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<Persistence>,

    /// Backup configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<Backup>,

    /// Audit configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<Audit>,

    /// Security configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<ServiceSecurity>,

    /// Labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,
}

/// Service import from another service (Component Model).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceImport {
    /// Service to import from.
    pub service: String,

    /// Interface to import.
    pub interface: String,
}

/// Service exports (Component Model).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceExports {
    /// List of exported interfaces.
    pub interfaces: Vec<String>,
}

/// Persistence configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Persistence {
    /// Enable persistence.
    pub enabled: bool,

    /// Persistence strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<PersistenceStrategy>,
}

/// Persistence strategies.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PersistenceStrategy {
    /// Append-only file.
    Aof,
    /// Redis database dump.
    Rdb,
    /// Both strategies.
    Both,
}

/// Backup configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Backup {
    /// Enable backups.
    pub enabled: bool,

    /// Backup schedule (cron format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,

    /// Retention period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention: Option<Duration>,

    /// Backup destination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
}

/// Audit configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Audit {
    /// Enable audit logging.
    pub enabled: bool,

    /// Audit log level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level: Option<AuditLogLevel>,

    /// Redact PII from logs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pii_redact: Option<bool>,
}

/// Audit log levels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuditLogLevel {
    /// Minimal logging.
    Minimal,
    /// Standard logging.
    Standard,
    /// Verbose logging.
    Verbose,
}

/// Service security configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceSecurity {
    /// Lock egress to explicit hosts only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub egress_locked: Option<bool>,

    /// Encrypt secrets at rest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets_encrypted: Option<bool>,

    /// Require TLS for all connections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_required: Option<bool>,

    /// Read-only root filesystem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only_root: Option<bool>,

    /// Run as specific user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Volume definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Volume {
    /// Volume size.
    pub size: ByteSize,

    /// Volume type.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub volume_type: Option<VolumeType>,

    /// Storage class (Kubernetes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,

    /// Access mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_mode: Option<AccessMode>,

    /// Enable encryption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,

    /// Backup configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<Backup>,
}

/// Volume types.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VolumeType {
    /// Persistent volume.
    #[default]
    Persistent,
    /// Ephemeral volume.
    Ephemeral,
}

/// Volume access modes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AccessMode {
    /// Read-write by one node.
    ReadWriteOnce,
    /// Read-only by many nodes.
    ReadOnlyMany,
    /// Read-write by many nodes.
    ReadWriteMany,
}

/// Security policy definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Policy {
    /// Policy description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Deny rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<DenyRule>>,

    /// Require rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require: Option<Vec<RequireRule>>,

    /// Warning rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn: Option<Vec<WarnRule>>,
}

/// Deny rule in a policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DenyRule {
    /// Source networks or services.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec<String>>,

    /// Destination networks or services.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec<String>>,

    /// Exceptions to the rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub except: Option<Vec<String>>,

    /// Reason for the denial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Require rule in a policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequireRule {
    /// Networks this rule applies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub networks: Option<Vec<String>>,

    /// Services this rule applies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<String>>,

    /// Require TLS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<bool>,

    /// Require audit logging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<bool>,

    /// Require encryption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encryption: Option<bool>,
}

/// Warning rule in a policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WarnRule {
    /// Warn on cross-network communication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_network: Option<bool>,

    /// Exceptions to the warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub except: Option<Vec<String>>,
}

/// Validation rules for mortar file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MortarValidate {
    /// Require all services to have health checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_health_checks: Option<bool>,

    /// Deny wildcard network connections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_wildcard_connect: Option<bool>,

    /// Require explicit capabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_explicit_capabilities: Option<bool>,

    /// Warn on old WASM versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn_on_old_wasm_versions: Option<bool>,

    /// Scan dependencies for vulnerabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_dependencies: Option<bool>,

    /// Check for circular dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_circular_dependencies: Option<bool>,
}

/// Deployment profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    /// Profile description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Deployment target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Kubernetes cluster name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster: Option<String>,

    /// Kubernetes namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Override settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<ProfileOverrides>,

    /// Profile-specific settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<ProfileSettings>,

    /// Approval requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<ApprovalRequirement>,
}

/// Profile override settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileOverrides {
    /// Apply to all services.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_services: Option<ServiceOverride>,
}

/// Service override settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceOverride {
    /// Override replicas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicas: Option<Replicas>,

    /// Override resources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,
}

/// Profile settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileSettings {
    /// Enable high availability mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_availability: Option<bool>,

    /// Enable monitoring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_monitoring: Option<bool>,

    /// Enable tracing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_tracing: Option<bool>,
}

/// Approval requirement for deployments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovalRequirement {
    /// Services requiring approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_for: Option<Vec<String>>,

    /// Approvers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approvers: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_mortar_deserialize() -> Result<(), toml::de::Error> {
        let toml = r#"
            mortar_version = "1.0"

            [project]
            name = "my-app"

            [service.api]
            build = "./api"
            networks = ["default"]

            [network.default]
            internal = true
        "#;

        let mortar: MortarFile = toml::from_str(toml)?;
        assert_eq!(mortar.mortar_version, "1.0");
        assert_eq!(mortar.project.name, "my-app");
        assert!(mortar.service.contains_key("api"));
        assert!(
            mortar
                .network
                .as_ref()
                .is_some_and(|n| n.contains_key("default"))
        );
        Ok(())
    }

    #[test]
    fn test_service_with_volumes() -> Result<(), toml::de::Error> {
        let toml = r#"
            mortar_version = "1.0"

            [project]
            name = "db-app"

            [service.postgres]
            image = "wasm://pglite:latest"
            networks = ["data"]

            [service.postgres.volumes]
            postgres_data = "/var/lib/postgresql/data"

            [volume.postgres_data]
            size = "50Gi"
            encrypted = true

            [network.data]
            internal = true
        "#;

        let mortar: MortarFile = toml::from_str(toml)?;

        // Verify postgres service exists and has the expected volume mount
        let postgres = mortar.service.get("postgres");
        assert!(postgres.is_some());
        let postgres = postgres.unwrap_or_else(|| unreachable!());
        assert!(
            postgres
                .volumes
                .as_ref()
                .is_some_and(|v| v.contains_key("postgres_data"))
        );

        // Verify volume exists with expected properties
        let volume = mortar.volume.as_ref().and_then(|v| v.get("postgres_data"));
        assert!(volume.is_some());
        let volume = volume.unwrap_or_else(|| unreachable!());
        assert_eq!(volume.size.as_gibibytes(), 50);
        assert_eq!(volume.encrypted, Some(true));

        Ok(())
    }
}
