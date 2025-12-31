# Capability Model

Fabricks uses a deny-by-default security model. Services can only access resources they are explicitly granted.

---

## Overview

The capability model is the foundation of Fabricks security. Every resource access—environment variables, network connections, filesystem paths—must be explicitly granted in the Fabrickfile.

**Key principles:**
- **Deny by default** - If not listed, access is denied
- **Least privilege** - Grant only what's needed
- **Explicit grants** - All capabilities declared in configuration
- **Runtime enforcement** - Capabilities enforced by the WASM runtime

---

## Capability Types

### Environment Variables

Control which environment variables a service can read:

```toml
[capabilities]
env = [
    "DATABASE_URL",
    "REDIS_URL",
    "API_KEY",
    "LOG_LEVEL"
]
```

**Behavior:**
- Service can only read variables listed in `env`
- Attempting to read unlisted variables returns empty/error
- Variables are passed at runtime via mortar file or CLI

**Example:**
```toml
# Fabrickfile
[capabilities]
env = ["DATABASE_URL", "LOG_LEVEL"]

# fabricks-mortar.toml
[service.api]
environment = {
    DATABASE_URL = "postgres://db:5432/app",
    LOG_LEVEL = "info",
    SECRET_KEY = "abc123"  # NOT accessible - not in capabilities!
}
```

---

### Network Capabilities

#### Listen Ports

Control which ports the service can bind to:

```toml
[capabilities.network]
listen = [8080, 9090]
```

**Behavior:**
- Service can only listen on listed ports
- Attempting to bind to other ports is denied
- Ports must also be mapped in mortar file to be accessible

#### Connect Hosts

Control outbound connections:

```toml
[capabilities.network]
connect = [
    "postgres:5432",
    "redis:6379",
    "api.stripe.com:443"
]
```

**Format:** `host:port`

**Behavior:**
- Service can only connect to listed host:port combinations
- DNS resolution only works for allowed hosts
- External connections require both capability AND network egress

#### Allow All Outbound (Not Recommended)

```toml
[capabilities.network]
allow_all_outbound = true  # Security risk!
```

**Warning:** This bypasses connection restrictions. Only use for development or when absolutely necessary.

---

### Filesystem Capabilities

Control file access:

```toml
[capabilities.filesystem]
# Read-only access
read = [
    "./config",
    "./templates",
    "/etc/ssl/certs"
]

# Write-only access
write = [
    "./logs",
    "./cache"
]

# Read and write access
read_write = [
    "./data"
]
```

**Behavior:**
- Paths are relative to service root
- Glob patterns supported: `./config/**/*.toml`
- Access outside listed paths is denied
- Parent directory traversal (`../`) is blocked

---

### WASM Feature Capabilities

Enable specific WASM features:

```toml
[capabilities.wasm]
simd = true           # SIMD instructions (performance)
threads = true        # Multi-threading
bulk_memory = true    # Bulk memory operations
```

**Behavior:**
- Features disabled by default
- Enable only if your code requires them
- Some features may have security implications

---

## Capability Inheritance

When using base images, capabilities are inherited and can be extended:

```toml
# Base Fabrickfile
[capabilities]
env = ["LOG_LEVEL"]

[capabilities.network]
listen = [8080]

# Extended Fabrickfile (using base)
[from]
image = "wasm://base-service:v1"

[capabilities]
env = ["LOG_LEVEL", "DATABASE_URL"]  # Extends base

[capabilities.network]
listen = [8080, 9090]  # Extends base
connect = ["postgres:5432"]  # Adds new capability
```

**Rules:**
- Child capabilities are merged with parent
- Child cannot reduce parent capabilities
- Child can add new capabilities

---

## Capability Validation

### Build-Time Validation

Fabricks validates capabilities during build:

```bash
fabricks validate
```

Output:
```
Validating Fabrickfile...
✓ Syntax valid
✓ Capabilities properly defined
⚠ Warning: allow_all_outbound is enabled
✓ Valid!
```

### Runtime Enforcement

Capabilities are enforced by the WASM runtime:

```
# Attempt to read unauthorized env var
[DENIED] env:SECRET_KEY - not in capabilities

# Attempt to connect to unauthorized host
[DENIED] connect:evil.com:443 - not in capabilities

# Attempt to read unauthorized file
[DENIED] read:/etc/passwd - not in capabilities
```

---

## Mortar-Level Overrides

The mortar file can further restrict (but not expand) capabilities:

```toml
[service.api]
build = "./services/api"

# Can restrict environment variables
environment = {
    DATABASE_URL = "postgres://db:5432/app"
    # Only passes DATABASE_URL, even if Fabrickfile allows more
}

# Can restrict network via network membership
networks = ["backend"]
# Even if Fabrickfile allows connect to "external:443",
# service can only reach hosts in "backend" network
```

---

## Security Layers

Fabricks enforces security at multiple layers:

```
Request: api → postgres:5432

Layer 1: Capability Check
├── Does api have connect = ["postgres:5432"]?
└── YES → Continue

Layer 2: Network Check
├── Is api on a network with egress to postgres's network?
└── YES → Continue

Layer 3: Policy Check
├── Do any policies deny this connection?
└── NO → Continue

Result: Connection ALLOWED
```

All three layers must pass for access to be granted.

---

## Common Patterns

### Minimal Web Service

```toml
[capabilities]
env = ["PORT", "LOG_LEVEL"]

[capabilities.network]
listen = [8080]

[capabilities.filesystem]
read = ["./static", "./templates"]
```

### Database-Connected Service

```toml
[capabilities]
env = ["DATABASE_URL", "LOG_LEVEL"]

[capabilities.network]
listen = [8080]
connect = ["postgres:5432"]

[capabilities.filesystem]
read = ["./config", "./migrations"]
```

### Worker with External API Access

```toml
[capabilities]
env = ["API_KEY", "QUEUE_URL"]

[capabilities.network]
# No listen - workers don't expose ports
connect = [
    "rabbitmq:5672",
    "api.external-service.com:443"
]

[capabilities.filesystem]
read = ["./config"]
write = ["./logs"]
```

### Cache-Enabled Service

```toml
[capabilities]
env = ["DATABASE_URL", "REDIS_URL", "LOG_LEVEL"]

[capabilities.network]
listen = [8080]
connect = [
    "postgres:5432",
    "redis:6379"
]

[capabilities.filesystem]
read = ["./config"]
read_write = ["./cache"]
```

### PCI-Compliant Payment Service

```toml
[capabilities]
env = [
    "STRIPE_API_KEY",
    "ENCRYPTION_KEY"
]

[capabilities.network]
listen = [9000]
connect = [
    "api.stripe.com:443"
    # No other connections allowed
]

[capabilities.filesystem]
read = ["./config"]
# No write access - stateless

[capabilities.wasm]
# No special features needed
```

---

## Best Practices

### 1. Start Minimal

Begin with no capabilities and add only what's needed:

```toml
# Start here
[capabilities]
# Empty - add as needed

# Add only what code requires
[capabilities]
env = ["LOG_LEVEL"]  # Added because code reads LOG_LEVEL
```

### 2. Be Specific with Hosts

Use exact host:port combinations:

```toml
# Good - specific
connect = ["postgres:5432", "redis:6379"]

# Bad - too broad
connect = ["*:*"]  # Not even supported!
```

### 3. Separate Read and Write

Use appropriate access levels:

```toml
[capabilities.filesystem]
read = ["./config"]      # Config is read-only
write = ["./logs"]       # Logs are write-only
read_write = ["./data"]  # Data needs both
```

### 4. Document Capabilities

Explain why each capability is needed:

```toml
[capabilities]
# DATABASE_URL - Connection string for PostgreSQL
# API_KEY - Authentication for external API
# LOG_LEVEL - Configure logging verbosity
env = ["DATABASE_URL", "API_KEY", "LOG_LEVEL"]
```

### 5. Review Regularly

Audit capabilities periodically:

```bash
fabricks inspect --show-capabilities ./services/api
```

Remove any capabilities no longer needed.

---

## Troubleshooting

### "Capability denied" Errors

```
Error: CapabilityDenied: env:SECRET_KEY
```

**Solution:** Add the variable to `[capabilities].env`:

```toml
[capabilities]
env = ["SECRET_KEY"]  # Add this
```

### "Connection refused" for Allowed Hosts

Even with correct capabilities, connections can fail if:

1. **Network isolation** - Check service is on correct network
2. **Policy denial** - Check no policies block the connection
3. **Host not running** - Verify target service is healthy

### Debugging Capabilities

```bash
# Inspect service capabilities
fabricks service inspect my-service

# Check what's actually configured
fabricks inspect ./services/my-service --show-capabilities
```

---

## Related Documentation

- [Networking](networking.md) - Network segmentation and policies
- [Fabrickfile Reference](fabrickfile-mortar-reference.md) - Complete configuration options
- [Production](production.md) - Production security practices
