//! Tests for the custom preallocated allocator functionality
//!
//! This test suite verifies that the custom bump allocator works correctly
//! and provides deterministic memory layout for WASM execution.

use std::sync::Arc;
use crate::allocator::{
    get_allocator_usage_stats, enable_preallocated_allocator, disable_preallocated_allocator,
    shutdown_preallocated_allocator, is_preallocated_allocator_enabled
};
use crate::{initialize, set, get, clear, lru_cache_stats, lru_cache_memory_usage};
use metashrew_support::lru_cache::{set_cache_allocation_mode, CacheAllocationMode};

#[test]
fn test_allocator_basic_functionality() {
    println!("🧪 Testing Custom Allocator Basic Functionality");
    println!("═══════════════════════════════════════════════════════════════");

    // Initialize metashrew-core (this will enable the allocator)
    initialize();
    
    // Check allocator stats
    let (used, total, percentage) = get_allocator_usage_stats();
    println!("📋 Allocator Status:");
    println!("├── Initial usage: {} bytes / {} bytes ({:.1}%)", used, total, percentage);
    println!("└── Total preallocated: {:.1} MB", total as f64 / (1024.0 * 1024.0));
    
    // Verify we have some preallocated memory
    assert!(total > 0, "Total preallocated memory should be greater than 0");
    assert!(is_preallocated_allocator_enabled(), "Allocator should be enabled after initialize()");
    
    // Clean up
    shutdown_preallocated_allocator();
    
    println!("✅ Basic allocator functionality test passed!");
}

#[test]
fn test_cache_operations_with_allocator() {
    println!("🧪 Testing Cache Operations with Allocator");
    println!("═══════════════════════════════════════════════════════════════");

    // Initialize metashrew-core
    initialize();
    clear(); // Start with clean cache
    
    let initial_cache_memory = lru_cache_memory_usage();
    println!("📋 Initial cache memory: {} bytes", initial_cache_memory);
    
    // Add test data to the cache
    println!("💾 Adding test data to cache...");
    let test_entries = 5;
    let entry_size = 10_000; // 10KB per entry
    
    for i in 0..test_entries {
        let key = Arc::new(format!("allocator_test_key_{}", i).into_bytes());
        let value = Arc::new(vec![42u8; entry_size]);
        set(key, value);
    }
    
    // Verify cache memory increased
    let final_cache_memory = lru_cache_memory_usage();
    println!("📊 Final cache memory: {} bytes", final_cache_memory);
    
    // Memory usage should have increased
    assert!(final_cache_memory > initial_cache_memory, 
            "Cache memory usage should have increased after adding entries");
    
    // Test cache operations work correctly
    println!("🔍 Testing cache operations...");
    let test_key = Arc::new(format!("allocator_test_key_2").into_bytes());
    let retrieved_value = get(test_key);
    
    println!("✅ Cache retrieval: found value with {} bytes", retrieved_value.len());
    assert_eq!(retrieved_value.len(), entry_size, "Retrieved value should have correct size");
    
    // Verify cache statistics
    let cache_stats = lru_cache_stats();
    println!("📈 Cache statistics: {} items, {} hits, {} misses", 
             cache_stats.items, cache_stats.hits, cache_stats.misses);
    
    // Clean up
    shutdown_preallocated_allocator();
    
    println!("✅ Cache operations with allocator test passed!");
}

#[test]
fn test_cache_memory_consistency() {
    println!("🧪 Testing Cache Memory Consistency");
    println!("═══════════════════════════════════════════════════════════════");

    // Initialize metashrew-core
    initialize();
    clear(); // Start with clean cache
    
    // Add test data and verify memory tracking is consistent
    let test_data_size = 50_000; // 50KB
    let key = Arc::new(b"consistency_test_key".to_vec());
    let value = Arc::new(vec![42u8; test_data_size]);
    
    let before_cache_memory = lru_cache_memory_usage();
    
    set(key.clone(), value);
    
    let after_cache_memory = lru_cache_memory_usage();
    
    println!("📊 Memory usage before: cache={} bytes", before_cache_memory);
    println!("📊 Memory usage after: cache={} bytes", after_cache_memory);
    
    // Cache memory usage should have increased
    assert!(after_cache_memory > before_cache_memory, "Cache memory usage should increase after adding data");
    
    // The increase should be reasonable (at least the data size)
    let cache_increase = after_cache_memory - before_cache_memory;
    println!("📈 Cache memory increase: +{} bytes", cache_increase);
    
    assert!(cache_increase >= test_data_size, "Cache increase should be at least the data size");
    
    // Verify we can retrieve the data
    let retrieved_value = get(key);
    assert_eq!(retrieved_value.len(), test_data_size, "Retrieved value should have correct size");
    println!("✅ Data retrieval successful");
    
    // Clean up
    shutdown_preallocated_allocator();
    
    println!("✅ Memory consistency test passed!");
}

#[test]
fn test_allocator_mode_configuration() {
    println!("🧪 Testing Allocator Mode Configuration");
    println!("═══════════════════════════════════════════════════════════════");

    // Test indexer mode configuration (default for metashrew-core)
    initialize(); // This sets indexer mode and enables allocator
    
    let (used, total, _) = get_allocator_usage_stats();
    println!("📋 Indexer mode - Allocator: {} bytes / {} bytes", used, total);
    
    // Should have preallocated memory in indexer mode
    assert!(total > 0, "Should have preallocated memory in indexer mode");
    assert!(is_preallocated_allocator_enabled(), "Allocator should be enabled in indexer mode");
    
    println!("✅ Indexer mode: allocator configured correctly");
    
    // Test view mode configuration
    set_cache_allocation_mode(CacheAllocationMode::View);
    disable_preallocated_allocator();
    
    assert!(!is_preallocated_allocator_enabled(), "Allocator should be disabled in view mode");
    println!("✅ View mode: allocator configured for view operations");
    
    // Reset to indexer mode
    set_cache_allocation_mode(CacheAllocationMode::Indexer);
    enable_preallocated_allocator();
    
    // Clean up
    shutdown_preallocated_allocator();
    
    println!("✅ Mode configuration test passed!");
}

#[test]
fn test_allocator_integration_with_core() {
    println!("🧪 Testing Allocator Integration with Core Functions");
    println!("═══════════════════════════════════════════════════════════════");

    // Test that metashrew-core functions work properly with the allocator
    initialize();
    clear();
    
    // Test basic core operations
    let key1 = Arc::new(b"test_key_1".to_vec());
    let value1 = Arc::new(b"test_value_1".to_vec());
    
    set(key1.clone(), value1.clone());
    let retrieved1 = get(key1);
    
    assert_eq!(retrieved1, value1, "Core set/get should work with allocator");
    
    // Test multiple operations
    for i in 0..10 {
        let key = Arc::new(format!("integration_key_{}", i).into_bytes());
        let value = Arc::new(format!("integration_value_{}", i).into_bytes());
        set(key.clone(), value.clone());
        
        let retrieved = get(key);
        assert_eq!(retrieved, value, "Multiple operations should work correctly");
    }
    
    // Verify allocator is still working
    let (used, total, _) = get_allocator_usage_stats();
    assert!(used > 0, "Allocator should have been used");
    assert!(total > 0, "Allocator should have preallocated memory");
    
    println!("📋 Final allocator usage: {} bytes / {} bytes", used, total);
    
    // Clean up
    shutdown_preallocated_allocator();
    
    println!("✅ Allocator integration test passed!");
}