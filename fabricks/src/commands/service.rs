//! Service command implementation.

use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use fabricks_common::{Fabrickfile, parse_fabrickfile};
use fabricks_oci::{FabricksModule, LocalStorage};
use tempfile::NamedTempFile;
use tracing::{debug, info};

use crate::cli::{OutputFormat, ServiceArgs, ServiceCommands};
use crate::daemon_client::{DaemonClient, RunFabrickfileRequest};
use crate::output;

/// Source of a resolved module.
enum ModuleSource {
    /// A module loaded from local storage.
    Storage {
        tag: String,
        fabrickfile: Fabrickfile,
        wasm_bytes: Vec<u8>,
    },
}

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
        ServiceCommands::Run { reference, format } => {
            run_service(&client, reference, *format).await
        }
        ServiceCommands::Inspect { id, format } => inspect_service(&client, id, *format).await,
        ServiceCommands::Start { id } => start_service(&client, id).await,
        ServiceCommands::Stop { id } => stop_service(&client, id).await,
        ServiceCommands::Scale { id, replicas } => scale_service(&client, id, *replicas).await,
        ServiceCommands::Remove { id, force } => remove_service(&client, id, *force).await,
    }
}

/// Resolve a module reference to its source.
///
/// Reference types:
/// - Local file path (exists on filesystem) -> build if needed, store, load from storage
/// - Registry reference (contains '/') -> not yet supported
/// - Local storage tag (e.g., "hello-http:0.1.0") -> load from storage
async fn resolve_module_reference(reference: &str) -> Result<ModuleSource> {
    let path = Path::new(reference);

    // 1. Local file/directory path
    if path.exists() {
        return resolve_from_path(path).await;
    }

    // 2. Registry reference (contains '/')
    if reference.contains('/') {
        bail!(
            "Registry references not yet supported.\n\
             Pull the module first with: fabricks pull {reference}"
        );
    }

    // 3. Local tag (e.g., "hello-http:0.1.0")
    resolve_from_storage(reference).await
}

/// Resolve module from a filesystem path.
///
/// This will build the module if needed and store it in local storage.
async fn resolve_from_path(path: &Path) -> Result<ModuleSource> {
    let fabrickfile_path = if path.is_dir() {
        path.join("Fabrickfile")
    } else {
        path.to_path_buf()
    };

    let fabrickfile_path = fabrickfile_path
        .canonicalize()
        .with_context(|| format!("Fabrickfile not found: {}", fabrickfile_path.display()))?;

    let workdir = fabrickfile_path
        .parent()
        .context("Fabrickfile has no parent directory")?;

    // Parse the Fabrickfile
    let fabrickfile = parse_fabrickfile(&fabrickfile_path)?;

    // Determine tag
    let tag = format!("{}:{}", fabrickfile.info.name, fabrickfile.info.version);

    // Check if already in storage
    if let Ok(source) = resolve_from_storage(&tag).await {
        info!("Using cached module: {tag}");
        return Ok(source);
    }

    // Build and store the module
    output::writeln(&format!("Building {tag}..."))?;
    let wasm_bytes = build_module(&fabrickfile, workdir)?;

    // Store in local storage
    let module = FabricksModule::new(fabrickfile.clone(), wasm_bytes.clone());
    store_module(&module, &tag).await?;

    output::writeln(&format!("Stored as: {tag}"))?;

    Ok(ModuleSource::Storage {
        tag,
        fabrickfile,
        wasm_bytes,
    })
}

/// Build a module using its build command.
fn build_module(fabrickfile: &Fabrickfile, workdir: &Path) -> Result<Vec<u8>> {
    let build = fabrickfile
        .build
        .as_ref()
        .context("Fabrickfile has no [build] section")?;

    // Determine the actual working directory
    let actual_workdir = if let Some(ref build_workdir) = build.workdir {
        workdir.join(build_workdir)
    } else {
        workdir.to_path_buf()
    };

    debug!("Running build command in {}", actual_workdir.display());
    debug!("Command: {}", build.command);

    // Build environment
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(&build.command)
        .current_dir(&actual_workdir);

    // Add build environment variables
    if let Some(ref environment) = build.environment {
        for (key, value) in environment {
            cmd.env(key, value);
        }
    }

    let output = cmd.output().context("Failed to execute build command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "Build command failed with exit code {}\nstdout: {}\nstderr: {}",
            output.status,
            stdout,
            stderr
        );
    }

    info!("Build command completed successfully");

    // Read the WASM output
    let output_path = actual_workdir.join(&build.output);

    if !output_path.exists() {
        bail!(
            "Build output not found: {}\nMake sure the build command creates this file.",
            output_path.display()
        );
    }

    let wasm_bytes = std::fs::read(&output_path).context("Failed to read WASM output file")?;

    debug!(
        "Read {} bytes from {}",
        wasm_bytes.len(),
        output_path.display()
    );
    Ok(wasm_bytes)
}

/// Get or create local storage.
async fn get_or_create_local_storage() -> Result<LocalStorage> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let storage_path = home.join(".fabricks").join("storage");
    LocalStorage::new(storage_path)
        .await
        .context("Failed to initialize local storage")
}

/// Store a module in local storage.
async fn store_module(module: &FabricksModule, tag: &str) -> Result<()> {
    let storage = get_or_create_local_storage().await?;

    // Store config blob
    let config_bytes = module
        .config_bytes()
        .context("Failed to serialize config")?;
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
    let manifest = build_manifest(module, &config_digest, &wasm_digest);
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).context("Failed to serialize manifest")?;
    let manifest_digest = storage
        .store_blob(&manifest_bytes)
        .await
        .context("Failed to store manifest")?;
    debug!("Stored manifest: {manifest_digest}");

    // Add to index
    storage
        .add_to_index(
            tag,
            &manifest_digest,
            i64::try_from(manifest_bytes.len()).unwrap_or(i64::MAX),
        )
        .await
        .context("Failed to update storage index")?;

    Ok(())
}

/// Build a manifest for storage.
fn build_manifest(
    module: &FabricksModule,
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

/// Resolve module from local OCI storage.
async fn resolve_from_storage(reference: &str) -> Result<ModuleSource> {
    let storage = get_local_storage()?;

    // Get manifest digest for this reference
    let manifest_digest = storage
        .get_manifest_digest(reference)
        .await
        .with_context(|| format!("Module not found: {reference}"))?;

    // Load and parse manifest
    let manifest_bytes = storage
        .get_blob(&manifest_digest)
        .await
        .context("Failed to load manifest")?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).context("Failed to parse manifest")?;

    // Extract config digest and load Fabrickfile
    let config_digest = manifest["config"]["digest"]
        .as_str()
        .context("Manifest missing config digest")?;
    let config_bytes = storage
        .get_blob(config_digest)
        .await
        .context("Failed to load config blob")?;
    let config_toml = String::from_utf8(config_bytes).context("Config blob is not valid UTF-8")?;
    let fabrickfile: Fabrickfile =
        toml::from_str(&config_toml).context("Failed to parse Fabrickfile from storage")?;

    // Extract WASM layer digest and load bytes
    let wasm_digest = manifest["layers"]
        .as_array()
        .and_then(|layers| layers.first())
        .and_then(|layer| layer["digest"].as_str())
        .context("Manifest missing WASM layer")?;
    let wasm_bytes = storage
        .get_blob(wasm_digest)
        .await
        .context("Failed to load WASM blob")?;

    Ok(ModuleSource::Storage {
        tag: reference.to_string(),
        fabrickfile,
        wasm_bytes,
    })
}

/// Get the local storage path.
fn get_local_storage() -> Result<LocalStorage> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let storage_path = home.join(".fabricks").join("storage");

    if !storage_path.exists() {
        bail!(
            "No local storage found at {}\n\
             Build a module first with: fabricks build <path>\n\
             Or pull from a registry with: fabricks pull <reference>",
            storage_path.display()
        );
    }

    LocalStorage::open(storage_path).context("Failed to open local storage")
}

/// Run a service from a module reference.
async fn run_service(client: &DaemonClient, reference: &str, format: OutputFormat) -> Result<()> {
    let ModuleSource::Storage {
        tag,
        fabrickfile,
        wasm_bytes,
    } = resolve_module_reference(reference).await?;

    // Write WASM to a temp file and create a temp Fabrickfile
    // (daemon currently expects file paths)
    let wasm_temp = write_temp_wasm(&wasm_bytes)?;
    let fabrickfile_temp = write_temp_fabrickfile(&fabrickfile)?;

    output::writeln(&format!("Running module: {tag}"))?;

    let req = RunFabrickfileRequest {
        fabrickfile_path: fabrickfile_temp.path().to_path_buf(),
        wasm_path: Some(wasm_temp.path().to_path_buf()),
    };

    // Keep temp files alive during the request
    let response = client.run_fabrickfile(req).await?;

    // Temp files are dropped here after daemon has read them
    drop(wasm_temp);
    drop(fabrickfile_temp);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&response)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!(
                "Service '{}' deployed successfully.",
                response.name
            ))?;
            output::writeln(&format!("ID: {}", response.id))?;
        }
    }

    Ok(())
}

/// Write WASM bytes to a temporary file.
fn write_temp_wasm(wasm_bytes: &[u8]) -> Result<NamedTempFile> {
    let mut temp = NamedTempFile::with_suffix(".wasm").context("Failed to create temp file")?;
    temp.write_all(wasm_bytes)
        .context("Failed to write WASM to temp file")?;
    temp.flush().context("Failed to flush temp file")?;
    Ok(temp)
}

/// Write Fabrickfile to a temporary file.
fn write_temp_fabrickfile(fabrickfile: &Fabrickfile) -> Result<NamedTempFile> {
    let mut temp = NamedTempFile::with_suffix(".toml").context("Failed to create temp file")?;
    let toml_content =
        toml::to_string_pretty(fabrickfile).context("Failed to serialize Fabrickfile")?;
    temp.write_all(toml_content.as_bytes())
        .context("Failed to write Fabrickfile to temp file")?;
    temp.flush().context("Failed to flush temp file")?;
    Ok(temp)
}

async fn inspect_service(client: &DaemonClient, id: &str, format: OutputFormat) -> Result<()> {
    let detail = client.get_service(id).await?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&detail)?;
            output::writeln(&json)?;
        }
        OutputFormat::Text => {
            output::writeln(&format!("Service: {}", detail.name))?;
            output::writeln(&format!("  ID:         {}", detail.id))?;
            output::writeln(&format!("  Version:    {}", detail.version))?;
            output::writeln(&format!("  State:      {}", detail.state))?;
            output::writeln(&format!("  Type:       {}", detail.config.service_type))?;
            output::writeln(&format!(
                "  Replicas:   {}/{}",
                detail.replicas.ready, detail.replicas.desired
            ))?;
            output::writeln(&format!("  Created:    {}", detail.created_at))?;
            output::writeln(&format!("  Updated:    {}", detail.updated_at))?;
            output::writeln(&format!("  WASM:       {}", detail.config.wasm_path))?;
            output::writeln(&format!("  Digest:     {}", detail.config.wasm_digest))?;

            if !detail.config.networks.is_empty() {
                output::writeln(&format!(
                    "  Networks:   {}",
                    detail.config.networks.join(", ")
                ))?;
            }

            if let Some(ref project) = detail.config.mortar_project {
                output::writeln(&format!("  Project:    {project}"))?;
            }

            if let Some(ref error) = detail.last_error {
                output::writeln(&format!("  Last Error: {error}"))?;
            }

            if !detail.instances.is_empty() {
                output::writeln("")?;
                output::writeln("Instances:")?;
                for instance in &detail.instances {
                    let started = instance.started_at.as_deref().unwrap_or("N/A");
                    output::writeln(&format!(
                        "  - {} ({}) started: {}",
                        instance.id, instance.state, started
                    ))?;
                }
            }
        }
    }

    Ok(())
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
