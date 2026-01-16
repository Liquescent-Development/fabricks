//! Local OCI layout storage for caching and offline access.
//!
//! Implements the OCI Image Layout specification for storing pulled modules
//! locally and providing cache functionality.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::digest::{compute_digest, verify_digest};
use crate::error::{OciError, Result};

/// OCI layout version.
const OCI_LAYOUT_VERSION: &str = "1.0.0";

/// Local storage implementing OCI Image Layout specification.
///
/// Directory structure:
/// ```text
/// <base_path>/
/// ├── oci-layout              # Layout version file
/// ├── index.json              # Image index
/// └── blobs/
///     └── sha256/
///         ├── <config_hash>   # Config blob
///         ├── <wasm_hash>     # WASM layer blob
///         └── <manifest_hash> # Manifest blob
/// ```
pub struct LocalStorage {
    /// Base path for OCI layout storage.
    base_path: PathBuf,
}

/// OCI layout version file content.
#[derive(Debug, Serialize, Deserialize)]
struct OciLayout {
    #[serde(rename = "imageLayoutVersion")]
    image_layout_version: String,
}

/// OCI image index.
#[derive(Debug, Serialize, Deserialize)]
struct ImageIndex {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifests: Option<Vec<IndexManifest>>,
}

/// A manifest reference in the image index.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct IndexManifest {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
    size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotations: Option<BTreeMap<String, String>>,
}

impl LocalStorage {
    /// Create a new local storage at the given path.
    ///
    /// Creates the directory structure if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation fails.
    pub async fn new(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();
        let storage = Self { base_path };
        storage.init().await?;
        Ok(storage)
    }

    /// Open existing storage without initializing.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage doesn't exist or is invalid.
    pub fn open(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();

        // Verify oci-layout file exists
        let layout_path = base_path.join("oci-layout");
        if !layout_path.exists() {
            return Err(OciError::StorageError {
                reason: format!("not an OCI layout: {}", base_path.display()),
            });
        }

        Ok(Self { base_path })
    }

    /// Initialize the OCI layout directory structure.
    async fn init(&self) -> Result<()> {
        // Create directories
        fs::create_dir_all(self.blobs_dir())
            .await
            .map_err(|e| OciError::StorageError {
                reason: format!("failed to create blobs directory: {e}"),
            })?;

        // Write oci-layout file
        let layout = OciLayout {
            image_layout_version: OCI_LAYOUT_VERSION.to_string(),
        };
        let layout_json =
            serde_json::to_string_pretty(&layout).map_err(|e| OciError::StorageError {
                reason: format!("failed to serialize oci-layout: {e}"),
            })?;
        fs::write(self.base_path.join("oci-layout"), layout_json)
            .await
            .map_err(|e| OciError::StorageError {
                reason: format!("failed to write oci-layout: {e}"),
            })?;

        // Initialize empty index if it doesn't exist
        let index_path = self.base_path.join("index.json");
        if !index_path.exists() {
            let index = ImageIndex {
                schema_version: 2,
                manifests: Some(Vec::new()),
            };
            let index_json =
                serde_json::to_string_pretty(&index).map_err(|e| OciError::StorageError {
                    reason: format!("failed to serialize index: {e}"),
                })?;
            fs::write(&index_path, index_json)
                .await
                .map_err(|e| OciError::StorageError {
                    reason: format!("failed to write index.json: {e}"),
                })?;
        }

        Ok(())
    }

    /// Get the blobs directory path.
    fn blobs_dir(&self) -> PathBuf {
        self.base_path.join("blobs/sha256")
    }

    /// Store a blob and return its digest.
    ///
    /// The blob is content-addressed by its SHA256 digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn store_blob(&self, data: &[u8]) -> Result<String> {
        let digest = compute_digest(data);
        let hash = digest
            .strip_prefix("sha256:")
            .ok_or_else(|| OciError::StorageError {
                reason: "invalid digest format".to_string(),
            })?;

        let blob_path = self.blobs_dir().join(hash);

        // Skip if already exists (content-addressed)
        if blob_path.exists() {
            return Ok(digest);
        }

        // Write atomically using temp file
        let temp_path = self.blobs_dir().join(format!("{hash}.tmp"));
        let mut file = fs::File::create(&temp_path)
            .await
            .map_err(|e| OciError::StorageError {
                reason: format!("failed to create temp file: {e}"),
            })?;

        file.write_all(data)
            .await
            .map_err(|e| OciError::StorageError {
                reason: format!("failed to write blob: {e}"),
            })?;

        file.sync_all().await.map_err(|e| OciError::StorageError {
            reason: format!("failed to sync blob: {e}"),
        })?;

        fs::rename(&temp_path, &blob_path)
            .await
            .map_err(|e| OciError::StorageError {
                reason: format!("failed to rename blob: {e}"),
            })?;

        Ok(digest)
    }

    /// Get a blob by its digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the blob doesn't exist or verification fails.
    pub async fn get_blob(&self, digest: &str) -> Result<Vec<u8>> {
        let hash = digest
            .strip_prefix("sha256:")
            .ok_or_else(|| OciError::StorageError {
                reason: format!("invalid digest format: {digest}"),
            })?;

        let blob_path = self.blobs_dir().join(hash);

        let data = fs::read(&blob_path)
            .await
            .map_err(|e| OciError::StorageError {
                reason: format!("failed to read blob {digest}: {e}"),
            })?;

        // Verify content integrity
        verify_digest(&data, digest).map_err(|e| OciError::StorageError {
            reason: format!("blob verification failed: {e}"),
        })?;

        Ok(data)
    }

    /// Check if a blob exists.
    #[must_use]
    pub fn has_blob(&self, digest: &str) -> bool {
        let Some(hash) = digest.strip_prefix("sha256:") else {
            return false;
        };
        self.blobs_dir().join(hash).exists()
    }

    /// Delete a blob by its digest.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    pub async fn delete_blob(&self, digest: &str) -> Result<()> {
        let hash = digest
            .strip_prefix("sha256:")
            .ok_or_else(|| OciError::StorageError {
                reason: format!("invalid digest format: {digest}"),
            })?;

        let blob_path = self.blobs_dir().join(hash);
        if blob_path.exists() {
            fs::remove_file(&blob_path)
                .await
                .map_err(|e| OciError::StorageError {
                    reason: format!("failed to delete blob: {e}"),
                })?;
        }

        Ok(())
    }

    /// Add a manifest to the index with a reference tag.
    ///
    /// # Arguments
    ///
    /// * `reference` - The image reference (e.g., "mymodule:1.0.0")
    /// * `manifest_digest` - The digest of the manifest blob
    /// * `manifest_size` - Size of the manifest in bytes
    ///
    /// # Errors
    ///
    /// Returns an error if updating the index fails.
    pub async fn add_to_index(
        &self,
        reference: &str,
        manifest_digest: &str,
        manifest_size: i64,
    ) -> Result<()> {
        let index_path = self.base_path.join("index.json");
        let index_content =
            fs::read_to_string(&index_path)
                .await
                .map_err(|e| OciError::StorageError {
                    reason: format!("failed to read index: {e}"),
                })?;

        let mut index: ImageIndex =
            serde_json::from_str(&index_content).map_err(|e| OciError::StorageError {
                reason: format!("failed to parse index: {e}"),
            })?;

        let manifests = index.manifests.get_or_insert_with(Vec::new);

        // Remove existing entry for this reference
        manifests.retain(|m| {
            m.annotations
                .as_ref()
                .and_then(|a| a.get("org.opencontainers.image.ref.name"))
                != Some(&reference.to_string())
        });

        // Add new entry
        let mut annotations = BTreeMap::new();
        annotations.insert(
            "org.opencontainers.image.ref.name".to_string(),
            reference.to_string(),
        );

        manifests.push(IndexManifest {
            media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
            digest: manifest_digest.to_string(),
            size: manifest_size,
            annotations: Some(annotations),
        });

        // Write updated index
        let index_json =
            serde_json::to_string_pretty(&index).map_err(|e| OciError::StorageError {
                reason: format!("failed to serialize index: {e}"),
            })?;

        fs::write(&index_path, index_json)
            .await
            .map_err(|e| OciError::StorageError {
                reason: format!("failed to write index: {e}"),
            })?;

        Ok(())
    }

    /// Look up a manifest digest by reference.
    ///
    /// # Errors
    ///
    /// Returns an error if the reference is not found.
    pub async fn get_manifest_digest(&self, reference: &str) -> Result<String> {
        let index_path = self.base_path.join("index.json");
        let index_content =
            fs::read_to_string(&index_path)
                .await
                .map_err(|e| OciError::StorageError {
                    reason: format!("failed to read index: {e}"),
                })?;

        let index: ImageIndex =
            serde_json::from_str(&index_content).map_err(|e| OciError::StorageError {
                reason: format!("failed to parse index: {e}"),
            })?;

        let manifests = index
            .manifests
            .as_ref()
            .ok_or_else(|| OciError::StorageError {
                reason: "index has no manifests".to_string(),
            })?;

        for manifest in manifests {
            if let Some(annotations) = &manifest.annotations
                && annotations.get("org.opencontainers.image.ref.name")
                    == Some(&reference.to_string())
            {
                return Ok(manifest.digest.clone());
            }
        }

        Err(OciError::StorageError {
            reason: format!("reference not found: {reference}"),
        })
    }

    /// List all references in the index.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the index fails.
    pub async fn list_references(&self) -> Result<Vec<String>> {
        let index_path = self.base_path.join("index.json");
        let index_content =
            fs::read_to_string(&index_path)
                .await
                .map_err(|e| OciError::StorageError {
                    reason: format!("failed to read index: {e}"),
                })?;

        let index: ImageIndex =
            serde_json::from_str(&index_content).map_err(|e| OciError::StorageError {
                reason: format!("failed to parse index: {e}"),
            })?;

        let mut refs = Vec::new();
        if let Some(manifests) = index.manifests {
            for manifest in manifests {
                if let Some(annotations) = manifest.annotations
                    && let Some(name) = annotations.get("org.opencontainers.image.ref.name")
                {
                    refs.push(name.clone());
                }
            }
        }

        Ok(refs)
    }

    /// Get the base path of the storage.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.base_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_init() {
        let temp = TempDir::new().expect("Failed to create temp directory");
        let storage = LocalStorage::new(temp.path())
            .await
            .expect("Failed to create storage");

        assert!(temp.path().join("oci-layout").exists());
        assert!(temp.path().join("index.json").exists());
        assert!(temp.path().join("blobs/sha256").exists());
        assert_eq!(storage.path(), temp.path());
    }

    #[tokio::test]
    async fn test_store_and_get_blob() {
        let temp = TempDir::new().expect("Failed to create temp directory");
        let storage = LocalStorage::new(temp.path())
            .await
            .expect("Failed to create storage");

        let data = b"hello world";
        let digest = storage
            .store_blob(data)
            .await
            .expect("Failed to store blob");

        assert!(digest.starts_with("sha256:"));
        assert!(storage.has_blob(&digest));

        let retrieved = storage.get_blob(&digest).await.expect("Failed to get blob");
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_blob_deduplication() {
        let temp = TempDir::new().expect("Failed to create temp directory");
        let storage = LocalStorage::new(temp.path())
            .await
            .expect("Failed to create storage");

        let data = b"duplicate content";
        let digest1 = storage
            .store_blob(data)
            .await
            .expect("Failed to store blob");
        let digest2 = storage
            .store_blob(data)
            .await
            .expect("Failed to store blob");

        assert_eq!(digest1, digest2);
    }

    #[tokio::test]
    async fn test_delete_blob() {
        let temp = TempDir::new().expect("Failed to create temp directory");
        let storage = LocalStorage::new(temp.path())
            .await
            .expect("Failed to create storage");

        let data = b"to be deleted";
        let digest = storage
            .store_blob(data)
            .await
            .expect("Failed to store blob");

        assert!(storage.has_blob(&digest));

        storage
            .delete_blob(&digest)
            .await
            .expect("Failed to delete blob");

        assert!(!storage.has_blob(&digest));
    }

    #[tokio::test]
    async fn test_index_operations() {
        let temp = TempDir::new().expect("Failed to create temp directory");
        let storage = LocalStorage::new(temp.path())
            .await
            .expect("Failed to create storage");

        let digest = "sha256:abc123def456abc123def456abc123def456abc123def456abc123def456abc1";

        storage
            .add_to_index("mymodule:1.0.0", digest, 1234)
            .await
            .expect("Failed to add to index");

        let refs = storage
            .list_references()
            .await
            .expect("Failed to list references");
        assert!(refs.contains(&"mymodule:1.0.0".to_string()));

        let found_digest = storage
            .get_manifest_digest("mymodule:1.0.0")
            .await
            .expect("Failed to get digest");
        assert_eq!(found_digest, digest);
    }

    #[tokio::test]
    async fn test_index_update_same_reference() {
        let temp = TempDir::new().expect("Failed to create temp directory");
        let storage = LocalStorage::new(temp.path())
            .await
            .expect("Failed to create storage");

        let digest1 = "sha256:111111111111111111111111111111111111111111111111111111111111111a";
        let digest2 = "sha256:222222222222222222222222222222222222222222222222222222222222222b";

        storage
            .add_to_index("mymodule:latest", digest1, 100)
            .await
            .expect("Failed to add first");
        storage
            .add_to_index("mymodule:latest", digest2, 200)
            .await
            .expect("Failed to add second");

        let refs = storage.list_references().await.expect("Failed to list");
        assert_eq!(refs.len(), 1);

        let found = storage
            .get_manifest_digest("mymodule:latest")
            .await
            .expect("Failed to get digest");
        assert_eq!(found, digest2);
    }
}
