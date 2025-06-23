//! Generic synchronization framework for Metashrew indexers
//!
//! This crate provides generic traits and utilities for building blockchain
//! synchronization systems that work with different storage backends.

pub mod traits;

pub use traits::*;

/// Re-export commonly used types
pub use anyhow::{Error, Result};
pub use async_trait::async_trait;