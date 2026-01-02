//! Mortar project command implementation.

use std::path::Path;

use anyhow::{bail, Result};

use crate::cli::{MortarArgs, MortarCommands, OutputFormat};
use crate::daemon_client::DaemonClient;
use crate::output;

/// Runs the mortar command.
///
/// # Errors
///
/// Returns an error if the mortar command fails.
pub async fn run(args: &MortarArgs) -> Result<()> {
    let client = match &args.socket {
        Some(path) => DaemonClient::with_socket(path.clone()),
        None => DaemonClient::new(),
    };

    match &args.command {
        MortarCommands::Up { path, format } => deploy_mortar(&client, path, *format).await,
        MortarCommands::Down { project } => teardown_mortar(&client, project).await,
        MortarCommands::Status { project, format } => show_status(&client, project, *format).await,
        MortarCommands::List { format } => list_projects(&client, *format).await,
    }
}

async fn deploy_mortar(client: &DaemonClient, path: &Path, format: OutputFormat) -> Result<()> {
    // Resolve the mortar file path
    let mortar_path = if path.is_file() {
        path.to_path_buf()
    } else {
        let candidate = path.join("fabricks-mortar.toml");
        if candidate.exists() {
            candidate
        } else {
            bail!(
                "No fabricks-mortar.toml found at {}",
                path.display()
            );
        }
    };

    // Canonicalize the path for the daemon
    let absolute_path = mortar_path
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Failed to resolve path {}: {}", mortar_path.display(), e))?;

    let response = client.deploy_mortar(absolute_path).await?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!("Project '{}' deployed successfully.", response.project))?;
            output::writeln("")?;
            output::writeln("Services created:")?;
            for id in &response.service_ids {
                output::writeln(&format!("  - {id}"))?;
            }
            output::writeln("")?;
            output::writeln(&format!("Total: {} service(s)", response.total))?;
        }
    }

    Ok(())
}

async fn teardown_mortar(client: &DaemonClient, project: &str) -> Result<()> {
    client.teardown_mortar(project).await?;
    output::writeln(&format!("Project '{project}' torn down successfully."))?;
    Ok(())
}

async fn show_status(client: &DaemonClient, project: &str, format: OutputFormat) -> Result<()> {
    let response = client.get_project_services(project).await?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!("Project: {}", response.project))?;
            output::writeln("")?;

            if response.services.is_empty() {
                output::writeln("No services in project.")?;
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

async fn list_projects(client: &DaemonClient, format: OutputFormat) -> Result<()> {
    let response = client.list_projects().await?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            if response.projects.is_empty() {
                output::writeln("No projects deployed.")?;
            } else {
                output::writeln("Projects:")?;
                for project in &response.projects {
                    output::writeln(&format!("  - {project}"))?;
                }
                output::writeln("")?;
                output::writeln(&format!("Total: {} project(s)", response.total))?;
            }
        }
    }

    Ok(())
}
