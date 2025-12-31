# Network Segmentation

Fabricks provides built-in network isolation to secure communication between services.

---

## Overview

Network segmentation is a core security feature in Fabricks. Unlike traditional container networking where all services can talk to each other by default, Fabricks enforces **explicit network boundaries**.

Key principles:
- **Deny by default** - Services cannot communicate unless explicitly allowed
- **Declarative networks** - All networks defined in configuration, no orphans
- **Layered security** - Network membership + capabilities + policies

---

## Defining Networks

Networks are defined in `fabricks-mortar.toml`:

```toml
[network.frontend]
description = "Public-facing tier"
ingress = "0.0.0.0/0"
egress = ["backend"]

[network.backend]
description = "Application logic tier"
internal = true
ingress = ["frontend"]
egress = ["data", "cache"]

[network.data]
description = "Database tier"
internal = true
ingress = ["backend"]
```

---

## Network Types

### Public Networks

Allow external traffic:

```toml
[network.public]
description = "Internet-facing services"
ingress = "0.0.0.0/0"  # Allow from anywhere
egress = ["application"]
```

### Internal Networks

No external access:

```toml
[network.application]
internal = true  # No external ingress
ingress = ["public"]  # Only from public network
egress = ["data", "cache"]
```

### Isolated Networks

Completely isolated from other networks:

```toml
[network.payment]
isolated = true  # Cannot communicate with other internal networks
audit_all = true  # Log all traffic
encryption = "required"
egress = ["external:payment-gateways"]  # Can only reach external hosts
```

### Ingress-Only Networks

Can receive but not initiate connections:

```toml
[network.monitoring]
ingress = ["*"]  # All networks can send metrics
ingress_only = true  # Cannot initiate connections
```

---

## Traffic Flow Control

### Ingress Rules

Control who can connect TO services in this network:

```toml
[network.backend]
# Option 1: Specific networks
ingress = ["frontend", "admin"]

# Option 2: All internal networks
ingress = ["*"]

# Option 3: External CIDR
ingress = "10.0.0.0/8"

# Option 4: Public internet
ingress = "0.0.0.0/0"
```

### Egress Rules

Control what services in this network can connect TO:

```toml
[network.application]
# Internal networks
egress = ["data", "cache", "messaging"]

# External hosts (defined separately)
egress = ["external:payment-apis", "external:smtp"]
```

---

## External Hosts

Define allowed external endpoints:

```toml
[external_hosts.payment-apis]
description = "Payment processing endpoints"
hosts = [
    "api.stripe.com:443",
    "api.paypal.com:443"
]
tls_required = true

[external_hosts.smtp]
description = "Email services"
hosts = ["smtp.sendgrid.net:587"]

[external_hosts.monitoring]
description = "Observability endpoints"
hosts = [
    "api.datadoghq.com:443",
    "otlp.nr-data.net:443"
]
```

Use in networks:
```toml
[network.payment]
egress = ["external:payment-apis"]

[network.workers]
egress = ["external:smtp", "external:monitoring"]
```

---

## Service Assignment

Assign services to networks:

```toml
[service.api]
build = "./services/api"
networks = ["frontend", "backend"]  # Can bridge networks

[service.database]
image = "wasm://pglite:latest"
networks = ["data"]  # Single network

[service.payment]
build = "./services/payment"
networks = ["payment"]  # Isolated network
```

### Bridge Services

Services on multiple networks can act as bridges:

```toml
[service.order]
networks = ["backend", "payment"]  # Bridges backend to payment
```

---

## Network Policies

Enforce additional rules beyond network membership:

```toml
[policy.pci_compliance]
description = "PCI-DSS requirements"

# Deny specific traffic
[[policy.pci_compliance.deny]]
from = ["payment"]
to = ["monitoring"]
reason = "PCI data must not be logged"

[[policy.pci_compliance.deny]]
from = ["payment"]
to = ["external:*"]
except = ["external:payment-apis"]
reason = "Payment zone can only reach approved gateways"

# Require TLS and audit
[[policy.pci_compliance.require]]
networks = ["payment"]
tls = true
audit = true
```

### Deny Rules

Block specific communication:

```toml
[[policy.security.deny]]
from = ["data"]
to = ["external:*"]
reason = "Databases cannot connect externally"

[[policy.security.deny]]
from = ["*"]
to = ["admin"]
except = ["vpn"]
reason = "Admin only accessible via VPN"
```

### Require Rules

Enforce security requirements:

```toml
[[policy.security.require]]
networks = ["payment", "data"]
encryption = true
audit = true

[[policy.security.require]]
services = ["user", "order", "payment"]
tls = true
```

### Warn Rules

Generate warnings without blocking:

```toml
[[policy.security.warn]]
cross_network = true
except = ["order"]  # Expected bridge
```

---

## Common Patterns

### Three-Tier Architecture

```toml
[network.dmz]
description = "Public-facing load balancer"
ingress = "0.0.0.0/0"
egress = ["application"]

[network.application]
description = "Business logic"
internal = true
ingress = ["dmz"]
egress = ["data"]

[network.data]
description = "Databases"
internal = true
ingress = ["application"]
# No egress - data tier is a sink
```

### Microservices with Shared Cache

```toml
[network.services]
internal = true
egress = ["cache", "data"]

[network.cache]
internal = true
ingress = ["services"]

[network.data]
internal = true
ingress = ["services"]

[service.api]
networks = ["services"]

[service.worker]
networks = ["services"]

[service.redis]
networks = ["cache"]

[service.postgres]
networks = ["data"]
```

### PCI-Compliant Payment Processing

```toml
[network.application]
internal = true
egress = ["payment-bridge"]

[network.payment-bridge]
description = "Controlled access to payment"
internal = true
ingress = ["application"]
egress = ["payment"]

[network.payment]
description = "PCI-DSS compliant zone"
isolated = true
audit_all = true
encryption = "required"
ingress = ["payment-bridge"]
egress = ["external:payment-gateways"]

[service.order]
networks = ["application", "payment-bridge"]

[service.payment-processor]
networks = ["payment"]

[service.payment-processor.audit]
enabled = true
log_level = "verbose"
pii_redact = true
```

---

## Inspecting Networks

### List Networks

```bash
fabricks network ls
```

Output:
```
ID          NAME          SERVICES    CREATED
x1y2z3w4    dmz           1           2 hours ago
a2b3c4d5    application   5           2 hours ago
b3c4d5e6    data          2           2 hours ago
c4d5e6f7    payment       2           2 hours ago
```

### Inspect Network

```bash
fabricks network inspect application
```

Output:
```yaml
id: a2b3c4d5
name: application
internal: true
ingress:
  - dmz
egress:
  - data
  - cache
services:
  - id: a1b2c3d4e5f6
    name: product-service
  - id: b2c3d4e5f6a7
    name: user-service
  - id: c3d4e5f6a7b8
    name: cart-service
created_at: 2025-01-15T10:00:00Z
```

### Visualize Network Topology

```bash
fabricks graph --show-networks
```

---

## How It Works

### Connection Validation

When a service attempts to connect, Fabricks validates:

1. **Network membership** - Do both services share a network?
2. **Capability grant** - Does the source have `connect` capability for the target?
3. **Policy check** - Do any policies deny this connection?

```
Connection Request: api → postgres:5432

1. Network Check:
   - api is in [application]
   - postgres is in [data]
   - application has egress to [data] ✓

2. Capability Check:
   - api.capabilities.network.connect includes "postgres:5432" ✓

3. Policy Check:
   - No deny rules match ✓

Result: Connection ALLOWED
```

### Failure Example

```
Connection Request: api → external:8.8.8.8:53

1. Network Check:
   - api is in [application]
   - application egress = [data, cache]
   - No external access ✗

Result: Connection DENIED (network isolation)
```

---

## Best Practices

1. **Minimize network membership** - Services should only be in networks they need
2. **Use internal networks** - Default to `internal = true`
3. **Explicit external hosts** - Define all external endpoints explicitly
4. **Audit sensitive networks** - Enable `audit_all` for compliance zones
5. **Use bridge services carefully** - Document why services need multi-network access
6. **Apply policies** - Enforce TLS and encryption where needed

---

## Related Documentation

- [Capabilities](capabilities.md) - Fine-grained permission model
- [Production](production.md) - Production deployment best practices
- [Fabrickfile Reference](fabrickfile-mortar-reference.md) - Complete configuration reference
