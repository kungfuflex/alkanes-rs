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
        use core::ops::{Deref, DerefMut};
        
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct Box<T: ?Sized>(PhantomData<T>);
        
        impl<T> Box<T> {
            pub fn new(_value: T) -> Self {
                panic!("Box::new not supported on SPIR-V")
            }
        }
        
        impl<T: ?Sized> Box<T> {
            pub fn downcast<U: 'static>(self) -> Result<Box<U>, Box<T>>
            where
                T: core::any::Any + 'static
            {
                panic!("Box::downcast not supported on SPIR-V")
            }
        }
        
        impl<T: ?Sized> Deref for Box<T> {
            type Target = T;
            
            fn deref(&self) -> &Self::Target {
                panic!("Box::deref not supported on SPIR-V")
            }
        }
        
        impl<T: ?Sized> DerefMut for Box<T> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                panic!("Box::deref_mut not supported on SPIR-V")
            }
        }
        
        // Display implementations for specific Box types
        impl core::fmt::Display for Box<str> {
            fn fmt(&self, _f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                panic!("Box<str>::fmt not supported on SPIR-V")
            }
        }
        
        impl core::fmt::Display for Box<dyn crate::host_error::HostError> {
            fn fmt(&self, _f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                panic!("Box<dyn HostError>::fmt not supported on SPIR-V")
            }
        }
        
        // FromIterator implementation for Box<[T]>
        impl<T> core::iter::FromIterator<T> for Box<[T]> {
            fn from_iter<I: IntoIterator<Item = T>>(_iter: I) -> Self {
                panic!("Box<[T]>::from_iter not supported on SPIR-V")
            }
        }
        
        // From implementation for Box<[T]> from arrays
        impl<T, const N: usize> From<[T; N]> for Box<[T]> {
            fn from(_array: [T; N]) -> Self {
                panic!("Box<[T]>::from array not supported on SPIR-V")
            }
        }
        
        // From implementation for trait object conversion
        impl<T: crate::host_error::HostError> From<Box<T>> for Box<dyn crate::host_error::HostError> {
            fn from(_boxed: Box<T>) -> Self {
                panic!("Box<T> to Box<dyn HostError> conversion not supported on SPIR-V")
            }
        }
    }
    pub mod sync {
        use core::marker::PhantomData;
        use core::ops::Deref;
        
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct Arc<T: ?Sized>(PhantomData<T>);
        
        impl<T> Arc<T> {
            pub fn new(_value: T) -> Self {
                panic!("Arc::new not supported on SPIR-V")
            }
        }
        
        impl<T: ?Sized> Deref for Arc<T> {
            type Target = T;
            
            fn deref(&self) -> &Self::Target {
                panic!("Arc::deref not supported on SPIR-V")
            }
        }
        
        // Manual Clone implementation to handle trait objects
        impl<T: ?Sized> Clone for Arc<T> {
            fn clone(&self) -> Self {
                panic!("Arc::clone not supported on SPIR-V")
            }
        }
        
        // From trait implementations for Arc
        impl<T> From<super::vec::Vec<T>> for Arc<[T]> {
            fn from(_vec: super::vec::Vec<T>) -> Self {
                panic!("Arc::from Vec not supported on SPIR-V")
            }
        }
    }
    pub mod vec {
        use core::marker::PhantomData;
        
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct Vec<T>(PhantomData<T>);
        
        impl<T> Vec<T> {
            pub fn new() -> Self {
                panic!("Vec::new not supported on SPIR-V")
            }
            
            pub fn extend<I: IntoIterator<Item = T>>(&mut self, _: I) {
                panic!("Vec::extend not supported on SPIR-V")
            }
            
            pub fn resize(&mut self, _new_len: usize, _value: T) where T: Clone {
                panic!("Vec::resize not supported on SPIR-V")
            }
            
            pub fn copy_within<R>(&mut self, _src: R, _dest: usize)
            where
                R: core::ops::RangeBounds<usize>,
                T: Copy
            {
                panic!("Vec::copy_within not supported on SPIR-V")
            }
            
            pub fn len(&self) -> usize {
                panic!("Vec::len not supported on SPIR-V")
            }
            
            pub fn capacity(&self) -> usize {
                panic!("Vec::capacity not supported on SPIR-V")
            }
            
            pub fn as_mut_ptr(&mut self) -> *mut T {
                panic!("Vec::as_mut_ptr not supported on SPIR-V")
            }
            
            pub fn as_ptr(&self) -> *const T {
                panic!("Vec::as_ptr not supported on SPIR-V")
            }
            
            pub fn try_reserve(&mut self, _additional: usize) -> Result<(), crate::std::collections::TryReserveError> {
                panic!("Vec::try_reserve not supported on SPIR-V")
            }
            
            pub unsafe fn from_raw_parts(_ptr: *mut T, _length: usize, _capacity: usize) -> Self {
                panic!("Vec::from_raw_parts not supported on SPIR-V")
            }
            
            // Generic get/get_mut methods that handle both usize and range types
            pub fn get<I>(&self, _index: I) -> Option<&<I as core::slice::SliceIndex<[T]>>::Output>
            where
                I: core::slice::SliceIndex<[T]>,
            {
                panic!("Vec::get with SliceIndex not supported on SPIR-V")
            }
            
            pub fn get_mut<I>(&mut self, _index: I) -> Option<&mut <I as core::slice::SliceIndex<[T]>>::Output>
            where
                I: core::slice::SliceIndex<[T]>,
            {
                panic!("Vec::get_mut with SliceIndex not supported on SPIR-V")
            }
        }
        
        impl<T> FromIterator<T> for Vec<T> {
            fn from_iter<I: IntoIterator<Item = T>>(_: I) -> Self {
                panic!("Vec::from_iter not supported on SPIR-V")
            }
        }
        
        pub use core::iter::FromIterator;
    }
    pub mod string {
        use core::fmt::{Display, Formatter, Result as FmtResult};
        
        pub struct String;
        
        impl String {
            pub fn into_boxed_str(self) -> super::boxed::Box<str> {
                panic!("String::into_boxed_str not supported on SPIR-V")
            }
        }
        
        impl core::fmt::Display for String {
            fn fmt(&self, _f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                panic!("String::fmt not supported on SPIR-V")
            }
        }
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
    pub mod collections {
        #[derive(Debug, Clone)]
        pub struct TryReserveError;
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
