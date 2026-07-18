//! Common types and utilities for Qubitcoin.
//!
//! Maps to: the `bitcoin_common` static library in Bitcoin Core.
//!
//! This crate provides shared types used across the Qubitcoin node, including
//! block-index and chain types, chain parameters, UTXO coin types and caching,
//! cryptographic key wrappers, proof-of-work utilities, and batch flush logic.

/// Optimized batch flush for the UTXO cache. See [`batch_flush::FlushConfig`].
pub mod batch_flush;
/// Block index, chain types, and skip-list helpers. Equivalent to `src/chain.h` in Bitcoin Core.
pub mod chain;
/// Full chain parameters for each supported network. Equivalent to `src/kernel/chainparams.h` in Bitcoin Core.
pub mod chainparams;
/// UTXO coin types, cache hierarchy, and database-backed views. Equivalent to `src/coins.h` in Bitcoin Core.
pub mod coins;
/// Cryptographic key types (private keys, public keys, x-only keys). Equivalent to `src/key.h` / `src/pubkey.h` in Bitcoin Core.
pub mod keys;
/// Proof-of-work difficulty adjustment logic. Equivalent to `src/pow.cpp` in Bitcoin Core.
pub mod pow;
