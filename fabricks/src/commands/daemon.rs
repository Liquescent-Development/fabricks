//! Daemon command implementation.

use anyhow::Result;

use crate::cli::{DaemonArgs, DaemonCommands, OutputFormat};
use crate::daemon_client::DaemonClient;
use crate::output;

/// Runs the daemon command.
///
/// # Errors
///
/// Returns an error if the daemon command fails.
pub async fn run(args: &DaemonArgs) -> Result<()> {
    match &args.command {
        DaemonCommands::Info { format, socket } => {
            let client = match socket {
                Some(path) => DaemonClient::with_socket(path.clone()),
                None => DaemonClient::new(),
            };

            let info = client.info().await?;

            match format {
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&info)?;
                    output::writeln(&json)?;
                }
                OutputFormat::Text => {
                    output::writeln(&format!("Version:     {}", info.version))?;
                    output::writeln(&format!("API Version: {}", info.api_version))?;
                    output::writeln(&format!("Runtime:     {}", info.runtime))?;
                    output::writeln(&format!("Platform:    {}", info.platform))?;
                    output::writeln(&format!("Started At:  {}", info.started_at))?;
                    output::writeln(&format!("Uptime:      {}", info.uptime))?;
                    output::writeln("")?;
                    output::writeln("Configuration:")?;
                    output::writeln(&format!("  Socket:       {}", info.config.socket))?;
                    output::writeln(&format!("  Data Dir:     {}", info.config.data_dir))?;
                    output::writeln(&format!("  Max Services: {}", info.config.max_services))?;
                }
            }

            Ok(())
        }

        DaemonCommands::Stop { socket } => {
            let client = match socket {
                Some(path) => DaemonClient::with_socket(path.clone()),
                None => DaemonClient::new(),
            };

            output::writeln("Stopping daemon...")?;
            let response = client.shutdown().await?;
            output::writeln(&response.message)?;

            Ok(())
        }
    }
}
