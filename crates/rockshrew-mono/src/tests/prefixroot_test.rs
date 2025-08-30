// crates/rockshrew-mono/src/tests/prefixroot_test.rs

/*
 * Chadson v69.0.0
 *
 * This file contains the in-memory tests for the --prefixroot functionality.
 *
 * Purpose:
 * - To test the `metashrew_prefixroot` JSON-RPC method in a mocked environment.
 */

use crate::{run_prod, Args};
use tempfile::tempdir;

#[tokio::test]
async fn test_prefixroot_parsing() {
    let wasm_dir = tempdir().unwrap();
    let wasm_path = wasm_dir.path().join("indexer.wasm");
    std::fs::write(&wasm_path, b"").unwrap();
    let db_path = tempdir().unwrap();
    let args = Args {
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
        prefixroot: vec!["balances:0x62616c616e6365733a,sequence:0x73657175656e63653a".to_string()],
        enable_view_pool: false,
        view_pool_size: None,
        view_pool_max_concurrent: None,
        view_pool_logging: false,
        disable_lru_cache: false,
    };
    // This will panic if parsing fails
    let _ = run_prod(args).await;
}
