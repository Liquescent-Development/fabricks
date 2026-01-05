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

There are two ways to run a service:

**Option A: Run directly from path (builds automatically)**

```bash
# Using the CLI - will build if needed and store in local storage
cargo run -p fabricks -- service run examples/hello-http

# Or with an absolute path
fabricks service run /path/to/examples/hello-http
```

**Option B: Build first, then run by tag**

```bash
# Build the module (stores in local OCI storage at ~/.fabricks/storage/)
cargo run -p fabricks -- build examples/hello-http
# ✓ Built hello-http:0.1.0

# List available modules
cargo run -p fabricks -- images
# REFERENCE           SIZE        VERSION    DIGEST
# hello-http:0.1.0    1.2 MB      0.1.0      sha256:...

# Run by tag
cargo run -p fabricks -- service run hello-http:0.1.0
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

---

## hello-tcp

A minimal TCP echo server that demonstrates the inetd model for TCP services.

### Prerequisites

- [Rust](https://rustup.rs/) (1.91+)
- [cargo-component](https://github.com/bytecodealliance/cargo-component): `cargo install cargo-component`

### Building

The hello-tcp example is a standalone crate that must be built separately with `cargo-component`:

```bash
cd examples/hello-tcp
cargo component build --release
```

This produces `target/wasm32-wasip1/release/hello_tcp.wasm`.

### Running

**1. Start the daemon (in a separate terminal):**

```bash
# From the fabricks workspace root
cargo run --release -p fabricksd
```

**2. Deploy the service:**

```bash
# Build the module
cargo run -p fabricks -- build examples/hello-tcp

# Run by tag
cargo run -p fabricks -- service run hello-tcp:0.1.0
```

**3. Test the service:**

```bash
# Connect with netcat
nc localhost 9000
# Response: Hello from Fabricks TCP! Type a message (or 'quit' to exit):

# Type a message and press Enter
hello world
# Response: Echo: hello world

# Type 'quit' to exit
quit
# Response: Goodbye!
```

### Files

- `Fabrickfile` - Service configuration
- `src/lib.rs` - TCP handler implementation (inetd model)
- `wit/world.wit` - WIT interface definition
- `Cargo.toml` - Rust dependencies

### How It Works

The hello-tcp service implements the `wasi:cli/run` interface (inetd model). When deployed:

1. The daemon parses the `Fabrickfile` and loads the WASM component
2. Port 9000 is bound (as specified in `capabilities.network.listen`)
3. For each incoming TCP connection:
   - The daemon connects the TCP stream to stdin/stdout of the WASM component
   - The WASM component runs and reads/writes to the stream
4. When the component exits, the connection is closed

### Fabrickfile

```toml
fabrick_version = "1.0"

[info]
name = "hello-tcp"
version = "0.1.0"
type = "tcp"
description = "A simple TCP echo server using the inetd model"

[from]
source = "rust"

[source]
path = "."

[build]
command = "cargo component build --release"
output = "target/wasm32-wasip1/release/hello_tcp.wasm"

[capabilities.network]
listen = [9000]
```

### inetd Model

The "inetd model" is a classic Unix pattern where a daemon (like inetd/xinetd) listens for connections and spawns a handler process with stdin/stdout connected to the socket. Fabricks uses this model for TCP services:

- **Advantages**: Simple to implement, secure isolation per connection, familiar programming model
- **Use cases**: Custom protocols, database-like services, text-based protocols

This contrasts with the HTTP model where the component implements `wasi:http/incoming-handler` and receives parsed HTTP requests
