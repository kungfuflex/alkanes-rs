//! Compatibility module that provides std::sync::LazyLock for older Rust versions

use crate::LazyLock;

/// Module that mimics std::sync for LazyLock compatibility
pub mod sync {
    pub use crate::LazyLock;
}