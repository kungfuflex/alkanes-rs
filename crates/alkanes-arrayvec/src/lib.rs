//! **arrayvec** provides the types [`ArrayVec`] and [`ArrayString`]: 
//! array-backed vector and string types, which store their contents inline.
//!
//! The arrayvec package has the following cargo features:
//!
//! - `std`
//!   - Optional, enabled by default
//!   - Use libstd; disable to use `no_std` instead.
//!
//! - `serde`
//!   - Optional
//!   - Enable serialization for ArrayVec and ArrayString using serde 1.x
//!
//! - `zeroize`
//!   - Optional
//!   - Implement `Zeroize` for ArrayVec and ArrayString
//!
//! ## Rust Version
//!
//! This version of arrayvec requires Rust 1.51 or later.
//!
#![doc(html_root_url="https://docs.rs/arrayvec/0.7/")]
#![cfg_attr(not(feature="std"), no_std)]

#[cfg(feature="serde")]
extern crate serde;

// SPIR-V compatibility: Always use core for SPIR-V target
#[cfg(target_arch = "spirv")]
extern crate core as std;

#[cfg(all(not(feature="std"), not(target_arch = "spirv")))]
extern crate core as std;

#[cfg(not(target_pointer_width = "16"))]
pub(crate) type LenUint = u32;

#[cfg(target_pointer_width = "16")]
pub(crate) type LenUint = u16;

macro_rules! assert_capacity_limit {
    ($cap:expr) => {
        if std::mem::size_of::<usize>() > std::mem::size_of::<LenUint>() {
            if $cap > LenUint::MAX as usize {
                #[cfg(not(target_pointer_width = "16"))]
                panic!("ArrayVec: largest supported capacity is u32::MAX");
                #[cfg(target_pointer_width = "16")]
                panic!("ArrayVec: largest supported capacity is u16::MAX");
            }
        }
    }
}

macro_rules! assert_capacity_limit_const {
    ($cap:expr) => {
        if std::mem::size_of::<usize>() > std::mem::size_of::<LenUint>() {
            if $cap > LenUint::MAX as usize {
                [/*ArrayVec: largest supported capacity is u32::MAX*/][$cap]
            }
        }
    }
}

// SPIR-V compatibility: Use stub implementations for SPIR-V target
#[cfg(target_arch = "spirv")]
mod spirv_stubs;

#[cfg(not(target_arch = "spirv"))]
mod arrayvec_impl;
#[cfg(not(target_arch = "spirv"))]
mod arrayvec;
#[cfg(not(target_arch = "spirv"))]
mod array_string;
#[cfg(not(target_arch = "spirv"))]
mod char;
#[cfg(not(target_arch = "spirv"))]
mod errors;
#[cfg(not(target_arch = "spirv"))]
mod utils;

#[cfg(target_arch = "spirv")]
pub use crate::spirv_stubs::{ArrayString, ArrayVec, IntoIter, Drain, CapacityError};

#[cfg(not(target_arch = "spirv"))]
pub use crate::array_string::ArrayString;
#[cfg(not(target_arch = "spirv"))]
pub use crate::errors::CapacityError;
#[cfg(not(target_arch = "spirv"))]
pub use crate::arrayvec::{ArrayVec, IntoIter, Drain};
