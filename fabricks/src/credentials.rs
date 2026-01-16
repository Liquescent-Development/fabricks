//! Credential management for OCI registries.
//!
//! This module handles saving and loading authentication credentials for
//! container registries using the OS native secure storage:
//! - macOS: Keychain
//! - Windows: Credential Manager
//! - Linux: Secret Service (libsecret)

use anyhow::{Context, Result};
use keyring::Entry;

/// Service name for Fabricks credentials in the OS keyring.
const SERVICE_NAME: &str = "fabricks";

/// Credentials for a registry.
#[derive(Debug, Clone)]
pub struct Credentials {
    /// Username for authentication.
    pub username: String,
    /// Password or token for authentication.
    pub password: String,
}

/// Store for registry credentials using the OS keyring.
pub struct CredentialStore;

impl CredentialStore {
    /// Get credentials for a registry from the OS keyring.
    ///
    /// # Errors
    ///
    /// Returns an error if the keyring cannot be accessed.
    pub fn get(registry: &str) -> Result<Option<Credentials>> {
        let username_key = format!("{registry}:username");
        let password_key = format!("{registry}:password");

        let username_entry = Entry::new(SERVICE_NAME, &username_key)
            .context("failed to access keyring for username")?;

        let password_entry = Entry::new(SERVICE_NAME, &password_key)
            .context("failed to access keyring for password")?;

        // Try to get username
        let username = match username_entry.get_password() {
            Ok(u) => u,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(e) => return Err(e).context("failed to read username from keyring"),
        };

        // Try to get password
        let password = match password_entry.get_password() {
            Ok(p) => p,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(e) => return Err(e).context("failed to read password from keyring"),
        };

        Ok(Some(Credentials { username, password }))
    }

    /// Store credentials for a registry in the OS keyring.
    ///
    /// # Errors
    ///
    /// Returns an error if the keyring cannot be accessed or written to.
    pub fn set(registry: &str, credentials: &Credentials) -> Result<()> {
        let username_key = format!("{registry}:username");
        let password_key = format!("{registry}:password");

        let username_entry = Entry::new(SERVICE_NAME, &username_key)
            .context("failed to access keyring for username")?;

        let password_entry = Entry::new(SERVICE_NAME, &password_key)
            .context("failed to access keyring for password")?;

        username_entry
            .set_password(&credentials.username)
            .context("failed to store username in keyring")?;

        password_entry
            .set_password(&credentials.password)
            .context("failed to store password in keyring")?;

        Ok(())
    }

    /// Remove credentials for a registry from the OS keyring.
    ///
    /// Returns true if credentials were removed, false if they didn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the keyring cannot be accessed.
    pub fn remove(registry: &str) -> Result<bool> {
        let username_key = format!("{registry}:username");
        let password_key = format!("{registry}:password");

        let username_entry = Entry::new(SERVICE_NAME, &username_key)
            .context("failed to access keyring for username")?;

        let password_entry = Entry::new(SERVICE_NAME, &password_key)
            .context("failed to access keyring for password")?;

        let username_removed = match username_entry.delete_credential() {
            Ok(()) => true,
            Err(keyring::Error::NoEntry) => false,
            Err(e) => return Err(e).context("failed to delete username from keyring"),
        };

        let password_removed = match password_entry.delete_credential() {
            Ok(()) => true,
            Err(keyring::Error::NoEntry) => false,
            Err(e) => return Err(e).context("failed to delete password from keyring"),
        };

        Ok(username_removed || password_removed)
    }
}

#[cfg(test)]
mod tests {
    // Note: Keyring tests require OS interaction and may prompt for permissions.
    // These are intentionally minimal to avoid interfering with the user's keyring.

    use super::*;

    #[test]
    fn test_credentials_struct() {
        let creds = Credentials {
            username: "user".to_string(),
            password: "pass".to_string(),
        };

        assert_eq!(creds.username, "user");
        assert_eq!(creds.password, "pass");
    }
}
