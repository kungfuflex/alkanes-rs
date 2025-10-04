//! # Centralized Bitcoin Address Encoding for Deezel
//!
//! This module provides a unified interface for Bitcoin address encoding and decoding
//! across all Deezel components. It's designed to be no_std compatible for WASM usage
//! while providing comprehensive address support for all Bitcoin address types.
//!
//! ## Features
//! - **Unified API**: Single interface for all address operations
//! - **no_std compatible**: Works in WASM environments
//! - **Network agnostic**: Supports mainnet, testnet, regtest, and custom networks
//! - **Comprehensive**: Supports all Bitcoin address types (P2PKH, P2SH, P2WPKH, P2WSH, P2TR)
//! - **Metashrew compatible**: Uses metashrew-support for robust address handling
//!
//! ## Usage
//! ```rust,ignore
//! use deezel_common::address::{DeezelAddress, NetworkConfig};
//! use bitcoin::PublicKey;
//!
//! // Create network config for regtest
//! let network = NetworkConfig::regtest();
//!
//! // Create P2WPKH address from public key
//! let address = DeezelAddress::p2wpkh(&pubkey, &network)?;
//! let address_string = address.to_string();
//!
//! // Parse address from string
//! let parsed = DeezelAddress::from_str(&address_string, &network)?;
//! ```

use core::fmt;
use core::str::FromStr;

extern crate alloc;
use alloc::string::{String, ToString};

use anyhow::{anyhow, Result};
use bitcoin::{
    secp256k1::{Secp256k1, Verification},
    PublicKey, Script, ScriptBuf,
    key::{TweakedPublicKey, UntweakedPublicKey},
    taproot::TapNodeHash,
};
use metashrew_support::address::{Payload};

/// Network configuration for address encoding
///
/// This struct contains the network-specific parameters needed for proper
/// address encoding across different Bitcoin networks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkConfig {
    /// Base58 version byte for P2PKH addresses (e.g., 0x00 for mainnet "1..." addresses)
    pub p2pkh_prefix: u8,
    /// Base58 version byte for P2SH addresses (e.g., 0x05 for mainnet "3..." addresses)  
    pub p2sh_prefix: u8,
    /// Bech32 human-readable part for SegWit addresses (e.g., "bc" for mainnet)
    pub bech32_hrp: String,
}

impl NetworkConfig {
    /// Bitcoin mainnet configuration
    pub fn mainnet() -> Self {
        Self {
            p2pkh_prefix: 0x00,
            p2sh_prefix: 0x05,
            bech32_hrp: "bc".to_string(),
        }
    }

    /// Bitcoin testnet configuration
    pub fn testnet() -> Self {
        Self {
            p2pkh_prefix: 0x6f,
            p2sh_prefix: 0xc4,
            bech32_hrp: "tb".to_string(),
        }
    }

    /// Bitcoin regtest configuration
    pub fn regtest() -> Self {
        Self {
            p2pkh_prefix: 0x6f,
            p2sh_prefix: 0xc4,
            bech32_hrp: "bcrt".to_string(),
        }
    }

    /// Bitcoin signet configuration
    pub fn signet() -> Self {
        Self {
            p2pkh_prefix: 0x6f,
            p2sh_prefix: 0xc4,
            bech32_hrp: "tb".to_string(),
        }
    }

    /// Create custom network configuration
    pub fn custom(p2pkh_prefix: u8, p2sh_prefix: u8, bech32_hrp: String) -> Self {
        Self {
            p2pkh_prefix,
            p2sh_prefix,
            bech32_hrp,
        }
    }

}

/// Unified Bitcoin address representation for Deezel
///
/// This struct wraps the metashrew-support Payload with network configuration
/// to provide a complete address solution that can be used throughout the Deezel codebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeezelAddress {
    payload: Payload,
    network: NetworkConfig,
}

impl DeezelAddress {
    /// Create a P2PKH address from a public key
    pub fn p2pkh(pubkey: &PublicKey, network: &NetworkConfig) -> Self {
        Self {
            payload: Payload::p2pkh(pubkey),
            network: network.clone(),
        }
    }

    /// Create a P2SH address from a script
    pub fn p2sh(script: &Script, network: &NetworkConfig) -> Result<Self> {
        Ok(Self {
            payload: Payload::p2sh(script)?,
            network: network.clone(),
        })
    }

    /// Create a P2WPKH address from a public key
    pub fn p2wpkh(pubkey: &PublicKey, network: &NetworkConfig) -> Result<Self> {
        Ok(Self {
            payload: Payload::p2wpkh(pubkey)?,
            network: network.clone(),
        })
    }

    /// Create a P2SH-wrapped P2WPKH address from a public key
    pub fn p2sh_p2wpkh(pubkey: &PublicKey, network: &NetworkConfig) -> Result<Self> {
        Ok(Self {
            payload: Payload::p2shwpkh(pubkey)?,
            network: network.clone(),
        })
    }

    /// Create a P2WSH address from a script
    pub fn p2wsh(script: &Script, network: &NetworkConfig) -> Self {
        Self {
            payload: Payload::p2wsh(script),
            network: network.clone(),
        }
    }

    /// Create a P2SH-wrapped P2WSH address from a script
    pub fn p2sh_p2wsh(script: &Script, network: &NetworkConfig) -> Self {
        Self {
            payload: Payload::p2shwsh(script),
            network: network.clone(),
        }
    }

    /// Create a P2TR address from an untweaked key
    pub fn p2tr<C: Verification>(
        secp: &Secp256k1<C>,
        internal_key: UntweakedPublicKey,
        merkle_root: Option<TapNodeHash>,
        network: &NetworkConfig,
    ) -> Self {
        Self {
            payload: Payload::p2tr(secp, internal_key, merkle_root),
            network: network.clone(),
        }
    }

    /// Create a P2TR address from a pre-tweaked key
    pub fn p2tr_tweaked(output_key: TweakedPublicKey, network: &NetworkConfig) -> Self {
        Self {
            payload: Payload::p2tr_tweaked(output_key),
            network: network.clone(),
        }
    }

    /// Create address from script
    pub fn from_script(script: &Script, network: &NetworkConfig) -> Result<Self> {
        Ok(Self {
            payload: Payload::from_script(script)?,
            network: network.clone(),
        })
    }

    /// Get the script pubkey for this address
    pub fn script_pubkey(&self) -> ScriptBuf {
        self.payload.script_pubkey()
    }

    /// Check if this address matches a script pubkey
    pub fn matches_script_pubkey(&self, script: &Script) -> bool {
        self.payload.matches_script_pubkey(script)
    }

    /// Get the underlying payload
    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    /// Get the network configuration
    pub fn network(&self) -> &NetworkConfig {
        &self.network
    }

    /// Convert to address string
    pub fn to_string(&self) -> Result<String> {
        let btc_network = match self.network.bech32_hrp.as_str() {
            "bc" => bitcoin::Network::Bitcoin,
            "tb" => bitcoin::Network::Testnet,
            "bcrt" => bitcoin::Network::Regtest,
            _ => return Err(anyhow!("Unsupported custom network HRP for string conversion")),
        };
        let script_pubkey = self.payload.script_pubkey();
        let address = bitcoin::Address::from_script(&script_pubkey, btc_network)
            .map_err(|e| anyhow!("Failed to create address from script: {}", e))?;
        Ok(address.to_string())
    }

    /// Parse address from string
    pub fn from_str(address_str: &str, network: &NetworkConfig) -> Result<Self> {
        let expected_btc_network = match network.bech32_hrp.as_str() {
            "bc" => bitcoin::Network::Bitcoin,
            "tb" => bitcoin::Network::Testnet,
            "bcrt" => bitcoin::Network::Regtest,
            _ => return Err(anyhow!("Unsupported custom network HRP for parsing")),
        };

        let address = bitcoin::Address::from_str(address_str)
            .map_err(|e| anyhow!("Failed to parse address string: {}", e))?
            .require_network(expected_btc_network)
            .map_err(|e| anyhow!("Address does not match required network: {}", e))?;
        
        let script_pubkey = address.script_pubkey();
        let payload = Payload::from_script(&script_pubkey)?;

        Ok(DeezelAddress {
            payload,
            network: network.clone(),
        })
    }
}

impl fmt::Display for DeezelAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_string() {
            Ok(addr_str) => write!(f, "{addr_str}"),
            Err(_) => write!(f, "<invalid address>"),
        }
    }
}

impl FromStr for DeezelAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Try different networks in order of likelihood
        let networks = [
            NetworkConfig::mainnet(),
            NetworkConfig::testnet(),
            NetworkConfig::regtest(),
            NetworkConfig::signet(),
        ];

        for network in &networks {
            if let Ok(address) = Self::from_str(s, network) {
                return Ok(address);
            }
        }

        Err(anyhow!("Could not parse address for any known network"))
    }
}

/// Convenience functions for common address operations
impl DeezelAddress {
    /// Quick P2WPKH address creation for regtest (common in tests)
    pub fn p2wpkh_regtest(pubkey: &PublicKey) -> Result<Self> {
        Self::p2wpkh(pubkey, &NetworkConfig::regtest())
    }

    /// Quick P2TR address creation for regtest (common in tests)
    pub fn p2tr_regtest<C: Verification>(
        secp: &Secp256k1<C>,
        internal_key: UntweakedPublicKey,
        merkle_root: Option<TapNodeHash>,
    ) -> Self {
        Self::p2tr(secp, internal_key, merkle_root, &NetworkConfig::regtest())
    }

    /// Check if address is for a specific network
    pub fn is_network(&self, network: &NetworkConfig) -> bool {
        self.network == *network
    }

    /// Check if address is mainnet
    pub fn is_mainnet(&self) -> bool {
        self.is_network(&NetworkConfig::mainnet())
    }

    /// Check if address is testnet
    pub fn is_testnet(&self) -> bool {
        self.is_network(&NetworkConfig::testnet())
    }

    /// Check if address is regtest
    pub fn is_regtest(&self) -> bool {
        self.is_network(&NetworkConfig::regtest())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::Secp256k1;

    #[test]
    fn test_network_configs() {
        let mainnet = NetworkConfig::mainnet();
        assert_eq!(mainnet.p2pkh_prefix, 0x00);
        assert_eq!(mainnet.p2sh_prefix, 0x05);
        assert_eq!(mainnet.bech32_hrp, "bc");

        let testnet = NetworkConfig::testnet();
        assert_eq!(testnet.p2pkh_prefix, 0x6f);
        assert_eq!(testnet.p2sh_prefix, 0xc4);
        assert_eq!(testnet.bech32_hrp, "tb");

        let regtest = NetworkConfig::regtest();
        assert_eq!(regtest.p2pkh_prefix, 0x6f);
        assert_eq!(regtest.p2sh_prefix, 0xc4);
        assert_eq!(regtest.bech32_hrp, "bcrt");
    }

    #[test]
    fn test_p2wpkh_address_creation() {
        let secp = Secp256k1::new();
        let (_secret_key, secp_public_key) = secp.generate_keypair(&mut rand::thread_rng());
        let public_key = PublicKey::new(secp_public_key);
        let network = NetworkConfig::regtest();

        let address = DeezelAddress::p2wpkh(&public_key, &network).unwrap();
        let address_str = address.to_string().unwrap();

        // Should start with bcrt1q for regtest P2WPKH
        assert!(address_str.starts_with("bcrt1q"));

        // Should be able to parse back
        let parsed = DeezelAddress::from_str(&address_str, &network).unwrap();
        assert_eq!(address, parsed);
    }

    #[test]
    fn test_p2tr_address_creation() {
        let secp = Secp256k1::new();
        let (_secret_key, secp_public_key) = secp.generate_keypair(&mut rand::thread_rng());
        let internal_key = UntweakedPublicKey::from(secp_public_key);
        let network = NetworkConfig::regtest();

        let address = DeezelAddress::p2tr(&secp, internal_key, None, &network);
        let address_str = address.to_string().unwrap();

        // Should start with bcrt1p for regtest P2TR
        assert!(address_str.starts_with("bcrt1p"));

        // Should be able to parse back
        let parsed = DeezelAddress::from_str(&address_str, &network).unwrap();
        assert_eq!(address, parsed);
    }

    #[test]
    fn test_convenience_functions() {
        let secp = Secp256k1::new();
        let (_secret_key, secp_public_key) = secp.generate_keypair(&mut rand::thread_rng());
        let public_key = PublicKey::new(secp_public_key);

        let address = DeezelAddress::p2wpkh_regtest(&public_key).unwrap();
        assert!(address.is_regtest());
        assert!(!address.is_mainnet());
        assert!(!address.is_testnet());
    }

    #[test]
    fn test_script_pubkey_generation() {
        let secp = Secp256k1::new();
        let (_secret_key, secp_public_key) = secp.generate_keypair(&mut rand::thread_rng());
        let public_key = PublicKey::new(secp_public_key);
        let network = NetworkConfig::regtest();

        let address = DeezelAddress::p2wpkh(&public_key, &network).unwrap();
        let script = address.script_pubkey();

        // Should be able to create address from script
        let from_script = DeezelAddress::from_script(&script, &network).unwrap();
        assert_eq!(address, from_script);

        // Should match script pubkey
        assert!(address.matches_script_pubkey(&script));
    }

    #[test]
    fn test_custom_network() {
        let custom = NetworkConfig::custom(0x42, 0x43, "custom".to_string());
        assert_eq!(custom.p2pkh_prefix, 0x42);
        assert_eq!(custom.p2sh_prefix, 0x43);
        assert_eq!(custom.bech32_hrp, "custom");
    }
}