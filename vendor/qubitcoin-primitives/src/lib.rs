//! qubitcoin-primitives: Primitive types for Qubitcoin.
//!
//! Maps to: src/uint256.h, src/arith_uint256.h, src/consensus/amount.h
//!
//! Provides:
//! - `Uint256`, `Uint160`: Fixed-size opaque byte blobs (for hashes)
//! - `ArithUint256`: 256-bit integer with full arithmetic (for difficulty)
//! - `Amount`: Satoshi amount type
//! - `Txid`, `Wtxid`, `BlockHash`: Typed hash wrappers

/// Satoshi amount type and consensus monetary constants.
pub mod amount;
/// 256-bit unsigned integer with full arithmetic for difficulty calculations.
pub mod arith_uint256;
/// Type-safe newtype wrappers for transaction, witness, and block hashes.
pub mod hash_types;
/// Fixed-size opaque byte blob types (`Uint256`, `Uint160`) for hash storage.
/// Maps to `base_blob` in Bitcoin Core (`uint256.h`).
pub mod uint256;

// Convenient re-exports
/// Re-export of monetary types and constants from the [`amount`] module.
pub use amount::{money_range, Amount, COIN, MAX_MONEY};
/// Re-export of [`ArithUint256`] and conversion functions from the [`arith_uint256`] module.
pub use arith_uint256::{arith_to_uint256, uint256_to_arith, ArithUint256};
/// Re-export of typed hash wrappers from the [`hash_types`] module.
pub use hash_types::{BlockHash, Txid, Wtxid};
/// Re-export of fixed-size blob types from the [`uint256`] module.
pub use uint256::{Uint160, Uint256};
