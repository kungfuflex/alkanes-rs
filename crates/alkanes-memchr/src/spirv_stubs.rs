//! SPIR-V stub implementations for memchr functionality
//! 
//! These provide minimal API-compatible stubs that panic when used,
//! allowing compilation for SPIR-V target while maintaining API compatibility.

use core::marker::PhantomData;

/// Search for a single byte in a haystack (SPIR-V stub)
pub fn memchr(_needle: u8, _haystack: &[u8]) -> Option<usize> {
    panic!("memchr operations not supported in SPIR-V")
}

/// Search for any of two bytes in a haystack (SPIR-V stub)
pub fn memchr2(_needle1: u8, _needle2: u8, _haystack: &[u8]) -> Option<usize> {
    panic!("memchr operations not supported in SPIR-V")
}

/// Search for any of three bytes in a haystack (SPIR-V stub)
pub fn memchr3(_needle1: u8, _needle2: u8, _needle3: u8, _haystack: &[u8]) -> Option<usize> {
    panic!("memchr operations not supported in SPIR-V")
}

/// Reverse search for a single byte in a haystack (SPIR-V stub)
pub fn memrchr(_needle: u8, _haystack: &[u8]) -> Option<usize> {
    panic!("memchr operations not supported in SPIR-V")
}

/// Reverse search for any of two bytes in a haystack (SPIR-V stub)
pub fn memrchr2(_needle1: u8, _needle2: u8, _haystack: &[u8]) -> Option<usize> {
    panic!("memchr operations not supported in SPIR-V")
}

/// Reverse search for any of three bytes in a haystack (SPIR-V stub)
pub fn memrchr3(_needle1: u8, _needle2: u8, _needle3: u8, _haystack: &[u8]) -> Option<usize> {
    panic!("memchr operations not supported in SPIR-V")
}

/// Create iterator for single byte search (SPIR-V stub)
pub fn memchr_iter(_needle: u8, _haystack: &[u8]) -> Memchr {
    panic!("memchr operations not supported in SPIR-V")
}

/// Create iterator for two byte search (SPIR-V stub)
pub fn memchr2_iter(_needle1: u8, _needle2: u8, _haystack: &[u8]) -> Memchr2 {
    panic!("memchr operations not supported in SPIR-V")
}

/// Create iterator for three byte search (SPIR-V stub)
pub fn memchr3_iter(_needle1: u8, _needle2: u8, _needle3: u8, _haystack: &[u8]) -> Memchr3 {
    panic!("memchr operations not supported in SPIR-V")
}

/// Create reverse iterator for single byte search (SPIR-V stub)
pub fn memrchr_iter(_needle: u8, _haystack: &[u8]) -> Memchr {
    panic!("memchr operations not supported in SPIR-V")
}

/// Create reverse iterator for two byte search (SPIR-V stub)
pub fn memrchr2_iter(_needle1: u8, _needle2: u8, _haystack: &[u8]) -> Memchr2 {
    panic!("memchr operations not supported in SPIR-V")
}

/// Create reverse iterator for three byte search (SPIR-V stub)
pub fn memrchr3_iter(_needle1: u8, _needle2: u8, _needle3: u8, _haystack: &[u8]) -> Memchr3 {
    panic!("memchr operations not supported in SPIR-V")
}

/// SPIR-V stub for Memchr iterator
pub struct Memchr<'h> {
    _phantom: PhantomData<&'h [u8]>,
}

impl<'h> Iterator for Memchr<'h> {
    type Item = usize;
    
    fn next(&mut self) -> Option<Self::Item> {
        panic!("memchr operations not supported in SPIR-V")
    }
}

impl<'h> DoubleEndedIterator for Memchr<'h> {
    fn next_back(&mut self) -> Option<Self::Item> {
        panic!("memchr operations not supported in SPIR-V")
    }
}

/// SPIR-V stub for Memchr2 iterator
pub struct Memchr2<'h> {
    _phantom: PhantomData<&'h [u8]>,
}

impl<'h> Iterator for Memchr2<'h> {
    type Item = usize;
    
    fn next(&mut self) -> Option<Self::Item> {
        panic!("memchr operations not supported in SPIR-V")
    }
}

impl<'h> DoubleEndedIterator for Memchr2<'h> {
    fn next_back(&mut self) -> Option<Self::Item> {
        panic!("memchr operations not supported in SPIR-V")
    }
}

/// SPIR-V stub for Memchr3 iterator
pub struct Memchr3<'h> {
    _phantom: PhantomData<&'h [u8]>,
}

impl<'h> Iterator for Memchr3<'h> {
    type Item = usize;
    
    fn next(&mut self) -> Option<Self::Item> {
        panic!("memchr operations not supported in SPIR-V")
    }
}

impl<'h> DoubleEndedIterator for Memchr3<'h> {
    fn next_back(&mut self) -> Option<Self::Item> {
        panic!("memchr operations not supported in SPIR-V")
    }
}