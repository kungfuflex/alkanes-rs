// #![feature(let_chains)]
#![cfg_attr(not(feature = "std"), no_std)]

//! Alkanes Common Library
//!
//! This library provides the core functionality for the alkanes project,
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
pub mod native_provider;
pub mod error;

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
pub use error::AlkanesError;

pub use rpc::{RpcClient, RpcRequest, RpcResponse};
pub use network::{RpcConfig, RpcError, AlkanesNetwork};

// Re-export common types for WASM compatibility - already imported above

// Re-export external types for convenience
pub use bitcoin::{Network, Transaction, Address, ScriptBuf};
pub use crate::alkanes::protostone::Protostone;
pub use serde_json::Value as JsonValue;
pub use alkanes_support::proto::alkanes as alkanes_pb;

/// Result type for alkanes-common operations
pub type Result<T> = core::result::Result<T, AlkanesError>;

/// Version information
pub const ALKANES_CLI_COMMON_VERSION: &str = env!("CARGO_PKG_VERSION");
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
pub use crate::network::{RpcConfig, AlkanesNetwork};
    pub use bitcoin::{Network, Transaction, Address, ScriptBuf};
    
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