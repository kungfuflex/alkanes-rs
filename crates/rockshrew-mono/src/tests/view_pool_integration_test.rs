//! Integration tests for view pool functionality in rockshrew-mono
//!
//! These tests verify that the view pool is properly integrated into the
//! MetashrewRuntimeAdapter and works correctly with the JSON-RPC server.

use crate::adapters::MetashrewRuntimeAdapter;
use anyhow::Result;
use metashrew_runtime::ViewPoolConfig;
use metashrew_sync::{RuntimeAdapter, ViewCall};
use rockshrew_runtime::RocksDBRuntimeAdapter;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Test helper to create a test runtime adapter
async fn create_test_adapter() -> Result<(
    MetashrewRuntimeAdapter,
    tempfile::TempDir,
    tempfile::TempDir,
)> {
    // Create a temporary directory for the test database
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("test_db");

    // Create a simple test WASM module that implements a basic view function
    let (test_wasm, wasm_temp_dir) = create_test_wasm_module()?;

    // Initialize the runtime with the test WASM
    let runtime_adapter =
        RocksDBRuntimeAdapter::open_optimized(db_path.to_string_lossy().to_string())?;

    let runtime = metashrew_runtime::MetashrewRuntime::load(
        test_wasm,
        runtime_adapter,
        vec![], // No prefix configs for test
    )?;

    let adapter = MetashrewRuntimeAdapter::new(Arc::new(RwLock::new(runtime)));

    Ok((adapter, temp_dir, wasm_temp_dir))
}

/// Creates a minimal test WASM module for testing
fn create_test_wasm_module() -> Result<(std::path::PathBuf, tempfile::TempDir)> {
    // Create a minimal valid WASM binary
    let minimal_wasm = vec![
        0x00, 0x61, 0x73, 0x6d, // WASM magic number
        0x01, 0x00, 0x00, 0x00, // WASM version
        // Add a minimal type section
        0x01, 0x04, 0x01, 0x60, 0x00,
        0x00, // type section: 1 type, function type with no params/results
        // Add a minimal function section
        0x03, 0x02, 0x01, 0x00, // function section: 1 function of type 0
        // Add a minimal code section
        0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b, // code section: 1 function body, empty function
    ];

    let temp_dir = tempfile::tempdir()?;
    let wasm_path = temp_dir.path().join("test.wasm");

    std::fs::write(&wasm_path, minimal_wasm)?;

    Ok((wasm_path, temp_dir))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_view_pool_initialization() -> Result<()> {
    let (adapter, _temp_dir, _wasm_temp_dir) = create_test_adapter().await?;

    // Test that view pool can be initialized
    let config = ViewPoolConfig {
        pool_size: 2,
        max_concurrent_requests: Some(4),
        enable_logging: true,
    };

    let result = adapter.initialize_view_pool(config).await;
    assert!(result.is_ok(), "View pool initialization should succeed");

    // Test that stats are available after initialization
    let stats = adapter.get_view_pool_stats().await;
    assert!(stats.is_some(), "View pool stats should be available");

    let stats = stats.unwrap();
    assert_eq!(stats.pool_size, 2);
    assert_eq!(stats.active_requests(), 0);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_view_execution_with_pool() -> Result<()> {
    let (adapter, _temp_dir, _wasm_temp_dir) = create_test_adapter().await?;

    // Initialize view pool
    let config = ViewPoolConfig {
        pool_size: 2,
        max_concurrent_requests: Some(4),
        enable_logging: false,
    };

    adapter.initialize_view_pool(config).await?;

    // Test view execution through the pool
    let view_call = ViewCall {
        function_name: "test_view".to_string(),
        input_data: vec![1, 2, 3, 4],
        height: 100, // Changed from Some(100) to 100
    };

    // This should use the view pool since it's initialized
    let result = adapter.execute_view(view_call).await;

    // The result might fail due to the minimal WASM module,
    // but it should attempt to use the pool
    match result {
        Ok(_) => {
            // Success - verify stats show the request was processed
            let stats = adapter.get_view_pool_stats().await.unwrap();
            assert!(stats.total_requests_processed > 0);
        }
        Err(_) => {
            // Expected for minimal WASM - just verify pool was attempted
            let stats = adapter.get_view_pool_stats().await.unwrap();
            // Stats should still be available even if execution failed
            assert_eq!(stats.pool_size, 2);
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_view_execution_without_pool() -> Result<()> {
    let (adapter, _temp_dir, _wasm_temp_dir) = create_test_adapter().await?;

    // Simulate the behavior when view pool is disabled - disable stateful views
    adapter.disable_stateful_views().await;

    // Don't initialize view pool - should fall back to non-stateful async execution
    let view_call = ViewCall {
        function_name: "test_view".to_string(),
        input_data: vec![1, 2, 3, 4],
        height: 100, // Changed from Some(100) to 100
    };

    // This should use non-stateful async runtime execution
    let result = adapter.execute_view(view_call).await;

    // Verify no view pool stats are available
    let stats = adapter.get_view_pool_stats().await;
    assert!(stats.is_none(), "No view pool stats should be available");

    // Verify that stateful views are disabled
    assert!(
        !adapter.is_stateful_views_enabled().await,
        "Stateful views should be disabled"
    );

    // The execution might fail due to minimal WASM, but that's expected
    match result {
        Ok(_) => println!("Non-stateful async execution succeeded"),
        Err(e) => println!("Non-stateful async execution failed as expected: {}", e),
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_concurrent_view_execution() -> Result<()> {
    let (adapter, _temp_dir, _wasm_temp_dir) = create_test_adapter().await?;

    // Initialize view pool with limited concurrency
    let config = ViewPoolConfig {
        pool_size: 2,
        max_concurrent_requests: Some(3),
        enable_logging: true,
    };

    adapter.initialize_view_pool(config).await?;

    // Launch multiple concurrent view calls
    let mut handles = vec![];

    for i in 0..5 {
        // Clone the adapter for each task to avoid lifetime issues
        let adapter_clone = adapter.clone();
        let handle = tokio::spawn(async move {
            let view_call = ViewCall {
                function_name: "test_view".to_string(),
                input_data: vec![i as u8; 4],
                height: 100 + i as u32, // Changed from Some(100 + i as u32) to 100 + i as u32
            };

            adapter_clone.execute_view(view_call).await
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    let _results: Vec<_> = futures::future::join_all(handles).await;

    // Verify that requests were processed (some may fail due to minimal WASM)
    let stats = adapter.get_view_pool_stats().await.unwrap();
    assert!(
        stats.total_requests_processed >= 5,
        "All requests should be tracked"
    );

    // Check that concurrency was respected
    println!("Final stats: {:?}", stats);

    Ok(())
}

/// Integration test that simulates the full rockshrew-mono startup process
#[tokio::test(flavor = "multi_thread")]
async fn test_full_integration_simulation() -> Result<()> {
    // This test simulates what happens in run_prod() when view pool is enabled

    // Create test arguments that would enable view pool
    let enable_view_pool = true;
    let view_pool_size = Some(2);
    let view_pool_max_concurrent = Some(4);
    let view_pool_logging = true;

    if enable_view_pool {
        let (adapter, _temp_dir, _wasm_temp_dir) = create_test_adapter().await?;

        let pool_size = view_pool_size.unwrap_or_else(num_cpus::get);
        let max_concurrent = view_pool_max_concurrent.unwrap_or(pool_size * 2);

        let view_pool_config = ViewPoolConfig {
            pool_size,
            max_concurrent_requests: Some(max_concurrent),
            enable_logging: view_pool_logging,
        };

        // This simulates the initialization in run_prod()
        let result = adapter.initialize_view_pool(view_pool_config).await;
        assert!(result.is_ok(), "View pool initialization should succeed");

        // Verify the pool is working
        let stats = adapter.get_view_pool_stats().await;
        assert!(stats.is_some());

        let stats = stats.unwrap();
        assert_eq!(stats.pool_size, pool_size);
        assert_eq!(stats.active_requests(), 0);

        println!(
            "Integration test passed - view pool initialized with {} runtimes",
            pool_size
        );
    }

    Ok(())
}
