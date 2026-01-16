//! OCI registry client for Fabricks modules.
//!
//! This module provides a high-level API for pushing and pulling Fabricks WASM
//! modules to/from OCI-compliant registries.

use std::collections::BTreeMap;

use oci_client::client::{ClientConfig as OciClientConfig, ImageLayer};
use oci_client::errors::OciErrorCode;
use oci_client::manifest::{OCI_IMAGE_MEDIA_TYPE, OciDescriptor, OciImageManifest};
use oci_client::secrets::RegistryAuth;
use oci_client::{Client as OciClient, Reference, RegistryOperation};
use tracing::{debug, info};

use crate::digest::compute_digest;
use crate::error::{OciError, Result};
use crate::media_types;
use crate::module::{FabricksModule, PulledModule};

/// Configuration for the Fabricks OCI client.
#[derive(Debug, Clone, Default)]
pub struct ClientConfig {
    /// Whether to accept invalid TLS certificates (for testing).
    pub accept_invalid_certs: bool,
}

/// Client for pushing and pulling Fabricks modules to/from OCI registries.
pub struct FabricksClient {
    /// The underlying OCI client.
    client: OciClient,
}

impl FabricksClient {
    /// Create a new Fabricks OCI client with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: OciClient::default(),
        }
    }

    /// Create a new client with custom configuration.
    #[must_use]
    pub fn with_config(config: &ClientConfig) -> Self {
        let oci_config = OciClientConfig {
            accept_invalid_certificates: config.accept_invalid_certs,
            ..Default::default()
        };

        Self {
            client: OciClient::new(oci_config),
        }
    }

    /// Push a Fabricks module to a registry.
    ///
    /// # Arguments
    ///
    /// * `reference` - The image reference (e.g., "ghcr.io/user/my-module:1.0.0")
    /// * `module` - The Fabricks module to push
    /// * `auth` - Registry authentication credentials
    ///
    /// # Returns
    ///
    /// The manifest digest of the pushed module.
    ///
    /// # Errors
    ///
    /// Returns an error if the push fails.
    pub async fn push(
        &self,
        reference: &Reference,
        module: &FabricksModule,
        auth: &RegistryAuth,
    ) -> Result<String> {
        info!(
            "Pushing module {}:{} to {}",
            module.name(),
            module.version(),
            reference
        );

        // Authenticate with the registry
        self.client
            .auth(reference, auth, RegistryOperation::Push)
            .await?;

        // Serialize config to TOML
        let config_bytes = module
            .config_bytes()
            .map_err(|e| OciError::ConfigParseError {
                reason: e.to_string(),
            })?;

        // Build layers
        let layers = vec![ImageLayer {
            data: module.wasm_bytes().to_vec(),
            media_type: media_types::WASM_LAYER_MEDIA_TYPE.to_string(),
            annotations: None,
        }];

        // Build annotations
        let annotations = module.build_annotations();
        let annotations_btree: BTreeMap<String, String> = annotations.into_iter().collect();

        // Build manifest
        let manifest = build_manifest(&config_bytes, &layers, annotations_btree);

        // Push config blob
        let config_digest = compute_digest(&config_bytes);
        debug!("Pushing config blob: {config_digest}");
        self.client
            .push_blob(reference, &config_bytes, &config_digest)
            .await?;

        // Push layers
        for layer in &layers {
            let digest = compute_digest(&layer.data);
            debug!("Pushing layer: {digest}");
            self.client
                .push_blob(reference, &layer.data, &digest)
                .await?;
        }

        // Push manifest
        debug!("Pushing manifest");
        let oci_manifest = oci_client::manifest::OciManifest::Image(manifest);
        let manifest_url = self.client.push_manifest(reference, &oci_manifest).await?;

        info!("Successfully pushed to {manifest_url}");
        Ok(manifest_url)
    }

    /// Pull a Fabricks module from a registry.
    ///
    /// # Arguments
    ///
    /// * `reference` - The image reference (e.g., "ghcr.io/user/my-module:1.0.0")
    /// * `auth` - Registry authentication credentials
    ///
    /// # Returns
    ///
    /// The pulled module with its manifest digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the pull fails or the module is invalid.
    pub async fn pull(&self, reference: &Reference, auth: &RegistryAuth) -> Result<PulledModule> {
        info!("Pulling module from {reference}");

        // Authenticate with the registry
        self.client
            .auth(reference, auth, RegistryOperation::Pull)
            .await?;

        // Pull manifest
        let (manifest, digest) = self.client.pull_manifest(reference, auth).await?;

        let image_manifest = match manifest {
            oci_client::manifest::OciManifest::Image(m) => m,
            oci_client::manifest::OciManifest::ImageIndex(_) => {
                return Err(OciError::UnsupportedMediaType {
                    media_type: "manifest list".to_string(),
                });
            }
        };

        // Find WASM layer
        let wasm_layer = image_manifest
            .layers
            .iter()
            .find(|l| media_types::is_fabricks_module(&l.media_type))
            .ok_or_else(|| OciError::InvalidModule {
                reason: "no WASM layer found".to_string(),
            })?;

        // Pull WASM blob
        debug!("Pulling WASM layer: {}", wasm_layer.digest);
        let mut wasm_bytes = Vec::new();
        self.client
            .pull_blob(reference, wasm_layer, &mut wasm_bytes)
            .await?;

        // Pull config blob
        debug!("Pulling config: {}", image_manifest.config.digest);
        let mut config_bytes = Vec::new();
        self.client
            .pull_blob(reference, &image_manifest.config, &mut config_bytes)
            .await?;

        // Parse config
        let config = parse_config(&config_bytes, &image_manifest)?;

        let module = FabricksModule::new(config, wasm_bytes);

        Ok(PulledModule { module, digest })
    }

    /// Check if a module exists in the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the check fails (other than 404).
    pub async fn exists(&self, reference: &Reference, auth: &RegistryAuth) -> Result<bool> {
        // Authenticate first
        self.client
            .auth(reference, auth, RegistryOperation::Pull)
            .await?;

        match self.client.fetch_manifest_digest(reference, auth).await {
            Ok(_) => Ok(true),
            Err(oci_client::errors::OciDistributionError::RegistryError { envelope, .. })
                if envelope.errors.iter().any(|e| {
                    e.code == OciErrorCode::ManifestUnknown || e.code == OciErrorCode::NameUnknown
                }) =>
            {
                Ok(false)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// List tags for an image.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn list_tags(
        &self,
        reference: &Reference,
        auth: &RegistryAuth,
    ) -> Result<Vec<String>> {
        // Authenticate first
        self.client
            .auth(reference, auth, RegistryOperation::Pull)
            .await?;

        let response = self.client.list_tags(reference, auth, None, None).await?;
        Ok(response.tags)
    }
}

impl Default for FabricksClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Build an OCI image manifest for a Fabricks module.
fn build_manifest(
    config_bytes: &[u8],
    layers: &[ImageLayer],
    annotations: BTreeMap<String, String>,
) -> OciImageManifest {
    let config_descriptor = OciDescriptor {
        media_type: media_types::CONFIG_MEDIA_TYPE.to_string(),
        digest: compute_digest(config_bytes),
        size: i64::try_from(config_bytes.len()).unwrap_or(i64::MAX),
        urls: None,
        annotations: None,
    };

    let layer_descriptors: Vec<OciDescriptor> = layers
        .iter()
        .map(|l| OciDescriptor {
            media_type: l.media_type.clone(),
            digest: compute_digest(&l.data),
            size: i64::try_from(l.data.len()).unwrap_or(i64::MAX),
            urls: None,
            annotations: l.annotations.clone(),
        })
        .collect();

    OciImageManifest {
        schema_version: 2,
        media_type: Some(OCI_IMAGE_MEDIA_TYPE.to_string()),
        artifact_type: Some(media_types::ARTIFACT_TYPE.to_string()),
        config: config_descriptor,
        layers: layer_descriptors,
        subject: None,
        annotations: Some(annotations),
    }
}

/// Parse config from pulled bytes, falling back to annotations if needed.
fn parse_config(
    config_bytes: &[u8],
    manifest: &OciImageManifest,
) -> Result<fabricks_common::Fabrickfile> {
    // First try to parse as TOML
    if let Ok(config_str) = std::str::from_utf8(config_bytes)
        && let Ok(config) = toml::from_str(config_str)
    {
        return Ok(config);
    }

    // Fall back to building minimal config from annotations
    let annotations = manifest
        .annotations
        .as_ref()
        .ok_or_else(|| OciError::InvalidModule {
            reason: "no config or annotations found".to_string(),
        })?;

    let name = annotations
        .get(media_types::ANNOTATION_NAME)
        .ok_or_else(|| OciError::InvalidModule {
            reason: "missing name annotation".to_string(),
        })?
        .clone();

    let version = annotations
        .get(media_types::ANNOTATION_VERSION)
        .ok_or_else(|| OciError::InvalidModule {
            reason: "missing version annotation".to_string(),
        })?
        .clone();

    Ok(fabricks_common::Fabrickfile {
        fabrick_version: annotations
            .get(media_types::ANNOTATION_FABRICK_VERSION)
            .cloned()
            .unwrap_or_else(|| "1.0".to_string()),
        info: fabricks_common::models::fabrickfile::Info {
            name,
            version,
            service_type: fabricks_common::models::fabrickfile::ServiceType::default(),
            description: annotations
                .get(media_types::ANNOTATION_DESCRIPTION)
                .cloned(),
            authors: annotations
                .get(media_types::ANNOTATION_AUTHORS)
                .map(|a| a.split(", ").map(String::from).collect()),
            license: annotations.get(media_types::ANNOTATION_LICENSE).cloned(),
            homepage: None,
            repository: annotations.get(media_types::ANNOTATION_SOURCE).cloned(),
            documentation: None,
            keywords: None,
        },
        from: None,
        source: None,
        runtime: None,
        build: None,
        exports: None,
        imports: None,
        capabilities: fabricks_common::Capabilities::default(),
        files: None,
        config: None,
        health_check: None,
        security: None,
        labels: None,
        validate: None,
    })
}
