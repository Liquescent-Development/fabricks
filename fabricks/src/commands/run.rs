//! Run command implementation.
//!
//! Executes a WASM module through the daemon. All execution goes through the
//! daemon to enforce security boundaries, capability restrictions, and network
//! isolation.

use std::path::Path;

use anyhow::{Context, Result, bail};
use tracing::info;

use crate::cli::RunArgs;
use crate::daemon_client::{DaemonClient, RunModuleRequest, RunFabrickfileRequest};
use crate::output::{writeln, writeln_stderr};

/// Run the run command.
///
/// All execution goes through the daemon to enforce:
/// - Capability-based security
/// - Network isolation
/// - Resource limits
///
/// # Errors
///
/// Returns an error if:
/// - The daemon is not running
/// - The module cannot be found or loaded
/// - The module execution fails
pub async fn run(args: &RunArgs) -> Result<()> {
    // Check if we're pointing to a directory with a Fabrickfile or to a Fabrickfile directly
    if is_fabrickfile_reference(&args.module) {
        return run_fabrickfile_via_daemon(&args.module).await;
    }

    // Otherwise, it's a module reference (tag or registry reference)
    run_module_via_daemon(args).await
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
async fn run_fabrickfile_via_daemon(reference: &str) -> Result<()> {
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
        .await
        .context("Failed to run via daemon. Is the daemon running? Start it with: fabricksd")?;

    writeln(&format!(
        "Service '{}' started with ID: {}",
        response.name, response.id
    ))?;

    info!("Service started via daemon: {}", response.id);

    Ok(())
}

/// Run a module (by tag or registry reference) through the daemon.
async fn run_module_via_daemon(args: &RunArgs) -> Result<()> {
    let reference = &args.module;

    // Check if it looks like a registry reference (contains a slash)
    if reference.contains('/') && !Path::new(reference).exists() {
        // TODO: Implement pull-on-demand in daemon
        bail!(
            "Registry references are not yet supported for `run`.\n\
             Use `fabricks pull {reference}` first, then run with the local tag."
        );
    }

    writeln_stderr(&format!("Running {reference} via daemon..."))?;

    let client = DaemonClient::new();

    // Parse environment variable overrides
    let env_vars: Vec<(String, String)> = args
        .envs
        .iter()
        .filter_map(|env_arg| {
            env_arg
                .split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
        })
        .collect();

    let response = client
        .run_module(RunModuleRequest {
            reference: reference.clone(),
            args: args.args.clone(),
            env_vars,
            no_capabilities: args.no_capabilities,
        })
        .await
        .context("Failed to run via daemon. Is the daemon running? Start it with: fabricksd")?;

    writeln(&format!(
        "Service '{}' started with ID: {}",
        response.name, response.id
    ))?;

    info!("Service started via daemon: {}", response.id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_fabrickfile_reference_direct_path() {
        let temp = TempDir::new().expect("create temp dir");
        let fabrickfile_path = temp.path().join("Fabrickfile");
        std::fs::write(&fabrickfile_path, "fabrick_version = \"1.0\"").expect("write");

        assert!(is_fabrickfile_reference(fabrickfile_path.to_str().unwrap()));
    }

    #[test]
    fn test_is_fabrickfile_reference_directory() {
        let temp = TempDir::new().expect("create temp dir");
        let fabrickfile_path = temp.path().join("Fabrickfile");
        std::fs::write(&fabrickfile_path, "fabrick_version = \"1.0\"").expect("write");

        assert!(is_fabrickfile_reference(temp.path().to_str().unwrap()));
    }

    #[test]
    fn test_is_fabrickfile_reference_tag() {
        assert!(!is_fabrickfile_reference("my-module:1.0.0"));
        assert!(!is_fabrickfile_reference("python-hello:latest"));
    }

    #[test]
    fn test_is_fabrickfile_reference_registry() {
        assert!(!is_fabrickfile_reference("ghcr.io/user/module:latest"));
        assert!(!is_fabrickfile_reference("docker.io/library/nginx:1.0"));
    }
}
