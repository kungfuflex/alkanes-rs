#![cfg_attr(not(feature = "std"), no_std)]

//! Alkanes Common Library
// ... (rest of the file content from deezel-common/src/lib.rs, with DeezelError -> AlkanesError)
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

impl From<protobuf::Error> for AlkanesError {
    fn from(err: protobuf::Error) -> Self {
        AlkanesError::Protobuf(err.to_string())
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

#[cfg(target_arch = "wasm32")]
impl AlkanesError {
    pub fn source(&self) -> Option<&dyn core::fmt::Display> {
        None
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AlkanesError {}

pub type Result<T> = core::result::Result<T, AlkanesError>;

impl From<anyhow::Error> for AlkanesError {
    fn from(err: anyhow::Error) -> Self {
        AlkanesError::Wallet(alloc::format!("{err}"))
    }
}

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

impl From<bip39::ErrorKind> for AlkanesError {
    fn from(err: bip39::ErrorKind) -> Self {
        AlkanesError::Wallet(format!("BIP39 error: {err:?}"))
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

impl From<alloc::string::FromUtf8Error> for AlkanesError {
    fn from(err: alloc::string::FromUtf8Error) -> Self {
        AlkanesError::Parse(err.to_string())
    }
}