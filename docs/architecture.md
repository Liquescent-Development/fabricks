# Fabricks Architecture

Technical architecture documentation for Fabricks WASM orchestration platform.

---

## Table of Contents

- [System Overview](#system-overview)
- [High-Level Architecture](#high-level-architecture)
- [Component Architecture](#component-architecture)
- [Data Flow](#data-flow)
- [Technology Stack](#technology-stack)
- [Module Structure](#module-structure)
- [Storage Architecture](#storage-architecture)
- [Network Architecture](#network-architecture)
- [Security Architecture](#security-architecture)
- [API Architecture](#api-architecture)
- [Design Patterns](#design-patterns)
- [Performance Considerations](#performance-considerations)
- [Scalability](#scalability)
- [Failure Handling](#failure-handling)

---

## System Overview

Fabricks is a WASM-native orchestration platform consisting of:

1. **CLI (`fabricks`)** - User-facing command-line interface
2. **Daemon (`fabricksd`)** - Long-running orchestration service
3. **Runtime Integration** - Wasmtime/Wasmer execution environment
4. **Registry Client** - OCI-compliant registry interaction
5. **File Parsers** - TOML parsing for Fabrickfile and mortar files

**Design Principles:**

- **Declarative-first** - Configuration over imperative commands
- **Security by default** - Deny-by-default capability model
- **WASM-native** - Built specifically for WebAssembly strengths
- **Standard protocols** - OCI, HTTP/REST, Unix sockets
- **Minimal dependencies** - Lean, focused codebase

---

## High-Level Architecture
```
┌─────────────────────────────────────────────────────────────────┐
│                        User Interface                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │ fabricks │  │   IDE    │  │ kubectl  │  │   Web    │         │
│  │   CLI    │  │Extension │  │          │  │    UI    │         │
│  └────┬─────┘  └────┬─────┘  └─────┬────┘  └─────┬────┘         │
│       │             │              │             │              │
└───────┼─────────────┼──────────────┼─────────────┼──────────────┘
        │             │              │             │
        │   ┌─────────▼──────────────▼─────────────▼────────┐
        │   │         Unix Socket / HTTP API                │
        │   └──────────────────────┬────────────────────────┘
        │                          │
        ▼                          ▼
┌───────────────────┐    ┌──────────────────────────────────────┐
│   fabricks CLI    │    │          fabricksd (Daemon)          │
│                   │    │                                      │
│ - Build           │    │  ┌────────────────────────────────┐  │
│ - Run (direct)    │    │  │     HTTP API Server            │  │
│ - Push/Pull       │    │  │  (Unix Socket: /var/run/...)   │  │
│ - Validate        │    │  └────────────────────────────────┘  │
│ - Inspect         │    │                                      │
│                   │    │  ┌────────────────────────────────┐  │
│ Uses:             │    │  │   Service Manager              │  │
│ - TOML parsers    │    │  │  - Lifecycle management        │  │
│ - Wasmtime (opt)  │    │  │  - Replica management          │  │
│ - OCI client      │    │  │  - Dependency resolution       │  │
│ - Build tools     │    │  └────────────────────────────────┘  │
└─────────┬─────────┘    │                                      │
          │              │  ┌────────────────────────────────┐  │
          │              │  │   Health Monitor               │  │
          │              │  │  - HTTP/TCP/Exec checks        │  │
          │              │  │  - Health history tracking     │  │
          │              │  │  - Failure detection           │  │
          │              │  └────────────────────────────────┘  │
          │              │                                      │
          │              │  ┌────────────────────────────────┐  │
          │              │  │   Auto-Scaler                  │  │
          │              │  │  - Resource monitoring         │  │
          │              │  │  - Replica adjustment          │  │
          │              │  │  - Cooldown management         │  │
          │              │  └────────────────────────────────┘  │
          │              │                                      │
          │              │  ┌────────────────────────────────┐  │
          │              │  │   Network Manager              │  │
          │              │  │  - Network creation/deletion   │  │
          │              │  │  - Policy enforcement          │  │
          │              │  │  - DNS resolution              │  │
          │              │  └────────────────────────────────┘  │
          │              │                                      │
          │              │  ┌────────────────────────────────┐  │
          │              │  │   Volume Manager               │  │
          │              │  │  - Volume lifecycle            │  │
          │              │  │  - Mount management            │  │
          │              │  │  - Backup scheduling           │  │
          │              │  └────────────────────────────────┘  │
          │              │                                      │
          │              │  ┌────────────────────────────────┐  │
          │              │  │   Policy Engine                │  │
          │              │  │  - Policy validation           │  │
          │              │  │  - Capability enforcement      │  │
          │              │  │  - Audit logging               │  │
          │              │  └────────────────────────────────┘  │
          │              │                                      │
          │              │  ┌────────────────────────────────┐  │
          │              │  │   Event Bus                    │  │
          │              │  │  - Event publishing            │  │
          │              │  │  - SSE streaming               │  │
          │              │  │  - Event history               │  │
          │              │  └────────────────────────────────┘  │
          │              │                                      │
          │              │  ┌────────────────────────────────┐  │
          │              │  │   State Store                  │  │
          │              │  │  - Service state               │  │
          │              │  │  - Configuration               │  │
          │              │  │  - Health history              │  │
          │              │  └────────────────────────────────┘  │
          │              └──────────────────────────────────────┘
          │                          │
          │                          ▼
          │              ┌──────────────────────────────────────┐
          │              │     WASM Runtime Layer               │
          │              │                                      │
          │              │  ┌────────────┐  ┌────────────┐      │
          │              │  │ Wasmtime   │  │  Wasmer    │      │
          │              │  │ Instance 1 │  │ Instance 1 │      │
          │              │  └────────────┘  └────────────┘      │
          │              │  ┌────────────┐  ┌────────────┐      │
          │              │  │ Wasmtime   │  │  Wasmer    │      │
          │              │  │ Instance 2 │  │ Instance 2 │      │
          │              │  └────────────┘  └────────────┘      │
          │              │         ...            ...           │
          │              └──────────────────────────────────────┘
          │                          │
          ▼                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    External Systems                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │   OCI    │  │   K8s    │  │ Storage  │  │  Vault   │         │
│  │ Registry │  │ Cluster  │  │ Backend  │  │ Secrets  │         │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘         │
└─────────────────────────────────────────────────────────────────┘
```

---

## Component Architecture

### 1. CLI (`fabricks`)

**Purpose:** User-facing command-line interface

**Responsibilities:**
- Parse command-line arguments
- Read and validate Fabrickfile/mortar files
- Build WASM modules (compile source code)
- Communicate with daemon via Unix socket
- Display formatted output to user
- Manage local cache and credentials

**Key Modules:**
```
fabricks/
├── src/
│   ├── main.rs                 # Entry point, command routing
│   ├── commands/
│   │   ├── build.rs            # fabricks build
│   │   ├── run.rs              # fabricks run
│   │   ├── push.rs             # fabricks push
│   │   ├── pull.rs             # fabricks pull
│   │   ├── mortar/
│   │   │   ├── up.rs           # fabricks mortar up
│   │   │   ├── down.rs         # fabricks mortar down
│   │   │   ├── ps.rs           # fabricks mortar ps
│   │   │   └── ...
│   │   ├── daemon/
│   │   │   ├── start.rs        # fabricks daemon start
│   │   │   └── ...
│   │   └── ...
│   ├── parser/
│   │   ├── fabrickfile.rs      # Parse Fabrickfile
│   │   ├── mortar.rs           # Parse mortar file
│   │   └── validation.rs       # Validate configs
│   ├── builder/
│   │   ├── compiler.rs         # Compile source to WASM
│   │   ├── optimizer.rs        # WASM optimization
│   │   └── cache.rs            # Build cache
│   ├── registry/
│   │   ├── client.rs           # OCI registry client
│   │   ├── auth.rs             # Registry authentication
│   │   └── manifest.rs         # OCI manifest handling
│   ├── daemon_client/
│   │   ├── client.rs           # Daemon API client
│   │   └── socket.rs           # Unix socket communication
│   ├── runtime/
│   │   ├── wasmtime.rs         # Wasmtime integration (direct run)
│   │   └── config.rs           # Runtime configuration
│   └── output/
│       ├── formatter.rs        # Output formatting
│       └── progress.rs         # Progress bars
```

**Technology:**
- Rust with clap for CLI parsing
- tokio for async operations
- serde + toml for parsing
- hyper for HTTP (registry, daemon)
- wasmtime (optional, for direct `fabricks run`)

---

### 2. Daemon (`fabricksd`)

**Purpose:** Long-running orchestration service

**Responsibilities:**
- Service lifecycle management
- Health monitoring
- Auto-scaling
- Network and volume management
- Policy enforcement
- Event streaming
- State persistence

**Key Modules:**
```
fabricksd/
├── src/
│   ├── main.rs                     # Entry point, daemon init
│   ├── api/
│   │   ├── server.rs               # HTTP API server
│   │   ├── handlers/
│   │   │   ├── services.rs         # /v1/services/*
│   │   │   ├── networks.rs         # /v1/networks/*
│   │   │   ├── volumes.rs          # /v1/volumes/*
│   │   │   ├── health.rs           # /v1/health/*
│   │   │   ├── events.rs           # /v1/events
│   │   │   ├── mortar.rs           # /v1/mortar/*
│   │   │   └── daemon.rs           # /v1/daemon/*
│   │   ├── middleware/
│   │   │   ├── auth.rs             # API authentication
│   │   │   └── logging.rs          # Request logging
│   │   └── models/
│   │       ├── request.rs          # Request DTOs
│   │       └── response.rs         # Response DTOs
│   ├── service/
│   │   ├── manager.rs              # Service lifecycle
│   │   ├── replica.rs              # Replica management
│   │   └── dependency.rs           # Dependency resolution
│   ├── health/
│   │   ├── monitor.rs              # Health check coordinator
│   │   ├── http_check.rs           # HTTP health checks
│   │   ├── tcp_check.rs            # TCP health checks
│   │   ├── exec_check.rs           # Exec health checks
│   │   └── history.rs              # Health history tracking
│   ├── scaler/
│   │   ├── autoscaler.rs           # Auto-scaling logic
│   │   ├── metrics.rs              # Metrics collection
│   │   └── cooldown.rs             # Cooldown management
│   ├── network/
│   │   ├── manager.rs              # Network lifecycle
│   │   ├── policy.rs               # Policy enforcement
│   │   └── dns.rs                  # DNS resolution
│   ├── volume/
│   │   ├── manager.rs              # Volume lifecycle
│   │   ├── mount.rs                # Mount management
│   │   └── backup.rs               # Backup scheduling
│   ├── policy/
│   │   ├── engine.rs               # Policy validation
│   │   ├── capability.rs           # Capability enforcement
│   │   └── audit.rs                # Audit logging
│   ├── events/
│   │   ├── bus.rs                  # Event bus
│   │   ├── publisher.rs            # Event publishing
│   │   ├── subscriber.rs           # Event subscription
│   │   └── sse.rs                  # Server-Sent Events
│   ├── state/
│   │   ├── store.rs                # State persistence
│   │   ├── service_state.rs        # Service state
│   │   └── config_state.rs         # Configuration state
│   ├── runtime/
│   │   ├── pool.rs                 # Runtime instance pool
│   │   ├── wasmtime.rs             # Wasmtime integration
│   │   └── wasmer.rs               # Wasmer integration (optional)
│   └── config/
│       ├── daemon_config.rs        # Daemon configuration
│       └── loader.rs               # Config loading
```

**Technology:**
- Rust with tokio for async runtime
- axum for HTTP API server
- wasmtime for WASM execution
- serde for serialization
- tokio::sync for concurrency primitives

---

### 3. Shared Libraries

**Purpose:** Common functionality shared between CLI and daemon
```
fabricks-common/
├── src/
│   ├── lib.rs
│   ├── models/
│   │   ├── fabrickfile.rs      # Fabrickfile data model
│   │   ├── mortar.rs            # Mortar file data model
│   │   ├── service.rs           # Service definition
│   │   ├── network.rs           # Network definition
│   │   ├── volume.rs            # Volume definition
│   │   └── capability.rs        # Capability model
│   ├── parser/
│   │   ├── toml_parser.rs       # TOML parsing utilities
│   │   └── validator.rs         # Validation logic
│   ├── oci/
│   │   ├── manifest.rs          # OCI manifest handling
│   │   ├── blob.rs              # Blob operations
│   │   └── digest.rs            # SHA256 digest utilities
│   ├── errors/
│   │   └── mod.rs               # Error types
│   └── constants.rs             # Shared constants
```

**Technology:**
- Rust library crate
- serde for serialization
- sha2 for digest computation

---

## Data Flow

### Build and Push Flow
```
User runs: fabricks build -t my-service:v1.0.0 && fabricks push my-service:v1.0.0

┌──────────┐
│ fabricks │
│   CLI    │
└────┬─────┘
     │
     │ 1. Read Fabrickfile
     ▼
┌─────────────┐
│ Fabrickfile │
│   Parser    │
└────┬────────┘
     │
     │ 2. Parse TOML, validate
     ▼
┌─────────────┐
│  Builder    │
└────┬────────┘
     │
     │ 3. Run build command (cargo build, etc.)
     ▼
┌─────────────┐
│   WASM      │
│  Binary     │
└────┬────────┘
     │
     │ 4. Compute SHA256 digest
     ▼
┌─────────────┐
│   OCI       │
│  Manifest   │
│  Generator  │
└────┬────────┘
     │
     │ 5. Create manifest with config blob + WASM layer
     ▼
┌─────────────┐
│  Registry   │
│   Client    │
└────┬────────┘
     │
     │ 6. Push config blob, WASM blob, manifest
     ▼
┌─────────────┐
│    OCI      │
│  Registry   │
└─────────────┘
```

### Pull and Run Flow
```
User runs: fabricks pull my-service:v1.0.0 && fabricks run my-service:v1.0.0

┌──────────┐
│ fabricks │
│   CLI    │
└────┬─────┘
     │
     │ 1. Request manifest
     ▼
┌─────────────┐
│  Registry   │
│   Client    │
└────┬────────┘
     │
     │ 2. GET /v2/.../manifests/v1.0.0
     ▼
┌─────────────┐
│    OCI      │
│  Registry   │
└────┬────────┘
     │
     │ 3. Return manifest (digests for config + WASM)
     ▼
┌─────────────┐
│  Registry   │
│   Client    │
└────┬────────┘
     │
     │ 4. Download blobs by digest
     ▼
┌─────────────┐
│   Local     │
│   Storage   │
│ ~/.fabricks │
└────┬────────┘
     │
     │ 5. Load WASM + config
     ▼
┌─────────────┐
│  Wasmtime   │
│  Runtime    │
└────┬────────┘
     │
     │ 6. Execute WASM module
     ▼
┌─────────────┐
│  Running    │
│  Service    │
└─────────────┘
```

### Mortar Up Flow
```
User runs: fabricks mortar up

┌──────────┐
│ fabricks │
│   CLI    │
└────┬─────┘
     │
     │ 1. Read fabricks-mortar.toml
     ▼
┌──────────────┐
│    Mortar    │
│    Parser    │
└────┬─────────┘
     │
     │ 2. Parse and validate
     ▼
┌──────────────┐
│   Daemon     │
│   Client     │
└────┬─────────┘
     │
     │ 3. POST /v1/mortar/deploy with mortar config
     ▼
┌──────────────┐
│  fabricksd   │
│   Daemon     │
└────┬─────────┘
     │
     │ 4. Create networks
     ▼
┌──────────────┐
│   Network    │
│   Manager    │
└────┬─────────┘
     │
     │ 5. Create volumes
     ▼
┌──────────────┐
│   Volume     │
│   Manager    │
└────┬─────────┘
     │
     │ 6. Resolve dependencies, create services
     ▼
┌──────────────┐
│   Service    │
│   Manager    │
└────┬─────────┘
     │
     │ 7. Start service instances (respecting dependencies)
     ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  Wasmtime    │  │  Wasmtime    │  │  Wasmtime    │
│ Instance 1   │  │ Instance 2   │  │ Instance 3   │
└──────────────┘  └──────────────┘  └──────────────┘
     │
     │ 8. Health checks start
     ▼
┌──────────────┐
│   Health     │
│   Monitor    │
└──────────────┘
```

---

## Technology Stack

### Core Languages
- **Rust** - Primary implementation language
  - Type safety, memory safety
  - Excellent async support (tokio)
  - Strong ecosystem for CLI and servers
  - Native WASM tooling

### WASM Runtime
- **Wasmtime** - Primary runtime
  - Bytecode Alliance project
  - Production-ready
  - Excellent security
  - Component Model support
  
- **Wasmer** - Alternative runtime (optional)
  - Multiple backends (Cranelift, LLVM, Singlepass)
  - Cross-platform support

### HTTP/API
- **axum** - HTTP framework for daemon API
  - Built on tokio and hyper
  - Excellent ergonomics
  - Type-safe routing
  
- **hyper** - HTTP client for registry operations
  - Low-level, flexible
  - HTTP/2 support

### CLI
- **clap** - Command-line argument parsing
  - Derive macros for clean API
  - Help generation
  - Subcommand support

### Serialization
- **serde** - Serialization framework
  - TOML, JSON support
  - Derive macros
  
- **toml** - TOML parsing
- **serde_json** - JSON parsing

### Storage
- **sled** - Embedded database for daemon state
  - ACID transactions
  - Zero-copy reads
  - Crash-safe
  
- **File system** - Blob storage
  - Content-addressable by SHA256
  - OCI layout format

### Concurrency
- **tokio** - Async runtime
  - Task spawning
  - Channels (mpsc, broadcast, watch)
  - Timers and intervals
  
- **tokio::sync** - Synchronization primitives
  - Mutex, RwLock
  - Semaphore
  - Notify

### Networking
- **hyper** - HTTP client/server
- **tower** - Service composition
- **Unix sockets** - Daemon communication

### Cryptography
- **sha2** - SHA256 digest computation
- **ring** - TLS, signatures (for Cosign support)

---

## Module Structure

### Workspace Layout
```
fabricks/
├── Cargo.toml              # Workspace root
├── README.md
├── LICENSE
├── ARCHITECTURE.md
├── IMPLEMENTATION_PLAN.md
│
├── fabricks/               # CLI binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       └── ...
│
├── fabricksd/              # Daemon binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       └── ...
│
├── fabricks-common/        # Shared library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── ...
│
├── fabricks-oci/           # OCI registry client library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── ...
│
└── fabricks-runtime/       # WASM runtime integration library
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        └── ...
```

---

## Storage Architecture

### Local Storage Layout
```
~/.fabricks/
├── registry/                       # OCI layout
│   ├── blobs/sha256/               # Content-addressable blobs
│   ├── manifests/                  # Tag -> digest mappings
│   ├── index.json                  # OCI index
│   └── oci-layout                  # Version marker
│
├── daemon/                         # Daemon data
│   ├── state.db                    # sled database (service state)
│   ├── logs/                       # Service logs
│   └── metrics/                    # Metrics history
│
├── cache/                          # Build cache
│   └── ...
│
├── config.toml                     # User configuration
└── credentials.json                # Registry credentials
```

### State Database Schema (sled)
```
Services Tree:
  Key: service_id (String)
  Value: ServiceState (JSON)
    - id, name, image
    - state (running, stopped, failed)
    - replicas, networks, environment
    - created_at, updated_at

Networks Tree:
  Key: network_id (String)
  Value: NetworkState (JSON)
    - id, name, internal, isolated
    - ingress, egress
    - services (array of service_ids)

Volumes Tree:
  Key: volume_id (String)
  Value: VolumeState (JSON)
    - id, name, size, encrypted
    - mounted_by (array of service_ids)

Health Tree:
  Key: service_id:timestamp
  Value: HealthCheck (JSON)
    - timestamp, status, response_time

Events Tree:
  Key: event_id (UUID)
  Value: Event (JSON)
    - type, service_id, data, timestamp
```

---

## Network Architecture

### Network Segmentation

Networks are logical isolation boundaries enforced by:

1. **Service-to-service communication**
   - Services can only talk to services on shared networks
   - Validated by network manager before connection

2. **Capability grants**
   - Services must have both network membership AND capability grant
   - Double-check: network + capability

3. **Policy enforcement**
   - Policy engine validates connections
   - Audit logging for compliance

**Implementation:**
```rust
// Check if connection is allowed
fn can_connect(from_service: &Service, to_host: &str, to_port: u16) -> bool {
    // 1. Check if services share a network
    let shared_network = from_service.networks.iter()
        .any(|net| to_service.networks.contains(net));
    
    // 2. Check capability grant
    let has_capability = from_service.capabilities.network.connect
        .contains(&format!("{}:{}", to_host, to_port));
    
    // 3. Check policy
    let policy_allows = policy_engine.allows(from_service, to_service);
    
    shared_network && has_capability && policy_allows
}
```

---

## Security Architecture

### Capability Model

**Deny-by-default:** Services have zero capabilities unless explicitly granted.

**Capability Types:**

1. **Environment variables**
```toml
   [capabilities]
   env = ["DATABASE_URL", "LOG_LEVEL"]
```

2. **Network - Listen**
```toml
   [capabilities.network]
   listen = [8080]
```

3. **Network - Connect**
```toml
   [capabilities.network]
   connect = ["postgres:5432", "redis:6379"]
```

4. **Filesystem - Read**
```toml
   [capabilities.filesystem]
   read = ["./config", "./templates"]
```

5. **Filesystem - Write**
```toml
   [capabilities.filesystem]
   write = ["./logs"]
```

**Enforcement:**
```rust
// Runtime capability enforcement
impl WasmRuntime {
    fn check_env_access(&self, key: &str) -> Result<String, Error> {
        if !self.capabilities.env.contains(&key.to_string()) {
            return Err(Error::CapabilityDenied(format!("env:{}", key)));
        }
        Ok(std::env::var(key)?)
    }
    
    fn check_network_listen(&self, port: u16) -> Result<(), Error> {
        if !self.capabilities.network.listen.contains(&port) {
            return Err(Error::CapabilityDenied(format!("listen:{}", port)));
        }
        Ok(())
    }
}
```

### Authentication

**Registry Authentication:**
- Standard OCI bearer tokens
- Stored in `~/.fabricks/credentials.json`
- Compatible with Docker credentials

**Daemon API Authentication:**
- Optional API key authentication
- Unix socket permissions (file-based security)
- Per-request API key header: `X-Fabricks-API-Key`

---

## API Architecture

### REST API Design

**Principles:**
- RESTful resource-oriented
- JSON request/response bodies
- HTTP status codes for errors
- Server-Sent Events for streaming

**Base URL:** `unix:///var/run/fabricks.sock/v1`

**Resource Hierarchy:**
```
/v1/services
/v1/services/{id}
/v1/services/{id}/logs
/v1/services/{id}/stats
/v1/health/{id}
/v1/networks
/v1/networks/{id}
/v1/volumes
/v1/volumes/{id}
/v1/events
/v1/mortar/deploy
/v1/mortar/undeploy
/v1/daemon/info
/v1/daemon/stats
```

**Response Format:**
```json
{
  "status": "success",
  "data": { ... }
}
```

**Error Format:**
```json
{
  "status": "error",
  "error": {
    "code": "SERVICE_NOT_FOUND",
    "message": "Service 'api' not found",
    "details": { ... }
  }
}
```

---

## Design Patterns

### 1. Actor Pattern (Service Manager)

Each service managed as independent actor:
```rust
struct ServiceActor {
    id: String,
    state: ServiceState,
    replicas: Vec<Instance>,
    health_tx: mpsc::Sender<HealthEvent>,
}

impl ServiceActor {
    async fn run(&mut self, mut rx: mpsc::Receiver<ServiceCommand>) {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                ServiceCommand::Start => self.start().await,
                ServiceCommand::Stop => self.stop().await,
                ServiceCommand::Scale(n) => self.scale(n).await,
                ServiceCommand::HealthCheck => self.check_health().await,
            }
        }
    }
}
```

### 2. Observer Pattern (Event Bus)

Event bus for pub/sub:
```rust
struct EventBus {
    subscribers: Arc<RwLock<Vec<mpsc::Sender<Event>>>>,
}

impl EventBus {
    async fn publish(&self, event: Event) {
        let subs = self.subscribers.read().await;
        for tx in subs.iter() {
            let _ = tx.send(event.clone()).await;
        }
    }
    
    async fn subscribe(&self) -> mpsc::Receiver<Event> {
        let (tx, rx) = mpsc::channel(100);
        self.subscribers.write().await.push(tx);
        rx
    }
}
```

### 3. Repository Pattern (State Store)

Abstract persistence layer:
```rust
#[async_trait]
trait ServiceRepository {
    async fn save(&self, service: &ServiceState) -> Result<()>;
    async fn find(&self, id: &str) -> Result<Option<ServiceState>>;
    async fn list(&self) -> Result<Vec<ServiceState>>;
    async fn delete(&self, id: &str) -> Result<()>;
}

struct SledServiceRepository {
    db: sled::Db,
}

#[async_trait]
impl ServiceRepository for SledServiceRepository {
    async fn save(&self, service: &ServiceState) -> Result<()> {
        let tree = self.db.open_tree("services")?;
        let json = serde_json::to_vec(service)?;
        tree.insert(service.id.as_bytes(), json)?;
        Ok(())
    }
    // ... other methods
}
```

### 4. Builder Pattern (Configuration)

Fluent API for complex objects:
```rust
impl ServiceConfig {
    fn builder() -> ServiceConfigBuilder {
        ServiceConfigBuilder::default()
    }
}

struct ServiceConfigBuilder {
    name: Option<String>,
    image: Option<String>,
    networks: Vec<String>,
    // ... other fields
}

impl ServiceConfigBuilder {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    
    fn network(mut self, network: impl Into<String>) -> Self {
        self.networks.push(network.into());
        self
    }
    
    fn build(self) -> Result<ServiceConfig> {
        Ok(ServiceConfig {
            name: self.name.ok_or(Error::MissingField("name"))?,
            image: self.image.ok_or(Error::MissingField("image"))?,
            networks: self.networks,
        })
    }
}
```

---

## Performance Considerations

### 1. Async I/O

All I/O operations are async using tokio:
- Non-blocking network operations
- Concurrent service management
- Parallel health checks

### 2. Zero-Copy Where Possible

- sled database provides zero-copy reads
- Shared memory for WASM instances (future optimization)
- Streaming responses for logs/events (no buffering)

### 3. Resource Pooling

- Wasmtime instance pool (reuse compiled modules)
- HTTP connection pooling (registry operations)
- Thread pool for CPU-intensive work

### 4. Caching

- Build cache (incremental compilation)
- OCI blob cache (content-addressable)
- DNS cache for service discovery

---

## Scalability

### Horizontal Scalability

- Multiple daemon instances (future: distributed mode)
- Service replicas scale independently
- Event bus supports multiple subscribers

### Vertical Scalability

- Efficient memory usage via WASM
- ~50x more services per node vs containers
- Shared memory for component model calls (future)

### Resource Limits

- Per-service CPU/memory limits
- Global resource limits in daemon config
- Back-pressure mechanisms

---

## Failure Handling

### Service Failures

1. Health check detects failure
2. Policy determines action (restart, alert, scale down)
3. Event published to event bus
4. Auto-scaler adjusts if needed
5. State persisted to database

### Daemon Failures

- State persisted to sled (ACID, crash-safe)
- Services continue running (Wasmtime process)
- On restart, daemon reconciles state

### Network Failures

- Retry with exponential backoff
- Circuit breaker pattern for external calls
- Graceful degradation

### Registry Failures

- Use local cache when registry unavailable
- Queue pushes for retry
- Warn user but don't block operations

---
