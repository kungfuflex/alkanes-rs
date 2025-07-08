//! Generic allocation strategy for alkanes
//! 
//! Provides different allocation implementations based on target:
//! - SPIR-V: Fixed-size arena allocator (no heap)
//! - WASM32: Standard heap allocator  
//! - Native: Standard heap allocator
//!
//! This crate is the foundation for all other alkanes crates that need allocation.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(target_arch = "spirv", no_std)]

use cfg_if::cfg_if;

// Always include spirv_alloc module for type definitions
mod spirv_alloc;
pub use spirv_alloc::{SpirvLayoutAllocator, SpirvBox, SpirvVec, spirv_box, spirv_vec, spirv_vec_with_capacity};

cfg_if! {
    if #[cfg(target_arch = "spirv")] {
        // SPIR-V target uses spirv_alloc
        pub use spirv_alloc::{DefaultAllocator, default_allocator};
    } else if #[cfg(target_arch = "wasm32")] {
        mod wasm_alloc;
        pub use wasm_alloc::{DefaultAllocator, default_allocator};
    } else {
        mod native_alloc;
        pub use native_alloc::{DefaultAllocator, default_allocator};
    }
}

/// Generic allocator trait that all alkanes collections will use
pub trait AlkanesAllocator {
    type Error;
    
    /// Allocate memory for `size` bytes with given alignment
    fn allocate(&self, size: usize, align: usize) -> Result<*mut u8, Self::Error>;
    
    /// Deallocate memory at the given pointer
    unsafe fn deallocate(&self, ptr: *mut u8, size: usize, align: usize);
    
    /// Reallocate memory, growing or shrinking the allocation
    unsafe fn reallocate(
        &self, 
        ptr: *mut u8, 
        old_size: usize, 
        new_size: usize, 
        align: usize
    ) -> Result<*mut u8, Self::Error>;
}

/// Generic Box implementation that works with any AlkanesAllocator
pub struct AlkanesBox<T, A: AlkanesAllocator> {
    ptr: *mut T,
    allocator: *const A,
}

impl<T, A: AlkanesAllocator> AlkanesBox<T, A> {
    pub fn new_in(value: T, allocator: &A) -> Result<Self, A::Error> {
        let layout = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        
        let ptr = allocator.allocate(layout, align)? as *mut T;
        
        unsafe {
            core::ptr::write(ptr, value);
        }
        
        Ok(AlkanesBox { ptr, allocator: allocator as *const A })
    }
    
    pub fn as_ref(&self) -> &T {
        unsafe { &*self.ptr }
    }
    
    pub fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
    
    pub fn into_inner(self) -> T {
        let value = unsafe { core::ptr::read(self.ptr) };
        let layout = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        
        unsafe {
            (*self.allocator).deallocate(self.ptr as *mut u8, layout, align);
        }
        
        core::mem::forget(self);
        value
    }
}

impl<T, A: AlkanesAllocator> Drop for AlkanesBox<T, A> {
    fn drop(&mut self) {
        let layout = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        
        unsafe {
            core::ptr::drop_in_place(self.ptr);
            (*self.allocator).deallocate(self.ptr as *mut u8, layout, align);
        }
    }
}

/// Generic Vec implementation that works with any AlkanesAllocator
pub struct AlkanesVec<T, A: AlkanesAllocator> {
    ptr: *mut T,
    len: usize,
    capacity: usize,
    allocator: *const A,
}

impl<T, A: AlkanesAllocator> AlkanesVec<T, A> {
    pub fn new_in(allocator: &A) -> Self {
        AlkanesVec {
            ptr: core::ptr::null_mut(),
            len: 0,
            capacity: 0,
            allocator: allocator as *const A,
        }
    }
    
    pub fn with_capacity_in(capacity: usize, allocator: &A) -> Result<Self, A::Error> {
        if capacity == 0 {
            return Ok(Self::new_in(allocator));
        }
        
        let layout = capacity * core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        
        let ptr = allocator.allocate(layout, align)? as *mut T;
        
        Ok(AlkanesVec {
            ptr,
            len: 0,
            capacity,
            allocator: allocator as *const A,
        })
    }
    
    pub fn push(&mut self, value: T) -> Result<(), A::Error> {
        if self.len == self.capacity {
            self.grow()?;
        }
        
        unsafe {
            core::ptr::write(self.ptr.add(self.len), value);
        }
        self.len += 1;
        
        Ok(())
    }
    
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            Some(unsafe { core::ptr::read(self.ptr.add(self.len)) })
        }
    }
    
    pub fn len(&self) -> usize {
        self.len
    }
    
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    
    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
    
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
    
    fn grow(&mut self) -> Result<(), A::Error> {
        let new_capacity = if self.capacity == 0 { 4 } else { self.capacity * 2 };
        
        let old_layout = self.capacity * core::mem::size_of::<T>();
        let new_layout = new_capacity * core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        
        let new_ptr = if self.ptr.is_null() {
            unsafe { (*self.allocator).allocate(new_layout, align)? }
        } else {
            unsafe {
                (*self.allocator).reallocate(
                    self.ptr as *mut u8,
                    old_layout,
                    new_layout,
                    align,
                )?
            }
        } as *mut T;
        
        self.ptr = new_ptr;
        self.capacity = new_capacity;
        
        Ok(())
    }
}

impl<T, A: AlkanesAllocator> Drop for AlkanesVec<T, A> {
    fn drop(&mut self) {
        // Drop all elements
        for i in 0..self.len {
            unsafe {
                core::ptr::drop_in_place(self.ptr.add(i));
            }
        }
        
        // Deallocate memory
        if !self.ptr.is_null() {
            let layout = self.capacity * core::mem::size_of::<T>();
            let align = core::mem::align_of::<T>();
            
            unsafe {
                (*self.allocator).deallocate(self.ptr as *mut u8, layout, align);
            }
        }
    }
}

/// Error types for allocation failures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocError {
    OutOfMemory,
    InvalidSize,
    InvalidAlignment,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_allocator_trait() {
        // Test that the trait compiles and basic functionality works
        // Actual tests will be in target-specific modules
    }
}