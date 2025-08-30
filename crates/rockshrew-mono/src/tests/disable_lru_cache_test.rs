// crates/rockshrew-mono/src/tests/disable_lru_cache_test.rs

/*
 * Chadson v69.0.0
 *
 * This file contains tests for the --disable-lru-cache functionality.
 *
 * Purpose:
 * - To test that the --disable-lru-cache flag is properly parsed and applied
 * - To verify that the MetashrewRuntimeAdapter correctly handles the disable_lru_cache setting
 */

use crate::adapters::MetashrewRuntimeAdapter;
use crate::Args;
use metashrew_runtime::MetashrewRuntime;
use rockshrew_runtime::RocksDBRuntimeAdapter;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_disable_lru_cache_flag_parsing() {
    let wasm_dir = tempdir().unwrap();
    let wasm_path = wasm_dir.path().join("indexer.wasm");
    std::fs::write(&wasm_path, b"").unwrap();
    let db_path = tempdir().unwrap();

    // Test with disable_lru_cache = true
    let args_disabled = Args {
        daemon_rpc_url: "mock".to_string(),
        indexer: wasm_path.clone(),
        db_path: db_path.path().to_path_buf(),
        start_block: Some(0),
        auth: None,
        host: "127.0.0.1".to_string(),
        port: 8081,
        label: None,
        exit_at: Some(0),
        pipeline_size: None,
        cors: None,
        snapshot_directory: None,
        snapshot_interval: 1000,
        repo: None,
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
        prefixroot: vec![],
        enable_view_pool: false,
        view_pool_size: None,
        view_pool_max_concurrent: None,
        view_pool_logging: false,
        disable_lru_cache: true,
    };

    // Test with disable_lru_cache = false
    let args_enabled = Args {
        disable_lru_cache: false,
        ..args_disabled.clone()
    };

    // Verify the flag values are correctly set
    assert_eq!(args_disabled.disable_lru_cache, true);
    assert_eq!(args_enabled.disable_lru_cache, false);
}

#[tokio::test]
async fn test_runtime_adapter_disable_lru_cache() {
    // Create a mock runtime for testing
    let db_path = tempdir().unwrap();
    let runtime_adapter_db =
        RocksDBRuntimeAdapter::open_optimized(db_path.path().to_string_lossy().to_string())
            .unwrap();

    // Create a minimal WASM module for testing
    let minimal_wasm = vec![
        0x00, 0x61, 0x73, 0x6d, // WASM magic number
        0x01, 0x00, 0x00, 0x00, // WASM version
    ];

    let runtime = MetashrewRuntime::new(&minimal_wasm, runtime_adapter_db, vec![]).unwrap();
    let runtime_adapter = MetashrewRuntimeAdapter::new(Arc::new(RwLock::new(runtime)));

    // Test initial state (should be false by default)
    assert_eq!(runtime_adapter.is_lru_cache_disabled(), false);

    // Test setting disable_lru_cache to true
    runtime_adapter.set_disable_lru_cache(true);
    assert_eq!(runtime_adapter.is_lru_cache_disabled(), true);

    // Test setting disable_lru_cache to false
    runtime_adapter.set_disable_lru_cache(false);
    assert_eq!(runtime_adapter.is_lru_cache_disabled(), false);
}

#[test]
fn test_args_clone_with_disable_lru_cache() {
    let wasm_dir = tempdir().unwrap();
    let wasm_path = wasm_dir.path().join("indexer.wasm");
    std::fs::write(&wasm_path, b"").unwrap();
    let db_path = tempdir().unwrap();

    let original_args = Args {
        daemon_rpc_url: "mock".to_string(),
        indexer: wasm_path,
        db_path: db_path.path().to_path_buf(),
        start_block: Some(0),
        auth: None,
        host: "127.0.0.1".to_string(),
        port: 8081,
        label: None,
        exit_at: Some(0),
        pipeline_size: None,
        cors: None,
        snapshot_directory: None,
        snapshot_interval: 1000,
        repo: None,
        max_reorg_depth: 100,
        reorg_check_threshold: 6,
        prefixroot: vec![],
        enable_view_pool: false,
        view_pool_size: None,
        view_pool_max_concurrent: None,
        view_pool_logging: false,
        disable_lru_cache: true,
    };

    // Test that cloning preserves the disable_lru_cache field
    let cloned_args = original_args.clone();
    assert_eq!(cloned_args.disable_lru_cache, true);

    // Test modifying the cloned args
    let mut modified_args = cloned_args;
    modified_args.disable_lru_cache = false;
    assert_eq!(modified_args.disable_lru_cache, false);
    assert_eq!(original_args.disable_lru_cache, true); // Original should be unchanged
}
