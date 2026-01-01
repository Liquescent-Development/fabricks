//! Push command implementation.
//!
//! Pushes a locally built module to an OCI registry.

use anyhow::{bail, Context, Result};
use fabricks_common::Fabrickfile;
use fabricks_oci::{ClientConfig, FabricksClient, FabricksModule, LocalStorage, Reference, RegistryAuth};
use tracing::{debug, info};

use crate::credentials::CredentialStore;
use crate::output::writeln_stderr;

use crate::cli::PushArgs;

/// Run the push command.
///
/// # Errors
///
/// Returns an error if:
/// - The local module cannot be found
/// - Authentication fails
/// - The push to registry fails
pub async fn run(args: &PushArgs) -> Result<()> {
    // Load the module from local storage
    let (fabrickfile, wasm_bytes) = load_local_module(&args.source).await?;

    info!(
        "Pushing {} v{} to {}",
        fabrickfile.info.name, fabrickfile.info.version, args.destination
    );

    // Parse the destination reference
    let reference: Reference = args
        .destination
        .parse()
        .with_context(|| format!("Invalid registry reference: {}", args.destination))?;

    // Get authentication for the registry
    let registry_host = reference.registry();
    let auth = get_auth_for_registry(registry_host)?;

    // Create the OCI client
    let client_config = ClientConfig {
        accept_invalid_certs: args.insecure,
    };
    let client = FabricksClient::with_config(&client_config);

    // Create the module
    let module = FabricksModule::new(fabrickfile, wasm_bytes);

    // Push to registry
    writeln_stderr(&format!("Pushing to {}...", args.destination))?;

    let manifest_url = client
        .push(&reference, &module, &auth)
        .await
        .context("Failed to push module to registry")?;

    writeln_stderr(&format!("✓ Pushed {}", args.destination))?;
    writeln_stderr(&format!("  Manifest: {manifest_url}"))?;

    Ok(())
}

/// Load a module from local storage by tag.
async fn load_local_module(tag: &str) -> Result<(Fabrickfile, Vec<u8>)> {
    debug!("Loading module from local storage: {tag}");

    let storage = get_local_storage_sync()?;

    // Get manifest digest
    let manifest_digest = storage
        .get_manifest_digest(tag)
        .await
        .with_context(|| format!("Module not found locally: {tag}\nRun `fabricks build` first."))?;

    // Load and parse manifest
    let manifest_bytes = storage
        .get_blob(&manifest_digest)
        .await
        .context("Failed to load manifest")?;

    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).context("Failed to parse manifest")?;

    // Extract config digest and load config
    let config_digest = manifest["config"]["digest"]
        .as_str()
        .context("Manifest missing config digest")?;

    let config_bytes = storage
        .get_blob(config_digest)
        .await
        .context("Failed to load config")?;

    let fabrickfile: Fabrickfile = toml::from_str(
        std::str::from_utf8(&config_bytes).context("Config is not valid UTF-8")?,
    )
    .context("Failed to parse Fabrickfile config")?;

    // Extract WASM layer digest and load WASM
    let layers = manifest["layers"]
        .as_array()
        .context("Manifest missing layers")?;

    let wasm_layer = layers
        .first()
        .context("Manifest has no layers")?;

    let wasm_digest = wasm_layer["digest"]
        .as_str()
        .context("Layer missing digest")?;

    let wasm_bytes = storage
        .get_blob(wasm_digest)
        .await
        .context("Failed to load WASM layer")?;

    Ok((fabrickfile, wasm_bytes))
}

/// Get the default local storage location.
fn get_local_storage_sync() -> Result<LocalStorage> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let storage_path = home.join(".fabricks").join("storage");

    if !storage_path.exists() {
        bail!(
            "Local storage not initialized.\n\
             Run `fabricks build` first."
        );
    }

    LocalStorage::open(storage_path).context("Failed to open local storage")
}

/// Get authentication for a registry.
fn get_auth_for_registry(registry: &str) -> Result<RegistryAuth> {
    // Try to load credentials from the OS keyring
    if let Some(creds) = CredentialStore::get(registry)? {
        debug!("Using stored credentials for {registry}");
        Ok(RegistryAuth::Basic(creds.username, creds.password))
    } else {
        debug!("No credentials found for {registry}, using anonymous auth");
        Ok(RegistryAuth::Anonymous)
    }
}
