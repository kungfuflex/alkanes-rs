//! Support library for WebAssembly Bitcoin indexers
//!
//! This crate provides essential utilities and abstractions for building Bitcoin
//! indexers that run in WebAssembly environments. It includes Bitcoin-specific
//! data structures, address handling, block parsing, and key-value storage abstractions.
//!
//! # Architecture
//!
//! The support library is organized into several key modules:
//!
//! ## Core Abstractions
//! - [`index_pointer`]: Key-value storage abstraction with hierarchical keys
//! - [`byte_view`]: Serialization trait for converting types to/from bytes
//! - [`utils`]: Bitcoin consensus encoding/decoding utilities
//!
//! ## Bitcoin Primitives
//! - [`block`]: Extended block parsing with AuxPoW support
//! - [`address`]: Comprehensive Bitcoin address handling
//! - [`proto`]: Protocol buffer definitions for data exchange
//!
//! ## Compatibility
//! - [`compat`]: Compatibility layer for different Bitcoin implementations
//!
//! # Key Features
//!
//! ## Index Pointer System
//! The [`index_pointer`] module provides a powerful abstraction for hierarchical
//! key-value storage that enables complex data structures to be built on top
//! of simple key-value stores.
//!
//! ## Extended Block Support
//! The [`block`] module extends standard Bitcoin block parsing to support
//! AuxPoW (Auxiliary Proof of Work) blocks used by merged-mined cryptocurrencies.
//!
//! ## Comprehensive Address Support
//! The [`address`] module provides complete Bitcoin address handling including
//! legacy, SegWit, and Taproot address types with proper encoding/decoding.
//!
//! # Usage Patterns
//!
//! ## Basic Key-Value Operations
//! ```rust,ignore
//! use metashrew_support::index_pointer::KeyValuePointer;
//!
//! // Create hierarchical keys
//! let base_ptr = IndexPointer::from_keyword("balances");
//! let user_ptr = base_ptr.keyword("user123");
//!
//! // Store and retrieve values
//! user_ptr.set_value(1000u64);
//! let balance: u64 = user_ptr.get_value();
//! ```
//!
//! ## Block Processing
//! ```rust,ignore
//! use metashrew_support::block::AuxpowBlock;
//!
//! // Parse extended blocks with AuxPoW support
//! let block = AuxpowBlock::parse(&mut cursor)?;
//! let consensus_block = block.to_consensus();
//! ```
//!
//! ## Address Handling
//! ```rust,ignore
//! use metashrew_support::address::{Payload, AddressType};
//!
//! // Create and validate Bitcoin addresses
//! let payload = Payload::p2pkh(&public_key);
//! let script = payload.script_pubkey();
//! ```

pub mod address;
pub mod block;
pub mod byte_view;
pub mod conservative_memory_test;
pub mod index_pointer;
pub mod lru_cache_memory_detection_test;
pub mod utils;

// Re-export commonly used items
pub use byte_view::ByteView;
pub use index_pointer::KeyValuePointer;
