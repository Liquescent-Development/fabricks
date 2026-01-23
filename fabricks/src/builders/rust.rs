//! Rust builder using cargo-component.
//!
//! Builds Rust projects to WebAssembly components using `cargo component build`.
//! This is the default builder for Rust projects.

use std::process::Command;

use anyhow::{Context, Result, bail};
use tracing::{debug, info};

use super::{BuildOutput, Builder, BuilderConfig};

/// Builder for Rust projects.
pub struct RustBuilder;

impl Builder for RustBuilder {
    fn check_toolchain(&self) -> Result<()> {
        // Check for cargo
        let cargo_output = Command::new("cargo").arg("--version").output();

        if cargo_output.is_err() {
            bail!(
                "Rust toolchain not found.\n\n\
                 Install Rust: https://rustup.rs/\n\
                 Then install cargo-component: cargo install cargo-component"
            );
        }

        // Check for cargo-component
        let component_output = Command::new("cargo")
            .args(["component", "--version"])
            .output();

        match component_output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                debug!("Found cargo-component: {}", version.trim());
                Ok(())
            }
            _ => {
                bail!(
                    "cargo-component not found.\n\n\
                     Install it with: cargo install cargo-component\n\
                     Documentation: https://github.com/bytecodealliance/cargo-component"
                );
            }
        }
    }

    fn build(&self, config: &BuilderConfig<'_>) -> Result<BuildOutput> {
        info!("Building Rust project with cargo-component");

        // Determine source directory
        let source_path = if let Some(ref source) = config.fabrickfile.source {
            config.workdir.join(&source.path)
        } else {
            config.workdir.to_path_buf()
        };

        // Build arguments
        let mut args = vec!["component", "build"];
        if config.release {
            args.push("--release");
        }

        debug!("Running: cargo {} in {}", args.join(" "), source_path.display());

        let output = Command::new("cargo")
            .args(&args)
            .current_dir(&source_path)
            .output()
            .context("Failed to execute cargo component build")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            bail!(
                "Rust build failed:\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}"
            );
        }

        // Find the output WASM file
        let wasm_path = find_rust_wasm_output(&source_path, config.release)?;
        let wasm_size = std::fs::metadata(&wasm_path)
            .context("Failed to get WASM file size")?
            .len();

        info!("Built {} ({wasm_size} bytes)", wasm_path.display());

        Ok(BuildOutput { wasm_path })
    }
}

/// Find the WASM output from a cargo-component build.
fn find_rust_wasm_output(source_path: &std::path::Path, release: bool) -> Result<std::path::PathBuf> {
    let profile = if release { "release" } else { "debug" };
    let target_dir = source_path.join("target").join("wasm32-wasip1").join(profile);

    // Look for .wasm files in the target directory
    if target_dir.exists() {
        for entry in std::fs::read_dir(&target_dir).context("Failed to read target directory")? {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "wasm") {
                return Ok(path);
            }
        }
    }

    // Also check wasm32-wasi target (older naming)
    let alt_target_dir = source_path.join("target").join("wasm32-wasi").join(profile);
    if alt_target_dir.exists() {
        for entry in std::fs::read_dir(&alt_target_dir).context("Failed to read target directory")? {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "wasm") {
                return Ok(path);
            }
        }
    }

    bail!(
        "Could not find WASM output in {}\n\
         Make sure the project is configured to build a WASM component.",
        target_dir.display()
    )
}
