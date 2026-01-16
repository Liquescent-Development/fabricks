//! Wasmtime-based WASM runtime with capability enforcement.
//!
//! This module provides the core runtime for executing WASM components with
//! Fabricks' deny-by-default security model using WASI Preview 2.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fabricks_common::Capabilities;
use tracing::{debug, info};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};

use crate::error::{Result, RuntimeError};

/// Volume mount configuration for the runtime.
///
/// Maps a host directory to a guest path for WASI preopened directories.
#[derive(Debug, Clone)]
pub struct VolumeMountConfig {
    /// Host path to the volume directory.
    pub host_path: PathBuf,
    /// Guest path where the volume will be mounted (e.g., "/data").
    pub guest_path: String,
    /// Whether the volume is read-only.
    pub read_only: bool,
}

impl VolumeMountConfig {
    /// Creates a new volume mount config.
    #[must_use]
    pub fn new(host_path: PathBuf, guest_path: String) -> Self {
        Self {
            host_path,
            guest_path,
            read_only: false,
        }
    }

    /// Creates a read-only volume mount config.
    #[must_use]
    pub fn read_only(host_path: PathBuf, guest_path: String) -> Self {
        Self {
            host_path,
            guest_path,
            read_only: true,
        }
    }
}

/// Configuration for creating a runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Capabilities granted to the module.
    pub capabilities: Capabilities,

    /// Command-line arguments to pass to the module.
    pub args: Vec<String>,

    /// Working directory for the module.
    pub working_dir: Option<String>,

    /// Whether to inherit stdio from the host.
    pub inherit_stdio: bool,

    /// Enable fuel-based execution limits.
    pub fuel_limit: Option<u64>,

    /// Enable epoch-based interruption.
    pub epoch_interruption: bool,

    /// Volume mounts for persistent storage.
    pub volume_mounts: Vec<VolumeMountConfig>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            capabilities: Capabilities::default(),
            args: Vec::new(),
            working_dir: None,
            inherit_stdio: true,
            fuel_limit: None,
            epoch_interruption: false,
            volume_mounts: Vec::new(),
        }
    }
}

/// Host state for WASI Preview 2.
pub struct WasiState {
    /// WASI context with configured capabilities.
    ctx: WasiCtx,
    /// Resource table for WASI resources.
    table: ResourceTable,
}

impl WasiState {
    /// Create a new WASI state with the given context.
    fn new(ctx: WasiCtx) -> Self {
        Self {
            ctx,
            table: ResourceTable::new(),
        }
    }
}

impl WasiView for WasiState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

/// WASM component runtime with capability enforcement.
///
/// Wraps Wasmtime with Fabricks' security model, enforcing capabilities
/// at the WASI layer using WASI Preview 2 and the Component Model.
pub struct Runtime {
    /// The Wasmtime engine (shared, thread-safe).
    engine: Arc<Engine>,

    /// The compiled component.
    component: Component,

    /// Runtime configuration including capabilities.
    config: RuntimeConfig,
}

impl Runtime {
    /// Create a new runtime from WASM component bytes.
    ///
    /// # Arguments
    ///
    /// * `wasm_bytes` - The compiled WASM component binary
    /// * `config` - Runtime configuration including capabilities
    ///
    /// # Errors
    ///
    /// Returns an error if the WASM component cannot be compiled.
    pub fn new(wasm_bytes: &[u8], config: RuntimeConfig) -> Result<Self> {
        let engine_config = Self::build_engine_config(&config);
        let engine = Engine::new(&engine_config)?;
        let component = Component::new(&engine, wasm_bytes)?;

        info!("Compiled WASM component successfully");

        Ok(Self {
            engine: Arc::new(engine),
            component,
            config,
        })
    }

    /// Create a runtime with a shared engine (for pooling).
    ///
    /// # Errors
    ///
    /// Returns an error if the WASM component cannot be compiled.
    pub fn with_engine(
        engine: Arc<Engine>,
        wasm_bytes: &[u8],
        config: RuntimeConfig,
    ) -> Result<Self> {
        let component = Component::new(&engine, wasm_bytes)?;

        Ok(Self {
            engine,
            component,
            config,
        })
    }

    /// Build the Wasmtime engine configuration.
    fn build_engine_config(config: &RuntimeConfig) -> Config {
        let mut engine_config = Config::new();

        // Enable component model (required for WASI Preview 2)
        engine_config.wasm_component_model(true);

        // Enable WASM features based on capabilities
        if let Some(ref wasm) = config.capabilities.wasm {
            if wasm.simd.unwrap_or(false) {
                engine_config.wasm_simd(true);
            }
            if wasm.threads.unwrap_or(false) {
                engine_config.wasm_threads(true);
            }
            if wasm.bulk_memory.unwrap_or(false) {
                engine_config.wasm_bulk_memory(true);
            }
        }

        // Enable fuel if configured
        if config.fuel_limit.is_some() {
            engine_config.consume_fuel(true);
        }

        // Enable epoch interruption if configured
        if config.epoch_interruption {
            engine_config.epoch_interruption(true);
        }

        engine_config
    }

    /// Run the component as a WASI command (calls `wasi:cli/run#run`).
    ///
    /// # Errors
    ///
    /// Returns an error if execution fails or a capability is violated.
    pub fn run(&self) -> Result<()> {
        let mut store = self.create_store()?;
        let mut linker = self.create_linker();

        // Add WASI to the linker
        wasmtime_wasi::add_to_linker_sync(&mut linker)?;

        // Instantiate and run using the Command interface
        let command = wasmtime_wasi::bindings::sync::Command::instantiate(
            &mut store,
            &self.component,
            &linker,
        )
        .map_err(|e| RuntimeError::InstantiationError {
            reason: e.to_string(),
        })?;

        info!("Executing WASI command");
        command
            .wasi_cli_run()
            .call_run(&mut store)
            .map_err(|e| RuntimeError::ExecutionError {
                reason: e.to_string(),
            })?
            .map_err(|()| RuntimeError::ExecutionError {
                reason: "command returned error".to_string(),
            })?;

        Ok(())
    }

    /// Create a new store with WASI state configured per capabilities.
    fn create_store(&self) -> Result<Store<WasiState>> {
        let wasi_ctx = self.build_wasi_context()?;
        let state = WasiState::new(wasi_ctx);
        let mut store = Store::new(&self.engine, state);

        // Set fuel limit if configured
        if let Some(fuel) = self.config.fuel_limit {
            store
                .set_fuel(fuel)
                .map_err(|e| RuntimeError::ExecutionError {
                    reason: format!("failed to set fuel: {e}"),
                })?;
        }

        Ok(store)
    }

    /// Create a linker for the component.
    fn create_linker(&self) -> Linker<WasiState> {
        Linker::new(&self.engine)
    }

    /// Build WASI context with capability-based restrictions.
    fn build_wasi_context(&self) -> Result<WasiCtx> {
        let mut builder = WasiCtxBuilder::new();

        // Configure stdio
        if self.config.inherit_stdio {
            builder.inherit_stdio();
        }

        // Configure arguments
        if !self.config.args.is_empty() {
            builder.args(&self.config.args);
        }

        // Configure environment variables (filtered by capabilities)
        self.configure_env(&mut builder);

        // Configure filesystem access (based on capabilities)
        self.configure_filesystem(&mut builder)?;

        // Configure volume mounts
        self.configure_volume_mounts(&mut builder)?;

        Ok(builder.build())
    }

    /// Configure allowed environment variables.
    fn configure_env(&self, builder: &mut WasiCtxBuilder) {
        let Some(ref allowed_vars) = self.config.capabilities.env else {
            debug!("No environment variables allowed");
            return;
        };

        for var_name in allowed_vars {
            if let Ok(value) = std::env::var(var_name) {
                debug!("Allowing env var: {var_name}");
                builder.env(var_name, &value);
            }
        }
    }

    /// Configure filesystem access based on capabilities.
    fn configure_filesystem(&self, builder: &mut WasiCtxBuilder) -> Result<()> {
        let Some(ref fs_caps) = self.config.capabilities.filesystem else {
            debug!("No filesystem access allowed");
            return Ok(());
        };

        // Read-only paths
        if let Some(ref read_paths) = fs_caps.read {
            for path_str in read_paths {
                let path = Path::new(path_str);
                if path.exists() {
                    debug!("Preopening read-only: {path_str}");
                    builder
                        .preopened_dir(path, path_str, DirPerms::READ, FilePerms::READ)
                        .map_err(|e| RuntimeError::FilesystemDenied {
                            path: path.to_path_buf(),
                            operation: format!("preopen read: {e}"),
                        })?;
                }
            }
        }

        // Write-only paths
        if let Some(ref write_paths) = fs_caps.write {
            for path_str in write_paths {
                let path = Path::new(path_str);
                if path.exists() {
                    debug!("Preopening write-only: {path_str}");
                    builder
                        .preopened_dir(path, path_str, DirPerms::empty(), FilePerms::WRITE)
                        .map_err(|e| RuntimeError::FilesystemDenied {
                            path: path.to_path_buf(),
                            operation: format!("preopen write: {e}"),
                        })?;
                }
            }
        }

        // Read-write paths
        if let Some(ref rw_paths) = fs_caps.read_write {
            for path_str in rw_paths {
                let path = Path::new(path_str);
                if path.exists() {
                    debug!("Preopening read-write: {path_str}");
                    builder
                        .preopened_dir(
                            path,
                            path_str,
                            DirPerms::all(),
                            FilePerms::READ | FilePerms::WRITE,
                        )
                        .map_err(|e| RuntimeError::FilesystemDenied {
                            path: path.to_path_buf(),
                            operation: format!("preopen read-write: {e}"),
                        })?;
                }
            }
        }

        Ok(())
    }

    /// Configure volume mounts for persistent storage.
    fn configure_volume_mounts(&self, builder: &mut WasiCtxBuilder) -> Result<()> {
        if self.config.volume_mounts.is_empty() {
            return Ok(());
        }

        for mount in &self.config.volume_mounts {
            if !mount.host_path.exists() {
                debug!(
                    host_path = %mount.host_path.display(),
                    guest_path = %mount.guest_path,
                    "Volume mount host path does not exist, skipping"
                );
                continue;
            }

            let (dir_perms, file_perms) = if mount.read_only {
                (DirPerms::READ, FilePerms::READ)
            } else {
                (DirPerms::all(), FilePerms::READ | FilePerms::WRITE)
            };

            debug!(
                host_path = %mount.host_path.display(),
                guest_path = %mount.guest_path,
                read_only = mount.read_only,
                "Mounting volume"
            );

            builder
                .preopened_dir(&mount.host_path, &mount.guest_path, dir_perms, file_perms)
                .map_err(|e| RuntimeError::FilesystemDenied {
                    path: mount.host_path.clone(),
                    operation: format!("mount volume at {}: {e}", mount.guest_path),
                })?;
        }

        Ok(())
    }

    /// Get the capabilities this runtime was configured with.
    #[must_use]
    pub const fn capabilities(&self) -> &Capabilities {
        &self.config.capabilities
    }

    /// Get the shared engine.
    #[must_use]
    pub fn engine(&self) -> Arc<Engine> {
        Arc::clone(&self.engine)
    }

    /// Check if a network connection would be allowed.
    ///
    /// Note: This is a policy check. Actual network enforcement happens
    /// at the daemon level since WASM modules don't directly access sockets.
    #[must_use]
    pub fn is_network_allowed(&self, host: &str, port: u16) -> bool {
        let target = format!("{host}:{port}");
        self.config.capabilities.can_connect(&target)
    }

    /// Check if listening on a port would be allowed.
    ///
    /// Note: This is a policy check. The daemon binds ports, not the WASM module.
    #[must_use]
    pub fn is_listen_allowed(&self, port: u16) -> bool {
        self.config.capabilities.can_listen(port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabricks_common::models::capability::{
        FilesystemCapabilities, NetworkCapabilities, WasmCapabilities,
    };

    /// Minimal valid WASM component (empty component).
    /// This is the simplest valid component - just the component magic and header.
    fn minimal_component() -> Vec<u8> {
        // Component magic: \0asm + version 0x0d (component)
        // Followed by empty component sections
        vec![
            0x00, 0x61, 0x73, 0x6d, // magic: \0asm
            0x0d, 0x00, 0x01, 0x00, // version: component model (0x0d = 13)
        ]
    }

    #[test]
    fn test_runtime_creation() {
        let config = RuntimeConfig::default();
        let runtime = Runtime::new(&minimal_component(), config);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_runtime_with_capabilities() {
        let config = RuntimeConfig {
            capabilities: Capabilities {
                env: Some(vec!["HOME".to_string()]),
                network: Some(NetworkCapabilities {
                    listen: Some(vec![8080]),
                    connect: Some(vec!["api.example.com:443".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let runtime = Runtime::new(&minimal_component(), config).expect("Failed to create runtime");

        assert!(runtime.is_network_allowed("api.example.com", 443));
        assert!(!runtime.is_network_allowed("evil.com", 443));
        assert!(runtime.is_listen_allowed(8080));
        assert!(!runtime.is_listen_allowed(9090));
    }

    #[test]
    fn test_wasm_feature_flags() {
        let config = RuntimeConfig {
            capabilities: Capabilities {
                wasm: Some(WasmCapabilities {
                    simd: Some(true),
                    threads: Some(true),
                    bulk_memory: Some(true),
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let runtime = Runtime::new(&minimal_component(), config);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_filesystem_capabilities() {
        let config = RuntimeConfig {
            capabilities: Capabilities {
                filesystem: Some(FilesystemCapabilities {
                    read: Some(vec!["/tmp".to_string()]),
                    write: Some(vec!["/var/log".to_string()]),
                    read_write: Some(vec!["/data".to_string()]),
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let runtime = Runtime::new(&minimal_component(), config).expect("Failed to create runtime");
        let caps = runtime.capabilities();

        assert!(caps.can_read("/tmp/file.txt"));
        assert!(!caps.can_write("/tmp/file.txt"));
        assert!(caps.can_write("/var/log/app.log"));
        assert!(!caps.can_read("/var/log/app.log"));
        assert!(caps.can_read("/data/db.sqlite"));
        assert!(caps.can_write("/data/db.sqlite"));
    }

    #[test]
    fn test_invalid_wasm() {
        let config = RuntimeConfig::default();
        let result = Runtime::new(b"not valid wasm", config);
        assert!(result.is_err());
    }

    #[test]
    fn test_shared_engine() {
        let mut engine_config = Config::new();
        engine_config.wasm_component_model(true);
        let engine = Arc::new(Engine::new(&engine_config).expect("Failed to create engine"));

        let config1 = RuntimeConfig::default();
        let config2 = RuntimeConfig::default();

        let runtime1 = Runtime::with_engine(Arc::clone(&engine), &minimal_component(), config1)
            .expect("Failed to create runtime1");
        let runtime2 = Runtime::with_engine(Arc::clone(&engine), &minimal_component(), config2)
            .expect("Failed to create runtime2");

        // Both share the same engine
        assert!(Arc::ptr_eq(&runtime1.engine(), &runtime2.engine()));
    }
}
