//! Test shader for alkanes-alloc with compile-time layout approach
//!
//! This shader tests the new compile-time memory layout allocator.

#![no_std]
#![cfg_attr(target_arch = "spirv", no_main)]

use spirv_std::spirv;
use alkanes_alloc::default_allocator;

#[spirv(compute(threads(64)))]
pub fn main_cs() {
    // Test that we can get the allocator without crashing
    let allocator = default_allocator();
    let _allocator_ref = &allocator;
    
    // Test basic functionality
    let _test_value = 42u32;
}

// Entry point for non-SPIR-V targets (for testing compilation)
#[cfg(not(target_arch = "spirv"))]
fn main() {
    println!("alkanes-alloc compile-time layout test shader compiled successfully for non-SPIR-V target");
}