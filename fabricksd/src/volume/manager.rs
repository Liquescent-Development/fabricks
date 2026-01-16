//! Volume manager for volume lifecycle and mounting.
//!
//! Provides CRUD operations for volumes and manages mounting/unmounting
//! to services. The manager is responsible for creating volume directories
//! and tracking mount state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::{DaemonError, Result};
use crate::store::StateStore;

use super::types::{VolumeConfig, VolumeDetail, VolumeInfo, VolumeState};

/// Volume manager for volume lifecycle and mounting.
///
/// Manages the creation, deletion, and mounting of volumes.
/// Volumes provide persistent storage that can be shared between services.
#[derive(Debug)]
pub struct VolumeManager {
    /// Active volumes indexed by ID.
    volumes: RwLock<HashMap<String, VolumeState>>,

    /// Base path for volume storage.
    base_path: PathBuf,

    /// Persistent state store.
    state_store: Arc<StateStore>,
}

impl VolumeManager {
    /// Creates a new volume manager.
    ///
    /// # Arguments
    ///
    /// * `state_store` - The persistent state store
    /// * `base_path` - Base directory for volume data (e.g., `/var/lib/fabricks/volumes`)
    #[must_use]
    pub fn new(state_store: Arc<StateStore>, base_path: PathBuf) -> Self {
        Self {
            volumes: RwLock::new(HashMap::new()),
            base_path,
            state_store,
        }
    }

    /// Loads persisted volume state from the state store.
    ///
    /// Should be called during daemon startup to restore volume state.
    ///
    /// # Errors
    ///
    /// Returns an error if state cannot be loaded.
    pub async fn load_state(&self) -> Result<()> {
        let volumes: Vec<VolumeState> = self.state_store.list_volumes()?;
        let mut map = self.volumes.write().await;

        for volume in volumes {
            info!(id = %volume.id, name = %volume.name, "Loaded volume from state");
            map.insert(volume.id.clone(), volume);
        }

        info!(count = map.len(), "Loaded volumes from state store");
        Ok(())
    }

    /// Creates a new volume.
    ///
    /// # Errors
    ///
    /// Returns an error if a volume with the same name already exists
    /// or if the directory cannot be created.
    pub async fn create_volume(&self, config: VolumeConfig) -> Result<String> {
        let mut volumes = self.volumes.write().await;

        // Check for duplicate name
        for existing in volumes.values() {
            if existing.name == config.name {
                return Err(DaemonError::VolumeExists(config.name));
            }
        }

        let state = VolumeState::new(config, &self.base_path);
        let id = state.id.clone();

        info!(id = %id, name = %state.name, path = ?state.path, "Creating volume");

        // Create the volume directory
        std::fs::create_dir_all(&state.path).map_err(|e| DaemonError::VolumeCreateFailed {
            name: state.name.clone(),
            reason: e.to_string(),
        })?;

        // Persist to state store
        self.persist_volume(&state)?;

        volumes.insert(id.clone(), state);

        Ok(id)
    }

    /// Deletes a volume by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the volume does not exist or is mounted.
    pub async fn delete_volume(&self, id: &str) -> Result<()> {
        let mut volumes = self.volumes.write().await;

        let volume = volumes
            .get(id)
            .ok_or_else(|| DaemonError::VolumeNotFound { id: id.to_string() })?;

        // Don't allow deletion of mounted volumes
        if volume.is_mounted() {
            return Err(DaemonError::VolumeMounted {
                id: id.to_string(),
                services: volume.mounted_by.clone(),
            });
        }

        info!(id = %id, name = %volume.name, "Deleting volume");

        // Remove the volume directory
        if volume.path.exists() {
            std::fs::remove_dir_all(&volume.path).map_err(|e| {
                warn!(id = %id, error = %e, "Failed to remove volume directory");
                DaemonError::VolumeDeleteFailed {
                    id: id.to_string(),
                    reason: e.to_string(),
                }
            })?;
        }

        // Remove from state store
        self.remove_volume_state(id)?;

        volumes.remove(id);

        Ok(())
    }

    /// Gets a volume by ID.
    pub async fn get_volume(&self, id: &str) -> Option<VolumeDetail> {
        let volumes = self.volumes.read().await;
        volumes.get(id).map(VolumeDetail::from)
    }

    /// Gets a volume by name.
    pub async fn get_volume_by_name(&self, name: &str) -> Option<VolumeDetail> {
        let volumes = self.volumes.read().await;
        volumes
            .values()
            .find(|v| v.name == name)
            .map(VolumeDetail::from)
    }

    /// Gets a volume by ID or name.
    pub async fn get_volume_by_id_or_name(&self, id_or_name: &str) -> Option<VolumeDetail> {
        // Try ID first
        if let Some(detail) = self.get_volume(id_or_name).await {
            return Some(detail);
        }
        // Fall back to name
        self.get_volume_by_name(id_or_name).await
    }

    /// Resolves an ID or name to a volume ID.
    pub async fn resolve_volume_id(&self, id_or_name: &str) -> Option<String> {
        let volumes = self.volumes.read().await;

        // Check if it's an ID
        if volumes.contains_key(id_or_name) {
            return Some(id_or_name.to_string());
        }

        // Check if it's a name
        volumes
            .values()
            .find(|v| v.name == id_or_name)
            .map(|v| v.id.clone())
    }

    /// Lists all volumes.
    pub async fn list_volumes(&self) -> Vec<VolumeInfo> {
        let volumes = self.volumes.read().await;
        volumes.values().map(VolumeInfo::from).collect()
    }

    /// Mounts a volume for a service.
    ///
    /// Returns the host path for the volume.
    ///
    /// # Errors
    ///
    /// Returns an error if the volume does not exist.
    pub async fn mount_volume(&self, volume_id: &str, service_id: &str) -> Result<PathBuf> {
        let mut volumes = self.volumes.write().await;

        let volume = volumes
            .get_mut(volume_id)
            .ok_or_else(|| DaemonError::VolumeNotFound {
                id: volume_id.to_string(),
            })?;

        debug!(
            volume_id = %volume_id,
            service_id = %service_id,
            "Mounting volume for service"
        );

        volume.add_mount(service_id.to_string());

        // Persist updated state
        self.persist_volume(volume)?;

        Ok(volume.path.clone())
    }

    /// Unmounts a volume from a service.
    ///
    /// # Errors
    ///
    /// Returns an error if the volume does not exist.
    pub async fn unmount_volume(&self, volume_id: &str, service_id: &str) -> Result<()> {
        let mut volumes = self.volumes.write().await;

        let volume = volumes
            .get_mut(volume_id)
            .ok_or_else(|| DaemonError::VolumeNotFound {
                id: volume_id.to_string(),
            })?;

        debug!(
            volume_id = %volume_id,
            service_id = %service_id,
            "Unmounting volume from service"
        );

        volume.remove_mount(service_id);

        // Persist updated state
        self.persist_volume(volume)?;

        Ok(())
    }

    /// Unmounts all volumes from a service.
    ///
    /// Used when a service is deleted or stopped.
    pub async fn unmount_all_for_service(&self, service_id: &str) {
        let mut volumes = self.volumes.write().await;

        for volume in volumes.values_mut() {
            if volume.is_mounted_by(service_id) {
                debug!(
                    volume_id = %volume.id,
                    service_id = %service_id,
                    "Unmounting volume from deleted service"
                );
                volume.remove_mount(service_id);

                // Best effort persist
                if let Err(e) = self.persist_volume(volume) {
                    warn!(
                        volume_id = %volume.id,
                        error = %e,
                        "Failed to persist volume state after unmount"
                    );
                }
            }
        }
    }

    /// Ensures a volume exists, creating it if necessary.
    ///
    /// Used during mortar deployment to create volumes defined in the mortar file.
    ///
    /// # Returns
    ///
    /// The volume ID (existing or newly created).
    ///
    /// # Errors
    ///
    /// Returns an error if the volume cannot be created.
    pub async fn ensure_volume(&self, name: &str, size: Option<String>) -> Result<String> {
        // Check if volume already exists
        if let Some(detail) = self.get_volume_by_name(name).await {
            debug!(id = %detail.id, name = %name, "Volume already exists");
            return Ok(detail.id);
        }

        // Create new volume
        let mut config = VolumeConfig::new(name.to_string());
        config.size = size;

        self.create_volume(config).await
    }

    /// Persists a volume to the state store.
    fn persist_volume(&self, volume: &VolumeState) -> Result<()> {
        self.state_store.save_volume(volume)
    }

    /// Removes a volume from the state store.
    fn remove_volume_state(&self, id: &str) -> Result<()> {
        self.state_store.delete_volume(id)?;
        Ok(())
    }
}

/// Shared reference to a volume manager.
pub type SharedVolumeManager = Arc<VolumeManager>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn create_test_manager() -> VolumeManager {
        let dir = tempdir().expect("should create temp dir");
        let db_path = dir.path().join("test.db");
        let volumes_path = dir.path().join("volumes");
        std::fs::create_dir_all(&volumes_path).expect("should create volumes dir");

        let db = sled::open(&db_path).expect("should open db");
        let state_store = Arc::new(StateStore::new(Arc::new(db)));
        VolumeManager::new(state_store, volumes_path)
    }

    #[tokio::test]
    async fn test_create_volume() {
        let manager = create_test_manager();

        let config = VolumeConfig::new("test-volume".to_string());
        let id = manager.create_volume(config).await.unwrap();

        assert!(id.starts_with("vol-"));

        let volume = manager.get_volume(&id).await.unwrap();
        assert_eq!(volume.name, "test-volume");
        assert!(volume.path.exists());
    }

    #[tokio::test]
    async fn test_create_duplicate_volume_name() {
        let manager = create_test_manager();

        let config = VolumeConfig::new("my-volume".to_string());
        manager.create_volume(config).await.unwrap();

        let config2 = VolumeConfig::new("my-volume".to_string());
        let result = manager.create_volume(config2).await;

        assert!(matches!(result, Err(DaemonError::VolumeExists(_))));
    }

    #[tokio::test]
    async fn test_delete_volume() {
        let manager = create_test_manager();

        let config = VolumeConfig::new("deletable".to_string());
        let id = manager.create_volume(config).await.unwrap();

        let path = manager.get_volume(&id).await.unwrap().path;
        assert!(path.exists());

        manager.delete_volume(&id).await.unwrap();

        assert!(manager.get_volume(&id).await.is_none());
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_delete_mounted_volume() {
        let manager = create_test_manager();

        let config = VolumeConfig::new("mounted-vol".to_string());
        let id = manager.create_volume(config).await.unwrap();

        manager.mount_volume(&id, "svc-1").await.unwrap();

        let result = manager.delete_volume(&id).await;
        assert!(matches!(result, Err(DaemonError::VolumeMounted { .. })));
    }

    #[tokio::test]
    async fn test_mount_unmount_volume() {
        let manager = create_test_manager();

        let config = VolumeConfig::new("test-vol".to_string());
        let id = manager.create_volume(config).await.unwrap();

        // Mount
        let path = manager.mount_volume(&id, "svc-1").await.unwrap();
        assert!(path.exists());

        let detail = manager.get_volume(&id).await.unwrap();
        assert!(detail.mounted_by.contains(&"svc-1".to_string()));

        // Unmount
        manager.unmount_volume(&id, "svc-1").await.unwrap();

        let detail = manager.get_volume(&id).await.unwrap();
        assert!(!detail.mounted_by.contains(&"svc-1".to_string()));
    }

    #[tokio::test]
    async fn test_list_volumes() {
        let manager = create_test_manager();

        manager
            .create_volume(VolumeConfig::new("vol-1".to_string()))
            .await
            .unwrap();
        manager
            .create_volume(VolumeConfig::new("vol-2".to_string()))
            .await
            .unwrap();

        let volumes = manager.list_volumes().await;
        assert_eq!(volumes.len(), 2);
    }

    #[tokio::test]
    async fn test_get_volume_by_name() {
        let manager = create_test_manager();

        let config = VolumeConfig::new("named-volume".to_string());
        let id = manager.create_volume(config).await.unwrap();

        let volume = manager.get_volume_by_name("named-volume").await.unwrap();
        assert_eq!(volume.id, id);
        assert_eq!(volume.name, "named-volume");

        assert!(manager.get_volume_by_name("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_resolve_volume_id() {
        let manager = create_test_manager();

        let config = VolumeConfig::new("test-vol".to_string());
        let id = manager.create_volume(config).await.unwrap();

        // By ID
        let resolved = manager.resolve_volume_id(&id).await.unwrap();
        assert_eq!(resolved, id);

        // By name
        let resolved = manager.resolve_volume_id("test-vol").await.unwrap();
        assert_eq!(resolved, id);

        // Nonexistent
        assert!(manager.resolve_volume_id("nope").await.is_none());
    }

    #[tokio::test]
    async fn test_ensure_volume() {
        let manager = create_test_manager();

        // First call creates
        let id1 = manager.ensure_volume("auto-vol", None).await.unwrap();
        assert!(id1.starts_with("vol-"));

        // Second call returns existing
        let id2 = manager.ensure_volume("auto-vol", None).await.unwrap();
        assert_eq!(id1, id2);
    }

    #[tokio::test]
    async fn test_unmount_all_for_service() {
        let manager = create_test_manager();

        let id1 = manager
            .create_volume(VolumeConfig::new("vol-1".to_string()))
            .await
            .unwrap();
        let id2 = manager
            .create_volume(VolumeConfig::new("vol-2".to_string()))
            .await
            .unwrap();

        manager.mount_volume(&id1, "svc-1").await.unwrap();
        manager.mount_volume(&id2, "svc-1").await.unwrap();

        manager.unmount_all_for_service("svc-1").await;

        let detail1 = manager.get_volume(&id1).await.unwrap();
        let detail2 = manager.get_volume(&id2).await.unwrap();

        assert!(!detail1.mounted_by.contains(&"svc-1".to_string()));
        assert!(!detail2.mounted_by.contains(&"svc-1".to_string()));
    }
}
