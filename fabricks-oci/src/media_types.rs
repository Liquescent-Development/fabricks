//! Fabricks-specific OCI media types.
//!
//! These media types follow the OCI artifact specification for custom content types.
//! They allow OCI registries to distinguish Fabricks WASM modules from container images.

/// Media type for the Fabricks module artifact.
pub const ARTIFACT_TYPE: &str = "application/vnd.fabricks.module.v1";

/// Media type for the Fabricks configuration (Fabrickfile as TOML).
pub const CONFIG_MEDIA_TYPE: &str = "application/vnd.fabricks.config.v1+toml";

/// Media type for the WASM module layer.
pub const WASM_LAYER_MEDIA_TYPE: &str = "application/vnd.fabricks.module.v1+wasm";

/// Media type for the runtime/base image layer.
/// Used for language runtimes (Python, JavaScript, etc.) that user code builds upon.
pub const RUNTIME_LAYER_MEDIA_TYPE: &str = "application/vnd.fabricks.runtime.v1+wasm";

/// Media type for static files layer (gzipped tar).
pub const FILES_LAYER_MEDIA_TYPE: &str = "application/vnd.fabricks.files.v1.tar+gzip";

/// Media type for source code layer (gzipped tar).
/// Used for interpreted language source (Python, JavaScript, etc.) that runs on a runtime.
/// These layers stack - later layers can override files from earlier layers.
pub const SOURCE_LAYER_MEDIA_TYPE: &str = "application/vnd.fabricks.source.v1.tar+gzip";

/// OCI manifest media type.
pub const MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";

/// OCI image index (manifest list) media type.
pub const INDEX_MEDIA_TYPE: &str = "application/vnd.oci.image.index.v1+json";

/// Empty config media type (used when config is embedded in annotations).
pub const EMPTY_CONFIG_MEDIA_TYPE: &str = "application/vnd.oci.empty.v1+json";

/// Annotation key for the Fabricks version.
pub const ANNOTATION_FABRICK_VERSION: &str = "dev.fabricks.version";

/// Annotation key for the module name.
pub const ANNOTATION_NAME: &str = "dev.fabricks.name";

/// Annotation key for the module version.
pub const ANNOTATION_VERSION: &str = "dev.fabricks.module.version";

/// Annotation key for the module description.
pub const ANNOTATION_DESCRIPTION: &str = "org.opencontainers.image.description";

/// Annotation key for authors.
pub const ANNOTATION_AUTHORS: &str = "org.opencontainers.image.authors";

/// Annotation key for license.
pub const ANNOTATION_LICENSE: &str = "org.opencontainers.image.licenses";

/// Annotation key for source repository.
pub const ANNOTATION_SOURCE: &str = "org.opencontainers.image.source";

/// Annotation key for creation timestamp.
pub const ANNOTATION_CREATED: &str = "org.opencontainers.image.created";

/// Check if a media type is a Fabricks WASM module.
#[must_use]
pub fn is_fabricks_module(media_type: &str) -> bool {
    media_type == WASM_LAYER_MEDIA_TYPE
}

/// Check if a media type is a Fabricks config.
#[must_use]
pub fn is_fabricks_config(media_type: &str) -> bool {
    media_type == CONFIG_MEDIA_TYPE
}

/// Check if a media type is a source code layer.
#[must_use]
pub fn is_source_layer(media_type: &str) -> bool {
    media_type == SOURCE_LAYER_MEDIA_TYPE
}

/// Check if a media type is a runtime layer.
#[must_use]
pub fn is_runtime_layer(media_type: &str) -> bool {
    media_type == RUNTIME_LAYER_MEDIA_TYPE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_fabricks_module() {
        assert!(is_fabricks_module(WASM_LAYER_MEDIA_TYPE));
        assert!(!is_fabricks_module("application/octet-stream"));
    }

    #[test]
    fn test_is_fabricks_config() {
        assert!(is_fabricks_config(CONFIG_MEDIA_TYPE));
        assert!(!is_fabricks_config("application/json"));
    }

    #[test]
    fn test_is_source_layer() {
        assert!(is_source_layer(SOURCE_LAYER_MEDIA_TYPE));
        assert!(!is_source_layer(WASM_LAYER_MEDIA_TYPE));
    }

    #[test]
    fn test_is_runtime_layer() {
        assert!(is_runtime_layer(RUNTIME_LAYER_MEDIA_TYPE));
        assert!(!is_runtime_layer(WASM_LAYER_MEDIA_TYPE));
    }
}
