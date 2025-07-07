//! WASM32-specific synchronization implementation
//! 
//! WASM32 can be single-threaded or multi-threaded depending on the environment.
//! This implementation provides both options.

use crate::{AlkanesArc, AlkanesMutex, AlkanesOnceCell, SyncError};
use core::cell::{Cell, UnsafeCell};
use core::ops::{Deref, DerefMut};

#[cfg(feature = "std")]
use std::sync::{Arc, Mutex, Once};

/// WASM32 mutex implementation
#[cfg(feature = "std")]
pub struct WasmMutex<T> {
    inner: Mutex<T>,
}

#[cfg(not(feature = "std"))]
pub struct WasmMutex<T> {
    data: UnsafeCell<T>,
}

#[cfg(feature = "std")]
impl<T> WasmMutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            inner: Mutex::new(data),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<T> WasmMutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }
}

/// WASM32 mutex guard
#[cfg(feature = "std")]
pub struct WasmMutexGuard<'a, T> {
    guard: std::sync::MutexGuard<'a, T>,
}

#[cfg(not(feature = "std"))]
pub struct WasmMutexGuard<'a, T> {
    data: &'a mut T,
}

#[cfg(feature = "std")]
impl<'a, T> Deref for WasmMutexGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

#[cfg(feature = "std")]
impl<'a, T> DerefMut for WasmMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> Deref for WasmMutexGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

#[cfg(not(feature = "std"))]
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
    
    #[cfg(feature = "std")]
    fn lock(&self) -> Self::Guard<'_> {
        WasmMutexGuard {
            guard: self.inner.lock().unwrap(),
        }
    }
    
    #[cfg(not(feature = "std"))]
    fn lock(&self) -> Self::Guard<'_> {
        // SAFETY: Single-threaded WASM32 without std
        let data = unsafe { &mut *self.data.get() };
        WasmMutexGuard { data }
    }
    
    #[cfg(feature = "std")]
    fn try_lock(&self) -> Option<Self::Guard<'_>> {
        self.inner.try_lock().ok().map(|guard| WasmMutexGuard { guard })
    }
    
    #[cfg(not(feature = "std"))]
    fn try_lock(&self) -> Option<Self::Guard<'_>> {
        Some(self.lock())
    }
}

/// WASM32 atomic reference counter
#[cfg(feature = "std")]
pub struct WasmArc<T> {
    inner: Arc<T>,
}

#[cfg(not(feature = "std"))]
pub struct WasmArc<T> {
    data: T,
}

#[cfg(feature = "std")]
impl<T> WasmArc<T> {
    pub fn new(data: T) -> Self {
        Self {
            inner: Arc::new(data),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<T> WasmArc<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

#[cfg(feature = "std")]
impl<T> Clone for WasmArc<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<T: Clone> Clone for WasmArc<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

impl<T> AlkanesArc<T> for WasmArc<T> {
    fn new(data: T) -> Self {
        Self::new(data)
    }
    
    fn clone(&self) -> Self {
        self.clone()
    }
    
    #[cfg(feature = "std")]
    fn as_ref(&self) -> &T {
        &self.inner
    }
    
    #[cfg(not(feature = "std"))]
    fn as_ref(&self) -> &T {
        &self.data
    }
}

/// WASM32 once cell
#[cfg(feature = "std")]
pub struct WasmOnceCell<T> {
    once: Once,
    data: UnsafeCell<Option<T>>,
}

#[cfg(not(feature = "std"))]
pub struct WasmOnceCell<T> {
    data: UnsafeCell<Option<T>>,
    initialized: Cell<bool>,
}

#[cfg(feature = "std")]
unsafe impl<T: Send> Send for WasmOnceCell<T> {}
#[cfg(feature = "std")]
unsafe impl<T: Send + Sync> Sync for WasmOnceCell<T> {}

#[cfg(not(feature = "std"))]
unsafe impl<T: Send> Send for WasmOnceCell<T> {}
#[cfg(not(feature = "std"))]
unsafe impl<T: Send + Sync> Sync for WasmOnceCell<T> {}

#[cfg(feature = "std")]
impl<T> WasmOnceCell<T> {
    pub const fn new() -> Self {
        Self {
            once: Once::new(),
            data: UnsafeCell::new(None),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<T> WasmOnceCell<T> {
    pub const fn new() -> Self {
        Self {
            data: UnsafeCell::new(None),
            initialized: Cell::new(false),
        }
    }
}

impl<T> AlkanesOnceCell<T> for WasmOnceCell<T> {
    fn new() -> Self {
        Self::new()
    }
    
    #[cfg(feature = "std")]
    fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        self.once.call_once(|| {
            // SAFETY: This is only called once due to Once
            let data = unsafe { &mut *self.data.get() };
            *data = Some(f());
        });
        
        // SAFETY: We just initialized it above
        unsafe { (*self.data.get()).as_ref().unwrap() }
    }
    
    #[cfg(not(feature = "std"))]
    fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if !self.initialized.get() {
            // SAFETY: Single-threaded without std
            let data = unsafe { &mut *self.data.get() };
            *data = Some(f());
            self.initialized.set(true);
        }
        
        // SAFETY: We just initialized it above
        unsafe { (*self.data.get()).as_ref().unwrap() }
    }
    
    #[cfg(feature = "std")]
    fn get(&self) -> Option<&T> {
        if self.once.is_completed() {
            // SAFETY: Once guarantees initialization is complete
            unsafe { (*self.data.get()).as_ref() }
        } else {
            None
        }
    }
    
    #[cfg(not(feature = "std"))]
    fn get(&self) -> Option<&T> {
        if self.initialized.get() {
            // SAFETY: We checked that it's initialized
            unsafe { (*self.data.get()).as_ref() }
        } else {
            None
        }
    }
}

/// Default types for WASM32
pub type DefaultMutex<T> = WasmMutex<T>;
pub type DefaultArc<T> = WasmArc<T>;
pub type DefaultOnceCell<T> = WasmOnceCell<T>;

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
    fn test_wasm_arc() {
        let arc = WasmArc::new(42u32);
        let arc2 = arc.clone();
        assert_eq!(*arc.as_ref(), 42);
        assert_eq!(*arc2.as_ref(), 42);
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