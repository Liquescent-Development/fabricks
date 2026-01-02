# Fabricks Implementation Plan

Step-by-step plan for implementing Fabricks from scratch.

---

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Phase 0: Project Setup](#phase-0-project-setup)
- [Phase 1: Core Data Models](#phase-1-core-data-models)
- [Phase 2: File Parsers](#phase-2-file-parsers)
- [Phase 3: OCI Registry Client](#phase-3-oci-registry-client)
- [Phase 4: WASM Runtime Integration](#phase-4-wasm-runtime-integration)
- [Phase 5: Basic CLI](#phase-5-basic-cli)
- [Phase 6: Daemon Foundation](#phase-6-daemon-foundation)
- [Phase 7: Service Manager](#phase-7-service-manager)
- [Phase 8: Health Monitoring](#phase-8-health-monitoring)
- [Phase 9: Network Manager](#phase-9-network-manager)
- [Phase 10: Volume Manager](#phase-10-volume-manager)
- [Phase 11: Auto-Scaler](#phase-11-auto-scaler)
- [Phase 12: Policy Engine](#phase-12-policy-engine)
- [Phase 13: Kubernetes Integration](#phase-13-kubernetes-integration)
- [Testing Strategy](#testing-strategy)
- [Documentation](#documentation)
- [Release Plan](#release-plan)

---

## Overview

This plan breaks down Fabricks implementation into manageable phases. Each phase builds on the previous ones and delivers working functionality.

**Estimated Timeline:** 6-9 months for MVP

**Team Size:** 1-3 developers

**Languages:** Primarily Rust

---

## Important: WASM Networking Reality vs. User Abstraction

The user-facing documentation presents networking in familiar terms (services "listen" on ports, "connect" to hosts). This is an intentional abstraction to make Fabricks approachable. However, **implementers must understand the reality**:

### What the Docs Say vs. What Actually Happens

| User-Facing Concept | Implementation Reality |
|---------------------|------------------------|
| `listen = [8080]` | The **daemon** binds the port. WASM modules implement `wasi:http/handler` or similar interfaces. Incoming requests are routed to the appropriate handler. |
| `connect = ["postgres:5432"]` | The **daemon** validates the capability, then either: (a) provides `wasi-sockets` primitives, or (b) exposes host-provided client interfaces. The WASM module never directly opens sockets. |
| Network segmentation | Enforced at the **daemon level**, not inside WASM. The daemon acts as a policy enforcement point, validating all connection attempts before proxying/allowing them. |
| Component Model imports | **Direct function calls** between linked WASM modules—no network involved. This is a genuine advantage over HTTP-based microservices. |

### Architecture Implications

1. **Daemon as Reverse Proxy**: For inbound traffic, fabricksd acts like nginx/envoy—it accepts connections and routes to WASM handlers.

2. **Daemon as Egress Proxy**: For outbound traffic, fabricksd validates capabilities and policies before allowing connections (or proxying them).

3. **WASI Interfaces**: We'll use WASI Preview 2 interfaces (`wasi:http`, `wasi:sockets`) where available. For databases and other protocols, we may need to provide host-implemented client components.

4. **Capability Enforcement**: The `[capabilities.network]` section maps directly to what the Wasmtime linker exposes. If `connect = ["postgres:5432"]` isn't listed, the host simply doesn't provide socket access to that destination.

### Why This Matters

- When implementing Phase 4 (WASM Runtime), capabilities are enforced by what we link into the WASM instance
- When implementing Phase 9 (Network Manager), we're building the daemon's proxy/routing layer, not WASM-level networking
- Health checks in Phase 8 are performed by the daemon against service endpoints, not by WASM modules checking themselves

---

## Prerequisites

### Required Tools
```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# WASM targets
rustup target add wasm32-wasi
rustup target add wasm32-unknown-unknown

# Development tools
cargo install cargo-watch
cargo install cargo-edit
cargo install wasm-opt
```

### Required Knowledge

- Rust programming (async, traits, error handling)
- TOML parsing and serialization
- HTTP/REST APIs
- OCI Distribution Specification
- WASM and Wasmtime basics
- Unix socket communication

---

## Phase 0: Project Setup

**Goal:** Create project structure and setup CI/CD

**Duration:** 1-2 days

### Tasks

1. **Create GitHub repository**
```bash
   mkdir fabricks
   cd fabricks
   git init
```

2. **Setup Cargo workspace**
   
   Create `Cargo.toml`:
```toml
   [workspace]
   members = [
       "fabricks",
       "fabricksd",
       "fabricks-common",
       "fabricks-oci",
       "fabricks-runtime",
   ]
   resolver = "2"
   
   [workspace.package]
   version = "0.1.0"
   edition = "2021"
   license = "MIT"
   authors = ["Your Name <you@example.com>"]
   
   [workspace.dependencies]
   tokio = { version = "1", features = ["full"] }
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   toml = "0.8"
   anyhow = "1"
   thiserror = "1"

   [workspace.lints.rust]
   unsafe_code = "deny"
   missing_docs = "deny"

   [workspace.lints.clippy]
   # Pedantic lints
   pedantic = { level = "deny", priority = -1 }

   # Unwrap is forbidden - use expect() with context or proper error handling
   unwrap_used = "deny"
   expect_used = "deny"

   # Additional strict lints
   panic = "deny"
   todo = "deny"
   unimplemented = "deny"
   dbg_macro = "deny"
   print_stdout = "deny"
   print_stderr = "deny"
```

3. **Create crate skeletons**
```bash
   cargo new --bin fabricks
   cargo new --bin fabricksd
   cargo new --lib fabricks-common
   cargo new --lib fabricks-oci
   cargo new --lib fabricks-runtime
```

4. **Setup GitHub Actions**
   
   Create `.github/workflows/ci.yml`:
```yaml
   name: CI
   
   on: [push, pull_request]
   
   jobs:
     test:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v3
         - uses: actions-rs/toolchain@v1
           with:
             toolchain: stable
         - run: cargo test --all
         - run: cargo clippy --all -- -D warnings
         - run: cargo fmt --all -- --check
```

5. **Add documentation files**
```bash
   cp docs/README.md .
   cp docs/ARCHITECTURE.md .
   cp docs/IMPLEMENTATION_PLAN.md .
   mkdir examples
```

### Success Criteria

- [x] Workspace compiles successfully
- [x] CI pipeline passes
- [x] Documentation is in place

---

## Phase 1: Core Data Models

**Goal:** Define core data structures for Fabrickfile and mortar files

**Duration:** 3-5 days

### Tasks

1. **Create `fabricks-common/src/models/fabrickfile.rs`**
```rust
   use serde::{Deserialize, Serialize};
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Fabrickfile {
       pub fabrick_version: String,
       pub info: Info,
       pub from: Option<From>,
       pub source: Option<Source>,
       pub runtime: Option<Runtime>,
       pub build: Option<Build>,
       pub exports: Option<Vec<String>>,
       pub imports: Option<Imports>,
       pub capabilities: Capabilities,
       pub files: Option<Files>,
       pub config: Option<Config>,
       pub health_check: Option<HealthCheck>,
       pub security: Option<Security>,
       pub labels: Option<Labels>,
       pub validate: Option<Validate>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Info {
       pub name: String,
       pub version: String,
       pub description: Option<String>,
       pub authors: Option<Vec<String>>,
       pub license: Option<String>,
       pub homepage: Option<String>,
       pub repository: Option<String>,
       pub documentation: Option<String>,
       pub keywords: Option<Vec<String>>,
   }
   
   // ... other structs
```

2. **Create `fabricks-common/src/models/mortar.rs`**
```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MortarFile {
       pub mortar_version: String,
       pub project: Project,
       pub variable: Option<HashMap<String, Variable>>,
       pub secret: Option<HashMap<String, Secret>>,
       pub network: Option<HashMap<String, Network>>,
       pub external_hosts: Option<HashMap<String, ExternalHosts>>,
       pub service: HashMap<String, Service>,
       pub volume: Option<HashMap<String, Volume>>,
       pub policy: Option<HashMap<String, Policy>>,
       pub validate: Option<Validate>,
       pub profile: Option<HashMap<String, Profile>>,
   }
   
   // ... other structs
```

3. **Create shared types**
   
   `fabricks-common/src/models/capability.rs`:
```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Capabilities {
       pub env: Option<Vec<String>>,
       pub network: Option<NetworkCapabilities>,
       pub filesystem: Option<FilesystemCapabilities>,
       pub wasm: Option<WasmCapabilities>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct NetworkCapabilities {
       pub listen: Option<Vec<u16>>,
       pub connect: Option<Vec<String>>,
       pub allow_all_outbound: Option<bool>,
   }
   
   // ... other capability types
```

4. **Add validation logic**
   
   `fabricks-common/src/validation.rs`:
```rust
   pub trait Validate {
       fn validate(&self) -> Result<(), ValidationError>;
   }
   
   impl Validate for Fabrickfile {
       fn validate(&self) -> Result<(), ValidationError> {
           // Check version
           if self.fabrick_version != "1.0" {
               return Err(ValidationError::UnsupportedVersion);
           }
           
           // Check name format
           if !is_valid_name(&self.info.name) {
               return Err(ValidationError::InvalidName);
           }
           
           // ... other validations
           
           Ok(())
       }
   }
```

### Success Criteria

- [x] All models defined and compile
- [x] Serde serialization/deserialization works
- [x] Basic validation logic in place
- [x] Unit tests for models

---

## Phase 2: File Parsers

**Goal:** Parse and validate Fabrickfile and mortar files

**Duration:** 2-3 days

### Tasks

1. **Create TOML parser**
   
   `fabricks-common/src/parser/toml_parser.rs`:
```rust
   use std::fs;
   use std::path::Path;
   
   pub fn parse_fabrickfile<P: AsRef<Path>>(path: P) -> Result<Fabrickfile> {
       let content = fs::read_to_string(path)?;
       let fabrickfile: Fabrickfile = toml::from_str(&content)?;
       fabrickfile.validate()?;
       Ok(fabrickfile)
   }
   
   pub fn parse_mortar_file<P: AsRef<Path>>(path: P) -> Result<MortarFile> {
       let content = fs::read_to_string(path)?;
       let mortar: MortarFile = toml::from_str(&content)?;
       mortar.validate()?;
       Ok(mortar)
   }
```

2. **Add comprehensive validation**
   
   - Name format validation (`[a-z0-9-]+`)
   - Version format validation (semver)
   - Port range validation (1-65535)
   - Path validation
   - Network reference validation
   - Service dependency cycle detection

3. **Create validator CLI utility**
   
   `fabricks/src/commands/validate.rs`:
```rust
   pub async fn validate(path: &Path, file_type: Option<FileType>) -> Result<()> {
       match file_type {
           Some(FileType::Fabrickfile) | None => {
               let fabrickfile = parse_fabrickfile(path.join("Fabrickfile"))?;
               println!("✓ Valid Fabrickfile");
           }
           Some(FileType::Mortar) => {
               let mortar = parse_mortar_file(path.join("fabricks-mortar.toml"))?;
               println!("✓ Valid mortar file");
           }
       }
       Ok(())
   }
```

### Success Criteria

- [x] Can parse valid Fabrickfile
- [x] Can parse valid mortar file
- [x] Validation errors provide helpful messages
- [x] `fabricks validate` command works
- [x] Integration tests with sample files

---

## Phase 3: OCI Registry Client

**Goal:** Push and pull WASM modules to/from OCI registries

**Duration:** 5-7 days

### Tasks

1. **Create OCI manifest builder**
   
   `fabricks-oci/src/manifest.rs`:
```rust
   use serde::{Deserialize, Serialize};
   
   #[derive(Debug, Serialize, Deserialize)]
   pub struct Manifest {
       #[serde(rename = "schemaVersion")]
       pub schema_version: u32,
       #[serde(rename = "mediaType")]
       pub media_type: String,
       #[serde(rename = "artifactType")]
       pub artifact_type: String,
       pub config: Descriptor,
       pub layers: Vec<Descriptor>,
       pub annotations: HashMap<String, String>,
   }
   
   pub struct ManifestBuilder {
       config_blob: Vec<u8>,
       wasm_blob: Vec<u8>,
       fabrickfile: Fabrickfile,
   }
   
   impl ManifestBuilder {
       pub fn build(self) -> Manifest {
           let config_digest = compute_digest(&self.config_blob);
           let wasm_digest = compute_digest(&self.wasm_blob);
           
           Manifest {
               schema_version: 2,
               media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
               artifact_type: "application/vnd.fabricks.module.v1".to_string(),
               config: Descriptor {
                   media_type: "application/vnd.fabricks.config.v1+toml".to_string(),
                   digest: config_digest,
                   size: self.config_blob.len() as i64,
               },
               layers: vec![
                   Descriptor {
                       media_type: "application/vnd.fabricks.module.v1+wasm".to_string(),
                       digest: wasm_digest,
                       size: self.wasm_blob.len() as i64,
                   }
               ],
               annotations: build_annotations(&self.fabrickfile),
           }
       }
   }
```

2. **Implement registry client**
   
   `fabricks-oci/src/client.rs`:
```rust
   use hyper::{Body, Client, Request, Response};
   use hyper_rustls::HttpsConnectorBuilder;
   
   pub struct RegistryClient {
       base_url: String,
       client: Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>,
       auth_token: Option<String>,
   }
   
   impl RegistryClient {
       pub async fn push_manifest(&self, name: &str, tag: &str, manifest: &Manifest) -> Result<String> {
           let url = format!("{}/v2/{}/manifests/{}", self.base_url, name, tag);
           let body = serde_json::to_vec(manifest)?;
           
           let req = Request::put(&url)
               .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
               .header("Authorization", format!("Bearer {}", self.auth_token.as_ref().unwrap()))
               .body(Body::from(body))?;
           
           let resp = self.client.request(req).await?;
           let digest = resp.headers()
               .get("Docker-Content-Digest")
               .unwrap()
               .to_str()?
               .to_string();
           
           Ok(digest)
       }
       
       pub async fn pull_manifest(&self, name: &str, tag: &str) -> Result<Manifest> {
           // Implementation
       }
       
       pub async fn upload_blob(&self, name: &str, blob: &[u8]) -> Result<String> {
           // Implementation with chunked upload
       }
       
       pub async fn download_blob(&self, name: &str, digest: &str) -> Result<Vec<u8>> {
           // Implementation
       }
   }
```

3. **Implement OAuth2 token flow**
   
   `fabricks-oci/src/auth.rs`:
```rust
   pub async fn get_token(realm: &str, service: &str, scope: &str, credentials: &Credentials) -> Result<String> {
       // Implement OAuth2 token request
   }
```

4. **Add credential management**
   
   `fabricks/src/registry/auth.rs`:
```rust
   pub fn load_credentials() -> Result<HashMap<String, Credentials>> {
       let path = dirs::home_dir().unwrap().join(".fabricks/credentials.json");
       let content = fs::read_to_string(path)?;
       Ok(serde_json::from_str(&content)?)
   }
   
   pub fn save_credentials(creds: &HashMap<String, Credentials>) -> Result<()> {
       let path = dirs::home_dir().unwrap().join(".fabricks/credentials.json");
       fs::write(path, serde_json::to_string_pretty(creds)?)?;
       Ok(())
   }
```

5. **Implement local storage**
   
   `fabricks-oci/src/storage.rs`:
```rust
   pub struct LocalStorage {
       base_path: PathBuf,
   }
   
   impl LocalStorage {
       pub fn store_blob(&self, digest: &str, data: &[u8]) -> Result<()> {
           let path = self.base_path.join("blobs/sha256").join(digest);
           fs::create_dir_all(path.parent().unwrap())?;
           fs::write(path, data)?;
           Ok(())
       }
       
       pub fn get_blob(&self, digest: &str) -> Result<Vec<u8>> {
           let path = self.base_path.join("blobs/sha256").join(digest);
           Ok(fs::read(path)?)
       }
       
       pub fn update_index(&self, manifest: &Manifest, reference: &str) -> Result<()> {
           // Update index.json and manifest references
       }
   }
```

### Success Criteria

- [x] Can push WASM module to a OCI Registry
- [x] Can pull WASM module from a OCI Registry
- [x] Content verification (SHA256) works
- [x] Local caching works
- [x] Authentication with major registries works
- [x] Integration tests with test registry

---

## Phase 4: WASM Runtime Integration

**Goal:** Execute WASM modules with Wasmtime

**Duration:** 4-6 days

### Tasks

1. **Create runtime wrapper**
   
   `fabricks-runtime/src/wasmtime.rs`:
```rust
   use wasmtime::*;
   use wasmtime_wasi::WasiCtxBuilder;
   
   pub struct WasmtimeRuntime {
       engine: Engine,
       module: Module,
       capabilities: Capabilities,
   }
   
   impl WasmtimeRuntime {
       pub fn new(wasm_bytes: &[u8], capabilities: Capabilities) -> Result<Self> {
           let engine = Engine::default();
           let module = Module::new(&engine, wasm_bytes)?;
           
           Ok(Self {
               engine,
               module,
               capabilities,
           })
       }
       
       pub fn run(&self) -> Result<()> {
           let mut linker = Linker::new(&self.engine);
           
           // Add WASI
           wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;
           
           // Create WASI context with capabilities
           let wasi = WasiCtxBuilder::new()
               .inherit_stdio()
               .inherit_args()?
               .envs(&self.get_allowed_env())?
               .preopened_dir(/* based on filesystem capabilities */)?
               .build();
           
           let mut store = Store::new(&self.engine, wasi);
           
           // Instantiate and run
           let instance = linker.instantiate(&mut store, &self.module)?;
           let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
           start.call(&mut store, ())?;
           
           Ok(())
       }
       
       fn get_allowed_env(&self) -> Vec<(String, String)> {
           // Filter environment variables based on capabilities
       }
   }
```

2. **Implement capability enforcement**
```rust
   impl WasmtimeRuntime {
       fn check_network_capability(&self, host: &str, port: u16) -> Result<()> {
           let allowed = self.capabilities.network
               .as_ref()
               .and_then(|n| n.connect.as_ref())
               .map(|connects| {
                   connects.iter().any(|c| c == &format!("{}:{}", host, port))
               })
               .unwrap_or(false);
           
           if !allowed {
               return Err(Error::CapabilityDenied(format!("connect:{}:{}", host, port)));
           }
           
           Ok(())
       }
   }
```

3. **Add instance pooling**
   
   `fabricks-runtime/src/pool.rs`:
```rust
   pub struct RuntimePool {
       pool: Vec<WasmtimeRuntime>,
       max_size: usize,
   }
   
   impl RuntimePool {
       pub async fn acquire(&mut self) -> Result<WasmtimeRuntime> {
           // Get or create runtime instance
       }
       
       pub async fn release(&mut self, runtime: WasmtimeRuntime) {
           // Return to pool
       }
   }
```

4. **Create Component Model support** (future enhancement)
```rust
   // Basic structure for Component Model
   // Full implementation in later phase
```

### Success Criteria

- [x] Can execute simple WASM module
- [x] Environment variable filtering works
- [x] Network capability checking works
- [x] Filesystem capability checking works
- [x] Can run HTTP server WASM module
- [x] Unit tests for capability enforcement

---

## Phase 5: Basic CLI

**Goal:** Implement core CLI commands

**Duration:** 5-7 days

### Tasks

1. **Setup CLI structure with clap**
   
   `fabricks/src/main.rs`:
```rust
   use clap::{Parser, Subcommand};
   
   #[derive(Parser)]
   #[command(name = "fabricks")]
   #[command(about = "WASM orchestration platform")]
   struct Cli {
       #[command(subcommand)]
       command: Commands,
   }
   
   #[derive(Subcommand)]
   enum Commands {
       Build(BuildArgs),
       Run(RunArgs),
       Push(PushArgs),
       Pull(PullArgs),
       Validate(ValidateArgs),
       Inspect(InspectArgs),
       Login(LoginArgs),
       Logout(LogoutArgs),
       #[command(subcommand)]
       Mortar(MortarCommands),
       #[command(subcommand)]
       Daemon(DaemonCommands),
   }
```

2. **Implement `fabricks build`**
   
   `fabricks/src/commands/build.rs`:
```rust
   pub async fn build(args: BuildArgs) -> Result<()> {
       // 1. Parse Fabrickfile
       let fabrickfile = parse_fabrickfile(&args.path)?;
       
       // 2. Run build command
       let output = Command::new("sh")
           .arg("-c")
           .arg(&fabrickfile.build.unwrap().command)
           .current_dir(&args.path)
           .output()?;
       
       if !output.status.success() {
           return Err(Error::BuildFailed);
       }
       
       // 3. Read WASM output
       let wasm_bytes = fs::read(args.path.join(fabrickfile.build.unwrap().output))?;
       
       // 4. Create OCI manifest
       let config_blob = toml::to_vec(&fabrickfile)?;
       let manifest = ManifestBuilder::new()
           .config_blob(config_blob)
           .wasm_blob(wasm_bytes)
           .fabrickfile(fabrickfile)
           .build();
       
       // 5. Store locally
       let storage = LocalStorage::new()?;
       storage.store_manifest(&manifest, &args.tag)?;
       
       println!("✓ Built {}", args.tag);
       Ok(())
   }
```

3. **Implement `fabricks run`**
   
   `fabricks/src/commands/run.rs`:
```rust
   pub async fn run(args: RunArgs) -> Result<()> {
       // 1. Load image (local or pull)
       let (fabrickfile, wasm_bytes) = if args.image.starts_with("wasm://") {
           pull_and_extract(&args.image).await?
       } else {
           load_local(&args.image)?
       };
       
       // 2. Create runtime
       let runtime = WasmtimeRuntime::new(&wasm_bytes, fabrickfile.capabilities)?;
       
       // 3. Run
       runtime.run()?;
       
       Ok(())
   }
```

4. **Implement `fabricks push`**
   
   `fabricks/src/commands/push.rs`:
```rust
   pub async fn push(args: PushArgs) -> Result<()> {
       // 1. Load manifest from local storage
       let storage = LocalStorage::new()?;
       let manifest = storage.get_manifest(&args.image)?;
       
       // 2. Parse registry URL
       let (registry, name, tag) = parse_image(&args.image)?;
       
       // 3. Create registry client
       let creds = load_credentials()?;
       let client = RegistryClient::new(registry, creds.get(registry))?;
       
       // 4. Upload blobs
       for layer in &manifest.layers {
           let blob = storage.get_blob(&layer.digest)?;
           client.upload_blob(name, &blob).await?;
       }
       
       // 5. Upload config
       let config = storage.get_blob(&manifest.config.digest)?;
       client.upload_blob(name, &config).await?;
       
       // 6. Upload manifest
       let digest = client.push_manifest(name, tag, &manifest).await?;
       
       println!("✓ Pushed {}", args.image);
       println!("Digest: {}", digest);
       Ok(())
   }
```

5. **Implement `fabricks pull`**
   
   Similar to push but in reverse

6. **Implement `fabricks login` and `fabricks logout`**
```rust
   pub async fn login(args: LoginArgs) -> Result<()> {
       let username = args.username.unwrap_or_else(|| {
           print!("Username: ");
           read_line()
       });
       
       let password = rpassword::prompt_password("Password: ")?;
       
       let mut creds = load_credentials()?;
       creds.insert(args.registry.clone(), Credentials {
           username,
           password,
       });
       save_credentials(&creds)?;
       
       println!("✓ Logged in to {}", args.registry);
       Ok(())
   }
```

### Success Criteria

- [x] `fabricks build` compiles WASM module
- [x] `fabricks run` executes WASM module
- [x] `fabricks push` uploads to an OCI Registry
- [x] `fabricks pull` downloads from an OCI Registry
- [x] `fabricks login` saves credentials
- [x] `fabricks validate` checks Fabrickfile
- [x] End-to-end workflow works

---

## Phase 6: Daemon Foundation

**Goal:** Create basic daemon with HTTP API

**Duration:** 5-7 days

### Tasks

1. **Setup daemon structure**
   
   `fabricksd/src/main.rs`:
```rust
   use axum::{Router, routing::get};
   use tokio::net::UnixListener;
   
   #[tokio::main]
   async fn main() -> Result<()> {
       // Load config
       let config = DaemonConfig::load()?;
       
       // Initialize components
       let state = AppState::new(config)?;
       
       // Setup routes
       let app = Router::new()
           .route("/v1/daemon/info", get(handlers::daemon_info))
           .route("/v1/services", get(handlers::list_services))
           .with_state(state);
       
       // Create Unix socket
       let _ = std::fs::remove_file(&config.socket);
       let listener = UnixListener::bind(&config.socket)?;
       
       println!("fabricksd listening on {}", config.socket);
       
       // Run server
       axum::serve(listener, app).await?;
       
       Ok(())
   }
```

2. **Create shared state**
   
   `fabricksd/src/state.rs`:
```rust
   use sled::Db;
   use tokio::sync::RwLock;
   
   #[derive(Clone)]
   pub struct AppState {
       pub config: Arc<DaemonConfig>,
       pub db: Arc<Db>,
       pub service_manager: Arc<RwLock<ServiceManager>>,
       pub network_manager: Arc<RwLock<NetworkManager>>,
       pub volume_manager: Arc<RwLock<VolumeManager>>,
       pub event_bus: Arc<EventBus>,
   }
   
   impl AppState {
       pub fn new(config: DaemonConfig) -> Result<Self> {
           let db = sled::open(&config.data_dir.join("state.db"))?;
           
           Ok(Self {
               config: Arc::new(config),
               db: Arc::new(db),
               service_manager: Arc::new(RwLock::new(ServiceManager::new())),
               network_manager: Arc::new(RwLock::new(NetworkManager::new())),
               volume_manager: Arc::new(RwLock::new(VolumeManager::new())),
               event_bus: Arc::new(EventBus::new()),
           })
       }
   }
```

3. **Implement basic API handlers**
   
   `fabricksd/src/api/handlers/daemon.rs`:
```rust
   use axum::{extract::State, Json};
   
   pub async fn daemon_info(
       State(state): State<AppState>,
   ) -> Json<ApiResponse<DaemonInfo>> {
       Json(ApiResponse::success(DaemonInfo {
           version: env!("CARGO_PKG_VERSION").to_string(),
           api_version: "v1".to_string(),
           runtime: "wasmtime".to_string(),
           started_at: state.config.started_at,
           uptime: state.config.started_at.elapsed(),
       }))
   }
```

4. **Add state persistence**
   
   `fabricksd/src/state/store.rs`:
```rust
   pub struct StateStore {
       db: Arc<Db>,
   }
   
   impl StateStore {
       pub fn save_service(&self, service: &ServiceState) -> Result<()> {
           let tree = self.db.open_tree("services")?;
           let json = serde_json::to_vec(service)?;
           tree.insert(service.id.as_bytes(), json)?;
           Ok(())
       }
       
       pub fn load_services(&self) -> Result<Vec<ServiceState>> {
           let tree = self.db.open_tree("services")?;
           let mut services = Vec::new();
           
           for item in tree.iter() {
               let (_, value) = item?;
               let service: ServiceState = serde_json::from_slice(&value)?;
               services.push(service);
           }
           
           Ok(services)
       }
   }
```

5. **Add event bus**
   
   `fabricksd/src/events/bus.rs`:
```rust
   pub struct EventBus {
       subscribers: Arc<RwLock<Vec<mpsc::Sender<Event>>>>,
   }
   
   impl EventBus {
       pub async fn publish(&self, event: Event) {
           let subs = self.subscribers.read().await;
           for tx in subs.iter() {
               let _ = tx.send(event.clone()).await;
           }
       }
       
       pub async fn subscribe(&self) -> mpsc::Receiver<Event> {
           let (tx, rx) = mpsc::channel(100);
           self.subscribers.write().await.push(tx);
           rx
       }
   }
```

### Success Criteria

- [x] Daemon starts successfully
- [x] Unix socket created
- [x] `/v1/daemon/info` endpoint works
- [x] State persists to sled database
- [x] Event bus publishes events
- [x] Can query daemon from CLI

---

## Phase 7: Service Manager & CLI Integration

**Goal:** Manage service lifecycle through the daemon, with full CLI integration. The daemon is the default execution environment for all operations.

**Duration:** 10-14 days

### Architecture Note

All CLI commands that execute WASM modules go through the daemon. The daemon is the single point of orchestration - there is no "local-only" mode for running services. This ensures consistent behavior, proper capability enforcement, and centralized state management.

### Tasks

#### Part A: Daemon Service Manager

1. **Implement service manager**

   `fabricksd/src/service/manager.rs`:
```rust
   pub struct ServiceManager {
       services: HashMap<String, ServiceHandle>,
       runtime_pool: RuntimePool,
       state_store: Arc<StateStore>,
       event_bus: Arc<EventBus>,
   }

   impl ServiceManager {
       pub async fn create_service(&mut self, config: ServiceConfig) -> Result<String> {
           let id = generate_id();

           let state = ServiceState {
               id: id.clone(),
               name: config.name.clone(),
               state: State::Creating,
               replicas: ReplicaState::default(),
               created_at: Utc::now(),
           };

           self.state_store.save_service(&state)?;

           let handle = ServiceHandle::new(id.clone(), config, self.runtime_pool.clone());
           self.services.insert(id.clone(), handle);

           self.event_bus.publish(Event::ServiceCreated { id: id.clone() }).await;

           Ok(id)
       }

       pub async fn start_service(&mut self, id: &str) -> Result<()> {
           let handle = self.services.get_mut(id).ok_or(Error::NotFound)?;
           handle.start().await?;

           self.event_bus.publish(Event::ServiceStarted { id: id.to_string() }).await;

           Ok(())
       }

       pub async fn stop_service(&mut self, id: &str) -> Result<()> {
           let handle = self.services.get_mut(id).ok_or(Error::NotFound)?;
           handle.stop().await?;

           self.event_bus.publish(Event::ServiceStopped { id: id.to_string() }).await;

           Ok(())
       }

       pub async fn scale_service(&mut self, id: &str, replicas: usize) -> Result<()> {
           let handle = self.services.get_mut(id).ok_or(Error::NotFound)?;
           handle.scale(replicas).await?;

           self.event_bus.publish(Event::ServiceScaled {
               id: id.to_string(),
               replicas,
           }).await;

           Ok(())
       }
   }
```

2. **Create service handle**

   `fabricksd/src/service/handle.rs`:
```rust
   pub struct ServiceHandle {
       id: String,
       config: ServiceConfig,
       instances: Vec<Instance>,
       runtime_pool: RuntimePool,
   }

   impl ServiceHandle {
       pub async fn start(&mut self) -> Result<()> {
           for _ in 0..self.config.replicas.min {
               let instance = self.spawn_instance().await?;
               self.instances.push(instance);
           }
           Ok(())
       }

       async fn spawn_instance(&self) -> Result<Instance> {
           let runtime = self.runtime_pool.acquire().await?;
           let instance_id = format!("{}-{}", self.id, self.instances.len());

           tokio::spawn(async move {
               runtime.run().await
           });

           Ok(Instance {
               id: instance_id,
               state: InstanceState::Running,
               started_at: Utc::now(),
           })
       }

       pub async fn scale(&mut self, target: usize) -> Result<()> {
           let current = self.instances.len();

           if target > current {
               // Scale up
               for _ in 0..(target - current) {
                   let instance = self.spawn_instance().await?;
                   self.instances.push(instance);
               }
           } else if target < current {
               // Scale down
               for _ in 0..(current - target) {
                   if let Some(instance) = self.instances.pop() {
                       instance.stop().await?;
                   }
               }
           }

           Ok(())
       }
   }
```

3. **Add dependency resolution**

   `fabricksd/src/service/dependency.rs`:
```rust
   pub fn resolve_startup_order(services: &[ServiceConfig]) -> Result<Vec<String>> {
       let mut graph = DiGraph::new();
       let mut nodes = HashMap::new();

       // Build dependency graph
       for service in services {
           let node = graph.add_node(service.name.clone());
           nodes.insert(service.name.clone(), node);
       }

       for service in services {
           let from = nodes[&service.name];
           for dep in &service.depends_on {
               let to = nodes[dep];
               graph.add_edge(from, to, ());
           }
       }

       // Topological sort
       match petgraph::algo::toposort(&graph, None) {
           Ok(order) => {
               Ok(order.into_iter()
                   .map(|n| graph[n].clone())
                   .collect())
           }
           Err(_) => Err(Error::CircularDependency),
       }
   }
```

#### Part B: Daemon API Endpoints

4. **Add service API handlers**

   `fabricksd/src/api/handlers/services.rs`:
```rust
   // POST /v1/services - Create and start a service from Fabrickfile
   pub async fn create_service(
       State(state): State<AppState>,
       Json(req): Json<CreateServiceRequest>,
   ) -> Json<ApiResponse<CreateServiceResponse>> {
       let mut manager = state.service_manager.write().await;

       match manager.create_service(req.into()).await {
           Ok(id) => Json(ApiResponse::success(CreateServiceResponse { id })),
           Err(e) => Json(ApiResponse::error(e)),
       }
   }

   // GET /v1/services - List all services
   pub async fn list_services(
       State(state): State<AppState>,
   ) -> Json<ApiResponse<Vec<ServiceInfo>>> {
       let manager = state.service_manager.read().await;
       let services = manager.list_services();
       Json(ApiResponse::success(services))
   }

   // GET /v1/services/:id - Get service details
   pub async fn get_service(
       State(state): State<AppState>,
       Path(id): Path<String>,
   ) -> Json<ApiResponse<ServiceDetail>> {
       let manager = state.service_manager.read().await;

       match manager.get_service(&id) {
           Some(service) => Json(ApiResponse::success(service)),
           None => Json(ApiResponse::error(Error::NotFound)),
       }
   }

   // POST /v1/services/:id/stop - Stop a service
   pub async fn stop_service(
       State(state): State<AppState>,
       Path(id): Path<String>,
   ) -> Json<ApiResponse<()>> {
       let mut manager = state.service_manager.write().await;

       match manager.stop_service(&id).await {
           Ok(()) => Json(ApiResponse::success(())),
           Err(e) => Json(ApiResponse::error(e)),
       }
   }

   // DELETE /v1/services/:id - Remove a service
   pub async fn delete_service(
       State(state): State<AppState>,
       Path(id): Path<String>,
   ) -> Json<ApiResponse<()>> {
       let mut manager = state.service_manager.write().await;

       match manager.delete_service(&id).await {
           Ok(()) => Json(ApiResponse::success(())),
           Err(e) => Json(ApiResponse::error(e)),
       }
   }

   // GET /v1/services/:id/logs - Get service logs
   pub async fn get_service_logs(
       State(state): State<AppState>,
       Path(id): Path<String>,
       Query(params): Query<LogParams>,
   ) -> Json<ApiResponse<Vec<LogEntry>>> {
       let manager = state.service_manager.read().await;

       match manager.get_logs(&id, params.lines, params.follow).await {
           Ok(logs) => Json(ApiResponse::success(logs)),
           Err(e) => Json(ApiResponse::error(e)),
       }
   }
```

#### Part C: CLI Commands (Daemon Integration)

5. **Update `fabricks run` to use daemon**

   `fabricks/src/commands/run.rs`:
```rust
   pub async fn run(args: &RunArgs) -> Result<()> {
       let client = DaemonClient::new();

       // 1. Parse the Fabrickfile
       let fabrickfile = if args.module.ends_with(".wasm") {
           // Direct WASM file - create minimal config
           Fabrickfile::from_wasm_path(&args.module)?
       } else {
           // Directory with Fabrickfile
           parse_fabrickfile(&args.module)?
       };

       // 2. Send to daemon to create and start service
       let service_id = client.create_service(&fabrickfile).await?;
       client.start_service(&service_id).await?;

       output::writeln(&format!("Service started: {}", service_id))?;

       // 3. If --attach, stream logs
       if args.attach {
           client.stream_logs(&service_id).await?;
       }

       Ok(())
   }
```

6. **Add `fabricks service` subcommands**

   `fabricks/src/commands/service.rs`:
```rust
   // fabricks service ls
   pub async fn list(args: &ServiceListArgs) -> Result<()> {
       let client = DaemonClient::new();
       let services = client.list_services().await?;

       if services.is_empty() {
           output::writeln("No services running")?;
           return Ok(());
       }

       output::writeln("ID                    NAME              STATUS    REPLICAS")?;
       for svc in services {
           output::writeln(&format!(
               "{:<21} {:<17} {:<9} {}/{}",
               svc.id, svc.name, svc.status, svc.running_replicas, svc.desired_replicas
           ))?;
       }
       Ok(())
   }

   // fabricks service logs <id>
   pub async fn logs(args: &ServiceLogsArgs) -> Result<()> {
       let client = DaemonClient::new();

       if args.follow {
           client.stream_logs(&args.service_id).await?;
       } else {
           let logs = client.get_logs(&args.service_id, args.lines).await?;
           for entry in logs {
               output::writeln(&format!("{} {}", entry.timestamp, entry.message))?;
           }
       }
       Ok(())
   }

   // fabricks service stop <id>
   pub async fn stop(args: &ServiceStopArgs) -> Result<()> {
       let client = DaemonClient::new();
       client.stop_service(&args.service_id).await?;
       output::writeln(&format!("Stopped service: {}", args.service_id))?;
       Ok(())
   }

   // fabricks service rm <id>
   pub async fn remove(args: &ServiceRemoveArgs) -> Result<()> {
       let client = DaemonClient::new();
       client.delete_service(&args.service_id).await?;
       output::writeln(&format!("Removed service: {}", args.service_id))?;
       Ok(())
   }
```

7. **Add `fabricks mortar` subcommands**

   `fabricks/src/commands/mortar.rs`:
```rust
   // fabricks mortar up
   pub async fn up(args: &MortarUpArgs) -> Result<()> {
       let client = DaemonClient::new();

       // 1. Parse mortar file
       let mortar = parse_mortar_file(&args.path)?;

       // 2. Resolve dependency order
       let order = resolve_startup_order(&mortar.services)?;

       // 3. Create and start each service in order
       for service_name in &order {
           let service_config = &mortar.services[service_name];
           output::writeln(&format!("Starting {}...", service_name))?;

           let id = client.create_service_from_mortar(&mortar.project.name, service_name, service_config).await?;
           client.start_service(&id).await?;

           output::writeln(&format!("  ✓ {} started ({})", service_name, id))?;
       }

       output::writeln(&format!("\n✓ All {} services started", order.len()))?;
       Ok(())
   }

   // fabricks mortar down
   pub async fn down(args: &MortarDownArgs) -> Result<()> {
       let client = DaemonClient::new();

       // 1. Parse mortar file to get project name
       let mortar = parse_mortar_file(&args.path)?;

       // 2. Find all services belonging to this project
       let services = client.list_services_by_project(&mortar.project.name).await?;

       // 3. Stop and remove each service
       for svc in services {
           output::writeln(&format!("Stopping {}...", svc.name))?;
           client.stop_service(&svc.id).await?;
           client.delete_service(&svc.id).await?;
       }

       output::writeln(&format!("\n✓ All services stopped"))?;
       Ok(())
   }

   // fabricks mortar ps
   pub async fn ps(args: &MortarPsArgs) -> Result<()> {
       let client = DaemonClient::new();
       let mortar = parse_mortar_file(&args.path)?;

       let services = client.list_services_by_project(&mortar.project.name).await?;

       output::writeln(&format!("Project: {}\n", mortar.project.name))?;
       output::writeln("SERVICE           STATUS    REPLICAS")?;
       for svc in services {
           output::writeln(&format!(
               "{:<17} {:<9} {}/{}",
               svc.name, svc.status, svc.running_replicas, svc.desired_replicas
           ))?;
       }
       Ok(())
   }
```

8. **Update CLI argument definitions**

   `fabricks/src/cli.rs` - Add new subcommands:
```rust
   #[derive(Subcommand, Debug)]
   pub enum Commands {
       // ... existing commands ...

       /// Service management commands.
       Service(ServiceArgs),

       /// Multi-service composition commands.
       Mortar(MortarArgs),
   }

   #[derive(Args, Debug)]
   pub struct ServiceArgs {
       #[command(subcommand)]
       pub command: ServiceCommands,
   }

   #[derive(Subcommand, Debug)]
   pub enum ServiceCommands {
       /// List running services.
       Ls,
       /// Show service logs.
       Logs(ServiceLogsArgs),
       /// Stop a service.
       Stop(ServiceStopArgs),
       /// Remove a service.
       Rm(ServiceRemoveArgs),
       /// Show service details.
       Inspect(ServiceInspectArgs),
   }

   #[derive(Args, Debug)]
   pub struct MortarArgs {
       #[command(subcommand)]
       pub command: MortarCommands,
   }

   #[derive(Subcommand, Debug)]
   pub enum MortarCommands {
       /// Start all services defined in fabricks-mortar.toml.
       Up(MortarUpArgs),
       /// Stop all services defined in fabricks-mortar.toml.
       Down(MortarDownArgs),
       /// Show status of services.
       Ps(MortarPsArgs),
       /// Restart services.
       Restart(MortarRestartArgs),
       /// View logs from all services.
       Logs(MortarLogsArgs),
   }
```

### Example Fabrickfile for Testing

Create `examples/hello-world/Fabrickfile`:
```toml
[service]
name = "hello-world"
version = "1.0.0"
description = "Simple hello world WASM service"

[build]
command = "cargo build --target wasm32-wasi --release"
output = "target/wasm32-wasi/release/hello_world.wasm"

[runtime]
engine = "wasmtime"

[capabilities]
# Minimal capabilities for hello world
```

Create `examples/hello-world/src/main.rs`:
```rust
fn main() {
    println!("Hello from Fabricks!");
}
```

### Example Mortar File for Testing

Create `examples/web-app/fabricks-mortar.toml`:
```toml
mortar_version = "1.0"

[project]
name = "web-app-example"
description = "Example web application with API and worker"

[service.api]
fabrickfile = "./api/Fabrickfile"
replicas = { min = 1, max = 3 }

[service.worker]
fabrickfile = "./worker/Fabrickfile"
replicas = { min = 1, max = 2 }
depends_on = ["api"]
```

### Success Criteria

#### Daemon API
- [ ] `POST /v1/services` creates a service from Fabrickfile
- [ ] `GET /v1/services` lists all running services
- [ ] `GET /v1/services/:id` returns service details
- [ ] `POST /v1/services/:id/stop` stops a service
- [ ] `DELETE /v1/services/:id` removes a service
- [ ] `GET /v1/services/:id/logs` returns service logs
- [ ] Dependency resolution prevents circular dependencies
- [ ] State persists across daemon restarts
- [ ] Events published for all service lifecycle changes

#### CLI Commands (End-to-End)
- [ ] `fabricks run ./examples/hello-world` starts service via daemon
- [ ] `fabricks service ls` shows running services
- [ ] `fabricks service logs <id>` shows service output
- [ ] `fabricks service stop <id>` stops a service
- [ ] `fabricks service rm <id>` removes a service
- [ ] `fabricks mortar up` starts all services from mortar file
- [ ] `fabricks mortar ps` shows project service status
- [ ] `fabricks mortar down` stops all project services

#### Integration Tests
- [ ] Build and run hello-world example end-to-end
- [ ] Build and run web-app mortar example end-to-end
- [ ] Service logs are captured and retrievable
- [ ] Daemon restart preserves service state
- [ ] CLI provides clear errors when daemon not running

---

## Phase 8: Health Monitoring

**Goal:** Monitor service health and handle failures

**Duration:** 4-5 days

### Tasks

1. **Implement health monitor**
   
   `fabricksd/src/health/monitor.rs`:
```rust
   pub struct HealthMonitor {
       checks: Arc<RwLock<HashMap<String, HealthCheckConfig>>>,
       history: Arc<RwLock<HashMap<String, Vec<HealthCheckResult>>>>,
       event_bus: Arc<EventBus>,
   }
   
   impl HealthMonitor {
       pub async fn start(&self) {
           loop {
               let checks = self.checks.read().await.clone();
               
               for (service_id, config) in checks {
                   let result = self.perform_check(&service_id, &config).await;
                   self.record_result(&service_id, result.clone()).await;
                   
                   if result.status != HealthStatus::Healthy {
                       self.event_bus.publish(Event::HealthChanged {
                           service_id,
                           status: result.status,
                       }).await;
                   }
               }
               
               tokio::time::sleep(Duration::from_secs(5)).await;
           }
       }
       
       async fn perform_check(&self, service_id: &str, config: &HealthCheckConfig) -> HealthCheckResult {
           match &config.check_type {
               HealthCheckType::Http { path, port, .. } => {
                   self.http_check(service_id, path, *port).await
               }
               HealthCheckType::Tcp { port } => {
                   self.tcp_check(service_id, *port).await
               }
               HealthCheckType::Exec { command } => {
                   self.exec_check(service_id, command).await
               }
           }
       }
   }
```

2. **Implement check types**
   
   `fabricksd/src/health/http_check.rs`:
```rust
   pub async fn http_check(&self, service_id: &str, path: &str, port: u16) -> HealthCheckResult {
       let url = format!("http://localhost:{}{}", port, path);
       
       match reqwest::get(&url).await {
           Ok(resp) if resp.status().is_success() => {
               HealthCheckResult {
                   timestamp: Utc::now(),
                   status: HealthStatus::Healthy,
                   response_time: resp.elapsed(),
               }
           }
           _ => {
               HealthCheckResult {
                   timestamp: Utc::now(),
                   status: HealthStatus::Unhealthy,
                   response_time: None,
               }
           }
       }
   }
```

3. **Add restart policy**
   
   `fabricksd/src/health/restart.rs`:
```rust
   pub async fn handle_failure(
       &self,
       service_id: &str,
       policy: &RestartPolicy,
   ) -> Result<()> {
       let attempts = self.get_restart_attempts(service_id);
       
       match policy.policy {
           Policy::Always => {
               self.restart_service(service_id).await?;
           }
           Policy::OnFailure => {
               if attempts < policy.max_attempts {
                   tokio::time::sleep(Duration::from_secs(policy.backoff_secs)).await;
                   self.restart_service(service_id).await?;
               }
           }
           Policy::Never => {}
       }
       
       Ok(())
   }
```

### Success Criteria

- [x] HTTP health checks work
- [x] TCP health checks work
- [x] Exec health checks work
- [x] Health history tracked
- [x] Restart on failure works
- [x] Events published on health change

---

## Phase 9: Network Manager

**Goal:** Create and manage network segmentation

**Duration:** 5-6 days

### Tasks

1. **Implement network manager**
   
   `fabricksd/src/network/manager.rs`:
```rust
   pub struct NetworkManager {
       networks: HashMap<String, Network>,
       state_store: Arc<StateStore>,
       policy_engine: Arc<PolicyEngine>,
   }
   
   impl NetworkManager {
       pub async fn create_network(&mut self, config: NetworkConfig) -> Result<String> {
           let id = generate_id();
           
           let network = Network {
               id: id.clone(),
               name: config.name.clone(),
               internal: config.internal,
               isolated: config.isolated,
               services: Vec::new(),
           };
           
           self.networks.insert(id.clone(), network.clone());
           self.state_store.save_network(&network)?;
           
           Ok(id)
       }
       
       pub fn can_communicate(&self, from: &str, to: &str) -> bool {
           // Check if services share a network
           let from_networks = self.get_service_networks(from);
           let to_networks = self.get_service_networks(to);
           
           from_networks.iter().any(|n| to_networks.contains(n))
       }
   }
```

2. **Add policy validation**
   
   `fabricksd/src/network/policy.rs`:
```rust
   pub fn validate_connection(
       &self,
       from_service: &Service,
       to_host: &str,
       to_port: u16,
   ) -> Result<()> {
       // 1. Check network membership
       if !self.network_manager.can_communicate(&from_service.id, to_host) {
           return Err(Error::NetworkIsolation);
       }
       
       // 2. Check capability
       let has_capability = from_service.capabilities.network
           .as_ref()
           .and_then(|n| n.connect.as_ref())
           .map(|connects| connects.contains(&format!("{}:{}", to_host, to_port)))
           .unwrap_or(false);
       
       if !has_capability {
           return Err(Error::CapabilityDenied);
       }
       
       // 3. Check policies
       self.policy_engine.validate_connection(from_service, to_host, to_port)?;
       
       Ok(())
   }
```

### Success Criteria

- [x] Can create networks
- [x] Services assigned to networks
- [x] Network isolation enforced
- [x] Policy validation works
- [x] API endpoints functional

---

## Phase 10: Volume Manager

**Goal:** Manage persistent volumes

**Duration:** 3-4 days

### Tasks

1. **Implement volume manager**
   
   `fabricksd/src/volume/manager.rs`:
```rust
   pub struct VolumeManager {
       volumes: HashMap<String, Volume>,
       base_path: PathBuf,
       state_store: Arc<StateStore>,
   }
   
   impl VolumeManager {
       pub async fn create_volume(&mut self, config: VolumeConfig) -> Result<String> {
           let id = generate_id();
           let path = self.base_path.join(&id);
           
           fs::create_dir_all(&path)?;
           
           let volume = Volume {
               id: id.clone(),
               name: config.name.clone(),
               size: config.size,
               path: path.clone(),
               mounted_by: Vec::new(),
           };
           
           self.volumes.insert(id.clone(), volume.clone());
           self.state_store.save_volume(&volume)?;
           
           Ok(id)
       }
       
       pub fn mount_volume(&mut self, volume_id: &str, service_id: &str) -> Result<PathBuf> {
           let volume = self.volumes.get_mut(volume_id).ok_or(Error::NotFound)?;
           volume.mounted_by.push(service_id.to_string());
           Ok(volume.path.clone())
       }
   }
```

2. **Add backup scheduling** (basic implementation)

### Success Criteria

- [x] Can create volumes
- [x] Can mount volumes to services
- [x] Volume persistence works
- [x] API endpoints functional

---

## Phase 11: Auto-Scaler

**Goal:** Automatically scale services based on metrics

**Duration:** 4-5 days

### Tasks

1. **Implement metrics collection**
   
   `fabricksd/src/scaler/metrics.rs`:
```rust
   pub struct MetricsCollector {
       metrics: Arc<RwLock<HashMap<String, ServiceMetrics>>>,
   }
   
   impl MetricsCollector {
       pub async fn collect(&self, service_id: &str) -> ServiceMetrics {
           // Collect CPU, memory, request rate
           // This is simplified - real implementation would use proper metrics
           ServiceMetrics {
               cpu_usage: self.get_cpu_usage(service_id),
               memory_usage: self.get_memory_usage(service_id),
               request_rate: self.get_request_rate(service_id),
           }
       }
   }
```

2. **Implement auto-scaler**
   
   `fabricksd/src/scaler/autoscaler.rs`:
```rust
   pub struct AutoScaler {
       service_manager: Arc<RwLock<ServiceManager>>,
       metrics_collector: MetricsCollector,
       cooldown: HashMap<String, Instant>,
   }
   
   impl AutoScaler {
       pub async fn run(&mut self) {
           loop {
               let services = self.get_scalable_services();
               
               for service in services {
                   if self.in_cooldown(&service.id) {
                       continue;
                   }
                   
                   let metrics = self.metrics_collector.collect(&service.id).await;
                   
                   if metrics.cpu_usage > service.config.cpu_threshold {
                       self.scale_up(&service.id).await;
                   } else if metrics.cpu_usage < service.config.cpu_threshold / 2 {
                       self.scale_down(&service.id).await;
                   }
               }
               
               tokio::time::sleep(Duration::from_secs(30)).await;
           }
       }
   }
```

### Success Criteria

- [x] Metrics collection works
- [x] Auto-scaling up works
- [x] Auto-scaling down works
- [x] Cooldown prevents thrashing
- [x] Respects min/max replicas

---

## Phase 12: Policy Engine

**Goal:** Enforce security policies

**Duration:** 3-4 days

### Tasks

1. **Implement policy engine**
   
   `fabricksd/src/policy/engine.rs`:
```rust
   pub struct PolicyEngine {
       policies: HashMap<String, Policy>,
   }
   
   impl PolicyEngine {
       pub fn validate_connection(
           &self,
           from: &Service,
           to_host: &str,
           to_port: u16,
       ) -> Result<()> {
           for policy in self.policies.values() {
               for deny_rule in &policy.deny {
                   if deny_rule.matches(from, to_host) {
                       return Err(Error::PolicyViolation(deny_rule.reason.clone()));
                   }
               }
           }
           Ok(())
       }
   }
```

2. **Add audit logging**

### Success Criteria

- [x] Policies enforced
- [x] Violations logged
- [x] Audit trail created

---

## Phase 13: Kubernetes Integration

**Goal:** Generate Kubernetes manifests

**Duration:** 5-7 days

### Tasks

1. **Implement K8s manifest generator**
   
   `fabricks/src/k8s/generator.rs`:
```rust
   pub fn generate_manifests(mortar: &MortarFile) -> Result<Vec<K8sManifest>> {
       let mut manifests = Vec::new();
       
       // Generate deployments
       for (name, service) in &mortar.service {
           manifests.push(generate_deployment(name, service)?);
           manifests.push(generate_service(name, service)?);
       }
       
       // Generate network policies
       for (name, network) in mortar.network.as_ref().unwrap_or(&HashMap::new()) {
           manifests.push(generate_network_policy(name, network)?);
       }
       
       // Generate PVCs
       for (name, volume) in mortar.volume.as_ref().unwrap_or(&HashMap::new()) {
           manifests.push(generate_pvc(name, volume)?);
       }
       
       Ok(manifests)
   }
```

### Success Criteria

- [x] Generates valid K8s YAML
- [x] Can deploy to cluster
- [x] Services communicate correctly

---

## Testing Strategy

### Unit Tests

- Test all core functions in isolation
- Mock external dependencies
- Run on every commit (CI)

### Integration Tests

- Test CLI commands end-to-end
- Test daemon API endpoints
- Test OCI registry operations

### E2E Tests

- Full workflow tests
- Multi-service deployments
- Kubernetes deployments

### Performance Tests

- Measure cold start times
- Measure service density
- Stress test auto-scaling

---

## Documentation

As we implement each phase:

1. Update API documentation
2. Add code comments
3. Create tutorials
4. Record demos
5. Update architecture docs

---

## Release Plan

### v0.1.0 - MVP (3 months)
- Phases 0-5 complete
- Basic CLI working
- Push/pull from registry
- Direct WASM execution

### v0.2.0 - Daemon (6 months)
- Phases 6-8 complete
- Daemon operational
- Service management
- Health monitoring

### v0.3.0 - Orchestration (9 months)
- Phases 9-12 complete
- Network segmentation
- Volume management
- Auto-scaling
- Policy enforcement

### v1.0.0 - Production Ready (12 months)
- Phase 13 complete
- Kubernetes integration
- Full documentation
- Production deployments

---
