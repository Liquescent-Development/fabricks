//! Dependency resolution for services.
//!
//! Uses topological sorting to determine the correct startup order for services
//! based on their dependencies.

use std::collections::HashMap;

use petgraph::algo::toposort;
use petgraph::graph::DiGraph;

use crate::error::{DaemonError, Result};

use super::types::ServiceConfig;

/// Resolves the startup order for a set of services based on their dependencies.
///
/// Returns the service names in the order they should be started (dependencies first).
///
/// # Arguments
///
/// * `services` - Slice of service configurations to resolve
///
/// # Errors
///
/// Returns an error if:
/// - A circular dependency is detected
/// - A dependency references a service that doesn't exist
///
/// # Example
///
/// ```ignore
/// let configs = vec![
///     ServiceConfig { name: "api".into(), depends_on: vec!["db".into()], .. },
///     ServiceConfig { name: "db".into(), depends_on: vec![], .. },
/// ];
/// let order = resolve_startup_order(&configs)?;
/// assert_eq!(order, vec!["db", "api"]);
/// ```
pub fn resolve_startup_order(services: &[ServiceConfig]) -> Result<Vec<String>> {
    if services.is_empty() {
        return Ok(Vec::new());
    }

    let mut graph: DiGraph<String, ()> = DiGraph::new();
    let mut nodes: HashMap<String, petgraph::graph::NodeIndex> = HashMap::new();

    // Add all services as nodes
    for service in services {
        let node = graph.add_node(service.name.clone());
        nodes.insert(service.name.clone(), node);
    }

    // Add dependency edges
    // Edge direction: dependency -> dependent (so deps come first in toposort)
    for service in services {
        let dependent_node = nodes[&service.name];

        for dep_name in &service.depends_on {
            let dep_node = nodes.get(dep_name).ok_or_else(|| DaemonError::DependencyNotFound {
                service: service.name.clone(),
                dependency: dep_name.clone(),
            })?;

            // Edge from dependency to dependent
            graph.add_edge(*dep_node, dependent_node, ());
        }
    }

    // Perform topological sort
    match toposort(&graph, None) {
        Ok(order) => {
            let names: Vec<String> = order.into_iter().map(|n| graph[n].clone()).collect();
            Ok(names)
        }
        Err(_cycle) => Err(DaemonError::CircularDependency),
    }
}

/// Resolves the shutdown order for a set of services.
///
/// Returns the reverse of startup order (dependents first, then dependencies).
///
/// # Errors
///
/// Returns an error if dependency resolution fails.
pub fn resolve_shutdown_order(services: &[ServiceConfig]) -> Result<Vec<String>> {
    let mut order = resolve_startup_order(services)?;
    order.reverse();
    Ok(order)
}

/// Validates that all dependencies exist in the service set.
///
/// # Errors
///
/// Returns an error if any service references a non-existent dependency.
pub fn validate_dependencies(services: &[ServiceConfig]) -> Result<()> {
    let names: std::collections::HashSet<&str> =
        services.iter().map(|s| s.name.as_str()).collect();

    for service in services {
        for dep in &service.depends_on {
            if !names.contains(dep.as_str()) {
                return Err(DaemonError::DependencyNotFound {
                    service: service.name.clone(),
                    dependency: dep.clone(),
                });
            }
        }
    }

    Ok(())
}

/// Gets the direct dependencies of a service.
#[must_use]
pub fn get_dependencies<'a>(service_name: &str, services: &'a [ServiceConfig]) -> Vec<&'a str> {
    services
        .iter()
        .find(|s| s.name == service_name)
        .map(|s| s.depends_on.iter().map(String::as_str).collect())
        .unwrap_or_default()
}

/// Gets all services that depend on the given service (direct dependents).
#[must_use]
pub fn get_dependents<'a>(service_name: &str, services: &'a [ServiceConfig]) -> Vec<&'a str> {
    services
        .iter()
        .filter(|s| s.depends_on.iter().any(|d| d == service_name))
        .map(|s| s.name.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn make_config(name: &str, depends_on: Vec<&str>) -> ServiceConfig {
        ServiceConfig {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            service_type: Default::default(),
            wasm_path: PathBuf::from("/tmp/test.wasm"),
            wasm_digest: "sha256:test".to_string(),
            capabilities: Default::default(),
            environment: Default::default(),
            args: Vec::new(),
            resources: None,
            replicas: Default::default(),
            health_check: None,
            depends_on: depends_on.into_iter().map(String::from).collect(),
            networks: Vec::new(),
            mortar_project: None,
        }
    }

    #[test]
    fn test_empty_services() {
        let result = resolve_startup_order(&[]);
        assert!(result.is_ok());
        assert!(result.expect("should succeed").is_empty());
    }

    #[test]
    fn test_single_service() {
        let services = vec![make_config("api", vec![])];
        let order = resolve_startup_order(&services).expect("should resolve");
        assert_eq!(order, vec!["api"]);
    }

    #[test]
    fn test_simple_dependency() {
        let services = vec![
            make_config("api", vec!["db"]),
            make_config("db", vec![]),
        ];

        let order = resolve_startup_order(&services).expect("should resolve");

        // db must come before api
        let db_pos = order.iter().position(|n| n == "db").expect("db should exist");
        let api_pos = order.iter().position(|n| n == "api").expect("api should exist");
        assert!(db_pos < api_pos, "db should start before api");
    }

    #[test]
    fn test_chain_dependency() {
        let services = vec![
            make_config("frontend", vec!["api"]),
            make_config("api", vec!["db"]),
            make_config("db", vec![]),
        ];

        let order = resolve_startup_order(&services).expect("should resolve");

        let db_pos = order.iter().position(|n| n == "db").expect("db should exist");
        let api_pos = order.iter().position(|n| n == "api").expect("api should exist");
        let fe_pos = order.iter().position(|n| n == "frontend").expect("frontend should exist");

        assert!(db_pos < api_pos, "db should start before api");
        assert!(api_pos < fe_pos, "api should start before frontend");
    }

    #[test]
    fn test_multiple_dependencies() {
        let services = vec![
            make_config("api", vec!["db", "cache"]),
            make_config("db", vec![]),
            make_config("cache", vec![]),
        ];

        let order = resolve_startup_order(&services).expect("should resolve");

        let db_pos = order.iter().position(|n| n == "db").expect("db should exist");
        let cache_pos = order.iter().position(|n| n == "cache").expect("cache should exist");
        let api_pos = order.iter().position(|n| n == "api").expect("api should exist");

        assert!(db_pos < api_pos, "db should start before api");
        assert!(cache_pos < api_pos, "cache should start before api");
    }

    #[test]
    fn test_circular_dependency() {
        let services = vec![
            make_config("a", vec!["b"]),
            make_config("b", vec!["c"]),
            make_config("c", vec!["a"]),
        ];

        let result = resolve_startup_order(&services);
        assert!(matches!(result, Err(DaemonError::CircularDependency)));
    }

    #[test]
    fn test_self_dependency() {
        let services = vec![make_config("a", vec!["a"])];

        let result = resolve_startup_order(&services);
        assert!(matches!(result, Err(DaemonError::CircularDependency)));
    }

    #[test]
    fn test_missing_dependency() {
        let services = vec![make_config("api", vec!["db"])];

        let result = resolve_startup_order(&services);
        assert!(matches!(
            result,
            Err(DaemonError::DependencyNotFound { service, dependency })
            if service == "api" && dependency == "db"
        ));
    }

    #[test]
    fn test_shutdown_order() {
        let services = vec![
            make_config("api", vec!["db"]),
            make_config("db", vec![]),
        ];

        let startup = resolve_startup_order(&services).expect("should resolve");
        let shutdown = resolve_shutdown_order(&services).expect("should resolve");

        // Shutdown should be reverse of startup
        let mut expected_shutdown = startup.clone();
        expected_shutdown.reverse();
        assert_eq!(shutdown, expected_shutdown);
    }

    #[test]
    fn test_validate_dependencies_ok() {
        let services = vec![
            make_config("api", vec!["db"]),
            make_config("db", vec![]),
        ];

        assert!(validate_dependencies(&services).is_ok());
    }

    #[test]
    fn test_validate_dependencies_missing() {
        let services = vec![make_config("api", vec!["missing"])];

        let result = validate_dependencies(&services);
        assert!(matches!(
            result,
            Err(DaemonError::DependencyNotFound { dependency, .. }) if dependency == "missing"
        ));
    }

    #[test]
    fn test_get_dependencies() {
        let services = vec![
            make_config("api", vec!["db", "cache"]),
            make_config("db", vec![]),
            make_config("cache", vec![]),
        ];

        let deps = get_dependencies("api", &services);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"db"));
        assert!(deps.contains(&"cache"));

        let deps = get_dependencies("db", &services);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_get_dependents() {
        let services = vec![
            make_config("frontend", vec!["api"]),
            make_config("api", vec!["db"]),
            make_config("worker", vec!["db"]),
            make_config("db", vec![]),
        ];

        let dependents = get_dependents("db", &services);
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&"api"));
        assert!(dependents.contains(&"worker"));

        let dependents = get_dependents("api", &services);
        assert_eq!(dependents, vec!["frontend"]);

        let dependents = get_dependents("frontend", &services);
        assert!(dependents.is_empty());
    }
}
