//! SPIR-V compatible bump allocator
//! 
//! This crate provides bump allocation functionality that works across different targets:
//! - SPIR-V: Uses alkanes-alloc infrastructure with fixed-size arena
//! - Other targets: Re-exports the full bumpalo functionality
//!
//! For SPIR-V targets, this provides a real bump allocator implementation using
//! the AlkanesAllocator trait from alkanes-alloc.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(target_arch = "spirv", no_std)]

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_arch = "spirv")] {
        mod spirv_bump;
        pub use spirv_bump::*;
        
        // Re-export alkanes-alloc types for convenience
        pub use alkanes_alloc::{AlkanesAllocator, AllocError};
    } else if #[cfg(feature = "bumpalo")] {
        // For non-SPIR-V targets, re-export the original bumpalo
        pub use bumpalo::*;
        
        // Also provide alkanes-alloc compatibility
        pub use alkanes_alloc::{AlkanesAllocator, AllocError};
        
        mod bumpalo_adapter;
        pub use bumpalo_adapter::*;
    } else {
        // Fallback: just provide alkanes-alloc compatibility
        pub use alkanes_alloc::{AlkanesAllocator, AllocError};
        
        // Provide a simple bump allocator interface
        mod simple_bump;
        pub use simple_bump::*;
    }
}

// Common error type that works across all targets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlkanesBumpError {
    OutOfMemory,
    InvalidSize,
    InvalidAlignment,
}

impl From<AllocError> for AlkanesBumpError {
    fn from(err: AllocError) -> Self {
        match err {
            AllocError::OutOfMemory => AlkanesBumpError::OutOfMemory,
            AllocError::InvalidSize => AlkanesBumpError::InvalidSize,
            AllocError::InvalidAlignment => AlkanesBumpError::InvalidAlignment,
        }
    }
}

#[cfg(all(feature = "bumpalo", not(target_arch = "spirv")))]
impl From<bumpalo::AllocErr> for AlkanesBumpError {
    fn from(_: bumpalo::AllocErr) -> Self {
        AlkanesBumpError::OutOfMemory
    }
}
