//! Login command implementation.
//!
//! Authenticates with an OCI registry and saves credentials to the OS keyring.

use std::io::{self, Read};

use anyhow::{Context, Result, bail};
use tracing::info;

use crate::cli::LoginArgs;
use crate::credentials::{CredentialStore, Credentials};
use crate::output::{prompt, writeln_stderr};

/// Run the login command.
///
/// # Errors
///
/// Returns an error if:
/// - Credentials cannot be read
/// - Keyring access fails
pub fn run(args: &LoginArgs) -> Result<()> {
    // Get username
    let username = if let Some(ref u) = args.username {
        u.clone()
    } else {
        prompt("Username: ").context("Failed to read username")?
    };

    if username.is_empty() {
        bail!("Username cannot be empty");
    }

    // Get password
    let password = if args.password_stdin {
        read_password_from_stdin()?
    } else if let Some(ref p) = args.password {
        p.clone()
    } else {
        rpassword::prompt_password("Password: ").context("Failed to read password")?
    };

    if password.is_empty() {
        bail!("Password cannot be empty");
    }

    // Store credentials
    let credentials = Credentials { username, password };
    CredentialStore::set(&args.registry, &credentials)
        .context("Failed to store credentials in keyring")?;

    info!("Logged in to {}", args.registry);
    writeln_stderr(&format!("✓ Logged in to {}", args.registry))?;

    Ok(())
}

/// Read password from stdin (for piped input).
fn read_password_from_stdin() -> Result<String> {
    let mut password = String::new();
    io::stdin()
        .read_to_string(&mut password)
        .context("Failed to read password from stdin")?;

    // Trim whitespace (including newline)
    Ok(password.trim().to_string())
}
