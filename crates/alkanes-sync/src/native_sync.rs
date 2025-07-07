//! Native target synchronization implementation
//! 
//! Uses standard library synchronization primitives for native targets.

use crate::{AlkanesArc, AlkanesMutex, AlkanesOnceCell, AlkanesRwLock};
use core::ops::{Deref, DerefMut};

#[cfg(feature = "std")]
use std::sync::{Arc, Mutex, Once, RwLock};

#[cfg(feature = "std")]
use std::cell::UnsafeCell;

#[cfg(not(feature = "std"))]
use core::cell::{Cell, UnsafeCell};

/// Native mutex implementation
#[cfg(feature = "std")]
pub struct NativeMutex<T> {
    inner: Mutex<T>,
}

#[cfg(not(feature = "std"))]
pub struct NativeMutex<T> {
    data: UnsafeCell<T>,
}

#[cfg(feature = "std")]
impl<T> NativeMutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            inner: Mutex::new(data),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<T> NativeMutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }
}

/// Native mutex guard
#[cfg(feature = "std")]
pub struct NativeMutexGuard<'a, T> {
    guard: std::sync::MutexGuard<'a, T>,
}

#[cfg(not(feature = "std"))]
pub struct NativeMutexGuard<'a, T> {
    data: &'a mut T,
}

#[cfg(feature = "std")]
impl<'a, T> Deref for NativeMutexGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

#[cfg(feature = "std")]
impl<'a, T> DerefMut for NativeMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> Deref for NativeMutexGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> DerefMut for NativeMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<T> AlkanesMutex<T> for NativeMutex<T> {
    type Guard<'a> = NativeMutexGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    
    fn new(data: T) -> Self {
        Self::new(data)
    }
    
    #[cfg(feature = "std")]
    fn lock(&self) -> Self::Guard<'_> {
        NativeMutexGuard {
            guard: self.inner.lock().unwrap(),
        }
    }
    
    #[cfg(not(feature = "std"))]
    fn lock(&self) -> Self::Guard<'_> {
        // SAFETY: Single-threaded no-std native
        let data = unsafe { &mut *self.data.get() };
        NativeMutexGuard { data }
    }
    
    #[cfg(feature = "std")]
    fn try_lock(&self) -> Option<Self::Guard<'_>> {
        self.inner.try_lock().ok().map(|guard| NativeMutexGuard { guard })
    }
    
    #[cfg(not(feature = "std"))]
    fn try_lock(&self) -> Option<Self::Guard<'_>> {
        Some(self.lock())
    }
}

/// Native atomic reference counter
#[cfg(feature = "std")]
pub struct NativeArc<T> {
    inner: Arc<T>,
}

#[cfg(not(feature = "std"))]
pub struct NativeArc<T> {
    data: T,
}

#[cfg(feature = "std")]
impl<T> NativeArc<T> {
    pub fn new(data: T) -> Self {
        Self {
            inner: Arc::new(data),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<T> NativeArc<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

#[cfg(feature = "std")]
impl<T> Clone for NativeArc<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<T: Clone> Clone for NativeArc<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

impl<T: Clone> AlkanesArc<T> for NativeArc<T> {
    fn new(data: T) -> Self {
        Self::new(data)
    }
    
    fn clone(&self) -> Self {
        Clone::clone(&self)
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

/// Native once cell
#[cfg(feature = "std")]
pub struct NativeOnceCell<T> {
    once: Once,
    data: UnsafeCell<Option<T>>,
}

#[cfg(not(feature = "std"))]
pub struct NativeOnceCell<T> {
    data: UnsafeCell<Option<T>>,
    initialized: Cell<bool>,
}

#[cfg(feature = "std")]
unsafe impl<T: Send> Send for NativeOnceCell<T> {}
#[cfg(feature = "std")]
unsafe impl<T: Send + Sync> Sync for NativeOnceCell<T> {}

#[cfg(not(feature = "std"))]
unsafe impl<T: Send> Send for NativeOnceCell<T> {}
#[cfg(not(feature = "std"))]
unsafe impl<T: Send + Sync> Sync for NativeOnceCell<T> {}

#[cfg(feature = "std")]
impl<T> NativeOnceCell<T> {
    pub const fn new() -> Self {
        Self {
            once: Once::new(),
            data: UnsafeCell::new(None),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<T> NativeOnceCell<T> {
    pub const fn new() -> Self {
        Self {
            data: UnsafeCell::new(None),
            initialized: Cell::new(false),
        }
    }
}

impl<T> AlkanesOnceCell<T> for NativeOnceCell<T> {
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

/// Native read-write lock
#[cfg(feature = "std")]
pub struct NativeRwLock<T> {
    inner: RwLock<T>,
}

#[cfg(not(feature = "std"))]
pub struct NativeRwLock<T> {
    data: UnsafeCell<T>,
}

#[cfg(feature = "std")]
impl<T> NativeRwLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            inner: RwLock::new(data),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<T> NativeRwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }
}

/// Native read guard
#[cfg(feature = "std")]
pub struct NativeReadGuard<'a, T> {
    guard: std::sync::RwLockReadGuard<'a, T>,
}

#[cfg(not(feature = "std"))]
pub struct NativeReadGuard<'a, T> {
    data: &'a T,
}

#[cfg(feature = "std")]
impl<'a, T> Deref for NativeReadGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> Deref for NativeReadGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

/// Native write guard
#[cfg(feature = "std")]
pub struct NativeWriteGuard<'a, T> {
    guard: std::sync::RwLockWriteGuard<'a, T>,
}

#[cfg(not(feature = "std"))]
pub struct NativeWriteGuard<'a, T> {
    data: &'a mut T,
}

#[cfg(feature = "std")]
impl<'a, T> Deref for NativeWriteGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

#[cfg(feature = "std")]
impl<'a, T> DerefMut for NativeWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> Deref for NativeWriteGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> DerefMut for NativeWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<T> AlkanesRwLock<T> for NativeRwLock<T> {
    type ReadGuard<'a> = NativeReadGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    
    type WriteGuard<'a> = NativeWriteGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    
    fn new(data: T) -> Self {
        Self::new(data)
    }
    
    #[cfg(feature = "std")]
    fn read(&self) -> Self::ReadGuard<'_> {
        NativeReadGuard {
            guard: self.inner.read().unwrap(),
        }
    }
    
    #[cfg(not(feature = "std"))]
    fn read(&self) -> Self::ReadGuard<'_> {
        // SAFETY: Single-threaded no-std native
        let data = unsafe { &*self.data.get() };
        NativeReadGuard { data }
    }
    
    #[cfg(feature = "std")]
    fn write(&self) -> Self::WriteGuard<'_> {
        NativeWriteGuard {
            guard: self.inner.write().unwrap(),
        }
    }
    
    #[cfg(not(feature = "std"))]
    fn write(&self) -> Self::WriteGuard<'_> {
        // SAFETY: Single-threaded no-std native
        let data = unsafe { &mut *self.data.get() };
        NativeWriteGuard { data }
    }
}

/// Default types for native targets
pub type DefaultMutex<T> = NativeMutex<T>;
pub type DefaultArc<T> = NativeArc<T>;
pub type DefaultOnceCell<T> = NativeOnceCell<T>;
pub type DefaultRwLock<T> = NativeRwLock<T>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_native_mutex() {
        let mutex = NativeMutex::new(42u32);
        let guard = mutex.lock();
        assert_eq!(*guard, 42);
    }
    
    #[test]
    fn test_native_arc() {
        let arc = NativeArc::new(42u32);
        let arc2 = arc.clone();
        assert_eq!(*arc.as_ref(), 42);
        assert_eq!(*arc2.as_ref(), 42);
    }
    
    #[test]
    fn test_native_once_cell() {
        let cell = NativeOnceCell::new();
        let value = cell.get_or_init(|| 42u32);
        assert_eq!(*value, 42);
        
        let value2 = cell.get().unwrap();
        assert_eq!(*value2, 42);
    }
}