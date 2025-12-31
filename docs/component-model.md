# Component Model

Fabricks supports the WASM Component Model for composing services through direct function calls.

---

## Overview

The WASM Component Model allows services to import and export interfaces, enabling direct function calls between modules without network overhead. Think of it as linking libraries, but across service boundaries.

**Benefits:**
- **Zero network overhead** - Direct function calls, no HTTP/gRPC
- **Type safety** - Interface contracts enforced at link time
- **Composability** - Build complex systems from simple components
- **Reusability** - Share functionality across services

---

## Exports

Exports define functions or interfaces your service provides to others.

### Simple Exports

List of functions:

```toml
# Fabrickfile
exports = [
    "list_products",
    "get_product",
    "create_product",
    "update_product",
    "delete_product"
]
```

These functions can be called directly by other services that import them.

### Interface Exports

Using WIT (WASM Interface Types):

```toml
[exports.interface]
"wasi:http/handler" = { version = "0.2.0" }
"acme:product/catalog" = { version = "1.0.0" }
```

This exports standardized interfaces that other components can depend on.

---

## Imports

Imports declare dependencies on other services' exported functions.

### From Registry

```toml
[imports]
# Import from registry image
logger = "wasm://logger:v1.0"
cache = "wasm://redis-client:v2.0"
```

### From Local Fabrick

```toml
[imports]
auth = { path = "../auth-lib" }
utils = { path = "../shared/utils" }
```

### With Specific Interface

```toml
[imports]
database = {
    image = "wasm://postgres-client:v1",
    interface = "wasi:sql/query@0.1.0"
}
```

---

## Service-to-Service Imports

In a mortar file, you can import interfaces from other services:

```toml
# fabricks-mortar.toml

[service.order]
build = "./services/order"
networks = ["application"]

# Import from other services in this composition
[service.order.imports]
payment = { service = "payment", interface = "process-payment" }
inventory = { service = "inventory", interface = "check-stock" }
user = { service = "user", interface = "get-user" }

# Export interfaces for others to import
[service.order.exports]
interfaces = ["acme:order/service@1.0.0"]
```

### How It Works

1. **Link time** - Fabricks resolves imports to actual service implementations
2. **Runtime** - Calls are made directly via Component Model, not HTTP
3. **Type checking** - Interface compatibility verified before deployment

---

## Writing Components

### Rust Example

Define a component that exports functions:

```rust
// src/lib.rs
wit_bindgen::generate!({
    world: "product-service",
    exports: {
        "acme:product/catalog": ProductCatalog,
    },
});

struct ProductCatalog;

impl acme::product::catalog::Guest for ProductCatalog {
    fn list_products() -> Vec<Product> {
        // Implementation
    }

    fn get_product(id: u32) -> Option<Product> {
        // Implementation
    }

    fn create_product(product: Product) -> u32 {
        // Implementation
    }
}
```

### Importing in Rust

```rust
// src/lib.rs
wit_bindgen::generate!({
    world: "order-service",
    imports: {
        "acme:inventory/stock": inventory,
        "acme:payment/processor": payment,
    },
});

fn process_order(order: Order) -> Result<OrderResult, Error> {
    // Direct function call - no network!
    let available = inventory::check_stock(order.product_id, order.quantity);

    if !available {
        return Err(Error::OutOfStock);
    }

    // Another direct call
    let payment_result = payment::process(order.payment_info)?;

    Ok(OrderResult { ... })
}
```

---

## WIT Interfaces

Define interfaces using WIT (WASM Interface Types):

```wit
// product.wit
package acme:product@1.0.0;

interface catalog {
    record product {
        id: u32,
        name: string,
        price: float64,
        description: string,
    }

    list-products: func() -> list<product>;
    get-product: func(id: u32) -> option<product>;
    create-product: func(product: product) -> u32;
    update-product: func(id: u32, product: product) -> bool;
    delete-product: func(id: u32) -> bool;
}

world product-service {
    export catalog;
}
```

```wit
// order.wit
package acme:order@1.0.0;

interface service {
    record order {
        id: u32,
        product-id: u32,
        quantity: u32,
        user-id: u32,
    }

    record order-result {
        order-id: u32,
        status: string,
    }

    create-order: func(order: order) -> result<order-result, string>;
    get-order: func(id: u32) -> option<order>;
}

world order-service {
    import acme:product/catalog@1.0.0;
    import acme:inventory/stock@1.0.0;
    import acme:payment/processor@1.0.0;

    export service;
}
```

---

## Standard Interfaces (WASI)

Fabricks supports WASI (WebAssembly System Interface) standard interfaces:

### HTTP Handler

```toml
[exports.interface]
"wasi:http/handler" = { version = "0.2.0" }
```

### Filesystem

```toml
[imports]
filesystem = { interface = "wasi:filesystem/types@0.2.0" }
```

### Sockets

```toml
[imports]
sockets = { interface = "wasi:sockets/tcp@0.2.0" }
```

### Clocks

```toml
[imports]
clocks = { interface = "wasi:clocks/monotonic-clock@0.2.0" }
```

---

## Composition Example

A complete e-commerce system using Component Model:

```toml
# fabricks-mortar.toml

mortar_version = "1.0"

[project]
name = "e-commerce"

# User Service - exports user interface
[service.user]
build = "./services/user"
networks = ["application"]

[service.user.exports]
interfaces = ["acme:user/service@1.0.0"]

# Product Service - exports catalog interface
[service.product]
build = "./services/product"
networks = ["application"]

[service.product.exports]
interfaces = ["acme:product/catalog@1.0.0"]

# Inventory Service - exports stock interface
[service.inventory]
build = "./services/inventory"
networks = ["application"]

[service.inventory.exports]
interfaces = ["acme:inventory/stock@1.0.0"]

# Payment Service - exports processor interface
[service.payment]
build = "./services/payment"
networks = ["payment"]

[service.payment.exports]
interfaces = ["acme:payment/processor@1.0.0"]

# Order Service - imports from multiple services
[service.order]
build = "./services/order"
networks = ["application", "payment"]  # Bridge to payment

[service.order.imports]
product = { service = "product", interface = "acme:product/catalog@1.0.0" }
inventory = { service = "inventory", interface = "acme:inventory/stock@1.0.0" }
payment = { service = "payment", interface = "acme:payment/processor@1.0.0" }
user = { service = "user", interface = "acme:user/service@1.0.0" }

[service.order.exports]
interfaces = ["acme:order/service@1.0.0"]

# API Gateway - imports all services
[service.api]
build = "./services/api"
networks = ["public", "application"]
ports = ["8080:8080"]

[service.api.imports]
user = { service = "user", interface = "acme:user/service@1.0.0" }
product = { service = "product", interface = "acme:product/catalog@1.0.0" }
order = { service = "order", interface = "acme:order/service@1.0.0" }
```

### Dependency Graph

```
        ┌─────────┐
        │   api   │
        └────┬────┘
             │ imports
    ┌────────┼────────┐
    ▼        ▼        ▼
┌──────┐ ┌───────┐ ┌───────┐
│ user │ │product│ │ order │
└──────┘ └───────┘ └───┬───┘
                       │ imports
              ┌────────┼────────┬────────┐
              ▼        ▼        ▼        ▼
          ┌──────┐ ┌───────┐ ┌─────────┐ ┌────────┐
          │ user │ │product│ │inventory│ │payment │
          └──────┘ └───────┘ └─────────┘ └────────┘
```

---

## Validation

Fabricks validates component compatibility:

```bash
fabricks validate
```

Checks:
- All imports have matching exports
- Interface versions are compatible
- No circular dependencies
- Types match across boundaries

---

## Performance Considerations

### Direct Calls vs HTTP

| Method | Latency | Overhead |
|--------|---------|----------|
| Component Model | ~1μs | Near zero |
| HTTP (same host) | ~100μs | Serialization |
| HTTP (network) | ~1ms+ | Network + serialization |

### When to Use Component Model

**Good for:**
- Frequently called functions
- Low-latency requirements
- Tightly coupled services
- Shared libraries

**Consider HTTP for:**
- Cross-cluster communication
- Load-balanced services
- Independent scaling
- External APIs

---

## Best Practices

1. **Define clear interfaces** - Use WIT to create explicit contracts
2. **Version interfaces** - Use semantic versioning for compatibility
3. **Keep interfaces small** - Focused interfaces are easier to maintain
4. **Document contracts** - Include descriptions in WIT files
5. **Test interfaces** - Verify compatibility in CI/CD

---

## Related Documentation

- [Fabrickfile Reference](fabrickfile-mortar-reference.md) - Configuration options
- [Architecture](architecture.md) - System design patterns
- [Tutorial](tutorial.md) - Build a complete application
