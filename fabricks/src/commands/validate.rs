//! Validate command implementation.
//!
//! Validates Fabrickfile or fabricks-mortar.toml configuration files.

use std::path::Path;

use anyhow::{Context, Result};
use fabricks_common::parser::{FABRICKFILE_NAME, MORTAR_FILE_NAME};
use fabricks_common::{parse_fabrickfile, parse_mortar_file};

use crate::cli::{FileType, OutputFormat, ValidateArgs};
use crate::output::writeln_stderr;

/// Run the validate command.
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be read
/// - The TOML is malformed
/// - Validation fails
pub fn run(args: &ValidateArgs) -> Result<()> {
    let path = &args.path;

    // Determine file type
    let file_type = args
        .file_type
        .or_else(|| detect_file_type(path))
        .context("Could not determine file type. Use --type to specify.")?;

    match file_type {
        FileType::Fabrickfile => validate_fabrickfile(path, args.format),
        FileType::Mortar => validate_mortar(path, args.format),
    }
}

/// Detect file type based on path.
fn detect_file_type(path: &Path) -> Option<FileType> {
    if path.is_file() {
        let filename = path.file_name()?.to_str()?;
        if filename == FABRICKFILE_NAME || filename.ends_with(".fabrickfile") {
            return Some(FileType::Fabrickfile);
        }
        if filename == MORTAR_FILE_NAME || filename.ends_with("-mortar.toml") {
            return Some(FileType::Mortar);
        }
        None
    } else if path.is_dir() {
        // Check which file exists in the directory
        let fabrickfile = path.join(FABRICKFILE_NAME);
        let mortar = path.join(MORTAR_FILE_NAME);

        if fabrickfile.exists() && mortar.exists() {
            // Both exist, prefer mortar (project-level)
            Some(FileType::Mortar)
        } else if mortar.exists() {
            Some(FileType::Mortar)
        } else if fabrickfile.exists() {
            Some(FileType::Fabrickfile)
        } else {
            None
        }
    } else {
        None
    }
}

/// Validate a Fabrickfile.
fn validate_fabrickfile(path: &Path, format: OutputFormat) -> Result<()> {
    let fabrickfile = parse_fabrickfile(path)?;

    match format {
        OutputFormat::Text => {
            writeln_stderr(&format!(
                "✓ Valid Fabrickfile: {} v{}",
                fabrickfile.info.name, fabrickfile.info.version
            ))?;
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "valid": true,
                "type": "fabrickfile",
                "name": fabrickfile.info.name,
                "version": fabrickfile.info.version,
            });
            writeln_stderr(&serde_json::to_string_pretty(&output)?)?;
        }
    }

    Ok(())
}

/// Validate a mortar file.
fn validate_mortar(path: &Path, format: OutputFormat) -> Result<()> {
    let mortar = parse_mortar_file(path)?;

    let service_count = mortar.service.len();
    let network_count = mortar
        .network
        .as_ref()
        .map_or(0, std::collections::HashMap::len);
    let volume_count = mortar
        .volume
        .as_ref()
        .map_or(0, std::collections::HashMap::len);

    match format {
        OutputFormat::Text => {
            writeln_stderr(&format!("✓ Valid mortar file: {}", mortar.project.name))?;
            writeln_stderr(&format!(
                "  Services: {service_count}, Networks: {network_count}, Volumes: {volume_count}"
            ))?;
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "valid": true,
                "type": "mortar",
                "name": mortar.project.name,
                "version": mortar.project.version,
                "services": service_count,
                "networks": network_count,
                "volumes": volume_count,
            });
            writeln_stderr(&serde_json::to_string_pretty(&output)?)?;
        }
    }

    Ok(())
}
