//! Build command implementation.
//!
//! Compiles a Fabrickfile into a WASM module and stores it locally.
//!
//! ## Language Builders
//!
//! If the Fabrickfile specifies `[from].source` (e.g., "python", "rust"),
//! the build uses the appropriate language builder automatically:
//!
//! - `source = "rust"` → cargo-component
//! - `source = "python"` → componentize-py
//!
//! ## Base Image Support
//!
//! If the Fabrickfile specifies `[from].image`, the build process will:
//! 1. Pull or load the base image from local cache
//! 2. Compose the base runtime with the user's code
//! 3. Store the result as a multi-layer module
//!
//! ## Custom Build Commands
//!
//! If neither `[from].source` nor `[from].image` is specified, the build
//! uses the `[build].command` for custom build steps.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use fabricks_common::parser::FABRICKFILE_NAME;
use fabricks_common::{Fabrickfile, parse_fabrickfile};
use fabricks_oci::{FabricksModule, LocalStorage, media_types};
use tracing::{debug, info, warn};

use crate::builders::{BuilderConfig, build_with_source};
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

    info!(
        "Building {} v{}",
        fabrickfile.info.name, fabrickfile.info.version
    );

    // Handle base image if specified
    let storage = get_local_storage().await?;
    let base_layer = if let Some(ref from) = fabrickfile.from {
        if let Some(ref image_ref) = from.image {
            info!("Loading base image: {image_ref}");
            Some(load_base_image(&storage, image_ref).await?)
        } else {
            None
        }
    } else {
        None
    };

    // Build the WASM output
    let wasm_bytes = if args.no_build {
        // --no-build: just read the existing output
        read_wasm_output(&fabrickfile, workdir)?
    } else {
        build_wasm(&fabrickfile, workdir)?
    };

    // Create the module, optionally with base layer
    let module = if let Some(runtime_wasm) = base_layer {
        info!("Composing with base runtime layer");
        FabricksModule::new(fabrickfile.clone(), wasm_bytes).with_runtime_layer(runtime_wasm)
    } else {
        FabricksModule::new(fabrickfile.clone(), wasm_bytes)
    };

    // Determine the tag
    let tag = args
        .tag
        .clone()
        .unwrap_or_else(|| format!("{}:{}", fabrickfile.info.name, fabrickfile.info.version));

    // Store locally using the new multi-layer-aware method
    storage.store_module(&module, &tag).await.context("Failed to store module")?;

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

/// Build WASM output using the appropriate method.
///
/// This function chooses between:
/// 1. Language builder (if `[from].source` is specified)
/// 2. Custom build command (if `[build].command` is specified)
fn build_wasm(fabrickfile: &Fabrickfile, workdir: &Path) -> Result<Vec<u8>> {
    // Check if we should use a language builder
    let use_builder = fabrickfile
        .from
        .as_ref()
        .is_some_and(|from| from.source.is_some());

    if use_builder {
        // Use language builder
        info!("Using language builder");
        let config = BuilderConfig {
            fabrickfile,
            workdir,
            release: true, // Always build release for production
        };

        let output = build_with_source(&config)?;

        // Read the built WASM
        std::fs::read(&output.wasm_path).context("Failed to read built WASM file")
    } else {
        // Use custom build command
        run_build_command(fabrickfile, workdir)?;
        read_wasm_output(fabrickfile, workdir)
    }
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

    let wasm_bytes = std::fs::read(&output_path).context("Failed to read WASM output file")?;

    debug!(
        "Read {} bytes from {}",
        wasm_bytes.len(),
        output_path.display()
    );
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

/// Load a base image from local storage.
///
/// Returns the runtime layer bytes if found.
async fn load_base_image(storage: &LocalStorage, image_ref: &str) -> Result<Vec<u8>> {
    debug!("Looking for base image in local storage: {image_ref}");

    // Try to get the runtime layer from local storage
    match storage
        .get_layer_by_media_type(image_ref, media_types::RUNTIME_LAYER_MEDIA_TYPE)
        .await
    {
        Ok(Some(layer)) => {
            debug!("Found runtime layer in local cache ({} bytes)", layer.len());
            return Ok(layer);
        }
        Ok(None) => {
            // No runtime layer - try module layer as fallback
            debug!("No runtime layer, trying module layer as base");
        }
        Err(e) => {
            debug!("Base image not in local cache: {e}");
        }
    }

    // Try the module layer if no runtime layer
    match storage
        .get_layer_by_media_type(image_ref, media_types::WASM_LAYER_MEDIA_TYPE)
        .await
    {
        Ok(Some(layer)) => {
            warn!(
                "Using module layer as runtime (base image '{}' has no dedicated runtime layer)",
                image_ref
            );
            Ok(layer)
        }
        Ok(None) => {
            bail!(
                "Base image '{image_ref}' not found in local storage.\n\
                 Run 'fabricks pull {image_ref}' first, or build the base image locally."
            );
        }
        Err(e) => {
            bail!(
                "Failed to load base image '{image_ref}': {e}\n\
                 Run 'fabricks pull {image_ref}' first."
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabricks_common::Capabilities;
    use fabricks_common::models::fabrickfile::{Build, Info};
    use tempfile::TempDir;

    fn test_fabrickfile() -> Fabrickfile {
        Fabrickfile {
            fabrick_version: "1.0".to_string(),
            info: Info {
                name: "test-module".to_string(),
                version: "1.0.0".to_string(),
                service_type: fabricks_common::models::fabrickfile::ServiceType::default(),
                description: Some("A test module".to_string()),
                authors: None,
                license: None,
                homepage: None,
                repository: None,
                documentation: None,
                keywords: None,
            },
            from: None,
            source: None,
            runtime: None,
            build: Some(Build {
                command: "echo 'test'".to_string(),
                workdir: None,
                output: "output.wasm".to_string(),
                watch: None,
                environment: None,
                pre_build: None,
                post_build: None,
            }),
            exports: None,
            imports: None,
            capabilities: Capabilities::default(),
            files: None,
            config: None,
            health_check: None,
            security: None,
            labels: None,
            validate: None,
        }
    }

    #[test]
    fn test_find_fabrickfile_file_path() {
        let temp = TempDir::new().expect("create temp dir");
        let fabrickfile_path = temp.path().join("Fabrickfile");

        let content = r#"
            fabrick_version = "1.0"
            [info]
            name = "test"
            version = "1.0.0"
        "#;
        std::fs::write(&fabrickfile_path, content).expect("write fabrickfile");

        let (path, fabrickfile) = find_fabrickfile(&fabrickfile_path).expect("find fabrickfile");
        assert_eq!(path, fabrickfile_path);
        assert_eq!(fabrickfile.info.name, "test");
    }

    #[test]
    fn test_find_fabrickfile_directory() {
        let temp = TempDir::new().expect("create temp dir");
        let fabrickfile_path = temp.path().join("Fabrickfile");

        let content = r#"
            fabrick_version = "1.0"
            [info]
            name = "dir-test"
            version = "2.0.0"
        "#;
        std::fs::write(&fabrickfile_path, content).expect("write fabrickfile");

        let (path, fabrickfile) = find_fabrickfile(temp.path()).expect("find fabrickfile");
        assert_eq!(path, fabrickfile_path);
        assert_eq!(fabrickfile.info.name, "dir-test");
    }

    #[test]
    fn test_find_fabrickfile_not_found() {
        let temp = TempDir::new().expect("create temp dir");
        let result = find_fabrickfile(temp.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No Fabrickfile found")
        );
    }

    #[test]
    fn test_find_fabrickfile_path_not_exists() {
        let result = find_fabrickfile(Path::new("/nonexistent/path"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_read_wasm_output_not_found() {
        let temp = TempDir::new().expect("create temp dir");
        let fabrickfile = test_fabrickfile();

        let result = read_wasm_output(&fabrickfile, temp.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Build output not found")
        );
    }

    #[test]
    fn test_read_wasm_output_success() {
        let temp = TempDir::new().expect("create temp dir");
        let mut fabrickfile = test_fabrickfile();
        fabrickfile.build.as_mut().expect("build").output = "test.wasm".to_string();

        let wasm_path = temp.path().join("test.wasm");
        let wasm_content = b"\x00asm\x01\x00\x00\x00";
        std::fs::write(&wasm_path, wasm_content).expect("write wasm");

        let result = read_wasm_output(&fabrickfile, temp.path()).expect("read wasm");
        assert_eq!(result, wasm_content);
    }

    #[test]
    fn test_run_build_command_no_build_section() {
        let temp = TempDir::new().expect("create temp dir");
        let mut fabrickfile = test_fabrickfile();
        fabrickfile.build = None;

        let result = run_build_command(&fabrickfile, temp.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no [build] section")
        );
    }

    #[test]
    fn test_run_build_command_success() {
        let temp = TempDir::new().expect("create temp dir");
        let mut fabrickfile = test_fabrickfile();
        fabrickfile.build.as_mut().expect("build").command = "true".to_string();

        let result = run_build_command(&fabrickfile, temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_build_command_failure() {
        let temp = TempDir::new().expect("create temp dir");
        let mut fabrickfile = test_fabrickfile();
        fabrickfile.build.as_mut().expect("build").command = "false".to_string();

        let result = run_build_command(&fabrickfile, temp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("failed"));
    }
}
