//! Test helper utilities for e2e tests.

use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;
use tokio::sync::RwLock;

use fabricksd::config::DaemonConfig;
use fabricksd::state::AppState;

/// Test environment for e2e tests.
///
/// Provides an isolated daemon state with temporary directories
/// for database and socket storage.
pub struct TestEnv {
    /// Application state for the test daemon.
    pub state: AppState,

    /// Temporary directory (kept alive for the test duration).
    _temp_dir: TempDir,
}

impl TestEnv {
    /// Creates a new test environment.
    ///
    /// # Errors
    ///
    /// Returns an error if the test environment cannot be created.
    pub async fn new() -> anyhow::Result<Self> {
        let temp_dir = TempDir::new()?;

        let mut config = DaemonConfig::default();
        config.daemon.data_dir = temp_dir.path().join("data");
        config.daemon.socket = temp_dir.path().join("fabricks.sock");

        let state = AppState::new(config)?;
        state.initialize().await?;

        Ok(Self {
            state,
            _temp_dir: temp_dir,
        })
    }

    /// Gets a reference to the service manager.
    pub fn service_manager(&self) -> &Arc<RwLock<fabricksd::service::ServiceManager>> {
        &self.state.service_manager
    }
}

/// Creates a minimal valid WASM component for testing.
///
/// This is the smallest valid component model binary - an empty component.
#[must_use]
pub fn minimal_wasm_component() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic: \0asm
        0x0d, 0x00, 0x01, 0x00, // version: component model (0x0d = 13)
    ]
}

/// Creates a temporary WASM file with the given bytes.
///
/// # Errors
///
/// Returns an error if the file cannot be created.
pub fn create_temp_wasm(bytes: &[u8]) -> anyhow::Result<(TempDir, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let wasm_path = temp_dir.path().join("test.wasm");
    std::fs::write(&wasm_path, bytes)?;
    Ok((temp_dir, wasm_path))
}

/// Creates a basic service configuration for testing.
#[must_use]
pub fn test_service_config(
    name: &str,
    wasm_path: PathBuf,
    digest: &str,
) -> fabricksd::service::ServiceConfig {
    fabricksd::service::ServiceConfig::new(
        name.to_string(),
        "1.0.0".to_string(),
        wasm_path,
        digest.to_string(),
    )
}
