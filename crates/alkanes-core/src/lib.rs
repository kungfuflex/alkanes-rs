#![no_std]
#![cfg_attr(not(target_arch = "spirv"), feature(error_in_core))]
#![cfg_attr(not(target_arch = "spirv"), feature(iter_repeat_n))]
#![warn(
    clippy::cast_lossless,
    clippy::missing_errors_doc,
    clippy::used_underscore_binding,
    clippy::redundant_closure_for_method_calls,
    clippy::type_repetition_in_bounds,
    clippy::inconsistent_struct_constructor,
    clippy::default_trait_access,
    clippy::map_unwrap_or,
    clippy::items_after_statements
)]

mod float;
mod fuel;
mod func_type;
mod global;
pub mod hint;
mod host_error;
mod index_ty;
mod limiter;
mod memory;
mod table;
mod trap;
mod typed;
mod untyped;
mod value;
pub mod wasm;

#[cfg(feature = "simd")]
pub mod simd;

#[cfg(not(target_arch = "spirv"))]
extern crate alloc;
#[cfg(all(feature = "std", not(target_arch = "spirv")))]
extern crate std;

// For SPIR-V, we need to provide stub alloc and std modules
#[cfg(target_arch = "spirv")]
mod alloc {
    pub mod boxed {
        use core::marker::PhantomData;
        pub struct Box<T: ?Sized>(PhantomData<T>);
    }
    pub mod sync {
        use core::marker::PhantomData;
        pub struct Arc<T: ?Sized>(PhantomData<T>);
    }
    pub mod vec {
        use core::marker::PhantomData;
        pub struct Vec<T>(PhantomData<T>);
    }
    pub mod string {
        pub struct String;
    }
    pub mod slice {
        pub fn from_raw_parts<T>(_: *const T, _: usize) -> &'static [T] {
            panic!("slice::from_raw_parts not supported on SPIR-V")
        }
        pub fn from_raw_parts_mut<T>(_: *mut T, _: usize) -> &'static mut [T] {
            panic!("slice::from_raw_parts_mut not supported on SPIR-V")
        }
    }
}

#[cfg(target_arch = "spirv")]
mod std {
    pub mod error {
        pub trait Error {}
    }
}

use self::value::{Float, Integer, SignExtendFrom, TruncateSaturateInto, TryTruncateInto};
pub use self::{
    float::{F32, F64},
    fuel::{Fuel, FuelCosts, FuelCostsProvider, FuelError},
    func_type::{FuncType, FuncTypeError},
    global::{Global, GlobalError, GlobalType, Mutability},
    host_error::HostError,
    index_ty::IndexType,
    limiter::{LimiterError, ResourceLimiter, ResourceLimiterRef},
    memory::{Memory, MemoryError, MemoryType, MemoryTypeBuilder},
    table::{ElementSegment, ElementSegmentRef, Table, TableError, TableType},
    trap::{Trap, TrapCode},
    typed::{Typed, TypedVal},
    untyped::{DecodeUntypedSlice, EncodeUntypedSlice, ReadAs, UntypedError, UntypedVal, WriteAs},
    value::{ValType, V128},
};
