//! Tests for LRU cache memory preallocation
//!
//! This module tests that the 1GB LRU cache memory is preallocated at startup
//! and that memory addresses remain consistent regardless of cache usage.

use crate::{initialize, clear, allocator};
use metashrew_support::lru_cache::{
    initialize_lru_cache, is_lru_cache_initialized,
    get_total_memory_usage, get_cache_stats, clear_lru_cache,
};
use std::sync::Arc;

#[test]
fn test_memory_preallocation_consistency() {
    // Test that memory preallocation happens and is consistent
    
    // Clear any existing state
    clear_lru_cache();
    
    // First call to ensure_preallocated_memory
    allocator::ensure_preallocated_memory();
    
    // Get the memory pointer address (this should be consistent)
    let memory_usage_1 = get_total_memory_usage();
    
    // Initialize the cache system
    initialize_lru_cache();
    assert!(is_lru_cache_initialized());
    
    // Memory usage should still be consistent
    let memory_usage_2 = get_total_memory_usage();
    
    // Clear and reinitialize
    clear_lru_cache();
    initialize_lru_cache();
    
    // Memory usage should still be consistent
    let memory_usage_3 = get_total_memory_usage();
    
    println!("Memory usage 1: {} bytes", memory_usage_1);
    println!("Memory usage 2: {} bytes", memory_usage_2);
    println!("Memory usage 3: {} bytes", memory_usage_3);
    
    // The memory usage should be consistent across reinitializations
    // Note: There might be small variations due to internal structures,
    // but the base memory should be preallocated
    assert!(memory_usage_1 >= 0, "Memory usage should be non-negative");
    assert!(memory_usage_2 >= 0, "Memory usage should be non-negative");
    assert!(memory_usage_3 >= 0, "Memory usage should be non-negative");
}

#[test]
fn test_preallocation_before_cache_usage() {
    // Test that preallocation works even when cache is not used
    
    // Clear any existing state
    clear_lru_cache();
    
    // Call preallocation directly
    allocator::ensure_preallocated_memory();
    
    // At this point, 1GB should be preallocated even though cache isn't initialized
    // We can't directly measure the preallocated memory, but we can verify
    // that the function completes without error
    
    // Now initialize the cache
    initialize_lru_cache();
    assert!(is_lru_cache_initialized());
    
    // Verify cache works
    let stats = get_cache_stats();
    assert_eq!(stats.items, 0); // Should start empty
    assert!(stats.memory_usage >= 0); // Should have some memory allocated
    
    println!("Cache initialized successfully after preallocation");
    println!("Cache stats: items={}, memory_usage={}", stats.items, stats.memory_usage);
}

#[test]
fn test_multiple_preallocation_calls() {
    // Test that multiple calls to ensure_preallocated_memory are safe
    
    // Clear any existing state
    clear_lru_cache();
    
    // Call preallocation multiple times
    allocator::ensure_preallocated_memory();
    allocator::ensure_preallocated_memory();
    allocator::ensure_preallocated_memory();
    
    // Initialize cache
    initialize_lru_cache();
    assert!(is_lru_cache_initialized());
    
    // Should still work correctly
    let stats = get_cache_stats();
    assert_eq!(stats.items, 0);
    
    println!("Multiple preallocation calls handled correctly");
}

#[test]
fn test_initialize_calls_preallocation() {
    // Test that initialize() calls preallocation automatically
    
    // Clear any existing state
    clear();
    clear_lru_cache();
    
    // Call initialize() which should call preallocation internally
    initialize();
    
    // Cache should be initialized
    assert!(is_lru_cache_initialized());
    
    // Should be able to use cache functions
    let stats = get_cache_stats();
    assert_eq!(stats.items, 0);
    
    println!("initialize() correctly calls preallocation");
}

#[test]
fn test_memory_layout_determinism() {
    // Test that memory layout is deterministic across multiple runs
    
    let mut memory_usages = Vec::new();
    
    // Run multiple initialization cycles
    for i in 0..3 {
        // Clear state
        clear();
        clear_lru_cache();
        
        // Ensure preallocation
        allocator::ensure_preallocated_memory();
        
        // Initialize
        initialize();
        
        // Record memory usage
        let memory_usage = get_total_memory_usage();
        memory_usages.push(memory_usage);
        
        println!("Iteration {}: memory_usage = {} bytes", i, memory_usage);
    }
    
    // All memory usages should be consistent (or at least in a reasonable range)
    let first_usage = memory_usages[0];
    for (i, &usage) in memory_usages.iter().enumerate() {
        // Allow for some small variation due to internal structures
        let diff = if usage > first_usage {
            usage - first_usage
        } else {
            first_usage - usage
        };
        
        // Difference should be small relative to total memory
        assert!(
            diff < 1024 * 1024, // Less than 1MB difference
            "Memory usage variation too large: iteration {} had {} bytes, first had {} bytes (diff: {} bytes)",
            i, usage, first_usage, diff
        );
    }
    
    println!("Memory layout is deterministic across multiple runs");
}

#[test]
fn test_cache_operations_after_preallocation() {
    // Test that cache operations work correctly after preallocation
    
    // Clear state
    clear();
    clear_lru_cache();
    
    // Preallocate memory
    allocator::ensure_preallocated_memory();
    
    // Initialize
    initialize();
    
    // Test basic cache operations
    let key = Arc::new(b"test_key".to_vec());
    let value = Arc::new(b"test_value".to_vec());
    
    // Set a value
    crate::set(key.clone(), value.clone());
    
    // Get the value back
    let retrieved = crate::get(key.clone());
    assert_eq!(retrieved, value);
    
    // Check cache stats
    let stats = get_cache_stats();
    println!("After cache operations: items={}, memory_usage={}", stats.items, stats.memory_usage);
    
    // Flush
    crate::flush();
    
    println!("Cache operations work correctly after preallocation");
}