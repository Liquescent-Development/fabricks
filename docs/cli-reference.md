# Fabricks CLI Reference

Complete command-line interface documentation for `fabricks`.

---

## Table of Contents

- [Installation](#installation)
- [Global Options](#global-options)
- [Core Commands](#core-commands)
  - [fabricks build](#fabricks-build)
  - [fabricks run](#fabricks-run)
  - [fabricks push](#fabricks-push)
  - [fabricks pull](#fabricks-pull)
- [Mortar Commands (Composition)](#mortar-commands-composition)
  - [fabricks mortar build](#fabricks-mortar-build)
  - [fabricks mortar up](#fabricks-mortar-up)
  - [fabricks mortar down](#fabricks-mortar-down)
  - [fabricks mortar ps](#fabricks-mortar-ps)
  - [fabricks mortar logs](#fabricks-mortar-logs)
  - [fabricks mortar restart](#fabricks-mortar-restart)
  - [fabricks mortar scale](#fabricks-mortar-scale)
  - [fabricks mortar exec](#fabricks-mortar-exec)
- [Service Commands (Inspection)](#service-commands-inspection)
  - [fabricks service ls](#fabricks-service-ls)
  - [fabricks service inspect](#fabricks-service-inspect)
  - [fabricks service logs](#fabricks-service-logs)
  - [fabricks service stats](#fabricks-service-stats)
- [Network Commands (Inspection)](#network-commands-inspection)
  - [fabricks network ls](#fabricks-network-ls)
  - [fabricks network inspect](#fabricks-network-inspect)
- [Volume Commands (Inspection)](#volume-commands-inspection)
  - [fabricks volume ls](#fabricks-volume-ls)
  - [fabricks volume inspect](#fabricks-volume-inspect)
- [Events & Monitoring](#events--monitoring)
  - [fabricks events](#fabricks-events)
- [Daemon Commands](#daemon-commands)
  - [fabricks daemon start](#fabricks-daemon-start)
  - [fabricks daemon stop](#fabricks-daemon-stop)
  - [fabricks daemon status](#fabricks-daemon-status)
  - [fabricks daemon logs](#fabricks-daemon-logs)
  - [fabricks daemon reload](#fabricks-daemon-reload)
  - [fabricks daemon info](#fabricks-daemon-info)
  - [fabricks daemon stats](#fabricks-daemon-stats)
- [Development Commands](#development-commands)
  - [fabricks dev](#fabricks-dev)
  - [fabricks watch](#fabricks-watch)
- [Validation & Inspection](#validation--inspection)
  - [fabricks validate](#fabricks-validate)
  - [fabricks inspect](#fabricks-inspect)
  - [fabricks graph](#fabricks-graph)
- [Registry Commands](#registry-commands)
  - [fabricks login](#fabricks-login)
  - [fabricks logout](#fabricks-logout)
  - [fabricks search](#fabricks-search)
- [Utility Commands](#utility-commands)
  - [fabricks init](#fabricks-init)
  - [fabricks version](#fabricks-version)
  - [fabricks clean](#fabricks-clean)
- [Kubernetes Integration](#kubernetes-integration)
  - [fabricks k8s generate](#fabricks-k8s-generate)
  - [fabricks k8s apply](#fabricks-k8s-apply)
- [Complete Examples](#complete-examples)

---

## Installation

### From Binary (Recommended)
```bash
# Linux/macOS
curl -fsSL https://get.fabricks.dev | sh

# Or download directly
curl -LO https://github.com/fabricks/fabricks/releases/latest/download/fabricks-$(uname -s)-$(uname -m)
chmod +x fabricks-*
sudo mv fabricks-* /usr/local/bin/fabricks
```

### From Source (Rust)
```bash
cargo install fabricks
```

### Verify Installation
```bash
fabricks version
# fabricks 1.0.0
```

---

## Global Options

Options available for all commands:
```
OPTIONS:
    -h, --help              Print help information
    -V, --version           Print version information
    -v, --verbose           Enable verbose logging
    -q, --quiet             Suppress output except errors
        --color <WHEN>      Control color output [default: auto] [possible: auto, always, never]
        --config <PATH>     Path to config file [default: ~/.fabricks/config.toml]
```

**Example:**
```bash
fabricks --verbose build ./services/api
fabricks --quiet mortar up
```

---

## Core Commands

### `fabricks build`

Build a WASM module from a Fabrickfile.

**Usage:**
```bash
fabricks build [OPTIONS] <PATH>
```

**Arguments:**
- `<PATH>` - Path to directory containing Fabrickfile (default: current directory)

**Options:**
```
    -f, --file <FILE>           Fabrickfile to use [default: Fabrickfile]
    -t, --tag <TAG>             Tag for the built image (can be used multiple times)
        --no-cache              Don't use build cache
        --progress <TYPE>       Progress output type [default: auto] [possible: auto, plain, tty]
        --platform <PLATFORM>   Target platform (wasm32-wasi, wasm32-unknown-unknown)
    -o, --output <PATH>         Output path for WASM file
        --build-arg <KEY=VAL>   Set build-time variables (can be used multiple times)
```

**Examples:**
```bash
# Build from current directory
fabricks build

# Build from specific directory
fabricks build ./services/api

# Build with custom Fabrickfile
fabricks build -f Fabrickfile.prod

# Build and tag
fabricks build -t my-service:v1.0.0 -t my-service:latest

# Build without cache
fabricks build --no-cache ./services/api

# Build with build arguments
fabricks build --build-arg RUSTFLAGS="-C opt-level=z" ./services/api

# Build and output to specific location
fabricks build -o ./dist/api.wasm ./services/api
```

**Output:**
```
Building product-service:2.1.0...
[1/4] Parsing Fabrickfile
[2/4] Preparing build context (12 files)
[3/4] Compiling to WASM
      Running: cargo build --target wasm32-wasi --release
      Finished release [optimized] target(s) in 45.2s
[4/4] Optimizing WASM module
✓ Built product-service.wasm (2.3 MB)
```

---

### `fabricks run`

Run a WASM module or fabrick.

**Usage:**
```bash
fabricks run [OPTIONS] <IMAGE|PATH|WASM>
```

**Arguments:**
- `<IMAGE|PATH|WASM>` - Image name, path to Fabrickfile directory, or WASM file

**Options:**
```
    -p, --port <HOST:CONTAINER>     Publish ports (can be used multiple times)
    -e, --env <KEY=VAL>             Set environment variables (can be used multiple times)
        --env-file <FILE>           Read environment from file
    -v, --volume <SRC:DEST>         Mount volumes (can be used multiple times)
        --name <NAME>               Assign a name to the instance
        --network <NETWORK>         Connect to network
        --rm                        Automatically remove when stopped
    -d, --detach                    Run in background
        --restart <POLICY>          Restart policy [default: no] [possible: no, on-failure, always]
```

**Examples:**
```bash
# Run from Fabrickfile in current directory
fabricks run .

# Run from specific directory
fabricks run ./services/api

# Run from registry image
fabricks run wasm://redis:7.2

# Run local WASM file
fabricks run ./dist/api.wasm

# Run with port mapping
fabricks run -p 8080:8080 ./services/api

# Run with environment variables
fabricks run -e DATABASE_URL=postgres://localhost/mydb -e LOG_LEVEL=debug ./services/api

# Run with volumes
fabricks run -v ./data:/app/data ./services/api

# Run in background
fabricks run -d --name my-api ./services/api

# Run with auto-restart
fabricks run --restart on-failure ./services/api
```

**Output:**
```
Starting product-service...
✓ Loaded product-service.wasm (2.3 MB)
✓ Network: listen on 0.0.0.0:8080
✓ Health check passed
Listening on http://0.0.0.0:8080
```

---

### `fabricks push`

Push a fabrick to a registry.

**Usage:**
```bash
fabricks push [OPTIONS] <IMAGE>
```

**Arguments:**
- `<IMAGE>` - Image name with tag (e.g., `registry.io/myorg/service:v1.0.0`)

**Options:**
```
    -f, --file <FILE>       Fabrickfile to push [default: Fabrickfile]
        --insecure          Allow insecure registry connections
```

**Examples:**
```bash
# Push to default registry
fabricks push my-service:v1.0.0

# Push to specific registry
fabricks push registry.acme.io/api-service:v2.1.0

# Push from specific Fabrickfile
fabricks push -f ./services/api/Fabrickfile my-service:v1.0.0
```

**Output:**
```
Pushing my-service:v1.0.0 to registry.acme.io...
✓ Uploading WASM module (2.3 MB)
✓ Uploading metadata
✓ Creating manifest
Digest: sha256:abc123def456...
```

---

### `fabricks pull`

Pull a fabrick from a registry.

**Usage:**
```bash
fabricks pull [OPTIONS] <IMAGE>
```

**Arguments:**
- `<IMAGE>` - Image name with tag

**Options:**
```
        --platform <PLATFORM>   Pull for specific platform
        --insecure              Allow insecure registry connections
```

**Examples:**
```bash
# Pull from registry
fabricks pull wasm://redis:7.2

# Pull from specific registry
fabricks pull registry.acme.io/my-service:v1.0.0

# Pull with insecure connection (for private registries)
fabricks pull --insecure localhost:5000/my-service:dev
```

**Output:**
```
Pulling wasm://redis:7.2...
✓ Downloaded manifest
✓ Downloaded WASM module (4.1 MB)
✓ Verified signature
redis:7.2: Pulled
```

---

## Mortar Commands (Composition)

These commands work with `fabricks-mortar.toml` files to manage multi-service applications declaratively.

### `fabricks mortar build`

Build all services defined in fabricks-mortar.toml.

**Usage:**
```bash
fabricks mortar build [OPTIONS] [SERVICES...]
```

**Arguments:**
- `[SERVICES...]` - Specific services to build (builds all if omitted)

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
        --parallel          Build services in parallel
        --no-cache          Don't use build cache
        --pull              Always pull base images
```

**Examples:**
```bash
# Build all services
fabricks mortar build

# Build specific services
fabricks mortar build api worker

# Build in parallel
fabricks mortar build --parallel

# Build without cache
fabricks mortar build --no-cache
```

**Output:**
```
Building 8 services...
[1/8] ✓ postgres (pulled from registry)
[2/8] ✓ redis (pulled from registry)
[3/8] ⚙ product-service (building...)
      Running: cargo build --target wasm32-wasi --release
      Finished in 42.3s
[4/8] ⚙ user-service (building...)
...
✓ Built 8 services in 2m 15s
```

---

### `fabricks mortar up`

Start services defined in fabricks-mortar.toml.

**Usage:**
```bash
fabricks mortar up [OPTIONS] [SERVICES...]
```

**Arguments:**
- `[SERVICES...]` - Specific services to start (starts all if omitted)

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
    -d, --detach            Run in background
        --build             Build images before starting
        --force-recreate    Recreate services even if unchanged
        --no-deps           Don't start dependencies
        --scale <SVC=NUM>   Scale service to NUM instances
        --remove-orphans    Remove services not in mortar file
```

**Examples:**
```bash
# Start all services
fabricks mortar up

# Start in background
fabricks mortar up -d

# Build and start
fabricks mortar up --build

# Start specific services
fabricks mortar up api postgres

# Start with scaling
fabricks mortar up --scale api=5

# Start without dependencies
fabricks mortar up --no-deps api
```

**Output:**
```
Starting acme-shop...
✓ Creating networks (4)
✓ Creating volumes (3)
✓ Starting services in dependency order...
  → postgres (1/1) ✓
  → redis (2/2) ✓
  → product-service (2/2) ✓
  → user-service (2/2) ✓
  → cart-service (2/2) ✓
  → order-service (3/3) ✓
  → payment-processor (3/3) ✓
  → notification-service (2/2) ✓

All services healthy!
Access points:
  → Frontend: http://localhost:3000
  → API: http://localhost:8080
  → Grafana: http://localhost:3001
```

---

### `fabricks mortar down`

Stop and remove services.

**Usage:**
```bash
fabricks mortar down [OPTIONS]
```

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
    -v, --volumes           Remove volumes
        --remove-orphans    Remove services not in mortar file
    -t, --timeout <SEC>     Timeout for graceful shutdown [default: 10]
```

**Examples:**
```bash
# Stop all services
fabricks mortar down

# Stop and remove volumes
fabricks mortar down -v

# Stop with custom timeout
fabricks mortar down -t 30
```

**Output:**
```
Stopping acme-shop...
✓ Stopping services (8)
✓ Removing services
✓ Removing networks (4)
Done!
```

---

### `fabricks mortar ps`

List running services from mortar composition.

**Usage:**
```bash
fabricks mortar ps [OPTIONS]
```

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
    -a, --all               Show all services (including stopped)
    -q, --quiet             Only show IDs
        --format <FORMAT>   Format output [possible: table, json, yaml]
```

**Examples:**
```bash
# List running services
fabricks mortar ps

# List all services
fabricks mortar ps -a

# Output as JSON
fabricks mortar ps --format json
```

**Output:**
```
NAME                STATUS      REPLICAS    PORTS                    NETWORKS
postgres            running     1/1         5432/tcp                 data
redis               running     2/2         6379/tcp                 cache
product-service     running     2/2         0.0.0.0:8080->8080/tcp  application
user-service        running     2/2         8081/tcp                 application
cart-service        running     2/2         8082/tcp                 application
order-service       running     3/3         8085/tcp                 application, payment
payment-processor   running     3/3         9000/tcp                 payment
notification-svc    running     2/2         8086/tcp                 application
```

---

### `fabricks mortar logs`

View service logs from mortar composition.

**Usage:**
```bash
fabricks mortar logs [OPTIONS] [SERVICES...]
```

**Arguments:**
- `[SERVICES...]` - Services to show logs for (all if omitted)

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
        --follow            Follow log output
        --tail <N>          Number of lines to show from end [default: all]
        --since <TIME>      Show logs since timestamp or duration (e.g., 2h, 1m30s)
        --timestamps        Show timestamps
        --no-color          Disable color output
```

**Examples:**
```bash
# View all logs
fabricks mortar logs

# Follow logs
fabricks mortar logs --follow

# Logs for specific service
fabricks mortar logs api

# Last 100 lines
fabricks mortar logs --tail 100

# Logs from last hour
fabricks mortar logs --since 1h

# Multiple services with timestamps
fabricks mortar logs --timestamps api worker
```

**Output:**
```
product-service-1  | 2025-01-15T10:23:45Z [INFO] Starting server on :8080
product-service-1  | 2025-01-15T10:23:45Z [INFO] Connected to database
product-service-2  | 2025-01-15T10:23:46Z [INFO] Starting server on :8080
user-service-1     | 2025-01-15T10:23:47Z [INFO] Auth service ready
order-service-1    | 2025-01-15T10:23:48Z [INFO] Connected to payment processor
```

---

### `fabricks mortar restart`

Restart services in mortar composition.

**Usage:**
```bash
fabricks mortar restart [OPTIONS] [SERVICES...]
```

**Arguments:**
- `[SERVICES...]` - Services to restart (all if omitted)

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
    -t, --timeout <SEC>     Timeout for graceful shutdown [default: 10]
```

**Examples:**
```bash
# Restart all services
fabricks mortar restart

# Restart specific service
fabricks mortar restart api

# Restart with custom timeout
fabricks mortar restart -t 30 api
```

**Output:**
```
Restarting services...
✓ api (2/2 instances)
Done!
```

---

### `fabricks mortar scale`

Scale services to specified number of instances.

**Usage:**
```bash
fabricks mortar scale [OPTIONS] <SERVICE=REPLICAS>...
```

**Arguments:**
- `<SERVICE=REPLICAS>` - Service name and replica count (e.g., `api=5`)

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
    -t, --timeout <SEC>     Timeout for operations [default: 10]
```

**Examples:**
```bash
# Scale single service
fabricks mortar scale api=5

# Scale multiple services
fabricks mortar scale api=5 worker=10 cache=3

# Scale down
fabricks mortar scale api=1
```

**Output:**
```
Scaling services...
✓ api: 2 → 5 instances
✓ worker: 3 → 10 instances
Done!
```

---

### `fabricks mortar exec`

Execute command in running service.

**Usage:**
```bash
fabricks mortar exec [OPTIONS] <SERVICE> <COMMAND> [ARGS...]
```

**Arguments:**
- `<SERVICE>` - Service name
- `<COMMAND>` - Command to execute
- `[ARGS...]` - Command arguments

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
    -i, --interactive       Keep STDIN open
    -t, --tty               Allocate pseudo-TTY
    -e, --env <KEY=VAL>     Set environment variables
        --index <N>         Execute on specific replica [default: 0]
```

**Examples:**
```bash
# Execute command
fabricks mortar exec api ps aux

# Interactive shell
fabricks mortar exec -it api /bin/sh

# Execute on specific replica
fabricks mortar exec --index 2 api ls /data

# Execute with environment
fabricks mortar exec -e DEBUG=1 api ./debug-script.sh
```

---

## Service Commands (Inspection)

These commands inspect services managed by the daemon. Services are created declaratively via `fabricks mortar up`, but these commands let you inspect their state.

### `fabricks service run`

Run a WASM module via the daemon (resolves and stores in OCI registry if needed).

**Usage:**
```bash
fabricks service run [OPTIONS] <PATH|TAG>
```

**Arguments:**
- `<PATH|TAG>` - Path to Fabrickfile directory or OCI image tag

**Options:**
```
    -p, --port <HOST:CONTAINER>     Publish ports (can be used multiple times)
    -e, --env <KEY=VAL>             Set environment variables (can be used multiple times)
        --env-file <FILE>           Read environment from file
    -v, --volume <SRC:DEST>         Mount volumes (can be used multiple times)
        --name <NAME>               Assign a name to the instance
        --network <NETWORK>         Connect to network
        --rm                        Automatically remove when stopped
    -d, --detach                    Run in background
        --restart <POLICY>          Restart policy [default: no] [possible: no, on-failure, always]
```

**How it Works:**

1. **Resolution Phase:**
   - If `<PATH|TAG>` is a directory path, checks for Fabrickfile
   - If Fabrickfile exists and module not built, builds it
   - If `<PATH|TAG>` is an OCI tag, pulls from registry if not cached
   - Stores/verifies module in local OCI storage

2. **Execution Phase:**
   - Daemon loads module from OCI storage
   - For interpreted runtimes (JS/Python), extracts source layer to temp directory
   - Mounts source files at `/app` via WASI preopens
   - Executes WASM module with specified capabilities and configuration

**Examples:**
```bash
# Run from Fabrickfile directory (builds if needed)
fabricks service run ./examples/nodejs-hello

# Run by OCI tag
fabricks service run nodejs-hello:1.0.0

# Run with port mapping
fabricks service run -p 8080:8089 ./examples/nodejs-hello

# Run with environment variables
fabricks service run -e LOG_LEVEL=debug nodejs-hello:1.0.0

# Run in background with auto-restart
fabricks service run -d --restart on-failure nodejs-hello:1.0.0
```

**Output:**
```
Resolving nodejs-hello...
✓ Found Fabrickfile at ./examples/nodejs-hello
✓ Building module (not found in OCI storage)
  [1/3] Resolving runtime: javascript:20
  [2/3] Packaging source files
  [3/3] Storing in OCI registry as nodejs-hello:1.0.0
✓ Stored: sha256:abc123...

Starting service via daemon...
✓ Service ID: srv_xyz789
✓ Network: listen on 0.0.0.0:8089
✓ Health check passed
Service running: nodejs-hello (nodejs-hello:1.0.0)
```

**Notes:**
- This command always runs through the daemon (unlike `fabricks run` which can run standalone)
- Supports interpreted runtimes (JavaScript, Python) via multi-layer OCI images
- For build failures, see `fabricks build --help` for troubleshooting

---

### `fabricks service ls`

List all running services (across all mortar compositions).

**Usage:**
```bash
fabricks service ls [OPTIONS]
```

**Options:**
```
    -a, --all               Show all services (including stopped)
    -q, --quiet             Only show IDs
        --filter <FILTER>   Filter by state, network, or label
        --format <FORMAT>   Format output [possible: table, json, yaml]
```

**Examples:**
```bash
# List all running services
fabricks service ls

# List all services including stopped
fabricks service ls -a

# Filter by state
fabricks service ls --filter state=running

# Filter by network
fabricks service ls --filter network=application

# Output as JSON
fabricks service ls --format json
```

**Output:**
```
ID              NAME                STATE     REPLICAS  CREATED
a1b2c3d4e5f6    product-service     running   2/2       2 hours ago
b2c3d4e5f6a7    user-service        running   2/2       2 hours ago
c3d4e5f6a7b8    cart-service        running   2/2       2 hours ago
d4e5f6a7b8c9    order-service       running   3/3       2 hours ago
e5f6a7b8c9d0    payment-processor   running   3/3       2 hours ago
```

---

### `fabricks service inspect`

Inspect detailed information about a service.

**Usage:**
```bash
fabricks service inspect [OPTIONS] <SERVICE>
```

**Arguments:**
- `<SERVICE>` - Service ID or name

**Options:**
```
        --format <FORMAT>   Output format [default: yaml] [possible: yaml, json]
```

**Examples:**
```bash
# Inspect service
fabricks service inspect product-service

# Inspect with JSON output
fabricks service inspect --format json a1b2c3d4e5f6
```

**Output:**
```yaml
id: a1b2c3d4e5f6
name: product-service
image: wasm://my-api:v1.0.0
state: running
networks:
  - application
  - cache
environment:
  DATABASE_URL: postgres://postgres:5432/products
  LOG_LEVEL: info
ports:
  - host: 8080
    container: 8080
replicas:
  desired: 2
  running: 2
  healthy: 2
  instances:
    - id: product-service-0
      state: running
      health: healthy
      started_at: 2025-01-15T10:23:45Z
      restarts: 0
    - id: product-service-1
      state: running
      health: healthy
      started_at: 2025-01-15T10:23:46Z
      restarts: 0
resources:
  memory:
    limit: 512Mi
    usage: 245Mi
  cpu:
    limit: 1.0
    usage: 0.35
created_at: 2025-01-15T10:23:45Z
```

---

### `fabricks service logs`

View logs for a specific service.

**Usage:**
```bash
fabricks service logs [OPTIONS] <SERVICE>
```

**Arguments:**
- `<SERVICE>` - Service ID or name

**Options:**
```
        --follow            Follow log output
        --tail <N>          Number of lines to show from end [default: all]
        --since <TIME>      Show logs since timestamp or duration
        --timestamps        Show timestamps
        --instance <ID>     Show logs from specific instance
```

**Examples:**
```bash
# View service logs
fabricks service logs product-service

# Follow logs
fabricks service logs --follow product-service

# Last 100 lines
fabricks service logs --tail 100 product-service

# Logs from specific instance
fabricks service logs --instance product-service-0 product-service
```

**Output:**
```
2025-01-15T10:23:45Z [INFO] Starting server on :8080
2025-01-15T10:23:45Z [INFO] Connected to database
2025-01-15T10:24:00Z [INFO] Request: GET /products
2025-01-15T10:24:01Z [INFO] Response: 200 OK (15ms)
```

---

### `fabricks service stats`

View resource usage statistics for a service.

**Usage:**
```bash
fabricks service stats [OPTIONS] <SERVICE>
```

**Arguments:**
- `<SERVICE>` - Service ID or name

**Options:**
```
        --stream            Stream stats continuously
        --format <FORMAT>   Output format [default: table] [possible: table, json]
```

**Examples:**
```bash
# View service stats
fabricks service stats product-service

# Stream stats continuously
fabricks service stats --stream product-service

# Output as JSON
fabricks service stats --format json product-service
```

**Output:**
```
SERVICE           CPU %    MEM USAGE / LIMIT     MEM %    NET I/O
product-service   35.0%    245Mi / 512Mi         47.9%    1.0MB / 2.0MB
```

---

## Network Commands (Inspection)

These commands inspect networks created by mortar compositions. Networks are defined in `fabricks-mortar.toml` and created by `fabricks mortar up`.

### `fabricks network ls`

List all networks.

**Usage:**
```bash
fabricks network ls [OPTIONS]
```

**Options:**
```
    -q, --quiet             Only show IDs
        --format <FORMAT>   Format output [possible: table, json, yaml]
```

**Examples:**
```bash
# List all networks
fabricks network ls

# Output as JSON
fabricks network ls --format json
```

**Output:**
```
ID          NAME          SERVICES    CREATED
x1y2z3w4    dmz           1           2 hours ago
a2b3c4d5    application   5           2 hours ago
b3c4d5e6    data          2           2 hours ago
c4d5e6f7    payment       2           2 hours ago
```

---

### `fabricks network inspect`

Inspect detailed information about a network.

**Usage:**
```bash
fabricks network inspect [OPTIONS] <NETWORK>
```

**Arguments:**
- `<NETWORK>` - Network ID or name

**Options:**
```
        --format <FORMAT>   Output format [default: yaml] [possible: yaml, json]
```

**Examples:**
```bash
# Inspect network
fabricks network inspect application

# Inspect with JSON output
fabricks network inspect --format json a2b3c4d5
```

**Output:**
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

---

## Volume Commands (Inspection)

These commands inspect volumes created by mortar compositions. Volumes are defined in `fabricks-mortar.toml` and created by `fabricks mortar up`.

### `fabricks volume ls`

List all volumes.

**Usage:**
```bash
fabricks volume ls [OPTIONS]
```

**Options:**
```
    -q, --quiet             Only show IDs
        --format <FORMAT>   Format output [possible: table, json, yaml]
```

**Examples:**
```bash
# List all volumes
fabricks volume ls

# Output as JSON
fabricks volume ls --format json
```

**Output:**
```
ID          NAME            SIZE     USED      MOUNTS    CREATED
p1q2r3s4    postgres-data   50Gi     12.3Gi    1         2 hours ago
q2r3s4t5    redis-data      10Gi     2.1Gi     2         2 hours ago
r3s4t5u6    search-data     20Gi     8.5Gi     1         2 hours ago
```

---

### `fabricks volume inspect`

Inspect detailed information about a volume.

**Usage:**
```bash
fabricks volume inspect [OPTIONS] <VOLUME>
```

**Arguments:**
- `<VOLUME>` - Volume ID or name

**Options:**
```
        --format <FORMAT>   Output format [default: yaml] [possible: yaml, json]
```

**Examples:**
```bash
# Inspect volume
fabricks volume inspect postgres-data

# Inspect with JSON output
fabricks volume inspect --format json p1q2r3s4
```

**Output:**
```yaml
id: p1q2r3s4
name: postgres-data
size: 50Gi
used: 12.3Gi
encrypted: true
mounted_by:
  - service_id: f6a7b8c9d0e1
    service_name: postgres
    mount_point: /var/lib/postgresql/data
backups:
  enabled: true
  last_backup: 2025-01-15T03:00:00Z
  next_backup: 2025-01-16T03:00:00Z
  schedule: "0 3 * * *"
  retention: 30d
created_at: 2025-01-15T10:00:00Z
```

---

## Events & Monitoring

### `fabricks events`

Stream real-time events from the daemon.

**Usage:**
```bash
fabricks events [OPTIONS]
```

**Options:**
```
        --types <TYPES>     Filter event types (comma-separated)
        --service <SVC>     Filter by service
        --since <TIME>      Only events after timestamp
        --format <FORMAT>   Output format [default: pretty] [possible: pretty, json]
```

**Event Types:**
- `service.created` - Service was created
- `service.started` - Service instance started
- `service.stopped` - Service instance stopped
- `service.failed` - Service instance failed
- `service.scaled` - Service was scaled
- `health.changed` - Service health status changed
- `network.created` - Network was created
- `volume.created` - Volume was created
- `policy.violated` - Security policy was violated

**Examples:**
```bash
# Stream all events
fabricks events

# Stream service events only
fabricks events --types service.started,service.failed

# Stream events for specific service
fabricks events --service product-service

# Stream events since 1 hour ago
fabricks events --since 1h

# Output as JSON
fabricks events --format json
```

**Output:**
```
2025-01-15T10:23:45Z [service.started] product-service-0 started
2025-01-15T10:23:46Z [service.started] product-service-1 started
2025-01-15T10:24:00Z [health.changed] product-service-0: healthy
2025-01-15T10:24:01Z [health.changed] product-service-1: healthy
2025-01-15T12:20:00Z [service.scaled] product-service: 2 → 5 replicas
2025-01-15T12:20:01Z [service.started] product-service-2 started
```

---

## Daemon Commands

### `fabricks daemon start`

Start the Fabricks daemon.

**Usage:**
```bash
fabricks daemon start [OPTIONS]
```

**Options:**
```
        --config <FILE>     Config file to use
        --socket <PATH>     Unix socket path
        --detach            Run in background
```

**Examples:**
```bash
# Start daemon (foreground)
fabricks daemon start

# Start daemon in background
fabricks daemon start --detach

# Start with custom config
fabricks daemon start --config /etc/fabricksd/config.toml

# Start with custom socket
fabricks daemon start --socket /tmp/fabricks.sock
```

**Output:**
```
Starting fabricksd...
✓ Loaded configuration
✓ Created socket: /var/run/fabricks.sock
✓ Started service manager
✓ Started health monitor
✓ Started auto-scaler
Daemon ready (PID: 12345)
```

---

### `fabricks daemon stop`

Stop the Fabricks daemon.

**Usage:**
```bash
fabricks daemon stop [OPTIONS]
```

**Options:**
```
    -t, --timeout <SEC>     Graceful shutdown timeout [default: 30]
        --force             Force shutdown (SIGKILL)
```

**Examples:**
```bash
# Graceful shutdown
fabricks daemon stop

# Force shutdown
fabricks daemon stop --force

# Custom timeout
fabricks daemon stop --timeout 60
```

**Output:**
```
Stopping fabricksd...
✓ Stopping services (8)
✓ Closing connections
✓ Daemon stopped
```

---

### `fabricks daemon status`

Check daemon status.

**Usage:**
```bash
fabricks daemon status [OPTIONS]
```

**Options:**
```
    -v, --verbose           Show detailed status
        --format <FORMAT>   Output format [possible: table, json, yaml]
```

**Examples:**
```bash
# Check status
fabricks daemon status

# Verbose status
fabricks daemon status --verbose

# Output as JSON
fabricks daemon status --format json
```

**Output:**
```
fabricksd is running
PID: 12345
Uptime: 2h 15m 32s
Socket: /var/run/fabricks.sock
Services: 8 running, 0 stopped
Networks: 4 active
Volumes: 3 mounted
Memory: 245 MB
CPU: 3.2%
```

---

### `fabricks daemon logs`

View daemon logs.

**Usage:**
```bash
fabricks daemon logs [OPTIONS]
```

**Options:**
```
        --follow            Follow log output
        --tail <N>          Number of lines to show from end [default: all]
        --since <TIME>      Show logs since timestamp or duration
```

**Examples:**
```bash
# View daemon logs
fabricks daemon logs

# Follow logs
fabricks daemon logs --follow

# Last 100 lines
fabricks daemon logs --tail 100

# Logs from last hour
fabricks daemon logs --since 1h
```

**Output:**
```
2025-01-15T08:00:00Z [INFO] Daemon starting
2025-01-15T08:00:01Z [INFO] Loaded configuration from /etc/fabricksd/config.toml
2025-01-15T08:00:02Z [INFO] Created socket: /var/run/fabricks.sock
2025-01-15T10:23:45Z [INFO] Service created: product-service
2025-01-15T10:24:00Z [INFO] Health check passed: product-service-0
```

---

### `fabricks daemon reload`

Reload daemon configuration without restarting.

**Usage:**
```bash
fabricks daemon reload [OPTIONS]
```

**Options:**
```
        --validate          Validate config before reloading
```

**Examples:**
```bash
# Reload configuration
fabricks daemon reload

# Validate before reload
fabricks daemon reload --validate
```

**Output:**
```
Reloading configuration...
✓ Configuration validated
✓ Applied changes:
  - log_level: info → debug
  - max_services: 100 → 200
Daemon reloaded
```

---

### `fabricks daemon info`

Show detailed daemon information.

**Usage:**
```bash
fabricks daemon info [OPTIONS]
```

**Options:**
```
        --format <FORMAT>   Output format [default: yaml] [possible: yaml, json]
```

**Examples:**
```bash
# Show daemon info
fabricks daemon info

# Output as JSON
fabricks daemon info --format json
```

**Output:**
```yaml
version: 1.0.0
api_version: v1
runtime: wasmtime 25.0.0
platform: linux/amd64
started_at: 2025-01-15T08:00:00Z
uptime: 4h 30m 15s
config:
  socket: /var/run/fabricks.sock
  data_dir: /var/lib/fabricksd
  max_services: 100
  log_level: info
```

---

### `fabricks daemon stats`

Show daemon resource usage and statistics.

**Usage:**
```bash
fabricks daemon stats [OPTIONS]
```

**Options:**
```
        --format <FORMAT>   Output format [default: table] [possible: table, json, yaml]
```

**Examples:**
```bash
# Show daemon stats
fabricks daemon stats

# Output as JSON
fabricks daemon stats --format json
```

**Output:**
```
SERVICES                  8 running, 0 stopped
NETWORKS                  4 active
VOLUMES                   3 mounted, 65.3 GiB total
MEMORY                    2.1 GiB / 8 GiB (26%)
CPU                       3.2 / 8.0 cores (40%)
API REQUESTS              15,234 total, 12.5/sec
API ERRORS                23 (0.15%)
```

---

## Development Commands

### `fabricks dev`

Start development mode with auto-rebuild and hot-reload.

**Usage:**
```bash
fabricks dev [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` - Path to Fabrickfile directory [default: current directory]

**Options:**
```
    -f, --file <FILE>       Fabrickfile to use [default: Fabrickfile]
    -p, --port <PORT>       Port to listen on [default: from Fabrickfile]
        --no-reload         Disable auto-reload
        --poll              Use polling instead of filesystem events
```

**Examples:**
```bash
# Start dev mode
fabricks dev

# Dev mode for specific service
fabricks dev ./services/api

# Dev mode without hot reload
fabricks dev --no-reload
```

**Output:**
```
Starting development mode...
✓ Built api.wasm (2.1 MB)
✓ Watching for changes...

Listening on http://localhost:8080

[10:45:23] File changed: src/main.rs
[10:45:24] Rebuilding...
[10:45:26] ✓ Rebuilt (2.1 MB)
[10:45:26] ✓ Reloaded
```

---

### `fabricks watch`

Watch and rebuild on file changes (similar to `cargo watch`).

**Usage:**
```bash
fabricks watch [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` - Path to Fabrickfile directory [default: current directory]

**Options:**
```
    -f, --file <FILE>       Fabrickfile to use [default: Fabrickfile]
    -x, --exec <CMD>        Execute command after successful build
        --clear             Clear screen before each build
        --debounce <MS>     Debounce file changes [default: 500]
```

**Examples:**
```bash
# Watch and rebuild
fabricks watch

# Watch and run tests after build
fabricks watch -x "cargo test"

# Clear screen on rebuild
fabricks watch --clear

# Custom debounce
fabricks watch --debounce 1000
```

**Output:**
```
Watching for changes in ./services/api...

[10:45:23] Change detected: src/main.rs
[10:45:23] Building...
[10:45:26] ✓ Built successfully
```

---

## Validation & Inspection

### `fabricks validate`

Validate Fabrickfile or fabricks-mortar.toml.

**Usage:**
```bash
fabricks validate [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` - Path to validate [default: current directory]

**Options:**
```
    -f, --file <FILE>       File to validate [default: auto-detect]
        --type <TYPE>       File type [possible: fabrickfile, mortar]
        --strict            Enable strict validation
```

**Examples:**
```bash
# Validate Fabrickfile
fabricks validate

# Validate mortar file
fabricks validate --type mortar

# Validate specific file
fabricks validate -f ./services/api/Fabrickfile

# Strict validation
fabricks validate --strict
```

**Output:**
```
Validating Fabrickfile...
✓ Syntax valid
✓ All required fields present
✓ Capabilities properly defined
✓ Health check configured
⚠ Warning: No security settings defined
✓ Valid!
```

---

### `fabricks inspect`

Inspect a fabrick or WASM module.

**Usage:**
```bash
fabricks inspect [OPTIONS] <IMAGE|PATH|WASM>
```

**Arguments:**
- `<IMAGE|PATH|WASM>` - Image, directory, or WASM file to inspect

**Options:**
```
        --format <FORMAT>   Output format [default: yaml] [possible: yaml, json, toml]
        --show-exports      Show exported functions
        --show-imports      Show imported modules
        --show-size         Show size breakdown
```

**Examples:**
```bash
# Inspect fabrick
fabricks inspect ./services/api

# Inspect WASM file
fabricks inspect ./dist/api.wasm

# Inspect registry image
fabricks inspect wasm://redis:7.2

# Show exports
fabricks inspect --show-exports ./services/api

# Output as JSON
fabricks inspect --format json ./services/api
```

**Output:**
```yaml
name: product-service
version: 2.1.0
wasm_size: 2.3 MB
exports:
  - list_products
  - get_product
  - create_product
imports:
  - search_indexer: wasm://search/indexer:v1.0
capabilities:
  network:
    listen: [8080]
    connect: [postgres:5432, redis:6379]
  env: [DATABASE_URL, REDIS_URL, LOG_LEVEL]
```

---

### `fabricks graph`

Visualize service dependencies.

**Usage:**
```bash
fabricks graph [OPTIONS]
```

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
    -o, --output <FILE>     Output file (supports .dot, .svg, .png)
        --show-networks     Include network topology
        --show-volumes      Include volume mounts
```

**Examples:**
```bash
# Show dependency graph (ASCII)
fabricks graph

# Generate DOT file
fabricks graph -o graph.dot

# Generate SVG
fabricks graph -o graph.svg

# Include networks
fabricks graph --show-networks
```

**Output (ASCII):**
```
┌─────────────────┐
│   frontend      │
└────────┬────────┘
         │
    ┌────▼────┐
    │   api   │
    └────┬────┘
         │
    ┌────┴────┬────────┬─────────┐
    ▼         ▼        ▼         ▼
┌────────┐ ┌──────┐ ┌──────┐ ┌─────────┐
│product │ │ user │ │ cart │ │  order  │
└───┬────┘ └──┬───┘ └──┬───┘ └────┬────┘
    │         │        │          │
    └─────────┴────────┴──────────┘
              │
         ┌────▼────┐
         │postgres │
         └─────────┘
```

---

## Registry Commands

### `fabricks login`

Log in to a registry.

**Usage:**
```bash
fabricks login [OPTIONS] [REGISTRY]
```

**Arguments:**
- `[REGISTRY]` - Registry URL [default: registry.fabricks.io]

**Options:**
```
    -u, --username <USER>   Username
    -p, --password <PASS>   Password (not recommended, use stdin)
        --password-stdin    Read password from stdin
```

**Examples:**
```bash
# Interactive login
fabricks login registry.acme.io

# Login with username
fabricks login -u myuser registry.acme.io

# Login with password from stdin
echo "$PASSWORD" | fabricks login --password-stdin registry.acme.io
```

**Output:**
```
Username: myuser
Password: 
✓ Logged in to registry.acme.io
```

---

### `fabricks logout`

Log out from a registry.

**Usage:**
```bash
fabricks logout [REGISTRY]
```

**Arguments:**
- `[REGISTRY]` - Registry URL [default: registry.fabricks.io]

**Examples:**
```bash
# Logout from default registry
fabricks logout

# Logout from specific registry
fabricks logout registry.acme.io
```

---

### `fabricks search`

Search for fabricks in registry.

**Usage:**
```bash
fabricks search [OPTIONS] <TERM>
```

**Arguments:**
- `<TERM>` - Search term

**Options:**
```
        --limit <N>         Limit results [default: 25]
        --filter <FILTER>   Filter by field (e.g., stars:>100)
        --format <FORMAT>   Output format [default: table] [possible: table, json]
```

**Examples:**
```bash
# Search registry
fabricks search redis

# Search with filters
fabricks search --filter "stars:>100" database

# Limit results
fabricks search --limit 10 web-server
```

**Output:**
```
NAME                        DESCRIPTION                          STARS    PULLS
redis:7.2                   Redis in-memory database             1.2k     10M
redis-cluster:7.2           Redis cluster configuration          450      1.5M
redis-sentinel:7.2          Redis sentinel for HA                320      800k
```

---

## Utility Commands

### `fabricks init`

Initialize a new fabrick project.

**Usage:**
```bash
fabricks init [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` - Project path [default: current directory]

**Options:**
```
        --name <NAME>       Project name
        --template <TPL>    Template to use [possible: rust, go, javascript, python]
        --mortar            Create fabricks-mortar.toml
```

**Examples:**
```bash
# Interactive init
fabricks init

# Init with template
fabricks init --template rust --name my-service

# Init with mortar file
fabricks init --mortar my-project
```

**Output:**
```
Creating new fabrick project...
? Project name: my-api
? Language: Rust
? Include mortar file? Yes

✓ Created Fabrickfile
✓ Created fabricks-mortar.toml
✓ Created src/main.rs
✓ Created Cargo.toml

Next steps:
  1. cd my-api
  2. fabricks build
  3. fabricks run
```

---

### `fabricks version`

Show version information.

**Usage:**
```bash
fabricks version [OPTIONS]
```

**Options:**
```
        --short             Show only version number
        --json              Output as JSON
```

**Examples:**
```bash
# Show version
fabricks version

# Short version
fabricks version --short

# JSON output
fabricks version --json
```

**Output:**
```
fabricks 1.0.0
commit: abc123def456
built: 2025-01-15T10:00:00Z
rust: 1.75.0
wasm-tools: 1.0.50
```

---

### `fabricks clean`

Clean build artifacts and caches.

**Usage:**
```bash
fabricks clean [OPTIONS]
```

**Options:**
```
        --all               Remove all cached data
        --builds            Remove build cache
        --registry          Remove registry cache
        --logs              Remove logs
```

**Examples:**
```bash
# Clean build cache
fabricks clean --builds

# Clean everything
fabricks clean --all

# Clean specific caches
fabricks clean --builds --registry
```

**Output:**
```
Cleaning caches...
✓ Removed build cache (1.2 GB)
✓ Removed registry cache (450 MB)
Done!
```

---

## Kubernetes Integration

### `fabricks k8s generate`

Generate Kubernetes manifests from fabricks-mortar.toml.

**Usage:**
```bash
fabricks k8s generate [OPTIONS]
```

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
    -o, --output <DIR>      Output directory [default: ./k8s]
        --namespace <NS>    Kubernetes namespace [default: default]
        --use-spinkube      Generate SpinKube resources
```

**Examples:**
```bash
# Generate manifests
fabricks k8s generate

# Generate to specific directory
fabricks k8s generate -o ./manifests

# Generate with namespace
fabricks k8s generate --namespace production

# Generate SpinKube manifests
fabricks k8s generate --use-spinkube
```

**Output:**
```
Generating Kubernetes manifests...
✓ services/product-deployment.yaml
✓ services/product-service.yaml
✓ services/user-deployment.yaml
✓ services/user-service.yaml
✓ networks/application-networkpolicy.yaml
✓ volumes/postgres-pvc.yaml
✓ volumes/redis-pvc.yaml
Generated 15 manifests in ./k8s
```

---

### `fabricks k8s apply`

Apply fabricks to Kubernetes cluster.

**Usage:**
```bash
fabricks k8s apply [OPTIONS]
```

**Options:**
```
    -f, --file <FILE>       Mortar file to use [default: fabricks-mortar.toml]
        --namespace <NS>    Kubernetes namespace
        --context <CTX>     Kubernetes context
        --dry-run           Show what would be applied
```

**Examples:**
```bash
# Apply to cluster
fabricks k8s apply

# Apply to specific namespace
fabricks k8s apply --namespace production

# Dry run
fabricks k8s apply --dry-run

# Apply to specific context
fabricks k8s apply --context prod-cluster
```

**Output:**
```
Applying to Kubernetes cluster (context: prod-cluster)...
✓ Creating namespace: acme-shop
✓ Creating network policies (4)
✓ Creating persistent volume claims (3)
✓ Creating deployments (8)
✓ Creating services (8)
✓ Waiting for rollout...
  → product-service: 2/2 ready
  → user-service: 2/2 ready
  ...
✓ All services ready!
```

---

## Complete Examples

### Local Development Workflow
```bash
# Initialize new project
fabricks init --template rust --name my-api
cd my-api

# Start development mode
fabricks dev

# In another terminal: watch tests
fabricks watch -x "cargo test"

# Build for production
fabricks build --no-cache

# Tag and push
fabricks push registry.acme.io/my-api:v1.0.0
```

### Multi-Service Application
```bash
# Clone project
git clone https://github.com/acme/shop
cd shop

# Build all services
fabricks mortar build --parallel

# Start in background
fabricks mortar up -d

# View logs
fabricks mortar logs --follow api

# Scale up
fabricks mortar scale api=5 worker=10

# Check status
fabricks mortar ps

# Inspect a service
fabricks service inspect product-service

# Monitor events
fabricks events --service product-service

# Stop everything
fabricks mortar down -v
```

### Production Deployment to Kubernetes
```bash
# Validate configuration
fabricks validate

# Generate K8s manifests
fabricks k8s generate --namespace production -o ./k8s

# Review manifests
ls -la ./k8s

# Apply to cluster
fabricks k8s apply --namespace production --context prod-cluster

# Or use kubectl directly
kubectl apply -f ./k8s/
```

### Registry Workflow
```bash
# Login
fabricks login registry.acme.io

# Search for base images
fabricks search redis

# Pull base image
fabricks pull wasm://redis:7.2

# Build on top of it
fabricks build -t my-redis:v1.0.0

# Push to private registry
fabricks push registry.acme.io/my-redis:v1.0.0

# Logout
fabricks logout registry.acme.io
```

### Troubleshooting Workflow
```bash
# Check daemon status
fabricks daemon status --verbose

# View daemon logs
fabricks daemon logs --follow

# List all services
fabricks service ls -a

# Inspect failing service
fabricks service inspect failed-service

# View service logs
fabricks service logs failed-service --tail 100

# Check resource usage
fabricks service stats failed-service

# Monitor events
fabricks events --types service.failed,health.changed

# View network configuration
fabricks network inspect application

# Check volume status
fabricks volume inspect postgres-data
```

---

## Environment Variables

Fabricks respects the following environment variables:
```bash
# Registry authentication
FABRICKS_REGISTRY_USER=myuser
FABRICKS_REGISTRY_PASSWORD=mypassword

# Default registry
FABRICKS_REGISTRY=registry.acme.io

# Build settings
FABRICKS_BUILD_CACHE_DIR=~/.fabricks/cache
FABRICKS_MAX_PARALLEL_BUILDS=4

# Runtime settings
FABRICKS_RUNTIME=wasmtime  # wasmtime | wasmer
FABRICKS_RUNTIME_FLAGS="--opt-level=2"

# Daemon settings
FABRICKS_SOCKET=/var/run/fabricks.sock
FABRICKS_DATA_DIR=/var/lib/fabricksd

# Kubernetes
FABRICKS_K8S_CONTEXT=prod-cluster
FABRICKS_K8S_NAMESPACE=default

# Colors
NO_COLOR=1  # Disable colors
FORCE_COLOR=1  # Force colors
```

---

## Configuration File

Global configuration at `~/.fabricks/config.toml`:
```toml
# Default registry
[registry]
default = "registry.fabricks.io"

# Registry credentials
[[registry.auth]]
url = "registry.acme.io"
username = "myuser"
# password in keychain

# Build settings
[build]
cache_dir = "~/.fabricks/cache"
max_parallel = 4
default_platform = "wasm32-wasi"

# Runtime settings
[runtime]
engine = "wasmtime"  # wasmtime | wasmer
flags = ["--opt-level=2"]

# Daemon settings
[daemon]
socket = "/var/run/fabricks.sock"
auto_start = true

# Kubernetes
[kubernetes]
context = "prod-cluster"
namespace = "default"
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Build failed |
| 4 | Runtime error |
| 5 | Network error |
| 6 | Authentication error |
| 7 | Validation error |
| 8 | Daemon not running |

---
