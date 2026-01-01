//! Inspect command implementation.
//!
//! Displays metadata and capabilities of a WASM module.

use std::path::Path;

use anyhow::{bail, Context, Result};
use fabricks_common::Fabrickfile;
use fabricks_oci::LocalStorage;
use tracing::debug;

use crate::cli::{InspectArgs, OutputFormat};
use crate::output::writeln_stderr;

/// Run the inspect command.
///
/// # Errors
///
/// Returns an error if the module cannot be found or loaded.
pub fn run(args: &InspectArgs) -> Result<()> {
    // Load the module synchronously (no async needed for just reading metadata)
    let (fabrickfile, wasm_size) = load_module_sync(&args.module)?;

    match args.format {
        OutputFormat::Text => output_text(&fabrickfile, wasm_size),
        OutputFormat::Json => output_json(&fabrickfile, wasm_size),
    }
}

/// Load module metadata synchronously.
fn load_module_sync(reference: &str) -> Result<(Fabrickfile, usize)> {
    // Check if it's a local file path
    let path = Path::new(reference);
    if path.exists() && path.is_file() {
        return load_from_file_sync(path);
    }

    // Check if it looks like a registry reference
    if reference.contains('/') {
        bail!(
            "Registry references are not yet supported for `inspect`.\n\
             Use `fabricks pull {reference}` first, then inspect the local tag."
        );
    }

    // Otherwise, treat as a local tag
    load_from_storage_sync(reference)
}

/// Load module from a file.
fn load_from_file_sync(path: &Path) -> Result<(Fabrickfile, usize)> {
    debug!("Inspecting file: {}", path.display());

    let wasm_bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read WASM file: {}", path.display()))?;

    let wasm_size = wasm_bytes.len();

    // Check for a Fabrickfile in the same directory
    let parent = path.parent().unwrap_or(Path::new("."));
    let fabrickfile_path = parent.join("Fabrickfile");

    let fabrickfile = if fabrickfile_path.exists() {
        fabricks_common::parse_fabrickfile(&fabrickfile_path)?
    } else {
        // Create a minimal fabrickfile from the filename
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        create_minimal_fabrickfile(name)
    };

    Ok((fabrickfile, wasm_size))
}

/// Load module from local storage.
fn load_from_storage_sync(tag: &str) -> Result<(Fabrickfile, usize)> {
    debug!("Inspecting local tag: {tag}");

    let home = dirs::home_dir().context("Could not determine home directory")?;
    let storage_path = home.join(".fabricks").join("storage");

    if !storage_path.exists() {
        bail!(
            "Module not found: {tag}\n\
             Local storage not initialized. Run `fabricks build` or `fabricks pull` first."
        );
    }

    let storage = LocalStorage::open(&storage_path).context("Failed to open local storage")?;

    // Use a runtime to run async operations
    let rt = tokio::runtime::Runtime::new().context("Failed to create runtime")?;

    rt.block_on(async {
        // Get manifest digest
        let manifest_digest = storage
            .get_manifest_digest(tag)
            .await
            .with_context(|| format!("Module not found: {tag}"))?;

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

        // Get WASM size from manifest
        let layers = manifest["layers"]
            .as_array()
            .context("Manifest missing layers")?;

        let wasm_size = layers
            .first()
            .and_then(|l| l["size"].as_u64())
            .and_then(|s| usize::try_from(s).ok())
            .unwrap_or(0);

        Ok((fabrickfile, wasm_size))
    })
}

/// Output module info in text format.
fn output_text(fabrickfile: &Fabrickfile, wasm_size: usize) -> Result<()> {
    writeln_stderr(&format!("Name: {}", fabrickfile.info.name))?;
    writeln_stderr(&format!("Version: {}", fabrickfile.info.version))?;

    if let Some(ref desc) = fabrickfile.info.description {
        writeln_stderr(&format!("Description: {desc}"))?;
    }

    if let Some(ref authors) = fabrickfile.info.authors {
        writeln_stderr(&format!("Authors: {}", authors.join(", ")))?;
    }

    if let Some(ref license) = fabrickfile.info.license {
        writeln_stderr(&format!("License: {license}"))?;
    }

    writeln_stderr(&format!("WASM size: {wasm_size} bytes"))?;
    writeln_stderr("")?;

    // Output capabilities
    writeln_stderr("Capabilities:")?;
    output_capabilities(fabrickfile)?;

    Ok(())
}

/// Output capabilities in text format.
fn output_capabilities(fabrickfile: &Fabrickfile) -> Result<()> {
    let caps = &fabrickfile.capabilities;

    // Environment variables
    if let Some(ref env) = caps.env {
        writeln_stderr(&format!("  Environment: {env:?}"))?;
    } else {
        writeln_stderr("  Environment: (none)")?;
    }

    // Filesystem
    if let Some(ref fs) = caps.filesystem {
        writeln_stderr("  Filesystem:")?;
        if let Some(ref read) = fs.read {
            writeln_stderr(&format!("    Read: {read:?}"))?;
        }
        if let Some(ref write) = fs.write {
            writeln_stderr(&format!("    Write: {write:?}"))?;
        }
        if let Some(ref rw) = fs.read_write {
            writeln_stderr(&format!("    Read/Write: {rw:?}"))?;
        }
    } else {
        writeln_stderr("  Filesystem: (none)")?;
    }

    // Network
    if let Some(ref net) = caps.network {
        writeln_stderr("  Network:")?;
        if let Some(ref connect) = net.connect {
            writeln_stderr(&format!("    Connect: {connect:?}"))?;
        }
        if let Some(ref listen) = net.listen {
            writeln_stderr(&format!("    Listen: {listen:?}"))?;
        }
    } else {
        writeln_stderr("  Network: (none)")?;
    }

    Ok(())
}

/// Output module info in JSON format.
fn output_json(fabrickfile: &Fabrickfile, wasm_size: usize) -> Result<()> {
    let output = serde_json::json!({
        "name": fabrickfile.info.name,
        "version": fabrickfile.info.version,
        "description": fabrickfile.info.description,
        "authors": fabrickfile.info.authors,
        "license": fabrickfile.info.license,
        "wasm_size": wasm_size,
        "capabilities": {
            "env": fabrickfile.capabilities.env,
            "filesystem": fabrickfile.capabilities.filesystem,
            "network": fabrickfile.capabilities.network,
        },
    });

    writeln_stderr(&serde_json::to_string_pretty(&output)?)?;
    Ok(())
}

/// Create a minimal Fabrickfile for a standalone WASM file.
fn create_minimal_fabrickfile(name: &str) -> Fabrickfile {
    Fabrickfile {
        fabrick_version: "1.0".to_string(),
        info: fabricks_common::models::fabrickfile::Info {
            name: name.to_string(),
            version: "0.0.0".to_string(),
            description: None,
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
        build: None,
        exports: None,
        imports: None,
        capabilities: fabricks_common::Capabilities::default(),
        files: None,
        config: None,
        health_check: None,
        security: None,
        labels: None,
        validate: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabricks_common::models::capability::{FilesystemCapabilities, NetworkCapabilities};
    use fabricks_common::Capabilities;
    use tempfile::TempDir;

    fn test_fabrickfile() -> Fabrickfile {
        Fabrickfile {
            fabrick_version: "1.0".to_string(),
            info: fabricks_common::models::fabrickfile::Info {
                name: "inspect-test".to_string(),
                version: "2.0.0".to_string(),
                description: Some("Test module for inspect".to_string()),
                authors: Some(vec!["Test Author".to_string()]),
                license: Some("MIT".to_string()),
                homepage: None,
                repository: None,
                documentation: None,
                keywords: None,
            },
            from: None,
            source: None,
            runtime: None,
            build: None,
            exports: None,
            imports: None,
            capabilities: Capabilities {
                env: Some(vec!["HOME".to_string(), "PATH".to_string()]),
                filesystem: Some(FilesystemCapabilities {
                    read: Some(vec!["/tmp".to_string()]),
                    write: None,
                    read_write: None,
                }),
                network: Some(NetworkCapabilities {
                    connect: Some(vec!["api.example.com:443".to_string()]),
                    listen: Some(vec![8080]),
                    allow_all_outbound: None,
                }),
                wasm: None,
            },
            files: None,
            config: None,
            health_check: None,
            security: None,
            labels: None,
            validate: None,
        }
    }

    #[test]
    fn test_create_minimal_fabrickfile() {
        let fabrickfile = create_minimal_fabrickfile("my-module");

        assert_eq!(fabrickfile.info.name, "my-module");
        assert_eq!(fabrickfile.info.version, "0.0.0");
    }

    #[test]
    fn test_load_from_file_sync() {
        let temp = TempDir::new().expect("create temp dir");
        let wasm_content = b"\x00asm\x01\x00\x00\x00";
        let wasm_path = temp.path().join("test.wasm");
        std::fs::write(&wasm_path, wasm_content).expect("write wasm");

        let (fabrickfile, wasm_size) = load_from_file_sync(&wasm_path).expect("load file");

        assert_eq!(fabrickfile.info.name, "test");
        assert_eq!(wasm_size, 8);
    }

    #[test]
    fn test_load_module_sync_file() {
        let temp = TempDir::new().expect("create temp dir");
        let wasm_content = b"\x00asm\x01\x00\x00\x00";
        let wasm_path = temp.path().join("module.wasm");
        std::fs::write(&wasm_path, wasm_content).expect("write wasm");

        let path_str = wasm_path.to_str().expect("path to str");
        let (fabrickfile, wasm_size) = load_module_sync(path_str).expect("load module");

        assert_eq!(fabrickfile.info.name, "module");
        assert_eq!(wasm_size, 8);
    }

    #[test]
    fn test_load_module_sync_registry_ref() {
        let result = load_module_sync("ghcr.io/user/module:latest");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet supported"));
    }

    #[test]
    fn test_output_json() {
        let fabrickfile = test_fabrickfile();
        // Just verify it doesn't panic
        let result = output_json(&fabrickfile, 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn test_output_text() {
        let fabrickfile = test_fabrickfile();
        // Just verify it doesn't panic
        let result = output_text(&fabrickfile, 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn test_output_capabilities() {
        let fabrickfile = test_fabrickfile();
        // Just verify it doesn't panic
        let result = output_capabilities(&fabrickfile);
        assert!(result.is_ok());
    }

    #[test]
    fn test_output_capabilities_empty() {
        let mut fabrickfile = test_fabrickfile();
        fabrickfile.capabilities = Capabilities::default();

        let result = output_capabilities(&fabrickfile);
        assert!(result.is_ok());
    }
}
