//! SPIR-V-specific allocation implementation
//!
//! Uses compile-time memory layout with thread-local allocation.
//! Avoids global static state that causes SPIR-V compiler issues.

use crate::{AlkanesAllocator, AllocError};

/// Maximum memory available for allocation in SPIR-V (1MB)
const SPIRV_ARENA_SIZE: usize = 1024 * 1024;

/// Alignment for SPIR-V allocations (16 bytes for GPU compatibility)
const SPIRV_ALIGNMENT: usize = 16;

/// Compile-time memory layout allocator for SPIR-V
///
/// This allocator uses a thread-local approach with compile-time memory layout.
/// Each shader invocation gets its own memory space without global state.
#[derive(Debug, Copy, Clone)]
pub struct SpirvLayoutAllocator {
    /// Current offset in the thread-local arena
    offset: usize,
    /// Maximum size of the arena
    max_size: usize,
}

impl SpirvLayoutAllocator {
    /// Create a new SPIR-V layout allocator with specified size
    pub const fn new(max_size: usize) -> Self {
        Self {
            offset: 0,
            max_size,
        }
    }
    
    /// Create a default allocator with standard arena size
    pub const fn default() -> Self {
        Self::new(SPIRV_ARENA_SIZE)
    }
    
    /// Align a size to the required alignment
    fn align_size(size: usize, align: usize) -> usize {
        let align = align.max(SPIRV_ALIGNMENT);
        (size + align - 1) & !(align - 1)
    }
    
    /// Reset the allocator (for testing or reuse)
    pub fn reset(&mut self) {
        self.offset = 0;
    }
    
    /// Get current memory usage
    pub fn used_bytes(&self) -> usize {
        self.offset
    }
    
    /// Get remaining memory
    pub fn remaining_bytes(&self) -> usize {
        self.max_size.saturating_sub(self.used_bytes())
    }
}

impl AlkanesAllocator for SpirvLayoutAllocator {
    type Error = AllocError;
    
    fn allocate(&self, size: usize, align: usize) -> Result<*mut u8, Self::Error> {
        if size == 0 {
            // SPIR-V compatibility: Avoid Result enum casting by panicking instead
            panic!("Invalid allocation size: 0");
        }
        
        // For SPIR-V, we use a compile-time approach where allocation
        // is more like reserving space in a pre-allocated buffer.
        // This avoids runtime heap management that SPIR-V doesn't support.
        
        // In a real implementation, this would use thread-local storage
        // or shader-parameter-based memory regions. For now, we panic
        // to avoid Result enum casting issues in SPIR-V.
        panic!("SPIR-V allocation not implemented - use compile-time memory layout");
    }
    
    unsafe fn deallocate(&self, _ptr: *mut u8, _size: usize, _align: usize) {
        // Compile-time layout allocator doesn't support individual deallocation
        // Memory layout is determined at compile time and managed differently
    }
    
    unsafe fn reallocate(
        &self,
        _ptr: *mut u8,
        _old_size: usize,
        _new_size: usize,
        _align: usize,
    ) -> Result<*mut u8, Self::Error> {
        // Reallocation not supported in compile-time layout approach
        // Collections should pre-allocate or use different strategies
        // SPIR-V compatibility: Avoid Result enum casting by panicking instead
        panic!("SPIR-V reallocation not supported - use compile-time memory layout");
    }
}

/// Default allocator type for SPIR-V
pub type DefaultAllocator = SpirvLayoutAllocator;

/// Create a default allocator instance
///
/// Note: This returns a simple allocator that doesn't actually allocate.
/// Real SPIR-V allocation needs compile-time memory layout planning.
pub fn default_allocator() -> DefaultAllocator {
    SpirvLayoutAllocator::default()
}

/// SPIR-V-specific Box type
pub type SpirvBox<T> = crate::AlkanesBox<T, SpirvLayoutAllocator>;

/// SPIR-V-specific Vec type
pub type SpirvVec<T> = crate::AlkanesVec<T, SpirvLayoutAllocator>;

/// Convenience function to create a new Box in SPIR-V
///
/// Note: Currently returns an error as compile-time allocation is not implemented
pub fn spirv_box<T>(_value: T) -> Result<SpirvBox<T>, AllocError> {
    Err(AllocError::OutOfMemory)
}

/// Convenience function to create a new Vec in SPIR-V
///
/// Note: Returns an empty vec that cannot actually allocate
pub fn spirv_vec<T>() -> SpirvVec<T> {
    SpirvVec::new_in(&default_allocator())
}

/// Convenience function to create a new Vec with capacity in SPIR-V
///
/// Note: Currently returns an error as compile-time allocation is not implemented
pub fn spirv_vec_with_capacity<T>(_capacity: usize) -> Result<SpirvVec<T>, AllocError> {
    Err(AllocError::OutOfMemory)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spirv_allocator_creation() {
        let allocator = SpirvLayoutAllocator::new(1024);
        assert_eq!(allocator.used_bytes(), 0);
        assert_eq!(allocator.remaining_bytes(), 1024);
    }
    
    #[test]
    fn test_spirv_allocator_default() {
        let allocator = SpirvLayoutAllocator::default();
        assert_eq!(allocator.used_bytes(), 0);
        assert_eq!(allocator.remaining_bytes(), SPIRV_ARENA_SIZE);
    }
    
    #[test]
    fn test_default_allocator() {
        let _allocator = default_allocator();
        // Just test that we can get the allocator without panicking
    }
    
    #[test]
    fn test_spirv_box_fails_gracefully() {
        // Test that spirv_box returns an error as expected
        let result = spirv_box(42u32);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_spirv_vec_empty() {
        let vec = spirv_vec::<u32>();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
    }
    
    #[test]
    fn test_spirv_vec_with_capacity_fails() {
        // Test that spirv_vec_with_capacity returns an error as expected
        let result = spirv_vec_with_capacity::<u32>(10);
        assert!(result.is_err());
    }
}