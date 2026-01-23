//! Language-specific builders for Fabricks.
//!
//! This module provides builders that compile source code in various languages
//! to WebAssembly components. Users specify `[from].source = "python"` (or other
//! language) in their Fabrickfile, and the appropriate builder handles compilation.
//!
//! ## Supported Languages
//!
//! - **Rust**: Uses `cargo component build` for native WASM component compilation
//! - **Python**: Uses `componentize-py` to bundle `CPython` + user code
//! - **Go**: Uses `tinygo` for WASM compilation (planned)
//! - **JavaScript**: Uses `javy` or `componentize-js` (planned)
//!
//! ## Example
//!
//! ```toml
//! # Fabrickfile
//! [from]
//! source = "python"
//! version = "3.12"
//!
//! [source]
//! path = "."
//! entrypoint = "app:handler"
//! ```

mod python;
mod rust;

use std::path::Path;

use anyhow::{Result, bail};
use fabricks_common::models::fabrickfile::SourceLanguage;
use fabricks_common::Fabrickfile;

pub use python::PythonBuilder;
pub use rust::RustBuilder;

/// Configuration for a language builder.
#[derive(Debug, Clone)]
pub struct BuilderConfig<'a> {
    /// The Fabrickfile being built.
    pub fabrickfile: &'a Fabrickfile,

    /// Working directory (where Fabrickfile is located).
    pub workdir: &'a Path,

    /// Whether to run in release mode.
    pub release: bool,
}

/// Result of a successful build.
#[derive(Debug)]
pub struct BuildOutput {
    /// Path to the generated WASM file.
    pub wasm_path: std::path::PathBuf,
}

/// Trait for language-specific builders.
pub trait Builder {
    /// Check if the required toolchain is installed.
    ///
    /// Returns Ok(()) if ready, or an error with installation instructions.
    fn check_toolchain(&self) -> Result<()>;

    /// Build the source code to a WASM component.
    fn build(&self, config: &BuilderConfig<'_>) -> Result<BuildOutput>;
}

/// Get the appropriate builder for a source language.
///
/// # Errors
///
/// Returns an error if the language is not yet supported.
pub fn get_builder(language: SourceLanguage) -> Result<Box<dyn Builder>> {
    match language {
        SourceLanguage::Rust => Ok(Box::new(RustBuilder)),
        SourceLanguage::Python => Ok(Box::new(PythonBuilder)),
        SourceLanguage::Go => bail!(
            "Go builder is not yet implemented.\n\
             Track progress at: https://github.com/user/fabricks/issues/XX"
        ),
        SourceLanguage::Javascript => bail!(
            "JavaScript builder is not yet implemented.\n\
             Track progress at: https://github.com/user/fabricks/issues/XX"
        ),
        SourceLanguage::Csharp => bail!(
            "C# builder is not yet implemented.\n\
             Track progress at: https://github.com/user/fabricks/issues/XX"
        ),
    }
}

/// Build a Fabrickfile using the appropriate language builder.
///
/// This is the main entry point for building when `[from].source` is specified.
///
/// # Errors
///
/// Returns an error if:
/// - The language is not supported
/// - The toolchain is not installed
/// - The build fails
pub fn build_with_source(config: &BuilderConfig<'_>) -> Result<BuildOutput> {
    let from = config
        .fabrickfile
        .from
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No [from] section in Fabrickfile"))?;

    let language = from
        .source
        .ok_or_else(|| anyhow::anyhow!("No source language specified in [from]"))?;

    let builder = get_builder(language)?;

    // Check toolchain is installed
    builder.check_toolchain()?;

    // Run the build
    builder.build(config)
}
