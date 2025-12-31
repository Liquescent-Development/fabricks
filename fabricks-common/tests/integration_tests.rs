//! Integration tests for parsing configuration files.
//!
//! These tests verify that the parser can correctly handle real-world configuration files.

use std::path::Path;

use fabricks_common::parser::{parse_fabrickfile, parse_fabrickfile_str, parse_mortar_file_str};

/// Test parsing the hello-world example Fabrickfile.
#[test]
fn test_parse_hello_world_fabrickfile() {
    let path = Path::new("../examples/hello-world/Fabrickfile");
    let result = parse_fabrickfile(path);

    assert!(
        result.is_ok(),
        "Failed to parse hello-world Fabrickfile: {result:?}"
    );
    let fabrickfile = result.unwrap_or_else(|e| panic!("unexpected error: {e}"));

    assert_eq!(fabrickfile.fabrick_version, "1.0");
    assert_eq!(fabrickfile.info.name, "hello-world");
    assert_eq!(fabrickfile.info.version, "1.0.0");
    assert!(fabrickfile.capabilities.can_listen(8080));
}

/// Test parsing a more complex Fabrickfile with all sections.
#[test]
fn test_parse_complex_fabrickfile() {
    let toml = r#"
fabrick_version = "1.0"

[info]
name = "payment-api"
version = "2.1.0"
description = "Payment processing service"
authors = ["Team <team@example.com>"]
license = "MIT"
homepage = "https://example.com"
repository = "https://github.com/example/payment"
documentation = "https://docs.example.com"
keywords = ["payment", "api"]

[from]
source = "rust"

[source]
path = "./src"
include = ["**/*.rs", "Cargo.toml"]
exclude = ["**/tests/**"]

[build]
command = "cargo build --target wasm32-wasi --release"
output = "target/wasm32-wasi/release/payment.wasm"
workdir = "."
watch = ["src/**/*.rs", "Cargo.toml"]

[exports]
interface = { "payment:api" = { version = "1.0.0" } }

[capabilities]
env = ["API_KEY", "LOG_LEVEL"]

[capabilities.network]
listen = [8080, 8443]
connect = ["stripe.com:443", "postgres:5432"]

[capabilities.filesystem]
read = ["/config"]
write = ["/tmp"]

[capabilities.wasm]
threads = true
max_memory = "512Mi"

[config]
port = 8080
timeout = 30
log_level = "info"

[config.resources]
memory = "256Mi"
cpu = 0.5

[health_check.http]
path = "/health"
port = 8080
interval = "30s"
timeout = "5s"
retries = 3
success_threshold = 1

[security]
user = "app"
deny_by_default = true
read_only_root = true
drop_capabilities = ["NET_RAW"]

[validate]
check_exports = true
check_imports = true
    "#;

    let result = parse_fabrickfile_str(toml);
    assert!(
        result.is_ok(),
        "Failed to parse complex Fabrickfile: {result:?}"
    );

    let fabrickfile = result.unwrap_or_else(|e| panic!("unexpected error: {e}"));
    assert_eq!(fabrickfile.info.name, "payment-api");
    assert_eq!(fabrickfile.info.version, "2.1.0");
    assert!(fabrickfile.capabilities.can_listen(8080));
    assert!(fabrickfile.capabilities.can_listen(8443));
    assert!(fabrickfile.capabilities.can_connect("stripe.com:443"));
    assert!(fabrickfile.capabilities.can_access_env("API_KEY"));
}

/// Test parsing a mortar file with multiple services.
#[test]
fn test_parse_mortar_with_services() {
    let toml = r#"
mortar_version = "1.0"

[project]
name = "my-app"
version = "1.0.0"
description = "Test application"

[network.internal]
internal = true
description = "Internal network"

[network.public]
ingress = "0.0.0.0/0"
egress = ["internal"]

[service.api]
build = "./api"
networks = ["public", "internal"]
ports = ["8080:8080"]
depends_on = ["db"]

[service.api.resources]
memory = "256Mi"
cpu = 0.5

[service.api.health_check.http]
path = "/health"
interval = "30s"

[service.db]
image = "wasm://postgres:latest"
networks = ["internal"]

[service.db.volumes]
data = "/var/lib/postgresql/data"

[service.db.resources]
memory = "512Mi"
cpu = 1.0

[volume.data]
size = "10Gi"
    "#;

    let result = parse_mortar_file_str(toml);
    assert!(result.is_ok(), "Failed to parse mortar file: {result:?}");

    let mortar = result.unwrap_or_else(|e| panic!("unexpected error: {e}"));
    assert_eq!(mortar.project.name, "my-app");
    assert_eq!(mortar.service.len(), 2);
    assert!(mortar.service.contains_key("api"));
    assert!(mortar.service.contains_key("db"));

    let network = mortar.network.as_ref().map(|n| n.len()).unwrap_or(0);
    assert_eq!(network, 2);
}

/// Test that validation catches missing required fields.
#[test]
fn test_validation_missing_service_source() {
    let toml = r#"
mortar_version = "1.0"

[project]
name = "test-app"

[network.internal]
internal = true

[service.api]
networks = ["internal"]
    "#;

    let result = parse_mortar_file_str(toml);
    assert!(
        result.is_err(),
        "Should fail validation - service missing build/image"
    );
}

/// Test that validation catches invalid service references.
#[test]
fn test_validation_invalid_network_reference() {
    let toml = r#"
mortar_version = "1.0"

[project]
name = "test-app"

[network.internal]
internal = true

[service.api]
build = "./api"
networks = ["nonexistent"]
    "#;

    let result = parse_mortar_file_str(toml);
    assert!(
        result.is_err(),
        "Should fail validation - network 'nonexistent' not found"
    );
}

/// Test that validation catches circular dependencies.
#[test]
fn test_validation_circular_dependency() {
    let toml = r#"
mortar_version = "1.0"

[project]
name = "test-app"

[network.internal]
internal = true

[service.a]
build = "./a"
networks = ["internal"]
depends_on = ["b"]

[service.b]
build = "./b"
networks = ["internal"]
depends_on = ["a"]
    "#;

    let result = parse_mortar_file_str(toml);
    assert!(
        result.is_err(),
        "Should fail validation - circular dependency detected"
    );
}
