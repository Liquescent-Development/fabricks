//! Fabricks module representation for OCI artifacts.
//!
//! A `FabricksModule` bundles a WASM binary with its Fabrickfile configuration
//! and optional static files for distribution via OCI registries.
//!
//! ## Multi-Layer Support
//!
//! `FabricksModule` supports multiple WASM layers for base image composition:
//! - **Runtime layer**: Base runtime (Python, JavaScript, etc.) - optional
//! - **Module layer**: User's WASM code - always required
//!
//! When a base image is specified via `[from].image` in the Fabrickfile,
//! the runtime layer is composed with the user's code at build time.

use std::collections::HashMap;

use fabricks_common::Fabrickfile;

use crate::digest::compute_digest;
use crate::media_types;

/// A single layer in a Fabricks module.
///
/// Layers are ordered in the OCI manifest with runtime layers first,
/// followed by the user's module layer.
#[derive(Debug, Clone)]
pub struct ModuleLayer {
    /// The OCI media type for this layer.
    pub media_type: String,

    /// The raw bytes of this layer.
    pub data: Vec<u8>,

    /// Optional annotations specific to this layer.
    pub annotations: Option<HashMap<String, String>>,
}

impl ModuleLayer {
    /// Create a new module layer.
    #[must_use]
    pub fn new(media_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            media_type: media_type.into(),
            data,
            annotations: None,
        }
    }

    /// Add annotations to this layer.
    #[must_use]
    pub fn with_annotations(mut self, annotations: HashMap<String, String>) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Compute the digest of this layer's data.
    #[must_use]
    pub fn digest(&self) -> String {
        compute_digest(&self.data)
    }

    /// Get the size of this layer in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Check if this is a runtime layer.
    #[must_use]
    pub fn is_runtime(&self) -> bool {
        self.media_type == media_types::RUNTIME_LAYER_MEDIA_TYPE
    }

    /// Check if this is a module (user code) layer.
    #[must_use]
    pub fn is_module(&self) -> bool {
        self.media_type == media_types::WASM_LAYER_MEDIA_TYPE
    }
}

/// A Fabricks module ready for OCI distribution.
///
/// Contains WASM layers, configuration, and metadata needed to push
/// to an OCI registry or run locally.
///
/// ## Layer Structure
///
/// A module can contain one or two layers:
/// - Single layer: Just the user's WASM module (backward compatible)
/// - Two layers: Runtime (base image) + user's module
///
/// The layers are stored in order: runtime first (if present), then module.
#[derive(Debug, Clone)]
pub struct FabricksModule {
    /// The Fabrickfile configuration.
    config: Fabrickfile,

    /// WASM layers in order (runtime first if present, then module).
    layers: Vec<ModuleLayer>,

    /// Optional static files to include (path -> contents).
    files: Option<HashMap<String, Vec<u8>>>,

    /// Additional annotations for the manifest.
    annotations: HashMap<String, String>,
}

impl FabricksModule {
    /// Create a new Fabricks module with a single WASM layer.
    ///
    /// This is the standard constructor for modules without a base image.
    /// The WASM bytes are stored as a module layer with the standard media type.
    ///
    /// # Arguments
    ///
    /// * `config` - The Fabrickfile configuration
    /// * `wasm_bytes` - The compiled WASM binary
    #[must_use]
    pub fn new(config: Fabrickfile, wasm_bytes: Vec<u8>) -> Self {
        let module_layer = ModuleLayer::new(media_types::WASM_LAYER_MEDIA_TYPE, wasm_bytes);

        Self {
            config,
            layers: vec![module_layer],
            files: None,
            annotations: HashMap::new(),
        }
    }

    /// Create a new Fabricks module from explicit layers.
    ///
    /// Use this when you have pre-built layers (e.g., from a pulled image).
    /// Layers should be in order: runtime first (if present), then module.
    ///
    /// # Arguments
    ///
    /// * `config` - The Fabrickfile configuration
    /// * `layers` - The WASM layers in order
    #[must_use]
    pub fn from_layers(config: Fabrickfile, layers: Vec<ModuleLayer>) -> Self {
        Self {
            config,
            layers,
            files: None,
            annotations: HashMap::new(),
        }
    }

    /// Add a runtime layer (base image) to this module.
    ///
    /// The runtime layer is prepended to the layers list, maintaining
    /// the convention that runtime layers come before module layers.
    ///
    /// # Arguments
    ///
    /// * `runtime_wasm` - The runtime WASM bytes
    #[must_use]
    pub fn with_runtime_layer(mut self, runtime_wasm: Vec<u8>) -> Self {
        let runtime_layer =
            ModuleLayer::new(media_types::RUNTIME_LAYER_MEDIA_TYPE, runtime_wasm);

        // Insert runtime at the beginning
        self.layers.insert(0, runtime_layer);
        self
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

    /// Get all layers in this module.
    #[must_use]
    pub fn layers(&self) -> &[ModuleLayer] {
        &self.layers
    }

    /// Get the user's WASM module bytes (backward compatible).
    ///
    /// This returns the data from the module layer (not the runtime layer).
    /// For single-layer modules, this is the only layer.
    /// For multi-layer modules, this is the last layer.
    #[must_use]
    pub fn wasm_bytes(&self) -> &[u8] {
        const EMPTY: &[u8] = &[];

        // Find the module layer, or fall back to the last layer
        self.layers
            .iter()
            .find(|l| l.is_module())
            .map_or_else(
                || {
                    // Fallback for backward compat: use last layer
                    self.layers.last().map_or(EMPTY, |l| l.data.as_slice())
                },
                |l| l.data.as_slice(),
            )
    }

    /// Get the runtime layer, if present.
    ///
    /// Returns `None` for single-layer modules (no base image).
    #[must_use]
    pub fn runtime_layer(&self) -> Option<&ModuleLayer> {
        self.layers.iter().find(|l| l.is_runtime())
    }

    /// Get the module layer (user code).
    ///
    /// Returns the layer containing the user's WASM code.
    #[must_use]
    pub fn module_layer(&self) -> Option<&ModuleLayer> {
        self.layers.iter().find(|l| l.is_module())
    }

    /// Check if this module has a runtime (base image) layer.
    #[must_use]
    pub fn has_runtime_layer(&self) -> bool {
        self.layers.iter().any(ModuleLayer::is_runtime)
    }

    /// Get the number of layers in this module.
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Get the static files, if any.
    #[must_use]
    pub const fn files(&self) -> Option<&HashMap<String, Vec<u8>>> {
        self.files.as_ref()
    }

    /// Compute the digest of the user's WASM binary (backward compatible).
    #[must_use]
    pub fn wasm_digest(&self) -> String {
        compute_digest(self.wasm_bytes())
    }

    /// Get the size of the user's WASM binary in bytes (backward compatible).
    #[must_use]
    pub fn wasm_size(&self) -> usize {
        self.wasm_bytes().len()
    }

    /// Get the total size of all layers in bytes.
    #[must_use]
    pub fn total_size(&self) -> usize {
        self.layers.iter().map(ModuleLayer::size).sum()
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
            annotations.insert(
                media_types::ANNOTATION_DESCRIPTION.to_string(),
                desc.clone(),
            );
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
    use fabricks_common::Capabilities;
    use fabricks_common::models::fabrickfile::Info;

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
        assert_eq!(module.layer_count(), 1);
        assert!(!module.has_runtime_layer());
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

        assert_eq!(
            annotations.get(media_types::ANNOTATION_NAME),
            Some(&"test-module".to_string())
        );
        assert_eq!(
            annotations.get(media_types::ANNOTATION_VERSION),
            Some(&"1.0.0".to_string())
        );
        assert_eq!(
            annotations.get(media_types::ANNOTATION_DESCRIPTION),
            Some(&"A test module".to_string())
        );
    }

    #[test]
    fn test_module_layer_creation() {
        let data = vec![0x00, 0x61, 0x73, 0x6d];
        let layer = ModuleLayer::new(media_types::WASM_LAYER_MEDIA_TYPE, data.clone());

        assert_eq!(layer.media_type, media_types::WASM_LAYER_MEDIA_TYPE);
        assert_eq!(layer.data, data);
        assert!(layer.annotations.is_none());
        assert!(layer.is_module());
        assert!(!layer.is_runtime());
        assert_eq!(layer.size(), 4);
    }

    #[test]
    fn test_runtime_layer() {
        let runtime_data = b"runtime wasm".to_vec();
        let layer = ModuleLayer::new(media_types::RUNTIME_LAYER_MEDIA_TYPE, runtime_data);

        assert!(layer.is_runtime());
        assert!(!layer.is_module());
    }

    #[test]
    fn test_layer_with_annotations() {
        let data = vec![0x00];
        let mut annotations = HashMap::new();
        annotations.insert("key".to_string(), "value".to_string());

        let layer = ModuleLayer::new(media_types::WASM_LAYER_MEDIA_TYPE, data)
            .with_annotations(annotations.clone());

        assert_eq!(layer.annotations, Some(annotations));
    }

    #[test]
    fn test_module_with_runtime_layer() {
        let user_wasm = b"user code".to_vec();
        let runtime_wasm = b"runtime code".to_vec();

        let module = FabricksModule::new(test_config(), user_wasm.clone())
            .with_runtime_layer(runtime_wasm.clone());

        // Should have 2 layers
        assert_eq!(module.layer_count(), 2);
        assert!(module.has_runtime_layer());

        // Runtime layer should be first
        let layers = module.layers();
        assert!(layers[0].is_runtime());
        assert_eq!(layers[0].data, runtime_wasm);

        // Module layer should be second
        assert!(layers[1].is_module());
        assert_eq!(layers[1].data, user_wasm);

        // wasm_bytes() should return user code (backward compat)
        assert_eq!(module.wasm_bytes(), user_wasm.as_slice());

        // Accessors should work
        assert_eq!(module.runtime_layer().map(|l| &l.data), Some(&runtime_wasm));
        assert_eq!(module.module_layer().map(|l| &l.data), Some(&user_wasm));
    }

    #[test]
    fn test_module_from_layers() {
        let runtime_layer =
            ModuleLayer::new(media_types::RUNTIME_LAYER_MEDIA_TYPE, b"runtime".to_vec());
        let module_layer =
            ModuleLayer::new(media_types::WASM_LAYER_MEDIA_TYPE, b"module".to_vec());

        let layers = vec![runtime_layer, module_layer];
        let module = FabricksModule::from_layers(test_config(), layers);

        assert_eq!(module.layer_count(), 2);
        assert!(module.has_runtime_layer());
        assert_eq!(module.wasm_bytes(), b"module");
    }

    #[test]
    fn test_total_size() {
        let user_wasm = b"user code here".to_vec();
        let runtime_wasm = b"runtime".to_vec();

        let module = FabricksModule::new(test_config(), user_wasm.clone())
            .with_runtime_layer(runtime_wasm.clone());

        assert_eq!(module.total_size(), user_wasm.len() + runtime_wasm.len());
    }

    #[test]
    fn test_layer_digest() {
        let data = b"test content".to_vec();
        let layer = ModuleLayer::new(media_types::WASM_LAYER_MEDIA_TYPE, data);

        let digest = layer.digest();
        assert!(digest.starts_with("sha256:"));
    }

    #[test]
    fn test_single_layer_backward_compat() {
        // Ensure single-layer modules work exactly as before
        let wasm = b"my wasm code".to_vec();
        let module = FabricksModule::new(test_config(), wasm.clone());

        // All the old methods should work
        assert_eq!(module.wasm_bytes(), wasm.as_slice());
        assert_eq!(module.wasm_size(), wasm.len());
        assert!(module.wasm_digest().starts_with("sha256:"));
        assert!(module.runtime_layer().is_none());
        assert!(module.module_layer().is_some());
    }
}
