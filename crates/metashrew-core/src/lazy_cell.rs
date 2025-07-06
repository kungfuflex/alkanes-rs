//! Compatibility shim for std::sync::LazyLock
//!
//! This module provides a compatibility layer for std::sync::LazyLock using once_cell::sync::Lazy
//! to support older Rust versions that don't have LazyLock in std.

use once_cell::sync::Lazy;
use std::ops::Deref;

/// Compatibility wrapper for std::sync::LazyLock using once_cell::sync::Lazy
pub struct LazyLock<T> {
    inner: Lazy<T, fn() -> T>,
}

impl<T> LazyLock<T> {
    /// Create a new LazyLock with the given initialization function
    pub const fn new(init: fn() -> T) -> LazyLock<T> {
        LazyLock {
            inner: Lazy::new(init),
        }
    }
}

impl<T> Deref for LazyLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// Implement Send and Sync if the inner type supports it
unsafe impl<T: Send + Sync> Send for LazyLock<T> {}
unsafe impl<T: Send + Sync> Sync for LazyLock<T> {}