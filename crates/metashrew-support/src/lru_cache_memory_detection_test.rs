//! Tests for LRU cache memory detection and graceful allocation
//!
//! This module tests that the LRU cache can gracefully handle resource-constrained
//! environments by detecting available memory and adjusting allocation accordingly.

#[cfg(test)]
mod tests {
    use super::super::lru_cache::{
        detect_available_memory, get_actual_lru_cache_memory_limit, initialize_lru_cache,
        clear_lru_cache, set_cache_allocation_mode, CacheAllocationMode,
        set_lru_cache, get_lru_cache, get_cache_stats, force_reinitialize_caches
    };
    use std::sync::Arc;

    #[test]
    fn test_memory_detection_functionality() {
        // Test that memory detection works and returns a reasonable value
        let detected_memory = detect_available_memory();
        
        // Should be at least 32MB (our minimum fallback)
        assert!(detected_memory >= 32 * 1024 * 1024, 
                "Detected memory should be at least 32MB, got: {} bytes", detected_memory);
        
        // Should be at most 1GB (our maximum target)
        assert!(detected_memory <= 1024 * 1024 * 1024, 
                "Detected memory should be at most 1GB, got: {} bytes", detected_memory);
        
        println!("✅ Memory detection returned: {} bytes ({} MB)", 
                 detected_memory, detected_memory / (1024 * 1024));
    }

    #[test]
    fn test_actual_memory_limit_consistency() {
        // Test that the actual memory limit is consistent across calls
        let limit1 = get_actual_lru_cache_memory_limit();
        let limit2 = get_actual_lru_cache_memory_limit();
        
        assert_eq!(limit1, limit2, "Memory limit should be consistent across calls");
        
        // Should be reasonable
        assert!(limit1 >= 32 * 1024 * 1024, "Memory limit should be at least 32MB");
        assert!(limit1 <= 1024 * 1024 * 1024, "Memory limit should be at most 1GB");
        
        println!("✅ Actual memory limit: {} bytes ({} MB)", limit1, limit1 / (1024 * 1024));
    }

    #[test]
    fn test_graceful_cache_initialization() {
        // Test that cache initialization works with detected memory limits
        set_cache_allocation_mode(CacheAllocationMode::Indexer);
        
        // Force complete reinitialization to avoid test interference
        force_reinitialize_caches();
        
        // This should not panic even on resource-constrained systems
        initialize_lru_cache();
        
        let actual_limit = get_actual_lru_cache_memory_limit();
        println!("Cache initialized with {} bytes ({} MB)",
                 actual_limit, actual_limit / (1024 * 1024));
        
        // Test basic cache operations work
        let key = Arc::new(b"test_key_memory_detection".to_vec());
        let value = Arc::new(b"test_value_memory_detection".to_vec());
        
        set_lru_cache(key.clone(), value.clone());
        
        // Debug: Check cache state before retrieval
        println!("Checking cache state after set...");
        // We can't access LRU_CACHE directly, so let's use the stats instead
        let stats_before = get_cache_stats();
        println!("Cache stats after set: items={}, memory={}", stats_before.items, stats_before.memory_usage);
        
        let retrieved = get_lru_cache(&key);
        println!("Retrieved value: {:?}", retrieved.is_some());
        assert_eq!(retrieved, Some(value), "Cache operations should work with detected memory limit");
        
        let stats = get_cache_stats();
        assert!(stats.items >= 1, "Cache should contain at least one item");
        
        println!("✅ Cache operations work correctly with detected memory limit");
    }

    #[test]
    fn test_cache_initialization_safety() {
        // Test that cache initialization doesn't panic on resource-constrained systems
        set_cache_allocation_mode(CacheAllocationMode::Indexer);
        
        // This should complete without panicking
        let result = std::panic::catch_unwind(|| {
            initialize_lru_cache();
        });
        
        assert!(result.is_ok(), "Cache initialization should not panic");
        
        println!("✅ Cache initialization completed safely");
    }

    #[test]
    fn test_cache_with_different_memory_limits() {
        // Test cache behavior with different memory scenarios
        set_cache_allocation_mode(CacheAllocationMode::Indexer);
        clear_lru_cache();
        initialize_lru_cache();
        
        let actual_limit = get_actual_lru_cache_memory_limit();
        
        // Add some data to test memory usage
        let test_data_size = (actual_limit / 100).max(1024); // Use 1% of available memory or at least 1KB
        let test_data = vec![42u8; test_data_size];
        
        for i in 0..10 {
            let key = Arc::new(format!("test_key_{}", i).into_bytes());
            let value = Arc::new(test_data.clone());
            set_lru_cache(key, value);
        }
        
        let stats = get_cache_stats();
        assert!(stats.items > 0, "Cache should contain items");
        assert!(stats.memory_usage > 0, "Cache should report memory usage");
        
        // Memory usage should be reasonable (not exceed our limit by too much)
        let memory_threshold = actual_limit + (actual_limit / 5); // Allow 20% overhead
        assert!(stats.memory_usage <= memory_threshold, 
                "Memory usage ({} bytes) should not significantly exceed limit ({} bytes)", 
                stats.memory_usage, actual_limit);
        
        println!("✅ Cache memory usage: {} bytes, limit: {} bytes, items: {}", 
                 stats.memory_usage, actual_limit, stats.items);
    }

    #[test]
    fn test_view_mode_memory_allocation() {
        // Test that view mode also works with detected memory limits
        set_cache_allocation_mode(CacheAllocationMode::View);
        clear_lru_cache();
        
        // Force reinitialization
        initialize_lru_cache();
        
        let actual_limit = get_actual_lru_cache_memory_limit();
        
        // In view mode, memory should be split between height-partitioned and API caches
        // Each should get roughly half the detected memory
        let expected_per_cache = actual_limit / 2;
        
        println!("✅ View mode initialized with {} bytes total ({} MB), {} bytes per cache ({} MB)", 
                 actual_limit, actual_limit / (1024 * 1024),
                 expected_per_cache, expected_per_cache / (1024 * 1024));
        
        // Reset to indexer mode for other tests
        set_cache_allocation_mode(CacheAllocationMode::Indexer);
        clear_lru_cache();
    }

    #[test]
    fn test_memory_detection_robustness() {
        // Test that memory detection handles edge cases gracefully
        
        // Call detection multiple times to ensure consistency
        let mut detected_values = Vec::new();
        for _ in 0..5 {
            detected_values.push(detect_available_memory());
        }
        
        // All values should be the same (detection should be deterministic)
        let first_value = detected_values[0];
        for &value in &detected_values {
            assert_eq!(value, first_value, "Memory detection should be deterministic");
        }
        
        // Values should be reasonable
        assert!(first_value >= 32 * 1024 * 1024, "Should detect at least 32MB");
        assert!(first_value <= 1024 * 1024 * 1024, "Should detect at most 1GB");
        
        println!("✅ Memory detection is robust and deterministic: {} bytes", first_value);
    }
}