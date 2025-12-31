# Multi-Service API Example

A REST API with PostgreSQL database demonstrating multi-service orchestration with Fabricks.

## Structure

```
multi-service-api/
├── fabricks-mortar.toml    # Multi-service composition
├── services/
│   └── api/
│       ├── Fabrickfile     # API service config
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
└── README.md
```

## Quick Start

```bash
# Start the daemon (if not running)
fabricks daemon start

# Build and run all services
fabricks mortar up --build

# Test the API
curl http://localhost:8080/health
curl http://localhost:8080/items
curl -X POST http://localhost:8080/items -d '{"name": "Widget", "price": 29.99}'

# View logs
fabricks mortar logs --follow

# Stop everything
fabricks mortar down
```

## What This Demonstrates

- Multi-service composition with `fabricks-mortar.toml`
- Network segmentation between tiers
- Database service with persistent volume
- Environment variable configuration
- Service dependencies (`depends_on`)
- Health checks

## Architecture

```
┌─────────────────────────────────────────────┐
│                  Internet                    │
└─────────────────────┬───────────────────────┘
                      │ :8080
┌─────────────────────▼───────────────────────┐
│              [public network]                │
│                                              │
│    ┌────────────────────────────────┐       │
│    │         API Service            │       │
│    │      (Rust WASM module)        │       │
│    └────────────────┬───────────────┘       │
└─────────────────────┼───────────────────────┘
                      │
┌─────────────────────▼───────────────────────┐
│              [data network]                  │
│                                              │
│    ┌────────────────────────────────┐       │
│    │       PostgreSQL Service       │       │
│    │     (pglite WASM module)       │       │
│    │                                │       │
│    │  Volume: postgres_data (10Gi)  │       │
│    └────────────────────────────────┘       │
└─────────────────────────────────────────────┘
```

## Key Configuration

### Network Segmentation

```toml
[network.public]
description = "Public-facing API"
ingress = "0.0.0.0/0"
egress = ["data"]

[network.data]
description = "Database tier"
internal = true
ingress = ["public"]
```

- The API is in `public` network (accessible from internet)
- PostgreSQL is in `data` network (internal only)
- API can reach database, but database can't reach internet

### Service Dependencies

```toml
[service.api]
depends_on = ["postgres"]  # Starts postgres first
```

### Persistent Storage

```toml
[service.postgres.volumes]
postgres_data = "/var/lib/postgresql/data"

[volume.postgres_data]
size = "10Gi"
```

## Scaling

```bash
# Scale the API to 3 instances
fabricks mortar scale api=3

# View status
fabricks mortar ps
```

## Next Steps

- Add Redis caching: See [e-commerce](../e-commerce/) example
- Add monitoring: See [monitoring](../monitoring/) example
- Production deployment: See [production docs](../../docs/production.md)
