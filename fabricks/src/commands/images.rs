//! Images command implementation.
//!
//! Lists all locally stored modules in OCI format.

use anyhow::{Context, Result};
use fabricks_oci::LocalStorage;

use crate::cli::{ImagesArgs, OutputFormat};
use crate::output;

/// Information about a stored image.
#[derive(Debug, serde::Serialize)]
struct ImageInfo {
    /// Image reference (e.g., "hello-http:0.1.0").
    reference: String,
    /// Manifest digest.
    digest: String,
    /// WASM size in bytes.
    wasm_size: u64,
    /// Module name from config.
    name: Option<String>,
    /// Module version from config.
    version: Option<String>,
}

/// Run the images command.
///
/// # Errors
///
/// Returns an error if storage operations fail.
pub async fn run(args: &ImagesArgs) -> Result<()> {
    let storage = get_local_storage()?;
    let refs = storage.list_references().await.context("Failed to list references")?;

    if refs.is_empty() {
        match args.format {
            OutputFormat::Text => {
                output::writeln("No images found.")?;
                output::writeln("")?;
                output::writeln("Build an image with: fabricks build <path>")?;
            }
            OutputFormat::Json => {
                output::writeln("[]")?;
            }
        }
        return Ok(());
    }

    let mut images = Vec::new();

    for reference in &refs {
        if let Ok(info) = get_image_info(&storage, reference).await {
            images.push(info);
        }
    }

    match args.format {
        OutputFormat::Text => {
            // Print header
            output::writeln(&format!(
                "{:<30} {:<12} {:<10} {:<64}",
                "REFERENCE", "SIZE", "VERSION", "DIGEST"
            ))?;
            output::writeln(&"-".repeat(116))?;

            for image in &images {
                let size = format_size(image.wasm_size);
                let version = image.version.as_deref().unwrap_or("-");
                let short_digest = if image.digest.len() > 19 {
                    &image.digest[..19]
                } else {
                    &image.digest
                };

                output::writeln(&format!(
                    "{:<30} {:<12} {:<10} {}...",
                    image.reference, size, version, short_digest
                ))?;
            }

            output::writeln("")?;
            output::writeln(&format!("Total: {} image(s)", images.len()))?;
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&images)?;
            output::writeln(&json)?;
        }
    }

    Ok(())
}

/// Get image info from storage.
async fn get_image_info(storage: &LocalStorage, reference: &str) -> Result<ImageInfo> {
    let manifest_digest = storage.get_manifest_digest(reference).await?;
    let manifest_bytes = storage.get_blob(&manifest_digest).await?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;

    // Extract WASM size from layers
    let wasm_size = manifest["layers"]
        .as_array()
        .and_then(|layers| layers.first())
        .and_then(|layer| layer["size"].as_u64())
        .unwrap_or(0);

    // Extract name and version from annotations
    let annotations = manifest["annotations"].as_object();
    let name = annotations
        .and_then(|a| a.get("dev.fabricks.name"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let version = annotations
        .and_then(|a| a.get("dev.fabricks.module.version"))
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(ImageInfo {
        reference: reference.to_string(),
        digest: manifest_digest,
        wasm_size,
        name,
        version,
    })
}

/// Get the default local storage location.
fn get_local_storage() -> Result<LocalStorage> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let storage_path = home.join(".fabricks").join("storage");

    // Check if storage exists
    if !storage_path.exists() {
        anyhow::bail!(
            "No local storage found at {}\n\
             Build an image first with: fabricks build <path>",
            storage_path.display()
        );
    }

    LocalStorage::open(storage_path).context("Failed to open local storage")
}

/// Format size in human-readable format.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        let whole = bytes / GB;
        let frac = (bytes % GB) * 10 / GB;
        format!("{whole}.{frac} GB")
    } else if bytes >= MB {
        let whole = bytes / MB;
        let frac = (bytes % MB) * 10 / MB;
        format!("{whole}.{frac} MB")
    } else if bytes >= KB {
        let whole = bytes / KB;
        let frac = (bytes % KB) * 10 / KB;
        format!("{whole}.{frac} KB")
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1572864), "1.5 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
    }
}
