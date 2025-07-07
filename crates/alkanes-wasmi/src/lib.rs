//! The Wasmi virtual machine definitions.
//!
//! These closely mirror the WebAssembly specification definitions.
//! The overall structure is heavily inspired by the `wasmtime` virtual
//! machine architecture.
//!
//! # Example
//!
//! The following example shows a "Hello, World!"-like example of creating
//! a Wasm module from some initial `.wat` contents, defining a simple host
//! function and calling the exported Wasm function.
//!
//! The example was inspired by
//! [Wasmtime's API example](https://docs.rs/wasmtime/0.39.1/wasmtime/).
//!
//! ```
//! use wasmi::*;
//!
//! // In this simple example we are going to compile the below Wasm source,
//! // instantiate a Wasm module from it and call its exported "hello" function.
//! fn main() -> Result<(), wasmi::Error> {
//!     let wasm = r#"
//!         (module
//!             (import "host" "hello" (func $host_hello (param i32)))
//!             (func (export "hello")
//!                 (call $host_hello (i32.const 3))
//!             )
//!         )
//!     "#;
//!     // First step is to create the Wasm execution engine with some config.
//!     //
//!     // In this example we are using the default configuration.
//!     let engine = Engine::default();
//!     // Now we can compile the above Wasm module with the given Wasm source.
//!     let module = Module::new(&engine, wasm)?;
//!
//!     // Wasm objects operate within the context of a Wasm `Store`.
//!     //
//!     // Each `Store` has a type parameter to store host specific data.
//!     // In this example the host state is a simple `u32` type with value `42`.
//!     type HostState = u32;
//!     let mut store = Store::new(&engine, 42);
//!
//!     // A linker can be used to instantiate Wasm modules.
//!     // The job of a linker is to satisfy the Wasm module's imports.
//!     let mut linker = <Linker<HostState>>::new(&engine);
//!     // We are required to define all imports before instantiating a Wasm module.
//!     linker.func_wrap("host", "hello", |caller: Caller<'_, HostState>, param: i32| {
//!         println!("Got {param} from WebAssembly and my host state is: {}", caller.data());
//!     });
//!     let instance = linker
//!         .instantiate(&mut store, &module)?
//!         .start(&mut store)?;
//!     // Now we can finally query the exported "hello" function and call it.
//!     instance
//!         .get_typed_func::<(), ()>(&store, "hello")?
//!         .call(&mut store, ())?;
//!     Ok(())
//! }
//! ```
//!
//! # Crate Features
//!
//! | Feature | Crates | Description |
//! |:-:|:--|:--|
//! | `std` | `wasmi`<br>`wasmi_core`<br>`wasmi_ir`<br>`wasmi_collections` | Enables usage of Rust's standard library. This may have some performance advantages when enabled. Disabling this feature makes Wasmi compile on platforms that do not provide Rust's standard library such as many embedded platforms. <br><br> Enabled by default. |
//! | `wat` | `wasmi` | Enables support to parse Wat encoded Wasm modules. <br><br> Enabled by default. |
//! | `simd` | `wasmi`<br>`wasmi_core`<br>`wasmi_ir`<br>`wasmi_cli` | Enables support for the Wasm `simd` and `relaxed-simd` proposals. Note that this may introduce execution overhead and increased memory consumption for Wasm executions that do not need Wasm `simd` functionality. <br><br> Disabled by default. |
//! | `hash-collections` | `wasmi`<br>`wasmi_collections` | Enables use of hash-map based collections in Wasmi internals. This might yield performance improvements in some use cases. <br><br> Disabled by default. |
//! | `prefer-btree-collections` | `wasmi`<br>`wasmi_collections` | Enforces use of btree-map based collections in Wasmi internals. This may yield performance improvements and memory consumption decreases in some use cases. Also it enables Wasmi to run on platforms that have no random source. <br><br> Disabled by default. |
//! | `extra-checks` | `wasmi` | Enables extra runtime checks in the Wasmi executor. Expected execution overhead is ~20%. Enable this if your focus is on safety. Disable this for maximum execution performance. <br><br> Disabled by default. |

#![no_std]
#![cfg_attr(target_arch = "spirv", feature(associated_type_bounds))]
#![cfg_attr(not(target_arch = "spirv"), feature(error_in_core))]
#![cfg_attr(target_arch = "spirv", feature(error_in_core))]
#![warn(
    clippy::cast_lossless,
    clippy::missing_errors_doc,
    clippy::used_underscore_binding,
    clippy::redundant_closure_for_method_calls,
    clippy::type_repetition_in_bounds,
    clippy::inconsistent_struct_constructor,
    clippy::default_trait_access,
    clippy::items_after_statements
)]
#![recursion_limit = "1000"]

#[cfg(not(target_arch = "spirv"))]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;


/// A small "prelude" to use throughout this crate for SPIR-V compatibility.
#[cfg(target_arch = "spirv")]
pub mod prelude {
    use core::marker::PhantomData;
    
    // For SPIR-V, we provide stub implementations that panic
    // This allows the code to compile but will panic if actually used
    
    // Stub Box implementation for SPIR-V
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Box<T: ?Sized>(PhantomData<T>);
    
    impl<T> Box<T> {
        pub fn new(_value: T) -> Self {
            panic!("Box not supported on SPIR-V")
        }
    }
    
    impl<T> Box<[T]> {
        pub fn iter(&self) -> core::iter::Empty<&T> {
            panic!("Box::iter not supported on SPIR-V")
        }
        
        pub fn len(&self) -> usize {
            panic!("Box::len not supported on SPIR-V")
        }
    }
    
    impl<T> core::ops::Index<usize> for Box<[T]> {
        type Output = T;
        
        fn index(&self, _index: usize) -> &Self::Output {
            panic!("Box indexing not supported on SPIR-V")
        }
    }
    
    impl<T> core::ops::Index<core::ops::Range<usize>> for Box<[T]> {
        type Output = [T];
        
        fn index(&self, _range: core::ops::Range<usize>) -> &Self::Output {
            panic!("Box range indexing not supported on SPIR-V")
        }
    }
    
    impl<T> core::ops::Index<core::ops::RangeFrom<usize>> for Box<[T]> {
        type Output = [T];
        
        fn index(&self, _range: core::ops::RangeFrom<usize>) -> &Self::Output {
            panic!("Box range indexing not supported on SPIR-V")
        }
    }
    
    impl<T> core::ops::Deref for Box<T> {
        type Target = T;
        
        fn deref(&self) -> &Self::Target {
            panic!("Box::deref not supported on SPIR-V")
        }
    }
    
    impl<T> core::ops::DerefMut for Box<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            panic!("Box::deref_mut not supported on SPIR-V")
        }
    }
    
    impl<T> From<T> for Box<T> {
        fn from(_: T) -> Self {
            panic!("Box::from not supported on SPIR-V")
        }
    }
    
    impl<T> From<[T; 0]> for Box<[T]> {
        fn from(_: [T; 0]) -> Self {
            panic!("Box::from not supported on SPIR-V")
        }
    }
    
    impl<T> core::iter::FromIterator<T> for Box<[T]> {
        fn from_iter<I: IntoIterator<Item = T>>(_: I) -> Self {
            panic!("Box::from_iter not supported on SPIR-V")
        }
    }
    
    impl<T> IntoIterator for Box<[T]> {
        type Item = T;
        type IntoIter = core::iter::Empty<T>;
        
        fn into_iter(self) -> Self::IntoIter {
            panic!("Box::into_iter not supported on SPIR-V")
        }
    }
    
    // Stub Vec implementation for SPIR-V
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[derive(Default)]
    pub struct Vec<T>(PhantomData<T>);
    
    impl<T> Vec<T> {
        pub fn new() -> Self {
            panic!("Vec not supported on SPIR-V")
        }
        
        pub fn with_capacity(_capacity: usize) -> Self {
            panic!("Vec not supported on SPIR-V")
        }
        
        pub fn push(&mut self, _: T) {
            panic!("Vec::push not supported on SPIR-V")
        }
        
        pub fn len(&self) -> usize {
            panic!("Vec::len not supported on SPIR-V")
        }
        
        pub fn is_empty(&self) -> bool {
            panic!("Vec::is_empty not supported on SPIR-V")
        }
        
        pub fn drain(&mut self) -> Drain<'_, T> {
            panic!("Vec::drain not supported on SPIR-V")
        }
    }
    
    impl<T> core::iter::FromIterator<T> for Vec<T> {
        fn from_iter<I: IntoIterator<Item = T>>(_: I) -> Self {
            panic!("Vec::from_iter not supported on SPIR-V")
        }
    }
    
    // Stub Arc implementation for SPIR-V
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Arc<T: ?Sized>(PhantomData<T>);
    
    impl<T: ?Sized> Clone for Arc<T> {
        fn clone(&self) -> Self {
            panic!("Arc::clone not supported on SPIR-V")
        }
    }
    
    impl<T> Arc<T> {
        pub fn new(_value: T) -> Self {
            panic!("Arc not supported on SPIR-V")
        }
        
        pub fn get_mut(_this: &mut Self) -> Option<&mut T> {
            panic!("Arc::get_mut not supported on SPIR-V")
        }
    }
    
    impl<T> From<T> for Arc<T> {
        fn from(_: T) -> Self {
            panic!("Arc::from not supported on SPIR-V")
        }
    }
    
    // Stub Weak implementation for SPIR-V
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Weak<T: ?Sized>(PhantomData<T>);
    
    impl<T> Weak<T> {
        pub fn new() -> Self {
            panic!("Weak not supported on SPIR-V")
        }
        
        pub fn upgrade(&self) -> Option<Arc<T>> {
            panic!("Weak::upgrade not supported on SPIR-V")
        }
    }
    
    // Stub BTreeMap implementation for SPIR-V
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BTreeMap<K, V>(PhantomData<(K, V)>);
    
    impl<K, V> BTreeMap<K, V> {
        pub fn new() -> Self {
            panic!("BTreeMap not supported on SPIR-V")
        }
        
        pub fn entry(&mut self, _key: K) -> Entry<'_, K, V> {
            panic!("BTreeMap::entry not supported on SPIR-V")
        }
        
        pub fn get(&self, _key: &K) -> Option<&V> {
            panic!("BTreeMap::get not supported on SPIR-V")
        }
        
        pub fn contains_key(&self, _key: &K) -> bool {
            panic!("BTreeMap::contains_key not supported on SPIR-V")
        }
        
        pub fn iter(&self) -> core::iter::Empty<(&K, &V)> {
            panic!("BTreeMap::iter not supported on SPIR-V")
        }
    }
    
    impl<K, V> Default for BTreeMap<K, V> {
        fn default() -> Self {
            Self::new()
        }
    }
    
    // Stub Drain implementation for SPIR-V
    #[derive(Debug)]
    pub struct Drain<'a, T>(PhantomData<&'a T>);
    
    impl<'a, T> Iterator for Drain<'a, T> {
        type Item = T;
        
        fn next(&mut self) -> Option<Self::Item> {
            panic!("Drain not supported on SPIR-V")
        }
    }
    
    // Stub Entry implementation for SPIR-V
    #[derive(Debug)]
    pub enum Entry<'a, K, V> {
        Occupied(OccupiedEntry<'a, K, V>),
        Vacant(VacantEntry<'a, K, V>),
    }
    
    #[derive(Debug)]
    pub struct OccupiedEntry<'a, K, V>(PhantomData<&'a (K, V)>);
    
    impl<'a, K, V> OccupiedEntry<'a, K, V> {
        pub fn insert(&mut self, _value: V) -> V {
            panic!("OccupiedEntry::insert not supported on SPIR-V")
        }
    }
    
    #[derive(Debug)]
    pub struct VacantEntry<'a, K, V>(PhantomData<&'a (K, V)>);
    
    impl<'a, K, V> VacantEntry<'a, K, V> {
        pub fn insert(self, _value: V) -> &'a mut V {
            panic!("VacantEntry::insert not supported on SPIR-V")
        }
    }
    
    // Stub BTreeSet implementation for SPIR-V
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BTreeSet<T>(PhantomData<T>);
    
    impl<T> BTreeSet<T> {
        pub fn new() -> Self {
            panic!("BTreeSet not supported on SPIR-V")
        }
        
        pub fn insert(&mut self, _value: T) -> bool {
            panic!("BTreeSet::insert not supported on SPIR-V")
        }
        
        pub fn contains(&self, _value: &T) -> bool {
            panic!("BTreeSet::contains not supported on SPIR-V")
        }
    }
    
    impl<T> Default for BTreeSet<T> {
        fn default() -> Self {
            Self::new()
        }
    }
    
    // Stub String implementation for SPIR-V
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct String(PhantomData<()>);
    
    impl String {
        pub fn new() -> Self {
            panic!("String not supported on SPIR-V")
        }
        
        pub fn from(_s: &str) -> Self {
            panic!("String::from not supported on SPIR-V")
        }
        
        pub fn push_str(&mut self, _s: &str) {
            panic!("String::push_str not supported on SPIR-V")
        }
    }
    
    impl Default for String {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// A small "prelude" to use throughout this crate for non-SPIR-V targets.
#[cfg(not(target_arch = "spirv"))]
pub mod prelude {
    pub use alloc::{boxed::Box, vec::Vec, sync::{Arc, Weak}, collections::{BTreeMap, BTreeSet}, string::String};
    pub use alloc::vec::Drain;
    pub use alloc::collections::btree_map::Entry;
}

// For SPIR-V compatibility, we need to replace standard alloc types
#[cfg(target_arch = "spirv")]
pub use prelude::*;

#[cfg(not(target_arch = "spirv"))]
pub use prelude as alloc_prelude;

#[macro_use]
mod foreach_tuple;

#[cfg(test)]
pub mod tests;

mod engine;
mod error;
mod externref;
mod func;
mod global;
mod instance;
mod limits;
mod linker;
mod memory;
mod module;
mod store;
mod table;
mod value;

/// Definitions from the `wasmi_core` crate.
#[doc(inline)]
pub use wasmi_core as core;

/// Definitions from the `wasmi_collections` crate.
#[doc(inline)]
use wasmi_collections as collections;

/// Definitions from the `wasmi_collections` crate.
#[doc(inline)]
use wasmi_ir as ir;

/// Defines some errors that may occur upon interaction with Wasmi.
pub mod errors {
    pub use super::{
        engine::EnforcedLimitsError,
        error::ErrorKind,
        func::FuncError,
        ir::Error as IrError,
        linker::LinkerError,
        module::{InstantiationError, ReadError},
    };
    pub use crate::core::{FuelError, GlobalError, MemoryError, TableError};
}

pub use self::{
    core::{GlobalType, Mutability},
    engine::{
        CompilationMode,
        Config,
        EnforcedLimits,
        Engine,
        EngineWeak,
        ResumableCall,
        ResumableCallHostTrap,
        ResumableCallOutOfFuel,
        StackLimits,
        TypedResumableCall,
        TypedResumableCallHostTrap,
        TypedResumableCallOutOfFuel,
    },
    error::Error,
    externref::ExternRef,
    func::{
        Caller,
        Func,
        FuncRef,
        FuncType,
        IntoFunc,
        TypedFunc,
        WasmParams,
        WasmResults,
        WasmRet,
        WasmTy,
        WasmTyList,
    },
    global::Global,
    instance::{Export, ExportsIter, Extern, ExternType, Instance},
    limits::{StoreLimits, StoreLimitsBuilder},
    linker::{state, Linker, LinkerBuilder},
    memory::{Memory, MemoryType, MemoryTypeBuilder},
    module::{
        CustomSection,
        CustomSectionsIter,
        ExportType,
        ImportType,
        InstancePre,
        Module,
        ModuleExportsIter,
        ModuleImportsIter,
        Read,
    },
    store::{AsContext, AsContextMut, CallHook, Store, StoreContext, StoreContextMut},
    table::{Table, TableType},
    value::Val,
};
use self::{
    func::{FuncEntity, FuncIdx},
    global::GlobalIdx,
    instance::{InstanceEntity, InstanceEntityBuilder, InstanceIdx},
    memory::{DataSegmentEntity, DataSegmentIdx, MemoryIdx},
    store::Stored,
    table::{ElementSegment, ElementSegmentIdx, TableIdx},
};
