//! Adapter to make standard bumpalo work with AlkanesAllocator trait
//!
//! This module provides compatibility between the standard bumpalo library
//! and the alkanes allocation ecosystem.

use alkanes_alloc::{AlkanesAllocator, AllocError};
use bumpalo::Bump as StandardBump;
use core::alloc::Layout;

/// Adapter that makes standard bumpalo::Bump implement AlkanesAllocator
pub struct BumpaloBridge<'a> {
    bump: &'a StandardBump,
}

impl<'a> BumpaloBridge<'a> {
    /// Create a new bridge to a bumpalo::Bump
    pub fn new(bump: &'a StandardBump) -> Self {
        Self { bump }
    }
}

impl<'a> AlkanesAllocator for BumpaloBridge<'a> {
    type Error = AllocError;

    fn allocate(&self, size: usize, align: usize) -> Result<*mut u8, Self::Error> {
        if size == 0 {
            return Err(AllocError::InvalidSize);
        }

        let layout = Layout::from_size_align(size, align)
            .map_err(|_| AllocError::InvalidAlignment)?;

        match self.bump.try_alloc_layout(layout) {
            Ok(ptr) => Ok(ptr.as_ptr()),
            Err(_) => Err(AllocError::OutOfMemory),
        }
    }

    unsafe fn deallocate(&self, _ptr: *mut u8, _size: usize, _align: usize) {
        // Bump allocators don't support individual deallocation
        // Memory is reclaimed only on reset()
    }

    unsafe fn reallocate(
        &self,
        _ptr: *mut u8,
        _old_size: usize,
        new_size: usize,
        align: usize,
    ) -> Result<*mut u8, Self::Error> {
        // For bump allocators, reallocation typically means new allocation
        self.allocate(new_size, align)
    }
}

/// Extension trait to add AlkanesAllocator compatibility to bumpalo::Bump
pub trait BumpaloAlkanesExt {
    /// Get an AlkanesAllocator adapter for this bump allocator
    fn as_alkanes_allocator(&self) -> BumpaloBridge<'_>;
}

impl BumpaloAlkanesExt for StandardBump {
    fn as_alkanes_allocator(&self) -> BumpaloBridge<'_> {
        BumpaloBridge::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn test_bumpalo_bridge() {
        let bump = Bump::new();
        let bridge = BumpaloBridge::new(&bump);
        
        // Test allocation
        let ptr = bridge.allocate(64, 8).unwrap();
        assert!(!ptr.is_null());
        
        // Test deallocation (should not panic)
        unsafe {
            bridge.deallocate(ptr, 64, 8);
        }
    }

    #[test]
    fn test_bumpalo_extension_trait() {
        let bump = Bump::new();
        let _bridge = bump.as_alkanes_allocator();
        
        // Test that the extension trait works
    }

    #[test]
    fn test_zero_size_allocation() {
        let bump = Bump::new();
        let bridge = BumpaloBridge::new(&bump);
        
        let result = bridge.allocate(0, 1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), AllocError::InvalidSize);
    }

    #[test]
    fn test_invalid_alignment() {
        let bump = Bump::new();
        let bridge = BumpaloBridge::new(&bump);
        
        // Test with invalid alignment (not a power of 2)
        let result = bridge.allocate(64, 3);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), AllocError::InvalidAlignment);
    }

    #[test]
    fn test_reallocation() {
        let bump = Bump::new();
        let bridge = BumpaloBridge::new(&bump);
        
        let ptr1 = bridge.allocate(32, 4).unwrap();
        let ptr2 = unsafe { bridge.reallocate(ptr1, 32, 64, 4).unwrap() };
        
        // For bump allocators, reallocation typically gives a new pointer
        // (though it might be the same in some cases)
        assert!(!ptr2.is_null());
    }
}