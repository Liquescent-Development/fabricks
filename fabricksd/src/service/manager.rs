//! Service manager for coordinating service lifecycle.
//!
//! The `ServiceManager` is responsible for:
//! - Creating and tracking services
//! - Starting and stopping services with dependency awareness
//! - Scaling services
//! - Managing mortar projects (multi-service deployments)

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use fabricks_common::models::fabrickfile::Fabrickfile;
use fabricks_common::models::mortar::MortarFile;
use fabricks_runtime::{RuntimePool, RuntimePoolBuilder};

use crate::error::{DaemonError, Result};
use crate::events::{Event, EventBus, EventType};
use crate::network::{
    NetworkAccess, NetworkAudit, NetworkConfig, NetworkEncryption, NetworkIsolation,
    NetworkOptions, SharedNetworkManager,
};
use fabricks_common::models::mortar::EncryptionRequirement;
use crate::proxy::SharedProxyServer;
use crate::store::StateStore;

use super::dependency::{resolve_shutdown_order, resolve_startup_order, validate_dependencies};
use super::handle::ServiceHandle;
use super::types::{ServiceConfig, ServiceDetail, ServiceInfo, ServiceState, State};

/// Tree name for service state persistence.
const SERVICES_TREE: &str = "services";

/// Service manager for coordinating service lifecycle.
pub struct ServiceManager {
    /// Active service handles.
    services: RwLock<HashMap<String, Arc<ServiceHandle>>>,

    /// Runtime pool for WASM execution.
    runtime_pool: Arc<RuntimePool>,

    /// State store for persistence.
    state_store: Arc<StateStore>,

    /// Event bus for publishing events.
    event_bus: Arc<EventBus>,

    /// Proxy server for HTTP/TCP port bindings.
    proxy_server: Option<SharedProxyServer>,

    /// Network manager for network isolation.
    network_manager: Option<SharedNetworkManager>,

    /// Mortar project tracking (`project_name` -> `service_ids`).
    mortar_projects: RwLock<HashMap<String, Vec<String>>>,
}

impl ServiceManager {
    /// Creates a new service manager.
    ///
    /// # Arguments
    ///
    /// * `state_store` - Store for persisting service state
    /// * `event_bus` - Bus for publishing service events
    /// * `max_cached_modules` - Maximum number of WASM modules to cache
    /// * `proxy_server` - Optional proxy server for HTTP/TCP port bindings
    /// * `network_manager` - Optional network manager for network isolation
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime pool cannot be created.
    pub fn new(
        state_store: Arc<StateStore>,
        event_bus: Arc<EventBus>,
        max_cached_modules: usize,
        proxy_server: Option<SharedProxyServer>,
        network_manager: Option<SharedNetworkManager>,
    ) -> Result<Self> {
        let runtime_pool = RuntimePoolBuilder::new()
            .max_modules(max_cached_modules)
            .with_fuel()
            .build()?;

        Ok(Self {
            services: RwLock::new(HashMap::new()),
            runtime_pool: Arc::new(runtime_pool),
            state_store,
            event_bus,
            proxy_server,
            network_manager,
            mortar_projects: RwLock::new(HashMap::new()),
        })
    }

    /// Creates a new service from a configuration.
    ///
    /// The service is created in the `Creating` state and must be started separately.
    ///
    /// # Arguments
    ///
    /// * `config` - Service configuration
    ///
    /// # Returns
    ///
    /// The ID of the created service.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A service with the same name already exists
    /// - The WASM module cannot be loaded
    /// - State persistence fails
    pub async fn create_service(&self, config: ServiceConfig) -> Result<String> {
        // Check for duplicate name
        {
            let services = self.services.read().await;
            for handle in services.values() {
                if handle.name().await == config.name {
                    return Err(DaemonError::ServiceAlreadyExists {
                        name: config.name.clone(),
                    });
                }
            }
        }

        // Load WASM bytes
        let wasm_bytes = tokio::fs::read(&config.wasm_path).await.map_err(|_| {
            DaemonError::WasmModuleNotFound {
                path: config.wasm_path.display().to_string(),
            }
        })?;

        // Create service handle
        let handle = ServiceHandle::new(
            config.clone(),
            Arc::clone(&self.runtime_pool),
            wasm_bytes,
            self.proxy_server.clone(),
        );
        let id = handle.id().await;

        // Persist state
        let state = handle.get_state().await;
        self.state_store.put(SERVICES_TREE, &id, &state)?;

        // Track in mortar project if applicable
        if let Some(ref project) = config.mortar_project {
            let mut projects = self.mortar_projects.write().await;
            projects
                .entry(project.clone())
                .or_default()
                .push(id.clone());
        }

        // Store handle
        {
            let mut services = self.services.write().await;
            services.insert(id.clone(), Arc::new(handle));
        }

        // Publish event
        self.event_bus
            .publish(Event::new(
                EventType::ServiceCreated,
                serde_json::json!({
                    "id": id,
                    "name": config.name,
                    "version": config.version,
                }),
            ))
            .await;

        info!(service_id = %id, name = %config.name, "Service created");

        Ok(id)
    }

    /// Creates and starts a service from a Fabrickfile.
    ///
    /// This is a convenience method that parses the Fabrickfile, creates the service,
    /// and starts it in one operation.
    ///
    /// # Arguments
    ///
    /// * `fabrickfile_path` - Path to the Fabrickfile
    /// * `wasm_path` - Optional path to pre-built WASM (if not specified, uses build.output)
    ///
    /// # Errors
    ///
    /// Returns an error if parsing, creation, or startup fails.
    pub async fn run_fabrickfile(
        &self,
        fabrickfile_path: &Path,
        wasm_path: Option<&Path>,
    ) -> Result<String> {
        // Parse Fabrickfile
        let content = tokio::fs::read_to_string(fabrickfile_path)
            .await
            .map_err(|e| DaemonError::FabrickfileParseError(e.to_string()))?;

        let fabrickfile: Fabrickfile = toml::from_str(&content)
            .map_err(|e| DaemonError::FabrickfileParseError(e.to_string()))?;

        // Determine WASM path
        let wasm_path = if let Some(path) = wasm_path {
            path.to_path_buf()
        } else if let Some(ref build) = fabrickfile.build {
            // Resolve relative to Fabrickfile directory
            let base_dir = fabrickfile_path
                .parent()
                .unwrap_or_else(|| Path::new("."));
            base_dir.join(&build.output)
        } else {
            return Err(DaemonError::FabrickfileParseError(
                "No WASM path specified and no build configuration found".to_string(),
            ));
        };

        // Compute digest
        let wasm_bytes = tokio::fs::read(&wasm_path).await.map_err(|_| {
            DaemonError::WasmModuleNotFound {
                path: wasm_path.display().to_string(),
            }
        })?;
        let digest = compute_digest(&wasm_bytes);

        // Build config from Fabrickfile
        let config = ServiceConfig {
            name: fabrickfile.info.name.clone(),
            version: fabrickfile.info.version.clone(),
            service_type: fabrickfile.info.service_type,
            wasm_path,
            wasm_digest: digest,
            capabilities: fabrickfile.capabilities.clone(),
            environment: fabrickfile
                .config
                .as_ref()
                .and_then(|c| c.environment.clone())
                .unwrap_or_default(),
            args: Vec::new(),
            resources: fabrickfile.config.as_ref().and_then(|c| c.resources.clone()),
            replicas: fabricks_common::models::Replicas::default(),
            health_check: fabrickfile.health_check.clone(),
            depends_on: Vec::new(),
            networks: Vec::new(),
            mortar_project: None,
        };

        // Create and start
        let id = self.create_service(config).await?;
        self.start_service(&id).await?;

        Ok(id)
    }

    /// Starts a service.
    ///
    /// # Errors
    ///
    /// Returns an error if the service is not found or cannot be started.
    pub async fn start_service(&self, id: &str) -> Result<()> {
        let handle = self.get_handle(id).await?;

        // Update state to stopped if creating (initialization complete)
        if handle.current_state().await == State::Creating {
            handle
                .update_state(|s| {
                    s.set_state(State::Stopped);
                })
                .await;
        }

        handle.start().await?;

        // Persist updated state
        let state = handle.get_state().await;
        self.state_store.put(SERVICES_TREE, id, &state)?;

        // Publish event
        self.event_bus
            .publish(Event::new(
                EventType::ServiceStarted,
                serde_json::json!({
                    "id": id,
                    "name": state.name,
                }),
            ))
            .await;

        Ok(())
    }

    /// Stops a service.
    ///
    /// # Errors
    ///
    /// Returns an error if the service is not found or cannot be stopped.
    pub async fn stop_service(&self, id: &str) -> Result<()> {
        let handle = self.get_handle(id).await?;

        handle.stop().await?;

        // Persist updated state
        let state = handle.get_state().await;
        self.state_store.put(SERVICES_TREE, id, &state)?;

        // Publish event
        self.event_bus
            .publish(Event::new(
                EventType::ServiceStopped,
                serde_json::json!({
                    "id": id,
                    "name": state.name,
                }),
            ))
            .await;

        Ok(())
    }

    /// Scales a service to the target number of replicas.
    ///
    /// # Errors
    ///
    /// Returns an error if the service is not found or scaling fails.
    pub async fn scale_service(&self, id: &str, replicas: usize) -> Result<()> {
        let handle = self.get_handle(id).await?;

        handle.scale(replicas).await?;

        // Persist updated state
        let state = handle.get_state().await;
        self.state_store.put(SERVICES_TREE, id, &state)?;

        // Publish event
        self.event_bus
            .publish(Event::new(
                EventType::ServiceScaled,
                serde_json::json!({
                    "id": id,
                    "name": state.name,
                    "replicas": replicas,
                }),
            ))
            .await;

        Ok(())
    }

    /// Deletes a service.
    ///
    /// The service must be stopped before it can be deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the service is not found or cannot be deleted.
    pub async fn delete_service(&self, id: &str) -> Result<()> {
        let handle = self.get_handle(id).await?;

        // Mark for deletion
        handle.mark_deleting().await?;

        let state = handle.get_state().await;
        let name = state.name.clone();
        let mortar_project = state.mortar_project.clone();

        // Remove from services
        {
            let mut services = self.services.write().await;
            services.remove(id);
        }

        // Remove from mortar project tracking
        if let Some(project) = mortar_project {
            let mut projects = self.mortar_projects.write().await;
            if let Some(service_ids) = projects.get_mut(&project) {
                service_ids.retain(|sid| sid != id);
                if service_ids.is_empty() {
                    projects.remove(&project);
                }
            }
        }

        // Remove from persistence
        self.state_store.delete(SERVICES_TREE, id)?;

        // Publish event
        self.event_bus
            .publish(Event::new(
                EventType::ServiceDeleted,
                serde_json::json!({
                    "id": id,
                    "name": name,
                }),
            ))
            .await;

        info!(service_id = %id, name = %name, "Service deleted");

        Ok(())
    }

    /// Gets a service by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the service is not found.
    pub async fn get_service(&self, id: &str) -> Result<ServiceDetail> {
        let handle = self.get_handle(id).await?;
        Ok(handle.get_detail().await)
    }

    /// Lists all services.
    pub async fn list_services(&self) -> Vec<ServiceInfo> {
        let services = self.services.read().await;
        let mut infos = Vec::with_capacity(services.len());

        for handle in services.values() {
            let state = handle.get_state().await;
            infos.push(ServiceInfo::from(&state));
        }

        infos
    }

    /// Gets a service handle by ID.
    async fn get_handle(&self, id: &str) -> Result<Arc<ServiceHandle>> {
        let services = self.services.read().await;
        services
            .get(id)
            .cloned()
            .ok_or_else(|| DaemonError::ServiceNotFound { id: id.to_string() })
    }

    /// Routes an HTTP request to the appropriate service.
    ///
    /// This is called by the proxy server's request handler to delegate
    /// incoming HTTP requests to the correct service's WASM runtime.
    ///
    /// # Arguments
    ///
    /// * `service_id` - The ID of the service to route to
    /// * `request` - The HTTP request to handle
    ///
    /// # Errors
    ///
    /// Returns an error if the service is not found or request handling fails.
    pub async fn route_http_request(
        &self,
        service_id: &str,
        request: fabricks_runtime::HttpRequest,
    ) -> Result<fabricks_runtime::HttpResponse> {
        let handle = self.get_handle(service_id).await?;
        handle
            .handle_http_request(request)
            .await
            .map_err(|e| DaemonError::ServiceError {
                id: service_id.to_string(),
                reason: e.to_string(),
            })
    }

    /// Routes a TCP connection to the appropriate service.
    ///
    /// This is called by the proxy server's TCP connection handler to delegate
    /// incoming TCP connections to the correct service's WASM runtime using the
    /// inetd model (stdin/stdout connected to the TCP stream).
    ///
    /// # Arguments
    ///
    /// * `service_id` - The ID of the service to route to
    /// * `stream` - The TCP stream to connect
    /// * `peer_addr` - The peer's socket address
    ///
    /// # Errors
    ///
    /// Returns an error if the service is not found or connection handling fails.
    pub async fn route_tcp_connection(
        &self,
        service_id: &str,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<()> {
        let handle = self.get_handle(service_id).await?;
        handle
            .handle_tcp_connection(stream, peer_addr)
            .await
            .map_err(|e| DaemonError::ServiceError {
                id: service_id.to_string(),
                reason: e.to_string(),
            })
    }

    // ==================== Mortar Project Operations ====================

    /// Creates networks defined in a mortar file.
    ///
    /// Networks that already exist are silently skipped (idempotent).
    async fn create_mortar_networks(&self, mortar: &MortarFile) -> Result<()> {
        let Some(network_manager) = &self.network_manager else {
            return Ok(());
        };

        let Some(networks) = &mortar.network else {
            return Ok(());
        };

        let mut created_networks: Vec<String> = Vec::new();
        for (network_name, network) in networks {
            let options = NetworkOptions::new(
                if network.internal.unwrap_or(false) {
                    NetworkAccess::Internal
                } else {
                    NetworkAccess::External
                },
                if network.isolated.unwrap_or(false) {
                    NetworkIsolation::Isolated
                } else {
                    NetworkIsolation::Connected
                },
                if network.encryption == Some(EncryptionRequirement::Required) {
                    NetworkEncryption::Required
                } else {
                    NetworkEncryption::Optional
                },
                if network.audit_all.unwrap_or(false) {
                    NetworkAudit::Enabled
                } else {
                    NetworkAudit::Disabled
                },
            );

            let config = if let Some(ref desc) = network.description {
                NetworkConfig::with_description(network_name.clone(), desc.clone(), options)
            } else {
                NetworkConfig::with_options(network_name.clone(), options)
            };

            match network_manager.create_network(config).await {
                Ok(id) => {
                    info!(network = %network_name, network_id = %id, "Created network");
                    created_networks.push(id);
                }
                Err(e) => {
                    // Duplicate name is OK for idempotent deploys
                    if e.to_string().contains("already exists") {
                        debug!(network = %network_name, "Network already exists, skipping");
                    } else {
                        // Rollback created networks
                        error!(network = %network_name, error = %e, "Failed to create network");
                        for id in &created_networks {
                            if let Err(del_err) = network_manager.delete_network(id).await {
                                warn!(network_id = %id, error = %del_err, "Failed to delete network");
                            }
                        }
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Joins a service to its specified networks.
    async fn join_service_to_networks(
        &self,
        service_id: &str,
        service_name: &str,
        networks: &[String],
    ) {
        let Some(network_manager) = &self.network_manager else {
            return;
        };

        for network_name in networks {
            if let Some(network) = network_manager.get_network_by_name(network_name).await {
                if let Err(e) = network_manager
                    .add_service(&network.id, service_id, service_name)
                    .await
                {
                    warn!(service_id = %service_id, network = %network_name, error = %e, "Failed to join network");
                } else {
                    debug!(service_id = %service_id, network = %network_name, "Service joined network");
                }
            } else {
                warn!(service = %service_name, network = %network_name, "Network not found, skipping");
            }
        }
    }

    /// Deploys a mortar project (multiple services).
    ///
    /// Services are started in dependency order.
    ///
    /// # Arguments
    ///
    /// * `mortar_path` - Path to the fabricks-mortar.toml file
    ///
    /// # Returns
    ///
    /// The project name and list of created service IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing or deployment fails.
    pub async fn deploy_mortar(&self, mortar_path: &Path) -> Result<(String, Vec<String>)> {
        // Parse mortar file
        let content = tokio::fs::read_to_string(mortar_path)
            .await
            .map_err(|e| DaemonError::FabrickfileParseError(e.to_string()))?;

        let mortar: MortarFile = toml::from_str(&content)
            .map_err(|e| DaemonError::FabrickfileParseError(e.to_string()))?;

        let project_name = mortar.project.name.clone();
        let base_dir = mortar_path.parent().unwrap_or_else(|| Path::new("."));

        info!(project = %project_name, "Deploying mortar project");

        // Create networks from mortar definition
        self.create_mortar_networks(&mortar).await?;

        // Build service configs
        let mut configs: Vec<ServiceConfig> = Vec::new();

        for (service_name, service) in &mortar.service {
            // Determine WASM path
            let wasm_path = resolve_wasm_path_for_service(service_name, service, base_dir).await?;

            // Load WASM and compute digest
            let wasm_bytes = tokio::fs::read(&wasm_path).await.map_err(|_| {
                DaemonError::WasmModuleNotFound {
                    path: wasm_path.display().to_string(),
                }
            })?;
            let digest = compute_digest(&wasm_bytes);

            let config = ServiceConfig {
                name: service.name.clone().unwrap_or_else(|| service_name.clone()),
                version: service.version.clone().unwrap_or_else(|| "latest".to_string()),
                service_type: service.service_type.unwrap_or_default(),
                wasm_path,
                wasm_digest: digest,
                capabilities: fabricks_common::Capabilities::default(),
                environment: service.environment.clone().unwrap_or_default(),
                args: Vec::new(),
                resources: service.resources.clone(),
                replicas: service.replicas.clone().unwrap_or_default(),
                health_check: service.health_check.clone(),
                depends_on: service.depends_on.clone().unwrap_or_default(),
                networks: service.networks.clone(),
                mortar_project: Some(project_name.clone()),
            };

            configs.push(config);
        }

        // Validate dependencies
        validate_dependencies(&configs)?;

        // Resolve startup order
        let startup_order = resolve_startup_order(&configs)?;

        // Create services in order
        let mut created_ids: Vec<String> = Vec::new();

        for service_name in &startup_order {
            let config = configs
                .iter()
                .find(|c| &c.name == service_name)
                .ok_or_else(|| DaemonError::ServiceNotFound {
                    id: service_name.clone(),
                })?
                .clone();

            let service_networks = config.networks.clone();

            match self.create_service(config).await {
                Ok(id) => {
                    // Join networks if specified
                    self.join_service_to_networks(&id, service_name, &service_networks)
                        .await;
                    created_ids.push(id);
                }
                Err(e) => {
                    // Rollback: delete already created services
                    error!(
                        project = %project_name,
                        service = %service_name,
                        error = %e,
                        "Failed to create service, rolling back"
                    );
                    for id in &created_ids {
                        if let Err(del_err) = self.delete_service(id).await {
                            warn!(service_id = %id, error = %del_err, "Failed to delete service during rollback");
                        }
                    }
                    return Err(e);
                }
            }
        }

        // Start services in order
        for id in &created_ids {
            if let Err(e) = self.start_service(id).await {
                error!(service_id = %id, error = %e, "Failed to start service");
                // Continue trying to start others, but log the failure
            }
        }

        info!(project = %project_name, services = created_ids.len(), "Mortar project deployed");

        Ok((project_name, created_ids))
    }

    /// Tears down a mortar project.
    ///
    /// Services are stopped in reverse dependency order.
    ///
    /// # Errors
    ///
    /// Returns an error if the project is not found or teardown fails.
    pub async fn teardown_mortar(&self, project_name: &str) -> Result<()> {
        let service_ids = {
            let projects = self.mortar_projects.read().await;
            projects
                .get(project_name)
                .cloned()
                .ok_or_else(|| DaemonError::MortarProjectNotFound {
                    name: project_name.to_string(),
                })?
        };

        info!(project = %project_name, services = service_ids.len(), "Tearing down mortar project");

        // Get configs for dependency resolution
        let mut configs: Vec<ServiceConfig> = Vec::new();
        for id in &service_ids {
            if let Ok(detail) = self.get_service(id).await {
                configs.push(detail.config);
            }
        }

        // Resolve shutdown order (reverse of startup)
        let shutdown_order = resolve_shutdown_order(&configs)?;

        // Stop and delete in order
        for service_name in &shutdown_order {
            // Find service ID by name
            let id = {
                let services = self.services.read().await;
                let mut found_id = None;
                for (sid, handle) in services.iter() {
                    if handle.name().await == *service_name {
                        found_id = Some(sid.clone());
                        break;
                    }
                }
                found_id
            };

            if let Some(id) = id {
                // Stop if running
                if let Ok(detail) = self.get_service(&id).await
                    && detail.state.can_stop()
                    && let Err(e) = self.stop_service(&id).await
                {
                    warn!(service_id = %id, error = %e, "Failed to stop service");
                }

                // Delete
                if let Err(e) = self.delete_service(&id).await {
                    warn!(service_id = %id, error = %e, "Failed to delete service");
                }
            }
        }

        info!(project = %project_name, "Mortar project torn down");

        Ok(())
    }

    /// Lists services in a mortar project.
    ///
    /// # Errors
    ///
    /// Returns an error if the project is not found.
    pub async fn list_mortar_services(&self, project_name: &str) -> Result<Vec<ServiceInfo>> {
        let service_ids = {
            let projects = self.mortar_projects.read().await;
            projects
                .get(project_name)
                .cloned()
                .ok_or_else(|| DaemonError::MortarProjectNotFound {
                    name: project_name.to_string(),
                })?
        };

        let mut infos = Vec::with_capacity(service_ids.len());
        for id in &service_ids {
            if let Ok(detail) = self.get_service(id).await {
                infos.push(ServiceInfo {
                    id: detail.id,
                    name: detail.name,
                    version: detail.version,
                    state: detail.state,
                    replicas: detail.replicas,
                    created_at: detail.created_at,
                    mortar_project: detail.mortar_project,
                });
            }
        }

        Ok(infos)
    }

    /// Lists all mortar projects.
    pub async fn list_mortar_projects(&self) -> Vec<String> {
        let projects = self.mortar_projects.read().await;
        projects.keys().cloned().collect()
    }

    /// Gets the total number of services.
    pub async fn service_count(&self) -> usize {
        let services = self.services.read().await;
        services.len()
    }

    /// Recovers services from persistent storage on startup.
    ///
    /// # Errors
    ///
    /// Returns an error if recovery fails.
    pub async fn recover_from_store(&self) -> Result<()> {
        let states: Vec<ServiceState> = self.state_store.list(SERVICES_TREE)?;

        if states.is_empty() {
            debug!("No services to recover from store");
            return Ok(());
        }

        info!(count = states.len(), "Recovering services from store");

        for state in states {
            // Load WASM bytes
            let wasm_bytes = match tokio::fs::read(&state.config.wasm_path).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    warn!(
                        service_id = %state.id,
                        path = %state.config.wasm_path.display(),
                        error = %e,
                        "Failed to load WASM for recovered service, skipping"
                    );
                    continue;
                }
            };

            let handle = ServiceHandle::from_state(
                state.clone(),
                Arc::clone(&self.runtime_pool),
                wasm_bytes,
                self.proxy_server.clone(),
            );
            let id = state.id.clone();

            // Track in mortar project if applicable
            if let Some(ref project) = state.mortar_project {
                let mut projects = self.mortar_projects.write().await;
                projects.entry(project.clone()).or_default().push(id.clone());
            }

            // Store handle
            {
                let mut services = self.services.write().await;
                services.insert(id.clone(), Arc::new(handle));
            }

            debug!(service_id = %id, name = %state.name, "Recovered service");
        }

        Ok(())
    }
}

/// Computes the SHA256 digest of WASM bytes.
fn compute_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    format!("sha256:{}", hex::encode(result))
}

/// Resolves the WASM path for a mortar service.
async fn resolve_wasm_path_for_service(
    service_name: &str,
    service: &fabricks_common::models::mortar::Service,
    base_dir: &Path,
) -> Result<std::path::PathBuf> {
    if let Some(ref build_path) = service.build {
        let fabrickfile_path = base_dir.join(build_path).join("Fabrickfile");

        if !fabrickfile_path.exists() {
            return Err(DaemonError::FabrickfileParseError(format!(
                "Fabrickfile not found at {}",
                fabrickfile_path.display()
            )));
        }

        let ff_content = tokio::fs::read_to_string(&fabrickfile_path)
            .await
            .map_err(|e| DaemonError::FabrickfileParseError(e.to_string()))?;

        let fabrickfile: Fabrickfile = toml::from_str(&ff_content)
            .map_err(|e| DaemonError::FabrickfileParseError(e.to_string()))?;

        if let Some(ref build) = fabrickfile.build {
            Ok(base_dir.join(build_path).join(&build.output))
        } else {
            Err(DaemonError::FabrickfileParseError(format!(
                "No build configuration in Fabrickfile for service '{service_name}'"
            )))
        }
    } else if service.image.is_some() {
        Err(DaemonError::FabrickfileParseError(
            "Image-based services not yet implemented".to_string(),
        ))
    } else {
        Err(DaemonError::FabrickfileParseError(format!(
            "Service '{service_name}' must have either 'build' or 'image'"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    async fn create_test_manager() -> (ServiceManager, tempfile::TempDir) {
        let dir = tempdir().expect("should create temp dir");
        let db = sled::open(dir.path()).expect("should open db");
        let store = Arc::new(StateStore::new(Arc::new(db)));
        let event_bus = Arc::new(EventBus::new(100, 1000));

        let manager =
            ServiceManager::new(store, event_bus, 10, None, None).expect("should create manager");
        (manager, dir)
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let (manager, _dir) = create_test_manager().await;
        assert_eq!(manager.service_count().await, 0);
    }

    #[tokio::test]
    async fn test_list_services_empty() {
        let (manager, _dir) = create_test_manager().await;
        let services = manager.list_services().await;
        assert!(services.is_empty());
    }

    #[tokio::test]
    async fn test_get_nonexistent_service() {
        let (manager, _dir) = create_test_manager().await;
        let result = manager.get_service("nonexistent").await;
        assert!(matches!(result, Err(DaemonError::ServiceNotFound { .. })));
    }

    #[tokio::test]
    async fn test_list_mortar_projects_empty() {
        let (manager, _dir) = create_test_manager().await;
        let projects = manager.list_mortar_projects().await;
        assert!(projects.is_empty());
    }

    #[test]
    fn test_compute_digest() {
        let bytes = b"hello world";
        let digest = compute_digest(bytes);
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), 7 + 64); // "sha256:" + 64 hex chars
    }
}
