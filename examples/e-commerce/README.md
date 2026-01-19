# E-Commerce Platform Example

A complete microservices e-commerce application demonstrating Fabricks patterns.

This example uses in-memory storage for simplicity while demonstrating how you would
structure a real e-commerce platform with multiple services.

## What This Demonstrates

- Microservices architecture with 5 services
- API gateway pattern for request routing
- Multi-tier network segmentation
- In-memory CRUD operations per service
- Auto-scaling configuration
- Health checks for each service

## Structure

```
e-commerce/
├── fabricks-mortar.toml      # Multi-service composition
├── services/
│   ├── api-gateway/          # Public API gateway (port 8080)
│   ├── product/              # Product catalog (port 8081)
│   ├── cart/                 # Shopping cart (port 8082)
│   ├── order/                # Order processing (port 8083)
│   └── user/                 # User management (port 8084)
└── README.md
```

## Prerequisites

- [Rust](https://rustup.rs/) (1.91+)
- [cargo-component](https://github.com/bytecodealliance/cargo-component): `cargo install cargo-component`

## Building

Build all services:

```bash
cd examples/e-commerce/services/api-gateway && cargo component build --release
cd examples/e-commerce/services/product && cargo component build --release
cd examples/e-commerce/services/cart && cargo component build --release
cd examples/e-commerce/services/order && cargo component build --release
cd examples/e-commerce/services/user && cargo component build --release
```

## Services

### API Gateway (port 8080)

Entry point for the platform. Returns service information.

```bash
curl http://localhost:8080/           # Service info
curl http://localhost:8080/health     # Health check
curl http://localhost:8080/api/products  # Product service info
curl http://localhost:8080/api/cart      # Cart service info
curl http://localhost:8080/api/orders    # Order service info
curl http://localhost:8080/api/users     # User service info
```

### Product Service (port 8081)

Product catalog with pre-populated items.

```bash
curl http://localhost:8081/           # List all products
curl http://localhost:8081/1          # Get product by ID
curl http://localhost:8081/health     # Health check
```

### Cart Service (port 8082)

Shopping cart management per user.

```bash
curl http://localhost:8082/1          # Get cart for user 1
curl -X POST http://localhost:8082/1/items  # Add item to cart
curl -X DELETE http://localhost:8082/1/items/1  # Remove item
curl http://localhost:8082/health     # Health check
```

### Order Service (port 8083)

Order creation and management.

```bash
curl http://localhost:8083/           # List all orders
curl http://localhost:8083/1          # Get order by ID
curl -X POST http://localhost:8083/   # Create new order
curl http://localhost:8083/health     # Health check
```

### User Service (port 8084)

User management with pre-populated users.

```bash
curl http://localhost:8084/           # List all users
curl http://localhost:8084/1          # Get user by ID
curl -X POST http://localhost:8084/   # Create new user
curl http://localhost:8084/health     # Health check
```

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
│   │  :8081   │  │  :8082   │  │  :8083   │  │  :8084   │ │
│   └──────────┘  └──────────┘  └──────────┘  └──────────┘ │
└──────────────────────────────────────────────────────────┘
```

## Running with Fabricks

```bash
# Start the daemon
fabricks daemon start

# Deploy all services
fabricks mortar up

# Test services
curl http://localhost:8080/health
curl http://localhost:8081/
curl http://localhost:8082/1
curl http://localhost:8083/
curl http://localhost:8084/

# Stop
fabricks mortar down
```

## Next Steps

- See [payment-service](../payment-service/) for security isolation patterns
- See [monitoring](../monitoring/) for observability setup
