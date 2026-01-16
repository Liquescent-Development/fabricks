//! Daemon configuration types and loading logic.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use fabricks_common::models::common::Duration;

use crate::error::{DaemonError, Result};

/// System-wide configuration file path.
const SYSTEM_CONFIG_PATH: &str = "/etc/fabricksd/config.toml";

/// User configuration file name within home directory.
const USER_CONFIG_FILE: &str = ".fabricks/daemon.toml";

/// Daemon configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Daemon-level settings.
    #[serde(default)]
    pub daemon: DaemonSettings,

    /// API settings.
    #[serde(default)]
    pub api: ApiSettings,

    /// Runtime settings.
    #[serde(default)]
    pub runtime: RuntimeSettings,

    /// Resource limits.
    #[serde(default)]
    pub resources: ResourceSettings,

    /// Monitoring settings.
    #[serde(default)]
    pub monitoring: MonitoringSettings,

    /// Event bus settings.
    #[serde(default)]
    pub events: EventSettings,
}

/// Core daemon settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSettings {
    /// Unix socket path.
    #[serde(default = "default_socket_path")]
    pub socket: PathBuf,

    /// PID file path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_file: Option<PathBuf>,

    /// Log level: trace, debug, info, warn, error.
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Data directory for state and cache.
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
}

/// API-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSettings {
    /// API version prefix.
    #[serde(default = "default_api_version")]
    pub version: String,

    /// Whether authentication is required.
    #[serde(default)]
    pub auth_enabled: bool,

    /// API key (if `auth_enabled` is true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Request timeout.
    #[serde(default = "default_request_timeout")]
    pub timeout: Duration,
}

/// WASM runtime settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSettings {
    /// Default WASM engine (wasmtime).
    #[serde(default = "default_engine")]
    pub default_engine: String,

    /// Maximum number of cached modules.
    #[serde(default = "default_max_modules")]
    pub max_cached_modules: usize,
}

/// Global resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSettings {
    /// Maximum total services.
    #[serde(default = "default_max_services")]
    pub max_services: u32,

    /// Maximum replicas per service.
    #[serde(default = "default_max_replicas")]
    pub max_replicas_per_service: u32,
}

/// Monitoring and health check settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringSettings {
    /// Default health check interval.
    #[serde(default = "default_health_interval")]
    pub health_check_interval: Duration,

    /// Default health check timeout.
    #[serde(default = "default_health_timeout")]
    pub health_check_timeout: Duration,
}

/// Event bus settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSettings {
    /// Event channel buffer size.
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,

    /// Maximum event history to retain.
    #[serde(default = "default_history_size")]
    pub history_size: usize,
}

// Default value functions

fn default_socket_path() -> PathBuf {
    // Use user-level socket path by default.
    // System-wide deployments should use config file to override.
    dirs::home_dir().map_or_else(
        || PathBuf::from("/tmp/fabricks.sock"),
        |h| h.join(".fabricks/fabricks.sock"),
    )
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_data_dir() -> PathBuf {
    // Use user-level data directory by default.
    // System-wide deployments should use config file to override.
    dirs::data_local_dir().map_or_else(
        || {
            dirs::home_dir().map_or_else(
                || PathBuf::from("/tmp/fabricks"),
                |h| h.join(".local/share/fabricks"),
            )
        },
        |d| d.join("fabricks"),
    )
}

fn default_api_version() -> String {
    "v1".to_string()
}

fn default_request_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_engine() -> String {
    "wasmtime".to_string()
}

fn default_max_modules() -> usize {
    100
}

fn default_max_services() -> u32 {
    100
}

fn default_max_replicas() -> u32 {
    20
}

fn default_health_interval() -> Duration {
    Duration::from_secs(5)
}

fn default_health_timeout() -> Duration {
    Duration::from_secs(3)
}

fn default_buffer_size() -> usize {
    1000
}

fn default_history_size() -> usize {
    10000
}

// Default trait implementations

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            socket: default_socket_path(),
            pid_file: None,
            log_level: default_log_level(),
            data_dir: default_data_dir(),
        }
    }
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            version: default_api_version(),
            auth_enabled: false,
            api_key: None,
            timeout: default_request_timeout(),
        }
    }
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            default_engine: default_engine(),
            max_cached_modules: default_max_modules(),
        }
    }
}

impl Default for ResourceSettings {
    fn default() -> Self {
        Self {
            max_services: default_max_services(),
            max_replicas_per_service: default_max_replicas(),
        }
    }
}

impl Default for MonitoringSettings {
    fn default() -> Self {
        Self {
            health_check_interval: default_health_interval(),
            health_check_timeout: default_health_timeout(),
        }
    }
}

impl Default for EventSettings {
    fn default() -> Self {
        Self {
            buffer_size: default_buffer_size(),
            history_size: default_history_size(),
        }
    }
}

impl DaemonConfig {
    /// Load configuration from file or use defaults.
    ///
    /// Searches for configuration files in the following order:
    /// 1. System-wide: `/etc/fabricksd/config.toml`
    /// 2. User-specific: `~/.fabricks/daemon.toml`
    /// 3. Falls back to defaults if no file found
    ///
    /// # Errors
    ///
    /// Returns an error if a configuration file exists but cannot be read or parsed.
    pub fn load() -> Result<Self> {
        // Try system config first
        let system_path = Path::new(SYSTEM_CONFIG_PATH);
        if system_path.exists() {
            return Self::load_from(system_path);
        }

        // Try user config
        if let Some(home) = dirs::home_dir() {
            let user_path = home.join(USER_CONFIG_FILE);
            if user_path.exists() {
                return Self::load_from(&user_path);
            }
        }

        // Fall back to defaults
        Ok(Self::default())
    }

    /// Load configuration from a specific path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed as valid TOML.
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| DaemonError::ConfigLoadError {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DaemonConfig::default();
        assert_eq!(config.api.version, "v1");
        assert!(!config.api.auth_enabled);
        assert_eq!(config.resources.max_services, 100);
        assert_eq!(config.events.buffer_size, 1000);
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
            [daemon]
            log_level = "debug"
        "#;

        let config: DaemonConfig = toml::from_str(toml).expect("should parse");
        assert_eq!(config.daemon.log_level, "debug");
        // Other fields should use defaults
        assert_eq!(config.api.version, "v1");
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
            [daemon]
            socket = "/tmp/test.sock"
            log_level = "trace"
            data_dir = "/tmp/fabricks-data"

            [api]
            version = "v2"
            auth_enabled = true
            api_key = "secret"
            timeout = "60s"

            [runtime]
            default_engine = "wasmtime"
            max_cached_modules = 50

            [resources]
            max_services = 200
            max_replicas_per_service = 50

            [monitoring]
            health_check_interval = "10s"
            health_check_timeout = "5s"

            [events]
            buffer_size = 2000
            history_size = 20000
        "#;

        let config: DaemonConfig = toml::from_str(toml).expect("should parse");
        assert_eq!(config.daemon.socket, PathBuf::from("/tmp/test.sock"));
        assert_eq!(config.daemon.log_level, "trace");
        assert_eq!(config.api.version, "v2");
        assert!(config.api.auth_enabled);
        assert_eq!(config.api.api_key.as_deref(), Some("secret"));
        assert_eq!(config.api.timeout.as_secs(), 60);
        assert_eq!(config.resources.max_services, 200);
        assert_eq!(config.events.buffer_size, 2000);
    }

    #[test]
    fn test_load_nonexistent_defaults_to_default() {
        // Loading from a path that doesn't exist should work via load()
        // since it falls back to defaults
        let config = DaemonConfig::load().expect("should fall back to defaults");
        assert_eq!(config.api.version, "v1");
    }
}
