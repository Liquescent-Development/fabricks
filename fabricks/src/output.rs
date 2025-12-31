//! Output utilities for CLI commands.
//!
//! This module provides a consistent way to write output to stderr,
//! avoiding the clippy `print_stderr` lint while maintaining testability.

use std::io::{self, Write};

/// Write a line to stderr.
///
/// # Errors
///
/// Returns an error if writing to stderr fails.
pub fn writeln_stderr(message: &str) -> io::Result<()> {
    let mut stderr = io::stderr();
    writeln!(stderr, "{message}")
}
