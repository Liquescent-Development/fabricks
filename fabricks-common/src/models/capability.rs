//! Capability types for security and resource access control.

use serde::{Deserialize, Serialize};

/// Capabilities that define what resources a module can access.
///
/// This follows a deny-by-default model - modules can only access
/// resources explicitly granted in this section.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Capabilities {
    /// Environment variables the module can read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<String>>,

    /// Network capabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkCapabilities>,

    /// Filesystem capabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filesystem: Option<FilesystemCapabilities>,

    /// WASM-specific feature flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm: Option<WasmCapabilities>,
}

/// Network access capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NetworkCapabilities {
    /// Ports this module can listen on.
    ///
    /// Note: In reality, the daemon binds these ports and proxies
    /// requests to the WASM module's `wasi:http/handler` implementation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen: Option<Vec<u16>>,

    /// Hosts and ports this module can connect to.
    ///
    /// Format: `"host:port"` (e.g., `"postgres:5432"`, `"api.stripe.com:443"`)
    ///
    /// Note: The daemon validates these connections and may proxy them
    /// or provide `wasi-sockets` primitives depending on the destination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect: Option<Vec<String>>,

    /// Allow all outbound connections.
    ///
    /// **Warning:** This is not recommended for security reasons.
    /// Prefer explicit `connect` entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_all_outbound: Option<bool>,
}

/// Filesystem access capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FilesystemCapabilities {
    /// Paths with read-only access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read: Option<Vec<String>>,

    /// Paths with write-only access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write: Option<Vec<String>>,

    /// Paths with read and write access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_write: Option<Vec<String>>,
}

/// WASM-specific feature flags.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WasmCapabilities {
    /// Enable WASM SIMD instructions for performance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simd: Option<bool>,

    /// Enable WASM threads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<bool>,

    /// Enable bulk memory operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bulk_memory: Option<bool>,
}

impl Capabilities {
    /// Returns true if the module can access the given environment variable.
    #[must_use]
    pub fn can_access_env(&self, name: &str) -> bool {
        self.env
            .as_ref()
            .is_some_and(|vars| vars.iter().any(|v| v == name))
    }

    /// Returns true if the module can listen on the given port.
    #[must_use]
    pub fn can_listen(&self, port: u16) -> bool {
        self.network
            .as_ref()
            .and_then(|n| n.listen.as_ref())
            .is_some_and(|ports| ports.contains(&port))
    }

    /// Returns true if the module can connect to the given host:port.
    #[must_use]
    pub fn can_connect(&self, host_port: &str) -> bool {
        self.network.as_ref().is_some_and(|n| {
            n.allow_all_outbound.unwrap_or(false)
                || n.connect
                    .as_ref()
                    .is_some_and(|connects| connects.iter().any(|c| c == host_port))
        })
    }

    /// Returns true if the module can read from the given path.
    #[must_use]
    pub fn can_read(&self, path: &str) -> bool {
        self.filesystem.as_ref().is_some_and(|fs| {
            let in_read = fs
                .read
                .as_ref()
                .is_some_and(|paths| paths.iter().any(|p| path.starts_with(p)));
            let in_read_write = fs
                .read_write
                .as_ref()
                .is_some_and(|paths| paths.iter().any(|p| path.starts_with(p)));
            in_read || in_read_write
        })
    }

    /// Returns true if the module can write to the given path.
    #[must_use]
    pub fn can_write(&self, path: &str) -> bool {
        self.filesystem.as_ref().is_some_and(|fs| {
            let in_write = fs
                .write
                .as_ref()
                .is_some_and(|paths| paths.iter().any(|p| path.starts_with(p)));
            let in_read_write = fs
                .read_write
                .as_ref()
                .is_some_and(|paths| paths.iter().any(|p| path.starts_with(p)));
            in_write || in_read_write
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_capability() {
        let caps = Capabilities {
            env: Some(vec!["DATABASE_URL".to_string(), "API_KEY".to_string()]),
            ..Default::default()
        };

        assert!(caps.can_access_env("DATABASE_URL"));
        assert!(caps.can_access_env("API_KEY"));
        assert!(!caps.can_access_env("SECRET"));
    }

    #[test]
    fn test_network_listen_capability() {
        let caps = Capabilities {
            network: Some(NetworkCapabilities {
                listen: Some(vec![8080, 9090]),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert!(caps.can_listen(8080));
        assert!(caps.can_listen(9090));
        assert!(!caps.can_listen(3000));
    }

    #[test]
    fn test_network_connect_capability() {
        let caps = Capabilities {
            network: Some(NetworkCapabilities {
                connect: Some(vec!["postgres:5432".to_string(), "redis:6379".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert!(caps.can_connect("postgres:5432"));
        assert!(caps.can_connect("redis:6379"));
        assert!(!caps.can_connect("mysql:3306"));
    }

    #[test]
    fn test_network_allow_all_outbound() {
        let caps = Capabilities {
            network: Some(NetworkCapabilities {
                allow_all_outbound: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert!(caps.can_connect("any:1234"));
        assert!(caps.can_connect("postgres:5432"));
    }

    #[test]
    fn test_filesystem_capability() {
        let caps = Capabilities {
            filesystem: Some(FilesystemCapabilities {
                read: Some(vec!["./config".to_string()]),
                write: Some(vec!["./logs".to_string()]),
                read_write: Some(vec!["./data".to_string()]),
            }),
            ..Default::default()
        };

        assert!(caps.can_read("./config/app.toml"));
        assert!(!caps.can_write("./config/app.toml"));

        assert!(!caps.can_read("./logs/app.log"));
        assert!(caps.can_write("./logs/app.log"));

        assert!(caps.can_read("./data/file.db"));
        assert!(caps.can_write("./data/file.db"));

        assert!(!caps.can_read("./other/file.txt"));
        assert!(!caps.can_write("./other/file.txt"));
    }
}
