//! Version command implementation.

use anyhow::Result;

use crate::output::writeln_stderr;

/// The current version of the fabricks CLI.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Run the version command.
///
/// Displays version information about the fabricks CLI.
///
/// # Errors
///
/// Returns an error if writing to stderr fails.
pub fn run() -> Result<()> {
    writeln_stderr(&format!("fabricks {VERSION}"))?;
    Ok(())
}
