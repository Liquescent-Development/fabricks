//! A simple TCP echo server for Fabricks using the inetd model.
//!
//! In the inetd model, the daemon accepts TCP connections on behalf of the service
//! and connects stdin/stdout to the TCP stream. The WASM component reads from stdin
//! and writes to stdout.
//!
//! This example implements a simple echo server that reads lines and echoes them back
//! with a prefix.

#[allow(warnings)]
mod bindings;

use std::io::{BufRead, Write};

/// Entry point for the TCP handler.
///
/// This function is called by the Fabricks daemon for each incoming TCP connection.
/// stdin is connected to the TCP receive stream, stdout to the send stream.
fn main() {
    // Write a greeting
    let mut stdout = std::io::stdout();
    writeln!(stdout, "Hello from Fabricks TCP! Type a message (or 'quit' to exit):").ok();
    stdout.flush().ok();

    // Read lines from stdin (the TCP connection) and echo them back
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(input) => {
                let trimmed = input.trim();

                // Check for quit command
                if trimmed.eq_ignore_ascii_case("quit") || trimmed.eq_ignore_ascii_case("exit") {
                    writeln!(stdout, "Goodbye!").ok();
                    stdout.flush().ok();
                    break;
                }

                // Echo back with prefix
                writeln!(stdout, "Echo: {}", trimmed).ok();
                stdout.flush().ok();
            }
            Err(_) => break, // Connection closed or error
        }
    }
}

// Export the main function as the component entry point
bindings::export!(Component with_types_in bindings);

struct Component;

impl bindings::exports::wasi::cli::run::Guest for Component {
    fn run() -> Result<(), ()> {
        main();
        Ok(())
    }
}
