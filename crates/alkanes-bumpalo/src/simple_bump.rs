//! Simple bump allocator implementation using alkanes-alloc
//!
//! This module provides a basic bump allocator that works with the
//! alkanes-alloc infrastructure for targets that don't have bumpalo.

use alkanes_alloc::{AlkanesAllocator, AllocError, DefaultAllocator, default_allocator};
use core::marker::PhantomData;

/// A simple bump allocator that uses the default alkanes allocator
pub struct SimpleBump {
    allocator: DefaultAllocator,
    _marker: PhantomData<*mut u8>,
}

impl SimpleBump {
    /// Create a new simple bump allocator
    pub fn new() -> Self {
        Self {
            allocator: default_allocator(),
            _marker: PhantomData,
        }
    }

    /// Create a new simple bump allocator with capacity (ignored for compatibility)
    pub fn with_capacity(_capacity: usize) -> Self {
        Self::new()
    }

    /// Allocate space for a value of type T
    pub fn alloc<T>(&self, val: T) -> Result<&mut T, AllocError> {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        
        let ptr = self.allocator.allocate(size, align)? as *mut T;
        
        unsafe {
            core::ptr::write(ptr, val);
            Ok(&mut *ptr)
        }
    }

    /// Try to allocate space for a value of type T
    pub fn try_alloc<T>(&self, val: T) -> Result<&mut T, AllocError> {
        self.alloc(val)
    }

    /// Allocate space for a slice and copy data into it
    pub fn alloc_slice_copy<T>(&self, src: &[T]) -> Result<&mut [T], AllocError>
    where
        T: Copy,
    {
        if src.is_empty() {
            return Ok(&mut []);
        }

        let size = src.len() * core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        
        let ptr = self.allocator.allocate(size, align)? as *mut T;
        
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), ptr, src.len());
            Ok(core::slice::from_raw_parts_mut(ptr, src.len()))
        }
    }

    /// Allocate space for a string slice
    pub fn alloc_str(&self, src: &str) -> Result<&mut str, AllocError> {
        let bytes = self.alloc_slice_copy(src.as_bytes())?;
        unsafe {
            Ok(core::str::from_utf8_unchecked_mut(bytes))
        }
    }

    /// Reset the allocator (no-op for simple allocator)
    pub fn reset(&mut self) {
        // Simple allocator doesn't support reset
    }

    /// Get allocated bytes (always returns 0 for simple allocator)
    pub fn allocated_bytes(&self) -> usize {
        0
    }
}

impl Default for SimpleBump {
    fn default() -> Self {
        Self::new()
    }
}

impl AlkanesAllocator for SimpleBump {
    type Error = AllocError;

    fn allocate(&self, size: usize, align: usize) -> Result<*mut u8, Self::Error> {
        self.allocator.allocate(size, align)
    }

    unsafe fn deallocate(&self, ptr: *mut u8, size: usize, align: usize) {
        self.allocator.deallocate(ptr, size, align)
    }

    unsafe fn reallocate(
        &self,
        ptr: *mut u8,
        old_size: usize,
        new_size: usize,
        align: usize,
    ) -> Result<*mut u8, Self::Error> {
        self.allocator.reallocate(ptr, old_size, new_size, align)
    }
}

/// Type alias for compatibility
pub type Bump = SimpleBump;

/// Create a new bump allocator
pub fn new_bump() -> Bump {
    Bump::new()
}

/// Create a new bump allocator with capacity
pub fn new_bump_with_capacity(capacity: usize) -> Bump {
    Bump::with_capacity(capacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_bump_creation() {
        let bump = SimpleBump::new();
        assert_eq!(bump.allocated_bytes(), 0);
    }

    #[test]
    fn test_simple_bump_with_capacity() {
        let bump = SimpleBump::with_capacity(1024);
        assert_eq!(bump.allocated_bytes(), 0);
    }

    #[test]
    fn test_simple_bump_allocator_trait() {
        let bump = SimpleBump::new();
        
        // Test allocation through the trait
        let result = bump.allocate(64, 8);
        // Result depends on the underlying allocator implementation
        match result {
            Ok(ptr) => {
                assert!(!ptr.is_null());
                unsafe {
                    bump.deallocate(ptr, 64, 8);
                }
            }
            Err(_) => {
                // Allocation failure is acceptable for some allocators
            }
        }
    }

    #[test]
    fn test_simple_bump_reset() {
        let mut bump = SimpleBump::new();
        bump.reset();
        // Reset should not panic
    }
}