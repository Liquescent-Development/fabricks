//! Volume type definitions.
//!
//! Defines the core types for volume management including volume configuration,
//! state, and mount tracking.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Volume configuration for creating a volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeConfig {
    /// Volume name.
    pub name: String,

    /// Volume description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Requested size (informational, not enforced).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

impl VolumeConfig {
    /// Creates a new volume configuration with the given name.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            size: None,
        }
    }

    /// Creates a volume configuration with description.
    #[must_use]
    pub fn with_description(name: String, description: String) -> Self {
        Self {
            name,
            description: Some(description),
            size: None,
        }
    }

    /// Creates a volume configuration with size.
    #[must_use]
    pub fn with_size(name: String, size: String) -> Self {
        Self {
            name,
            description: None,
            size: Some(size),
        }
    }
}

/// Persisted volume state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeState {
    /// Unique volume ID.
    pub id: String,

    /// Volume name.
    pub name: String,

    /// Volume description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Requested size (informational).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Path to the volume data directory on the host.
    pub path: PathBuf,

    /// Services that currently have this volume mounted.
    pub mounted_by: Vec<String>,

    /// When the volume was created.
    pub created_at: DateTime<Utc>,

    /// When the volume was last updated.
    pub updated_at: DateTime<Utc>,
}

impl VolumeState {
    /// Creates a new volume state from configuration.
    #[must_use]
    pub fn new(config: VolumeConfig, base_path: &Path) -> Self {
        let now = Utc::now();
        let id = generate_volume_id();
        let path = base_path.join(&id);
        Self {
            id,
            name: config.name,
            description: config.description,
            size: config.size,
            path,
            mounted_by: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Marks this volume as mounted by a service.
    pub fn add_mount(&mut self, service_id: String) {
        if !self.mounted_by.contains(&service_id) {
            self.mounted_by.push(service_id);
            self.updated_at = Utc::now();
        }
    }

    /// Removes a mount for a service.
    pub fn remove_mount(&mut self, service_id: &str) {
        self.mounted_by.retain(|id| id != service_id);
        self.updated_at = Utc::now();
    }

    /// Checks if this volume is mounted by any service.
    #[must_use]
    pub fn is_mounted(&self) -> bool {
        !self.mounted_by.is_empty()
    }

    /// Checks if this volume is mounted by a specific service.
    #[must_use]
    pub fn is_mounted_by(&self, service_id: &str) -> bool {
        self.mounted_by.contains(&service_id.to_string())
    }

    /// Returns the number of services mounting this volume.
    #[must_use]
    pub fn mount_count(&self) -> usize {
        self.mounted_by.len()
    }
}

/// Information about a volume for API responses (list view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    /// Volume ID.
    pub id: String,

    /// Volume name.
    pub name: String,

    /// Volume description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Requested size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Number of services mounting this volume.
    pub mount_count: usize,

    /// When the volume was created.
    pub created_at: DateTime<Utc>,
}

impl From<&VolumeState> for VolumeInfo {
    fn from(state: &VolumeState) -> Self {
        Self {
            id: state.id.clone(),
            name: state.name.clone(),
            description: state.description.clone(),
            size: state.size.clone(),
            mount_count: state.mount_count(),
            created_at: state.created_at,
        }
    }
}

/// Detailed information about a volume for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeDetail {
    /// Volume ID.
    pub id: String,

    /// Volume name.
    pub name: String,

    /// Volume description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Requested size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Path to the volume data directory on the host.
    pub path: PathBuf,

    /// Services that currently have this volume mounted.
    pub mounted_by: Vec<String>,

    /// When the volume was created.
    pub created_at: DateTime<Utc>,

    /// When the volume was last updated.
    pub updated_at: DateTime<Utc>,
}

impl From<&VolumeState> for VolumeDetail {
    fn from(state: &VolumeState) -> Self {
        Self {
            id: state.id.clone(),
            name: state.name.clone(),
            description: state.description.clone(),
            size: state.size.clone(),
            path: state.path.clone(),
            mounted_by: state.mounted_by.clone(),
            created_at: state.created_at,
            updated_at: state.updated_at,
        }
    }
}

/// Volume mount configuration for services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    /// Volume ID.
    pub volume_id: String,

    /// Volume name (for reference).
    pub volume_name: String,

    /// Path to the volume on the host.
    pub host_path: PathBuf,

    /// Path where the volume is mounted in the guest.
    pub guest_path: String,

    /// Whether the mount is read-only.
    #[serde(default)]
    pub read_only: bool,
}

impl VolumeMount {
    /// Creates a new read-write volume mount.
    #[must_use]
    pub fn new(
        volume_id: String,
        volume_name: String,
        host_path: PathBuf,
        guest_path: String,
    ) -> Self {
        Self {
            volume_id,
            volume_name,
            host_path,
            guest_path,
            read_only: false,
        }
    }

    /// Creates a new read-only volume mount.
    #[must_use]
    pub fn read_only(
        volume_id: String,
        volume_name: String,
        host_path: PathBuf,
        guest_path: String,
    ) -> Self {
        Self {
            volume_id,
            volume_name,
            host_path,
            guest_path,
            read_only: true,
        }
    }
}

/// Generates a unique volume ID.
#[must_use]
pub fn generate_volume_id() -> String {
    let uuid = Uuid::new_v4();
    format!("vol-{}", &uuid.to_string()[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_config_new() {
        let config = VolumeConfig::new("test-vol".to_string());
        assert_eq!(config.name, "test-vol");
        assert!(config.description.is_none());
        assert!(config.size.is_none());
    }

    #[test]
    fn test_volume_config_with_description() {
        let config =
            VolumeConfig::with_description("test-vol".to_string(), "Test volume".to_string());
        assert_eq!(config.name, "test-vol");
        assert_eq!(config.description, Some("Test volume".to_string()));
    }

    #[test]
    fn test_volume_config_with_size() {
        let config = VolumeConfig::with_size("test-vol".to_string(), "10Gi".to_string());
        assert_eq!(config.name, "test-vol");
        assert_eq!(config.size, Some("10Gi".to_string()));
    }

    #[test]
    fn test_volume_state_new() {
        let config = VolumeConfig::new("test-vol".to_string());
        let base_path = PathBuf::from("/var/lib/fabricks/volumes");
        let state = VolumeState::new(config, &base_path);

        assert!(state.id.starts_with("vol-"));
        assert_eq!(state.name, "test-vol");
        assert!(state.path.starts_with(&base_path));
        assert!(!state.is_mounted());
    }

    #[test]
    fn test_volume_state_mounts() {
        let config = VolumeConfig::new("test-vol".to_string());
        let base_path = PathBuf::from("/var/lib/fabricks/volumes");
        let mut state = VolumeState::new(config, &base_path);

        assert!(!state.is_mounted());
        assert_eq!(state.mount_count(), 0);

        state.add_mount("svc-1".to_string());
        assert!(state.is_mounted());
        assert!(state.is_mounted_by("svc-1"));
        assert_eq!(state.mount_count(), 1);

        state.add_mount("svc-2".to_string());
        assert_eq!(state.mount_count(), 2);

        // Adding same service again shouldn't increase count
        state.add_mount("svc-1".to_string());
        assert_eq!(state.mount_count(), 2);

        state.remove_mount("svc-1");
        assert!(!state.is_mounted_by("svc-1"));
        assert!(state.is_mounted_by("svc-2"));
        assert_eq!(state.mount_count(), 1);

        state.remove_mount("svc-2");
        assert!(!state.is_mounted());
    }

    #[test]
    fn test_volume_info_from_state() {
        let config =
            VolumeConfig::with_description("test-vol".to_string(), "Test description".to_string());
        let base_path = PathBuf::from("/var/lib/fabricks/volumes");
        let mut state = VolumeState::new(config, &base_path);
        state.add_mount("svc-1".to_string());

        let info: VolumeInfo = (&state).into();
        assert_eq!(info.id, state.id);
        assert_eq!(info.name, "test-vol");
        assert_eq!(info.description, Some("Test description".to_string()));
        assert_eq!(info.mount_count, 1);
    }

    #[test]
    fn test_volume_detail_from_state() {
        let config = VolumeConfig::new("test-vol".to_string());
        let base_path = PathBuf::from("/var/lib/fabricks/volumes");
        let mut state = VolumeState::new(config, &base_path);
        state.add_mount("svc-1".to_string());

        let detail: VolumeDetail = (&state).into();
        assert_eq!(detail.id, state.id);
        assert_eq!(detail.name, "test-vol");
        assert_eq!(detail.path, state.path);
        assert_eq!(detail.mounted_by, vec!["svc-1".to_string()]);
    }

    #[test]
    fn test_volume_mount() {
        let mount = VolumeMount::new(
            "vol-123".to_string(),
            "data".to_string(),
            PathBuf::from("/var/lib/fabricks/volumes/vol-123"),
            "/data".to_string(),
        );

        assert_eq!(mount.volume_id, "vol-123");
        assert_eq!(mount.guest_path, "/data");
        assert!(!mount.read_only);

        let ro_mount = VolumeMount::read_only(
            "vol-456".to_string(),
            "config".to_string(),
            PathBuf::from("/var/lib/fabricks/volumes/vol-456"),
            "/config".to_string(),
        );

        assert!(ro_mount.read_only);
    }

    #[test]
    fn test_volume_id_generation() {
        let id1 = generate_volume_id();
        let id2 = generate_volume_id();

        assert!(id1.starts_with("vol-"));
        assert!(id2.starts_with("vol-"));
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 12); // "vol-" + 8 hex chars
    }
}
