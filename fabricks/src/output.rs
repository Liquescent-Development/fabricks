//! Output utilities for CLI commands.
//!
//! This module provides a consistent way to write output to stdout/stderr,
//! avoiding the clippy `print_stdout`/`print_stderr` lints while maintaining testability.

use std::io::{self, Write};

/// Write a line to stdout.
///
/// # Errors
///
/// Returns an error if writing to stdout fails.
pub fn writeln(message: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    writeln!(stdout, "{message}")
}

/// Write a line to stderr.
///
/// # Errors
///
/// Returns an error if writing to stderr fails.
pub fn writeln_stderr(message: &str) -> io::Result<()> {
    let mut stderr = io::stderr();
    writeln!(stderr, "{message}")
}

/// Read a line from stdin.
///
/// # Errors
///
/// Returns an error if reading from stdin fails.
pub fn read_line() -> io::Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Prompt the user for input (writes prompt to stderr, reads from stdin).
///
/// # Errors
///
/// Returns an error if reading or writing fails.
pub fn prompt(message: &str) -> io::Result<String> {
    let mut stderr = io::stderr();
    write!(stderr, "{message}")?;
    stderr.flush()?;
    read_line()
}
