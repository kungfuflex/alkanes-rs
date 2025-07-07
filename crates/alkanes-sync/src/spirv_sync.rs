//! SPIR-V-specific synchronization implementation
//! 
//! SPIR-V compute shaders are single-threaded within a workgroup,
//! so synchronization primitives are mostly no-ops.

use crate::{AlkanesArc, AlkanesMutex, AlkanesOnceCell};
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

/// SPIR-V mutex (no-op since single-threaded)
pub struct SpirvMutex<T> {
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for SpirvMutex<T> {}
unsafe impl<T: Send> Sync for SpirvMutex<T> {}

impl<T> SpirvMutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }
}

/// SPIR-V mutex guard
pub struct SpirvMutexGuard<'a, T> {
    data: &'a mut T,
}

impl<'a, T> Deref for SpirvMutexGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> DerefMut for SpirvMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<T> AlkanesMutex<T> for SpirvMutex<T> {
    type Guard<'a> = SpirvMutexGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    
    fn new(data: T) -> Self {
        Self::new(data)
    }
    
    fn lock(&self) -> Self::Guard<'_> {
        // SAFETY: SPIR-V is single-threaded, so this is safe
        let data = unsafe { &mut *self.data.get() };
        SpirvMutexGuard { data }
    }
    
    fn try_lock(&self) -> Option<Self::Guard<'_>> {
        // Always succeeds in single-threaded environment
        Some(self.lock())
    }
}

/// SPIR-V atomic reference counter (simplified for single-threaded use)
///
/// Note: This is a simplified implementation that doesn't actually
/// provide reference counting. In SPIR-V, we typically use stack
/// allocation or compile-time memory layout.
pub struct SpirvArc<T> {
    data: T,
}

impl<T> SpirvArc<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T: Clone> Clone for SpirvArc<T> {
    fn clone(&self) -> Self {
        // In SPIR-V, we clone the data since we can't do heap allocation
        Self {
            data: self.data.clone(),
        }
    }
}

impl<T: Clone> AlkanesArc<T> for SpirvArc<T> {
    fn new(data: T) -> Self {
        Self::new(data)
    }
    
    fn clone(&self) -> Self {
        Clone::clone(&self)
    }
    
    fn as_ref(&self) -> &T {
        &self.data
    }
}

/// SPIR-V once cell - simplified for SPIR-V compatibility
///
/// Note: This is a very simplified implementation that doesn't actually
/// provide lazy initialization. In SPIR-V, we typically know values at
/// compile time or use direct initialization.
pub struct SpirvOnceCell<T> {
    // For SPIR-V, we'll use a simple Option without UnsafeCell
    // This means we can't actually do lazy initialization, but
    // we can provide the interface for compatibility
    data: Option<T>,
}

impl<T> SpirvOnceCell<T> {
    pub const fn new() -> Self {
        Self { data: None }
    }
    
    pub const fn with_value(value: T) -> Self {
        Self { data: Some(value) }
    }
}

impl<T> AlkanesOnceCell<T> for SpirvOnceCell<T> {
    fn new() -> Self {
        Self::new()
    }
    
    fn get_or_init<F>(&self, _f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        // In SPIR-V, we can't actually do lazy initialization due to
        // the lack of mutable references in this context.
        // For now, we'll panic if not initialized - this encourages
        // compile-time initialization in SPIR-V shaders.
        match &self.data {
            Some(ref value) => value,
            None => {
                // In a real SPIR-V shader, this should be initialized
                // at compile time or construction time
                panic!("SpirvOnceCell not initialized - use with_value() for SPIR-V")
            }
        }
    }
    
    fn get(&self) -> Option<&T> {
        self.data.as_ref()
    }
}

/// Default types for SPIR-V
pub type DefaultMutex<T> = SpirvMutex<T>;
pub type DefaultArc<T> = SpirvArc<T>;
pub type DefaultOnceCell<T> = SpirvOnceCell<T>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spirv_mutex() {
        let mutex = SpirvMutex::new(42u32);
        let guard = mutex.lock();
        assert_eq!(*guard, 42);
    }
    
    #[test]
    fn test_spirv_once_cell() {
        let cell = SpirvOnceCell::new();
        let value = cell.get_or_init(|| 42u32);
        assert_eq!(*value, 42);
        
        let value2 = cell.get().unwrap();
        assert_eq!(*value2, 42);
    }
}