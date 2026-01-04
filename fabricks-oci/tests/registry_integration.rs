//! Integration tests for OCI registry operations.
//!
//! These tests require a real OCI registry to run. Set the following environment
//! variables to enable them:
//!
//! - `FABRICKS_TEST_REGISTRY`: Registry URL (e.g., "registry.ldllc.dev")
//! - `FABRICKS_TEST_REGISTRY_USER`: (optional) Username for authentication
//! - `FABRICKS_TEST_REGISTRY_PASSWORD`: (optional) Password for authentication
//!
//! Example:
//! ```bash
//! FABRICKS_TEST_REGISTRY=registry.ldllc.dev cargo test -p fabricks-oci --test registry_integration
//! ```

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use fabricks_common::models::fabrickfile::Info;
use fabricks_common::{Capabilities, Fabrickfile};
use fabricks_oci::{FabricksClient, FabricksModule, Reference, RegistryAuth};

/// Get the test registry URL from environment.
fn get_registry() -> Option<String> {
    env::var("FABRICKS_TEST_REGISTRY").ok()
}

/// Get registry authentication from environment.
fn get_auth() -> RegistryAuth {
    match (
        env::var("FABRICKS_TEST_REGISTRY_USER"),
        env::var("FABRICKS_TEST_REGISTRY_PASSWORD"),
    ) {
        (Ok(user), Ok(pass)) => RegistryAuth::Basic(user, pass),
        _ => RegistryAuth::Anonymous,
    }
}

/// Create a test Fabrickfile configuration.
fn test_config(name: &str, version: &str) -> Fabrickfile {
    Fabrickfile {
        fabrick_version: "1.0".to_string(),
        info: Info {
            name: name.to_string(),
            version: version.to_string(),
            service_type: fabricks_common::models::fabrickfile::ServiceType::default(),
            description: Some("Integration test module".to_string()),
            authors: Some(vec!["Test Author".to_string()]),
            license: Some("MIT".to_string()),
            homepage: None,
            repository: Some("https://github.com/example/test".to_string()),
            documentation: None,
            keywords: Some(vec!["test".to_string(), "integration".to_string()]),
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
    }
}

/// Generate a unique tag for test isolation.
fn unique_tag() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("test-{timestamp}")
}

/// Create a minimal valid WASM module (just the magic bytes + version).
fn minimal_wasm() -> Vec<u8> {
    // Minimal valid WASM: magic number (0x00 0x61 0x73 0x6d) + version (0x01 0x00 0x00 0x00)
    vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
}

#[tokio::test]
async fn test_push_and_pull_module() {
    let Some(registry) = get_registry() else {
        eprintln!("Skipping test: FABRICKS_TEST_REGISTRY not set");
        return;
    };

    let auth = get_auth();
    let client = FabricksClient::new();

    let tag = unique_tag();
    let reference: Reference = format!("{registry}/fabricks-test/hello:{tag}")
        .parse()
        .expect("Invalid reference");

    // Create test module
    let config = test_config("hello", "1.0.0");
    let wasm = minimal_wasm();
    let module = FabricksModule::new(config, wasm.clone());

    // Push module
    let manifest_url = client
        .push(&reference, &module, &auth)
        .await
        .expect("Failed to push module");

    println!("Pushed to: {manifest_url}");
    assert!(!manifest_url.is_empty());

    // Pull module back
    let pulled = client
        .pull(&reference, &auth)
        .await
        .expect("Failed to pull module");

    assert_eq!(pulled.module.name(), "hello");
    assert_eq!(pulled.module.version(), "1.0.0");
    assert_eq!(pulled.module.wasm_bytes(), wasm.as_slice());
    assert!(!pulled.digest.is_empty());
}

#[tokio::test]
async fn test_module_exists() {
    let Some(registry) = get_registry() else {
        eprintln!("Skipping test: FABRICKS_TEST_REGISTRY not set");
        return;
    };

    let auth = get_auth();
    let client = FabricksClient::new();

    let tag = unique_tag();
    let reference: Reference = format!("{registry}/fabricks-test/exists-test:{tag}")
        .parse()
        .expect("Invalid reference");

    // Should not exist initially
    let exists_before = client
        .exists(&reference, &auth)
        .await
        .expect("Failed to check existence");
    assert!(!exists_before, "Module should not exist before push");

    // Push module
    let config = test_config("exists-test", "1.0.0");
    let module = FabricksModule::new(config, minimal_wasm());
    client
        .push(&reference, &module, &auth)
        .await
        .expect("Failed to push module");

    // Should exist now
    let exists_after = client
        .exists(&reference, &auth)
        .await
        .expect("Failed to check existence");
    assert!(exists_after, "Module should exist after push");
}

#[tokio::test]
async fn test_list_tags() {
    let Some(registry) = get_registry() else {
        eprintln!("Skipping test: FABRICKS_TEST_REGISTRY not set");
        return;
    };

    let auth = get_auth();
    let client = FabricksClient::new();

    let base_tag = unique_tag();
    let tag1 = format!("{base_tag}-v1");
    let tag2 = format!("{base_tag}-v2");

    let ref1: Reference = format!("{registry}/fabricks-test/tags-test:{tag1}")
        .parse()
        .expect("Invalid reference");
    let ref2: Reference = format!("{registry}/fabricks-test/tags-test:{tag2}")
        .parse()
        .expect("Invalid reference");

    // Push two versions
    let config1 = test_config("tags-test", "1.0.0");
    let module1 = FabricksModule::new(config1, minimal_wasm());
    client
        .push(&ref1, &module1, &auth)
        .await
        .expect("Failed to push v1");

    let config2 = test_config("tags-test", "2.0.0");
    let module2 = FabricksModule::new(config2, minimal_wasm());
    client
        .push(&ref2, &module2, &auth)
        .await
        .expect("Failed to push v2");

    // List tags
    let tags = client
        .list_tags(&ref1, &auth)
        .await
        .expect("Failed to list tags");

    println!("Found tags: {tags:?}");
    assert!(tags.contains(&tag1), "Should contain first tag");
    assert!(tags.contains(&tag2), "Should contain second tag");
}

#[tokio::test]
async fn test_push_with_annotations() {
    let Some(registry) = get_registry() else {
        eprintln!("Skipping test: FABRICKS_TEST_REGISTRY not set");
        return;
    };

    let auth = get_auth();
    let client = FabricksClient::new();

    let tag = unique_tag();
    let reference: Reference = format!("{registry}/fabricks-test/annotated:{tag}")
        .parse()
        .expect("Invalid reference");

    // Create module with custom annotation
    let config = test_config("annotated", "1.0.0");
    let module = FabricksModule::new(config, minimal_wasm())
        .with_annotation("custom.key".to_string(), "custom-value".to_string());

    // Push and pull
    client
        .push(&reference, &module, &auth)
        .await
        .expect("Failed to push");

    let pulled = client
        .pull(&reference, &auth)
        .await
        .expect("Failed to pull");

    // Verify metadata survived round-trip
    assert_eq!(pulled.module.name(), "annotated");
    assert_eq!(pulled.module.version(), "1.0.0");
    assert_eq!(
        pulled.module.config().info.description,
        Some("Integration test module".to_string())
    );
}

#[tokio::test]
async fn test_content_integrity() {
    let Some(registry) = get_registry() else {
        eprintln!("Skipping test: FABRICKS_TEST_REGISTRY not set");
        return;
    };

    let auth = get_auth();
    let client = FabricksClient::new();

    let tag = unique_tag();
    let reference: Reference = format!("{registry}/fabricks-test/integrity:{tag}")
        .parse()
        .expect("Invalid reference");

    // Create module with specific content
    let wasm_content = b"This is test WASM content for integrity verification";
    let config = test_config("integrity", "1.0.0");
    let module = FabricksModule::new(config, wasm_content.to_vec());

    let original_digest = module.wasm_digest();

    // Push and pull
    client
        .push(&reference, &module, &auth)
        .await
        .expect("Failed to push");

    let pulled = client
        .pull(&reference, &auth)
        .await
        .expect("Failed to pull");

    // Verify content is identical
    assert_eq!(pulled.module.wasm_bytes(), wasm_content);
    assert_eq!(pulled.module.wasm_digest(), original_digest);
}
