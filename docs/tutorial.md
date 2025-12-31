# Tutorial: Build a Complete Application

This tutorial walks through building a multi-service e-commerce API with Fabricks.

---

## What We're Building

A product catalog API with:
- **API service** - REST API for products (Rust)
- **PostgreSQL** - Database for product data
- **Redis** - Caching layer
- **Network isolation** - Proper security boundaries

---

## Prerequisites

- [Fabricks installed](installation.md)
- Rust with `wasm32-wasi` target
- Basic familiarity with REST APIs

```bash
rustup target add wasm32-wasi
```

---

## Step 1: Project Structure

Create the project directory:

```bash
mkdir product-catalog
cd product-catalog
```

Create the following structure:
```
product-catalog/
├── fabricks-mortar.toml      # Multi-service composition
├── services/
│   └── api/
│       ├── Fabrickfile       # API service config
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
```

---

## Step 2: Create the API Service

### Fabrickfile

Create `services/api/Fabrickfile`:

```toml
fabrick_version = "1.0"

[info]
name = "product-api"
version = "1.0.0"
description = "Product catalog REST API"

[from]
source = "rust"

[source]
path = "."
include = ["src/**/*.rs", "Cargo.toml", "Cargo.lock"]

[build]
command = "cargo build --target wasm32-wasi --release"
output = "target/wasm32-wasi/release/product_api.wasm"
watch = ["src/**/*.rs", "Cargo.toml"]

[capabilities]
env = ["DATABASE_URL", "REDIS_URL", "LOG_LEVEL"]

[capabilities.network]
listen = [8080]
connect = ["postgres:5432", "redis:6379"]

[capabilities.filesystem]
read = ["./config"]

[health_check.http]
path = "/health"
port = 8080
interval = "30s"
timeout = "5s"

[config]
port = 8080
log_level = "info"

[config.resources]
memory = "256Mi"
cpu = 0.5
```

### Cargo.toml

Create `services/api/Cargo.toml`:

```toml
[package]
name = "product-api"
version = "1.0.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Application Code

Create `services/api/src/main.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Serialize, Deserialize)]
struct Product {
    id: u32,
    name: String,
    price: f64,
    description: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

fn main() {
    // Simple HTTP server implementation
    // In production, use a WASM-compatible HTTP framework

    let products = vec![
        Product {
            id: 1,
            name: "Widget".to_string(),
            price: 29.99,
            description: "A fantastic widget".to_string(),
        },
        Product {
            id: 2,
            name: "Gadget".to_string(),
            price: 49.99,
            description: "An amazing gadget".to_string(),
        },
    ];

    // Handle requests (simplified)
    loop {
        // Read request, route to handler, write response
        // This is a simplified example - real apps would use
        // a WASM-compatible HTTP framework
    }
}

fn handle_health() -> String {
    serde_json::to_string(&HealthResponse { status: "ok" }).unwrap()
}

fn handle_products(products: &[Product]) -> String {
    serde_json::to_string(products).unwrap()
}
```

---

## Step 3: Create the Mortar File

Create `fabricks-mortar.toml` in the project root:

```toml
mortar_version = "1.0"

[project]
name = "product-catalog"
version = "1.0.0"
description = "Product catalog e-commerce API"

# ============================================================================
# Variables
# ============================================================================

[variable.log_level]
type = "string"
default = "info"
description = "Log level for all services"

[variable.db_pool_size]
type = "number"
default = 10
description = "Database connection pool size"

# ============================================================================
# Networks
# ============================================================================

[network.public]
description = "Public-facing API"
ingress = "0.0.0.0/0"
egress = ["application"]

[network.application]
description = "Application tier"
internal = true
ingress = ["public"]
egress = ["data", "cache"]

[network.data]
description = "Database tier"
internal = true
ingress = ["application"]

[network.cache]
description = "Cache tier"
internal = true
ingress = ["application"]

# ============================================================================
# Services
# ============================================================================

[service.api]
build = "./services/api"
networks = ["public", "application"]
ports = ["8080:8080"]
depends_on = ["postgres", "redis"]

environment = {
    DATABASE_URL = "postgres://postgres:5432/products?pool_size=${variable.db_pool_size}",
    REDIS_URL = "redis://redis:6379/0",
    LOG_LEVEL = "${variable.log_level}"
}

[service.api.replicas]
min = 2
max = 10
cpu_threshold = 70

[service.api.resources]
memory = "512Mi"
cpu = 1.0

# ----------------------------------------------------------------------------

[service.postgres]
image = "wasm://pglite:latest"
networks = ["data"]

environment = {
    POSTGRES_DB = "products",
    POSTGRES_USER = "app",
    POSTGRES_PASSWORD = "secret"
}

[service.postgres.volumes]
postgres_data = "/var/lib/postgresql/data"

[service.postgres.replicas]
min = 1
max = 1

[service.postgres.resources]
memory = "1Gi"
cpu = 1.0

[service.postgres.health_check.tcp]
port = 5432
interval = "10s"

# ----------------------------------------------------------------------------

[service.redis]
image = "wasm://redis:7.2"
networks = ["cache"]

[service.redis.volumes]
redis_data = "/data"

[service.redis.replicas]
min = 2
max = 2

[service.redis.resources]
memory = "256Mi"
cpu = 0.5

[service.redis.health_check.tcp]
port = 6379
interval = "10s"

# ============================================================================
# Volumes
# ============================================================================

[volume.postgres_data]
size = "10Gi"

[volume.redis_data]
size = "1Gi"

# ============================================================================
# Validation
# ============================================================================

[validate]
require_health_checks = true
deny_wildcard_connect = true
check_circular_dependencies = true
```

---

## Step 4: Build and Run

### Validate Configuration

```bash
fabricks validate
```

Output:
```
Validating fabricks-mortar.toml...
✓ Syntax valid
✓ All services have health checks
✓ No circular dependencies
✓ Network configuration valid
✓ Valid!
```

### Visualize Dependencies

```bash
fabricks graph
```

Output:
```
┌─────────────────┐
│       api       │
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
┌────────┐ ┌───────┐
│postgres│ │ redis │
└────────┘ └───────┘
```

### Build All Services

```bash
fabricks mortar build --parallel
```

Output:
```
Building 3 services...
[1/3] ✓ postgres (pulled from registry)
[2/3] ✓ redis (pulled from registry)
[3/3] ⚙ api (building...)
      Running: cargo build --target wasm32-wasi --release
      Finished in 32.1s
✓ Built 3 services in 35.2s
```

### Start Everything

```bash
# Start the daemon first
fabricks daemon start

# Start all services
fabricks mortar up -d
```

Output:
```
Starting product-catalog...
✓ Creating networks (4)
✓ Creating volumes (2)
✓ Starting services in dependency order...
  → postgres (1/1) ✓
  → redis (2/2) ✓
  → api (2/2) ✓

All services healthy!
Access: http://localhost:8080
```

---

## Step 5: Test the API

```bash
# Health check
curl http://localhost:8080/health
# {"status":"ok"}

# List products
curl http://localhost:8080/products
# [{"id":1,"name":"Widget",...},{"id":2,"name":"Gadget",...}]

# Get single product
curl http://localhost:8080/products/1
# {"id":1,"name":"Widget","price":29.99,"description":"A fantastic widget"}
```

---

## Step 6: Monitor and Scale

### View Status

```bash
fabricks mortar ps
```

Output:
```
NAME       STATUS     REPLICAS    PORTS                    NETWORKS
postgres   running    1/1         5432/tcp                 data
redis      running    2/2         6379/tcp                 cache
api        running    2/2         0.0.0.0:8080->8080/tcp   public, application
```

### View Logs

```bash
# All services
fabricks mortar logs --follow

# Specific service
fabricks mortar logs api --tail 50
```

### Scale the API

```bash
fabricks mortar scale api=5
```

Output:
```
Scaling services...
✓ api: 2 → 5 instances
Done!
```

### View Resource Usage

```bash
fabricks service stats api
```

Output:
```
SERVICE   CPU %    MEM USAGE / LIMIT     MEM %    NET I/O
api       35.0%    245Mi / 512Mi         47.9%    1.0MB / 2.0MB
```

### Monitor Events

```bash
fabricks events --service api
```

---

## Step 7: Inspect Services

### Service Details

```bash
fabricks service inspect api
```

Output:
```yaml
id: a1b2c3d4e5f6
name: api
image: product-api:1.0.0
state: running
networks:
  - public
  - application
environment:
  DATABASE_URL: postgres://postgres:5432/products
  REDIS_URL: redis://redis:6379/0
  LOG_LEVEL: info
replicas:
  desired: 5
  running: 5
  healthy: 5
```

### Network Details

```bash
fabricks network inspect application
```

Output:
```yaml
id: x1y2z3w4
name: application
internal: true
ingress:
  - public
egress:
  - data
  - cache
services:
  - api
```

---

## Step 8: Clean Up

```bash
# Stop all services
fabricks mortar down

# Stop and remove volumes
fabricks mortar down -v
```

---

## Next Steps

- [Networking](networking.md) - Deep dive into network segmentation
- [Capabilities](capabilities.md) - Understanding the security model
- [Production](production.md) - Deploy to production
- [Kubernetes](kubernetes.md) - Deploy to Kubernetes
