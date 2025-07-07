//! Synchronization primitives for alkanes with target-specific implementations
//! 
//! Provides different synchronization implementations based on target:
//! - SPIR-V: No-op implementations (single-threaded compute shaders)
//! - WASM32: Standard sync primitives or no-op for single-threaded
//! - Native: Standard sync primitives
//!
//! This crate enables alkanes collections and WASM interpreter to work across all targets.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(target_arch = "spirv", no_std)]

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_arch = "spirv")] {
        mod spirv_sync;
        pub use spirv_sync::*;
    } else if #[cfg(target_arch = "wasm32")] {
        mod wasm_sync;
        pub use wasm_sync::*;
    } else {
        mod native_sync;
        pub use native_sync::*;
    }
}

/// Generic mutex trait that all alkanes code will use
pub trait AlkanesMutex<T> {
    type Guard<'a>: core::ops::Deref<Target = T> + core::ops::DerefMut<Target = T>
    where
        Self: 'a,
        T: 'a;
    
    /// Create a new mutex protecting the given data
    fn new(data: T) -> Self;
    
    /// Lock the mutex and return a guard
    fn lock(&self) -> Self::Guard<'_>;
    
    /// Try to lock the mutex without blocking
    fn try_lock(&self) -> Option<Self::Guard<'_>>;
}

/// Generic atomic reference counter trait
pub trait AlkanesArc<T: Clone> {
    /// Create a new atomic reference counter
    fn new(data: T) -> Self;
    
    /// Clone the reference (increment reference count)
    fn clone(&self) -> Self;
    
    /// Get a reference to the inner data
    fn as_ref(&self) -> &T;
}

/// Generic once cell trait for lazy initialization
pub trait AlkanesOnceCell<T> {
    /// Create a new empty once cell
    fn new() -> Self;
    
    /// Get the value, initializing it if necessary
    fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T;
    
    /// Get the value if it has been initialized
    fn get(&self) -> Option<&T>;
}

/// Generic read-write lock trait
pub trait AlkanesRwLock<T> {
    type ReadGuard<'a>: core::ops::Deref<Target = T>
    where
        Self: 'a,
        T: 'a;
    
    type WriteGuard<'a>: core::ops::Deref<Target = T> + core::ops::DerefMut<Target = T>
    where
        Self: 'a,
        T: 'a;
    
    /// Create a new read-write lock protecting the given data
    fn new(data: T) -> Self;
    
    /// Acquire a read lock
    fn read(&self) -> Self::ReadGuard<'_>;
    
    /// Acquire a write lock
    fn write(&self) -> Self::WriteGuard<'_>;
}

/// Error types for synchronization failures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncError {
    LockFailed,
    Poisoned,
    WouldBlock,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mutex_basic() {
        let mutex = DefaultMutex::new(42u32);
        let guard = mutex.lock();
        assert_eq!(*guard, 42);
    }
    
    #[test]
    fn test_arc_basic() {
        let arc = DefaultArc::new(42u32);
        let arc2 = arc.clone();
        assert_eq!(*arc.as_ref(), 42);
        assert_eq!(*arc2.as_ref(), 42);
    }
    
    #[test]
    fn test_once_cell_basic() {
        let cell = DefaultOnceCell::new();
        let value = cell.get_or_init(|| 42u32);
        assert_eq!(*value, 42);
        
        let value2 = cell.get().unwrap();
        assert_eq!(*value2, 42);
    }
}