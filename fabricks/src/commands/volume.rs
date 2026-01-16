//! Volume management commands.

use anyhow::{Context, Result};

use crate::cli::{OutputFormat, VolumeArgs, VolumeCommands};
use crate::daemon_client::{CreateVolumeRequest, DaemonClient};
use crate::output;

/// Runs the volume command.
///
/// # Errors
///
/// Returns an error if the volume command fails.
pub async fn run(args: &VolumeArgs) -> Result<()> {
    let client = match &args.socket {
        Some(path) => DaemonClient::with_socket(path.clone()),
        None => DaemonClient::new(),
    };

    match &args.command {
        VolumeCommands::Create {
            name,
            description,
            size,
            format,
        } => create_volume(&client, name, description.clone(), size.clone(), *format).await,
        VolumeCommands::List { format } => list_volumes(&client, *format).await,
        VolumeCommands::Inspect { volume, format } => {
            inspect_volume(&client, volume, *format).await
        }
        VolumeCommands::Remove { volume } => remove_volume(&client, volume).await,
    }
}

/// Creates a new volume.
async fn create_volume(
    client: &DaemonClient,
    name: &str,
    description: Option<String>,
    size: Option<String>,
    format: OutputFormat,
) -> Result<()> {
    let req = CreateVolumeRequest {
        name: name.to_string(),
        description,
        size,
    };

    let response = client
        .create_volume(req)
        .await
        .context("Failed to create volume")?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!(
                "Created volume: {} ({})",
                response.name, response.id
            ))?;
        }
    }

    Ok(())
}

/// Lists all volumes.
async fn list_volumes(client: &DaemonClient, format: OutputFormat) -> Result<()> {
    let response = client
        .list_volumes()
        .await
        .context("Failed to list volumes")?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response.volumes)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            if response.volumes.is_empty() {
                output::writeln("No volumes found")?;
                return Ok(());
            }

            output::writeln(&format!(
                "{:<12} {:<20} {:<10} {:<8} {}",
                "ID", "NAME", "SIZE", "MOUNTS", "CREATED"
            ))?;

            for vol in &response.volumes {
                // Truncate ID for display
                let short_id = if vol.id.len() > 12 {
                    &vol.id[..12]
                } else {
                    &vol.id
                };

                let size_display = vol.size.as_deref().unwrap_or("-");

                output::writeln(&format!(
                    "{:<12} {:<20} {:<10} {:<8} {}",
                    short_id, vol.name, size_display, vol.mount_count, vol.created_at
                ))?;
            }
        }
    }

    Ok(())
}

/// Inspects a volume.
async fn inspect_volume(client: &DaemonClient, volume: &str, format: OutputFormat) -> Result<()> {
    let detail = client
        .get_volume(volume)
        .await
        .context("Failed to get volume")?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&detail)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!("Volume: {}", detail.name))?;
            output::writeln(&format!("  ID:          {}", detail.id))?;

            if let Some(ref desc) = detail.description {
                output::writeln(&format!("  Description: {desc}"))?;
            }

            if let Some(ref size) = detail.size {
                output::writeln(&format!("  Size:        {size}"))?;
            }

            output::writeln(&format!("  Path:        {}", detail.path))?;
            output::writeln(&format!("  Created:     {}", detail.created_at))?;
            output::writeln(&format!("  Updated:     {}", detail.updated_at))?;

            if detail.mounted_by.is_empty() {
                output::writeln("\nMounted by: (none)")?;
            } else {
                output::writeln(&format!("\nMounted by ({}):", detail.mounted_by.len()))?;

                for service_id in &detail.mounted_by {
                    // Truncate ID for display
                    let short_id = if service_id.len() > 12 {
                        &service_id[..12]
                    } else {
                        service_id
                    };

                    output::writeln(&format!("  {short_id}"))?;
                }
            }
        }
    }

    Ok(())
}

/// Removes a volume.
async fn remove_volume(client: &DaemonClient, volume: &str) -> Result<()> {
    client
        .delete_volume(volume)
        .await
        .context("Failed to delete volume")?;

    output::writeln(&format!("Deleted volume: {volume}"))?;

    Ok(())
}
