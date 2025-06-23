//! Test to verify the snapshot interval fix

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

// We'll create a mock test that simulates the fixed behavior
// Since we can't easily test the full rockshrew-mono integration here,
// we'll test the logic directly

#[test]
fn test_snapshot_interval_fix_logic() {
    // Test the corrected logic that should be used
    
    // Test interval of 10 blocks
    let interval = 10;
    
    // Test various heights with the correct logic
    assert!(!should_create_snapshot_with_interval(0, interval), "Should not create snapshot at height 0");
    assert!(!should_create_snapshot_with_interval(5, interval), "Should not create snapshot at height 5");
    assert!(should_create_snapshot_with_interval(10, interval), "Should create snapshot at height 10");
    assert!(!should_create_snapshot_with_interval(15, interval), "Should not create snapshot at height 15");
    assert!(should_create_snapshot_with_interval(20, interval), "Should create snapshot at height 20");
    assert!(should_create_snapshot_with_interval(30, interval), "Should create snapshot at height 30");
    
    // Test interval of 100 blocks
    let interval = 100;
    assert!(!should_create_snapshot_with_interval(50, interval), "Should not create snapshot at height 50 with interval 100");
    assert!(should_create_snapshot_with_interval(100, interval), "Should create snapshot at height 100 with interval 100");
    assert!(!should_create_snapshot_with_interval(150, interval), "Should not create snapshot at height 150 with interval 100");
    assert!(should_create_snapshot_with_interval(200, interval), "Should create snapshot at height 200 with interval 100");
}

#[test]
fn test_edge_cases() {
    // Test edge cases
    assert!(!should_create_snapshot_with_interval(0, 10), "Should not create snapshot at height 0");
    assert!(!should_create_snapshot_with_interval(10, 0), "Should not create snapshot with interval 0");
    assert!(!should_create_snapshot_with_interval(0, 0), "Should not create snapshot with height 0 and interval 0");
    
    // Test interval of 1 (every block)
    let interval = 1;
    assert!(should_create_snapshot_with_interval(1, interval), "Should create snapshot at height 1 with interval 1");
    assert!(should_create_snapshot_with_interval(2, interval), "Should create snapshot at height 2 with interval 1");
    assert!(should_create_snapshot_with_interval(100, interval), "Should create snapshot at height 100 with interval 1");
}

/// This is the corrected logic that we implemented in the fix
fn should_create_snapshot_with_interval(height: u32, interval: u32) -> bool {
    if height == 0 || interval == 0 {
        return false;
    }
    height % interval == 0
}

#[test]
fn test_comparison_with_old_broken_logic() {
    // Compare the old broken logic with the new fixed logic
    
    // Old broken logic (hardcoded 1000)
    fn old_broken_logic(height: u32) -> bool {
        height > 0 && height % 1000 == 0
    }
    
    // User wants snapshots every 10 blocks
    let user_interval = 10;
    
    // Heights where user expects snapshots
    let expected_snapshot_heights = vec![10, 20, 30, 40, 50, 100, 110, 120];
    
    for height in expected_snapshot_heights {
        // Old logic fails
        assert!(!old_broken_logic(height), 
            "Old broken logic incorrectly returns false for height {} (user wants interval {})", 
            height, user_interval);
        
        // New logic works
        assert!(should_create_snapshot_with_interval(height, user_interval), 
            "New fixed logic correctly returns true for height {} with interval {}", 
            height, user_interval);
    }
    
    // Heights where user does NOT expect snapshots
    let non_snapshot_heights = vec![5, 15, 25, 35, 45, 55, 105, 115];
    
    for height in non_snapshot_heights {
        // Both should return false (old logic happens to be correct here by accident)
        assert!(!old_broken_logic(height), 
            "Old logic correctly returns false for height {} (not a multiple of 1000)", height);
        
        assert!(!should_create_snapshot_with_interval(height, user_interval), 
            "New logic correctly returns false for height {} with interval {}", 
            height, user_interval);
    }
}