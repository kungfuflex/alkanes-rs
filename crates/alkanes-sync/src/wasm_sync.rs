//! WASM32 target synchronization implementation
//! 
//! Uses no-op implementations for WASM32 since it's typically single-threaded.

use crate::{AlkanesArc, AlkanesMutex, AlkanesOnceCell, AlkanesRwLock};
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

/// WASM32 mutex (no-op since single-threaded)
pub struct WasmMutex<T> {
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for WasmMutex<T> {}
unsafe impl<T: Send> Sync for WasmMutex<T> {}

impl<T> WasmMutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }
}

/// WASM32 mutex guard
pub struct WasmMutexGuard<'a, T> {
    data: &'a mut T,
}

impl<'a, T> Deref for WasmMutexGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> DerefMut for WasmMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<T> AlkanesMutex<T> for WasmMutex<T> {
    type Guard<'a> = WasmMutexGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    
    fn new(data: T) -> Self {
        Self::new(data)
    }
    
    fn lock(&self) -> Self::Guard<'_> {
        // SAFETY: WASM32 is single-threaded, so this is safe
        let data = unsafe { &mut *self.data.get() };
        WasmMutexGuard { data }
    }
    
    fn try_lock(&self) -> Option<Self::Guard<'_>> {
        // Always succeeds in single-threaded environment
        Some(self.lock())
    }
}

/// WASM32 atomic reference counter (simplified for single-threaded use)
pub struct WasmArc<T> {
    data: T,
}

impl<T> WasmArc<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T: Clone> Clone for WasmArc<T> {
    fn clone(&self) -> Self {
        // In WASM32, we clone the data since we can't do heap allocation easily
        Self {
            data: self.data.clone(),
        }
    }
}

impl<T: Clone> AlkanesArc<T> for WasmArc<T> {
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

/// WASM32 once cell - simplified for WASM32 compatibility
pub struct WasmOnceCell<T> {
    data: UnsafeCell<Option<T>>,
}

impl<T> WasmOnceCell<T> {
    pub const fn new() -> Self {
        Self { 
            data: UnsafeCell::new(None),
        }
    }
    
    pub const fn with_value(value: T) -> Self {
        Self { 
            data: UnsafeCell::new(Some(value)),
        }
    }
}

impl<T> AlkanesOnceCell<T> for WasmOnceCell<T> {
    fn new() -> Self {
        Self::new()
    }
    
    fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        // SAFETY: WASM32 is single-threaded
        let data = unsafe { &mut *self.data.get() };
        if data.is_none() {
            *data = Some(f());
        }
        data.as_ref().unwrap()
    }
    
    fn get(&self) -> Option<&T> {
        // SAFETY: WASM32 is single-threaded
        unsafe { (*self.data.get()).as_ref() }
    }
}

/// WASM32 read-write lock (no-op since single-threaded)
pub struct WasmRwLock<T> {
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for WasmRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for WasmRwLock<T> {}

impl<T> WasmRwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }
}

/// WASM32 read guard
pub struct WasmReadGuard<'a, T> {
    data: &'a T,
}

impl<'a, T> Deref for WasmReadGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

/// WASM32 write guard
pub struct WasmWriteGuard<'a, T> {
    data: &'a mut T,
}

impl<'a, T> Deref for WasmWriteGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> DerefMut for WasmWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<T> AlkanesRwLock<T> for WasmRwLock<T> {
    type ReadGuard<'a> = WasmReadGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    
    type WriteGuard<'a> = WasmWriteGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    
    fn new(data: T) -> Self {
        Self::new(data)
    }
    
    fn read(&self) -> Self::ReadGuard<'_> {
        // SAFETY: WASM32 is single-threaded, so this is safe
        let data = unsafe { &*self.data.get() };
        WasmReadGuard { data }
    }
    
    fn write(&self) -> Self::WriteGuard<'_> {
        // SAFETY: WASM32 is single-threaded, so this is safe
        let data = unsafe { &mut *self.data.get() };
        WasmWriteGuard { data }
    }
}

/// Default types for WASM32
pub type DefaultMutex<T> = WasmMutex<T>;
pub type DefaultArc<T> = WasmArc<T>;
pub type DefaultOnceCell<T> = WasmOnceCell<T>;
pub type DefaultRwLock<T> = WasmRwLock<T>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wasm_mutex() {
        let mutex = WasmMutex::new(42u32);
        let guard = mutex.lock();
        assert_eq!(*guard, 42);
    }
    
    #[test]
    fn test_wasm_rwlock() {
        let rwlock = WasmRwLock::new(42u32);
        let read_guard = rwlock.read();
        assert_eq!(*read_guard, 42);
        drop(read_guard);
        
        let mut write_guard = rwlock.write();
        *write_guard = 84;
        assert_eq!(*write_guard, 84);
    }
    
    #[test]
    fn test_wasm_once_cell() {
        let cell = WasmOnceCell::new();
        let value = cell.get_or_init(|| 42u32);
        assert_eq!(*value, 42);
        
        let value2 = cell.get().unwrap();
        assert_eq!(*value2, 42);
    }
}