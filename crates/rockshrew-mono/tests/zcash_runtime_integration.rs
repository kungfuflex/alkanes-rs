//! Runtime integration tests for Zcash
//!
//! These tests load the actual compiled WASM and test it through metashrew-runtime
//! to verify the full integration works correctly.

mod zcash_runtime_tests {
    use std::path::PathBuf;
    use rockshrew_runtime::adapter::RocksDBRuntimeAdapter;
    use metashrew_runtime::MetashrewRuntime;
    
    /// Test that we can load the actual compiled Zcash WASM into metashrew-runtime
    #[tokio::test]
    async fn test_load_zcash_wasm() {
        let mut wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        wasm_path.push("../../src/tests/indexer_precompiled/alkanes_zcash.wasm");
        
        println!("Loading Zcash WASM from: {:?}", wasm_path);
        assert!(wasm_path.exists(), "Zcash WASM file not found at {:?}. Current dir: {:?}", wasm_path, std::env::current_dir());
        
        // Create temporary RocksDB for testing
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().to_string_lossy().to_string();
        println!("Using temp DB: {:?}", db_path);
        
        // Create RocksDB adapter
        let adapter = RocksDBRuntimeAdapter::open_optimized(db_path)
            .expect("Failed to create RocksDB adapter");
        
        // Create WASM engine with async support
        let mut config = wasmtime::Config::default();
        config.async_support(true);
        let engine = wasmtime::Engine::new(&config).expect("Failed to create engine");
        
        // Load the WASM
        let runtime = MetashrewRuntime::load(wasm_path, adapter, engine).await;
        
        match runtime {
            Ok(_) => {
                println!("✓ Successfully loaded Zcash WASM into metashrew-runtime");
            },
            Err(e) => {
                eprintln!("✗ Failed to load Zcash WASM: {:?}", e);
                eprintln!("Error source: {:?}", e.source());
                eprintln!("Error chain: {:#}", e);
                panic!("Failed to load WASM: {}", e);
            }
        }
    }
    
    /// Test that we can process Zcash block 0 through the runtime
    #[tokio::test]
    async fn test_runtime_process_zcash_block_0() {
        let _ = env_logger::builder().is_test(true).try_init();
        
        let mut wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        wasm_path.push("../../src/tests/indexer_precompiled/alkanes_zcash.wasm");
        
        assert!(wasm_path.exists(), "Zcash WASM not found");
        
        // Get block 0 hex data from the test file
        let mut block_hex_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        block_hex_path.push("../alkanes/src/tests/blocks/zec_0.hex");
        
        let block_hex = std::fs::read_to_string(block_hex_path)
            .expect("Failed to read block 0 hex file")
            .trim()
            .to_string();
        
        let block_bytes = hex::decode(&block_hex).expect("Failed to decode block hex");
        
        println!("Block 0 size: {} bytes", block_bytes.len());
        println!("Block hex (first 100 chars): {}", &block_hex[..100.min(block_hex.len())]);
        
        // Create temporary RocksDB for testing
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().to_string_lossy().to_string();
        
        // Create RocksDB adapter
        let adapter = RocksDBRuntimeAdapter::open_optimized(db_path)
            .expect("Failed to create RocksDB adapter");
        
        // Create WASM engine with async support
        let mut config = wasmtime::Config::default();
        config.async_support(true);
        let engine = wasmtime::Engine::new(&config).expect("Failed to create engine");
        
        // Load the WASM
        let runtime = MetashrewRuntime::load(wasm_path, adapter, engine)
            .await
            .expect("Failed to load Zcash WASM");
        
        println!("✓ WASM loaded successfully");
        
        let height = 0u32;
        println!("Processing block {} ({} bytes)...", height, block_bytes.len());
        
        let result = runtime.process_block(height, &block_bytes).await;
        
        match result {
            Ok(_) => {
                println!("✓ Successfully processed Zcash block 0");
            },
            Err(e) => {
                eprintln!("✗ Failed to process block 0: {:?}", e);
                eprintln!("Error source: {:?}", e.source());
                eprintln!("Error chain: {:#}", e);
                panic!("Block 0 processing failed: {}", e);
            }
        }
    }
    
    /// Test processing block 1
    #[tokio::test]
    #[ignore] // Requires fetching block 1 from RPC
    async fn test_runtime_process_zcash_block_1() {
        // This test would fetch block 1 from RPC or use a static hex file
        // For now, it's ignored
        println!("Block 1 test - needs implementation");
    }
}
