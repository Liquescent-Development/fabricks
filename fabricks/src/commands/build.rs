//! Build command implementation.
//!
//! Compiles a Fabrickfile into a WASM module and stores it locally.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use fabricks_common::parser::FABRICKFILE_NAME;
use fabricks_common::{parse_fabrickfile, Fabrickfile};
use fabricks_oci::{FabricksModule, LocalStorage};
use tracing::{debug, info};

use crate::cli::{BuildArgs, OutputFormat};
use crate::output::writeln_stderr;

/// Run the build command.
///
/// # Errors
///
/// Returns an error if:
/// - The Fabrickfile cannot be found or parsed
/// - The build command fails
/// - The WASM output cannot be found
/// - Storage operations fail
pub async fn run(args: &BuildArgs) -> Result<()> {
    // Find and parse Fabrickfile
    let (fabrickfile_path, fabrickfile) = find_fabrickfile(&args.path)?;
    let workdir = fabrickfile_path
        .parent()
        .context("Fabrickfile has no parent directory")?;

    info!("Building {} v{}", fabrickfile.info.name, fabrickfile.info.version);

    // Run build command unless --no-build is specified
    if !args.no_build {
        run_build_command(&fabrickfile, workdir)?;
    }

    // Read the WASM output
    let wasm_bytes = read_wasm_output(&fabrickfile, workdir)?;

    // Create the module
    let module = FabricksModule::new(fabrickfile.clone(), wasm_bytes);

    // Determine the tag
    let tag = args.tag.clone().unwrap_or_else(|| {
        format!("{}:{}", fabrickfile.info.name, fabrickfile.info.version)
    });

    // Store locally
    let storage = get_local_storage().await?;
    store_module(&storage, &module, &tag).await?;

    // Output result
    match args.format {
        OutputFormat::Text => {
            writeln_stderr(&format!("✓ Built {tag}"))?;
            writeln_stderr(&format!("  WASM size: {} bytes", module.wasm_size()))?;
            writeln_stderr(&format!("  Digest: {}", module.wasm_digest()))?;
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "success": true,
                "tag": tag,
                "name": module.name(),
                "version": module.version(),
                "wasm_size": module.wasm_size(),
                "wasm_digest": module.wasm_digest(),
            });
            writeln_stderr(&serde_json::to_string_pretty(&output)?)?;
        }
    }

    Ok(())
}

/// Find the Fabrickfile, returning its path and parsed content.
fn find_fabrickfile(path: &Path) -> Result<(std::path::PathBuf, Fabrickfile)> {
    let fabrickfile_path = if path.is_file() {
        path.to_path_buf()
    } else if path.is_dir() {
        let file_path = path.join(FABRICKFILE_NAME);
        if !file_path.exists() {
            bail!("No Fabrickfile found in {}", path.display());
        }
        file_path
    } else {
        bail!("Path does not exist: {}", path.display());
    };

    let fabrickfile = parse_fabrickfile(&fabrickfile_path)?;
    Ok((fabrickfile_path, fabrickfile))
}

/// Run the build command from the Fabrickfile.
fn run_build_command(fabrickfile: &Fabrickfile, workdir: &Path) -> Result<()> {
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
    cmd.arg("-c").arg(&build.command).current_dir(&actual_workdir);

    // Add build environment variables
    if let Some(ref environment) = build.environment {
        for (key, value) in environment {
            cmd.env(key, value);
        }
    }

    let output = cmd
        .output()
        .context("Failed to execute build command")?;

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
    Ok(())
}

/// Read the WASM output file.
fn read_wasm_output(fabrickfile: &Fabrickfile, workdir: &Path) -> Result<Vec<u8>> {
    let build = fabrickfile
        .build
        .as_ref()
        .context("Fabrickfile has no [build] section")?;

    // Determine the actual working directory for output
    let actual_workdir = if let Some(ref build_workdir) = build.workdir {
        workdir.join(build_workdir)
    } else {
        workdir.to_path_buf()
    };

    let output_path = actual_workdir.join(&build.output);

    if !output_path.exists() {
        bail!(
            "Build output not found: {}\nMake sure the build command creates this file.",
            output_path.display()
        );
    }

    let wasm_bytes =
        std::fs::read(&output_path).context("Failed to read WASM output file")?;

    debug!("Read {} bytes from {}", wasm_bytes.len(), output_path.display());
    Ok(wasm_bytes)
}

/// Get the default local storage location.
async fn get_local_storage() -> Result<LocalStorage> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let storage_path = home.join(".fabricks").join("storage");
    LocalStorage::new(storage_path)
        .await
        .context("Failed to initialize local storage")
}

/// Store a module in local storage.
async fn store_module(storage: &LocalStorage, module: &FabricksModule, tag: &str) -> Result<()> {
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
