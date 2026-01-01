//! Run command implementation.
//!
//! Executes a WASM module locally with capability enforcement.

use std::path::Path;

use anyhow::{bail, Context, Result};
use fabricks_common::{Capabilities, Fabrickfile};
use fabricks_oci::LocalStorage;
use fabricks_runtime::{Runtime, RuntimeConfig};
use tracing::{debug, info};

use crate::cli::RunArgs;
use crate::output::writeln_stderr;

/// Run the run command.
///
/// # Errors
///
/// Returns an error if:
/// - The module cannot be found or loaded
/// - The module execution fails
pub async fn run(args: &RunArgs) -> Result<()> {
    // Load the module
    let (fabrickfile, wasm_bytes) = load_module(&args.module).await?;

    info!("Running {} v{}", fabrickfile.info.name, fabrickfile.info.version);

    // Determine capabilities
    let capabilities = if args.no_capabilities {
        debug!("Capability enforcement disabled");
        Capabilities::default()
    } else {
        fabrickfile.capabilities.clone()
    };

    // Parse environment variable overrides
    let mut env_overrides = Vec::new();
    for env_arg in &args.envs {
        if let Some((key, value)) = env_arg.split_once('=') {
            env_overrides.push((key.to_string(), value.to_string()));
            debug!("Environment override: {key}={value}");
        } else {
            bail!("Invalid environment variable format: {env_arg}\nExpected: NAME=VALUE");
        }
    }

    // Build args for the WASM module
    let module_args = build_module_args(&fabrickfile.info.name, &args.args);

    // Create runtime config
    let config = RuntimeConfig {
        capabilities,
        args: module_args,
        working_dir: None,
        inherit_stdio: true,
        fuel_limit: None,
        epoch_interruption: false,
    };

    // Create and run the runtime
    let runtime = Runtime::new(&wasm_bytes, config)
        .context("Failed to create WASM runtime")?;

    writeln_stderr(&format!("Running {}...", fabrickfile.info.name))?;

    runtime.run().context("Module execution failed")?;

    info!("Module execution completed");
    Ok(())
}

/// Load a module from various sources.
///
/// Supports:
/// - Local WASM file paths (./module.wasm)
/// - Local tags (my-module:1.0.0)
/// - Registry references (ghcr.io/user/module:latest) - not yet implemented
async fn load_module(reference: &str) -> Result<(Fabrickfile, Vec<u8>)> {
    // Check if it's a local file path
    let path = Path::new(reference);
    if path.exists() && path.is_file() {
        return load_from_file_sync(path);
    }

    // Check if it looks like a registry reference (contains a slash)
    if reference.contains('/') {
        // TODO: Implement registry pull on demand
        bail!(
            "Registry references are not yet supported for `run`.\n\
             Use `fabricks pull {reference}` first, then run with the local tag."
        );
    }

    // Otherwise, treat as a local tag
    load_from_storage(reference).await
}

/// Load a module from a local WASM file.
fn load_from_file_sync(path: &Path) -> Result<(Fabrickfile, Vec<u8>)> {
    debug!("Loading module from file: {}", path.display());

    let wasm_bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read WASM file: {}", path.display()))?;

    // Check for a Fabrickfile in the same directory
    let parent = path.parent().unwrap_or(Path::new("."));
    let fabrickfile_path = parent.join("Fabrickfile");

    let fabrickfile = if fabrickfile_path.exists() {
        fabricks_common::parse_fabrickfile(&fabrickfile_path)?
    } else {
        // Create a minimal fabrickfile from the filename
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        create_minimal_fabrickfile(name)
    };

    Ok((fabrickfile, wasm_bytes))
}

/// Load a module from local storage by tag.
async fn load_from_storage(tag: &str) -> Result<(Fabrickfile, Vec<u8>)> {
    debug!("Loading module from storage: {tag}");

    let storage = get_local_storage_sync()?;

    // Get manifest digest
    let manifest_digest = storage
        .get_manifest_digest(tag)
        .await
        .with_context(|| format!("Module not found: {tag}"))?;

    // Load and parse manifest
    let manifest_bytes = storage
        .get_blob(&manifest_digest)
        .await
        .context("Failed to load manifest")?;

    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).context("Failed to parse manifest")?;

    // Extract config digest and load config
    let config_digest = manifest["config"]["digest"]
        .as_str()
        .context("Manifest missing config digest")?;

    let config_bytes = storage
        .get_blob(config_digest)
        .await
        .context("Failed to load config")?;

    let fabrickfile: Fabrickfile = toml::from_str(
        std::str::from_utf8(&config_bytes).context("Config is not valid UTF-8")?,
    )
    .context("Failed to parse Fabrickfile config")?;

    // Extract WASM layer digest and load WASM
    let layers = manifest["layers"]
        .as_array()
        .context("Manifest missing layers")?;

    let wasm_layer = layers
        .first()
        .context("Manifest has no layers")?;

    let wasm_digest = wasm_layer["digest"]
        .as_str()
        .context("Layer missing digest")?;

    let wasm_bytes = storage
        .get_blob(wasm_digest)
        .await
        .context("Failed to load WASM layer")?;

    Ok((fabrickfile, wasm_bytes))
}

/// Get the default local storage location.
fn get_local_storage_sync() -> Result<LocalStorage> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let storage_path = home.join(".fabricks").join("storage");

    if !storage_path.exists() {
        bail!(
            "Local storage not initialized.\n\
             Run `fabricks build` or `fabricks pull` first."
        );
    }

    LocalStorage::open(storage_path).context("Failed to open local storage")
}

/// Build the argument list for the WASM module.
fn build_module_args(module_name: &str, extra_args: &[String]) -> Vec<String> {
    let mut args = vec![module_name.to_string()];
    args.extend(extra_args.iter().cloned());
    args
}

/// Create a minimal Fabrickfile for a standalone WASM file.
fn create_minimal_fabrickfile(name: &str) -> Fabrickfile {
    Fabrickfile {
        fabrick_version: "1.0".to_string(),
        info: fabricks_common::models::fabrickfile::Info {
            name: name.to_string(),
            version: "0.0.0".to_string(),
            description: None,
            authors: None,
            license: None,
            homepage: None,
            repository: None,
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
