# Quick Start

Get up and running with Fabricks in 5 minutes.

---

## Prerequisites

- [Fabricks installed](installation.md)
- Rust toolchain with `wasm32-wasi` target (for Rust projects)

```bash
# Verify installation
fabricks version

# Add WASM target (if using Rust)
rustup target add wasm32-wasi
```

---

## Create Your First Fabrick

### Step 1: Initialize a New Project

```bash
fabricks init --template rust --name hello-fabricks
cd hello-fabricks
```

This creates:
```
hello-fabricks/
├── Fabrickfile          # Service configuration
├── Cargo.toml           # Rust project
└── src/
    └── main.rs          # Application code
```

### Step 2: Examine the Fabrickfile

```toml
fabrick_version = "1.0"

[info]
name = "hello-fabricks"
version = "1.0.0"

[from]
source = "rust"

[build]
command = "cargo build --target wasm32-wasi --release"
output = "target/wasm32-wasi/release/hello_fabricks.wasm"

[capabilities.network]
listen = [8080]

[health_check.http]
path = "/health"
interval = "30s"
```

### Step 3: Build

```bash
fabricks build
```

Output:
```
Building hello-fabricks:1.0.0...
[1/3] Parsing Fabrickfile
[2/3] Compiling to WASM
      Running: cargo build --target wasm32-wasi --release
[3/3] Creating image
✓ Built hello-fabricks.wasm (1.2 MB)
```

### Step 4: Run

```bash
fabricks run .
```

Output:
```
Starting hello-fabricks...
✓ Loaded hello-fabricks.wasm (1.2 MB)
✓ Network: listen on 0.0.0.0:8080
Listening on http://localhost:8080
```

Test it:
```bash
curl http://localhost:8080/health
# {"status": "ok"}
```

---

## Development Mode

For active development, use dev mode with hot reload:

```bash
fabricks dev
```

This watches for file changes and automatically rebuilds:
```
Starting development mode...
✓ Built hello-fabricks.wasm (1.2 MB)
✓ Watching for changes...

Listening on http://localhost:8080

[10:45:23] File changed: src/main.rs
[10:45:24] Rebuilding...
[10:45:26] ✓ Rebuilt (1.2 MB)
[10:45:26] ✓ Reloaded
```

---

## Multi-Service Application

### Step 1: Create a Mortar File

Create `fabricks-mortar.toml` in your project root:

```toml
mortar_version = "1.0"

[project]
name = "my-app"

[service.api]
build = "."
networks = ["backend"]
ports = ["8080:8080"]

[service.redis]
image = "wasm://redis:7.2"
networks = ["backend"]

[network.backend]
internal = true
```

### Step 2: Start All Services

```bash
# Start the daemon (if not running)
fabricks daemon start

# Build and start all services
fabricks mortar up --build
```

Output:
```
Starting my-app...
✓ Creating networks (1)
✓ Starting services...
  → redis (1/1) ✓
  → api (1/1) ✓

All services healthy!
Access: http://localhost:8080
```

### Step 3: View Status

```bash
fabricks mortar ps
```

Output:
```
NAME    STATUS     REPLICAS    PORTS                    NETWORKS
redis   running    1/1         6379/tcp                 backend
api     running    1/1         0.0.0.0:8080->8080/tcp   backend
```

### Step 4: View Logs

```bash
fabricks mortar logs --follow
```

### Step 5: Stop Everything

```bash
fabricks mortar down
```

---

## Push to Registry

Share your fabrick with others:

```bash
# Login to registry
fabricks login registry.example.com

# Tag and push
fabricks build -t registry.example.com/my-org/hello-fabricks:v1.0.0
fabricks push registry.example.com/my-org/hello-fabricks:v1.0.0
```

---

## Common Commands

| Command | Description |
|---------|-------------|
| `fabricks build` | Build WASM module from Fabrickfile |
| `fabricks run .` | Run a single service |
| `fabricks dev` | Development mode with hot reload |
| `fabricks mortar up` | Start multi-service application |
| `fabricks mortar down` | Stop all services |
| `fabricks mortar ps` | List running services |
| `fabricks mortar logs` | View service logs |
| `fabricks push` | Push to registry |
| `fabricks pull` | Pull from registry |

---

## Next Steps

- [Tutorial](tutorial.md) - Build a complete application step-by-step
- [Fabrickfile Reference](fabrickfile-mortar-reference.md) - Configuration options
- [CLI Reference](cli-reference.md) - All available commands
- [Capabilities](capabilities.md) - Security and permissions
