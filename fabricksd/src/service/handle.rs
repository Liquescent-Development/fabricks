//! Service handle for managing service instances.
//!
//! A `ServiceHandle` manages the lifecycle of individual WASM instances for a service,
//! including spawning, scaling, and stopping instances.

use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use fabricks_runtime::{Runtime, RuntimeConfig, RuntimePool};

use crate::error::{DaemonError, Result};

use super::types::{
    Instance, InstanceState, ServiceConfig, ServiceDetail, ServiceState, State,
};

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
    ///
    /// # Returns
    ///
    /// A new service handle in the Creating state.
    pub fn new(config: ServiceConfig, runtime_pool: Arc<RuntimePool>, wasm_bytes: Vec<u8>) -> Self {
        let state = ServiceState::new(config);
        Self {
            state: RwLock::new(state),
            instances: Mutex::new(Vec::new()),
            runtime_pool,
            wasm_bytes: Arc::new(wasm_bytes),
        }
    }

    /// Creates a service handle from existing state (for recovery).
    pub fn from_state(
        state: ServiceState,
        runtime_pool: Arc<RuntimePool>,
        wasm_bytes: Vec<u8>,
    ) -> Self {
        Self {
            state: RwLock::new(state),
            instances: Mutex::new(Vec::new()),
            runtime_pool,
            wasm_bytes: Arc::new(wasm_bytes),
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

        let instances: Vec<Instance> = instances_guard
            .iter()
            .map(|h| h.instance.clone())
            .collect();

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
        }
    }

    /// Starts the service.
    ///
    /// Spawns the minimum number of replicas defined in the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The service is not in a startable state
    /// - Instance spawning fails
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
        let desired_replicas = state.config.replicas.min as usize;
        state.replicas.desired = desired_replicas;

        // Drop state lock before spawning
        drop(state);

        info!(service_id = %service_id, replicas = desired_replicas, "Starting service");

        // Spawn instances
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
            return Err(spawn_errors.remove(0));
        }

        state.set_state(State::Running);
        info!(service_id = %state.id, running = running_count, "Service started");

        Ok(())
    }

    /// Stops the service.
    ///
    /// Stops all running instances gracefully.
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

        // Create runtime configuration from service config
        let runtime_config = RuntimeConfig {
            capabilities: config.capabilities.clone(),
            args: config.args.clone(),
            ..Default::default()
        };

        // Get a runtime from the pool
        let runtime = self
            .runtime_pool
            .get_runtime(&config.wasm_digest, &self.wasm_bytes, runtime_config)
            .await?;

        let mut instance = Instance::new(service_id.clone());
        let instance_id = instance.id.clone();

        // Spawn the WASM execution task
        let task = tokio::spawn(async move { run_wasm_instance(&runtime) });

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

    /// Updates the service state for persistence.
    pub async fn update_state<F>(&self, f: F)
    where
        F: FnOnce(&mut ServiceState),
    {
        let mut state = self.state.write().await;
        f(&mut state);
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

/// Runs a WASM instance to completion.
fn run_wasm_instance(runtime: &Runtime) -> Result<()> {
    // Run the WASM module
    runtime.run()?;
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
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component());

        assert_eq!(handle.name().await, "test-service");
        assert_eq!(handle.current_state().await, State::Creating);
    }

    #[tokio::test]
    async fn test_service_handle_state_transitions() {
        let pool = RuntimePool::new(10).expect("should create pool");
        let mut config = make_test_config();
        config.replicas.min = 0; // No replicas for simpler testing
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component());

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
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component());

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
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component());

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
        let handle = ServiceHandle::new(config, Arc::new(pool), minimal_component());

        let detail = handle.get_detail().await;
        assert_eq!(detail.name, "test-service");
        assert_eq!(detail.version, "1.0.0");
        assert!(detail.instances.is_empty());
    }
}
