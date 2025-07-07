//! Test shader for alkanes-sync with SPIR-V target
//! 
//! This shader tests the synchronization primitives work in SPIR-V.

#![no_std]
#![cfg_attr(target_arch = "spirv", no_main)]

use spirv_std::spirv;
use alkanes_sync::{DefaultMutex, DefaultArc, DefaultOnceCell, AlkanesMutex, AlkanesArc, AlkanesOnceCell};

#[spirv(compute(threads(64)))]
pub fn main_cs() {
    // Test mutex
    test_mutex();
    
    // Test arc
    test_arc();
    
    // Test once cell
    test_once_cell();
}

fn test_mutex() {
    let mutex = DefaultMutex::new(42u32);
    let guard = mutex.lock();
    let _value = *guard;
}

fn test_arc() {
    let arc = DefaultArc::new(42u32);
    let arc2 = AlkanesArc::clone(&arc);
    let _value1 = *arc.as_ref();
    let _value2 = *arc2.as_ref();
}

fn test_once_cell() {
    let cell = DefaultOnceCell::new();
    let _value = cell.get_or_init(|| 42u32);
}

// Entry point for non-SPIR-V targets (for testing compilation)
#[cfg(not(target_arch = "spirv"))]
fn main() {
    // Test completed successfully - no output needed in no_std
}