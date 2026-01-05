//! TCP runtime for WASM services.
//!
//! Provides the `TcpRuntime` struct that can instantiate WASM components
//! and connect TCP streams to them using the "inetd model" - stdin/stdout
//! are connected to the TCP socket.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tracing::{debug, error, info};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::pipe::{AsyncReadStream, AsyncWriteStream};
use wasmtime_wasi::{AsyncStdinStream, AsyncStdoutStream, WasiCtx, WasiCtxBuilder, WasiView};

use fabricks_common::Capabilities;

use crate::error::{Result, RuntimeError};

/// Configuration for the TCP runtime.
#[derive(Debug, Clone, Default)]
pub struct TcpRuntimeConfig {
    /// Capabilities granted to the module.
    pub capabilities: Capabilities,

    /// Command-line arguments to pass to the module.
    pub args: Vec<String>,

    /// Enable fuel-based execution limits.
    pub fuel_limit: Option<u64>,

    /// Connection timeout.
    pub connection_timeout: Option<Duration>,
}

/// State for TCP WASM execution.
struct TcpWasiState {
    /// WASI context.
    wasi_ctx: WasiCtx,

    /// Resource table for WASI.
    table: wasmtime::component::ResourceTable,
}

impl WasiView for TcpWasiState {
    fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }
}

/// TCP runtime for WASM components.
///
/// This runtime handles incoming TCP connections by connecting them to WASM
/// components via stdin/stdout (inetd model). The daemon binds ports and calls
/// this runtime to handle each connection.
pub struct TcpRuntime {
    /// The Wasmtime engine (shared, thread-safe).
    engine: Arc<Engine>,

    /// The compiled component.
    component: Component,

    /// Runtime configuration including capabilities.
    config: TcpRuntimeConfig,
}

impl TcpRuntime {
    /// Create a new TCP runtime from WASM component bytes.
    ///
    /// # Arguments
    ///
    /// * `wasm_bytes` - The compiled WASM component binary
    /// * `config` - Runtime configuration including capabilities
    ///
    /// # Errors
    ///
    /// Returns an error if the WASM component cannot be compiled.
    pub fn new(wasm_bytes: &[u8], config: TcpRuntimeConfig) -> Result<Self> {
        let engine_config = Self::build_engine_config(&config);
        let engine = Engine::new(&engine_config)?;
        let component = Component::new(&engine, wasm_bytes)?;

        info!("Compiled TCP WASM component successfully");

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
        config: TcpRuntimeConfig,
    ) -> Result<Self> {
        let component = Component::new(&engine, wasm_bytes)?;

        Ok(Self {
            engine,
            component,
            config,
        })
    }

    /// Build the Wasmtime engine configuration.
    fn build_engine_config(config: &TcpRuntimeConfig) -> Config {
        let mut engine_config = Config::new();

        // Enable component model (required for WASI Preview 2)
        engine_config.wasm_component_model(true);

        // Enable async for TCP handling
        engine_config.async_support(true);

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

        engine_config
    }

    /// Handle an incoming TCP connection.
    ///
    /// This instantiates the WASM component and connects the TCP stream to
    /// stdin/stdout (inetd model). The component runs until completion or timeout.
    ///
    /// # Arguments
    ///
    /// * `stream` - The incoming TCP stream
    /// * `peer_addr` - The peer's socket address
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be handled.
    pub async fn handle_connection(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<()> {
        debug!(%peer_addr, "Handling TCP connection");

        // Split the stream into read and write halves
        let (read_half, write_half) = stream.into_split();

        // Create async streams for WASI stdin/stdout
        // AsyncStdinStream wraps an AsyncReadStream
        // AsyncStdoutStream wraps an AsyncWriteStream
        let stdin_stream = AsyncReadStream::new(read_half);
        let stdout_stream = AsyncWriteStream::new(1024, write_half);

        let stdin = AsyncStdinStream::new(stdin_stream);
        let stdout = AsyncStdoutStream::new(stdout_stream);

        // Create WASI context with streams connected
        let wasi_ctx = self.build_wasi_context_with_streams(stdin, stdout)?;
        let state = TcpWasiState {
            wasi_ctx,
            table: wasmtime::component::ResourceTable::new(),
        };

        let mut store = Store::new(&self.engine, state);

        // Set fuel limit if configured
        if let Some(fuel) = self.config.fuel_limit {
            store
                .set_fuel(fuel)
                .map_err(|e| RuntimeError::ExecutionError {
                    reason: format!("failed to set fuel: {e}"),
                })?;
        }

        // Create linker with WASI
        let mut linker: Linker<TcpWasiState> = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        // Use the Command bindings to instantiate and run
        use wasmtime_wasi::bindings::Command;

        let command = Command::instantiate_async(&mut store, &self.component, &linker)
            .await
            .map_err(|e| RuntimeError::InstantiationError {
                reason: e.to_string(),
            })?;

        // Call wasi:cli/run.run
        let run_result = command.wasi_cli_run().call_run(&mut store).await;

        match run_result {
            Ok(Ok(())) => {
                debug!(%peer_addr, "TCP handler completed successfully");
                Ok(())
            }
            Ok(Err(())) => {
                debug!(%peer_addr, "TCP handler exited with error status");
                // Non-zero exit is not necessarily an error for our purposes
                Ok(())
            }
            Err(e) => {
                error!(%peer_addr, error = %e, "TCP handler failed");
                Err(RuntimeError::ExecutionError {
                    reason: format!("run function failed: {e}"),
                })
            }
        }
    }

    /// Build WASI context with TCP streams connected to stdin/stdout.
    fn build_wasi_context_with_streams(
        &self,
        stdin: AsyncStdinStream,
        stdout: AsyncStdoutStream,
    ) -> Result<WasiCtx> {
        let mut builder = WasiCtxBuilder::new();

        // Connect TCP to stdin/stdout
        builder.stdin(stdin);
        builder.stdout(stdout);

        // Keep stderr for logging
        builder.inherit_stderr();

        // Configure arguments
        if !self.config.args.is_empty() {
            builder.args(&self.config.args);
        }

        // Configure environment variables (filtered by capabilities)
        self.configure_env(&mut builder);

        // Configure filesystem access (based on capabilities)
        self.configure_filesystem(&mut builder)?;

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
        use wasmtime_wasi::{DirPerms, FilePerms};

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

    /// Check if a network connection would be allowed by capabilities.
    #[must_use]
    pub fn is_network_allowed(&self, host: &str, port: u16) -> bool {
        let target = format!("{host}:{port}");
        self.config.capabilities.can_connect(&target)
    }

    /// Check if listening on a port would be allowed by capabilities.
    #[must_use]
    pub fn is_listen_allowed(&self, port: u16) -> bool {
        self.config.capabilities.can_listen(port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabricks_common::models::capability::NetworkCapabilities;

    /// Minimal valid WASM component (empty component).
    fn minimal_component() -> Vec<u8> {
        vec![
            0x00, 0x61, 0x73, 0x6d, // magic: \0asm
            0x0d, 0x00, 0x01, 0x00, // version: component model (0x0d = 13)
        ]
    }

    #[test]
    fn test_tcp_runtime_creation() {
        let config = TcpRuntimeConfig::default();
        let runtime = TcpRuntime::new(&minimal_component(), config);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_tcp_runtime_with_capabilities() {
        let config = TcpRuntimeConfig {
            capabilities: Capabilities {
                env: Some(vec!["HOME".to_string()]),
                network: Some(NetworkCapabilities {
                    listen: Some(vec![9000]),
                    connect: Some(vec!["database:5432".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let runtime =
            TcpRuntime::new(&minimal_component(), config).expect("Failed to create runtime");

        assert!(runtime.is_network_allowed("database", 5432));
        assert!(!runtime.is_network_allowed("evil.com", 443));
        assert!(runtime.is_listen_allowed(9000));
        assert!(!runtime.is_listen_allowed(8080));
    }

    #[test]
    fn test_invalid_wasm() {
        let config = TcpRuntimeConfig::default();
        let result = TcpRuntime::new(b"not valid wasm", config);
        assert!(result.is_err());
    }
}
