//! Service command implementation.

use anyhow::Result;

use crate::cli::{OutputFormat, ServiceArgs, ServiceCommands};
use crate::daemon_client::DaemonClient;
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
        ServiceCommands::Start { id } => start_service(&client, id).await,
        ServiceCommands::Stop { id } => stop_service(&client, id).await,
        ServiceCommands::Scale { id, replicas } => scale_service(&client, id, *replicas).await,
        ServiceCommands::Remove { id, force } => remove_service(&client, id, *force).await,
    }
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
