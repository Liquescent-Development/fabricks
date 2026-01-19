# Multi-Service API Example

A REST API demonstrating service composition with Fabricks.

This example uses in-memory storage to keep things simple while demonstrating
the patterns you'd use in a real multi-service application.

## What This Demonstrates

- Multi-service composition with `fabricks-mortar.toml`
- HTTP REST API implementation with WASM
- In-memory CRUD operations
- Network configuration and health checks
- Auto-scaling configuration

## Structure

```
multi-service-api/
├── fabricks-mortar.toml    # Multi-service composition
├── services/
│   └── api/
│       ├── Fabrickfile     # API service config
│       ├── Cargo.toml
│       ├── wit/world.wit   # WIT interface
│       └── src/lib.rs      # HTTP handler
└── README.md
```

## Prerequisites

- [Rust](https://rustup.rs/) (1.91+)
- [cargo-component](https://github.com/bytecodealliance/cargo-component): `cargo install cargo-component`

## Building

```bash
cd examples/multi-service-api/services/api
cargo component build --release
```

This produces `target/wasm32-wasip1/release/api.wasm`.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Service info |
| GET | `/health` | Health check |
| GET | `/items` | List all items |
| GET | `/items/{id}` | Get item by ID |
| POST | `/items` | Create item |
| DELETE | `/items/{id}` | Delete item |

### Example Requests

```bash
# Health check
curl http://localhost:8080/health
# Response: {"status":"ok"}

# List items (pre-populated with sample data)
curl http://localhost:8080/items
# Response: [{"id":1,"name":"Widget",...},...]

# Get single item
curl http://localhost:8080/items/1
# Response: {"id":1,"name":"Widget","description":"A useful widget","price":19.99}

# Create new item
curl -X POST http://localhost:8080/items \
  -d '{"name":"New Item","description":"A new item","price":9.99}'
# Response: {"id":4,"name":"New Item","description":"A new item","price":9.99}

# Delete item
curl -X DELETE http://localhost:8080/items/1
# Response: {"deleted":true}
```

## Configuration

### fabricks-mortar.toml

```toml
[project]
name = "multi-service-api"
version = "1.0.0"

[network.public]
description = "Public-facing API tier"
ingress = "0.0.0.0/0"

[service.api]
build = "./services/api"
networks = ["public"]
ports = ["8080:8080"]

[service.api.replicas]
min = 1
max = 5
cpu_threshold = 70
```

### Fabrickfile

```toml
[info]
name = "api"
type = "http"

[build]
command = "cargo component build --release"
output = "target/wasm32-wasip1/release/api.wasm"

[capabilities.network]
listen = [8080]

[health_check.http]
path = "/health"
```

## Running with Fabricks

```bash
# Start the daemon
fabricks daemon start

# Deploy the mortar project
fabricks mortar up

# Test the API
curl http://localhost:8080/items

# Stop
fabricks mortar down
```

## Next Steps

- See [e-commerce](../e-commerce/) for a more complex multi-service example
- See [payment-service](../payment-service/) for security isolation patterns
- See [monitoring](../monitoring/) for observability setup
