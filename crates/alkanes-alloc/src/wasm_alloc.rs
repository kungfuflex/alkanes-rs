//! WASM32-specific allocation implementation
//! 
//! Uses the standard heap allocator but with WASM-specific optimizations

use crate::{AlkanesAllocator, AllocError};

#[cfg(feature = "std")]
use std::alloc::{GlobalAlloc, Layout, System};

/// WASM32 heap allocator
#[derive(Copy, Clone)]
pub struct WasmHeapAllocator;

impl AlkanesAllocator for WasmHeapAllocator {
    type Error = AllocError;
    
    fn allocate(&self, size: usize, align: usize) -> Result<*mut u8, Self::Error> {
        if size == 0 {
            return Err(AllocError::InvalidSize);
        }
        
        #[cfg(feature = "std")]
        {
            let layout = Layout::from_size_align(size, align)
                .map_err(|_| AllocError::InvalidAlignment)?;
            
            let ptr = unsafe { System.alloc(layout) };
            
            if ptr.is_null() {
                Err(AllocError::OutOfMemory)
            } else {
                Ok(ptr)
            }
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no-std WASM, we'd need a custom allocator
            // For now, return an error
            Err(AllocError::OutOfMemory)
        }
    }
    
    unsafe fn deallocate(&self, ptr: *mut u8, size: usize, align: usize) {
        #[cfg(feature = "std")]
        {
            if let Ok(layout) = Layout::from_size_align(size, align) {
                System.dealloc(ptr, layout);
            }
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no-std WASM, we'd need a custom allocator
            // For now, do nothing (memory leak, but better than crash)
        }
    }
    
    unsafe fn reallocate(
        &self,
        ptr: *mut u8,
        old_size: usize,
        new_size: usize,
        align: usize,
    ) -> Result<*mut u8, Self::Error> {
        #[cfg(feature = "std")]
        {
            let old_layout = Layout::from_size_align(old_size, align)
                .map_err(|_| AllocError::InvalidAlignment)?;
            
            let new_ptr = System.realloc(ptr, old_layout, new_size);
            
            if new_ptr.is_null() {
                Err(AllocError::OutOfMemory)
            } else {
                Ok(new_ptr)
            }
        }
        
        #[cfg(not(feature = "std"))]
        {
            // Fallback: allocate new, copy, deallocate old
            let new_ptr = self.allocate(new_size, align)?;
            core::ptr::copy_nonoverlapping(ptr, new_ptr, old_size.min(new_size));
            self.deallocate(ptr, old_size, align);
            Ok(new_ptr)
        }
    }
}

/// Default allocator type for WASM32
pub type DefaultAllocator = WasmHeapAllocator;

/// Create a default allocator instance
pub fn default_allocator() -> &'static DefaultAllocator {
    static ALLOCATOR: WasmHeapAllocator = WasmHeapAllocator;
    &ALLOCATOR
}

/// WASM32-specific Box type
pub type WasmBox<T> = crate::AlkanesBox<T, WasmHeapAllocator>;

/// WASM32-specific Vec type
pub type WasmVec<T> = crate::AlkanesVec<T, WasmHeapAllocator>;

/// Convenience function to create a new Box in WASM32
pub fn wasm_box<T>(value: T) -> Result<WasmBox<T>, AllocError> {
    WasmBox::new_in(value, default_allocator())
}

/// Convenience function to create a new Vec in WASM32
pub fn wasm_vec<T>() -> WasmVec<T> {
    WasmVec::new_in(default_allocator())
}

/// Convenience function to create a new Vec with capacity in WASM32
pub fn wasm_vec_with_capacity<T>(capacity: usize) -> Result<WasmVec<T>, AllocError> {
    WasmVec::with_capacity_in(capacity, default_allocator())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wasm_allocator() {
        let allocator = WasmHeapAllocator;
        
        // Test basic allocation
        let ptr = allocator.allocate(64, 8).unwrap();
        assert!(!ptr.is_null());
        
        // Test deallocation
        unsafe {
            allocator.deallocate(ptr, 64, 8);
        }
    }
    
    #[test]
    fn test_wasm_box() {
        let boxed = wasm_box(42u32).unwrap();
        assert_eq!(*boxed.as_ref(), 42);
        
        let value = boxed.into_inner();
        assert_eq!(value, 42);
    }
    
    #[test]
    fn test_wasm_vec() {
        let mut vec = wasm_vec::<u32>();
        
        vec.push(1).unwrap();
        vec.push(2).unwrap();
        vec.push(3).unwrap();
        
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.as_slice(), &[1, 2, 3]);
        
        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.len(), 2);
    }
}