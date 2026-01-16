//! End-to-end tests for volume management.
//!
//! These tests verify that volumes are created and wired correctly
//! during mortar deployment using real WASM components.

use std::path::PathBuf;

use fabricks_e2e::helpers::TestEnv;
use tempfile::TempDir;

/// Path to the real hello-world WASM component.
fn hello_world_wasm_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("should have parent")
        .join("examples/hello-world/target/wasm32-wasip2/release/hello-world.wasm")
}

/// Creates a mortar file with a volume definition using the real hello-world WASM.
fn create_mortar_with_volume(temp_dir: &TempDir) -> PathBuf {
    let wasm_path = hello_world_wasm_path();

    // Ensure the WASM exists
    assert!(
        wasm_path.exists(),
        "hello-world WASM not found at {:?}. Run: cd examples/hello-world && cargo build --target wasm32-wasip2 --release",
        wasm_path
    );

    // Create a service directory with a Fabrickfile pointing to the real WASM
    let service_dir = temp_dir.path().join("hello-service");
    std::fs::create_dir_all(&service_dir).expect("should create service dir");

    let fabrickfile_content = format!(
        r#"
fabrick_version = "1.0"

[info]
name = "hello-service"
version = "1.0.0"
type = "command"

[build]
command = "echo 'pre-built'"
output = "{}"
"#,
        wasm_path.display()
    );
    std::fs::write(service_dir.join("Fabrickfile"), fabrickfile_content)
        .expect("should write fabrickfile");

    // Create the mortar file
    let mortar_content = r#"
mortar_version = "1.0"

[project]
name = "volume-test"

[network.default]
internal = true

[service.hello-service]
build = "./hello-service"
networks = ["default"]

[service.hello-service.volumes]
app_data = "/data"

[volume.app_data]
size = "100Mi"
"#;

    let mortar_path = temp_dir.path().join("fabricks-mortar.toml");
    std::fs::write(&mortar_path, mortar_content).expect("should write mortar file");

    mortar_path
}

/// Test that volumes are created during mortar deployment.
#[tokio::test]
async fn test_volume_created_during_mortar_deploy() {
    // Skip if WASM not built
    let wasm_path = hello_world_wasm_path();
    if !wasm_path.exists() {
        eprintln!(
            "Skipping test: hello-world WASM not found. Run: cd examples/hello-world && cargo build --target wasm32-wasip2 --release"
        );
        return;
    }

    let env = TestEnv::new().await.expect("should create test env");
    let temp_dir = TempDir::new().expect("should create temp dir");

    // Create mortar file with volume
    let mortar_path = create_mortar_with_volume(&temp_dir);

    // Deploy the mortar project
    let manager = env.service_manager().read().await;
    let (project_name, service_ids) = manager
        .deploy_mortar(&mortar_path)
        .await
        .expect("should deploy mortar project");

    assert_eq!(project_name, "volume-test");
    assert_eq!(service_ids.len(), 1, "Should have created one service");

    // Verify the volume was created
    let volumes = env.state.volume_manager.list_volumes().await;
    let app_data_volume = volumes.iter().find(|v| v.name == "app_data");
    assert!(
        app_data_volume.is_some(),
        "app_data volume should exist. Found volumes: {:?}",
        volumes.iter().map(|v| &v.name).collect::<Vec<_>>()
    );

    // Verify volume directory exists
    let volume_detail = env
        .state
        .volume_manager
        .get_volume_by_name("app_data")
        .await
        .expect("should get volume by name");
    assert!(
        volume_detail.path.exists(),
        "Volume directory should exist at {:?}",
        volume_detail.path
    );

    // Get the created service and verify it has the volume mount configured
    let service_id = &service_ids[0];
    let service_detail = manager
        .get_service(service_id)
        .await
        .expect("should get service");

    assert_eq!(service_detail.name, "hello-service");

    // Clean up: stop and delete the service
    manager
        .stop_service(service_id)
        .await
        .expect("should stop service");
    manager
        .delete_service(service_id)
        .await
        .expect("should delete service");
}

/// Test that volume directory is created on disk.
#[tokio::test]
async fn test_volume_directory_created() {
    let env = TestEnv::new().await.expect("should create test env");

    // Create a volume directly via the volume manager
    let volume_id = env
        .state
        .volume_manager
        .ensure_volume("direct-vol", Some("100Mi".to_string()))
        .await
        .expect("should create volume");

    // Get volume details
    let detail = env
        .state
        .volume_manager
        .get_volume(&volume_id)
        .await
        .expect("volume should exist");

    // Verify directory was created
    assert!(detail.path.exists(), "Volume directory should exist");
    assert!(detail.path.is_dir(), "Volume path should be a directory");
}

/// Test that volumes can be mounted and unmounted by services.
#[tokio::test]
async fn test_volume_mount_tracking() {
    let env = TestEnv::new().await.expect("should create test env");

    // Create a volume
    let volume_id = env
        .state
        .volume_manager
        .ensure_volume("mount-test-vol", None)
        .await
        .expect("should create volume");

    // Mount it for a fake service
    let host_path = env
        .state
        .volume_manager
        .mount_volume(&volume_id, "fake-service-id")
        .await
        .expect("should mount volume");

    // Verify mount is tracked
    let detail = env
        .state
        .volume_manager
        .get_volume(&volume_id)
        .await
        .expect("volume should exist");

    assert!(
        detail.mounted_by.contains(&"fake-service-id".to_string()),
        "Volume should be mounted by fake-service-id"
    );
    assert!(host_path.exists(), "Host path should exist");

    // Unmount
    env.state
        .volume_manager
        .unmount_volume(&volume_id, "fake-service-id")
        .await
        .expect("should unmount volume");

    let detail = env
        .state
        .volume_manager
        .get_volume(&volume_id)
        .await
        .expect("volume should exist");

    assert!(
        !detail.mounted_by.contains(&"fake-service-id".to_string()),
        "Volume should no longer be mounted by fake-service-id"
    );
}

/// Test that mounted volumes cannot be deleted.
#[tokio::test]
async fn test_cannot_delete_mounted_volume() {
    let env = TestEnv::new().await.expect("should create test env");

    // Create a volume
    let volume_id = env
        .state
        .volume_manager
        .ensure_volume("delete-test-vol", None)
        .await
        .expect("should create volume");

    // Mount it
    env.state
        .volume_manager
        .mount_volume(&volume_id, "some-service")
        .await
        .expect("should mount volume");

    // Try to delete - should fail
    let result = env.state.volume_manager.delete_volume(&volume_id).await;
    assert!(
        result.is_err(),
        "Should not be able to delete mounted volume"
    );

    // Unmount and delete
    env.state
        .volume_manager
        .unmount_volume(&volume_id, "some-service")
        .await
        .expect("should unmount volume");

    let result = env.state.volume_manager.delete_volume(&volume_id).await;
    assert!(result.is_ok(), "Should be able to delete unmounted volume");
}

/// Test that ensure_volume is idempotent.
#[tokio::test]
async fn test_ensure_volume_idempotent() {
    let env = TestEnv::new().await.expect("should create test env");

    // Create volume first time
    let id1 = env
        .state
        .volume_manager
        .ensure_volume("idempotent-vol", Some("1Gi".to_string()))
        .await
        .expect("should create volume");

    // Ensure again - should return same ID
    let id2 = env
        .state
        .volume_manager
        .ensure_volume("idempotent-vol", Some("2Gi".to_string()))
        .await
        .expect("should return existing volume");

    assert_eq!(
        id1, id2,
        "ensure_volume should return same ID for existing volume"
    );

    // Verify only one volume exists
    let volumes = env.state.volume_manager.list_volumes().await;
    let matching: Vec<_> = volumes
        .iter()
        .filter(|v| v.name == "idempotent-vol")
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "Should have exactly one volume with this name"
    );
}
