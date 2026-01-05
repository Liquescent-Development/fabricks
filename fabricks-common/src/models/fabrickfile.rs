//! Fabrickfile data model.
//!
//! A `Fabrickfile` defines a single WASM module or service, specifying how to build
//! it, what capabilities it needs, what it exports/imports, and its default configuration.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::capability::Capabilities;
use super::common::{Labels, Resources};
use super::health_check::HealthCheck;

/// The current supported Fabrickfile format version.
pub const FABRICK_VERSION: &str = "1.0";

/// Service type - determines execution model and WASI interface.
///
/// This must be explicitly declared in the Fabrickfile. The daemon uses this
/// to determine how to instantiate and communicate with the WASM module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ServiceType {
    /// CLI command that runs to completion (`wasi:cli/run`).
    ///
    /// The daemon spawns the WASM, it runs, and exits. This is the default.
    #[default]
    Command,

    /// HTTP handler that processes requests (`wasi:http/incoming-handler`).
    ///
    /// The daemon binds ports and routes HTTP requests to the handler.
    /// The WASM works with structured HTTP request/response types.
    Http,

    /// TCP socket server for raw protocols (`wasi:sockets`).
    ///
    /// For services like Redis or Postgres that use custom wire protocols.
    /// The daemon forwards raw TCP connections to the WASM.
    Tcp,
}

impl ServiceType {
    /// Returns true if this is an HTTP handler service.
    #[must_use]
    pub const fn is_http(&self) -> bool {
        matches!(self, Self::Http)
    }

    /// Returns true if this is a command service.
    #[must_use]
    pub const fn is_command(&self) -> bool {
        matches!(self, Self::Command)
    }

    /// Returns true if this is a TCP socket service.
    #[must_use]
    pub const fn is_tcp(&self) -> bool {
        matches!(self, Self::Tcp)
    }
}

impl std::fmt::Display for ServiceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command => write!(f, "command"),
            Self::Http => write!(f, "http"),
            Self::Tcp => write!(f, "tcp"),
        }
    }
}

/// A complete Fabrickfile definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fabrickfile {
    /// Fabrickfile format version (required).
    pub fabrick_version: String,

    /// Metadata about the fabrick (required).
    pub info: Info,

    /// Base image or source language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<From>,

    /// Source code configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,

    /// Runtime configuration for interpreted languages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Runtime>,

    /// Build configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<Build>,

    /// Exported functions or interfaces (Component Model).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exports: Option<Exports>,

    /// Imported modules (Component Model).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imports: Option<HashMap<String, Import>>,

    /// Capability definitions (required).
    #[serde(default)]
    pub capabilities: Capabilities,

    /// Static files to include.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,

    /// Default configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Config>,

    /// Health check configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheck>,

    /// Security hardening options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<Security>,

    /// Arbitrary labels for organization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    /// Validation rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validate: Option<Validate>,
}

/// Metadata about the fabrick.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Info {
    /// Name of the fabrick (lowercase, hyphens allowed).
    pub name: String,

    /// Semantic version.
    pub version: String,

    /// Service type - determines execution model and WASI interface.
    ///
    /// Defaults to `Command` for backward compatibility.
    #[serde(default, rename = "type")]
    pub service_type: ServiceType,

    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Author information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,

    /// License identifier (SPDX format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Homepage URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    /// Repository URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    /// Documentation URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,

    /// Keywords for discoverability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
}

/// Base image or source language specification.
///
/// Only one of `source`, `image`, or `path` should be specified.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct From {
    /// Build from scratch using a language runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceLanguage>,

    /// Build on top of another fabrick image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Build on top of a local fabrick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Supported source languages.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceLanguage {
    /// Rust programming language.
    Rust,
    /// Go programming language.
    Go,
    /// JavaScript (requires runtime).
    Javascript,
    /// Python (requires runtime).
    Python,
    /// C# (requires runtime).
    Csharp,
}

/// Source code configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Source {
    /// Path to source code (relative to Fabrickfile).
    pub path: String,

    /// Files to include in build context (glob patterns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,

    /// Files to exclude from build context (glob patterns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
}

/// Runtime configuration for interpreted languages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Runtime {
    /// WASM runtime image reference.
    pub image: String,

    /// Runtime-specific configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, toml::Value>>,
}

/// Build configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Build {
    /// Build command to compile to WASM.
    pub command: String,

    /// Working directory for build (relative to Fabrickfile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,

    /// Output WASM file path (relative to workdir).
    pub output: String,

    /// Files to watch for rebuild in dev mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<Vec<String>>,

    /// Environment variables for build process.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,

    /// Commands to run before main build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_build: Option<Vec<String>>,

    /// Commands to run after main build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_build: Option<Vec<String>>,
}

/// Exported functions or interfaces.
///
/// Can be either a simple list of function names or interface definitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Exports {
    /// Simple list of exported function names.
    Functions(Vec<String>),
    /// Interface exports with version information.
    Interfaces {
        /// Map of interface name to version info.
        interface: HashMap<String, InterfaceVersion>,
    },
}

/// Interface version specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InterfaceVersion {
    /// Interface version.
    pub version: String,
}

/// Import specification for a module dependency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Import {
    /// Simple image reference.
    Image(String),
    /// Detailed import specification.
    Detailed(DetailedImport),
}

/// Detailed import specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetailedImport {
    /// Image reference from registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Path to local fabrick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Specific interface version to import.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<String>,
}

/// Default configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Default port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// Default timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,

    /// Default log level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,

    /// Environment variables with defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,

    /// Resource defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,
}

/// Security hardening options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Security {
    /// Run as specific user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Deny all access by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_by_default: Option<bool>,

    /// Make root filesystem read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only_root: Option<bool>,

    /// Capabilities to drop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_capabilities: Option<Vec<String>>,
}

/// Validation rules to run during build.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Validate {
    /// Verify exported functions exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_exports: Option<bool>,

    /// Verify imported modules are available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_imports: Option<bool>,

    /// Check for known vulnerabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_vulnerabilities: Option<bool>,
}

impl Fabrickfile {
    /// Returns the full image reference for this fabrick.
    #[must_use]
    pub fn image_reference(&self, registry: Option<&str>) -> String {
        match registry {
            Some(reg) => format!("{}/{}:{}", reg, self.info.name, self.info.version),
            None => format!("{}:{}", self.info.name, self.info.version),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_fabrickfile_deserialize() -> Result<(), toml::de::Error> {
        let toml = r#"
            fabrick_version = "1.0"

            [info]
            name = "my-service"
            version = "1.0.0"

            [from]
            source = "rust"

            [source]
            path = "."

            [build]
            command = "cargo build --target wasm32-wasi --release"
            output = "target/wasm32-wasi/release/my_service.wasm"

            [capabilities.network]
            listen = [8080]
        "#;

        let fabrickfile: Fabrickfile = toml::from_str(toml)?;
        assert_eq!(fabrickfile.fabrick_version, "1.0");
        assert_eq!(fabrickfile.info.name, "my-service");
        assert_eq!(fabrickfile.info.version, "1.0.0");
        assert!(fabrickfile.capabilities.can_listen(8080));
        Ok(())
    }

    #[test]
    fn test_image_reference() {
        let fabrickfile = Fabrickfile {
            fabrick_version: "1.0".to_string(),
            info: Info {
                name: "my-service".to_string(),
                version: "2.1.0".to_string(),
                service_type: ServiceType::default(),
                description: None,
                authors: None,
                license: None,
                homepage: None,
                repository: None,
                documentation: None,
                keywords: None,
            },
            from: None,
            source: None,
            runtime: None,
            build: None,
            exports: None,
            imports: None,
            capabilities: Capabilities::default(),
            files: None,
            config: None,
            health_check: None,
            security: None,
            labels: None,
            validate: None,
        };

        assert_eq!(fabrickfile.image_reference(None), "my-service:2.1.0");
        assert_eq!(
            fabrickfile.image_reference(Some("ghcr.io/acme")),
            "ghcr.io/acme/my-service:2.1.0"
        );
    }
}
