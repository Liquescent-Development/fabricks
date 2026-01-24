//! Build command implementation.
//!
//! Compiles or packages a Fabrickfile into an OCI image and stores it locally.
//!
//! ## Two Build Models
//!
//! Fabricks supports two distinct build models:
//!
//! ### Compiled Languages (Rust, Go, etc.)
//!
//! User code is compiled to WASM:
//! - Uses language builders (cargo-component, tinygo, etc.)
//! - Or custom `[build].command`
//! - Optionally composes with a base runtime via wac-graph
//!
//! ### Interpreted Languages (Python, JavaScript, etc.)
//!
//! User code stays as source files:
//! - Pre-built runtime is pulled from registry
//! - Source files are packaged as a tar layer
//! - No compilation needed
//!
//! ## Build Mode Detection
//!
//! The build mode is determined by the Fabrickfile:
//!
//! 1. `[from].source = "python"` (no `[build]` section) → Interpreted mode
//! 2. `[from].source = "rust"` → Compiled mode with Rust builder
//! 3. `[build].command` specified → Custom build mode
//! 4. `[from].image` specified → Base image composition mode

use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use fabricks_common::models::fabrickfile::SourceLanguage;
use fabricks_common::parser::FABRICKFILE_NAME;
use fabricks_common::{Fabrickfile, parse_fabrickfile};
use fabricks_oci::{FabricksModule, LocalStorage, media_types};
use flate2::Compression;
use flate2::write::GzEncoder;
use tracing::{debug, info, warn};

use crate::builders::{BuilderConfig, build_with_source};
use crate::cli::{BuildArgs, OutputFormat};
use crate::output::writeln_stderr;

/// Interpreted languages that use the source-layer model.
const INTERPRETED_LANGUAGES: &[SourceLanguage] = &[
    SourceLanguage::Python,
    SourceLanguage::Javascript,
];

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

    let storage = get_local_storage().await?;

    // Determine build mode and create module
    let module = if is_interpreted_language(&fabrickfile) {
        build_interpreted(&fabrickfile, workdir, &storage, args).await?
    } else {
        build_compiled(&fabrickfile, workdir, &storage, args).await?
    };

    // Determine the tag
    let tag = args
        .tag
        .clone()
        .unwrap_or_else(|| format!("{}:{}", fabrickfile.info.name, fabrickfile.info.version));

    // Store locally
    storage
        .store_module(&module, &tag)
        .await
        .context("Failed to store module")?;

    // Output result
    output_build_result(&module, &tag, args)?;

    Ok(())
}

/// Check if the Fabrickfile specifies an interpreted language.
fn is_interpreted_language(fabrickfile: &Fabrickfile) -> bool {
    fabrickfile
        .from
        .as_ref()
        .and_then(|from| from.source.as_ref())
        .is_some_and(|lang| INTERPRETED_LANGUAGES.contains(lang))
        && fabrickfile.build.is_none() // No custom build = interpreted mode
}

/// Build for interpreted languages (Python, JavaScript, etc.).
///
/// This packages source files as a layer without compilation.
async fn build_interpreted(
    fabrickfile: &Fabrickfile,
    workdir: &Path,
    storage: &LocalStorage,
    _args: &BuildArgs,
) -> Result<FabricksModule> {
    let from = fabrickfile
        .from
        .as_ref()
        .context("Interpreted build requires [from] section")?;

    let language = from
        .source
        .as_ref()
        .context("Interpreted build requires [from].source")?;

    let version = from.version.as_deref().unwrap_or("latest");

    info!(
        "Building interpreted {:?} application (no compilation)",
        language
    );

    // Determine the runtime image reference
    let runtime_ref = get_runtime_image_ref(*language, version);
    info!("Using runtime: {runtime_ref}");

    // Load the pre-built runtime
    let runtime_wasm = load_runtime_image(storage, &runtime_ref).await?;

    // Package source files
    let source = fabrickfile
        .source
        .as_ref()
        .context("Interpreted build requires [source] section")?;

    let source_path = workdir.join(&source.path);
    info!("Packaging source files from {}", source_path.display());

    let source_tar = package_source_files(&source_path, fabrickfile)?;
    info!("Packaged {} bytes of source files", source_tar.len());

    // Create the interpreted module (runtime + source layers)
    Ok(FabricksModule::new_interpreted(
        fabrickfile.clone(),
        runtime_wasm,
        source_tar,
    ))
}

/// Build for compiled languages (Rust, Go, etc.) or custom builds.
async fn build_compiled(
    fabrickfile: &Fabrickfile,
    workdir: &Path,
    storage: &LocalStorage,
    args: &BuildArgs,
) -> Result<FabricksModule> {
    // Handle base image if specified
    let base_layer = if let Some(ref from) = fabrickfile.from {
        if let Some(ref image_ref) = from.image {
            info!("Loading base image: {image_ref}");
            Some(load_base_image(storage, image_ref).await?)
        } else {
            None
        }
    } else {
        None
    };

    // Build the WASM output
    let wasm_bytes = if args.no_build {
        // --no-build: just read the existing output
        read_wasm_output(fabrickfile, workdir)?
    } else {
        build_wasm(fabrickfile, workdir)?
    };

    // Create the module, optionally with base layer
    if let Some(runtime_wasm) = base_layer {
        info!("Composing with base runtime layer");
        Ok(FabricksModule::new(fabrickfile.clone(), wasm_bytes).with_runtime_layer(runtime_wasm))
    } else {
        Ok(FabricksModule::new(fabrickfile.clone(), wasm_bytes))
    }
}

/// Get the runtime image reference for a language.
fn get_runtime_image_ref(language: SourceLanguage, version: &str) -> String {
    let lang_name = match language {
        SourceLanguage::Python => "python",
        SourceLanguage::Javascript => "javascript",
        SourceLanguage::Rust => "rust",
        SourceLanguage::Go => "go",
        SourceLanguage::Csharp => "dotnet",
    };

    format!("fabricks.dev/runtimes/{lang_name}:{version}")
}

/// Load the pre-built runtime image.
async fn load_runtime_image(storage: &LocalStorage, runtime_ref: &str) -> Result<Vec<u8>> {
    debug!("Looking for runtime image: {runtime_ref}");

    // Try to get the runtime layer from local storage
    match storage
        .get_layer_by_media_type(runtime_ref, media_types::RUNTIME_LAYER_MEDIA_TYPE)
        .await
    {
        Ok(Some(layer)) => {
            debug!("Found runtime in local cache ({} bytes)", layer.len());
            return Ok(layer);
        }
        Ok(None) => {
            debug!("No runtime layer, trying module layer");
        }
        Err(e) => {
            debug!("Runtime not in local cache: {e}");
        }
    }

    // Try the module layer as fallback (some runtimes may be stored this way)
    match storage
        .get_layer_by_media_type(runtime_ref, media_types::WASM_LAYER_MEDIA_TYPE)
        .await
    {
        Ok(Some(layer)) => {
            warn!("Using module layer as runtime for '{runtime_ref}'");
            Ok(layer)
        }
        Ok(None) => {
            bail!(
                "Runtime '{runtime_ref}' not found in local storage.\n\n\
                 To fix this, either:\n\
                 1. Pull the runtime: fabricks pull {runtime_ref}\n\
                 2. Build the runtime locally from examples/runtimes/"
            );
        }
        Err(e) => {
            bail!("Failed to load runtime '{runtime_ref}': {e}");
        }
    }
}

/// Package source files into a gzipped tar archive.
fn package_source_files(source_path: &Path, fabrickfile: &Fabrickfile) -> Result<Vec<u8>> {
    if !source_path.exists() {
        bail!("Source path does not exist: {}", source_path.display());
    }

    // Create tar.gz in memory
    let mut tar_gz_bytes = Vec::new();
    {
        let encoder = GzEncoder::new(&mut tar_gz_bytes, Compression::default());
        let mut tar_builder = tar::Builder::new(encoder);

        // Add all files from source directory
        if source_path.is_dir() {
            add_directory_to_tar(&mut tar_builder, source_path, Path::new(""))?;
        } else {
            // Single file
            tar_builder
                .append_path_with_name(source_path, source_path.file_name().unwrap_or_default())
                .context("Failed to add file to tar")?;
        }

        // Add entrypoint config file (.fabricks.toml)
        let entrypoint_config = create_entrypoint_config(fabrickfile);
        let mut header = tar::Header::new_gnu();
        header.set_size(entrypoint_config.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder
            .append_data(&mut header, ".fabricks.toml", entrypoint_config.as_bytes())
            .context("Failed to add entrypoint config to tar")?;

        // Finish the tar
        let encoder = tar_builder.into_inner().context("Failed to finish tar")?;
        encoder.finish().context("Failed to finish gzip")?;
    }

    Ok(tar_gz_bytes)
}

/// Add a directory recursively to the tar archive.
fn add_directory_to_tar<W: Write>(
    tar_builder: &mut tar::Builder<W>,
    dir_path: &Path,
    prefix: &Path,
) -> Result<()> {
    for entry in std::fs::read_dir(dir_path).context("Failed to read source directory")? {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        let name = prefix.join(entry.file_name());

        // Skip common unwanted files/directories
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        if file_name_str.starts_with('.')
            || file_name_str == "__pycache__"
            || file_name_str == "node_modules"
            || file_name_str == "target"
            || file_name_str == ".git"
            || file_name_str == ".venv"
            || file_name_str.ends_with(".wasm")
        {
            continue;
        }

        if path.is_dir() {
            add_directory_to_tar(tar_builder, &path, &name)?;
        } else {
            tar_builder
                .append_path_with_name(&path, &name)
                .with_context(|| format!("Failed to add {} to tar", path.display()))?;
        }
    }

    Ok(())
}

/// Create the entrypoint configuration file content.
fn create_entrypoint_config(fabrickfile: &Fabrickfile) -> String {
    let entrypoint = fabrickfile
        .source
        .as_ref()
        .and_then(|s| s.entrypoint.as_ref())
        .map_or("app:handler", String::as_str);

    format!("# Generated by fabricks build\nentrypoint = \"{entrypoint}\"\n")
}

/// Build WASM output using the appropriate method.
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
            release: true,
        };

        let output = build_with_source(&config)?;
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

    let actual_workdir = if let Some(ref build_workdir) = build.workdir {
        workdir.join(build_workdir)
    } else {
        workdir.to_path_buf()
    };

    debug!("Running build command in {}", actual_workdir.display());
    debug!("Command: {}", build.command);

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(&build.command)
        .current_dir(&actual_workdir);

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
async fn load_base_image(storage: &LocalStorage, image_ref: &str) -> Result<Vec<u8>> {
    debug!("Looking for base image in local storage: {image_ref}");

    match storage
        .get_layer_by_media_type(image_ref, media_types::RUNTIME_LAYER_MEDIA_TYPE)
        .await
    {
        Ok(Some(layer)) => {
            debug!("Found runtime layer in local cache ({} bytes)", layer.len());
            return Ok(layer);
        }
        Ok(None) => {
            debug!("No runtime layer, trying module layer as base");
        }
        Err(e) => {
            debug!("Base image not in local cache: {e}");
        }
    }

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

/// Output the build result.
fn output_build_result(module: &FabricksModule, tag: &str, args: &BuildArgs) -> Result<()> {
    match args.format {
        OutputFormat::Text => {
            writeln_stderr(&format!("✓ Built {tag}"))?;
            writeln_stderr(&format!("  Layers: {}", module.layer_count()))?;
            if module.has_runtime_layer() {
                writeln_stderr("  Type: Interpreted (runtime + source)")?;
            } else if module.has_source_layers() {
                writeln_stderr("  Type: Source layers only")?;
            } else {
                writeln_stderr("  Type: Compiled WASM")?;
                writeln_stderr(&format!("  WASM size: {} bytes", module.wasm_size()))?;
            }
            writeln_stderr(&format!("  Total size: {} bytes", module.total_size()))?;
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "success": true,
                "tag": tag,
                "name": module.name(),
                "version": module.version(),
                "layer_count": module.layer_count(),
                "has_runtime_layer": module.has_runtime_layer(),
                "has_source_layers": module.has_source_layers(),
                "total_size": module.total_size(),
            });
            writeln_stderr(&serde_json::to_string_pretty(&output)?)?;
        }
    }

    Ok(())
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

    #[test]
    fn test_package_source_files() {
        let temp = TempDir::new().expect("create temp dir");

        // Create test files
        std::fs::write(temp.path().join("app.py"), "def handler(r): pass").expect("write");
        std::fs::write(temp.path().join("utils.py"), "# utils").expect("write");

        let fabrickfile = test_fabrickfile();
        let tar_bytes = package_source_files(temp.path(), &fabrickfile).expect("package");

        // Verify it's valid gzip
        assert!(tar_bytes.len() > 0);
        assert_eq!(tar_bytes[0], 0x1f); // gzip magic
        assert_eq!(tar_bytes[1], 0x8b);
    }

    #[test]
    fn test_create_entrypoint_config() {
        let fabrickfile = test_fabrickfile();
        let config = create_entrypoint_config(&fabrickfile);

        assert!(config.contains("entrypoint"));
        assert!(config.contains("app:handler")); // default
    }

    #[test]
    fn test_is_interpreted_language() {
        use fabricks_common::models::fabrickfile::{From, Source};

        let mut fabrickfile = test_fabrickfile();

        // No from section = not interpreted
        assert!(!is_interpreted_language(&fabrickfile));

        // Python with no build = interpreted
        fabrickfile.from = Some(From {
            source: Some(SourceLanguage::Python),
            image: None,
            version: Some("3.12".to_string()),
            path: None,
        });
        fabrickfile.source = Some(Source {
            path: ".".to_string(),
            entrypoint: Some("app:handler".to_string()),
            include: None,
            exclude: None,
        });
        fabrickfile.build = None;
        assert!(is_interpreted_language(&fabrickfile));

        // Rust = not interpreted (even without build)
        fabrickfile.from = Some(From {
            source: Some(SourceLanguage::Rust),
            image: None,
            version: None,
            path: None,
        });
        assert!(!is_interpreted_language(&fabrickfile));
    }
}
