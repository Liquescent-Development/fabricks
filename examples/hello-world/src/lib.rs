//! Hello World - The simplest possible Fabricks service.
//!
//! This is a minimal WASM component that prints a greeting and exits.
//! It demonstrates the basic structure of a Fabricks service using
//! the `wasi:cli/command` interface.

#[allow(warnings)]
mod bindings;

/// Entry point for the CLI component.
///
/// This function is called when the service starts. It prints a greeting
/// message and exits successfully.
fn main() {
    println!("Hello from Fabricks!");
    println!();
    println!("This is the simplest possible WASM service.");
    println!("It demonstrates a CLI component using wasi:cli/command.");
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
