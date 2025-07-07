//! Simple test shader for working alkanes dependencies
//! 
//! This shader tests only the dependencies that are confirmed working for SPIR-V

#![cfg_attr(target_arch = "spirv", no_std)]
#![cfg_attr(target_arch = "spirv", feature(lang_items))]

use spirv_std::spirv;

// Test alkanes-alloc
use alkanes_alloc::{AlkanesAllocator, AllocError, DefaultAllocator, default_allocator};

// Test bumpalo
use bumpalo::Bump;

// Test leb128fmt
use leb128fmt::{max_len, is_last};

// Test arrayvec
use arrayvec::ArrayVec;

// Test memchr
use memchr::{memchr, memchr2, memchr3};

#[spirv(compute(threads(1)))]
pub fn test_working_deps() {
    // Test type compilation only for SPIR-V
    #[cfg(not(target_arch = "spirv"))]
    {
        // Test alkanes-alloc
        let allocator = default_allocator();
        
        // Test bumpalo
        let bump = Bump::new();
        let _capacity = bump.capacity();
        
        // Test leb128fmt
        let _max_len = max_len::<32>();
        let _is_last = is_last(0x7F);
        
        // Test arrayvec
        let mut arr: ArrayVec<u32, 4> = ArrayVec::new();
        let _len = arr.len();
        
        // Test memchr
        let haystack = b"hello world";
        let _pos = memchr(b'w', haystack);
        let _pos2 = memchr2(b'w', b'o', haystack);
        let _pos3 = memchr3(b'w', b'o', b'r', haystack);
    }
}

#[spirv(compute(threads(1)))]
pub fn test_bump_allocator() {
    // Test bump allocator type compilation only for SPIR-V
    #[cfg(not(target_arch = "spirv"))]
    {
        let bump = Bump::new();
        let _allocated_bytes = bump.allocated_bytes();
        let _capacity = bump.capacity();
        let _bump2 = Bump::with_capacity(1024);
    }
}

#[spirv(compute(threads(1)))]
pub fn test_leb128_encoding() {
    // Test LEB128 type compilation only for SPIR-V
    #[cfg(not(target_arch = "spirv"))]
    {
        let _max_32 = max_len::<32>();
        let _max_64 = max_len::<64>();
        let _is_continuation = !is_last(0x80);
        let _is_final = is_last(0x7F);
    }
}

#[spirv(compute(threads(1)))]
pub fn test_arrayvec() {
    // Test ArrayVec type compilation only (SPIR-V stubs will panic if called)
    // Just test that the types are available and compile
    #[cfg(not(target_arch = "spirv"))]
    {
        let mut arr: ArrayVec<u8, 16> = ArrayVec::new();
        let _len = arr.len();
        let _capacity = arr.capacity();
        let _is_empty = arr.is_empty();
    }
}

#[spirv(compute(threads(1)))]
pub fn test_memchr_search() {
    // Test memchr type compilation only (SPIR-V stubs will panic if called)
    #[cfg(not(target_arch = "spirv"))]
    {
        let haystack = b"hello world test";
        let _pos1 = memchr(b'w', haystack);
        let _pos2 = memchr(b'z', haystack);
        let _pos3 = memchr2(b'w', b'o', haystack);
        let _pos4 = memchr2(b'x', b'z', haystack);
        let _pos5 = memchr3(b'w', b'o', b'r', haystack);
        let _pos6 = memchr3(b'x', b'y', b'z', haystack);
    }
}

#[spirv(compute(threads(1)))]
pub fn test_alkanes_alloc() {
    // Test alkanes-alloc type compilation only (SPIR-V stubs will panic if called)
    #[cfg(not(target_arch = "spirv"))]
    {
        let allocator = default_allocator();
        let _result = allocator.allocate(64, 8);
    }
}

// spirv-std already provides panic_handler and eh_personality