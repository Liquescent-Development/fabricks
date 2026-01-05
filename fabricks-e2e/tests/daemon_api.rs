//! End-to-end tests for daemon API functionality.
//!
//! These tests verify that the daemon's REST API endpoints work correctly
//! with real state and service management.

use fabricks_common::models::health_check::HttpHealthCheck;
use fabricks_e2e::helpers::{create_temp_wasm, minimal_wasm_component, test_service_config, TestEnv};
use fabricksd::service::State;

/// Test that the daemon state initializes correctly.
#[tokio::test]
async fn test_daemon_initialization() {
    let env = TestEnv::new().await.expect("should create test env");

    // Verify state is accessible
    let uptime = env.state.uptime();
    assert!(uptime.as_secs() < 5, "Uptime should be recent");
}

/// Test creating a service via the service manager.
#[tokio::test]
async fn test_create_service() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    let config = test_service_config("test-service", wasm_path, "sha256:test");

    let manager = env.service_manager().read().await;
    let id = manager.create_service(config).await.expect("should create service");

    // Verify service exists
    let detail = manager.get_service(&id).await.expect("should get service");
    assert_eq!(detail.name, "test-service");
    assert_eq!(detail.state, State::Creating);
}

/// Test listing services.
#[tokio::test]
async fn test_list_services() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    let manager = env.service_manager().read().await;

    // Initially empty
    let services = manager.list_services().await;
    assert!(services.is_empty());

    // Create a service
    let config = test_service_config("list-test", wasm_path, "sha256:test");
    let _id = manager.create_service(config).await.expect("should create service");

    // Now should have one service
    let services = manager.list_services().await;
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].name, "list-test");
}

/// Test service lifecycle: create -> start -> stop -> delete.
#[tokio::test]
async fn test_service_lifecycle() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    // Create service with no replicas (for quick testing)
    let mut config = test_service_config("lifecycle-test", wasm_path, "sha256:test");
    config.replicas.min = 0;

    let manager = env.service_manager().read().await;
    let id = manager.create_service(config).await.expect("should create service");

    // Start the service
    manager.start_service(&id).await.expect("should start service");

    let detail = manager.get_service(&id).await.expect("should get service");
    assert_eq!(detail.state, State::Running);

    // Stop the service
    manager.stop_service(&id).await.expect("should stop service");

    let detail = manager.get_service(&id).await.expect("should get service");
    assert_eq!(detail.state, State::Stopped);

    // Delete the service
    manager.delete_service(&id).await.expect("should delete service");

    // Verify service is gone
    let result = manager.get_service(&id).await;
    assert!(result.is_err());
}

/// Test that duplicate service names are rejected.
#[tokio::test]
async fn test_duplicate_service_name_rejected() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    let manager = env.service_manager().read().await;

    let config1 = test_service_config("dupe-test", wasm_path.clone(), "sha256:test1");
    let _id1 = manager.create_service(config1).await.expect("should create first service");

    let config2 = test_service_config("dupe-test", wasm_path, "sha256:test2");
    let result = manager.create_service(config2).await;

    assert!(result.is_err(), "Should reject duplicate name");
}

/// Test network creation and management.
#[tokio::test]
async fn test_network_management() {
    let env = TestEnv::new().await.expect("should create test env");

    // Create a network
    let config = fabricksd::network::NetworkConfig::new("test-network".to_string());
    let net_id = env
        .state
        .network_manager
        .create_network(config)
        .await
        .expect("should create network");

    // List networks
    let networks = env.state.network_manager.list_networks().await;
    assert_eq!(networks.len(), 1);
    assert_eq!(networks[0].name, "test-network");

    // Get network detail
    let detail = env
        .state
        .network_manager
        .get_network(&net_id)
        .await
        .expect("should get network");
    assert_eq!(detail.name, "test-network");
    assert!(detail.members.is_empty());

    // Delete network
    env.state
        .network_manager
        .delete_network(&net_id)
        .await
        .expect("should delete network");

    // Verify network is gone
    let result = env.state.network_manager.get_network(&net_id).await;
    assert!(result.is_none());
}

/// Test network with service membership.
#[tokio::test]
async fn test_network_service_membership() {
    let env = TestEnv::new().await.expect("should create test env");
    let (_temp_dir, wasm_path) = create_temp_wasm(&minimal_wasm_component()).expect("should create wasm");

    // Create a network
    let config = fabricksd::network::NetworkConfig::new("member-network".to_string());
    let net_id = env
        .state
        .network_manager
        .create_network(config)
        .await
        .expect("should create network");

    // Create a service
    let svc_config = test_service_config("network-member", wasm_path, "sha256:test");
    let manager = env.service_manager().read().await;
    let svc_id = manager.create_service(svc_config).await.expect("should create service");
    drop(manager);

    // Add service to network
    env.state
        .network_manager
        .add_service(&net_id, &svc_id, "network-member")
        .await
        .expect("should add service to network");

    // Verify membership
    let detail = env
        .state
        .network_manager
        .get_network(&net_id)
        .await
        .expect("should get network");
    assert_eq!(detail.members.len(), 1);

    // Try to delete network with members (should fail)
    let result = env.state.network_manager.delete_network(&net_id).await;
    assert!(result.is_err(), "Should not delete network with members");

    // Remove service from network
    env.state
        .network_manager
        .remove_service(&net_id, &svc_id)
        .await
        .expect("should remove service from network");

    // Now deletion should succeed
    env.state
        .network_manager
        .delete_network(&net_id)
        .await
        .expect("should delete empty network");
}

/// Test health monitor registration.
#[tokio::test]
async fn test_health_monitor_registration() {
    let env = TestEnv::new().await.expect("should create test env");

    // Initially no services monitored
    let health = env.state.health_monitor.get_all_health().await;
    assert!(health.is_empty());

    // Register a service for monitoring
    let http_check = HttpHealthCheck {
        path: "/health".to_string(),
        port: None,
        interval: None,
        timeout: None,
        retries: Some(3),
        method: None,
        expected_status: None,
    };
    env.state.health_monitor.register("test-svc".to_string(), http_check, 8080).await;

    // Should now be tracked
    let health = env.state.health_monitor.get_all_health().await;
    assert_eq!(health.len(), 1);
    assert!(health.contains_key("test-svc"));
}

/// Test proxy server bindings.
#[tokio::test]
async fn test_proxy_bindings() {
    let env = TestEnv::new().await.expect("should create test env");

    // Initially no bindings
    let bindings = env.state.proxy_server.list_bindings().await;
    assert!(bindings.is_empty());

    // Bind a port
    let port = env
        .state
        .proxy_server
        .bind_port(18080, "test-svc".to_string(), "test-service".to_string())
        .await
        .expect("should bind port");
    assert_eq!(port, 18080);

    // Verify binding
    let bindings = env.state.proxy_server.list_bindings().await;
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].port, 18080);
    assert_eq!(bindings[0].service_id, "test-svc");
    assert_eq!(bindings[0].service_name, "test-service");

    // Unbind port
    env.state
        .proxy_server
        .unbind_port(18080)
        .await
        .expect("should unbind port");

    // Verify unbound
    let bindings = env.state.proxy_server.list_bindings().await;
    assert!(bindings.is_empty());
}

/// Test event bus publishing.
#[tokio::test]
async fn test_event_bus() {
    let env = TestEnv::new().await.expect("should create test env");

    // Subscribe to events
    let mut rx = env.state.event_bus.subscribe().await;

    // Publish an event
    let event = fabricksd::events::Event::new(
        fabricksd::events::EventType::ServiceCreated,
        serde_json::json!({"id": "test", "name": "test-service"}),
    );
    env.state.event_bus.publish(event).await;

    // Receive the event
    let received = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        rx.recv(),
    )
    .await
    .expect("should receive event")
    .expect("channel should not close");

    assert!(matches!(
        received.event_type,
        fabricksd::events::EventType::ServiceCreated
    ));
}

/// Test shutdown signal.
#[tokio::test]
async fn test_shutdown_signal() {
    let env = TestEnv::new().await.expect("should create test env");

    let mut rx = env.state.subscribe_shutdown();

    // Send shutdown
    env.state.shutdown();

    // Should receive signal
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        rx.recv(),
    )
    .await;

    assert!(result.is_ok(), "Should receive shutdown signal");
}
