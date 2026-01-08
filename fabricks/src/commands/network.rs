//! Network management commands.

use anyhow::{Context, Result};

use crate::cli::{NetworkAccessArg, NetworkArgs, NetworkCommands, NetworkIsolationArg, OutputFormat};
use crate::daemon_client::{
    CreateNetworkRequest, DaemonClient, JoinNetworkRequest, LeaveNetworkRequest,
};
use crate::output;

/// Runs the network command.
///
/// # Errors
///
/// Returns an error if the network command fails.
pub async fn run(args: &NetworkArgs) -> Result<()> {
    let client = match &args.socket {
        Some(path) => DaemonClient::with_socket(path.clone()),
        None => DaemonClient::new(),
    };

    match &args.command {
        NetworkCommands::Create {
            name,
            description,
            access,
            isolation,
            format,
        } => create_network(&client, name, description.clone(), *access, *isolation, *format).await,
        NetworkCommands::List { format } => list_networks(&client, *format).await,
        NetworkCommands::Inspect { network, format } => {
            inspect_network(&client, network, *format).await
        }
        NetworkCommands::Remove { network } => remove_network(&client, network).await,
        NetworkCommands::Join { network, service } => {
            join_network(&client, network, service).await
        }
        NetworkCommands::Leave { network, service } => {
            leave_network(&client, network, service).await
        }
    }
}

/// Creates a new network.
async fn create_network(
    client: &DaemonClient,
    name: &str,
    description: Option<String>,
    access: NetworkAccessArg,
    isolation: NetworkIsolationArg,
    format: OutputFormat,
) -> Result<()> {
    let access_str = match access {
        NetworkAccessArg::External => "external",
        NetworkAccessArg::Internal => "internal",
    };

    let isolation_str = match isolation {
        NetworkIsolationArg::Connected => "connected",
        NetworkIsolationArg::Isolated => "isolated",
    };

    let req = CreateNetworkRequest {
        name: name.to_string(),
        description,
        access: access_str.to_string(),
        isolation: isolation_str.to_string(),
    };

    let response = client
        .create_network(req)
        .await
        .context("Failed to create network")?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!(
                "Created network: {} ({})",
                response.name, response.id
            ))?;
        }
    }

    Ok(())
}

/// Lists all networks.
async fn list_networks(client: &DaemonClient, format: OutputFormat) -> Result<()> {
    let response = client
        .list_networks()
        .await
        .context("Failed to list networks")?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response.networks)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            if response.networks.is_empty() {
                output::writeln("No networks found")?;
                return Ok(());
            }

            output::writeln(&format!(
                "{:<12} {:<20} {:<10} {:<8} {}",
                "ID", "NAME", "ACCESS", "MEMBERS", "CREATED"
            ))?;

            for net in &response.networks {
                // Truncate ID for display
                let short_id = if net.id.len() > 12 {
                    &net.id[..12]
                } else {
                    &net.id
                };

                output::writeln(&format!(
                    "{:<12} {:<20} {:<10} {:<8} {}",
                    short_id, net.name, net.access, net.member_count, net.created_at
                ))?;
            }
        }
    }

    Ok(())
}

/// Inspects a network.
async fn inspect_network(client: &DaemonClient, network: &str, format: OutputFormat) -> Result<()> {
    let detail = client
        .get_network(network)
        .await
        .context("Failed to get network")?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&detail)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!("Network: {}", detail.name))?;
            output::writeln(&format!("  ID:          {}", detail.id))?;

            if let Some(ref desc) = detail.description {
                output::writeln(&format!("  Description: {desc}"))?;
            }

            output::writeln(&format!("  Access:      {}", detail.access))?;
            output::writeln(&format!("  Isolation:   {}", detail.isolation))?;
            output::writeln(&format!("  Encryption:  {}", detail.encryption))?;
            output::writeln(&format!("  Audit:       {}", detail.audit))?;
            output::writeln(&format!("  Created:     {}", detail.created_at))?;
            output::writeln(&format!("  Updated:     {}", detail.updated_at))?;

            if detail.members.is_empty() {
                output::writeln("\nMembers: (none)")?;
            } else {
                output::writeln(&format!("\nMembers ({}):", detail.members.len()))?;

                for service_id in &detail.members {
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

/// Removes a network.
async fn remove_network(client: &DaemonClient, network: &str) -> Result<()> {
    client
        .delete_network(network)
        .await
        .context("Failed to delete network")?;

    output::writeln(&format!("Deleted network: {network}"))?;

    Ok(())
}

/// Adds a service to a network.
async fn join_network(client: &DaemonClient, network: &str, service: &str) -> Result<()> {
    // First, get the service details to get the name
    let service_detail = client
        .get_service(service)
        .await
        .context("Failed to get service details")?;

    let req = JoinNetworkRequest {
        service_id: service_detail.id.clone(),
        service_name: service_detail.name.clone(),
    };

    client
        .join_network(network, req)
        .await
        .context("Failed to join network")?;

    output::writeln(&format!(
        "Added service {} ({}) to network {}",
        service_detail.name, service_detail.id, network
    ))?;

    Ok(())
}

/// Removes a service from a network.
async fn leave_network(client: &DaemonClient, network: &str, service: &str) -> Result<()> {
    // First, get the service details to confirm it exists
    let service_detail = client
        .get_service(service)
        .await
        .context("Failed to get service details")?;

    let req = LeaveNetworkRequest {
        service_id: service_detail.id.clone(),
    };

    client
        .leave_network(network, req)
        .await
        .context("Failed to leave network")?;

    output::writeln(&format!(
        "Removed service {} ({}) from network {}",
        service_detail.name, service_detail.id, network
    ))?;

    Ok(())
}
