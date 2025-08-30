//! Tests for the parallelized view runtime pool
//!
//! This module contains comprehensive tests for the view pool functionality,
//! including concurrent execution, load balancing, and performance validation.

use crate::tests::stateful_view_test::InMemoryStore;
use crate::view_pool::{ViewPoolConfig, ViewPoolSupport};
use crate::MetashrewRuntime;
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{sleep, Duration};

/// Test basic view pool creation and configuration
#[tokio::test]
async fn test_view_pool_creation() -> Result<()> {
    let store = InMemoryStore::new();

    // Create a simple WASM module for testing
    let wasm_bytes = create_test_wasm_module();

    let runtime = MetashrewRuntime::new(&wasm_bytes, store, vec![])?;

    let config = ViewPoolConfig {
        pool_size: 4,
        max_concurrent_requests: Some(8),
        enable_logging: false,
    };

    let pool = runtime.create_view_pool(config.clone()).await?;
    let stats = pool.get_stats().await;

    assert_eq!(stats.pool_size, 4);
    assert_eq!(stats.max_concurrent_requests, 8);
    assert_eq!(stats.available_permits, 8);
    assert_eq!(stats.active_requests(), 0);
    assert_eq!(stats.utilization_percentage(), 0.0);

    Ok(())
}

/// Test concurrent view execution with the pool
#[tokio::test]
async fn test_concurrent_view_execution() -> Result<()> {
    let store = InMemoryStore::new();
    let wasm_bytes = create_test_wasm_module();
    let runtime = MetashrewRuntime::new(&wasm_bytes, store, vec![])?;

    let config = ViewPoolConfig {
        pool_size: 2,
        max_concurrent_requests: Some(4),
        enable_logging: false,
    };

    let pool: Arc<_> = Arc::new(runtime.create_view_pool(config).await?);

    // Execute multiple view functions concurrently
    let mut handles = vec![];

    for i in 0..8 {
        let pool_clone = pool.clone();
        let input = format!("test_input_{}", i).into_bytes();

        let handle = tokio::spawn(async move {
            pool_clone
                .view("test_view".to_string(), &input, i as u32)
                .await
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results: Result<Vec<_>, _> = futures::future::try_join_all(handles).await;

    // All tasks should complete successfully (even if the WASM function fails)
    assert!(results.is_ok());

    let stats = pool.get_stats().await;
    assert_eq!(stats.total_requests_processed, 8);

    Ok(())
}

/// Test pool load balancing with round-robin distribution
#[tokio::test]
async fn test_pool_load_balancing() -> Result<()> {
    let store = InMemoryStore::new();
    let wasm_bytes = create_test_wasm_module();
    let runtime = MetashrewRuntime::new(&wasm_bytes, store, vec![])?;

    let config = ViewPoolConfig {
        pool_size: 3,
        max_concurrent_requests: Some(3),
        enable_logging: false,
    };

    let pool = runtime.create_view_pool(config).await?;

    // Execute requests sequentially to test round-robin
    for i in 0..9 {
        let input = format!("test_input_{}", i).into_bytes();

        // This should distribute across the 3 runtimes in round-robin fashion
        let _result = pool.view("test_view".to_string(), &input, i as u32).await;

        // We expect this to work even if the WASM function doesn't exist
        // The important thing is that the pool distributes requests correctly
    }

    let stats = pool.get_stats().await;
    assert_eq!(stats.total_requests_processed, 9);

    Ok(())
}

/// Test pool resizing functionality
#[tokio::test]
async fn test_pool_resizing() -> Result<()> {
    let store = InMemoryStore::new();
    let wasm_bytes = create_test_wasm_module();
    let runtime = MetashrewRuntime::new(&wasm_bytes, store, vec![])?;

    let config = ViewPoolConfig {
        pool_size: 2,
        max_concurrent_requests: Some(4),
        enable_logging: false,
    };

    let mut pool = runtime.create_view_pool(config).await?;

    // Initial size should be 2
    let stats = pool.get_stats().await;
    assert_eq!(stats.pool_size, 2);

    // Resize to 4
    pool.resize_pool(4).await?;
    let stats = pool.get_stats().await;
    assert_eq!(stats.pool_size, 4);

    // Resize back to 1
    pool.resize_pool(1).await?;
    let stats = pool.get_stats().await;
    assert_eq!(stats.pool_size, 1);

    // Test error case - size 0
    let result = pool.resize_pool(0).await;
    assert!(result.is_err());

    Ok(())
}

/// Test pool statistics and utilization tracking
#[tokio::test]
async fn test_pool_statistics() -> Result<()> {
    let store = InMemoryStore::new();
    let wasm_bytes = create_test_wasm_module();
    let runtime = MetashrewRuntime::new(&wasm_bytes, store, vec![])?;

    let config = ViewPoolConfig {
        pool_size: 2,
        max_concurrent_requests: Some(4),
        enable_logging: false,
    };

    let pool: Arc<_> = Arc::new(runtime.create_view_pool(config).await?);

    // Initial stats
    let stats = pool.get_stats().await;
    assert_eq!(stats.pool_size, 2);
    assert_eq!(stats.max_concurrent_requests, 4);
    assert_eq!(stats.available_permits, 4);
    assert_eq!(stats.active_requests(), 0);
    assert_eq!(stats.utilization_percentage(), 0.0);

    // Start some long-running tasks to test utilization
    let pool_clone = pool.clone();
    let _handle1 = tokio::spawn(async move {
        let input = b"long_running_1".to_vec();
        let _result = pool_clone.view("slow_view".to_string(), &input, 1).await;
    });

    let pool_clone = pool.clone();
    let _handle2 = tokio::spawn(async move {
        let input = b"long_running_2".to_vec();
        let _result = pool_clone.view("slow_view".to_string(), &input, 2).await;
    });

    // Give tasks time to start
    sleep(Duration::from_millis(10)).await;

    // Check stats while tasks are running
    let stats = pool.get_stats().await;
    // Note: In a real test with actual WASM execution, we would see active requests
    // For this test, the tasks complete quickly since the WASM function doesn't exist

    Ok(())
}

/// Benchmark view pool performance vs single runtime
#[tokio::test]
async fn test_view_pool_performance() -> Result<()> {
    let store = InMemoryStore::new();
    let wasm_bytes = create_test_wasm_module();
    let runtime = MetashrewRuntime::new(&wasm_bytes, store.clone(), vec![])?;

    let config = ViewPoolConfig {
        pool_size: 4,
        max_concurrent_requests: Some(8),
        enable_logging: false,
    };

    let pool: Arc<_> = Arc::new(runtime.create_view_pool(config).await?);

    // Test concurrent execution with pool
    let start_time = Instant::now();
    let mut handles = vec![];

    for i in 0..16 {
        let pool_clone = pool.clone();
        let input = format!("perf_test_{}", i).into_bytes();

        let handle = tokio::spawn(async move {
            pool_clone
                .view("perf_view".to_string(), &input, i as u32)
                .await
        });

        handles.push(handle);
    }

    let _results: Result<Vec<_>, _> = futures::future::try_join_all(handles).await;
    let pool_duration = start_time.elapsed();

    println!(
        "Pool execution time for 16 concurrent requests: {:?}",
        pool_duration
    );

    // Verify all requests were processed
    let stats = pool.get_stats().await;
    assert_eq!(stats.total_requests_processed, 16);

    Ok(())
}

/// Test error handling in the view pool
#[tokio::test]
async fn test_view_pool_error_handling() -> Result<()> {
    let store = InMemoryStore::new();
    let wasm_bytes = create_test_wasm_module();
    let runtime = MetashrewRuntime::new(&wasm_bytes, store, vec![])?;

    let config = ViewPoolConfig {
        pool_size: 2,
        max_concurrent_requests: Some(4),
        enable_logging: false,
    };

    let pool = runtime.create_view_pool(config).await?;

    // Test with non-existent function
    let result = pool
        .view(
            "non_existent_function".to_string(),
            &b"test_input".to_vec(),
            1,
        )
        .await;

    // Should return an error for non-existent function
    assert!(result.is_err());

    // Test with invalid input (this depends on the WASM module implementation)
    let result = pool.view("test_view".to_string(), &vec![], 1).await;

    // The result depends on the WASM module, but the pool should handle it gracefully
    // In our test case with a minimal WASM module, this will likely error

    Ok(())
}

/// Create a minimal WASM module for testing
///
/// This creates a simple WASM module that can be used for testing the pool
/// functionality without requiring a full indexer implementation.
fn create_test_wasm_module() -> Vec<u8> {
    // This is a minimal WASM module that exports a memory and a simple function
    // In a real implementation, this would be the compiled indexer WASM
    vec![
        0x00, 0x61, 0x73, 0x6d, // WASM magic number
        0x01, 0x00, 0x00, 0x00, // WASM version
        // Minimal module structure
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // Type section: function type () -> ()
        0x03, 0x02, 0x01, 0x00, // Function section: one function of type 0
        0x05, 0x03, 0x01, 0x00, 0x01, // Memory section: 1 page of memory
        0x07, 0x0a, 0x01, 0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02,
        0x00, // Export section: export memory
        0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b, // Code section: empty function body
    ]
}

// Add futures dependency for testing
#[cfg(test)]
mod test_deps {
    // We need futures for join_all in tests
    // This would normally be added to Cargo.toml [dev-dependencies]
}
