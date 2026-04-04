//! qubitcoin-crypto: Cryptographic primitives for Qubitcoin.
//!
//! Wraps secp256k1 and bitcoin_hashes to provide:
//! - SHA256d (double SHA256)
//! - RIPEMD160, Hash160, SHA256
//! - SipHash-2-4
//! - secp256k1 ECDSA and Schnorr operations
//!
//! Maps to: src/crypto/ in Bitcoin Core

/// Cryptographic hash functions (SHA256, SHA256d, RIPEMD160, Hash160, etc.).
pub mod hash;
/// MuHash3072 rolling hash for UTXO set commitment.
pub mod muhash;
/// SipHash-2-4 keyed hash function for compact blocks and hash table randomization.
pub mod siphash;

// Re-export secp256k1 for downstream crates
/// Re-export of the `bitcoin_hashes` crate for downstream use.
pub use bitcoin_hashes;
/// Re-export of the `secp256k1` crate for ECDSA and Schnorr signature operations.
pub use secp256k1;
