# Fabricks

**Declarative WASM orchestration platform for building, deploying, and managing microservices.**

[![License: MIT](https://img.shields.io/badge/License-AGPLv3-blue.svg)](https://opensource.org/licenses/MIT)
[![Status: Design Phase](https://img.shields.io/badge/Status-Design%20Phase-yellow.svg)](https://github.com/Liquescent-Development/fabricks)

---

## What is Fabricks?

Fabricks is a next-generation orchestration platform for WebAssembly (WASM) microservices. Think "Docker Compose for WASM" - but built from the ground up for the unique capabilities and constraints of WebAssembly.

**Key Features:**

-  **Declarative Configuration** - Define services in TOML (Fabrickfile & fabricks-mortar.toml)
-  **Capability-based Security** - Deny-by-default security model with explicit grants
-  **Network Segmentation** - Built-in network isolation and policy enforcement
-  **Lightning Fast** - Sub-millisecond cold starts, minimal resource overhead
-  **OCI Registry Compatible** - Works with Docker Hub, GHCR, ECR, Harbor, and more
-  **Component Model Native** - First-class support for WASM Component Model
-  **Developer Friendly** - Hot reload, auto-rebuild, comprehensive CLI
- ️ **Kubernetes Ready** - Generate SpinKube manifests or deploy standalone

---

## Quick Start

### Build from Source

```bash
# Clone the repository
git clone https://github.com/Liquescent-Development/fabricks.git
cd fabricks

# Build all crates
cargo build --release

# Install cargo-component for building WASM HTTP services
cargo install cargo-component
```

### Run the Hello HTTP Example

**1. Start the daemon (in a separate terminal):**

```bash
./target/release/fabricksd
```

**2. Build and deploy the example service:**

```bash
# Build the WASM component
cd examples/hello-http
cargo component build --release
cd ../..

# Deploy via CLI
./target/release/fabricks service run examples/hello-http
```

**3. Test it:**

```bash
curl http://localhost:8080/
# Response: Hello from Fabricks!
```

**4. Manage the service:**

```bash
# List services
./target/release/fabricks service ls

# Stop the service
./target/release/fabricks service stop <service-id>
```

### Create Your Own Service

**Fabrickfile:**
```toml
fabrick_version = "1.0"

[info]
name = "my-api"
version = "1.0.0"

[from]
source = "rust"

[build]
command = "cargo build --target wasm32-wasi --release"
output = "target/wasm32-wasi/release/my_api.wasm"

[capabilities.network]
listen = [8080]

[health_check.http]
path = "/health"
interval = "30s"
```

### Multi-Service Application

**fabricks-mortar.toml:**
```toml
mortar_version = "1.0"

[project]
name = "my-app"

[service.api]
build = "./services/api"
networks = ["application"]
ports = ["8080:8080"]

[service.postgres]
image = "wasm://pglite:latest"
networks = ["data"]

[service.postgres.volumes]
postgres_data = "/data"

[network.application]
internal = true

[network.data]
internal = true
ingress = ["application"]

[volume.postgres_data]
size = "10Gi"
```

**Deploy:**
```bash
# Build all services
fabricks mortar build

# Start everything
fabricks mortar up

# View logs
fabricks mortar logs --follow

# Scale services
fabricks mortar scale api=5

# Stop everything
fabricks mortar down
```

---

## Why Fabricks?

- ✅ **Lightweight** - Run more services per node vs containers
- ✅ **Instant startup** - Sub-millisecond cold starts
- ✅ **Declarative everything** - Networks and volumes defined in config, no orphans
- ✅ **Explicit capabilities** - Deny-by-default security model
- ✅ **WASM-native** - Built specifically for WebAssembly's strengths

---

## Architecture

Fabricks consists of two main components:

### 1. Fabrickfile (Single Service)

Defines how to build and run a single WASM module:
```toml
fabrick_version = "1.0"

[info]
name = "product-service"
version = "2.1.0"

[from]
source = "rust"  # or: image = "wasm://base:v1.0"

[build]
command = "cargo build --target wasm32-wasi --release"
output = "target/wasm32-wasi/release/product_service.wasm"

exports = ["list_products", "get_product"]

[imports]
logger = "wasm://logger:v1.0"

[capabilities]
env = ["DATABASE_URL"]

[capabilities.network]
listen = [8080]
connect = ["postgres:5432", "redis:6379"]

[health_check.http]
path = "/health"
interval = "30s"
```

### 2. fabricks-mortar.toml (Multi-Service)

Composes multiple services into a complete application:
```toml
mortar_version = "1.0"

[project]
name = "e-commerce"

[service.product]
build = "./services/product"
networks = ["application"]
environment = { DATABASE_URL = "postgres://db:5432/products" }

[service.product.replicas]
min = 2
max = 10
cpu_threshold = 70

[network.application]
internal = true

[volume.postgres_data]
size = "50Gi"
encrypted = true
```

---

## Documentation

Comprehensive documentation is available in the `/docs` directory:

### Core Documentation
- **[Fabrickfile and fabricks-mortar.toml Reference](docs/fabrickfile-mortar-reference.md)** - Complete Fabrickfile and fabricks-mortar.toml specification
- **[CLI Reference](docs/cli-reference.md)** - All `fabricks` commands
- **[Daemon API Reference](docs/daemon-api-reference.md)** - REST API for fabricksd
- **[Registry Specification](docs/registry-spec.md)** - OCI registry integration

### Getting Started
- **[Installation](docs/installation.md)** - Install Fabricks
- **[Quick Start](docs/quick-start.md)** - Your first fabrick
- **[Tutorial](docs/tutorial.md)** - Build a complete application

### Advanced Topics
- **[Network Segmentation](docs/networking.md)** - Security and isolation
- **[Capability Model](docs/capabilities.md)** - Fine-grained permissions
- **[Component Model](docs/component-model.md)** - WASM imports/exports
- **[Kubernetes Integration](docs/kubernetes.md)** - Deploy to K8s
- **[Production Best Practices](docs/production.md)** - Running in production

---

## Key Concepts

### Declarative-First

Fabricks is **declarative-first**. Unlike Docker where you can imperatively create networks and volumes that get lost, everything in Fabricks is defined in your mortar file:
```toml
# This creates the network when you run `fabricks mortar up`
[network.application]
internal = true

# This creates the volume
[volume.postgres_data]
size = "50Gi"

# When you run `fabricks mortar down`, everything is cleaned up
```

No more `docker volume ls` showing dozens of orphaned volumes!

### Capability-Based Security

Fabricks uses **deny-by-default** security. Services can only access what you explicitly grant:
```toml
[capabilities]
env = ["DATABASE_URL"]  # Can only read DATABASE_URL

[capabilities.network]
listen = [8080]                    # Can only listen on port 8080
connect = ["postgres:5432"]        # Can only connect to postgres

[capabilities.filesystem]
read = ["./config"]                # Can only read ./config
```

If it's not listed, access is **denied**.

### Network Segmentation

Built-in network isolation prevents unauthorized communication:
```toml
[network.dmz]
ingress = "0.0.0.0/0"  # Public internet
egress = ["application"]

[network.application]
internal = true
egress = ["data"]

[network.data]
internal = true
# Databases can't reach the internet

[network.payment]
isolated = true  # Completely isolated
audit_all = true # Log everything (PCI compliance)
```

### Component Model Integration

Native support for WASM Component Model:
```toml
# Export functions
exports = ["handle_request", "process_payment"]

# Import from other services
[imports]
logger = "wasm://logger:v1.0"
cache = { service = "redis", interface = "cache" }

# Component Model makes these direct function calls (no network overhead!)
```

---

## Registry Integration

Fabricks uses **OCI registries** for distribution. Any OCI-compliant registry works:
```bash
# Docker Hub
fabricks pull docker.io/library/redis:7.2

# GitHub Container Registry
fabricks pull ghcr.io/myorg/my-service:v1.0.0

# AWS ECR
fabricks pull 123456789.dkr.ecr.us-east-1.amazonaws.com/my-service:v1.0.0

# Private registry
fabricks pull registry.acme.io/my-service:v1.0.0

# Fabricks shorthand
fabricks pull wasm://redis:7.2
```

**Benefits:**
- Works with existing registry infrastructure
- Standard authentication (`docker login` works)
- Content-addressable storage (automatic deduplication)
- Cosign signature support
- SBOM attestation

---

## Project Status

**Fabricks is in active development.**

**Implemented:**
- ✅ Core WASM runtime (Wasmtime integration)
- ✅ HTTP service support (`wasi:http/incoming-handler`)
- ✅ CLI tool (`fabricks`)
- ✅ Daemon (`fabricksd`) with REST API
- ✅ Service management (create, start, stop, scale, delete)
- ✅ Network management and port binding
- ✅ Fabrickfile and mortar file parsing
- ✅ OCI registry client
- ✅ Health monitoring infrastructure

**In Progress:**
- 🔄 Auto-scaling
- 🔄 Policy engine
- 🔄 Volume management

**Planned:**
- ⏳ Kubernetes operator
- ⏳ SpinKube manifest generation
- ⏳ Public registry (registry.fabricks.dev)

---

## Contributing

We welcome contributions! Fabricks is an open-source project and we'd love your help building it.

### How to Contribute

1. **Review the design** - Read through the specifications in `/docs`
2. **Open an issue** - Discuss ideas, report bugs, suggest features
3. **Submit PRs** - Implementation, documentation, examples
4. **Share feedback** - Tell us what you think!

### Development Setup
```bash
# Clone the repository
git clone https://github.com/Liquescent-Development/fabricks.git
cd fabricks

# Read the specifications
ls docs/

# Start implementing!
```

---

## Roadmap

### Phase 1: MVP (Q1 2026)
- [ ] Core runtime (Wasmtime integration)
- [ ] Basic CLI (`build`, `run`, `push`, `pull`)
- [ ] OCI registry client
- [ ] Simple daemon for `fabricks mortar up`
- [ ] Fabrickfile parsing and validation

### Phase 2: Orchestration (Q1/Q2 2026)
- [ ] Full daemon API
- [ ] Health checks and auto-restart
- [ ] Auto-scaling
- [ ] Network management
- [ ] Volume management
- [ ] Policy engine

### Phase 3: Production Ready (Q3 2025)
- [ ] Kubernetes operator
- [ ] SpinKube manifest generation
- [ ] High availability
- [ ] Monitoring and observability
- [ ] Security scanning
- [ ] Production documentation

### Phase 4: Ecosystem (Q3 2025)
- [ ] Public registry (registry.fabricks.dev)
- [ ] Template library
- [ ] IDE extensions (VS Code, IntelliJ)
- [ ] CI/CD integrations
- [ ] Community fabricks registry

---

## Examples

Check out the `/examples` directory for working examples:

- **[hello-http](examples/hello-http/)** - HTTP service using `wasi:http/incoming-handler`

See `examples/README.md` for detailed instructions on building and running examples

---

## Community

- **GitHub Issues** - [Report bugs, request features](https://github.com/Liquescent-Development/fabricks/issues)
- **GitHub Discussions** - [Ask questions, share ideas](https://github.com/Liquescent-Development/fabricks/discussions)
- **Discord** - [Join our community](https://discord.gg/fabricks) (coming soon)

---

## License

Fabricks is licensed under the [AGPL-3.0 License](LICENSE).
---

## About

Fabricks is developed by [Liquescent Development LLC](https://github.com/Liquescent-Development).

**Author:** Richard Kiene ([@richardkiene](https://github.com/richardkiene))

---

