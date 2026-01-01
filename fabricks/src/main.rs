//! Fabricks CLI
//!
//! Command-line interface for the Fabricks WASM orchestration platform.
//!
//! # Commands
//!
//! - `fabricks build` - Build a Fabrickfile into a WASM module
//! - `fabricks run` - Run a WASM module locally
//! - `fabricks push` - Push a module to an OCI registry
//! - `fabricks pull` - Pull a module from an OCI registry
//! - `fabricks validate` - Validate configuration files
//! - `fabricks inspect` - Inspect a WASM module
//! - `fabricks login` - Authenticate with a registry
//! - `fabricks logout` - Remove registry credentials
//! - `fabricks version` - Show version information

use std::process::ExitCode;

use clap::Parser;
use tokio::runtime::Runtime as TokioRuntime;

mod cli;
mod commands;
mod credentials;
mod output;

use cli::{Cli, Commands};

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Initialize tracing if verbose mode is enabled
    if cli.verbose {
        init_tracing();
    }

    // Create async runtime
    let rt = match TokioRuntime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = output::writeln_stderr(&format!("Failed to create runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };

    let result = match cli.command {
        Commands::Build(args) => rt.block_on(commands::build::run(&args)),
        Commands::Run(args) => rt.block_on(commands::run::run(&args)),
        Commands::Push(args) => rt.block_on(commands::push::run(&args)),
        Commands::Pull(args) => rt.block_on(commands::pull::run(&args)),
        Commands::Inspect(args) => commands::inspect::run(&args),
        Commands::Validate(args) => commands::validate::run(&args),
        Commands::Login(args) => commands::login::run(&args),
        Commands::Logout(args) => commands::logout::run(&args),
        Commands::Version => commands::version::run(),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Ignore I/O errors when writing error messages
            let _ = output::writeln_stderr(&format!("Error: {e}"));
            ExitCode::FAILURE
        }
    }
}

/// Initialize tracing subscriber for verbose output.
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}
