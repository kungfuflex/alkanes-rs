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

// Core modules
pub mod address;
#[cfg(feature = "std")]
pub mod commands;
pub mod traits;
pub mod network;
pub mod rpc;
pub mod alkanes;
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
pub mod bitcoind;
pub mod ord;
pub mod metashrew;
pub mod index_pointer;
pub mod byte_view;
pub mod proto;

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
    Other(String),
    Protobuf(String),
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
            AlkanesError::Other(msg) => write!(f, "Other error: {msg}"),
            AlkanesError::Protobuf(msg) => write!(f, "Protobuf error: {msg}"),
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
        assert_eq!(NAME, "alkanes-common");
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
