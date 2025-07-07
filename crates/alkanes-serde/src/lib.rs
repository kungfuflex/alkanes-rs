//! Re-export of the serde crate for alkanes compatibility
//! 
//! This crate provides a no_std compatible version of serde
//! that can be used in SPIR-V and other constrained environments.

#![no_std]

// Re-export everything from the actual serde crate
pub use serde::*;