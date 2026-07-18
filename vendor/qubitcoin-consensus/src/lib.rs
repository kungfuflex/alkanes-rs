//! Embeddable consensus library for Qubitcoin.
//!
//! Zero I/O, `no_std`-optional. Pure functions for consensus validation.
//!
//! Maps to: `bitcoin_consensus` static lib + `consensus/` directory in Bitcoin Core.
//!
//! # Provided functionality
//!
//! - **Transaction types**: [`OutPoint`], [`TxIn`], [`TxOut`], [`Transaction`],
//!   [`Block`], [`BlockHeader`]
//! - **Consensus validation**: [`check_transaction`], [`check_proof_of_work`],
//!   [`get_block_subsidy`], Merkle root computation
//! - **Signature hashing**: legacy, BIP143 (segwit v0), and BIP341 (taproot)
//! - **Signature verification**: [`TransactionSignatureChecker`] for ECDSA and Schnorr
//! - **Network parameters**: [`ConsensusParams`] for mainnet, testnet, regtest, signet
//! - **Error reporting**: [`ValidationState`] with typed result codes

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

/// Block and block header types. Maps to `src/primitives/block.h` in Bitcoin Core.
pub mod block;
/// Context-free consensus validation functions (transaction checks, PoW, subsidy).
pub mod check;
/// Merkle tree computation for block transaction and witness roots.
pub mod merkle;
/// Consensus parameters for different networks (mainnet, testnet, regtest, signet).
pub mod params;
/// Signature hash computation for legacy, segwit v0 (BIP143), and taproot (BIP341).
pub mod sighash;
/// Transaction signature verification (ECDSA and Schnorr).
pub mod sign;
/// Transaction primitive types: [`OutPoint`], [`TxIn`], [`TxOut`], [`Transaction`].
pub mod transaction;
/// Validation state types for structured error reporting.
pub mod validation_state;

pub use block::{Block, BlockHeader};
pub use check::{
    check_proof_of_work, check_transaction, get_block_subsidy, get_legacy_sigop_count,
    get_p2sh_sigop_count, get_transaction_sigop_cost, MAX_BLOCK_SIGOPS_COST, MAX_BLOCK_WEIGHT,
    WITNESS_SCALE_FACTOR,
};
pub use merkle::{block_merkle_root, block_witness_merkle_root};
pub use params::ConsensusParams;
pub use sighash::{
    remove_codeseparators, signature_hash, taproot_signature_hash, witness_v0_signature_hash,
    PrecomputedTransactionData, SIGHASH_ALL, SIGHASH_ANYONECANPAY, SIGHASH_NONE, SIGHASH_SINGLE,
};
pub use sign::TransactionSignatureChecker;
pub use transaction::{
    OutPoint, Transaction, TransactionRef, TxIn, TxOut, Witness, MAX_SEQUENCE_NONFINAL,
    SEQUENCE_FINAL, SEQUENCE_LOCKTIME_DISABLE_FLAG, SEQUENCE_LOCKTIME_GRANULARITY,
    SEQUENCE_LOCKTIME_MASK, SEQUENCE_LOCKTIME_TYPE_FLAG,
};
pub use validation_state::{BlockValidationResult, TxValidationResult, ValidationState};

/// Compatibility conversions between qubitcoin types and `rust-bitcoin` 0.32 types.
///
/// Gated behind the `rust-bitcoin-compat` feature.
#[cfg(feature = "rust-bitcoin-compat")]
pub mod compat;
