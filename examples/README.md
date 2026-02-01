# Fabricks Examples

This directory contains example services demonstrating Fabricks capabilities across multiple languages.

## Quick Start

All examples can be run with a single command - Fabricks handles the build automatically:

```bash
# Start the daemon
fabricksd &

# Run any example
fabricks service run examples/rust-hello
fabricks service run examples/rust-http --network default
fabricks service run examples/go-hello
fabricks service run examples/go-http --network default
```

## Examples by Language

### Rust

| Example | Type | Description |
|---------|------|-------------|
| [rust-hello](./rust-hello/) | CLI | Simple "Hello World" using `wasi:cli/command` |
| [rust-http](./rust-http/) | HTTP | HTTP service using `wasi:http/incoming-handler` |
| [hello-tcp](./hello-tcp/) | TCP | TCP echo server using the inetd model |

### Go (TinyGo)

| Example | Type | Description |
|---------|------|-------------|
| [go-hello](./go-hello/) | CLI | Simple "Hello World" using `wasi:cli/command` |
| [go-http](./go-http/) | HTTP | HTTP service using `wasi:http/incoming-handler` |

### JavaScript (Node.js)

| Example | Type | Description |
|---------|------|-------------|
| [nodejs-hello](./nodejs-hello/) | HTTP | HTTP service using ComponentizeJS |

### Python

| Example | Type | Description |
|---------|------|-------------|
| [python-hello](./python-hello/) | HTTP | HTTP service using componentize-py |

---

## rust-http

A minimal HTTP service that responds to requests using the WASI HTTP interface.

### Prerequisites

- [Rust](https://rustup.rs/) (1.80+)
- [cargo-component](https://github.com/bytecodealliance/cargo-component): `cargo install cargo-component`

### Running

```bash
# Deploy the service (builds automatically)
fabricks service run examples/rust-http --network default

# Test the service
curl http://localhost:8888/
# Response: Hello from Fabricks!

curl http://localhost:8888/health
# Response: OK
```

### Fabrickfile

```toml
fabrick_version = "1.0"

[info]
name = "rust-http"
version = "0.1.0"
type = "http"

[from]
source = "rust"  # Uses RustBuilder - no manual build needed!

[source]
path = "."

[capabilities.network]
listen = [8888]

[health_check.http]
path = "/health"
```

---

## hello-tcp

A minimal TCP echo server that demonstrates the inetd model for TCP services.

### Prerequisites

- [Rust](https://rustup.rs/) (1.80+)
- [cargo-component](https://github.com/bytecodealliance/cargo-component): `cargo install cargo-component`

### Running

```bash
# Deploy the service
fabricks service run examples/hello-tcp --network default

# Test with netcat
nc localhost 9000
# Response: Hello from Fabricks TCP! Type a message (or 'quit' to exit):
```

### inetd Model

The "inetd model" is a classic Unix pattern where a daemon listens for connections and spawns a handler process with stdin/stdout connected to the socket. Fabricks uses this model for TCP services:

- **Advantages**: Simple to implement, secure isolation per connection, familiar programming model
- **Use cases**: Custom protocols, database-like services, text-based protocols

---

## Service Management

```bash
# List services
fabricks service ls

# Inspect service details
fabricks service inspect <service-id>

# View logs
fabricks service logs <service-id>

# Stop the service
fabricks service stop <service-id>

# Remove the service
fabricks service rm <service-id>
```
