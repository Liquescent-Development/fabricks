# Hello World

The simplest possible Fabricks service - a CLI component that prints a greeting and exits.

## What This Demonstrates

- Basic WASM component structure using `wasi:cli/command`
- Minimal Fabrickfile configuration
- How to build a Rust WASM component with `cargo-component`

## Structure

```
hello-world/
├── Fabrickfile       # Service configuration
├── Cargo.toml        # Rust project
├── wit/
│   └── world.wit     # WIT interface definition
├── src/
│   └── lib.rs        # Component implementation
└── README.md
```

## Prerequisites

- [Rust](https://rustup.rs/) (1.91+)
- [cargo-component](https://github.com/bytecodealliance/cargo-component): `cargo install cargo-component`

## Building

```bash
cd examples/hello-world
cargo component build --release
```

This produces `target/wasm32-wasip1/release/hello_world.wasm`.

## Running

```bash
# Using wasmtime directly
wasmtime target/wasm32-wasip1/release/hello_world.wasm

# Or with the Fabricks CLI (once daemon is running)
fabricks service run examples/hello-world
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
[info]
name = "hello-world"
version = "0.1.0"
type = "cli"  # CLI type - runs once and exits

[build]
command = "cargo component build --release"
output = "target/wasm32-wasip1/release/hello_world.wasm"
```

## Next Steps

- See [hello-http](../hello-http/) for an HTTP service example
- See [hello-tcp](../hello-tcp/) for a TCP service example
- See [multi-service-api](../multi-service-api/) for a multi-service application
