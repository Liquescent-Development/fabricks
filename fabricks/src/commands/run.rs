//! Run command implementation.
//!
//! Executes a WASM module through the daemon (default) or locally.

use std::path::Path;

use anyhow::{Context, Result, bail};
use fabricks_common::{Capabilities, Fabrickfile};
use fabricks_oci::LocalStorage;
use fabricks_runtime::{Runtime, RuntimeConfig};
use tracing::{debug, info};

use crate::cli::RunArgs;
use crate::daemon_client::{DaemonClient, RunFabrickfileRequest};
use crate::output::{writeln, writeln_stderr};

/// Run the run command.
///
/// # Errors
///
/// Returns an error if:
/// - The module cannot be found or loaded
/// - The module execution fails
pub async fn run(args: &RunArgs) -> Result<()> {
    // Check if we're pointing to a directory with a Fabrickfile or to a Fabrickfile directly
    if is_fabrickfile_reference(&args.module) {
        return run_via_daemon(&args.module).await;
    }

    // Otherwise, fall back to local execution
    run_locally(args).await
}

/// Checks if the reference points to a Fabrickfile.
fn is_fabrickfile_reference(reference: &str) -> bool {
    let path = Path::new(reference);

    // Direct path to a Fabrickfile
    if path.is_file() && path.file_name().is_some_and(|n| n == "Fabrickfile") {
        return true;
    }

    // Directory containing a Fabrickfile
    if path.is_dir() && path.join("Fabrickfile").exists() {
        return true;
    }

    false
}

/// Run a Fabrickfile through the daemon.
async fn run_via_daemon(reference: &str) -> Result<()> {
    let path = Path::new(reference);

    // Resolve the Fabrickfile path
    let fabrickfile_path = if path.is_dir() {
        path.join("Fabrickfile")
    } else {
        path.to_path_buf()
    };

    // Canonicalize the path
    let absolute_path = fabrickfile_path
        .canonicalize()
        .with_context(|| format!("Failed to resolve path: {}", fabrickfile_path.display()))?;

    writeln_stderr(&format!(
        "Running {} via daemon...",
        fabrickfile_path.display()
    ))?;

    let client = DaemonClient::new();

    let response = client
        .run_fabrickfile(RunFabrickfileRequest {
            fabrickfile_path: absolute_path,
            wasm_path: None,
        })
        .await?;

    writeln(&format!(
        "Service '{}' started with ID: {}",
        response.name, response.id
    ))?;

    Ok(())
}

/// Run a module locally (fallback for direct WASM files).
async fn run_locally(args: &RunArgs) -> Result<()> {
    // Load the module
    let (fabrickfile, wasm_bytes) = load_module(&args.module).await?;

    info!(
        "Running {} v{}",
        fabrickfile.info.name, fabrickfile.info.version
    );

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
        volume_mounts: Vec::new(),
    };

    // Create and run the runtime
    let runtime = Runtime::new(&wasm_bytes, config).context("Failed to create WASM runtime")?;

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

    let fabrickfile: Fabrickfile =
        toml::from_str(std::str::from_utf8(&config_bytes).context("Config is not valid UTF-8")?)
            .context("Failed to parse Fabrickfile config")?;

    // Extract WASM layer digest and load WASM
    let layers = manifest["layers"]
        .as_array()
        .context("Manifest missing layers")?;

    let wasm_layer = layers.first().context("Manifest has no layers")?;

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
            service_type: fabricks_common::models::fabrickfile::ServiceType::default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_minimal_fabrickfile() {
        let fabrickfile = create_minimal_fabrickfile("my-module");

        assert_eq!(fabrickfile.info.name, "my-module");
        assert_eq!(fabrickfile.info.version, "0.0.0");
        assert_eq!(fabrickfile.fabrick_version, "1.0");
        assert!(fabrickfile.build.is_none());
    }

    #[test]
    fn test_build_module_args_empty() {
        let args = build_module_args("test-module", &[]);
        assert_eq!(args, vec!["test-module".to_string()]);
    }

    #[test]
    fn test_build_module_args_with_extras() {
        let extra_args = vec!["--flag".to_string(), "value".to_string()];
        let args = build_module_args("test-module", &extra_args);

        assert_eq!(
            args,
            vec![
                "test-module".to_string(),
                "--flag".to_string(),
                "value".to_string(),
            ]
        );
    }

    #[test]
    fn test_load_from_file_sync_with_fabrickfile() {
        let temp = TempDir::new().expect("create temp dir");

        // Create a Fabrickfile
        let fabrickfile_content = r#"
            fabrick_version = "1.0"
            [info]
            name = "file-test"
            version = "1.2.3"
        "#;
        std::fs::write(temp.path().join("Fabrickfile"), fabrickfile_content)
            .expect("write fabrickfile");

        // Create a WASM file
        let wasm_content = b"\x00asm\x01\x00\x00\x00";
        let wasm_path = temp.path().join("module.wasm");
        std::fs::write(&wasm_path, wasm_content).expect("write wasm");

        let (fabrickfile, wasm_bytes) = load_from_file_sync(&wasm_path).expect("load from file");

        assert_eq!(fabrickfile.info.name, "file-test");
        assert_eq!(fabrickfile.info.version, "1.2.3");
        assert_eq!(wasm_bytes, wasm_content);
    }

    #[test]
    fn test_load_from_file_sync_without_fabrickfile() {
        let temp = TempDir::new().expect("create temp dir");

        // Create a WASM file without a Fabrickfile
        let wasm_content = b"\x00asm\x01\x00\x00\x00";
        let wasm_path = temp.path().join("standalone.wasm");
        std::fs::write(&wasm_path, wasm_content).expect("write wasm");

        let (fabrickfile, wasm_bytes) = load_from_file_sync(&wasm_path).expect("load from file");

        assert_eq!(fabrickfile.info.name, "standalone");
        assert_eq!(fabrickfile.info.version, "0.0.0");
        assert_eq!(wasm_bytes, wasm_content);
    }

    #[test]
    fn test_load_from_file_sync_not_found() {
        let result = load_from_file_sync(Path::new("/nonexistent/module.wasm"));
        assert!(result.is_err());
    }
}
