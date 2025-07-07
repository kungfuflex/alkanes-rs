//! Test shader for alkanes-wasmi with SPIR-V target
//! 
//! This shader tests that our generified collections work in SPIR-V.

#![no_std]
#![cfg_attr(target_arch = "spirv", no_main)]

use spirv_std::spirv;
use alkanes_alloc::{DefaultAllocator, AlkanesAllocator};
use alkanes_sync::{DefaultMutex, DefaultArc, DefaultOnceCell, DefaultRwLock, AlkanesMutex, AlkanesArc, AlkanesOnceCell, AlkanesRwLock};
use wasmi::{Engine, Config};

#[spirv(compute(threads(64)))]
pub fn main_cs() {
    // Test allocator functionality
    test_allocator();
    
    // Test synchronization primitives
    test_sync();
    
    // Test read-write locks
    test_rwlock();
    
    // Test alkanes-wasmi
    test_wasmi();
}

fn test_allocator() {
    // Test that our allocator can be created
    let _allocator = DefaultAllocator::default();
    
    // Test basic allocation (this is a simple test since we can't do much with raw pointers in SPIR-V)
    // In a real scenario, this would be used by Arena or other data structures
}

fn test_sync() {
    // Test mutex
    let mutex = DefaultMutex::new(42i32);
    let guard = mutex.lock();
    let _value = *guard;
    
    // Test arc
    let arc = DefaultArc::new(42i32);
    let arc2 = AlkanesArc::clone(&arc);
    let _value1 = *arc.as_ref();
    let _value2 = *arc2.as_ref();
    
    // Test once cell with compile-time initialization for SPIR-V
    let cell = DefaultOnceCell::with_value(42i32);
    let _ = cell;
}

fn test_rwlock() {
    // Test read-write lock
    let rwlock = DefaultRwLock::new(42i32);
    
    // Test read access
    let read_guard = rwlock.read();
    let _value = *read_guard;
    drop(read_guard);
    
    // Test write access
    let mut write_guard = rwlock.write();
    *write_guard = 84;
    let _new_value = *write_guard;
    drop(write_guard);
}

fn test_wasmi() {
    // Test basic wasmi engine creation
    let config = Config::default();
    let _engine = Engine::new(&config);
}

// Entry point for non-SPIR-V targets (for testing compilation)
#[cfg(not(target_arch = "spirv"))]
fn main() {
    // Test completed successfully - no output needed in no_std
}