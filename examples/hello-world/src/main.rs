//! Hello World - Minimal Fabricks Example
//!
//! This is the simplest possible WASM service that responds
//! to HTTP requests with "Hello, World!".

fn main() {
    // In a real implementation, this would use a WASM-compatible
    // HTTP framework to handle incoming requests.
    //
    // The service exposes:
    // - GET /        -> "Hello, World!"
    // - GET /health  -> {"status": "ok"}
    //
    // For now, this is a placeholder demonstrating the structure.

    println!("Hello World service starting on port 8080...");

    // Main event loop would go here
    loop {
        // Handle HTTP requests
        // This is a placeholder - real implementation would use
        // wasi-http or similar WASM-compatible HTTP handling
    }
}

/// Health check response
#[derive(Debug)]
struct HealthResponse {
    status: &'static str,
}

impl HealthResponse {
    fn ok() -> Self {
        Self { status: "ok" }
    }
}
