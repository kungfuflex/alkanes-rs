//! Test to verify snapshot interval functionality

use anyhow::Result;

#[test]
fn test_snapshot_interval_logic() {
    // Test the basic logic that should be used for snapshot intervals
    
    // Test interval of 10 blocks
    let interval = 10;
    
    // Test various heights
    assert!(!should_create_snapshot_at_height(0, interval), "Should not create snapshot at height 0");
    assert!(!should_create_snapshot_at_height(5, interval), "Should not create snapshot at height 5");
    assert!(should_create_snapshot_at_height(10, interval), "Should create snapshot at height 10");
    assert!(!should_create_snapshot_at_height(15, interval), "Should not create snapshot at height 15");
    assert!(should_create_snapshot_at_height(20, interval), "Should create snapshot at height 20");
    assert!(should_create_snapshot_at_height(30, interval), "Should create snapshot at height 30");
}

#[test]
fn test_snapshot_interval_100_blocks() {
    // Test interval of 100 blocks
    let interval = 100;
    
    // Test various heights
    assert!(!should_create_snapshot_at_height(0, interval), "Should not create snapshot at height 0");
    assert!(!should_create_snapshot_at_height(50, interval), "Should not create snapshot at height 50");
    assert!(should_create_snapshot_at_height(100, interval), "Should create snapshot at height 100");
    assert!(!should_create_snapshot_at_height(150, interval), "Should not create snapshot at height 150");
    assert!(should_create_snapshot_at_height(200, interval), "Should create snapshot at height 200");
}

#[test]
fn test_current_hardcoded_logic() {
    // Test the current hardcoded logic that's causing the issue
    let hardcoded_interval = 1000;
    
    // This demonstrates the current broken behavior
    assert!(!should_create_snapshot_at_height(10, hardcoded_interval), "Current logic: Should NOT create snapshot at height 10 with interval 1000");
    assert!(!should_create_snapshot_at_height(100, hardcoded_interval), "Current logic: Should NOT create snapshot at height 100 with interval 1000");
    assert!(should_create_snapshot_at_height(1000, hardcoded_interval), "Current logic: Should create snapshot at height 1000 with interval 1000");
    
    // But if we pass --snapshot-interval 10, we want snapshots every 10 blocks, not every 1000
    let desired_interval = 10;
    assert!(should_create_snapshot_at_height(10, desired_interval), "Desired behavior: Should create snapshot at height 10 with interval 10");
    assert!(should_create_snapshot_at_height(20, desired_interval), "Desired behavior: Should create snapshot at height 20 with interval 10");
}

/// This is the correct logic that should be used for snapshot intervals
fn should_create_snapshot_at_height(height: u32, interval: u32) -> bool {
    if height == 0 || interval == 0 {
        return false;
    }
    height % interval == 0
}

/// This simulates the current broken logic in the codebase
fn current_broken_logic(height: u32) -> bool {
    // This is what's currently implemented in snapshot_adapters.rs:229
    height > 0 && height % 1000 == 0
}

#[test]
fn test_demonstrate_the_bug() {
    // This test demonstrates the bug: the interval parameter is ignored
    
    // User passes --snapshot-interval 10
    let user_specified_interval = 10;
    
    // But the current implementation ignores it and uses hardcoded 1000
    assert!(!current_broken_logic(10), "BUG: Current implementation ignores user interval and doesn't create snapshot at height 10");
    assert!(!current_broken_logic(20), "BUG: Current implementation ignores user interval and doesn't create snapshot at height 20");
    assert!(!current_broken_logic(100), "BUG: Current implementation ignores user interval and doesn't create snapshot at height 100");
    
    // Only creates snapshots at multiples of 1000
    assert!(current_broken_logic(1000), "Current implementation only creates snapshots at height 1000");
    assert!(current_broken_logic(2000), "Current implementation only creates snapshots at height 2000");
    
    // What the user expects with --snapshot-interval 10
    assert!(should_create_snapshot_at_height(10, user_specified_interval), "User expects snapshot at height 10");
    assert!(should_create_snapshot_at_height(20, user_specified_interval), "User expects snapshot at height 20");
    assert!(should_create_snapshot_at_height(30, user_specified_interval), "User expects snapshot at height 30");
}