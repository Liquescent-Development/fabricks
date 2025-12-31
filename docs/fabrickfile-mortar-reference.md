# Fabricks Files Documentation

Complete reference for Fabrickfile and fabricks-mortar.toml configuration files.

---

## Table of Contents

- [Fabrickfile Reference](#fabrickfile-reference)
  - [Overview](#fabrickfile-overview)
  - [Complete Example](#complete-fabrickfile-example)
  - [Section Reference](#fabrickfile-section-reference)
  - [Quick Start Examples](#fabrickfile-quick-start-examples)
- [fabricks-mortar.toml Reference](#fabricks-mortartoml-reference)
  - [Overview](#fabricks-mortartoml-overview)
  - [Complete Example](#complete-fabricks-mortartoml-example)
  - [Section Reference](#fabricks-mortartoml-section-reference)
  - [Quick Start Examples](#fabricks-mortartoml-quick-start-examples)

---

# Fabrickfile Reference

## Fabrickfile Overview

A `Fabrickfile` defines a single WASM module or service. It specifies how to build the module, what capabilities it needs, what it exports/imports, and its default configuration.

**Location:** Typically in the root of your service directory (e.g., `./services/product/Fabrickfile`)

**Format:** TOML

---

## Complete Fabrickfile Example
```toml
# Fabrickfile - Complete example showing all options

# Required: Fabrick format version
fabrick_version = "1.0"

# ============================================================================
# METADATA (Required)
# ============================================================================

[info]
# Required: Name of the fabrick (lowercase, hyphens allowed)
name = "product-service"

# Required: Semantic version
version = "2.1.0"

# Optional: Human-readable description
description = "Product catalog service for e-commerce platform"

# Optional: Author information
authors = ["backend-team@acme.com", "platform@acme.com"]

# Optional: License identifier (SPDX format)
license = "MIT"

# Optional: Homepage URL
homepage = "https://github.com/acme/product-service"

# Optional: Repository URL
repository = "https://github.com/acme/product-service"

# Optional: Documentation URL
documentation = "https://docs.acme.com/services/product"

# Optional: Keywords for discoverability
keywords = ["ecommerce", "catalog", "products", "api"]

# ============================================================================
# BASE (Optional - specify starting point)
# ============================================================================

[from]
# Option 1: Build from scratch using a language runtime
source = "rust"  # rust | go | javascript | python | csharp

# Option 2: Build on top of another fabrick (like FROM in Dockerfile)
# image = "wasm://nginx:alpine"

# Option 3: Build on top of a local fabrick
# path = "../base-service"

# ============================================================================
# SOURCE (Required for builds)
# ============================================================================

[source]
# Required: Path to source code (relative to Fabrickfile)
path = "."

# Optional: Files to include in build context (glob patterns)
include = [
    "src/**/*.rs",
    "Cargo.toml",
    "Cargo.lock",
    "config/**/*.toml"
]

# Optional: Files to exclude from build context
exclude = [
    "target/**",
    "*.log",
    ".git/**",
    "tests/**"
]

# ============================================================================
# RUNTIME (Optional - for interpreted languages)
# ============================================================================

[runtime]
# Specify WASM runtime for interpreted languages
image = "wasm://node:20"  # For JavaScript/TypeScript

# Optional: Runtime-specific configuration
# config = { heap_size = "512Mi" }

# ============================================================================
# BUILD (Required for source builds)
# ============================================================================

[build]
# Required: Build command to compile to WASM
command = "cargo build --target wasm32-wasi --release"

# Optional: Working directory for build (relative to Fabrickfile)
# workdir = "."

# Required: Output WASM file path (relative to workdir)
output = "target/wasm32-wasi/release/product_service.wasm"

# Optional: Files to watch for rebuild in dev mode
watch = [
    "src/**/*.rs",
    "Cargo.toml"
]

# Optional: Environment variables for build process
# environment = { RUSTFLAGS = "-C opt-level=z" }

# Optional: Additional build steps (run before main command)
# pre_build = ["cargo fmt --check", "cargo clippy"]

# Optional: Additional build steps (run after main command)
# post_build = ["wasm-opt -Oz -o optimized.wasm output.wasm"]

# ============================================================================
# EXPORTS (Optional - Component Model)
# ============================================================================

# List of functions/interfaces this module exports
exports = [
    "list_products",
    "get_product",
    "create_product",
    "update_product",
    "delete_product",
    "search_products"
]

# Alternative: Export interfaces (Component Model)
# [exports.interface]
# "wasi:http/handler" = { version = "0.2.0" }
# "acme:product/catalog" = { version = "1.0.0" }

# ============================================================================
# IMPORTS (Optional - Component Model)
# ============================================================================

[imports]
# Import from registry
search_indexer = "wasm://search/indexer:v1.0"
cache_client = "wasm://redis-client:v2.0"

# Import from local fabrick
# auth = { path = "../auth-lib" }

# Import specific interface version
# logger = { image = "wasm://logger:v1", interface = "wasi:logging/logger@0.1.0" }

# ============================================================================
# CAPABILITIES (Required)
# ============================================================================

[capabilities]
# List of environment variables this module can access
env = [
    "DATABASE_URL",
    "REDIS_URL",
    "LOG_LEVEL",
    "API_KEY"
]

# Network capabilities
[capabilities.network]
# Ports this module listens on
listen = [8080]

# Hosts/ports this module can connect to
connect = [
    "postgres:5432",
    "redis:6379",
    "search-service:8083"
]

# Optional: Allow all outbound connections (not recommended)
# allow_all_outbound = false

# Filesystem capabilities
[capabilities.filesystem]
# Paths with read access (relative to module root)
read = [
    "./config",
    "./templates",
    "./static"
]

# Paths with write access
# write = ["./logs", "./cache"]

# Paths with read+write access
# read_write = ["./data"]

# Optional: WASM-specific features
[capabilities.wasm]
# Enable WASM SIMD instructions (for performance)
# simd = false

# Enable WASM threads
# threads = false

# Enable bulk memory operations
# bulk_memory = false

# ============================================================================
# FILES (Optional - include static files)
# ============================================================================

# Copy files into the module at runtime
[files]
# Key = source path (relative to Fabrickfile)
# Value = destination path (absolute in module)
"./config/app.toml" = "/etc/product/config.toml"
"./templates/" = "/templates"
"./static/" = "/static"

# Glob patterns supported
# "./migrations/*.sql" = "/migrations/"

# ============================================================================
# CONFIG (Optional - default configuration)
# ============================================================================

[config]
# Default port (can be overridden in mortar file)
port = 8080

# Default timeout in seconds
timeout = 30

# Default log level
log_level = "info"

# Environment variables with defaults
[config.environment]
LOG_LEVEL = "info"
LOG_FORMAT = "json"
RUST_BACKTRACE = "0"

# Resource defaults
[config.resources]
memory = "256Mi"  # Memory limit (Mi, Gi)
cpu = 0.5         # CPU cores (fractional allowed)

# ============================================================================
# HEALTH CHECK (Recommended)
# ============================================================================

# HTTP-based health check
[health_check.http]
# Required: Path to health endpoint
path = "/health"

# Optional: Port (defaults to first listen port)
port = 8080

# Optional: Interval between checks
interval = "30s"

# Optional: Timeout for each check
timeout = "5s"

# Optional: Number of consecutive failures before unhealthy
retries = 3

# Optional: HTTP method
method = "GET"

# Optional: Expected status code
expected_status = 200

# Alternative: TCP-based health check
# [health_check.tcp]
# port = 8080
# interval = "10s"
# timeout = "3s"

# Alternative: Exec-based health check (run command in module)
# [health_check.exec]
# command = ["./healthcheck.sh"]
# interval = "30s"
# timeout = "5s"

# ============================================================================
# SECURITY (Optional)
# ============================================================================

[security]
# Run as non-root user
# user = "appuser"

# Deny all network by default (must explicitly allow)
# deny_by_default = true

# Read-only root filesystem
# read_only_root = false

# Drop all capabilities except those specified
# drop_capabilities = ["ALL"]

# ============================================================================
# METADATA FOR REGISTRY (Optional)
# ============================================================================

[labels]
# Arbitrary key-value labels for organization
"team" = "backend"
"tier" = "application"
"compliance" = "pci-dss"
"cost-center" = "engineering"

# ============================================================================
# VALIDATION (Optional)
# ============================================================================

[validate]
# Ensure exported functions are actually present
check_exports = true

# Ensure imported modules are available
check_imports = true

# Check for known vulnerabilities
scan_vulnerabilities = true
```

---

## Fabrickfile Section Reference

### `fabrick_version` (Required)

Specifies the Fabrickfile format version.

- **Type:** String
- **Required:** Yes
- **Example:** `fabrick_version = "1.0"`

---

### `[info]` (Required)

Metadata about the fabrick.

#### `name` (Required)
- **Type:** String
- **Pattern:** `[a-z0-9-]+` (lowercase, numbers, hyphens)
- **Example:** `name = "product-service"`

#### `version` (Required)
- **Type:** String
- **Format:** Semantic versioning (MAJOR.MINOR.PATCH)
- **Example:** `version = "2.1.0"`

#### `description` (Optional)
- **Type:** String
- **Example:** `description = "Product catalog service"`

#### `authors` (Optional)
- **Type:** Array of strings
- **Example:** `authors = ["team@acme.com", "John Doe <john@acme.com>"]`

#### `license` (Optional)
- **Type:** String (SPDX identifier)
- **Examples:** `license = "MIT"`, `license = "Apache-2.0"`, `license = "GPL-3.0-or-later"`

#### `homepage`, `repository`, `documentation` (Optional)
- **Type:** String (URL)
- **Examples:**
```toml
  homepage = "https://acme.com/product-service"
  repository = "https://github.com/acme/product-service"
  documentation = "https://docs.acme.com/services/product"
```

#### `keywords` (Optional)
- **Type:** Array of strings
- **Example:** `keywords = ["ecommerce", "catalog", "api"]`

---

### `[from]` (Optional)

Specifies the base to build upon (like `FROM` in Dockerfile). Mutually exclusive options:

#### Option 1: Language Runtime
```toml
[from]
source = "rust"  # rust | go | javascript | python | csharp
```

#### Option 2: Pre-built Image
```toml
[from]
image = "wasm://nginx:alpine"
```

#### Option 3: Local Fabrick
```toml
[from]
path = "../base-service"
```

**If omitted:** Builds from scratch

---

### `[source]` (Required for builds)

Specifies source code location and files to include.

#### `path` (Required)
- **Type:** String (relative path)
- **Example:** `path = "."` or `path = "./src"`

#### `include` (Optional)
- **Type:** Array of strings (glob patterns)
- **Default:** All files
- **Example:**
```toml
  include = [
      "src/**/*.rs",
      "Cargo.toml",
      "Cargo.lock"
  ]
```

#### `exclude` (Optional)
- **Type:** Array of strings (glob patterns)
- **Example:**
```toml
  exclude = [
      "target/**",
      "*.log",
      ".git/**",
      "node_modules/**"
  ]
```

---

### `[runtime]` (Optional)

Required for interpreted languages (JavaScript, Python).

#### `image` (Required if using runtime)
- **Type:** String
- **Examples:** `image = "wasm://node:20"`, `image = "wasm://python:3.11"`

#### `config` (Optional)
- **Type:** Table
- **Example:**
```toml
  [runtime.config]
  heap_size = "512Mi"
  stack_size = "2Mi"
```

---

### `[build]` (Required for source builds)

Specifies how to build the WASM module.

#### `command` (Required)
- **Type:** String
- **Examples:**
```toml
  command = "cargo build --target wasm32-wasi --release"
  command = "GOOS=wasip1 GOARCH=wasm go build -o app.wasm"
  command = "npm run build:wasm"
```

#### `output` (Required)
- **Type:** String (relative path)
- **Examples:**
```toml
  output = "target/wasm32-wasi/release/app.wasm"
  output = "dist/bundle.wasm"
```

#### `workdir` (Optional)
- **Type:** String (relative path)
- **Default:** `.` (Fabrickfile directory)
- **Example:** `workdir = "./backend"`

#### `watch` (Optional)
- **Type:** Array of strings (glob patterns)
- **Purpose:** Files to watch for auto-rebuild in dev mode
- **Example:** `watch = ["src/**/*.rs", "Cargo.toml"]`

#### `environment` (Optional)
- **Type:** Table
- **Purpose:** Environment variables for build process
- **Example:**
```toml
  [build.environment]
  RUSTFLAGS = "-C opt-level=z"
  CGO_ENABLED = "0"
```

#### `pre_build` (Optional)
- **Type:** Array of strings
- **Purpose:** Commands to run before main build
- **Example:**
```toml
  pre_build = [
      "cargo fmt --check",
      "cargo clippy -- -D warnings"
  ]
```

#### `post_build` (Optional)
- **Type:** Array of strings
- **Purpose:** Commands to run after main build
- **Example:**
```toml
  post_build = [
      "wasm-opt -Oz -o optimized.wasm output.wasm",
      "wasm-strip optimized.wasm"
  ]
```

---

### `exports` (Optional)

Functions or interfaces this module provides (Component Model).

- **Type:** Array of strings
- **Example:**
```toml
  exports = [
      "handle_request",
      "process_payment",
      "validate_user"
  ]
```

**Alternative: Interface exports**
```toml
[exports.interface]
"wasi:http/handler" = { version = "0.2.0" }
"acme:product/catalog" = { version = "1.0.0" }
```

---

### `[imports]` (Optional)

Dependencies on other WASM modules (Component Model).

- **Type:** Table
- **Examples:**
```toml
  [imports]
  # From registry
  logger = "wasm://logger:v1.0"
  cache = "wasm://redis-client:v2"
  
  # From local fabrick
  auth = { path = "../auth-lib" }
  
  # With specific interface version
  database = {
      image = "wasm://postgres-client:v1",
      interface = "wasi:sql/query@0.1.0"
  }
```

---

### `[capabilities]` (Required)

Specifies what resources the module can access.

#### `env` (Optional)
- **Type:** Array of strings
- **Purpose:** Environment variables the module can read
- **Example:**
```toml
  [capabilities]
  env = [
      "DATABASE_URL",
      "API_KEY",
      "LOG_LEVEL"
  ]
```

#### `[capabilities.network]` (Optional)

##### `listen` (Optional)
- **Type:** Array of integers
- **Purpose:** Ports the module can listen on
- **Example:** `listen = [8080, 9090]`

##### `connect` (Optional)
- **Type:** Array of strings
- **Format:** `host:port`
- **Purpose:** Hosts/ports the module can connect to
- **Example:**
```toml
  connect = [
      "postgres:5432",
      "redis:6379",
      "api.stripe.com:443"
  ]
```

##### `allow_all_outbound` (Optional)
- **Type:** Boolean
- **Default:** `false`
- **Warning:** Not recommended for security

#### `[capabilities.filesystem]` (Optional)

##### `read` (Optional)
- **Type:** Array of strings (paths)
- **Example:** `read = ["./config", "./templates"]`

##### `write` (Optional)
- **Type:** Array of strings (paths)
- **Example:** `write = ["./logs", "./cache"]`

##### `read_write` (Optional)
- **Type:** Array of strings (paths)
- **Example:** `read_write = ["./data"]`

#### `[capabilities.wasm]` (Optional)

WASM-specific feature flags.
```toml
[capabilities.wasm]
simd = true          # Enable SIMD instructions
threads = true       # Enable threading
bulk_memory = true   # Enable bulk memory operations
```

---

### `[files]` (Optional)

Static files to include with the module.

- **Type:** Table (source → destination mapping)
- **Example:**
```toml
  [files]
  "./config/app.toml" = "/etc/app/config.toml"
  "./templates/" = "/templates"
  "./static/**/*.css" = "/static/css/"
```

---

### `[config]` (Optional)

Default configuration (can be overridden in mortar file).

- **Type:** Table
- **Example:**
```toml
  [config]
  port = 8080
  timeout = 30
  log_level = "info"
  
  [config.environment]
  LOG_FORMAT = "json"
  RUST_BACKTRACE = "0"
  
  [config.resources]
  memory = "256Mi"
  cpu = 0.5
```

---

### `[health_check]` (Recommended)

Health check configuration.

#### HTTP Health Check
```toml
[health_check.http]
path = "/health"              # Required
port = 8080                   # Optional (defaults to first listen port)
interval = "30s"              # Optional (default: 30s)
timeout = "5s"                # Optional (default: 5s)
retries = 3                   # Optional (default: 3)
method = "GET"                # Optional (default: GET)
expected_status = 200         # Optional (default: 200)
```

#### TCP Health Check
```toml
[health_check.tcp]
port = 8080                   # Required
interval = "10s"              # Optional
timeout = "3s"                # Optional
```

#### Exec Health Check
```toml
[health_check.exec]
command = ["./healthcheck.sh"]  # Required
interval = "30s"                # Optional
timeout = "5s"                  # Optional
```

---

### `[security]` (Optional)

Security hardening options.
```toml
[security]
user = "appuser"              # Run as specific user
deny_by_default = true        # Deny all access unless explicitly allowed
read_only_root = true         # Make root filesystem read-only
drop_capabilities = ["ALL"]   # Drop all Linux capabilities
```

---

### `[labels]` (Optional)

Arbitrary key-value metadata.

- **Type:** Table
- **Example:**
```toml
  [labels]
  team = "backend"
  tier = "application"
  cost-center = "engineering"
```

---

### `[validate]` (Optional)

Validation rules to run during build.
```toml
[validate]
check_exports = true          # Verify exported functions exist
check_imports = true          # Verify imported modules available
scan_vulnerabilities = true   # Check for known CVEs
```

---

## Fabrickfile Quick Start Examples

### Minimal Fabrickfile (Rust)
```toml
fabrick_version = "1.0"

[info]
name = "my-service"
version = "1.0.0"

[from]
source = "rust"

[source]
path = "."

[build]
command = "cargo build --target wasm32-wasi --release"
output = "target/wasm32-wasi/release/my_service.wasm"

[capabilities.network]
listen = [8080]
```

### Minimal Fabrickfile (Pre-built Image)
```toml
fabrick_version = "1.0"

[info]
name = "redis-cache"
version = "1.0.0"

[from]
image = "wasm://redis:7.2"

[capabilities.network]
listen = [6379]

[capabilities.filesystem]
read_write = ["./data"]
```

### Minimal Fabrickfile (JavaScript)
```toml
fabrick_version = "1.0"

[info]
name = "api-gateway"
version = "1.0.0"

[from]
source = "javascript"

[runtime]
image = "wasm://node:20"

[source]
path = "."

[build]
command = "npm run build:wasm"
output = "dist/api.wasm"

[capabilities.network]
listen = [3000]
connect = ["backend:8080"]
```

---

# fabricks-mortar.toml Reference

## fabricks-mortar.toml Overview

A `fabricks-mortar.toml` file composes multiple Fabricks into a complete application. It defines services, networks, volumes, secrets, and policies.

**Location:** Typically in your project root (e.g., `./fabricks-mortar.toml`)

**Format:** TOML

---

## Complete fabricks-mortar.toml Example
```toml
# fabricks-mortar.toml - Complete example showing all options

# Required: Mortar format version
mortar_version = "1.0"

# ============================================================================
# PROJECT (Required)
# ============================================================================

[project]
# Required: Project name
name = "acme-shop"

# Optional: Project version
version = "2.1.0"

# Optional: Description
description = "E-commerce platform built on WASM"

# Optional: Authors
authors = ["platform-team@acme.com"]

# ============================================================================
# VARIABLES (Optional)
# ============================================================================

[variable.environment]
type = "string"
default = "production"
description = "Deployment environment"
# allowed_values = ["development", "staging", "production"]

[variable.log_level]
type = "string"
default = "info"
description = "Log level for all services"

[variable.database_pool_size]
type = "number"
default = 20
description = "Database connection pool size"

[variable.enable_debug]
type = "boolean"
default = false
description = "Enable debug mode"

# ============================================================================
# SECRETS (Optional)
# ============================================================================

# From Vault
[secret.database_password]
provider = "vault"
path = "secret/data/postgres"
key = "password"  # Optional: specific key in secret

# From environment variable
[secret.api_key]
provider = "env"
key = "API_KEY"

# From file
[secret.tls_cert]
provider = "file"
path = "./secrets/tls.crt"

# ============================================================================
# NETWORKS (Optional)
# ============================================================================

[network.dmz]
description = "Public-facing services"
# Allow inbound from anywhere
ingress = "0.0.0.0/0"
# Can egress to application network
egress = ["application"]

[network.application]
description = "Business logic tier"
# Internal only (no external ingress)
internal = true
# Can access multiple networks
ingress = ["dmz"]
egress = ["data", "cache", "messaging"]

[network.data]
description = "Database tier"
internal = true
# Only application tier can access
ingress = ["application"]
# Can only send metrics to monitoring
egress = ["monitoring"]

[network.payment]
description = "PCI-compliant payment processing"
# Completely isolated
isolated = true
# Audit all traffic
audit_all = true
# Require TLS for all connections
encryption = "required"
# Can only reach external payment gateways
egress = ["external:payment-gateways"]

[network.monitoring]
description = "Observability"
# Can receive from all networks
ingress = ["*"]
# But cannot initiate connections
ingress_only = true
egress = ["external:observability"]

# ============================================================================
# EXTERNAL HOSTS (Optional)
# ============================================================================

[external_hosts.payment-gateways]
description = "Payment processing endpoints"
hosts = [
    "api.stripe.com:443",
    "api.braintreegateway.com:443"
]
tls_required = true

[external_hosts.smtp]
description = "Email services"
hosts = ["smtp.sendgrid.net:587"]

# ============================================================================
# SERVICES - Build from Fabrickfile
# ============================================================================

[service.product]
# Build from local Fabrickfile
build = "./services/product"

# Optional: Override fabrick name/version
# name = "product-service"
# version = "2.1.0"

# Required: Networks this service belongs to
networks = ["application"]

# Optional: Override environment from Fabrickfile
environment = {
    DATABASE_URL = "postgres://postgres:5432/products?pool_size=${variable.database_pool_size}",
    REDIS_URL = "redis://redis:6379/0",
    LOG_LEVEL = "${variable.log_level}"
}

# Optional: Override port mappings
# ports = ["8080:8080"]  # host:container

# Optional: Override resource limits
[service.product.resources]
memory = "512Mi"
cpu = 1.0

# Optional: Scaling configuration
[service.product.replicas]
min = 2
max = 10
# Optional: Autoscaling based on CPU
cpu_threshold = 70

# Optional: Override health check
[service.product.health_check.http]
path = "/health"
interval = "30s"

# Optional: Service dependencies (start order)
depends_on = ["postgres", "redis"]

# Optional: Restart policy
[service.product.restart]
policy = "on-failure"  # always | on-failure | never
max_attempts = 3
backoff = "10s"

# Optional: Labels
[service.product.labels]
tier = "application"
team = "backend"

# ============================================================================
# SERVICES - From pre-built image
# ============================================================================

[service.redis]
# Use pre-built image from registry
image = "wasm://redis:7.2"

networks = ["cache"]

# Optional: Override ports
# ports = ["6379:6379"]

# Optional: Volumes
[service.redis.volumes]
redis_data = "/data"

# Optional: Persistence configuration
[service.redis.persistence]
enabled = true
strategy = "aof"  # aof | rdb | both

[service.redis.replicas]
min = 2
max = 2

# ============================================================================
# SERVICES - With imports/exports (Component Model)
# ============================================================================

[service.order]
build = "./services/order"

networks = ["application", "payment"]  # Bridge between networks

# Component Model: Import from other services
[service.order.imports]
payment_processor = { service = "payment", interface = "process-payment" }
inventory = { service = "inventory", interface = "check-stock" }

# Component Model: Export interfaces
[service.order.exports]
interfaces = ["acme:order/service@1.0.0"]

environment = {
    DATABASE_URL = "postgres://postgres:5432/orders",
    PAYMENT_ENDPOINT = "http://payment:9000"
}

[service.order.replicas]
min = 3
max = 20

# ============================================================================
# SERVICES - Worker pattern (no ports)
# ============================================================================

[service.email-worker]
build = "./workers/email"

networks = ["workers"]

# Workers don't listen on ports (no [ports] section)

# But can connect to message queue
environment = {
    NATS_URL = "nats://nats:4222",
    QUEUE_SUBJECT = "emails",
    SENDGRID_API_KEY = "${secret.sendgrid_api_key}"
}

[service.email-worker.replicas]
min = 2
max = 20

# ============================================================================
# SERVICES - Database
# ============================================================================

[service.postgres]
image = "wasm://electric-sql/pglite:v0.2"

networks = ["data"]

environment = {
    POSTGRES_PASSWORD = "${secret.database_password}",
    POSTGRES_DB = "acme_shop"
}

[service.postgres.volumes]
postgres_data = "/var/lib/postgresql/data"

# Backup configuration
[service.postgres.backup]
enabled = true
schedule = "0 2 * * *"  # Cron format: daily at 2 AM
retention = "7d"
destination = "s3://backups/postgres"

# Single instance (no replication in this example)
[service.postgres.replicas]
min = 1
max = 1

[service.postgres.resources]
memory = "4Gi"
cpu = 2.0

# ============================================================================
# SERVICES - With audit and security
# ============================================================================

[service.payment]
image = "wasm://payment/stripe-processor:v3.2"

# Isolated in payment network
networks = ["payment"]

environment = {
    STRIPE_API_KEY = "${secret.stripe_api_key}",
    STRIPE_WEBHOOK_SECRET = "${secret.stripe_webhook}"
}

# No autoscaling in PCI zone
[service.payment.replicas]
min = 3
max = 3

# Enhanced audit logging
[service.payment.audit]
enabled = true
log_level = "verbose"
pii_redact = true

# Extra security
[service.payment.security]
egress_locked = true  # Only explicitly allowed hosts
secrets_encrypted = true
tls_required = true
read_only_root = true

# ============================================================================
# VOLUMES (Optional)
# ============================================================================

[volume.postgres_data]
# Size specification
size = "50Gi"

# Optional: Volume type
# type = "persistent"  # persistent | ephemeral

# Optional: Storage class (Kubernetes)
# storage_class = "fast-ssd"

# Optional: Access mode
# access_mode = "read-write-once"  # read-write-once | read-only-many | read-write-many

# Optional: Backup configuration
[volume.postgres_data.backup]
enabled = true
schedule = "0 3 * * *"
retention = "30d"

[volume.redis_data]
size = "10Gi"

[volume.search_data]
size = "20Gi"

# Encrypted volume
[volume.vault_data]
size = "5Gi"
encrypted = true

# ============================================================================
# POLICIES (Optional)
# ============================================================================

[policy.pci_compliance]
description = "Enforce PCI-DSS requirements"

# Deny rules
[[policy.pci_compliance.deny]]
from = ["payment"]
to = ["monitoring"]
reason = "PCI data must not be logged"

[[policy.pci_compliance.deny]]
from = ["payment"]
to = ["external:*"]
except = ["external:payment-gateways"]
reason = "Payment zone can only reach approved gateways"

# Require rules
[[policy.pci_compliance.require]]
networks = ["payment"]
tls = true
audit = true

[policy.data_protection]
description = "Prevent data exfiltration"

[[policy.data_protection.deny]]
from = ["data"]
to = ["external:*"]
reason = "Databases cannot connect externally"

[[policy.data_protection.require]]
services = ["user", "order"]
encryption = true

[policy.least_privilege]
description = "Minimize cross-network communication"

# Warn on cross-network connections (except bridges)
[[policy.least_privilege.warn]]
cross_network = true
except = ["order"]  # order bridges app to payment

# ============================================================================
# VALIDATION (Optional)
# ============================================================================

[validate]
# Require all services to have health checks
require_health_checks = true

# Deny wildcard network connections
deny_wildcard_connect = true

# All services must specify explicit capabilities
require_explicit_capabilities = true

# Warn on deprecated WASM versions
warn_on_old_wasm_versions = true

# Check dependencies for known vulnerabilities
scan_dependencies = true

# Check for circular dependencies
check_circular_dependencies = true

# ============================================================================
# DEPLOYMENT PROFILES (Optional)
# ============================================================================

[profile.local]
description = "Local development"
target = "localhost"

# Override replicas for local
[profile.local.overrides]
all_services = { replicas = { min = 1, max = 1 } }

# Use smaller resources
all_services = { resources = { memory = "128Mi", cpu = 0.25 } }

[profile.staging]
description = "Staging environment"
target = "kubernetes"
cluster = "staging-cluster"
namespace = "acme-shop-staging"

[profile.staging.overrides]
# Smaller resources for staging
all_services = { resources = { memory = "256Mi", cpu = 0.5 } }

[profile.production]
description = "Production environment"
target = "kubernetes"
cluster = "prod-cluster"
namespace = "acme-shop"

# High availability settings
[profile.production.settings]
high_availability = true
enable_monitoring = true
enable_tracing = true

# Require manual approval for critical services
[profile.production.approval]
required_for = ["payment", "order"]
approvers = ["platform-team@acme.com"]
```

---

## fabricks-mortar.toml Section Reference

### `mortar_version` (Required)

Specifies the mortar file format version.

- **Type:** String
- **Required:** Yes
- **Example:** `mortar_version = "1.0"`

---

### `[project]` (Required)

Project-level metadata.

#### `name` (Required)
- **Type:** String
- **Example:** `name = "acme-shop"`

#### `version` (Optional)
- **Type:** String (semantic version)
- **Example:** `version = "2.1.0"`

#### `description`, `authors` (Optional)
- **Type:** String / Array of strings
- **Examples:**
```toml
  description = "E-commerce platform"
  authors = ["platform@acme.com"]
```

---

### `[variable.*]` (Optional)

Define reusable variables.

**Fields:**
- `type` (Required): `"string"` | `"number"` | `"boolean"`
- `default` (Optional): Default value
- `description` (Optional): Human-readable description
- `allowed_values` (Optional): Array of allowed values

**Example:**
```toml
[variable.environment]
type = "string"
default = "production"
description = "Deployment environment"
allowed_values = ["dev", "staging", "production"]

[variable.replicas]
type = "number"
default = 3
```

**Usage:**
```toml
environment = { ENV = "${variable.environment}" }
```

---

### `[secret.*]` (Optional)

Define secrets from various providers.

#### Vault Provider
```toml
[secret.db_password]
provider = "vault"
path = "secret/data/postgres"
key = "password"  # Optional: specific key
```

#### Environment Variable Provider
```toml
[secret.api_key]
provider = "env"
key = "API_KEY"
```

#### File Provider
```toml
[secret.tls_cert]
provider = "file"
path = "./secrets/tls.crt"
```

**Usage:**
```toml
environment = { DB_PASS = "${secret.db_password}" }
```

---

### `[network.*]` (Optional)

Define network segments for service isolation.

**Fields:**
- `description` (Optional): Description of network purpose
- `internal` (Optional, Boolean): No external access (default: false)
- `isolated` (Optional, Boolean): Completely isolated from other networks
- `ingress` (Optional): Who can connect IN (Array of network names or CIDR)
- `egress` (Optional): Who can connect OUT (Array of network names or external hosts)
- `ingress_only` (Optional, Boolean): Can receive but not initiate connections
- `audit_all` (Optional, Boolean): Log all traffic
- `encryption` (Optional): `"required"` | `"optional"`

**Examples:**
```toml
# Public-facing network
[network.dmz]
description = "Public services"
ingress = "0.0.0.0/0"  # Allow from anywhere
egress = ["application"]

# Internal application network
[network.application]
internal = true  # No external access
ingress = ["dmz"]
egress = ["data", "cache"]

# Isolated payment zone
[network.payment]
isolated = true  # Cannot talk to other networks
audit_all = true
encryption = "required"

# Monitoring network (ingress only)
[network.monitoring]
ingress = ["*"]  # All networks can send
ingress_only = true  # But can't initiate
```

---

### `[external_hosts.*]` (Optional)

Define allowed external hosts for egress.

**Fields:**
- `description` (Optional): Description
- `hosts` (Required): Array of `host:port` strings
- `tls_required` (Optional, Boolean): Require TLS

**Example:**
```toml
[external_hosts.payment-gateways]
description = "Payment APIs"
hosts = [
    "api.stripe.com:443",
    "api.paypal.com:443"
]
tls_required = true
```

**Usage in network:**
```toml
[network.payment]
egress = ["external:payment-gateways"]
```

---

### `[service.*]` (Required)

Define services to run.

#### Basic Service Fields

##### `build` OR `image` (Required - mutually exclusive)
```toml
# Option 1: Build from Fabrickfile
[service.api]
build = "./services/api"

# Option 2: Use pre-built image
[service.redis]
image = "wasm://redis:7.2"
```

##### `networks` (Required)
- **Type:** Array of strings (network names)
- **Example:** `networks = ["application", "cache"]`

##### `environment` (Optional)
- **Type:** Table
- **Example:**
```toml
  environment = {
      DATABASE_URL = "postgres://db:5432/myapp",
      LOG_LEVEL = "${variable.log_level}",
      API_KEY = "${secret.api_key}"
  }
```

##### `ports` (Optional)
- **Type:** Array of strings
- **Format:** `"host:service"` or `"port"` (same on both sides)
- **Example:** `ports = ["8080:8080", "9090"]`

##### `depends_on` (Optional)
- **Type:** Array of strings (service names)
- **Purpose:** Start order dependencies
- **Example:** `depends_on = ["postgres", "redis"]`

#### `[service.*.resources]` (Optional)

Resource limits.
```toml
[service.api.resources]
memory = "512Mi"  # Memory limit
cpu = 1.0         # CPU cores (fractional allowed)
```

#### `[service.*.replicas]` (Optional)

Scaling configuration.
```toml
[service.api.replicas]
min = 2                # Minimum instances
max = 10               # Maximum instances
cpu_threshold = 70     # Autoscale trigger (optional)
```

#### `[service.*.volumes]` (Optional)

Mount volumes.

- **Type:** Table (volume_name → mount_path)
- **Example:**
```toml
  [service.postgres.volumes]
  postgres_data = "/var/lib/postgresql/data"
  redis_data = "/data"
```

#### `[service.*.files]` (Optional)

Mount individual files.

- **Type:** Table (source → destination)
- **Example:**
```toml
  [service.nginx.files]
  "./nginx.conf" = "/etc/nginx/nginx.conf"
  "./certs/" = "/etc/nginx/certs/"
```

#### `[service.*.health_check]` (Optional)

Override health check from Fabrickfile.
```toml
[service.api.health_check.http]
path = "/health"
interval = "30s"
timeout = "5s"
retries = 3

# Or TCP
[service.redis.health_check.tcp]
port = 6379
interval = "10s"

# Or exec
[service.custom.health_check.exec]
command = ["./check.sh"]
interval = "30s"
```

#### `[service.*.restart]` (Optional)

Restart policy.
```toml
[service.api.restart]
policy = "on-failure"  # always | on-failure | never
max_attempts = 3
backoff = "10s"
```

#### `[service.*.imports]` (Optional)

Component Model imports from other services.
```toml
[service.order.imports]
payment = { service = "payment", interface = "process-payment" }
inventory = { service = "inventory", interface = "check-stock" }
```

#### `[service.*.exports]` (Optional)

Component Model exports.
```toml
[service.order.exports]
interfaces = ["acme:order/service@1.0.0"]
```

#### `[service.*.persistence]` (Optional)

Persistence configuration.
```toml
[service.redis.persistence]
enabled = true
strategy = "aof"  # aof | rdb | both
```

#### `[service.*.backup]` (Optional)

Backup configuration.
```toml
[service.postgres.backup]
enabled = true
schedule = "0 2 * * *"  # Cron format
retention = "7d"
destination = "s3://backups/postgres"
```

#### `[service.*.audit]` (Optional)

Audit logging.
```toml
[service.payment.audit]
enabled = true
log_level = "verbose"  # minimal | standard | verbose
pii_redact = true
```

#### `[service.*.security]` (Optional)

Security hardening.
```toml
[service.payment.security]
egress_locked = true      # Only allowed hosts
secrets_encrypted = true
tls_required = true
read_only_root = true
user = "appuser"
```

#### `[service.*.labels]` (Optional)

Arbitrary labels.
```toml
[service.api.labels]
team = "backend"
tier = "application"
```

---

### `[volume.*]` (Optional)

Define persistent volumes.

**Fields:**
- `size` (Required): Volume size (`"10Gi"`, `"500Mi"`)
- `type` (Optional): `"persistent"` | `"ephemeral"` (default: persistent)
- `storage_class` (Optional): Storage class name (K8s)
- `access_mode` (Optional): `"read-write-once"` | `"read-only-many"` | `"read-write-many"`
- `encrypted` (Optional, Boolean): Enable encryption

**Example:**
```toml
[volume.postgres_data]
size = "50Gi"
type = "persistent"
encrypted = true

[volume.postgres_data.backup]
enabled = true
schedule = "0 3 * * *"
retention = "30d"
```

---

### `[policy.*]` (Optional)

Security and compliance policies.

#### Deny Rules
```toml
[policy.my_policy]
description = "Policy description"

[[policy.my_policy.deny]]
from = ["payment"]
to = ["monitoring"]
reason = "No PCI data to logs"

[[policy.my_policy.deny]]
from = ["data"]
to = ["external:*"]
except = ["external:backup"]
reason = "No external connections from data"
```

#### Require Rules
```toml
[[policy.my_policy.require]]
networks = ["payment"]
tls = true
audit = true
encryption = true

[[policy.my_policy.require]]
services = ["user", "order"]
encryption = true
```

#### Warn Rules
```toml
[[policy.my_policy.warn]]
cross_network = true
except = ["order"]  # Exceptions
```

---

### `[validate]` (Optional)

Validation rules.
```toml
[validate]
require_health_checks = true          # All services must have health checks
deny_wildcard_connect = true          # No wildcard hosts
require_explicit_capabilities = true  # Must specify capabilities
warn_on_old_wasm_versions = true     # Warn on outdated WASM
scan_dependencies = true              # Check for CVEs
check_circular_dependencies = true    # Detect circular deps
```

---

### `[profile.*]` (Optional)

Deployment profiles for different environments.
```toml
[profile.local]
description = "Local development"
target = "localhost"

[profile.local.overrides]
# Apply to all services
all_services = {
    replicas = { min = 1, max = 1 },
    resources = { memory = "128Mi", cpu = 0.25 }
}

# Override specific service
[profile.local.overrides.postgres]
replicas = { min = 1, max = 1 }

[profile.production]
target = "kubernetes"
cluster = "prod-cluster"
namespace = "acme-shop"

[profile.production.settings]
high_availability = true
enable_monitoring = true

[profile.production.approval]
required_for = ["payment", "order"]
approvers = ["platform-team@acme.com"]
```

---

## fabricks-mortar.toml Quick Start Examples

### Minimal mortar file
```toml
mortar_version = "1.0"

[project]
name = "my-app"

[service.api]
build = "./api"
networks = ["default"]

[service.postgres]
image = "wasm://pglite:latest"
networks = ["default"]

[network.default]
internal = true
```

### Simple web app
```toml
mortar_version = "1.0"

[project]
name = "blog"

[service.web]
build = "./web"
ports = ["3000:3000"]
networks = ["public"]

[service.api]
build = "./api"
networks = ["public", "backend"]
environment = { DATABASE_URL = "postgres://db:5432/blog" }

[service.db]
image = "wasm://pglite:latest"
networks = ["backend"]

[service.db.volumes]
db_data = "/data"

[network.public]
ingress = "0.0.0.0/0"

[network.backend]
internal = true

[volume.db_data]
size = "10Gi"
```

---
