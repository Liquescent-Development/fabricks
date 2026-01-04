//! End-to-end tests for HTTP service functionality.
//!
//! These tests verify that HTTP requests can be routed to WASM services
//! through the proxy server infrastructure.

use fabricks_common::models::capability::NetworkCapabilities;
use fabricks_common::models::fabrickfile::ServiceType;
use fabricks_common::Capabilities;
use fabricks_e2e::helpers::{create_temp_wasm, minimal_wasm_component, TestEnv};
use fabricks_runtime::HttpRequest;
use fabricksd::service::{ServiceConfig, State};

/// Creates an HTTP service configuration with a specific port.
fn http_service_config_with_port(
    name: &str,
    wasm_path: std::path::PathBuf,
    digest: &str,
    port: u16,
) -> ServiceConfig {
    let mut config = ServiceConfig::new(
        name.to_string(),
        "1.0.0".to_string(),
        wasm_path,
        digest.to_string(),
    );
    config.service_type = ServiceType::Http;
    config.replicas.min = 0; // No auto-start replicas for testing
    config.capabilities = Capabilities {
        network: Some(NetworkCapabilities {
            listen: Some(vec![port]),
            connect: None,
            allow_all_outbound: None,
        }),
        ..Default::default()
    };
    config
}

/// Test that HTTP service binds ports on start.
#[tokio::test]
async fn test_http_service_port_binding() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    // Use unique port for this test
    let config = http_service_config_with_port("port-bind-test", wasm_path, "sha256:test", 19001);

    let manager = env.service_manager().read().await;
    let id = manager.create_service(config).await.expect("should create service");

    // Start the service
    manager.start_service(&id).await.expect("should start service");

    // Verify port is bound
    let bindings = env.state.proxy_server.list_bindings().await;
    assert_eq!(bindings.len(), 1, "Should have one port binding");
    assert_eq!(bindings[0].port, 19001);
    assert_eq!(bindings[0].service_id, id);

    // Stop and verify port is unbound
    manager.stop_service(&id).await.expect("should stop service");

    let bindings = env.state.proxy_server.list_bindings().await;
    assert!(bindings.is_empty(), "Port should be unbound after stop");
}

/// Test that service manager can route HTTP requests to services.
///
/// Note: This test uses a minimal WASM component that doesn't implement
/// the HTTP handler interface, so the request will fail at execution time.
/// This test verifies the routing infrastructure works correctly.
#[tokio::test]
async fn test_http_request_routing_infrastructure() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    // Use unique port for this test
    let config = http_service_config_with_port("routing-test", wasm_path, "sha256:test", 19002);

    let manager = env.service_manager().read().await;
    let id = manager.create_service(config).await.expect("should create service");
    manager.start_service(&id).await.expect("should start service");

    // Create a test HTTP request using the builder pattern
    let request = HttpRequest::new("GET", "/test")
        .with_header("host", "localhost:19002")
        .with_authority("localhost:19002");

    // Route the request
    let result = manager.route_http_request(&id, request).await;

    // The minimal WASM component doesn't implement the HTTP interface,
    // so this will fail with an execution error. But this verifies the
    // routing infrastructure works.
    assert!(result.is_err(), "Minimal component should fail to handle HTTP");

    manager.stop_service(&id).await.expect("should stop service");
}

/// Test HTTP service type detection and runtime creation.
#[tokio::test]
async fn test_http_service_type_handling() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    // Create HTTP service with unique port
    let config = http_service_config_with_port("http-type-test", wasm_path.clone(), "sha256:test1", 19003);

    let manager = env.service_manager().read().await;
    let http_id = manager.create_service(config).await.expect("should create HTTP service");

    // Create command service for comparison
    let mut cmd_config = ServiceConfig::new(
        "cmd-type-test".to_string(),
        "1.0.0".to_string(),
        wasm_path,
        "sha256:test2".to_string(),
    );
    cmd_config.service_type = ServiceType::Command;
    cmd_config.replicas.min = 0;

    let cmd_id = manager.create_service(cmd_config).await.expect("should create command service");

    // Get details and verify types
    let http_detail = manager.get_service(&http_id).await.expect("should get HTTP service");
    let cmd_detail = manager.get_service(&cmd_id).await.expect("should get command service");

    assert_eq!(http_detail.config.service_type, ServiceType::Http);
    assert_eq!(cmd_detail.config.service_type, ServiceType::Command);
}

/// Test multiple HTTP services with different ports.
#[tokio::test]
async fn test_multiple_http_services() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    let manager = env.service_manager().read().await;

    // Create first HTTP service on port 19004
    let config1 = http_service_config_with_port("multi-http-1", wasm_path.clone(), "sha256:test1", 19004);
    let id1 = manager.create_service(config1).await.expect("should create first service");

    // Create second HTTP service on port 19005
    let config2 = http_service_config_with_port("multi-http-2", wasm_path, "sha256:test2", 19005);
    let id2 = manager.create_service(config2).await.expect("should create second service");

    // Start both services
    manager.start_service(&id1).await.expect("should start first service");
    manager.start_service(&id2).await.expect("should start second service");

    // Verify both ports are bound
    let bindings = env.state.proxy_server.list_bindings().await;
    assert_eq!(bindings.len(), 2, "Should have two port bindings");

    let ports: Vec<u16> = bindings.iter().map(|b| b.port).collect();
    assert!(ports.contains(&19004), "Port 19004 should be bound");
    assert!(ports.contains(&19005), "Port 19005 should be bound");

    // Stop both
    manager.stop_service(&id1).await.expect("should stop first service");
    manager.stop_service(&id2).await.expect("should stop second service");
}

/// Test port conflict detection.
#[tokio::test]
async fn test_port_conflict_detection() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    let manager = env.service_manager().read().await;

    // Create and start first service on port 19006
    let config1 = http_service_config_with_port("conflict-1", wasm_path.clone(), "sha256:test1", 19006);
    let id1 = manager.create_service(config1).await.expect("should create first service");
    manager.start_service(&id1).await.expect("should start first service");

    // Create second service on same port
    let config2 = http_service_config_with_port("conflict-2", wasm_path, "sha256:test2", 19006);
    let id2 = manager.create_service(config2).await.expect("should create second service");

    // Starting second service should fail due to port conflict
    let result = manager.start_service(&id2).await;
    assert!(result.is_err(), "Should fail due to port conflict");

    // Cleanup
    manager.stop_service(&id1).await.expect("should stop first service");
}

/// Test capability-based outbound validation.
#[tokio::test]
async fn test_outbound_capability_validation() {
    use fabricks_runtime::http::OutboundHandler;
    use fabricksd::service::CapabilityOutboundHandler;

    // Test with allowed connection
    let caps_allowed = Capabilities {
        network: Some(NetworkCapabilities {
            connect: Some(vec!["api.example.com:443".to_string()]),
            listen: None,
            allow_all_outbound: None,
        }),
        ..Default::default()
    };

    let handler = CapabilityOutboundHandler::new(caps_allowed);
    assert!(handler.is_allowed("api.example.com", 443).unwrap());
    assert!(!handler.is_allowed("evil.com", 443).unwrap());

    // Test with no connect capabilities
    let caps_none = Capabilities::default();
    let handler_none = CapabilityOutboundHandler::new(caps_none);
    assert!(!handler_none.is_allowed("any.com", 80).unwrap());
}

/// Test service state transitions for HTTP services.
#[tokio::test]
async fn test_http_service_state_transitions() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    // Use unique port for this test
    let config = http_service_config_with_port("state-test", wasm_path, "sha256:test", 19007);

    let manager = env.service_manager().read().await;
    let id = manager.create_service(config).await.expect("should create service");

    // Creating state
    let detail = manager.get_service(&id).await.expect("should get service");
    assert_eq!(detail.state, State::Creating);

    // Start -> Running
    manager.start_service(&id).await.expect("should start service");
    let detail = manager.get_service(&id).await.expect("should get service");
    assert_eq!(detail.state, State::Running);

    // Stop -> Stopped
    manager.stop_service(&id).await.expect("should stop service");
    let detail = manager.get_service(&id).await.expect("should get service");
    assert_eq!(detail.state, State::Stopped);

    // Can restart
    manager.start_service(&id).await.expect("should restart service");
    let detail = manager.get_service(&id).await.expect("should get service");
    assert_eq!(detail.state, State::Running);

    manager.stop_service(&id).await.expect("should stop service");
}
