//! Go builder using `TinyGo`.
//!
//! Builds Go projects to WebAssembly components using `tinygo build`.
//! Requires:
//! - `TinyGo` 0.32+ for WASI Preview 2 support
//! - `wasm-tools` CLI for component model processing
//!
//! The build process uses `-target=wasip2` which produces WASI Preview 2
//! components directly, supporting both command and HTTP service types.

use std::process::Command;

use anyhow::{Context, Result, bail};
use tracing::{debug, info};

use super::{BuildOutput, Builder, BuilderConfig};

/// Minimum required `TinyGo` version.
const MIN_TINYGO_VERSION: (u32, u32) = (0, 32);

/// Builder for Go projects using `TinyGo`.
pub struct GoBuilder;

impl Builder for GoBuilder {
    fn check_toolchain(&self) -> Result<()> {
        // Check TinyGo
        let output = Command::new("tinygo").arg("version").output();

        match output {
            Ok(output) if output.status.success() => {
                let version_str = String::from_utf8_lossy(&output.stdout);
                debug!("Found TinyGo: {}", version_str.trim());

                // Parse version (format: "tinygo version 0.32.0 ...")
                if let Some(version) = parse_tinygo_version(&version_str) {
                    if version < MIN_TINYGO_VERSION {
                        bail!(
                            "TinyGo version {}.{} found, but {}.{} or later is required.\n\n\
                             Update TinyGo: https://tinygo.org/getting-started/install/",
                            version.0,
                            version.1,
                            MIN_TINYGO_VERSION.0,
                            MIN_TINYGO_VERSION.1
                        );
                    }
                } else {
                    // Couldn't parse version, assume it's fine
                    debug!("Could not parse TinyGo version, proceeding anyway");
                }
            }
            _ => {
                bail!(
                    "TinyGo not found.\n\n\
                     Go builder requires TinyGo 0.32+.\n\
                     Install: https://tinygo.org/getting-started/install/"
                );
            }
        }

        // Check wasm-tools
        let output = Command::new("wasm-tools").arg("--version").output();

        match output {
            Ok(output) if output.status.success() => {
                let version_str = String::from_utf8_lossy(&output.stdout);
                debug!("Found wasm-tools: {}", version_str.trim());
                Ok(())
            }
            _ => {
                bail!(
                    "wasm-tools not found.\n\n\
                     Go builder requires wasm-tools for WASI Preview 2 component generation.\n\
                     Install: cargo install wasm-tools"
                );
            }
        }
    }

    fn build(&self, config: &BuilderConfig<'_>) -> Result<BuildOutput> {
        info!("Building Go project with TinyGo");

        // Determine source directory
        let source_path = if let Some(ref source) = config.fabrickfile.source {
            let path = &source.path;
            if path == "." {
                config.workdir.to_path_buf()
            } else {
                config.workdir.join(path)
            }
        } else {
            config.workdir.to_path_buf()
        };

        // Determine output filename
        let output_name = &config.fabrickfile.info.name;
        let wasm_filename = format!("{output_name}.wasm");

        // Full path for file operations
        let wasm_path = source_path.join(&wasm_filename);

        // Build with TinyGo using wasip2 target
        // This produces a WASI Preview 2 component directly
        let mut args = vec!["build", "-target=wasip2", "-o", &wasm_filename];

        // Add optimization for release builds
        if config.release {
            args.push("-opt=2");
        }

        // Source directory (current dir)
        args.push(".");

        debug!(
            "Running: tinygo {} in {}",
            args.join(" "),
            source_path.display()
        );

        let output = Command::new("tinygo")
            .args(&args)
            .current_dir(&source_path)
            .output()
            .context("Failed to execute tinygo build")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            bail!("Go build failed:\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}");
        }

        // Verify the output exists
        if !wasm_path.exists() {
            bail!(
                "TinyGo build succeeded but WASM output not found at: {}",
                wasm_path.display()
            );
        }

        let wasm_size = std::fs::metadata(&wasm_path)
            .context("Failed to get WASM file size")?
            .len();

        info!("Built component {} ({wasm_size} bytes)", wasm_path.display());

        Ok(BuildOutput { wasm_path })
    }
}

/// Parse `TinyGo` version from version string.
///
/// Expected format: "tinygo version 0.32.0 linux/amd64 ..."
fn parse_tinygo_version(version_str: &str) -> Option<(u32, u32)> {
    // Find "version X.Y.Z" pattern
    let parts: Vec<&str> = version_str.split_whitespace().collect();
    let version_idx = parts.iter().position(|&s| s == "version")?;
    let version_part = parts.get(version_idx + 1)?;

    // Parse X.Y from X.Y.Z
    let version_nums: Vec<&str> = version_part.split('.').collect();
    let major = version_nums.first()?.parse().ok()?;
    let minor = version_nums.get(1)?.parse().ok()?;

    Some((major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tinygo_version() {
        assert_eq!(
            parse_tinygo_version("tinygo version 0.32.0 linux/amd64"),
            Some((0, 32))
        );
        assert_eq!(
            parse_tinygo_version("tinygo version 0.33.1 darwin/arm64"),
            Some((0, 33))
        );
        assert_eq!(
            parse_tinygo_version("tinygo version 1.0.0"),
            Some((1, 0))
        );
        assert_eq!(parse_tinygo_version("invalid"), None);
    }
}
