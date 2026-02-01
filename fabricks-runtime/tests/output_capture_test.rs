//! Integration test for stdout/stderr capture.
//!
//! Tests that the LogCaptureSink properly captures output from WASM modules.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use fabricks_runtime::output::{LogCaptureSink, LogStream, LogWriter};
use fabricks_runtime::{Runtime, RuntimeConfig};

/// Test log writer that collects entries.
struct TestLogWriter {
    entries: Mutex<VecDeque<(LogStream, String)>>,
}

impl TestLogWriter {
    fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
        }
    }

    fn get_entries(&self) -> Vec<(LogStream, String)> {
        self.entries
            .lock()
            .expect("lock poisoned")
            .iter()
            .cloned()
            .collect()
    }
}

impl LogWriter for TestLogWriter {
    fn write_log(&self, stream: LogStream, message: &str) {
        eprintln!("[TestLogWriter] {:?}: {}", stream, message);
        self.entries
            .lock()
            .expect("lock poisoned")
            .push_back((stream, message.to_string()));
    }
}

#[test]
fn test_hello_world_output_capture() {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("fabricks_runtime=debug")
        .try_init();

    // Load the rust-hello WASM component
    let wasm_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/rust-hello/target/wasm32-wasip1/release/rust_hello.wasm"
    );

    eprintln!("Loading WASM from: {}", wasm_path);

    let wasm_bytes = match std::fs::read(wasm_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Skipping test - WASM not found: {}. Run `fabricks service run examples/rust-hello` first.", e);
            return;
        }
    };

    eprintln!("Loaded {} bytes of WASM", wasm_bytes.len());

    // Create runtime with default config
    let config = RuntimeConfig::default();
    let runtime = match Runtime::new(&wasm_bytes, config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to create runtime: {}", e);
            panic!("Runtime creation failed: {}", e);
        }
    };

    eprintln!("Runtime created successfully");

    // Create log capture
    let writer = Arc::new(TestLogWriter::new());
    let stdout_sink = LogCaptureSink::new(
        Arc::clone(&writer) as Arc<dyn LogWriter>,
        LogStream::Stdout,
    );
    let stderr_sink = LogCaptureSink::new(
        Arc::clone(&writer) as Arc<dyn LogWriter>,
        LogStream::Stderr,
    );

    eprintln!("Running WASM with output capture...");

    // Run with output capture
    let result = runtime.run_with_output(stdout_sink, stderr_sink);

    eprintln!("WASM execution result: {:?}", result);

    match result {
        Ok(()) => eprintln!("WASM executed successfully"),
        Err(e) => eprintln!("WASM execution error: {}", e),
    }

    // Check captured logs
    let entries = writer.get_entries();
    eprintln!("\n=== Captured {} log entries ===", entries.len());
    for (i, (stream, msg)) in entries.iter().enumerate() {
        eprintln!("  [{}] {:?}: {}", i, stream, msg);
    }

    // Verify we captured some output
    assert!(
        !entries.is_empty(),
        "Expected to capture stdout from rust-hello, but got no logs"
    );

    // Check for expected output
    let has_hello = entries.iter().any(|(_, msg)| msg.contains("Hello"));
    assert!(
        has_hello,
        "Expected 'Hello' in captured output, got: {:?}",
        entries
    );
}
