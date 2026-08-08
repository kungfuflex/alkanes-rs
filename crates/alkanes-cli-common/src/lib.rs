#![cfg_attr(not(feature = "std"), no_std)]

//! Deezel Common Library
//!
//! This library provides the core functionality for the deezel project,
//! designed to be WASM-compatible and platform-agnostic.
//!
//! The library is structured around trait abstractions that allow the same
//! business logic to work across different environments:
//! - Native CLI applications
//! - WASM web applications
//! - Testing environments
//!
//! ## Architecture
//!
//! The library is organized into several key modules:
//! - `traits`: Core trait abstractions for platform independence
//! - `wallet`: Bitcoin wallet functionality with BDK integration
//! - `alkanes`: Smart contract operations and inspection
pub mod crypto;
pub mod crypto_worker;
/// - `runestone`: Runestone analysis and decoding
/// - `network`: Network parameter management
/// - `rpc`: RPC client abstractions
/// - `address_resolver`: Address identifier resolution
/// - `monitor`: Blockchain monitoring
/// - `transaction`: Transaction construction and signing
/// - `utils`: Common utilities
pub mod provider;
extern crate alloc;

#[cfg(not(feature = "std"))]
pub use alloc::{
    string::{String, ToString},
    format,
    vec,
    vec::Vec,
};

#[cfg(feature = "std")]
pub use std::{
    string::{String, ToString},
    format,
    vec,
    vec::Vec,
};

pub mod vendored_ord;

// Conversion utilities for protobuf types
pub mod conversion;

// Core modules
pub mod address;
#[cfg(feature = "std")]
pub mod commands;
pub mod traits;
pub mod pending_tx_store;
pub mod network;
pub mod rpc;
pub mod alkanes;
pub mod bridge;
pub mod brc20_prog;
#[cfg(feature = "std")]
pub mod brc20_prog_rpc;
#[cfg(feature = "std")]
pub mod brc20_prog_rpc_types;
pub mod wallet;
pub mod address_resolver;
pub mod address_parser;
pub mod runestone;
pub mod runestone_analysis;
pub mod runestone_enhanced;
pub mod transaction;
pub mod monitor;
pub mod utils;
pub mod trace;
pub mod keystore;
pub mod esplora;
pub mod psbt_utils;
pub mod unwrap;
pub mod lua_script;
pub mod bitcoind;
pub mod ord;
pub mod params_parser;
pub mod metashrew;
pub mod index_pointer;
pub mod byte_view;
pub mod proto;
pub mod subfrost;
pub mod ordinals;
#[cfg(feature = "std")]
pub mod buildinfo;

// New-protocol WalletConnect signer for SUBFROST mobile vc=419+.
// Gated on the wc-signer feature so non-WC consumers don't pay the
// crypto/transport dep cost. Native impl (file storage + frtun-pair
// dialer + reqwest pair-wake) requires `wc-signer-native`; wasm callers
// only get the codec + traits.
#[cfg(feature = "wc-signer")]
pub mod wc_signer;

#[cfg(feature = "std")]
pub mod cache;

#[cfg(feature = "std")]
pub mod dataapi;

#[cfg(feature = "std")]
pub mod opi;

#[cfg(any(test, feature = "test-utils"))]
pub mod mock_provider;

// Re-export key types and traits for convenience
pub use traits::*;

pub use rpc::{RpcClient, RpcRequest, RpcResponse};
pub use network::{RpcConfig, RpcError, DeezelNetwork};

// Re-export common types for WASM compatibility - already imported above

// Re-export external types for convenience
pub use bitcoin::{Network, Transaction, Address, ScriptBuf};
pub use crate::alkanes::protostone::Protostone;
pub use serde_json::Value as JsonValue;
pub use alkanes_support::proto::alkanes as alkanes_pb;

/// Error types for the deezel-common library
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlkanesError {
    JsonRpc(String),
    RpcError(String),
    Storage(String),
    Network(String),
    Wallet(String),
    Alkanes(String),
    Runestone(String),
    Serialization(String),
    Validation(String),
    Configuration(String),
    InvalidParameters(String),
    AddressResolution(String),
    InvalidUrl(String),
    Transaction(String),
    Monitor(String),
    WasmExecution(String),
    Crypto(String),
    Io(String),
    Parse(String),
    Pgp(String),
    Hex(String),
    Armor(String),
    NotImplemented(String),
    NotConfigured(String),
    WalletNotAvailable(String),
    JsError(String),
    NoAddressFound,
    UncompressedPublicKey,
    /// Coin selection could not fund the transaction, with the numbers needed
    /// to say by how much — see [`AlkanesError::bitcoin_shortfall`].
    ///
    /// Callers previously had to regex the message to learn anything
    /// actionable, so the wallet UI could only say "not enough BTC" and leave
    /// the user to guess a smaller amount. `needed` and `collected` are now
    /// carried structurally.
    ///
    /// `Display` renders BYTE-IDENTICALLY to the string this replaced
    /// (`Wallet("Insufficient funds: need N sats, have M")`) — existing
    /// parsers in the CLI, mobile and web consumers keep working unchanged.
    /// The `insufficient_funds_message_is_byte_identical` test pins that.
    InsufficientBitcoin { needed: u64, collected: u64 },
    Other(String),
    Protobuf(String),
    CodegenError(String),  // For Huff bytecode generation errors
}

impl AlkanesError {
    /// Sats the wallet was short, or `None` if this is not a funding failure.
    ///
    /// ## This number is directly actionable
    ///
    /// Reducing the spend amount by the shortfall makes the transaction fund —
    /// exactly, not approximately. The amount a caller spends is an output
    /// value (`B:amount:vN`), so dropping it by `S` drops
    /// `total_bitcoin_needed` by `S` one-for-one, while the output COUNT is
    /// unchanged so the vsize and therefore the fee stay put. The new
    /// requirement is `needed - S == collected`.
    ///
    /// That holds for multi-transaction flows too: reducing what the user
    /// spends does not move a CPFP package's carrier funding, which is sized
    /// off the fee rate and the package vbytes rather than the amount.
    ///
    /// Spending to the exact maximum leaves zero change, which is only safe
    /// because `build_psbt_and_fee` now drops a sub-dust change output into
    /// the fee instead of emitting one the validator rejects. Before that fix
    /// this number would have pointed callers straight at a different failure.
    pub fn bitcoin_shortfall(&self) -> Option<u64> {
        match self {
            AlkanesError::InsufficientBitcoin { needed, collected } => {
                Some(needed.saturating_sub(*collected))
            }
            _ => None,
        }
    }

    /// The largest amount that WOULD have funded, given the amount attempted.
    /// Saturates at zero — a wallet can be short by more than it was spending
    /// (fees alone can exceed the balance).
    pub fn max_spendable_given(&self, attempted_amount: u64) -> Option<u64> {
        self.bitcoin_shortfall()
            .map(|short| attempted_amount.saturating_sub(short))
    }
}

impl core::fmt::Display for AlkanesError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AlkanesError::JsonRpc(msg) => write!(f, "JSON-RPC error: {msg}"),
            AlkanesError::RpcError(msg) => write!(f, "RPC error: {msg}"),
            AlkanesError::Storage(msg) => write!(f, "Storage error: {msg}"),
            AlkanesError::Network(msg) => write!(f, "Network error: {msg}"),
            AlkanesError::Wallet(msg) => write!(f, "Wallet error: {msg}"),
            AlkanesError::Alkanes(msg) => write!(f, "Alkanes error: {msg}"),
            AlkanesError::Runestone(msg) => write!(f, "Runestone error: {msg}"),
            AlkanesError::Serialization(msg) => write!(f, "Serialization error: {msg}"),
            AlkanesError::Validation(msg) => write!(f, "Validation error: {msg}"),
            AlkanesError::Configuration(msg) => write!(f, "Configuration error: {msg}"),
            AlkanesError::InvalidParameters(msg) => write!(f, "Invalid parameters: {msg}"),
            AlkanesError::AddressResolution(msg) => write!(f, "Address resolution error: {msg}"),
            AlkanesError::InvalidUrl(msg) => write!(f, "Invalid URL: {msg}"),
            AlkanesError::Transaction(msg) => write!(f, "Transaction error: {msg}"),
            AlkanesError::Monitor(msg) => write!(f, "Monitoring error: {msg}"),
            AlkanesError::WasmExecution(msg) => write!(f, "WASM execution error: {msg}"),
            AlkanesError::Crypto(msg) => write!(f, "Cryptography error: {msg}"),
            AlkanesError::Io(msg) => write!(f, "I/O error: {msg}"),
            AlkanesError::Parse(msg) => write!(f, "Parse error: {msg}"),
            AlkanesError::Pgp(msg) => write!(f, "PGP error: {msg}"),
            AlkanesError::Hex(msg) => write!(f, "Hex error: {msg}"),
            AlkanesError::Armor(msg) => write!(f, "Armor error: {msg}"),
            AlkanesError::NotImplemented(msg) => write!(f, "Not implemented: {msg}"),
            AlkanesError::NotConfigured(msg) => write!(f, "Not configured: {msg}"),
            AlkanesError::WalletNotAvailable(msg) => write!(f, "Wallet not available: {msg}"),
            AlkanesError::JsError(msg) => write!(f, "JavaScript error: {msg}"),
            AlkanesError::NoAddressFound => write!(f, "No address found"),
            AlkanesError::UncompressedPublicKey => write!(f, "Uncompressed public key error"),
            // Byte-identical to the `Wallet(...)` string this replaced —
            // including the "Wallet error: " prefix Display added. Consumers
            // regex this today; the structured fields are additive.
            AlkanesError::InsufficientBitcoin { needed, collected } => write!(
                f,
                "Wallet error: Insufficient funds: need {needed} sats, have {collected}"
            ),
            AlkanesError::Other(msg) => write!(f, "Other error: {msg}"),
            AlkanesError::Protobuf(msg) => write!(f, "Protobuf error: {msg}"),
            AlkanesError::CodegenError(msg) => write!(f, "Codegen error: {msg}"),
        }
    }
}

impl From<bitcoin::key::UncompressedPublicKeyError> for AlkanesError {
    fn from(_: bitcoin::key::UncompressedPublicKeyError) -> Self {
        AlkanesError::UncompressedPublicKey
    }
}

impl From<core::convert::Infallible> for AlkanesError {
    fn from(never: core::convert::Infallible) -> Self {
        match never {}
    }
}

// WASM-compatible error trait implementation
#[cfg(target_arch = "wasm32")]
impl AlkanesError {
    /// Get the error source (WASM-compatible alternative to std::error::Error::source)
    pub fn source(&self) -> Option<&dyn core::fmt::Display> {
        None // For now, we don't chain errors in WASM
    }
}

// Implement error trait for both WASM and non-WASM targets
// This is needed for anyhow compatibility
#[cfg(feature = "std")]
impl std::error::Error for AlkanesError {}

// For anyhow compatibility, we need to implement conversion from AlkanesError to anyhow::Error
// This is needed for the ? operator to work with anyhow::Result

/// Result type for deezel-common operations
pub type Result<T> = core::result::Result<T, AlkanesError>;

/// Convert anyhow::Error to AlkanesError
impl From<anyhow::Error> for AlkanesError {
    fn from(err: anyhow::Error) -> Self {
        AlkanesError::Wallet(alloc::format!("{err}"))
    }
}

/// Convert serde_json::Error to AlkanesError
impl From<serde_json::Error> for AlkanesError {
    fn from(err: serde_json::Error) -> Self {
        AlkanesError::Serialization(alloc::format!("{err}"))
    }
}

impl From<prost::DecodeError> for AlkanesError {
    fn from(err: prost::DecodeError) -> Self {
        AlkanesError::Serialization(format!("Prost decode error: {err}"))
    }
}

impl From<prost::EncodeError> for AlkanesError {
    fn from(err: prost::EncodeError) -> Self {
        AlkanesError::Serialization(format!("Prost encode error: {err}"))
    }
}

impl From<bitcoin::address::ParseError> for AlkanesError {
    fn from(err: bitcoin::address::ParseError) -> Self {
        AlkanesError::AddressResolution(format!("{err:?}"))
    }
}

impl From<bitcoin::address::FromScriptError> for AlkanesError {
    fn from(err: bitcoin::address::FromScriptError) -> Self {
        AlkanesError::AddressResolution(format!("{err:?}"))
    }
}


impl From<bitcoin::sighash::TaprootError> for AlkanesError {
    fn from(err: bitcoin::sighash::TaprootError) -> Self {
        AlkanesError::Transaction(format!("{err:?}"))
    }
}

impl From<bitcoin::sighash::P2wpkhError> for AlkanesError {
    fn from(err: bitcoin::sighash::P2wpkhError) -> Self {
        AlkanesError::Transaction(format!("{err:?}"))
    }
}

/// Convert bitcoin::consensus::encode::Error to AlkanesError
impl From<bitcoin::consensus::encode::Error> for AlkanesError {
    fn from(err: bitcoin::consensus::encode::Error) -> Self {
        AlkanesError::Transaction(alloc::format!("{err}"))
    }
}

impl From<bitcoin::blockdata::transaction::ParseOutPointError> for AlkanesError {
    fn from(err: bitcoin::blockdata::transaction::ParseOutPointError) -> Self {
        AlkanesError::Transaction(format!("{err:?}"))
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for AlkanesError {
    fn from(err: std::io::Error) -> Self {
        AlkanesError::Io(format!("{err:?}"))
    }
}

impl From<bitcoin::psbt::Error> for AlkanesError {
    fn from(err: bitcoin::psbt::Error) -> Self {
        AlkanesError::Transaction(format!("PSBT error: {err}"))
    }
}

impl From<bitcoin::psbt::ExtractTxError> for AlkanesError {
    fn from(err: bitcoin::psbt::ExtractTxError) -> Self {
        AlkanesError::Transaction(format!("PSBT extraction error: {err}"))
    }
}


impl From<hex::FromHexError> for AlkanesError {
    fn from(err: hex::FromHexError) -> Self {
        AlkanesError::Hex(format!("{err:?}"))
    }
}

impl From<core::num::ParseIntError> for AlkanesError {
    fn from(err: core::num::ParseIntError) -> Self {
        AlkanesError::Parse(format!("Failed to parse integer: {err}"))
    }
}

impl From<bitcoin::hashes::hex::HexToBytesError> for AlkanesError {
    fn from(err: bitcoin::hashes::hex::HexToBytesError) -> Self {
        AlkanesError::Hex(format!("{err:?}"))
    }
}

impl From<bitcoin::bip32::Error> for AlkanesError {
    fn from(err: bitcoin::bip32::Error) -> Self {
        AlkanesError::Wallet(format!("{err:?}"))
    }
}

impl From<bip39::Error> for AlkanesError {
    fn from(err: bip39::Error) -> Self {
        AlkanesError::Wallet(format!("BIP39 error: {err}"))
    }
}

impl From<bitcoin::secp256k1::Error> for AlkanesError {
    fn from(err: bitcoin::secp256k1::Error) -> Self {
        AlkanesError::Crypto(format!("{err:?}"))
    }
}

impl From<bitcoin::hashes::hex::HexToArrayError> for AlkanesError {
    fn from(err: bitcoin::hashes::hex::HexToArrayError) -> Self {
        AlkanesError::Hex(format!("{err:?}"))
    }
}

#[cfg(feature = "native-deps")]
impl From<reqwest::Error> for AlkanesError {
    fn from(err: reqwest::Error) -> Self {
        AlkanesError::Network(format!("{err:?}"))
    }
}

impl From<alkanes_asc::errors::Error> for AlkanesError {
    fn from(err: alkanes_asc::errors::Error) -> Self {
        AlkanesError::Pgp(err.to_string())
    }
}

impl From<alloc::string::FromUtf8Error> for AlkanesError {
    fn from(err: alloc::string::FromUtf8Error) -> Self {
        AlkanesError::Parse(err.to_string())
    }
}

/// Version information
pub const DEEZEL_COMMON_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Initialize the library (for WASM compatibility)
#[cfg(target_arch = "wasm32")]
pub fn init() {
    // WASM initialization would go here
    // Set up panic hook, logging, etc.
}

/// Initialize the library (no-op for native)
#[cfg(not(target_arch = "wasm32"))]
pub fn init() {
    // No initialization needed for native
}

/// Utility functions for common operations
pub mod prelude {
    pub use crate::traits::*;
    pub use crate::index_pointer::{StubPointer};
    pub use crate::{AlkanesError, Result};
    pub use crate::address::{DeezelAddress, NetworkConfig};
pub use crate::rpc::{RpcClient};
pub use crate::network::{RpcConfig, DeezelNetwork};
    pub use bitcoin::{Network, Transaction, Address, ScriptBuf};
    pub use ordinals::Runestone;
    pub use crate::alkanes::protostone::Protostone;
}

#[cfg(test)]
pub mod tests;

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_version_info() {
        // The version is a constant and will never be empty.
        // This assert is for demonstration purposes.
        assert_eq!(NAME, "alkanes-cli-common");
    }
    
    #[test]
    fn test_error_conversions() {
        let anyhow_err = anyhow::anyhow!("test error");
        let deezel_err: AlkanesError = anyhow_err.into();
        assert!(matches!(deezel_err, AlkanesError::Wallet(_)));
        
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let deezel_err: AlkanesError = json_err.into();
        assert!(matches!(deezel_err, AlkanesError::Serialization(_)));
    }
}

#[cfg(test)]
mod insufficient_bitcoin_tests {
    //! `AlkanesError::InsufficientBitcoin` — the shortfall the wallet UI needs.
    //!
    //! Before this variant, coin-selection failure was a formatted string, so
    //! the only way a caller could act on it was to regex the message. In
    //! practice nobody did the arithmetic, so surfaces guessed a fee reserve
    //! instead — and a guess that is short by a few thousand sats is exactly
    //! the 2026-08-06 report ("need 14933522 sats, have 14928129").
    use super::AlkanesError;

    #[test]
    fn insufficient_funds_message_is_byte_identical() {
        // The whole point of the additive design: every existing consumer
        // (CLI output, mobile, the web app's humanizeError regex) keeps
        // matching. If this assertion ever needs changing, downstream string
        // parsers break — check them first.
        let typed = AlkanesError::InsufficientBitcoin {
            needed: 14_933_522,
            collected: 14_928_129,
        };
        let legacy = AlkanesError::Wallet(
            "Insufficient funds: need 14933522 sats, have 14928129".to_string(),
        );
        assert_eq!(typed.to_string(), legacy.to_string());
        assert_eq!(
            typed.to_string(),
            "Wallet error: Insufficient funds: need 14933522 sats, have 14928129",
        );
    }

    #[test]
    fn shortfall_is_the_reported_gap() {
        let err = AlkanesError::InsufficientBitcoin {
            needed: 14_933_522,
            collected: 14_928_129,
        };
        assert_eq!(err.bitcoin_shortfall(), Some(5_393));
    }

    #[test]
    fn max_spendable_given_lands_exactly_on_the_fundable_amount() {
        // The reported attempt: 0.14920313 BTC. Reducing it by the shortfall
        // must leave a requirement equal to what the wallet actually holds —
        // the amount is an output value, so it moves the requirement 1:1 while
        // the output count (and therefore the fee) stays put.
        let attempted = 14_920_313u64;
        let needed = 14_933_522u64;
        let collected = 14_928_129u64;
        let err = AlkanesError::InsufficientBitcoin { needed, collected };

        let max = err.max_spendable_given(attempted).unwrap();
        assert_eq!(max, 14_914_920);

        let new_requirement = needed - (attempted - max);
        assert_eq!(new_requirement, collected, "must fund exactly, not approximately");
    }

    #[test]
    fn max_spendable_saturates_when_fees_alone_exceed_the_balance() {
        // Short by more than the user was even spending — a dust wallet at a
        // high fee rate. Must not underflow into a huge u64.
        let err = AlkanesError::InsufficientBitcoin { needed: 50_000, collected: 1_000 };
        assert_eq!(err.bitcoin_shortfall(), Some(49_000));
        assert_eq!(err.max_spendable_given(10_000), Some(0));
    }

    #[test]
    fn other_errors_carry_no_shortfall() {
        assert_eq!(AlkanesError::Wallet("nope".into()).bitcoin_shortfall(), None);
        assert_eq!(AlkanesError::Other("nope".into()).max_spendable_given(1), None);
    }
}
