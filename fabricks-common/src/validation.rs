//! Validation logic for Fabrickfile and `MortarFile`.

use std::collections::HashSet;

use crate::error::ValidationError;
use crate::models::fabrickfile::{FABRICK_VERSION, Fabrickfile, From};
use crate::models::mortar::{MORTAR_VERSION, MortarFile, Service};

/// Trait for types that can be validated.
pub trait Validate {
    /// Validates this instance, returning an error if validation fails.
    ///
    /// # Errors
    ///
    /// Returns a `ValidationError` if validation fails.
    fn validate(&self) -> Result<(), ValidationError>;
}

/// Validates a name follows the required pattern: `[a-z0-9-]+`
fn validate_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::InvalidName {
            name: name.to_string(),
        });
    }

    let valid = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');

    if !valid {
        return Err(ValidationError::InvalidName {
            name: name.to_string(),
        });
    }

    // Name shouldn't start or end with hyphen
    if name.starts_with('-') || name.ends_with('-') {
        return Err(ValidationError::InvalidName {
            name: name.to_string(),
        });
    }

    Ok(())
}

/// Validates a semantic version string.
fn validate_semver(version: &str) -> Result<(), ValidationError> {
    semver::Version::parse(version).map_err(|_| ValidationError::InvalidVersion {
        version: version.to_string(),
    })?;
    Ok(())
}

/// Validates a port number is in valid range.
fn validate_port(port: u32) -> Result<(), ValidationError> {
    if port == 0 || port > 65535 {
        return Err(ValidationError::InvalidPort { port });
    }
    Ok(())
}

/// Validates a host:port string.
fn validate_host_port(value: &str) -> Result<(), ValidationError> {
    let parts: Vec<&str> = value.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(ValidationError::InvalidHostPort {
            value: value.to_string(),
            reason: "expected format 'host:port'".to_string(),
        });
    }

    let port_str = parts[0];
    let port: u32 = port_str
        .parse()
        .map_err(|_| ValidationError::InvalidHostPort {
            value: value.to_string(),
            reason: format!("invalid port number: {port_str}"),
        })?;

    validate_port(port).map_err(|_| ValidationError::InvalidHostPort {
        value: value.to_string(),
        reason: format!("port {port} out of range (1-65535)"),
    })?;

    Ok(())
}

impl Validate for Fabrickfile {
    fn validate(&self) -> Result<(), ValidationError> {
        let mut errors = Vec::new();

        // Check version
        if self.fabrick_version != FABRICK_VERSION {
            errors.push(ValidationError::UnsupportedVersion {
                version: self.fabrick_version.clone(),
                expected: FABRICK_VERSION.to_string(),
            });
        }

        // Validate info
        if let Err(e) = validate_name(&self.info.name) {
            errors.push(e);
        }

        if let Err(e) = validate_semver(&self.info.version) {
            errors.push(e);
        }

        // Validate URLs if present
        if let Some(ref url) = self.info.homepage
            && let Err(e) = validate_url(url)
        {
            errors.push(e);
        }
        if let Some(ref url) = self.info.repository
            && let Err(e) = validate_url(url)
        {
            errors.push(e);
        }
        if let Some(ref url) = self.info.documentation
            && let Err(e) = validate_url(url)
        {
            errors.push(e);
        }

        // Validate [from] - only one option should be specified
        if let Some(ref from) = self.from
            && let Err(e) = validate_from(from)
        {
            errors.push(e);
        }

        // Validate capabilities
        if let Some(ref network) = self.capabilities.network {
            if let Some(ref ports) = network.listen {
                for port in ports {
                    if let Err(e) = validate_port(u32::from(*port)) {
                        errors.push(e);
                    }
                }
            }
            if let Some(ref connects) = network.connect {
                for connect in connects {
                    if let Err(e) = validate_host_port(connect) {
                        errors.push(e);
                    }
                }
            }
        }

        // Return errors
        if errors.is_empty() {
            Ok(())
        } else if errors.len() == 1 {
            Err(errors.remove(0))
        } else {
            Err(ValidationError::Multiple(errors))
        }
    }
}

fn validate_url(url: &str) -> Result<(), ValidationError> {
    url::Url::parse(url).map_err(|e| ValidationError::InvalidUrl {
        url: url.to_string(),
        reason: e.to_string(),
    })?;
    Ok(())
}

fn validate_from(from: &From) -> Result<(), ValidationError> {
    let specified = [
        from.source.is_some(),
        from.image.is_some(),
        from.path.is_some(),
    ];
    let count = specified.iter().filter(|&&x| x).count();

    if count > 1 {
        return Err(ValidationError::MutuallyExclusive {
            option1: "from.source/from.image/from.path".to_string(),
            option2: "(only one can be specified)".to_string(),
        });
    }

    Ok(())
}

impl Validate for MortarFile {
    fn validate(&self) -> Result<(), ValidationError> {
        let mut errors = Vec::new();

        // Check version
        if self.mortar_version != MORTAR_VERSION {
            errors.push(ValidationError::UnsupportedVersion {
                version: self.mortar_version.clone(),
                expected: MORTAR_VERSION.to_string(),
            });
        }

        // Validate project name
        if let Err(e) = validate_name(&self.project.name) {
            errors.push(e);
        }

        // Validate project version if present
        if let Some(ref version) = self.project.version
            && let Err(e) = validate_semver(version)
        {
            errors.push(e);
        }

        // Collect known network names
        let network_names: HashSet<&str> = self
            .network
            .as_ref()
            .map(|n| n.keys().map(String::as_str).collect())
            .unwrap_or_default();

        // Collect known volume names
        let volume_names: HashSet<&str> = self
            .volume
            .as_ref()
            .map(|v| v.keys().map(String::as_str).collect())
            .unwrap_or_default();

        // Collect service names for dependency validation
        let service_names: HashSet<&str> = self.service.keys().map(String::as_str).collect();

        // Validate each service
        for (name, service) in &self.service {
            if let Err(e) =
                validate_service(name, service, &network_names, &volume_names, &service_names)
            {
                match e {
                    ValidationError::Multiple(mut sub_errors) => errors.append(&mut sub_errors),
                    e => errors.push(e),
                }
            }
        }

        // Check for circular dependencies
        if let Err(e) = check_circular_dependencies(&self.service) {
            errors.push(e);
        }

        // Return errors
        if errors.is_empty() {
            Ok(())
        } else if errors.len() == 1 {
            Err(errors.remove(0))
        } else {
            Err(ValidationError::Multiple(errors))
        }
    }
}

fn validate_service(
    name: &str,
    service: &Service,
    network_names: &HashSet<&str>,
    volume_names: &HashSet<&str>,
    service_names: &HashSet<&str>,
) -> Result<(), ValidationError> {
    let mut errors = Vec::new();

    // Service must have either build or image (mutually exclusive)
    match (&service.build, &service.image) {
        (None, None) => {
            errors.push(ValidationError::MissingField {
                field: format!("service.{name}.build or service.{name}.image"),
            });
        }
        (Some(_), Some(_)) => {
            errors.push(ValidationError::MutuallyExclusive {
                option1: format!("service.{name}.build"),
                option2: format!("service.{name}.image"),
            });
        }
        _ => {}
    }

    // Validate networks exist
    for network in &service.networks {
        if !network_names.contains(network.as_str()) {
            errors.push(ValidationError::NotFound {
                entity_type: "network".to_string(),
                name: network.clone(),
            });
        }
    }

    // Validate volumes exist
    if let Some(ref volumes) = service.volumes {
        for volume_name in volumes.keys() {
            if !volume_names.contains(volume_name.as_str()) {
                errors.push(ValidationError::NotFound {
                    entity_type: "volume".to_string(),
                    name: volume_name.clone(),
                });
            }
        }
    }

    // Validate depends_on references exist
    if let Some(ref deps) = service.depends_on {
        for dep in deps {
            if !service_names.contains(dep.as_str()) {
                errors.push(ValidationError::NotFound {
                    entity_type: "service".to_string(),
                    name: dep.clone(),
                });
            }
        }
    }

    // Return errors
    if errors.is_empty() {
        Ok(())
    } else if errors.len() == 1 {
        Err(errors.remove(0))
    } else {
        Err(ValidationError::Multiple(errors))
    }
}

fn check_circular_dependencies(
    services: &std::collections::HashMap<String, Service>,
) -> Result<(), ValidationError> {
    // Build adjacency list
    let mut graph: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();

    for (name, service) in services {
        let deps: Vec<&str> = service
            .depends_on
            .as_ref()
            .map(|d| d.iter().map(String::as_str).collect())
            .unwrap_or_default();
        graph.insert(name.as_str(), deps);
    }

    // DFS to detect cycles
    let mut visited: HashSet<&str> = HashSet::new();
    let mut rec_stack: HashSet<&str> = HashSet::new();

    for node in graph.keys() {
        if !visited.contains(node)
            && let Some(cycle) = dfs_detect_cycle(node, &graph, &mut visited, &mut rec_stack)
        {
            return Err(ValidationError::CircularDependency { cycle });
        }
    }

    Ok(())
}

fn dfs_detect_cycle<'a>(
    node: &'a str,
    graph: &std::collections::HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    rec_stack: &mut HashSet<&'a str>,
) -> Option<String> {
    visited.insert(node);
    rec_stack.insert(node);

    if let Some(neighbors) = graph.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                if let Some(cycle) = dfs_detect_cycle(neighbor, graph, visited, rec_stack) {
                    return Some(cycle);
                }
            } else if rec_stack.contains(neighbor) {
                return Some(format!("{node} -> {neighbor}"));
            }
        }
    }

    rec_stack.remove(node);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_name() {
        assert!(validate_name("my-service").is_ok());
        assert!(validate_name("api").is_ok());
        assert!(validate_name("service-123").is_ok());
    }

    #[test]
    fn test_invalid_name() {
        assert!(validate_name("").is_err());
        assert!(validate_name("My-Service").is_err()); // uppercase
        assert!(validate_name("my_service").is_err()); // underscore
        assert!(validate_name("-service").is_err()); // starts with hyphen
        assert!(validate_name("service-").is_err()); // ends with hyphen
    }

    #[test]
    fn test_valid_semver() {
        assert!(validate_semver("1.0.0").is_ok());
        assert!(validate_semver("2.1.3").is_ok());
        assert!(validate_semver("0.1.0-alpha").is_ok());
    }

    #[test]
    fn test_invalid_semver() {
        assert!(validate_semver("1.0").is_err());
        assert!(validate_semver("v1.0.0").is_err());
        assert!(validate_semver("1").is_err());
    }

    #[test]
    fn test_valid_port() {
        assert!(validate_port(80).is_ok());
        assert!(validate_port(8080).is_ok());
        assert!(validate_port(65535).is_ok());
    }

    #[test]
    fn test_invalid_port() {
        assert!(validate_port(0).is_err());
        assert!(validate_port(65536).is_err());
    }

    #[test]
    fn test_valid_host_port() {
        assert!(validate_host_port("postgres:5432").is_ok());
        assert!(validate_host_port("api.example.com:443").is_ok());
        assert!(validate_host_port("localhost:8080").is_ok());
    }

    #[test]
    fn test_invalid_host_port() {
        assert!(validate_host_port("postgres").is_err()); // no port
        assert!(validate_host_port("postgres:abc").is_err()); // invalid port
        assert!(validate_host_port("postgres:0").is_err()); // port 0
    }

    #[test]
    fn test_fabrickfile_validation() {
        let fabrickfile = Fabrickfile {
            fabrick_version: "1.0".to_string(),
            info: crate::models::fabrickfile::Info {
                name: "my-service".to_string(),
                version: "1.0.0".to_string(),
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
            capabilities: crate::models::Capabilities::default(),
            files: None,
            config: None,
            health_check: None,
            security: None,
            labels: None,
            validate: None,
        };

        assert!(fabrickfile.validate().is_ok());
    }

    #[test]
    fn test_fabrickfile_invalid_name() {
        let fabrickfile = Fabrickfile {
            fabrick_version: "1.0".to_string(),
            info: crate::models::fabrickfile::Info {
                name: "My_Service".to_string(), // Invalid
                version: "1.0.0".to_string(),
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
            capabilities: crate::models::Capabilities::default(),
            files: None,
            config: None,
            health_check: None,
            security: None,
            labels: None,
            validate: None,
        };

        assert!(fabrickfile.validate().is_err());
    }
}
