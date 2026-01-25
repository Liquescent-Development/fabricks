//! Run command implementation.
//!
//! Executes a WASM module through the daemon. All execution goes through the
//! daemon to enforce security boundaries, capability restrictions, and network
//! isolation.

use anyhow::{Context, Result};
use tracing::info;

use crate::cli::RunArgs;
use crate::commands::service::{ModuleSource, resolve_module_reference};
use crate::daemon_client::{DaemonClient, RunModuleRequest};
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
    // Resolve the module reference (builds and stores in OCI if needed)
    let ModuleSource::Storage { tag, .. } = resolve_module_reference(&args.module).await?;

    writeln_stderr(&format!("Running {tag} via daemon..."))?;

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
            reference: tag,
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
