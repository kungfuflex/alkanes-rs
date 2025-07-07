//! SPIR-V compatible LEB128 formatting library
//! 
//! This crate provides LEB128 encoding and decoding functionality that works
//! across different targets including SPIR-V compute shaders.
//! 
//! For SPIR-V targets, this provides stub implementations that panic.
//! For other targets, this re-exports the full leb128fmt functionality.

#![cfg_attr(not(feature = "std"), no_std)]

// For SPIR-V targets, provide stub implementations
#[cfg(target_arch = "spirv")]
mod spirv_stubs {
    use core::fmt;

    /// Stub error type for SPIR-V
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Error;

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("leb128fmt not supported on SPIR-V")
        }
    }

    impl Error {
        pub const fn is_more_bytes_needed(&self) -> bool {
            false
        }

        pub const fn is_invalid_encoding(&self) -> bool {
            true
        }
    }

    /// Stub trait for unsigned integers
    pub trait UInt {
        const BITS: u32;
    }

    impl UInt for u8 { const BITS: u32 = 8; }
    impl UInt for u16 { const BITS: u32 = 16; }
    impl UInt for u32 { const BITS: u32 = 32; }
    impl UInt for u64 { const BITS: u32 = 64; }
    impl UInt for u128 { const BITS: u32 = 128; }

    /// Stub trait for signed integers
    pub trait SInt {
        const BITS: u32;
    }

    impl SInt for i8 { const BITS: u32 = 8; }
    impl SInt for i16 { const BITS: u32 = 16; }
    impl SInt for i32 { const BITS: u32 = 32; }
    impl SInt for i64 { const BITS: u32 = 64; }
    impl SInt for i128 { const BITS: u32 = 128; }

    // Stub functions that panic
    pub const fn max_len<const BITS: u32>() -> usize {
        panic!("leb128fmt::max_len not supported on SPIR-V")
    }

    pub const fn is_last(_byte: u8) -> bool {
        panic!("leb128fmt::is_last not supported on SPIR-V")
    }

    pub const fn encode_u32(_value: u32) -> Option<([u8; 5], usize)> {
        panic!("leb128fmt::encode_u32 not supported on SPIR-V")
    }

    pub const fn encode_u64(_value: u64) -> Option<([u8; 10], usize)> {
        panic!("leb128fmt::encode_u64 not supported on SPIR-V")
    }

    pub const fn encode_fixed_u32(_value: u32) -> Option<[u8; 5]> {
        panic!("leb128fmt::encode_fixed_u32 not supported on SPIR-V")
    }

    pub const fn encode_fixed_u64(_value: u64) -> Option<[u8; 10]> {
        panic!("leb128fmt::encode_fixed_u64 not supported on SPIR-V")
    }

    pub const fn decode_u32(_input: [u8; 5]) -> Option<(u32, usize)> {
        panic!("leb128fmt::decode_u32 not supported on SPIR-V")
    }

    pub const fn decode_u64(_input: [u8; 10]) -> Option<(u64, usize)> {
        panic!("leb128fmt::decode_u64 not supported on SPIR-V")
    }

    pub fn encode_s32(_value: i32) -> Option<([u8; 5], usize)> {
        panic!("leb128fmt::encode_s32 not supported on SPIR-V")
    }

    pub fn encode_s64(_value: i64) -> Option<([u8; 10], usize)> {
        panic!("leb128fmt::encode_s64 not supported on SPIR-V")
    }

    pub const fn encode_fixed_s32(_value: i32) -> Option<[u8; 5]> {
        panic!("leb128fmt::encode_fixed_s32 not supported on SPIR-V")
    }

    pub const fn encode_fixed_s64(_value: i64) -> Option<[u8; 10]> {
        panic!("leb128fmt::encode_fixed_s64 not supported on SPIR-V")
    }

    pub const fn decode_s32(_input: [u8; 5]) -> Option<(i32, usize)> {
        panic!("leb128fmt::decode_s32 not supported on SPIR-V")
    }

    pub const fn decode_s64(_input: [u8; 10]) -> Option<(i64, usize)> {
        panic!("leb128fmt::decode_s64 not supported on SPIR-V")
    }

    pub fn encode_uint_slice<T, const BITS: u32>(
        _value: T,
        _output: &mut [u8],
        _pos: &mut usize,
    ) -> Option<usize>
    where
        T: UInt,
    {
        panic!("leb128fmt::encode_uint_slice not supported on SPIR-V")
    }

    pub fn encode_fixed_uint_slice<T, const BITS: u32>(
        _value: T,
        _output: &mut [u8],
        _pos: &mut usize,
    ) -> Option<usize>
    where
        T: UInt,
    {
        panic!("leb128fmt::encode_fixed_uint_slice not supported on SPIR-V")
    }

    pub fn decode_uint_slice<T, const BITS: u32>(
        _input: &[u8],
        _pos: &mut usize,
    ) -> Result<T, Error>
    where
        T: UInt,
    {
        panic!("leb128fmt::decode_uint_slice not supported on SPIR-V")
    }

    pub fn encode_sint_slice<T, const BITS: u32>(
        _value: T,
        _output: &mut [u8],
        _pos: &mut usize,
    ) -> Option<usize>
    where
        T: SInt,
    {
        panic!("leb128fmt::encode_sint_slice not supported on SPIR-V")
    }

    pub fn encode_fixed_sint_slice<T, const BITS: u32>(
        _value: T,
        _output: &mut [u8],
        _pos: &mut usize,
    ) -> Option<usize>
    where
        T: SInt,
    {
        panic!("leb128fmt::encode_fixed_sint_slice not supported on SPIR-V")
    }

    pub fn decode_sint_slice<T, const BITS: u32>(
        _input: &[u8],
        _pos: &mut usize,
    ) -> Result<T, Error>
    where
        T: SInt,
    {
        panic!("leb128fmt::decode_sint_slice not supported on SPIR-V")
    }

    // Re-export macros as stub macros
    #[macro_export]
    macro_rules! encode_uint_arr {
        ($func:ident, $num_ty:ty, $bits:literal) => {
            pub const fn $func(_value: $num_ty) -> Option<([u8; (($bits / 7) + if $bits % 7 == 0 { 0 } else { 1 }) as usize], usize)> {
                panic!("leb128fmt macro not supported on SPIR-V")
            }
        };
    }

    #[macro_export]
    macro_rules! encode_fixed_uint_arr {
        ($func:ident, $num_ty:ty, $bits:literal) => {
            pub const fn $func(_value: $num_ty) -> Option<[u8; (($bits / 7) + if $bits % 7 == 0 { 0 } else { 1 }) as usize]> {
                panic!("leb128fmt macro not supported on SPIR-V")
            }
        };
    }

    #[macro_export]
    macro_rules! decode_uint_arr {
        ($func:ident, $num_ty:ty, $bits:literal) => {
            pub const fn $func(_input: [u8; (($bits / 7) + if $bits % 7 == 0 { 0 } else { 1 }) as usize]) -> Option<($num_ty, usize)> {
                panic!("leb128fmt macro not supported on SPIR-V")
            }
        };
    }

    #[macro_export]
    macro_rules! encode_sint_arr {
        ($func:ident, $num_ty:ty, $bits:literal) => {
            pub fn $func(_value: $num_ty) -> Option<([u8; (($bits / 7) + if $bits % 7 == 0 { 0 } else { 1 }) as usize], usize)> {
                panic!("leb128fmt macro not supported on SPIR-V")
            }
        };
    }

    #[macro_export]
    macro_rules! encode_fixed_sint_arr {
        ($func:ident, $num_ty:ty, $bits:literal) => {
            pub const fn $func(_value: $num_ty) -> Option<[u8; (($bits / 7) + if $bits % 7 == 0 { 0 } else { 1 }) as usize]> {
                panic!("leb128fmt macro not supported on SPIR-V")
            }
        };
    }

    #[macro_export]
    macro_rules! decode_sint_arr {
        ($func:ident, $num_ty:ty, $bits:literal) => {
            pub const fn $func(_input: [u8; (($bits / 7) + if $bits % 7 == 0 { 0 } else { 1 }) as usize]) -> Option<($num_ty, usize)> {
                panic!("leb128fmt macro not supported on SPIR-V")
            }
        };
    }
}

// For non-SPIR-V targets, re-export the actual implementation
#[cfg(not(target_arch = "spirv"))]
pub use crate::leb128fmt::*;

// For SPIR-V targets, use stub implementations
#[cfg(target_arch = "spirv")]
pub use spirv_stubs::*;

// Include the actual implementation as a module for non-SPIR-V targets
#[cfg(not(target_arch = "spirv"))]
#[path = "../leb128fmt/src/lib.rs"]
mod leb128fmt;