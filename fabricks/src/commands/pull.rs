//! Pull command implementation.
//!
//! Downloads a module from an OCI registry and stores it locally.

use anyhow::{Context, Result};
use fabricks_oci::{ClientConfig, FabricksClient, LocalStorage, Reference, RegistryAuth};
use tracing::{debug, info};

use crate::credentials::CredentialStore;
use crate::output::writeln_stderr;

use crate::cli::PullArgs;

/// Run the pull command.
///
/// # Errors
///
/// Returns an error if:
/// - The registry reference is invalid
/// - Authentication fails
/// - The pull from registry fails
/// - Local storage operations fail
pub async fn run(args: &PullArgs) -> Result<()> {
    // Parse the registry reference
    let reference: Reference = args
        .reference
        .parse()
        .with_context(|| format!("Invalid registry reference: {}", args.reference))?;

    info!("Pulling module from {}", args.reference);

    // Get authentication for the registry
    let registry_host = reference.registry();
    let auth = get_auth_for_registry(registry_host)?;

    // Create the OCI client
    let client_config = ClientConfig {
        accept_invalid_certs: args.insecure,
    };
    let client = FabricksClient::with_config(&client_config);

    // Pull from registry
    writeln_stderr(&format!("Pulling {}...", args.reference))?;

    let pulled = client
        .pull(&reference, &auth)
        .await
        .context("Failed to pull module from registry")?;

    // Determine the local tag
    let local_tag = args.tag.clone().unwrap_or_else(|| {
        format!("{}:{}", pulled.module.name(), pulled.module.version())
    });

    // Store locally
    let storage = get_local_storage().await?;
    store_module(&storage, &pulled.module, &local_tag).await?;

    writeln_stderr(&format!("✓ Pulled {}", args.reference))?;
    writeln_stderr(&format!("  Saved as: {local_tag}"))?;
    writeln_stderr(&format!("  Digest: {}", pulled.digest))?;
    writeln_stderr(&format!("  WASM size: {} bytes", pulled.module.wasm_size()))?;

    Ok(())
}

/// Get the default local storage location.
async fn get_local_storage() -> Result<LocalStorage> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let storage_path = home.join(".fabricks").join("storage");
    LocalStorage::new(storage_path)
        .await
        .context("Failed to initialize local storage")
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

/// Store a pulled module in local storage.
async fn store_module(
    storage: &LocalStorage,
    module: &fabricks_oci::FabricksModule,
    tag: &str,
) -> Result<()> {
    // Store config blob
    let config_bytes = module.config_bytes().context("Failed to serialize config")?;
    let config_digest = storage
        .store_blob(&config_bytes)
        .await
        .context("Failed to store config blob")?;
    debug!("Stored config: {config_digest}");

    // Store WASM blob
    let wasm_digest = storage
        .store_blob(module.wasm_bytes())
        .await
        .context("Failed to store WASM blob")?;
    debug!("Stored WASM: {wasm_digest}");

    // Build and store manifest
    let manifest = build_local_manifest(module, &config_digest, &wasm_digest);
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .context("Failed to serialize manifest")?;
    let manifest_digest = storage
        .store_blob(&manifest_bytes)
        .await
        .context("Failed to store manifest")?;
    debug!("Stored manifest: {manifest_digest}");

    // Add to index
    storage
        .add_to_index(tag, &manifest_digest, i64::try_from(manifest_bytes.len()).unwrap_or(i64::MAX))
        .await
        .context("Failed to update storage index")?;

    Ok(())
}

/// Build a local manifest for storage.
fn build_local_manifest(
    module: &fabricks_oci::FabricksModule,
    config_digest: &str,
    wasm_digest: &str,
) -> serde_json::Value {
    let config_bytes = module.config_bytes().unwrap_or_default();
    let annotations = module.build_annotations();

    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "artifactType": "application/vnd.fabricks.module.v1",
        "config": {
            "mediaType": "application/vnd.fabricks.config.v1+toml",
            "digest": config_digest,
            "size": config_bytes.len(),
        },
        "layers": [{
            "mediaType": "application/vnd.fabricks.module.v1+wasm",
            "digest": wasm_digest,
            "size": module.wasm_size(),
        }],
        "annotations": annotations,
    })
}
