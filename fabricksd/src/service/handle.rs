//! Service handle for managing service instances.
//!
//! A `ServiceHandle` manages the lifecycle of individual WASM instances for a service,
//! including spawning, scaling, and stopping instances.
//!
//! For HTTP services, the handle also manages port bindings via the proxy server
//! and maintains an `HttpRuntime` for handling incoming requests.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use fabricks_runtime::http::{HttpRuntime, HttpRuntimeConfig, OutboundHandler};
use fabricks_runtime::output::{LogCaptureSink, LogStream, LogWriter};
use fabricks_runtime::tcp::{TcpRuntime, TcpRuntimeConfig};
use fabricks_runtime::{HttpRequest, HttpResponse, Runtime, RuntimeConfig, RuntimePool};

use crate::error::{DaemonError, Result};
use crate::proxy::SharedProxyServer;

use fabricks_common::Capabilities;

use super::logs::{LogEntry, ServiceLogBuffer};
use super::types::{Instance, InstanceState, ServiceConfig, ServiceDetail, ServiceState, State};

/// Outbound handler that validates connections based on service capabilities.
///
/// This checks if the service has the `connect` capability for the target host:port.
pub struct CapabilityOutboundHandler {
    /// Service capabilities.
    capabilities: Capabilities,
}

impl CapabilityOutboundHandler {
    /// Creates a new capability-based outbound handler.
    #[must_use]
    pub fn new(capabilities: Capabilities) -> Self {
        Self { capabilities }
    }
}

impl OutboundHandler for CapabilityOutboundHandler {
    fn is_allowed(&self, host: &str, port: u16) -> fabricks_runtime::error::Result<bool> {
        let target = format!("{host}:{port}");
        Ok(self.capabilities.can_connect(&target))
    }
}

/// Handle for managing a running service and its instances.
pub struct ServiceHandle {
    /// Service state (persisted).
    state: RwLock<ServiceState>,

    /// Running instances.
    instances: Mutex<Vec<InstanceHandle>>,

    /// Runtime pool for creating WASM runtimes.
    runtime_pool: Arc<RuntimePool>,

    /// Cached WASM bytes.
    wasm_bytes: Arc<Vec<u8>>,

    /// Proxy server for port bindings (HTTP/TCP services).
    proxy_server: Option<SharedProxyServer>,

    /// Ports bound for this service.
    bound_ports: Mutex<Vec<u16>>,

    /// HTTP runtime for handling incoming requests (HTTP services only).
    http_runtime: RwLock<Option<Arc<HttpRuntime>>>,

    /// TCP runtime for handling incoming connections (TCP services only).
    tcp_runtime: RwLock<Option<Arc<TcpRuntime>>>,

    /// Outbound handler for validating outbound HTTP requests.
    outbound_handler: RwLock<Option<Arc<dyn OutboundHandler>>>,

    /// Per-service log buffer for capturing stdout/stderr.
    log_buffer: Arc<ServiceLogBuffer>,
}

/// Handle for a running instance.
struct InstanceHandle {
    /// Instance metadata.
    instance: Instance,

    /// Task handle for the running WASM.
    task: Option<JoinHandle<Result<()>>>,
}

impl ServiceHandle {
    /// Creates a new service handle.
    ///
    /// # Arguments
    ///
    /// * `config` - Service configuration
    /// * `runtime_pool` - Pool for creating WASM runtimes
    /// * `wasm_bytes` - The WASM module bytes
    /// * `proxy_server` - Optional proxy server for HTTP/TCP port bindings
    ///
    /// # Returns
    ///
    /// A new service handle in the Creating state.
    pub fn new(
        config: ServiceConfig,
        runtime_pool: Arc<RuntimePool>,
        wasm_bytes: Vec<u8>,
        proxy_server: Option<SharedProxyServer>,
    ) -> Self {
        let state = ServiceState::new(config);
        Self {
            state: RwLock::new(state),
            instances: Mutex::new(Vec::new()),
            runtime_pool,
            wasm_bytes: Arc::new(wasm_bytes),
            proxy_server,
            bound_ports: Mutex::new(Vec::new()),
            http_runtime: RwLock::new(None),
            tcp_runtime: RwLock::new(None),
            outbound_handler: RwLock::new(None),
            log_buffer: Arc::new(ServiceLogBuffer::default()),
        }
    }

    /// Creates a service handle from existing state (for recovery).
    pub fn from_state(
        state: ServiceState,
        runtime_pool: Arc<RuntimePool>,
        wasm_bytes: Vec<u8>,
        proxy_server: Option<SharedProxyServer>,
    ) -> Self {
        Self {
            state: RwLock::new(state),
            instances: Mutex::new(Vec::new()),
            runtime_pool,
            wasm_bytes: Arc::new(wasm_bytes),
            proxy_server,
            bound_ports: Mutex::new(Vec::new()),
            http_runtime: RwLock::new(None),
            tcp_runtime: RwLock::new(None),
            outbound_handler: RwLock::new(None),
            log_buffer: Arc::new(ServiceLogBuffer::default()),
        }
    }

    /// Returns the service ID.
    pub async fn id(&self) -> String {
        self.state.read().await.id.clone()
    }

    /// Returns the service name.
    pub async fn name(&self) -> String {
        self.state.read().await.name.clone()
    }

    /// Returns the current service state.
    pub async fn current_state(&self) -> State {
        self.state.read().await.state
    }

    /// Returns a copy of the service state.
    pub async fn get_state(&self) -> ServiceState {
        self.state.read().await.clone()
    }

    /// Returns detailed service information.
    pub async fn get_detail(&self) -> ServiceDetail {
        let state = self.state.read().await;
        let instances_guard = self.instances.lock().await;

        let instances: Vec<Instance> = instances_guard.iter().map(|h| h.instance.clone()).collect();

        ServiceDetail {
            id: state.id.clone(),
            name: state.name.clone(),
            version: state.version.clone(),
            state: state.state,
            replicas: state.replicas.clone(),
            config: state.config.clone(),
            created_at: state.created_at,
            updated_at: state.updated_at,
            last_error: state.last_error.clone(),
            instances,
            mortar_project: state.mortar_project.clone(),
            ports: Vec::new(),    // Populated by API handler from proxy server
            networks: Vec::new(), // Populated by API handler from network manager
        }
    }

    /// Sets the outbound handler for HTTP services.
    ///
    /// This handler is used to validate outbound requests from the WASM module.
    pub async fn set_outbound_handler(&self, handler: Arc<dyn OutboundHandler>) {
        let mut guard = self.outbound_handler.write().await;
        *guard = Some(handler);
    }

    /// Handles an incoming HTTP request (HTTP services only).
    ///
    /// This routes the request to the WASM component's `wasi:http/incoming-handler`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The service is not an HTTP service
    /// - The HTTP runtime is not initialized
    /// - Request handling fails
    pub async fn handle_http_request(&self, request: HttpRequest) -> Result<HttpResponse> {
        let runtime_guard = self.http_runtime.read().await;
        let runtime = runtime_guard.as_ref().ok_or_else(|| {
            DaemonError::ServiceError {
                id: String::new(), // ID will be filled in by caller
                reason: "HTTP runtime not initialized".to_string(),
            }
        })?;

        let outbound_guard = self.outbound_handler.read().await;
        let outbound_handler =
            outbound_guard
                .as_ref()
                .ok_or_else(|| DaemonError::ServiceError {
                    id: String::new(),
                    reason: "Outbound handler not configured".to_string(),
                })?;

        runtime
            .handle_request(request, Arc::clone(outbound_handler))
            .await
            .map_err(|e| DaemonError::ServiceError {
                id: String::new(),
                reason: e.to_string(),
            })
    }

    /// Handles an incoming TCP connection (TCP services only).
    ///
    /// This routes the connection to the WASM component using the inetd model -
    /// stdin/stdout are connected to the TCP stream.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The service is not a TCP service
    /// - The TCP runtime is not initialized
    /// - Connection handling fails
    pub async fn handle_tcp_connection(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<()> {
        let runtime_guard = self.tcp_runtime.read().await;
        let runtime = runtime_guard.as_ref().ok_or_else(|| {
            DaemonError::ServiceError {
                id: String::new(), // ID will be filled in by caller
                reason: "TCP runtime not initialized".to_string(),
            }
        })?;

        runtime
            .handle_connection(stream, peer_addr)
            .await
            .map_err(|e| DaemonError::ServiceError {
                id: String::new(),
                reason: e.to_string(),
            })
    }

    /// Creates the HTTP runtime for this service.
    ///
    /// Called during `start()` for HTTP services. Also sets up the outbound handler
    /// for validating outbound connections.
    async fn create_http_runtime(&self) -> Result<()> {
        use fabricks_runtime::VolumeMountConfig;

        let state = self.state.read().await;
        let capabilities = state.config.capabilities.clone();
        let service_volumes = state.config.volumes.clone();
        drop(state);

        // Convert daemon VolumeMount to runtime VolumeMountConfig
        let volume_mounts: Vec<VolumeMountConfig> = service_volumes
            .iter()
            .map(|vm| VolumeMountConfig {
                host_path: vm.host_path.clone(),
                guest_path: vm.guest_path.clone(),
                read_only: vm.read_only,
            })
            .collect();

        let config = HttpRuntimeConfig {
            capabilities: capabilities.clone(),
            args: Vec::new(),
            fuel_limit: None,
            epoch_interruption: false,
            volume_mounts,
            resource_limits: None,
        };

        let runtime =
            HttpRuntime::new(&self.wasm_bytes, config).map_err(|e| DaemonError::ServiceError {
                id: String::new(),
                reason: format!("Failed to create HTTP runtime: {e}"),
            })?;

        let mut runtime_guard = self.http_runtime.write().await;
        *runtime_guard = Some(Arc::new(runtime));

        // Create and set the outbound handler
        let outbound_handler = Arc::new(CapabilityOutboundHandler::new(capabilities));
        let mut handler_guard = self.outbound_handler.write().await;
        *handler_guard = Some(outbound_handler);

        debug!("Created HTTP runtime and outbound handler for service");

        Ok(())
    }

    /// Creates the TCP runtime for this service.
    ///
    /// Called during `start()` for TCP services.
    async fn create_tcp_runtime(&self) -> Result<()> {
        use fabricks_runtime::VolumeMountConfig;

        let state = self.state.read().await;
        let capabilities = state.config.capabilities.clone();
        let args = state.config.args.clone();
        let service_volumes = state.config.volumes.clone();
        drop(state);

        // Convert daemon VolumeMount to runtime VolumeMountConfig
        let volume_mounts: Vec<VolumeMountConfig> = service_volumes
            .iter()
            .map(|vm| VolumeMountConfig {
                host_path: vm.host_path.clone(),
                guest_path: vm.guest_path.clone(),
                read_only: vm.read_only,
            })
            .collect();

        let config = TcpRuntimeConfig {
            capabilities,
            args,
            fuel_limit: None,
            connection_timeout: None,
            volume_mounts,
            resource_limits: None,
        };

        let runtime =
            TcpRuntime::new(&self.wasm_bytes, config).map_err(|e| DaemonError::ServiceError {
                id: String::new(),
                reason: format!("Failed to create TCP runtime: {e}"),
            })?;

        let mut runtime_guard = self.tcp_runtime.write().await;
        *runtime_guard = Some(Arc::new(runtime));

        debug!("Created TCP runtime for service");

        Ok(())
    }

    /// Starts the service.
    ///
    /// For command services, spawns the minimum number of replicas.
    /// For HTTP/TCP services, binds the configured listening ports.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The service is not in a startable state
    /// - Instance spawning or port binding fails
    pub async fn start(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if !state.state.can_start() {
            return Err(DaemonError::InvalidStateTransition {
                from: state.state.to_string(),
                to: "starting".to_string(),
            });
        }

        state.set_state(State::Starting);
        let service_id = state.id.clone();
        let service_name = state.name.clone();
        let service_type = state.config.service_type;
        let desired_replicas = state.config.replicas.min as usize;
        state.replicas.desired = desired_replicas;

        // Get listen ports from capabilities for HTTP/TCP services
        let listen_ports: Vec<u16> = state
            .config
            .capabilities
            .network
            .as_ref()
            .and_then(|n| n.listen.clone())
            .unwrap_or_default();

        // Drop state lock before operations
        drop(state);

        info!(
            service_id = %service_id,
            service_type = %service_type,
            replicas = desired_replicas,
            "Starting service"
        );

        // Bind ports for HTTP/TCP services
        if service_type.is_http() || service_type.is_tcp() {
            self.bind_service_ports(
                &service_id,
                &service_name,
                &listen_ports,
                service_type.is_tcp(),
            )
            .await?;
        }

        // Create HTTP runtime for HTTP services
        if service_type.is_http() {
            if let Err(e) = self.create_http_runtime().await {
                error!(service_id = %service_id, error = %e, "Failed to create HTTP runtime");
                self.unbind_service_ports().await;
                return Err(e);
            }
            info!(service_id = %service_id, "Created HTTP runtime");
        }

        // Create TCP runtime for TCP services
        if service_type.is_tcp() {
            if let Err(e) = self.create_tcp_runtime().await {
                error!(service_id = %service_id, error = %e, "Failed to create TCP runtime");
                self.unbind_service_ports().await;
                return Err(e);
            }
            info!(service_id = %service_id, "Created TCP runtime");
        }

        // Spawn instances for command services
        if service_type.is_command() {
            let spawn_errors = self
                .spawn_command_instances(&service_id, desired_replicas)
                .await;

            // Update state based on spawn results
            let mut state = self.state.write().await;
            let instances = self.instances.lock().await;
            let running_count = instances
                .iter()
                .filter(|h| h.instance.state == InstanceState::Running)
                .count();

            state.replicas.running = running_count;
            state.replicas.ready = running_count;
            state.replicas.failed = spawn_errors.len();

            if running_count == 0 && !spawn_errors.is_empty() {
                state.set_state(State::Failed);
                state.set_error(format!("All instances failed to start: {spawn_errors:?}"));
                // Unbind any ports we may have bound
                self.unbind_service_ports().await;
                // Return the first error - we know spawn_errors is not empty from the condition
                if let Some(first_error) = spawn_errors.into_iter().next() {
                    return Err(first_error);
                }
            }

            state.set_state(State::Running);
            info!(service_id = %state.id, running = running_count, "Service started");
        } else {
            // For HTTP/TCP services, we're "running" once ports are bound
            let mut state = self.state.write().await;
            state.replicas.running = 1;
            state.replicas.ready = 1;
            state.set_state(State::Running);
            info!(
                service_id = %state.id,
                ports = ?listen_ports,
                "HTTP/TCP service started"
            );
        }

        Ok(())
    }

    /// Stops the service.
    ///
    /// Stops all running instances and unbinds any ports.
    ///
    /// # Errors
    ///
    /// Returns an error if the service is not in a stoppable state.
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if !state.state.can_stop() {
            return Err(DaemonError::InvalidStateTransition {
                from: state.state.to_string(),
                to: "stopping".to_string(),
            });
        }

        state.set_state(State::Stopping);
        let service_id = state.id.clone();
        drop(state);

        info!(service_id = %service_id, "Stopping service");

        // Unbind any ports for HTTP/TCP services
        self.unbind_service_ports().await;

        // Stop all instances
        let mut instances = self.instances.lock().await;
        for handle in instances.iter_mut() {
            self.stop_instance(handle).await;
        }
        instances.clear();

        // Update state
        let mut state = self.state.write().await;
        state.replicas.running = 0;
        state.replicas.ready = 0;
        state.set_state(State::Stopped);

        info!(service_id = %state.id, "Service stopped");

        Ok(())
    }

    /// Scales the service to the target number of replicas.
    ///
    /// # Arguments
    ///
    /// * `target` - The target number of replicas
    ///
    /// # Errors
    ///
    /// Returns an error if scaling fails.
    pub async fn scale(&self, target: usize) -> Result<()> {
        let state = self.state.read().await;
        if state.state != State::Running {
            return Err(DaemonError::ServiceNotRunning {
                id: state.id.clone(),
            });
        }
        let service_id = state.id.clone();
        drop(state);

        let mut instances = self.instances.lock().await;
        let current = instances.len();

        if target > current {
            // Scale up
            info!(service_id = %service_id, from = current, to = target, "Scaling up");
            drop(instances);

            for _ in 0..(target - current) {
                if let Err(e) = self.spawn_instance().await {
                    warn!(service_id = %service_id, error = %e, "Failed to spawn instance during scale up");
                }
            }
        } else if target < current {
            // Scale down
            info!(service_id = %service_id, from = current, to = target, "Scaling down");

            let to_remove = current - target;
            for _ in 0..to_remove {
                if let Some(mut handle) = instances.pop() {
                    self.stop_instance(&mut handle).await;
                }
            }
        }

        // Update replica state
        let instances = self.instances.lock().await;
        let running_count = instances
            .iter()
            .filter(|h| h.instance.state == InstanceState::Running)
            .count();

        let mut state = self.state.write().await;
        state.replicas.desired = target;
        state.replicas.running = running_count;
        state.replicas.ready = running_count;

        Ok(())
    }

    /// Spawns a new instance.
    async fn spawn_instance(&self) -> Result<()> {
        let state = self.state.read().await;
        let service_id = state.id.clone();
        let config = state.config.clone();
        drop(state);

        // Create runtime configuration from service config.
        // Default fuel limit of 10 billion allows substantial computation while
        // providing a safety cap (similar to Docker's default resource limits).
        const DEFAULT_FUEL_LIMIT: u64 = 10_000_000_000;

        let runtime_config = RuntimeConfig {
            capabilities: config.capabilities.clone(),
            args: config.args.clone(),
            fuel_limit: Some(DEFAULT_FUEL_LIMIT),
            ..Default::default()
        };

        // Get a runtime from the pool
        let runtime = self
            .runtime_pool
            .get_runtime(&config.wasm_digest, &self.wasm_bytes, runtime_config)
            .await?;

        let mut instance = Instance::new(service_id.clone());
        let instance_id = instance.id.clone();

        // Create log capture sinks for stdout/stderr
        let log_buffer = Arc::clone(&self.log_buffer);
        let stdout_sink = LogCaptureSink::new(log_buffer.clone() as Arc<dyn LogWriter>, LogStream::Stdout);
        let stderr_sink = LogCaptureSink::new(log_buffer as Arc<dyn LogWriter>, LogStream::Stderr);

        // Spawn the WASM execution task with log capture.
        // Use spawn_blocking since run_wasm_instance is a synchronous blocking operation.
        let task_service_id = service_id.clone();
        let task = tokio::spawn(async move {
            info!(service_id = %task_service_id, "Spawning blocking task for WASM execution");
            let blocking_result = tokio::task::spawn_blocking(move || {
                info!("Inside spawn_blocking, calling run_wasm_instance");
                let result = run_wasm_instance(&runtime, stdout_sink, stderr_sink);
                info!("run_wasm_instance completed");
                result
            })
            .await;
            info!(service_id = %task_service_id, "Blocking task completed");
            blocking_result.map_err(|e| DaemonError::ServiceError {
                id: task_service_id.clone(),
                reason: format!("Task join error: {e}"),
            })?
        });

        instance.set_state(InstanceState::Running);

        let handle = InstanceHandle {
            instance,
            task: Some(task),
        };

        let mut instances = self.instances.lock().await;
        instances.push(handle);

        debug!(service_id = %service_id, instance_id = %instance_id, "Instance spawned");

        Ok(())
    }

    /// Stops an instance.
    async fn stop_instance(&self, handle: &mut InstanceHandle) {
        handle.instance.set_state(InstanceState::Stopping);

        if let Some(task) = handle.task.take() {
            task.abort();
            match task.await {
                Ok(Ok(())) => {
                    handle.instance.set_exit(0);
                }
                Ok(Err(e)) => {
                    handle.instance.set_error(e.to_string());
                }
                Err(e) if e.is_cancelled() => {
                    handle.instance.set_state(InstanceState::Stopped);
                }
                Err(e) => {
                    handle.instance.set_error(format!("Task panicked: {e}"));
                }
            }
        } else {
            handle.instance.set_state(InstanceState::Stopped);
        }
    }

    /// Binds ports for HTTP/TCP services.
    ///
    /// # Arguments
    ///
    /// * `service_id` - The service ID
    /// * `service_name` - The service name
    /// * `ports` - The ports to bind
    /// * `is_tcp` - Whether to bind as TCP (raw) or HTTP ports
    async fn bind_service_ports(
        &self,
        service_id: &str,
        service_name: &str,
        ports: &[u16],
        is_tcp: bool,
    ) -> Result<()> {
        let Some(proxy_server) = &self.proxy_server else {
            if !ports.is_empty() {
                warn!(
                    service_id = %service_id,
                    "Service has listen ports but no proxy server configured"
                );
            }
            return Ok(());
        };

        let mut bound = self.bound_ports.lock().await;
        let protocol = if is_tcp { "tcp" } else { "http" };

        for &port in ports {
            let result = if is_tcp {
                proxy_server
                    .bind_tcp_port(port, service_id.to_string(), service_name.to_string())
                    .await
            } else {
                proxy_server
                    .bind_port(port, service_id.to_string(), service_name.to_string())
                    .await
            };

            match result {
                Ok(actual_port) => {
                    info!(service_id = %service_id, port = actual_port, protocol, "Bound port");
                    bound.push(actual_port);
                }
                Err(e) => {
                    error!(service_id = %service_id, port, protocol, error = %e, "Failed to bind port");
                    // Unbind any ports we already bound
                    for &p in bound.iter() {
                        let _ = proxy_server.unbind_port(p).await;
                    }
                    bound.clear();
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Unbinds all ports for this service.
    async fn unbind_service_ports(&self) {
        let Some(proxy_server) = &self.proxy_server else {
            return;
        };

        let mut bound = self.bound_ports.lock().await;

        for &port in bound.iter() {
            if let Err(e) = proxy_server.unbind_port(port).await {
                warn!(port, error = %e, "Failed to unbind port");
            } else {
                debug!(port, "Unbound port");
            }
        }

        bound.clear();
    }

    /// Spawns command instances and returns any spawn errors.
    async fn spawn_command_instances(
        &self,
        service_id: &str,
        desired_replicas: usize,
    ) -> Vec<DaemonError> {
        let mut spawn_errors = Vec::new();

        for i in 0..desired_replicas {
            match self.spawn_instance().await {
                Ok(()) => {
                    debug!(service_id = %service_id, instance = i, "Spawned instance");
                }
                Err(e) => {
                    error!(service_id = %service_id, instance = i, error = %e, "Failed to spawn instance");
                    spawn_errors.push(e);
                }
            }
        }

        spawn_errors
    }

    /// Updates the service state for persistence.
    pub async fn update_state<F>(&self, f: F)
    where
        F: FnOnce(&mut ServiceState),
    {
        let mut state = self.state.write().await;
        f(&mut state);
    }

    /// Returns captured log entries for this service.
    ///
    /// # Arguments
    ///
    /// * `tail` - If `Some(n)`, return only the last `n` entries.
    pub fn get_logs(&self, tail: Option<usize>) -> Vec<LogEntry> {
        self.log_buffer.entries(tail)
    }

    /// Marks the service for deletion.
    ///
    /// # Errors
    ///
    /// Returns an error if the service cannot be deleted from its current state.
    pub async fn mark_deleting(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if !state.state.can_delete() {
            return Err(DaemonError::InvalidStateTransition {
                from: state.state.to_string(),
                to: "deleting".to_string(),
            });
        }

        state.set_state(State::Deleting);
        Ok(())
    }
}

/// Runs a WASM instance to completion with log capture.
fn run_wasm_instance(
    runtime: &Runtime,
    stdout: LogCaptureSink,
    stderr: LogCaptureSink,
) -> Result<()> {
    runtime.run_with_output(stdout, stderr)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_test_config() -> ServiceConfig {
        ServiceConfig::new(
            "test-service".to_string(),
            "1.0.0".to_string(),
            PathBuf::from("/tmp/test.wasm"),
            "sha256:test".to_string(),
        )
    }

    /// Minimal valid WASM component for testing.
    fn minimal_component() -> Vec<u8> {
        vec![
            0x00, 0x61, 0x73, 0x6d, // magic: \0asm
            0x0d, 0x00, 0x01, 0x00, // version: component model
        ]
    }

    #[tokio::test]
    async fn test_service_handle_creation() {
        let pool = RuntimePool::new(10).expect("should create pool");
        let config = make_test_config();
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component(), None);

        assert_eq!(handle.name().await, "test-service");
        assert_eq!(handle.current_state().await, State::Creating);
    }

    #[tokio::test]
    async fn test_service_handle_state_transitions() {
        let pool = RuntimePool::new(10).expect("should create pool");
        let mut config = make_test_config();
        config.replicas.min = 0; // No replicas for simpler testing
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component(), None);

        // Update to stopped state manually (simulating initialization complete)
        handle
            .update_state(|s| {
                s.set_state(State::Stopped);
            })
            .await;

        assert_eq!(handle.current_state().await, State::Stopped);

        // Should be able to start from stopped
        let state = handle.current_state().await;
        assert!(state.can_start());
    }

    #[tokio::test]
    async fn test_cannot_start_from_running() {
        let pool = RuntimePool::new(10).expect("should create pool");
        let config = make_test_config();
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component(), None);

        // Manually set to running
        handle
            .update_state(|s| {
                s.set_state(State::Running);
            })
            .await;

        let result = handle.start().await;
        assert!(matches!(
            result,
            Err(DaemonError::InvalidStateTransition { .. })
        ));
    }

    #[tokio::test]
    async fn test_cannot_stop_from_stopped() {
        let pool = RuntimePool::new(10).expect("should create pool");
        let config = make_test_config();
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component(), None);

        // Manually set to stopped
        handle
            .update_state(|s| {
                s.set_state(State::Stopped);
            })
            .await;

        let result = handle.stop().await;
        assert!(matches!(
            result,
            Err(DaemonError::InvalidStateTransition { .. })
        ));
    }

    #[tokio::test]
    async fn test_get_detail() {
        let pool = RuntimePool::new(10).expect("should create pool");
        let config = make_test_config();
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component(), None);

        let detail = handle.get_detail().await;
        assert_eq!(detail.name, "test-service");
        assert_eq!(detail.version, "1.0.0");
        assert!(detail.instances.is_empty());
    }
}
