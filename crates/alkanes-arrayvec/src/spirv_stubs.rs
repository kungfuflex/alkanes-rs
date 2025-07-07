//! SPIR-V stub implementations for arrayvec types
//! 
//! These provide minimal API-compatible stubs that panic when used,
//! allowing compilation for SPIR-V target while maintaining API compatibility.

use core::fmt;
use core::marker::PhantomData;

/// SPIR-V stub for CapacityError
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityError;

impl fmt::Display for CapacityError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CapacityError")
    }
}

/// SPIR-V stub for ArrayVec
pub struct ArrayVec<T, const CAP: usize> {
    _phantom: PhantomData<T>,
}

impl<T, const CAP: usize> ArrayVec<T, CAP> {
    pub fn new() -> Self {
        panic!("ArrayVec operations not supported in SPIR-V")
    }
    
    pub fn len(&self) -> usize {
        panic!("ArrayVec operations not supported in SPIR-V")
    }
    
    pub fn is_empty(&self) -> bool {
        panic!("ArrayVec operations not supported in SPIR-V")
    }
    
    pub fn capacity(&self) -> usize {
        panic!("ArrayVec operations not supported in SPIR-V")
    }
    
    pub fn push(&mut self, _element: T) -> Result<(), CapacityError> {
        panic!("ArrayVec operations not supported in SPIR-V")
    }
    
    pub fn pop(&mut self) -> Option<T> {
        panic!("ArrayVec operations not supported in SPIR-V")
    }
    
    pub fn clear(&mut self) {
        panic!("ArrayVec operations not supported in SPIR-V")
    }
    
    pub fn as_slice(&self) -> &[T] {
        panic!("ArrayVec operations not supported in SPIR-V")
    }
    
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        panic!("ArrayVec operations not supported in SPIR-V")
    }
}

impl<T, const CAP: usize> Default for ArrayVec<T, CAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const CAP: usize> Drop for ArrayVec<T, CAP> {
    fn drop(&mut self) {
        // No-op for SPIR-V
    }
}

/// SPIR-V stub for ArrayString
pub struct ArrayString<const CAP: usize> {
    _phantom: PhantomData<[u8; CAP]>,
}

impl<const CAP: usize> ArrayString<CAP> {
    pub fn new() -> Self {
        panic!("ArrayString operations not supported in SPIR-V")
    }
    
    pub fn len(&self) -> usize {
        panic!("ArrayString operations not supported in SPIR-V")
    }
    
    pub fn is_empty(&self) -> bool {
        panic!("ArrayString operations not supported in SPIR-V")
    }
    
    pub fn capacity(&self) -> usize {
        panic!("ArrayString operations not supported in SPIR-V")
    }
    
    pub fn clear(&mut self) {
        panic!("ArrayString operations not supported in SPIR-V")
    }
    
    pub fn as_str(&self) -> &str {
        panic!("ArrayString operations not supported in SPIR-V")
    }
    
    pub fn as_mut_str(&mut self) -> &mut str {
        panic!("ArrayString operations not supported in SPIR-V")
    }
}

impl<const CAP: usize> Default for ArrayString<CAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CAP: usize> Drop for ArrayString<CAP> {
    fn drop(&mut self) {
        // No-op for SPIR-V
    }
}

/// SPIR-V stub for IntoIter
pub struct IntoIter<T, const CAP: usize> {
    _phantom: PhantomData<T>,
}

impl<T, const CAP: usize> Iterator for IntoIter<T, CAP> {
    type Item = T;
    
    fn next(&mut self) -> Option<Self::Item> {
        panic!("IntoIter operations not supported in SPIR-V")
    }
}

/// SPIR-V stub for Drain
pub struct Drain<'a, T, const CAP: usize> {
    _phantom: PhantomData<&'a T>,
}

impl<'a, T, const CAP: usize> Iterator for Drain<'a, T, CAP> {
    type Item = T;
    
    fn next(&mut self) -> Option<Self::Item> {
        panic!("Drain operations not supported in SPIR-V")
    }
}

impl<'a, T, const CAP: usize> Drop for Drain<'a, T, CAP> {
    fn drop(&mut self) {
        // No-op for SPIR-V
    }
}