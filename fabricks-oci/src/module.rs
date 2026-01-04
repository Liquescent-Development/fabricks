//! Fabricks module representation for OCI artifacts.
//!
//! A `FabricksModule` bundles a WASM binary with its Fabrickfile configuration
//! and optional static files for distribution via OCI registries.

use std::collections::HashMap;

use fabricks_common::Fabrickfile;

use crate::digest::compute_digest;
use crate::media_types;

/// A Fabricks module ready for OCI distribution.
///
/// Contains the WASM binary, configuration, and metadata needed to push
/// to an OCI registry or run locally.
#[derive(Debug, Clone)]
pub struct FabricksModule {
    /// The Fabrickfile configuration.
    config: Fabrickfile,

    /// The compiled WASM binary.
    wasm_bytes: Vec<u8>,

    /// Optional static files to include (path -> contents).
    files: Option<HashMap<String, Vec<u8>>>,

    /// Additional annotations for the manifest.
    annotations: HashMap<String, String>,
}

impl FabricksModule {
    /// Create a new Fabricks module.
    ///
    /// # Arguments
    ///
    /// * `config` - The Fabrickfile configuration
    /// * `wasm_bytes` - The compiled WASM binary
    #[must_use]
    pub fn new(config: Fabrickfile, wasm_bytes: Vec<u8>) -> Self {
        Self {
            config,
            wasm_bytes,
            files: None,
            annotations: HashMap::new(),
        }
    }

    /// Add static files to include with the module.
    #[must_use]
    pub fn with_files(mut self, files: HashMap<String, Vec<u8>>) -> Self {
        self.files = Some(files);
        self
    }

    /// Add custom annotations to the manifest.
    #[must_use]
    pub fn with_annotation(mut self, key: String, value: String) -> Self {
        self.annotations.insert(key, value);
        self
    }

    /// Get the module name from the config.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.config.info.name
    }

    /// Get the module version from the config.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.config.info.version
    }

    /// Get the Fabrickfile configuration.
    #[must_use]
    pub const fn config(&self) -> &Fabrickfile {
        &self.config
    }

    /// Get the WASM bytes.
    #[must_use]
    pub fn wasm_bytes(&self) -> &[u8] {
        &self.wasm_bytes
    }

    /// Get the static files, if any.
    #[must_use]
    pub const fn files(&self) -> Option<&HashMap<String, Vec<u8>>> {
        self.files.as_ref()
    }

    /// Compute the digest of the WASM binary.
    #[must_use]
    pub fn wasm_digest(&self) -> String {
        compute_digest(&self.wasm_bytes)
    }

    /// Get the size of the WASM binary in bytes.
    #[must_use]
    pub fn wasm_size(&self) -> usize {
        self.wasm_bytes.len()
    }

    /// Serialize the config to TOML bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if TOML serialization fails.
    pub fn config_bytes(&self) -> Result<Vec<u8>, toml::ser::Error> {
        let toml_str = toml::to_string_pretty(&self.config)?;
        Ok(toml_str.into_bytes())
    }

    /// Build the annotations map for the OCI manifest.
    #[must_use]
    pub fn build_annotations(&self) -> HashMap<String, String> {
        let mut annotations = self.annotations.clone();

        // Add standard Fabricks annotations
        annotations.insert(
            media_types::ANNOTATION_FABRICK_VERSION.to_string(),
            self.config.fabrick_version.clone(),
        );
        annotations.insert(
            media_types::ANNOTATION_NAME.to_string(),
            self.config.info.name.clone(),
        );
        annotations.insert(
            media_types::ANNOTATION_VERSION.to_string(),
            self.config.info.version.clone(),
        );

        // Add optional metadata
        if let Some(ref desc) = self.config.info.description {
            annotations.insert(media_types::ANNOTATION_DESCRIPTION.to_string(), desc.clone());
        }

        if let Some(ref authors) = self.config.info.authors {
            annotations.insert(
                media_types::ANNOTATION_AUTHORS.to_string(),
                authors.join(", "),
            );
        }

        if let Some(ref license) = self.config.info.license {
            annotations.insert(media_types::ANNOTATION_LICENSE.to_string(), license.clone());
        }

        if let Some(ref repo) = self.config.info.repository {
            annotations.insert(media_types::ANNOTATION_SOURCE.to_string(), repo.clone());
        }

        annotations
    }
}

/// A pulled Fabricks module with its manifest digest.
#[derive(Debug, Clone)]
pub struct PulledModule {
    /// The module contents.
    pub module: FabricksModule,

    /// The manifest digest from the registry.
    pub digest: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabricks_common::models::fabrickfile::Info;
    use fabricks_common::Capabilities;

    fn test_config() -> Fabrickfile {
        Fabrickfile {
            fabrick_version: "1.0".to_string(),
            info: Info {
                name: "test-module".to_string(),
                version: "1.0.0".to_string(),
                service_type: fabricks_common::models::fabrickfile::ServiceType::default(),
                description: Some("A test module".to_string()),
                authors: Some(vec!["Test Author".to_string()]),
                license: Some("MIT".to_string()),
                homepage: None,
                repository: Some("https://github.com/example/test".to_string()),
                documentation: None,
                keywords: None,
            },
            from: None,
            source: None,
            runtime: None,
            build: None,
            exports: None,
            imports: None,
            capabilities: Capabilities::default(),
            files: None,
            config: None,
            health_check: None,
            security: None,
            labels: None,
            validate: None,
        }
    }

    #[test]
    fn test_module_creation() {
        let wasm = vec![0x00, 0x61, 0x73, 0x6d]; // WASM magic bytes
        let module = FabricksModule::new(test_config(), wasm);

        assert_eq!(module.name(), "test-module");
        assert_eq!(module.version(), "1.0.0");
        assert_eq!(module.wasm_size(), 4);
    }

    #[test]
    fn test_module_digest() {
        let wasm = b"test wasm content".to_vec();
        let module = FabricksModule::new(test_config(), wasm);

        let digest = module.wasm_digest();
        assert!(digest.starts_with("sha256:"));
    }

    #[test]
    fn test_build_annotations() {
        let module = FabricksModule::new(test_config(), vec![]);
        let annotations = module.build_annotations();

        assert_eq!(annotations.get(media_types::ANNOTATION_NAME), Some(&"test-module".to_string()));
        assert_eq!(
            annotations.get(media_types::ANNOTATION_VERSION),
            Some(&"1.0.0".to_string())
        );
        assert_eq!(
            annotations.get(media_types::ANNOTATION_DESCRIPTION),
            Some(&"A test module".to_string())
        );
    }
}
