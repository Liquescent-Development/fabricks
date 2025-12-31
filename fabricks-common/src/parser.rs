//! File parsers for Fabrickfile and `MortarFile`.
//!
//! This module provides functions to parse configuration files from the filesystem.
//! Each parser reads the file, deserializes the TOML content, and validates the result.
//!
//! # Example
//!
//! ```no_run
//! use fabricks_common::parser::{parse_fabrickfile, parse_mortar_file};
//! use std::path::Path;
//!
//! // Parse a Fabrickfile
//! let fabrickfile = parse_fabrickfile(Path::new("./Fabrickfile"))?;
//!
//! // Parse a mortar file
//! let mortar = parse_mortar_file(Path::new("./fabricks-mortar.toml"))?;
//! # Ok::<(), fabricks_common::ParseError>(())
//! ```

use std::fs;
use std::path::Path;

use crate::error::ParseError;
use crate::models::{Fabrickfile, MortarFile};
use crate::validation::Validate;

/// The default filename for a Fabrickfile.
pub const FABRICKFILE_NAME: &str = "Fabrickfile";

/// The default filename for a mortar file.
pub const MORTAR_FILE_NAME: &str = "fabricks-mortar.toml";

/// Parse a Fabrickfile from a file path.
///
/// This function reads the file, parses the TOML content, and validates the result.
/// The path can point either to the Fabrickfile directly or to a directory containing
/// a file named "Fabrickfile".
///
/// # Arguments
///
/// * `path` - Path to the Fabrickfile or directory containing it
///
/// # Returns
///
/// The parsed and validated [`Fabrickfile`], or an error if parsing/validation fails.
///
/// # Errors
///
/// Returns [`ParseError::IoError`] if the file cannot be read.
/// Returns [`ParseError::TomlError`] if the TOML is malformed.
/// Returns [`ParseError::ValidationError`] if validation fails.
///
/// # Example
///
/// ```no_run
/// use fabricks_common::parser::parse_fabrickfile;
/// use std::path::Path;
///
/// let fabrickfile = parse_fabrickfile(Path::new("./Fabrickfile"))?;
/// println!("Parsed fabrick: {}", fabrickfile.info.name);
/// # Ok::<(), fabricks_common::ParseError>(())
/// ```
pub fn parse_fabrickfile(path: &Path) -> Result<Fabrickfile, ParseError> {
    let file_path = resolve_fabrickfile_path(path);
    let content = read_file(&file_path)?;
    let fabrickfile: Fabrickfile = parse_toml(&content)?;
    fabrickfile.validate()?;
    Ok(fabrickfile)
}

/// Parse a Fabrickfile from a TOML string.
///
/// This function parses the TOML content and validates the result.
/// Useful when you already have the file content in memory.
///
/// # Arguments
///
/// * `content` - TOML string content
///
/// # Returns
///
/// The parsed and validated [`Fabrickfile`], or an error if parsing/validation fails.
///
/// # Errors
///
/// Returns [`ParseError::TomlError`] if the TOML is malformed.
/// Returns [`ParseError::ValidationError`] if validation fails.
pub fn parse_fabrickfile_str(content: &str) -> Result<Fabrickfile, ParseError> {
    let fabrickfile: Fabrickfile = parse_toml(content)?;
    fabrickfile.validate()?;
    Ok(fabrickfile)
}

/// Parse a mortar file from a file path.
///
/// This function reads the file, parses the TOML content, and validates the result.
/// The path can point either to the mortar file directly or to a directory containing
/// a file named "fabricks-mortar.toml".
///
/// # Arguments
///
/// * `path` - Path to the mortar file or directory containing it
///
/// # Returns
///
/// The parsed and validated [`MortarFile`], or an error if parsing/validation fails.
///
/// # Errors
///
/// Returns [`ParseError::IoError`] if the file cannot be read.
/// Returns [`ParseError::TomlError`] if the TOML is malformed.
/// Returns [`ParseError::ValidationError`] if validation fails.
///
/// # Example
///
/// ```no_run
/// use fabricks_common::parser::parse_mortar_file;
/// use std::path::Path;
///
/// let mortar = parse_mortar_file(Path::new("./fabricks-mortar.toml"))?;
/// println!("Project: {}", mortar.project.name);
/// # Ok::<(), fabricks_common::ParseError>(())
/// ```
pub fn parse_mortar_file(path: &Path) -> Result<MortarFile, ParseError> {
    let file_path = resolve_mortar_path(path);
    let content = read_file(&file_path)?;
    let mortar: MortarFile = parse_toml(&content)?;
    mortar.validate()?;
    Ok(mortar)
}

/// Parse a mortar file from a TOML string.
///
/// This function parses the TOML content and validates the result.
/// Useful when you already have the file content in memory.
///
/// # Arguments
///
/// * `content` - TOML string content
///
/// # Returns
///
/// The parsed and validated [`MortarFile`], or an error if parsing/validation fails.
///
/// # Errors
///
/// Returns [`ParseError::TomlError`] if the TOML is malformed.
/// Returns [`ParseError::ValidationError`] if validation fails.
pub fn parse_mortar_file_str(content: &str) -> Result<MortarFile, ParseError> {
    let mortar: MortarFile = parse_toml(content)?;
    mortar.validate()?;
    Ok(mortar)
}

/// Resolve the path to a Fabrickfile.
///
/// If the path is a directory, appends the default Fabrickfile name.
fn resolve_fabrickfile_path(path: &Path) -> std::path::PathBuf {
    if path.is_dir() {
        path.join(FABRICKFILE_NAME)
    } else {
        path.to_path_buf()
    }
}

/// Resolve the path to a mortar file.
///
/// If the path is a directory, appends the default mortar file name.
fn resolve_mortar_path(path: &Path) -> std::path::PathBuf {
    if path.is_dir() {
        path.join(MORTAR_FILE_NAME)
    } else {
        path.to_path_buf()
    }
}

/// Read a file's content as a string.
fn read_file(path: &std::path::PathBuf) -> Result<String, ParseError> {
    fs::read_to_string(path).map_err(|source| ParseError::IoError {
        path: path.display().to_string(),
        source,
    })
}

/// Parse TOML content into a type.
fn parse_toml<T>(content: &str) -> Result<T, ParseError>
where
    T: serde::de::DeserializeOwned,
{
    toml::from_str(content).map_err(ParseError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fabrickfile_str_valid() {
        let toml = r#"
            fabrick_version = "1.0"

            [info]
            name = "my-service"
            version = "1.0.0"

            [capabilities.network]
            listen = [8080]
        "#;

        let result = parse_fabrickfile_str(toml);
        assert!(result.is_ok());
        let fabrickfile = result.unwrap_or_else(|e| panic!("unexpected error: {e}"));
        assert_eq!(fabrickfile.info.name, "my-service");
        assert_eq!(fabrickfile.info.version, "1.0.0");
    }

    #[test]
    fn test_parse_fabrickfile_str_invalid_toml() {
        let toml = "this is not valid toml {{{";
        let result = parse_fabrickfile_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fabrickfile_str_validation_error() {
        let toml = r#"
            fabrick_version = "1.0"

            [info]
            name = "Invalid_Name"
            version = "1.0.0"
        "#;

        let result = parse_fabrickfile_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_mortar_file_str_valid() {
        let toml = r#"
            mortar_version = "1.0"

            [project]
            name = "my-project"

            [network.internal]
            internal = true

            [service.api]
            build = "./api"
            networks = ["internal"]
        "#;

        let result = parse_mortar_file_str(toml);
        assert!(result.is_ok());
        let mortar = result.unwrap_or_else(|e| panic!("unexpected error: {e}"));
        assert_eq!(mortar.project.name, "my-project");
        assert!(mortar.service.contains_key("api"));
    }

    #[test]
    fn test_parse_mortar_file_str_invalid_toml() {
        let toml = "this is not valid toml {{{";
        let result = parse_mortar_file_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_fabrickfile_path_file() {
        let path = Path::new("/some/path/Fabrickfile");
        let resolved = resolve_fabrickfile_path(path);
        assert_eq!(resolved, path);
    }

    #[test]
    fn test_resolve_mortar_path_file() {
        let path = Path::new("/some/path/fabricks-mortar.toml");
        let resolved = resolve_mortar_path(path);
        assert_eq!(resolved, path);
    }
}
