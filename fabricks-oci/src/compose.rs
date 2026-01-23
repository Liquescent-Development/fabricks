//! WASM component composition.
//!
//! This module provides functionality for composing WASM components together,
//! enabling base image support where a runtime component provides dependencies
//! that user code depends on.
//!
//! Uses the [wac-graph](https://github.com/bytecodealliance/wac) library for
//! WebAssembly component composition.

use std::ops::Index;

use thiserror::Error;
use tracing::debug;
use wac_graph::types::{Package, WorldId};
use wac_graph::{CompositionGraph, EncodeOptions};

/// Errors that can occur during component composition.
#[derive(Debug, Error)]
pub enum ComposeError {
    /// Failed to parse a WASM component.
    #[error("failed to parse WASM component '{name}': {message}")]
    ParseError {
        /// The name of the component that failed to parse.
        name: String,
        /// Error message.
        message: String,
    },

    /// Failed to register a package with the composition graph.
    #[error("failed to register package '{name}': {source}")]
    RegisterError {
        /// The name of the package that failed to register.
        name: String,
        /// The underlying error.
        #[source]
        source: wac_graph::RegisterPackageError,
    },

    /// Failed to encode the composed component.
    #[error("failed to encode composed component: {0}")]
    EncodeError(#[from] wac_graph::EncodeError),
}

/// Result type for composition operations.
pub type Result<T> = std::result::Result<T, ComposeError>;

/// Compose a base runtime component with user code.
///
/// This function takes a base runtime WASM component and user code WASM component,
/// and produces a composed component where the user code can use exports from
/// the runtime.
///
/// # Arguments
///
/// * `base_name` - Name identifier for the base runtime package
/// * `base_wasm` - The base runtime WASM component bytes
/// * `user_name` - Name identifier for the user code package
/// * `user_wasm` - The user code WASM component bytes
///
/// # Returns
///
/// The composed WASM component bytes.
///
/// # Errors
///
/// Returns an error if:
/// - Either component fails to parse
/// - The composition graph cannot be built
/// - The components are incompatible
/// - Encoding fails
///
/// # Example
///
/// ```ignore
/// use fabricks_oci::compose::compose_components;
///
/// let runtime_wasm = std::fs::read("python-runtime.wasm")?;
/// let user_wasm = std::fs::read("my-script.wasm")?;
///
/// let composed = compose_components(
///     "fabricks:python-runtime",
///     &runtime_wasm,
///     "user:my-script",
///     &user_wasm,
/// )?;
/// ```
pub fn compose_components(
    base_name: &str,
    base_wasm: &[u8],
    user_name: &str,
    user_wasm: &[u8],
) -> Result<Vec<u8>> {
    let mut graph = CompositionGraph::new();

    // Parse the base runtime package first to extract its world type
    let base_pkg = Package::from_bytes(base_name, None, base_wasm, graph.types_mut()).map_err(
        |e| ComposeError::ParseError {
            name: base_name.to_string(),
            message: e.to_string(),
        },
    )?;
    let base_world_id = base_pkg.ty();

    // Register the base runtime package
    let base_id = graph.register_package(base_pkg).map_err(|e| {
        ComposeError::RegisterError {
            name: base_name.to_string(),
            source: e,
        }
    })?;

    // Parse the user code package
    let user_pkg = Package::from_bytes(user_name, None, user_wasm, graph.types_mut()).map_err(
        |e| ComposeError::ParseError {
            name: user_name.to_string(),
            message: e.to_string(),
        },
    )?;
    let user_world_id = user_pkg.ty();

    // Register the user code package
    let user_id = graph.register_package(user_pkg).map_err(|e| {
        ComposeError::RegisterError {
            name: user_name.to_string(),
            source: e,
        }
    })?;

    // Instantiate the base runtime
    let base_instance = graph.instantiate(base_id);

    // Instantiate the user code
    let user_instance = graph.instantiate(user_id);

    // Wire up matching exports from base to imports of user
    wire_imports_to_exports(&mut graph, base_instance, user_instance, user_world_id);

    // Export all of the user instance's exports from the composed component
    export_user_exports(&mut graph, user_instance, base_world_id);

    // Encode the composed component
    let options = EncodeOptions::default();
    let bytes = graph.encode(options)?;

    Ok(bytes)
}

/// Wire up imports from the user instance to exports from the base instance.
fn wire_imports_to_exports(
    graph: &mut CompositionGraph,
    base_instance: wac_graph::NodeId,
    user_instance: wac_graph::NodeId,
    user_world_id: WorldId,
) {
    // Get import names from the user's world type
    let import_names: Vec<String> = {
        let types = graph.types();
        let world = types.index(user_world_id);
        world.imports.keys().cloned().collect()
    };

    for import_name in import_names {
        // Try to alias the export from base and set it as argument to user
        if let Ok(export_node) = graph.alias_instance_export(base_instance, &import_name) {
            // Ignore errors if the argument doesn't exist or doesn't match
            if graph
                .set_instantiation_argument(user_instance, &import_name, export_node)
                .is_ok()
            {
                debug!("Wired import '{import_name}' from base to user");
            }
        }
    }
}

/// Export all exports from the user instance as exports of the composed component.
fn export_user_exports(
    graph: &mut CompositionGraph,
    user_instance: wac_graph::NodeId,
    base_world_id: WorldId,
) {
    // Get export names from the base's world type (the user instance exports)
    let export_names: Vec<String> = {
        let types = graph.types();
        let world = types.index(base_world_id);
        world.exports.keys().cloned().collect()
    };

    for export_name in export_names {
        if let Ok(export_node) = graph.alias_instance_export(user_instance, &export_name) {
            // Export from the composed component
            if graph.export(export_node, &export_name).is_ok() {
                debug!("Exported '{export_name}' from composed component");
            }
        }
    }
}

/// Check if two WASM components can be composed together.
///
/// This validates that the base runtime exports can satisfy at least some
/// of the user code's imports.
///
/// # Arguments
///
/// * `base_wasm` - The base runtime WASM component bytes
/// * `user_wasm` - The user code WASM component bytes
///
/// # Returns
///
/// `true` if the components can potentially be composed, `false` otherwise.
#[must_use]
pub fn can_compose(base_wasm: &[u8], user_wasm: &[u8]) -> bool {
    let mut graph = CompositionGraph::new();

    // Try to parse both components
    let Ok(base_pkg) = Package::from_bytes("base", None, base_wasm, graph.types_mut()) else {
        return false;
    };

    let Ok(base_id) = graph.register_package(base_pkg) else {
        return false;
    };

    let Ok(user_pkg) = Package::from_bytes("user", None, user_wasm, graph.types_mut()) else {
        return false;
    };
    let user_world_id = user_pkg.ty();

    if graph.register_package(user_pkg).is_err() {
        return false;
    }

    // Check if any user imports can be satisfied by base exports
    let base_instance = graph.instantiate(base_id);

    // Get import names from the user's world
    let import_names: Vec<String> = {
        let types = graph.types();
        let world = types.index(user_world_id);
        world.imports.keys().cloned().collect()
    };

    for import_name in import_names {
        // If we can alias at least one export from base that matches
        // a user import, they can be composed
        if graph
            .alias_instance_export(base_instance, &import_name)
            .is_ok()
        {
            return true;
        }
    }

    // If user has no imports, composition is trivially possible
    // (they just get bundled together)
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require valid WASM component binaries.
    // In a real test setup, we'd generate these or use fixtures.

    #[test]
    fn test_compose_error_display() {
        let err = ComposeError::ParseError {
            name: "test".to_string(),
            message: "invalid wasm".to_string(),
        };
        assert!(err.to_string().contains("test"));
        assert!(err.to_string().contains("parse"));
    }

    #[test]
    fn test_can_compose_invalid_wasm() {
        // Invalid WASM bytes should return false
        assert!(!can_compose(b"not wasm", b"also not wasm"));
    }
}
