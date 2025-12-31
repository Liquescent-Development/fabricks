//! CLI argument definitions using clap.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Fabricks - Declarative WASM Orchestration Platform.
///
/// Build, deploy, and manage WebAssembly microservices with a familiar,
/// container-like workflow.
#[derive(Parser, Debug)]
#[command(name = "fabricks")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Enable verbose output.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Validate configuration files.
    ///
    /// Checks Fabrickfile or fabricks-mortar.toml for syntax errors
    /// and validates all fields according to the specification.
    Validate(ValidateArgs),

    /// Show version information.
    Version,
}

/// Arguments for the validate command.
#[derive(Parser, Debug)]
pub struct ValidateArgs {
    /// Path to the file or directory to validate.
    ///
    /// If a directory is provided, looks for Fabrickfile or
    /// fabricks-mortar.toml based on the --type flag.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Type of file to validate.
    ///
    /// If not specified, attempts to auto-detect based on filename.
    #[arg(short = 't', long = "type", value_enum)]
    pub file_type: Option<FileType>,

    /// Output format for validation results.
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

/// Type of configuration file.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FileType {
    /// Fabrickfile (single service definition).
    Fabrickfile,
    /// fabricks-mortar.toml (multi-service composition).
    Mortar,
}

/// Output format for command results.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output.
    #[default]
    Text,
    /// JSON output for programmatic consumption.
    Json,
}
