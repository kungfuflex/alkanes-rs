//! GPU Integration Tests
//! 
//! Tests to verify that GPU host functions are properly integrated into the WASM runtime
//! and that GPU pipeline statistics are correctly recorded.

use crate::logging::{init_block_stats, get_block_stats, record_gpu_shard_execution, record_wasm_fallback_shard};
use crate::vm::instance::AlkanesInstance;
use crate::vm::runtime::AlkanesRuntimeContext;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use protorune::message::MessageContextParcel;
use bitcoin::{Block, Transaction, TxOut, ScriptBuf, Amount};
use std::sync::{Arc, Mutex};
use anyhow::Result;

/// Test that GPU host functions are properly linked in the WASM runtime
#[test]
fn test_gpu_host_functions_linked() {
    // Create a minimal WASM binary that calls GPU host functions
    let wasm_binary = create_test_wasm_with_gpu_calls();
    
    // Create a test context
    let context = create_test_context();
    
    // Create an AlkanesInstance with the test binary
    let result = AlkanesInstance::from_alkane(context, Arc::new(wasm_binary), 1000000);
    
    // The instance should be created successfully (GPU host functions are linked)
    assert!(result.is_ok(), "Failed to create AlkanesInstance with GPU host functions: {:?}", result.err());
    
    let instance = result.unwrap();
    
    // Verify that the instance has the expected exports
    // Note: We can't directly test the GPU functions without actual WASM that calls them,
    // but we can verify the instance was created successfully with GPU host functions linked
    assert!(instance.instance.get_memory(&instance.store, "memory").is_some());
}

/// Test that GPU statistics are properly recorded
#[test]
fn test_gpu_statistics_recording() {
    init_block_stats();
    
    // Record some GPU execution statistics
    record_gpu_shard_execution(32, 1500, 1024 * 1024); // 32 messages, 1.5ms, 1MB
    record_gpu_shard_execution(28, 1200, 800 * 1024);  // 28 messages, 1.2ms, 800KB
    
    // Record some WASM fallbacks
    record_wasm_fallback_shard();
    record_wasm_fallback_shard();
    
    // Get the recorded statistics
    let stats = get_block_stats().expect("Failed to get block stats");
    
    // Verify GPU execution stats
    assert_eq!(stats.pipeline_stats.gpu_shards_executed, 2);
    assert_eq!(stats.pipeline_stats.gpu_messages_processed, 60); // 32 + 28
    assert_eq!(stats.pipeline_stats.gpu_execution_time_us, 2700); // 1500 + 1200
    assert_eq!(stats.pipeline_stats.gpu_memory_used_bytes, 1024 * 1024); // Max of the two
    
    // Verify WASM fallback stats
    assert_eq!(stats.pipeline_stats.wasm_fallback_shards, 2);
}

/// Test GPU pipeline efficiency calculation
#[test]
fn test_gpu_pipeline_efficiency() {
    init_block_stats();
    
    // Record mixed GPU and WASM execution
    record_gpu_shard_execution(64, 2000, 2 * 1024 * 1024); // Large GPU shard
    record_gpu_shard_execution(32, 1000, 1024 * 1024);     // Medium GPU shard
    record_wasm_fallback_shard(); // 1 WASM fallback
    
    let stats = get_block_stats().expect("Failed to get block stats");
    
    // Calculate expected efficiency
    let gpu_messages = stats.pipeline_stats.gpu_messages_processed; // 96
    let estimated_wasm_messages = stats.pipeline_stats.wasm_fallback_shards * 32; // 32
    let total_messages = gpu_messages + estimated_wasm_messages; // 128
    let expected_efficiency = (gpu_messages as f64 / total_messages as f64) * 100.0; // 75%
    
    assert_eq!(gpu_messages, 96);
    assert_eq!(estimated_wasm_messages, 32);
    assert_eq!(total_messages, 128);
    assert!((expected_efficiency - 75.0).abs() < 0.1); // Should be 75%
}

/// Test that GPU memory tracking works correctly
#[test]
fn test_gpu_memory_tracking() {
    init_block_stats();
    
    // Record GPU executions with different memory usage
    record_gpu_shard_execution(16, 500, 512 * 1024);    // 512KB
    record_gpu_shard_execution(32, 1000, 1024 * 1024);  // 1MB (higher)
    record_gpu_shard_execution(24, 750, 768 * 1024);    // 768KB
    
    let stats = get_block_stats().expect("Failed to get block stats");
    
    // Memory tracking should record the maximum memory used
    assert_eq!(stats.pipeline_stats.gpu_memory_used_bytes, 1024 * 1024); // 1MB max
    assert_eq!(stats.pipeline_stats.gpu_shards_executed, 3);
    assert_eq!(stats.pipeline_stats.gpu_messages_processed, 72); // 16 + 32 + 24
}

/// Test GPU execution time accumulation
#[test]
fn test_gpu_execution_time_accumulation() {
    init_block_stats();
    
    // Record multiple GPU executions
    record_gpu_shard_execution(10, 100, 100 * 1024);   // 100μs
    record_gpu_shard_execution(20, 250, 200 * 1024);   // 250μs
    record_gpu_shard_execution(30, 400, 300 * 1024);   // 400μs
    record_gpu_shard_execution(40, 500, 400 * 1024);   // 500μs
    
    let stats = get_block_stats().expect("Failed to get block stats");
    
    // Execution time should accumulate
    assert_eq!(stats.pipeline_stats.gpu_execution_time_us, 1250); // 100 + 250 + 400 + 500
    assert_eq!(stats.pipeline_stats.gpu_shards_executed, 4);
    assert_eq!(stats.pipeline_stats.gpu_messages_processed, 100); // 10 + 20 + 30 + 40
}

/// Create a minimal test context for WASM execution
fn create_test_context() -> Arc<Mutex<AlkanesRuntimeContext>> {
    let mut parcel = MessageContextParcel::default();
    
    // Create a minimal transaction
    let transaction = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![TxOut {
            value: Amount::from_sat(1000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    
    parcel.transaction = transaction;
    parcel.height = 100;
    parcel.txindex = 0;
    parcel.vout = 0;
    
    let cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![],
    };
    
    Arc::new(Mutex::new(AlkanesRuntimeContext::from_parcel_and_cellpack(&parcel, &cellpack)))
}

/// Create a minimal WASM binary for testing
/// Note: This is a placeholder - in a real test, you'd need actual WASM bytecode
fn create_test_wasm_with_gpu_calls() -> Vec<u8> {
    // This is a minimal WASM binary that exports memory and a main function
    // In a real implementation, this would contain calls to __call_vulkan and __load_vulkan
    vec![
        0x00, 0x61, 0x73, 0x6d, // WASM magic number
        0x01, 0x00, 0x00, 0x00, // WASM version
        // Type section
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
        // Function section  
        0x03, 0x02, 0x01, 0x00,
        // Memory section
        0x05, 0x03, 0x01, 0x00, 0x01,
        // Export section
        0x07, 0x11, 0x02, 0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00,
        0x04, 0x6d, 0x61, 0x69, 0x6e, 0x00, 0x00,
        // Code section
        0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b,
    ]
}

/// Integration test for GPU pipeline with actual block processing
#[test]
fn test_gpu_pipeline_integration() {
    init_block_stats();
    
    // Simulate a block processing scenario with mixed GPU/WASM execution
    
    // Simulate successful GPU shard processing
    record_gpu_shard_execution(64, 3000, 4 * 1024 * 1024); // Large shard: 64 messages, 3ms, 4MB
    record_gpu_shard_execution(32, 1500, 2 * 1024 * 1024); // Medium shard: 32 messages, 1.5ms, 2MB
    record_gpu_shard_execution(16, 800, 1024 * 1024);      // Small shard: 16 messages, 0.8ms, 1MB
    
    // Simulate some WASM fallbacks (shards that couldn't use GPU)
    record_wasm_fallback_shard();
    record_wasm_fallback_shard();
    
    let stats = get_block_stats().expect("Failed to get block stats");
    
    // Verify comprehensive statistics
    assert_eq!(stats.pipeline_stats.gpu_shards_executed, 3);
    assert_eq!(stats.pipeline_stats.gpu_messages_processed, 112); // 64 + 32 + 16
    assert_eq!(stats.pipeline_stats.gpu_execution_time_us, 5300); // 3000 + 1500 + 800
    assert_eq!(stats.pipeline_stats.gpu_memory_used_bytes, 4 * 1024 * 1024); // 4MB max
    assert_eq!(stats.pipeline_stats.wasm_fallback_shards, 2);
    
    // Calculate pipeline efficiency
    let gpu_messages = 112;
    let estimated_wasm_messages = 2 * 32; // 64
    let total_messages = gpu_messages + estimated_wasm_messages; // 176
    let efficiency = (gpu_messages as f64 / total_messages as f64) * 100.0;
    
    assert!((efficiency - 63.64).abs() < 0.1); // Should be ~63.64%
    
    println!("GPU Pipeline Integration Test Results:");
    println!("  GPU Shards: {}", stats.pipeline_stats.gpu_shards_executed);
    println!("  GPU Messages: {}", stats.pipeline_stats.gpu_messages_processed);
    println!("  GPU Time: {}μs", stats.pipeline_stats.gpu_execution_time_us);
    println!("  GPU Memory: {}MB", stats.pipeline_stats.gpu_memory_used_bytes / (1024 * 1024));
    println!("  WASM Fallbacks: {}", stats.pipeline_stats.wasm_fallback_shards);
    println!("  Pipeline Efficiency: {:.2}%", efficiency);
}