# Fabricks Examples

This directory contains example services demonstrating Fabricks capabilities.

## hello-http

A minimal HTTP service that responds to requests using the WASI HTTP interface.

### Prerequisites

- [Rust](https://rustup.rs/) (1.91+)
- [cargo-component](https://github.com/bytecodealliance/cargo-component): `cargo install cargo-component`

### Building

The hello-http example is a standalone crate that must be built separately with `cargo-component`:

```bash
cd examples/hello-http
cargo component build --release
```

This produces `target/wasm32-wasip1/release/hello_http.wasm`.

### Running

**1. Start the daemon (in a separate terminal):**

```bash
# From the fabricks workspace root
cargo run --release -p fabricksd
```

**2. Deploy the service:**

```bash
# Using the CLI
cargo run -p fabricks -- service run examples/hello-http

# Or with an absolute path
fabricks service run /path/to/examples/hello-http
```

**3. Test the service:**

```bash
# Root endpoint
curl http://localhost:8080/
# Response: Hello from Fabricks!

# Health endpoint
curl http://localhost:8080/health
# Response: OK

# Unknown path (404)
curl http://localhost:8080/unknown
# Response: Not Found
```

**4. Manage the service:**

```bash
# List services
fabricks service ls

# Inspect service details
fabricks service inspect <service-id>

# Stop the service
fabricks service stop <service-id>

# Remove the service
fabricks service rm <service-id>
```

### Files

- `Fabrickfile` - Service configuration
- `src/lib.rs` - HTTP handler implementation
- `wit/world.wit` - WIT interface definition
- `Cargo.toml` - Rust dependencies

### How It Works

The hello-http service implements the `wasi:http/incoming-handler` interface. When deployed:

1. The daemon parses the `Fabrickfile` and loads the WASM component
2. Port 8080 is bound (as specified in `capabilities.network.listen`)
3. Incoming HTTP requests are routed to the WASM handler
4. The handler generates responses based on the request path

### Fabrickfile

```toml
fabrick_version = "1.0"

[info]
name = "hello-http"
version = "0.1.0"
type = "http"
description = "A simple HTTP service that responds with Hello from Fabricks!"

[from]
source = "rust"

[source]
path = "."

[build]
command = "cargo component build --release"
output = "target/wasm32-wasip1/release/hello_http.wasm"

[capabilities.network]
listen = [8080]

[health_check.http]
path = "/health"
```
