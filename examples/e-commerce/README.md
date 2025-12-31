# E-Commerce Platform Example

A complete microservices e-commerce application demonstrating advanced Fabricks features.

## Structure

```
e-commerce/
├── fabricks-mortar.toml      # Multi-service composition
├── services/
│   ├── api-gateway/          # Public API gateway
│   ├── product/              # Product catalog service
│   ├── cart/                 # Shopping cart service
│   ├── order/                # Order processing service
│   └── user/                 # User management service
└── README.md
```

## Quick Start

```bash
# Start the daemon
fabricks daemon start

# Build and run all services
fabricks mortar up --build

# Test the API
curl http://localhost:8080/health
curl http://localhost:8080/api/products
curl http://localhost:8080/api/cart

# View service status
fabricks mortar ps

# View logs
fabricks mortar logs --follow

# Stop everything
fabricks mortar down
```

## What This Demonstrates

- Complete microservices architecture
- API gateway pattern
- Multi-tier network segmentation
- Service-to-service communication
- Redis caching layer
- PostgreSQL database
- Component Model imports/exports
- Auto-scaling configuration
- Production-ready health checks

## Architecture

```
                    ┌─────────────────┐
                    │    Internet     │
                    └────────┬────────┘
                             │ :8080
┌────────────────────────────▼─────────────────────────────┐
│                    [dmz network]                          │
│                                                           │
│              ┌─────────────────────┐                     │
│              │    API Gateway      │                     │
│              │   (routes requests) │                     │
│              └──────────┬──────────┘                     │
└─────────────────────────┼────────────────────────────────┘
                          │
┌─────────────────────────▼────────────────────────────────┐
│                 [application network]                     │
│                                                           │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│   │ Product  │  │   Cart   │  │  Order   │  │   User   │ │
│   │ Service  │  │ Service  │  │ Service  │  │ Service  │ │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘ │
└────────┼─────────────┼─────────────┼─────────────┼───────┘
         │             │             │             │
┌────────▼─────────────▼─────────────▼─────────────▼───────┐
│                   [data network]                          │
│                                                           │
│        ┌──────────────────┐    ┌──────────────────┐      │
│        │    PostgreSQL    │    │      Redis       │      │
│        │   (persistence)  │    │    (caching)     │      │
│        └──────────────────┘    └──────────────────┘      │
└──────────────────────────────────────────────────────────┘
```

## Key Features

### Network Segmentation

```toml
[network.dmz]
description = "Public-facing gateway"
ingress = "0.0.0.0/0"
egress = ["application"]

[network.application]
description = "Business logic tier"
internal = true
ingress = ["dmz"]
egress = ["data", "cache"]

[network.data]
description = "Database tier"
internal = true
ingress = ["application"]
```

### Component Model Integration

Services import functionality from each other:

```toml
[service.order.imports]
cart = { service = "cart", interface = "get-cart" }
user = { service = "user", interface = "get-user" }
product = { service = "product", interface = "get-product" }
```

### Auto-Scaling

```toml
[service.api-gateway.replicas]
min = 2
max = 20
cpu_threshold = 60

[service.product.replicas]
min = 2
max = 10
cpu_threshold = 70
```

## Scaling

```bash
# Scale services individually
fabricks mortar scale api-gateway=5
fabricks mortar scale product=3

# View current scale
fabricks mortar ps
```

## Development Mode

```bash
# Run with hot reload
fabricks mortar dev

# Run specific service in dev mode
fabricks dev ./services/product
```

## Next Steps

- Add payment processing: See [payment-service](../payment-service/) example
- Add monitoring: See [monitoring](../monitoring/) example
- Production deployment: See [production docs](../../docs/production.md)
