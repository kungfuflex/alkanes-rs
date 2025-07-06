//! Compatibility shim for std::sync::LazyLock using once_cell
//! 
//! This crate provides a drop-in replacement for std::sync::LazyLock
//! for older Rust versions that don't have it stabilized yet.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use once_cell::sync::Lazy;
use core::ops::Deref;

/// A thread-safe lazy-initialized value.
/// 
/// This is a compatibility shim for std::sync::LazyLock that uses once_cell::sync::Lazy
/// under the hood. It provides the same API as the standard library version.
pub struct LazyLock<T, F = fn() -> T> {
    inner: Lazy<T, F>,
}

impl<T, F> LazyLock<T, F>
where
    F: FnOnce() -> T,
{
    /// Creates a new lazy value with the given initializing function.
    pub const fn new(f: F) -> LazyLock<T, F> {
        LazyLock {
            inner: Lazy::new(f),
        }
    }

    /// Forces the evaluation of this lazy value and returns a reference to result.
    /// 
    /// This is equivalent to the `Deref` impl, but is explicit.
    pub fn force(this: &LazyLock<T, F>) -> &T {
        &this.inner
    }
}

impl<T, F> Deref for LazyLock<T, F>
where
    F: FnOnce() -> T,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// Re-export for convenience
pub use LazyLock as LazyCell;

/// Module that mimics std::sync for compatibility
pub mod sync {
    pub use super::LazyLock;
}

/// Module that mimics std::cell for compatibility  
pub mod cell {
    pub use super::LazyLock as LazyCell;
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    #[test]
    fn test_lazy_lock_basic() {
        static LAZY: LazyLock<String> = LazyLock::new(|| "hello".to_string());
        assert_eq!(&*LAZY, "hello");
    }

    #[test]
    fn test_lazy_lock_force() {
        let lazy = LazyLock::new(|| 42);
        assert_eq!(*LazyLock::force(&lazy), 42);
    }
}