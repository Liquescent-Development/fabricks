# Hello World Example

The simplest possible Fabrickfile demonstrating a minimal WASM service.

## Structure

```
hello-world/
├── Fabrickfile       # Service configuration
├── Cargo.toml        # Rust project
├── src/
│   └── main.rs       # Application code
└── README.md
```

## Quick Start

```bash
# Build the fabrick
fabricks build

# Run locally
fabricks run .

# Test it
curl http://localhost:8080/
curl http://localhost:8080/health
```

## What This Demonstrates

- Minimal Fabrickfile structure
- Basic capability grants (network listen)
- HTTP health check configuration
- Rust-to-WASM compilation

## Key Configuration

```toml
# Specify the language runtime
[from]
source = "rust"

# Build command for WASM target
[build]
command = "cargo build --target wasm32-wasi --release"
output = "target/wasm32-wasi/release/hello_world.wasm"

# Grant permission to listen on port 8080
[capabilities.network]
listen = [8080]
```

## Next Steps

- Add environment variables: `[capabilities] env = ["LOG_LEVEL"]`
- Add filesystem access: `[capabilities.filesystem] read = ["./config"]`
- Connect to other services: `[capabilities.network] connect = ["redis:6379"]`

See the [multi-service-api](../multi-service-api/) example for a more complete application.
