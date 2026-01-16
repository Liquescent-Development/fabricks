//! Logout command implementation.
//!
//! Removes credentials for an OCI registry from the OS keyring.

use anyhow::{Context, Result};
use tracing::info;

use crate::cli::LogoutArgs;
use crate::credentials::CredentialStore;
use crate::output::writeln_stderr;

/// Run the logout command.
///
/// # Errors
///
/// Returns an error if keyring access fails.
pub fn run(args: &LogoutArgs) -> Result<()> {
    // Remove credentials
    let removed = CredentialStore::remove(&args.registry).context("Failed to access keyring")?;

    if removed {
        info!("Logged out from {}", args.registry);
        writeln_stderr(&format!("✓ Logged out from {}", args.registry))?;
    } else {
        writeln_stderr(&format!("No credentials found for {}", args.registry))?;
    }

    Ok(())
}
