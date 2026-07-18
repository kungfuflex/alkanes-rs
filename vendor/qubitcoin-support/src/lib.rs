//! qubitcoin-support — generic traits for qubitcoin indexer authoring.
//!
//! Build-agnostic: no host functions, no WASM imports. Defines the
//! interfaces that secondary and tertiary indexer crates implement.

pub mod byte_view;
pub mod index_pointer;

pub use byte_view::ByteView;
pub use index_pointer::KeyValuePointer;
