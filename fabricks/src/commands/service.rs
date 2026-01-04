//! Service command implementation.

use std::path::Path;

use anyhow::{Context, Result};

use crate::cli::{OutputFormat, ServiceArgs, ServiceCommands};
use crate::daemon_client::{DaemonClient, RunFabrickfileRequest};
use crate::output;

/// Runs the service command.
///
/// # Errors
///
/// Returns an error if the service command fails.
pub async fn run(args: &ServiceArgs) -> Result<()> {
    let client = match &args.socket {
        Some(path) => DaemonClient::with_socket(path.clone()),
        None => DaemonClient::new(),
    };

    match &args.command {
        ServiceCommands::List { format } => list_services(&client, *format).await,
        ServiceCommands::Run { path, wasm, format } => {
            run_fabrickfile(&client, path, wasm.as_deref(), *format).await
        }
        ServiceCommands::Inspect { id, format } => inspect_service(&client, id, *format).await,
        ServiceCommands::Start { id } => start_service(&client, id).await,
        ServiceCommands::Stop { id } => stop_service(&client, id).await,
        ServiceCommands::Scale { id, replicas } => scale_service(&client, id, *replicas).await,
        ServiceCommands::Remove { id, force } => remove_service(&client, id, *force).await,
    }
}

async fn run_fabrickfile(
    client: &DaemonClient,
    path: &Path,
    wasm: Option<&Path>,
    format: OutputFormat,
) -> Result<()> {
    // Resolve Fabrickfile path
    let fabrickfile_path = if path.is_dir() {
        path.join("Fabrickfile")
    } else {
        path.to_path_buf()
    };

    // Make path absolute
    let fabrickfile_path = fabrickfile_path
        .canonicalize()
        .with_context(|| format!("Fabrickfile not found: {}", fabrickfile_path.display()))?;

    // Make wasm path absolute if provided
    let wasm_path = wasm
        .map(Path::canonicalize)
        .transpose()
        .context("WASM path not found")?;

    let req = RunFabrickfileRequest {
        fabrickfile_path,
        wasm_path,
    };

    let response = client.run_fabrickfile(req).await?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!(
                "Service '{}' deployed successfully.",
                response.name
            ))?;
            output::writeln(&format!("ID: {}", response.id))?;
        }
    }

    Ok(())
}

async fn inspect_service(client: &DaemonClient, id: &str, format: OutputFormat) -> Result<()> {
    let detail = client.get_service(id).await?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&detail)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!("Service: {}", detail.name))?;
            output::writeln(&format!("  ID:         {}", detail.id))?;
            output::writeln(&format!("  Version:    {}", detail.version))?;
            output::writeln(&format!("  State:      {}", detail.state))?;
            output::writeln(&format!("  Type:       {}", detail.config.service_type))?;
            output::writeln(&format!(
                "  Replicas:   {}/{}",
                detail.replicas.ready, detail.replicas.desired
            ))?;
            output::writeln(&format!("  Created:    {}", detail.created_at))?;
            output::writeln(&format!("  Updated:    {}", detail.updated_at))?;
            output::writeln(&format!("  WASM:       {}", detail.config.wasm_path))?;
            output::writeln(&format!("  Digest:     {}", detail.config.wasm_digest))?;

            if !detail.config.networks.is_empty() {
                output::writeln(&format!(
                    "  Networks:   {}",
                    detail.config.networks.join(", ")
                ))?;
            }

            if let Some(ref project) = detail.config.mortar_project {
                output::writeln(&format!("  Project:    {project}"))?;
            }

            if let Some(ref error) = detail.last_error {
                output::writeln(&format!("  Last Error: {error}"))?;
            }

            if !detail.instances.is_empty() {
                output::writeln("")?;
                output::writeln("Instances:")?;
                for instance in &detail.instances {
                    let started = instance
                        .started_at
                        .as_deref()
                        .unwrap_or("N/A");
                    output::writeln(&format!(
                        "  - {} ({}) started: {}",
                        instance.id, instance.state, started
                    ))?;
                }
            }
        }
    }

    Ok(())
}

async fn list_services(client: &DaemonClient, format: OutputFormat) -> Result<()> {
    let response = client.list_services().await?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response.services)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            if response.services.is_empty() {
                output::writeln("No services running.")?;
            } else {
                // Print header
                output::writeln(&format!(
                    "{:<12} {:<20} {:<10} {:<10} {:<10}",
                    "ID", "NAME", "VERSION", "STATE", "REPLICAS"
                ))?;
                output::writeln(&"-".repeat(62))?;

                for svc in &response.services {
                    let replicas = format!("{}/{}", svc.replicas.ready, svc.replicas.desired);
                    output::writeln(&format!(
                        "{:<12} {:<20} {:<10} {:<10} {:<10}",
                        svc.id, svc.name, svc.version, svc.state, replicas
                    ))?;
                }

                output::writeln("")?;
                output::writeln(&format!("Total: {} service(s)", response.total))?;
            }
        }
    }

    Ok(())
}

async fn start_service(client: &DaemonClient, id: &str) -> Result<()> {
    client.start_service(id).await?;
    output::writeln(&format!("Service {id} started."))?;
    Ok(())
}

async fn stop_service(client: &DaemonClient, id: &str) -> Result<()> {
    client.stop_service(id).await?;
    output::writeln(&format!("Service {id} stopped."))?;
    Ok(())
}

async fn scale_service(client: &DaemonClient, id: &str, replicas: usize) -> Result<()> {
    client.scale_service(id, replicas).await?;
    output::writeln(&format!("Service {id} scaled to {replicas} replica(s)."))?;
    Ok(())
}

async fn remove_service(client: &DaemonClient, id: &str, force: bool) -> Result<()> {
    if force {
        // Try to stop first, ignore errors
        let _ = client.stop_service(id).await;
    }

    client.delete_service(id).await?;
    output::writeln(&format!("Service {id} removed."))?;
    Ok(())
}
