//! Test shader for alkanes dependencies
//! 
//! This shader tests that our generified alkanes dependencies compile for SPIR-V

#![cfg_attr(target_arch = "spirv", no_std)]
#![cfg_attr(target_arch = "spirv", feature(lang_items))]

use spirv_std::spirv;

// Test alkanes-alloc
use alkanes_alloc::{AlkanesAllocator, AllocError, DefaultAllocator, default_allocator};

// Test bumpalo
use bumpalo::{Bump, AlkanesBumpError};

// Test leb128fmt
use leb128fmt::{max_len, is_last, encode_u32, decode_u32};

// Test arrayvec
use arrayvec::ArrayVec;

// Test memchr
use memchr::{memchr, memchr2, memchr3};

// Test wasmi integration
use wasmi::{Engine, Store, Module, Instance, Linker, Caller, Func, Value};
use wasmi_core::{ValueType, Pages};
use wasmi_collections::{Map, Set};

#[spirv(compute(threads(1)))]
pub fn test_alkanes_deps() {
    // Test alkanes-alloc
    let allocator = default_allocator();
    
    // Test alkanes-bumpalo
    let bump = Bump::new();
    let _capacity = bump.capacity();
    
    // Test alkanes-leb128fmt
    let _max_len = max_len::<32>();
    let _is_last = is_last(0x7F);
    
    // Test alkanes-arrayvec
    let mut arr: ArrayVec<u32, 4> = ArrayVec::new();
    let _len = arr.len();
    
    // Test alkanes-memchr
    let haystack = b"hello world";
    let _pos = memchr(b'w', haystack);
    let _pos2 = memchr2(b'w', b'o', haystack);
    let _pos3 = memchr3(b'w', b'o', b'r', haystack);
    
    // Test wasmi integration
    let engine = Engine::default();
    let _store = Store::new(&engine, ());
    
    // Test wasmi_core types
    let _value_type = ValueType::I32;
    let _pages = Pages::new(1).unwrap();
    
    // Test wasmi_collections
    let _map: Map<u32, u32> = Map::new();
    let _set: Set<u32> = Set::new();
}

#[spirv(compute(threads(1)))]
pub fn test_bump_allocator() {
    // Test bump allocator functionality
    let bump = Bump::new();
    
    // Test basic allocation interface
    let _allocated_bytes = bump.allocated_bytes();
    let _capacity = bump.capacity();
    
    // Test that we can create the allocator without panicking
    let _bump2 = Bump::with_capacity(1024);
}

#[spirv(compute(threads(1)))]
pub fn test_leb128_encoding() {
    // Test LEB128 encoding/decoding
    let _max_32 = max_len::<32>();
    let _max_64 = max_len::<64>();
    
    // Test byte checking
    let _is_continuation = !is_last(0x80);
    let _is_final = is_last(0x7F);
    
    // Test encoding (will return errors in SPIR-V stub, but should compile)
    let _encoded = encode_u32(42);
    let _decoded = decode_u32([0x2A, 0x00, 0x00, 0x00, 0x00]);
}

#[spirv(compute(threads(1)))]
pub fn test_arrayvec() {
    // Test ArrayVec functionality
    let mut arr: ArrayVec<u8, 16> = ArrayVec::new();
    
    // Test basic operations
    let _len = arr.len();
    let _capacity = arr.capacity();
    let _is_empty = arr.is_empty();
    let _is_full = arr.is_full();
    
    // Test that we can create different sized arrays
    let _arr2: ArrayVec<u32, 8> = ArrayVec::new();
    let _arr3: ArrayVec<u64, 4> = ArrayVec::new();
}

#[spirv(compute(threads(1)))]
pub fn test_memchr_search() {
    // Test memchr search functionality
    let haystack = b"hello world test";
    
    // Test single byte search
    let _pos1 = memchr(b'w', haystack);
    let _pos2 = memchr(b'z', haystack); // Not found
    
    // Test two byte search
    let _pos3 = memchr2(b'w', b'o', haystack);
    let _pos4 = memchr2(b'x', b'z', haystack); // Not found
    
    // Test three byte search
    let _pos5 = memchr3(b'w', b'o', b'r', haystack);
    let _pos6 = memchr3(b'x', b'y', b'z', haystack); // Not found
}

#[spirv(compute(threads(1)))]
pub fn test_wasmi_types() {
    // Test that wasmi types compile
    let engine = Engine::default();
    let _store = Store::new(&engine, ());
    
    // Test value types
    let _i32_type = ValueType::I32;
    let _i64_type = ValueType::I64;
    let _f32_type = ValueType::F32;
    let _f64_type = ValueType::F64;
    
    // Test values
    let _val1 = Value::I32(42);
    let _val2 = Value::I64(42);
    let _val3 = Value::F32(3.14);
    let _val4 = Value::F64(3.14159);
    
    // Test pages
    let _pages = Pages::new(1).unwrap_or(Pages::new(0).unwrap());
}

#[spirv(compute(threads(1)))]
pub fn test_wasmi_collections() {
    // Test wasmi collections
    let _map: Map<u32, u64> = Map::new();
    let _set: Set<u32> = Set::new();
    
    // Test different key/value types
    let _map2: Map<u64, u32> = Map::new();
    let _set2: Set<u64> = Set::new();
    
    // Test that we can create collections without panicking
    let _map3: Map<usize, usize> = Map::new();
    let _set3: Set<usize> = Set::new();
}

// Required for SPIR-V
#[cfg(target_arch = "spirv")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(target_arch = "spirv")]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}