# Rust Hello World

A simple CLI service built with Rust using `wasi:cli/command`.

## What This Demonstrates

- Basic WASM component structure using `wasi:cli/command`
- Minimal Fabrickfile configuration with `[from].source = "rust"`
- Automatic Rust-to-WASM compilation via the RustBuilder

## Structure

```
rust-hello/
├── Fabrickfile       # Service configuration
├── Cargo.toml        # Rust project
├── wit/
│   └── world.wit     # WIT interface definition
├── src/
│   └── lib.rs        # Component implementation
└── README.md
```

## Prerequisites

- [Rust](https://rustup.rs/) (1.80+)
- [cargo-component](https://github.com/bytecodealliance/cargo-component): `cargo install cargo-component`

## Running

```bash
# Start the daemon (if not running)
fabricksd &

# Run the service
fabricks service run examples/rust-hello

# Check the logs
fabricks service logs rust-hello
```

## Output

```
Hello from Fabricks!

This is the simplest possible WASM service.
It demonstrates a CLI component using wasi:cli/command.
```

## How It Works

This service implements the `wasi:cli/run` interface. When executed:

1. The WASM runtime calls the `run()` function
2. The component prints its greeting to stdout
3. The component returns `Ok(())` to indicate success
4. The runtime exits cleanly

## Key Configuration

```toml
# Fabrickfile
[info]
name = "rust-hello"
version = "0.1.0"
type = "command"

[from]
source = "rust"  # Uses RustBuilder - no manual build needed!

[source]
path = "."
```

## Next Steps

- See [rust-http](../rust-http/) for a Rust HTTP service example
- See [go-hello](../go-hello/) for a Go CLI example
- See [go-http](../go-http/) for a Go HTTP service example
