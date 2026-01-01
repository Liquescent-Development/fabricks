//! Instance pooling for efficient runtime reuse.
//!
//! Provides a pool of compiled WASM modules with shared engines to reduce
//! compilation overhead and memory usage.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use wasmtime::Config;
use wasmtime::Engine;

use crate::error::Result;
use crate::runtime::{Runtime, RuntimeConfig};

/// A pool of WASM runtimes for efficient reuse.
///
/// The pool shares a single Wasmtime engine across all runtimes, reducing
/// memory usage and compilation time for the same module.
pub struct RuntimePool {
    /// Shared Wasmtime engine.
    engine: Arc<Engine>,

    /// Cached compiled modules by digest.
    modules: Mutex<HashMap<String, Arc<Vec<u8>>>>,

    /// Maximum number of cached modules.
    max_modules: usize,

    /// Default configuration for new runtimes.
    default_config: RuntimeConfig,
}

impl RuntimePool {
    /// Create a new runtime pool.
    ///
    /// # Arguments
    ///
    /// * `max_modules` - Maximum number of modules to cache
    ///
    /// # Errors
    ///
    /// Returns an error if the engine cannot be created.
    pub fn new(max_modules: usize) -> Result<Self> {
        let mut engine_config = Config::new();
        // Enable component model for WASI Preview 2
        engine_config.wasm_component_model(true);
        let engine = Engine::new(&engine_config)?;

        Ok(Self {
            engine: Arc::new(engine),
            modules: Mutex::new(HashMap::new()),
            max_modules,
            default_config: RuntimeConfig::default(),
        })
    }

    /// Create a pool with custom engine configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine cannot be created.
    pub fn with_config(max_modules: usize, engine_config: &Config) -> Result<Self> {
        let engine = Engine::new(engine_config)?;

        Ok(Self {
            engine: Arc::new(engine),
            modules: Mutex::new(HashMap::new()),
            max_modules,
            default_config: RuntimeConfig::default(),
        })
    }

    /// Set the default runtime configuration.
    pub fn set_default_config(&mut self, config: RuntimeConfig) {
        self.default_config = config;
    }

    /// Create a runtime from cached or new WASM bytes.
    ///
    /// If the module with the given digest is already cached, it will be reused.
    /// Otherwise, the WASM bytes will be compiled and cached.
    ///
    /// # Arguments
    ///
    /// * `digest` - Unique identifier for the module (typically SHA256)
    /// * `wasm_bytes` - The WASM binary (only used if not cached)
    /// * `config` - Runtime configuration
    ///
    /// # Errors
    ///
    /// Returns an error if compilation fails.
    pub async fn get_runtime(
        &self,
        digest: &str,
        wasm_bytes: &[u8],
        config: RuntimeConfig,
    ) -> Result<Runtime> {
        // Check cache
        let cached = {
            let modules = self.modules.lock().await;
            modules.get(digest).cloned()
        };

        let bytes = if let Some(cached_bytes) = cached {
            cached_bytes
        } else {
            // Cache the module bytes
            let bytes = Arc::new(wasm_bytes.to_vec());
            let mut modules = self.modules.lock().await;

            // Evict oldest if at capacity
            if modules.len() >= self.max_modules {
                // Simple eviction: remove first key
                if let Some(key) = modules.keys().next().cloned() {
                    modules.remove(&key);
                }
            }

            modules.insert(digest.to_string(), Arc::clone(&bytes));
            bytes
        };

        Runtime::with_engine(Arc::clone(&self.engine), &bytes, config)
    }

    /// Create a runtime with the default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if compilation fails.
    pub async fn get_runtime_default(
        &self,
        digest: &str,
        wasm_bytes: &[u8],
    ) -> Result<Runtime> {
        self.get_runtime(digest, wasm_bytes, self.default_config.clone())
            .await
    }

    /// Check if a module is cached.
    pub async fn has_module(&self, digest: &str) -> bool {
        let modules = self.modules.lock().await;
        modules.contains_key(digest)
    }

    /// Remove a module from the cache.
    pub async fn evict(&self, digest: &str) {
        let mut modules = self.modules.lock().await;
        modules.remove(digest);
    }

    /// Clear all cached modules.
    pub async fn clear(&self) {
        let mut modules = self.modules.lock().await;
        modules.clear();
    }

    /// Get the number of cached modules.
    pub async fn cached_count(&self) -> usize {
        let modules = self.modules.lock().await;
        modules.len()
    }

    /// Get the shared engine.
    #[must_use]
    pub fn engine(&self) -> Arc<Engine> {
        Arc::clone(&self.engine)
    }
}

/// Builder for creating a runtime pool with custom configuration.
pub struct RuntimePoolBuilder {
    max_modules: usize,
    engine_config: Config,
    default_runtime_config: RuntimeConfig,
}

impl Default for RuntimePoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimePoolBuilder {
    /// Create a new pool builder with defaults.
    #[must_use]
    pub fn new() -> Self {
        let mut engine_config = Config::new();
        // Enable component model for WASI Preview 2
        engine_config.wasm_component_model(true);

        Self {
            max_modules: 100,
            engine_config,
            default_runtime_config: RuntimeConfig::default(),
        }
    }

    /// Set the maximum number of cached modules.
    #[must_use]
    pub const fn max_modules(mut self, max: usize) -> Self {
        self.max_modules = max;
        self
    }

    /// Enable fuel-based metering in the engine.
    #[must_use]
    pub fn with_fuel(mut self) -> Self {
        self.engine_config.consume_fuel(true);
        self
    }

    /// Enable epoch-based interruption.
    #[must_use]
    pub fn with_epoch_interruption(mut self) -> Self {
        self.engine_config.epoch_interruption(true);
        self
    }

    /// Enable WASM SIMD.
    #[must_use]
    pub fn with_simd(mut self) -> Self {
        self.engine_config.wasm_simd(true);
        self
    }

    /// Enable WASM threads.
    #[must_use]
    pub fn with_threads(mut self) -> Self {
        self.engine_config.wasm_threads(true);
        self
    }

    /// Set the default runtime configuration.
    #[must_use]
    pub fn default_runtime_config(mut self, config: RuntimeConfig) -> Self {
        self.default_runtime_config = config;
        self
    }

    /// Build the runtime pool.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine cannot be created.
    pub fn build(self) -> Result<RuntimePool> {
        let engine = Engine::new(&self.engine_config)?;

        Ok(RuntimePool {
            engine: Arc::new(engine),
            modules: Mutex::new(HashMap::new()),
            max_modules: self.max_modules,
            default_config: self.default_runtime_config,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid WASM component.
    fn minimal_component() -> Vec<u8> {
        vec![
            0x00, 0x61, 0x73, 0x6d, // magic: \0asm
            0x0d, 0x00, 0x01, 0x00, // version: component model
        ]
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let pool = RuntimePool::new(10);
        assert!(pool.is_ok());
    }

    #[tokio::test]
    async fn test_pool_caching() {
        let pool = RuntimePool::new(10).expect("Failed to create pool");

        let digest = "sha256:test123";
        let component = minimal_component();

        // First call compiles and caches
        assert!(!pool.has_module(digest).await);
        let _runtime = pool
            .get_runtime_default(digest, &component)
            .await
            .expect("Failed to get runtime");
        assert!(pool.has_module(digest).await);

        // Second call uses cache
        let _runtime2 = pool
            .get_runtime_default(digest, &component)
            .await
            .expect("Failed to get runtime");

        assert_eq!(pool.cached_count().await, 1);
    }

    #[tokio::test]
    async fn test_pool_eviction() {
        let pool = RuntimePool::new(2).expect("Failed to create pool");
        let component = minimal_component();

        // Fill the cache
        pool.get_runtime_default("digest1", &component)
            .await
            .expect("Failed");
        pool.get_runtime_default("digest2", &component)
            .await
            .expect("Failed");
        assert_eq!(pool.cached_count().await, 2);

        // Adding a third should evict one
        pool.get_runtime_default("digest3", &component)
            .await
            .expect("Failed");
        assert_eq!(pool.cached_count().await, 2);
    }

    #[tokio::test]
    async fn test_pool_clear() {
        let pool = RuntimePool::new(10).expect("Failed to create pool");
        let component = minimal_component();

        pool.get_runtime_default("digest1", &component)
            .await
            .expect("Failed");
        pool.get_runtime_default("digest2", &component)
            .await
            .expect("Failed");
        assert_eq!(pool.cached_count().await, 2);

        pool.clear().await;
        assert_eq!(pool.cached_count().await, 0);
    }

    #[tokio::test]
    async fn test_pool_builder() {
        let pool = RuntimePoolBuilder::new()
            .max_modules(50)
            .with_simd()
            .with_fuel()
            .build()
            .expect("Failed to build pool");

        assert_eq!(pool.cached_count().await, 0);
    }

    #[tokio::test]
    async fn test_shared_engine() {
        let pool = RuntimePool::new(10).expect("Failed to create pool");
        let component = minimal_component();

        let runtime1 = pool
            .get_runtime_default("digest1", &component)
            .await
            .expect("Failed");
        let runtime2 = pool
            .get_runtime_default("digest2", &component)
            .await
            .expect("Failed");

        // Both runtimes share the same engine
        assert!(Arc::ptr_eq(&runtime1.engine(), &runtime2.engine()));
    }
}
