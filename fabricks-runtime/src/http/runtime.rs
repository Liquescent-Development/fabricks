//! HTTP runtime for WASM services.
//!
//! Provides the `HttpRuntime` struct that can instantiate WASM components
//! implementing `wasi:http/incoming-handler` and route HTTP requests to them.

use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use tracing::{debug, info};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi_http::bindings::Proxy;
use wasmtime_wasi_http::bindings::http::types::Scheme as WasiScheme;
use wasmtime_wasi_http::body::HyperOutgoingBody;

use fabricks_common::Capabilities;

use super::state::{OutboundHandler, WasiHttpState};
use super::types::{HttpRequest, HttpResponse, Scheme};
use crate::error::{Result, RuntimeError};
use crate::runtime::VolumeMountConfig;

/// Configuration for the HTTP runtime.
#[derive(Debug, Clone, Default)]
pub struct HttpRuntimeConfig {
    /// Capabilities granted to the module.
    pub capabilities: Capabilities,

    /// Command-line arguments to pass to the module.
    pub args: Vec<String>,

    /// Enable fuel-based execution limits.
    pub fuel_limit: Option<u64>,

    /// Enable epoch-based interruption.
    pub epoch_interruption: bool,

    /// Volume mounts for persistent storage.
    pub volume_mounts: Vec<VolumeMountConfig>,
}

/// HTTP runtime for WASM components implementing `wasi:http/incoming-handler`.
///
/// This runtime handles incoming HTTP requests by routing them to WASM
/// components. The daemon binds ports and calls this runtime to handle
/// each request.
pub struct HttpRuntime {
    /// The Wasmtime engine (shared, thread-safe).
    engine: Arc<Engine>,

    /// The compiled component.
    component: Component,

    /// Runtime configuration including capabilities.
    config: HttpRuntimeConfig,
}

impl HttpRuntime {
    /// Create a new HTTP runtime from WASM component bytes.
    ///
    /// # Arguments
    ///
    /// * `wasm_bytes` - The compiled WASM component binary
    /// * `config` - Runtime configuration including capabilities
    ///
    /// # Errors
    ///
    /// Returns an error if the WASM component cannot be compiled.
    pub fn new(wasm_bytes: &[u8], config: HttpRuntimeConfig) -> Result<Self> {
        let engine_config = Self::build_engine_config(&config);
        let engine = Engine::new(&engine_config)?;
        let component = Component::new(&engine, wasm_bytes)?;

        info!("Compiled HTTP WASM component successfully");

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
        config: HttpRuntimeConfig,
    ) -> Result<Self> {
        let component = Component::new(&engine, wasm_bytes)?;

        Ok(Self {
            engine,
            component,
            config,
        })
    }

    /// Build the Wasmtime engine configuration.
    fn build_engine_config(config: &HttpRuntimeConfig) -> Config {
        let mut engine_config = Config::new();

        // Enable component model (required for WASI Preview 2)
        engine_config.wasm_component_model(true);

        // Enable async for HTTP handling
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

        // Enable epoch interruption if configured
        if config.epoch_interruption {
            engine_config.epoch_interruption(true);
        }

        engine_config
    }

    /// Handle an incoming HTTP request.
    ///
    /// This instantiates the WASM component and calls its `wasi:http/incoming-handler.handle`
    /// function with the request.
    ///
    /// # Arguments
    ///
    /// * `request` - The incoming HTTP request
    /// * `outbound_handler` - Handler for validating outbound requests from the WASM module
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be handled.
    pub async fn handle_request(
        &self,
        request: HttpRequest,
        outbound_handler: Arc<dyn OutboundHandler>,
    ) -> Result<HttpResponse> {
        use wasmtime_wasi_http::WasiHttpView;

        let mut store = self.create_store(outbound_handler)?;
        let mut linker: Linker<WasiHttpState> = Linker::new(&self.engine);

        // Add WASI and WASI-HTTP to the linker
        wasmtime_wasi::add_to_linker_async(&mut linker)?;
        wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;

        // Pre-instantiate the proxy component
        let proxy = Proxy::instantiate_async(&mut store, &self.component, &linker)
            .await
            .map_err(|e| RuntimeError::InstantiationError {
                reason: e.to_string(),
            })?;

        debug!(method = %request.method, uri = %request.uri, "Handling HTTP request");

        // Convert our request to WASI HTTP types (returns a Resource handle)
        let wasi_request = Self::create_wasi_request(&mut store, &request)?;

        // Create a response outparam channel
        // The sender takes Result<Response, ErrorCode>
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let response_outparam = store
            .data_mut()
            .new_response_outparam(response_tx)
            .map_err(|e| RuntimeError::ExecutionError {
                reason: format!("Failed to create response outparam: {e}"),
            })?;

        // Call the incoming handler
        proxy
            .wasi_http_incoming_handler()
            .call_handle(&mut store, wasi_request, response_outparam)
            .await
            .map_err(|e| RuntimeError::ExecutionError {
                reason: format!("Handler call failed: {e}"),
            })?;

        // Get the response from the channel
        let response = response_rx
            .await
            .map_err(|_| RuntimeError::ExecutionError {
                reason: "Response channel closed".to_string(),
            })?;

        self.convert_response(response).await
    }

    /// Create a WASI HTTP request from our request type.
    ///
    /// Returns a Resource handle for the incoming request.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be converted.
    fn create_wasi_request(
        store: &mut Store<WasiHttpState>,
        request: &HttpRequest,
    ) -> Result<wasmtime::component::Resource<wasmtime_wasi_http::types::HostIncomingRequest>> {
        use http_body_util::BodyExt;
        use wasmtime_wasi_http::WasiHttpView;

        // Build an http::Request with Full body
        let mut builder = http::Request::builder()
            .method(request.method.as_str())
            .uri(&request.uri);

        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }

        // Create the body - need to map Infallible error to hyper::Error
        // Full<Bytes> has Error = Infallible, but new_incoming_request needs Error = hyper::Error
        let body = http_body_util::Full::new(request.body.clone())
            .map_err(|never: std::convert::Infallible| match never {});

        // BoxBody allows us to erase the concrete error type
        let boxed_body: http_body_util::combinators::BoxBody<Bytes, hyper::Error> =
            http_body_util::BodyExt::boxed(body);

        let http_request = builder
            .body(boxed_body)
            .map_err(|e| RuntimeError::ExecutionError {
                reason: format!("Failed to build request: {e}"),
            })?;

        // Convert scheme
        let scheme = match request.scheme {
            Scheme::Http => WasiScheme::Http,
            Scheme::Https => WasiScheme::Https,
        };

        // Create the WASI incoming request (returns a single Resource)
        let incoming_request = store
            .data_mut()
            .new_incoming_request(scheme, http_request)
            .map_err(|e| RuntimeError::ExecutionError {
                reason: format!("Failed to create incoming request: {e}"),
            })?;

        Ok(incoming_request)
    }

    /// Convert a WASI HTTP response to our response type.
    async fn convert_response(
        &self,
        response: std::result::Result<
            http::Response<HyperOutgoingBody>,
            wasmtime_wasi_http::bindings::http::types::ErrorCode,
        >,
    ) -> Result<HttpResponse> {
        use http_body_util::BodyExt;

        let response = response.map_err(|e| RuntimeError::ExecutionError {
            reason: format!("WASI HTTP error: {e:?}"),
        })?;

        let status = response.status().as_u16();
        let mut headers = std::collections::HashMap::new();

        for (name, value) in response.headers() {
            if let Ok(v) = value.to_str() {
                headers.insert(name.to_string(), v.to_string());
            }
        }

        // Collect the body
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|e| RuntimeError::ExecutionError {
                reason: format!("Failed to read response body: {e}"),
            })?
            .to_bytes();

        Ok(HttpResponse {
            status,
            headers,
            body: Bytes::from(body.to_vec()),
        })
    }

    /// Create a new store with WASI HTTP state.
    fn create_store(
        &self,
        outbound_handler: Arc<dyn OutboundHandler>,
    ) -> Result<Store<WasiHttpState>> {
        let wasi_ctx = self.build_wasi_context()?;
        let state = WasiHttpState::new(wasi_ctx, outbound_handler);
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

    /// Build WASI context with capability-based restrictions.
    fn build_wasi_context(&self) -> Result<wasmtime_wasi::WasiCtx> {
        let mut builder = WasiCtxBuilder::new();

        // Always inherit stdio for logging
        builder.inherit_stdio();

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

    /// Configure volume mounts for persistent storage.
    fn configure_volume_mounts(&self, builder: &mut WasiCtxBuilder) -> Result<()> {
        use wasmtime_wasi::{DirPerms, FilePerms};

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
    fn test_http_runtime_creation() {
        let config = HttpRuntimeConfig::default();
        let runtime = HttpRuntime::new(&minimal_component(), config);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_http_runtime_with_capabilities() {
        let config = HttpRuntimeConfig {
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

        let runtime =
            HttpRuntime::new(&minimal_component(), config).expect("Failed to create runtime");

        assert!(runtime.is_network_allowed("api.example.com", 443));
        assert!(!runtime.is_network_allowed("evil.com", 443));
        assert!(runtime.is_listen_allowed(8080));
        assert!(!runtime.is_listen_allowed(9090));
    }

    #[test]
    fn test_invalid_wasm() {
        let config = HttpRuntimeConfig::default();
        let result = HttpRuntime::new(b"not valid wasm", config);
        assert!(result.is_err());
    }
}
