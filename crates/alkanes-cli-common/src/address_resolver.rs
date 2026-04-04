//! Address resolution system for handling address identifiers
//!
//! This module provides functionality to resolve address identifiers like:
//! - \[self:p2tr:0\] - Full format with wallet reference
//! - p2tr:0 - Shorthand format
//! - [external:bc1q...] - External address reference
//! - Raw Bitcoin addresses

use crate::{Result, AlkanesError};
use crate::traits::DeezelProvider;
use prost::Message as ProstMessage;
#[allow(unused_imports)]
use crate::NetworkProvider;
use crate::wallet::AddressType;
use bitcoin::Network;
use regex::Regex;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    collections::HashMap,
    str::FromStr,
    vec,
    vec::Vec,
    string::String,
};
#[cfg(target_arch = "wasm32")]
use alloc::{
    collections::BTreeMap as HashMap,
    str::FromStr,
    vec::Vec,
    string::{String, ToString},
    format,
};

/// Address identifier types
#[derive(Debug, Clone, PartialEq)]
pub enum AddressIdentifier {
    /// Self-wallet address with type and index
    SelfWallet { address_type: AddressType, index: u32 },
    /// External address reference
    External { address: String },
    /// Raw Bitcoin address (no identifier)
    Raw { address: String },
}

/// Address resolver that works with any provider
pub struct AddressResolver<P: DeezelProvider> {
    provider: P,
    cache: HashMap<String, String>,
}

impl<P: DeezelProvider> AddressResolver<P> {
    /// Create a new address resolver
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            cache: HashMap::new(),
        }
    }
    
    /// Check if string contains identifiers
    pub fn contains_identifiers(&self, input: &str) -> bool {
        !self.find_identifiers(input).is_empty()
    }
    
    /// Find all identifiers in a string
    pub fn find_identifiers(&self, input: &str) -> Vec<String> {
        let mut identifiers = Vec::new();
        
        // Pattern for full identifiers: [self:p2tr:0], [external:bc1q...]
        let full_pattern = Regex::new(r"\[([^]]+)\]").unwrap();
        for cap in full_pattern.captures_iter(input) {
            if let Some(identifier) = cap.get(1) {
                identifiers.push(format!("[{}]", identifier.as_str()));
            }
        }
        
        // Pattern for shorthand identifiers: p2tr:0, p2wpkh:5, etc.
        if identifiers.is_empty() && self.is_shorthand_identifier(input) {
            identifiers.push(input.to_string());
        }
        
        identifiers
    }
    
    /// Check if string is a shorthand identifier
    pub fn is_shorthand_identifier(&self, input: &str) -> bool {
        let parts: Vec<&str> = input.split(':').collect();
        
        if parts.is_empty() || parts.len() > 2 {
            return false;
        }
        
        // Check if first part is a valid address type
        let address_type = parts[0].to_lowercase();
        let valid_types = ["p2pk", "p2tr", "p2pkh", "p2sh", "p2wpkh", "p2wsh", "p2sh-p2wpkh", "p2sh-p2wsh",
                           "subfrost-alkanes", "subfrost-brc20"];

        if !valid_types.contains(&address_type.as_str()) {
            return false;
        }
        
        // If there's a second part, it should be a valid index or range
        if parts.len() == 2 {
            // Check if it's a single index
            if parts[1].parse::<u32>().is_ok() {
                return true;
            }
            // Check if it's a range (e.g., "0-50")
            if parts[1].contains('-') {
                let range_parts: Vec<&str> = parts[1].split('-').collect();
                if range_parts.len() == 2 {
                    return range_parts[0].parse::<u32>().is_ok() && range_parts[1].parse::<u32>().is_ok();
                }
            }
            return false;
        }
        
        true
    }
    
    /// Parse an identifier string
    pub fn parse_identifier(&self, identifier: &str) -> Result<AddressIdentifier> {
        // Remove brackets if present
        let clean_identifier = identifier.trim_start_matches('[').trim_end_matches(']');
        
        let parts: Vec<&str> = clean_identifier.split(':').collect();
        
        match parts.len() {
            1 => {
                // Could be just an address type (p2tr) or a raw address
                if self.is_valid_address_type(parts[0]) {
                    let address_type = AddressType::from_str(parts[0])?;
                    Ok(AddressIdentifier::SelfWallet { address_type, index: 0 })
                } else {
                    // Assume it's a raw address
                    Ok(AddressIdentifier::Raw { address: parts[0].to_string() })
                }
            },
            2 => {
                if parts[0] == "self" {
                    // [self:p2tr] format
                    let address_type = AddressType::from_str(parts[1])?;
                    Ok(AddressIdentifier::SelfWallet { address_type, index: 0 })
                } else if parts[0] == "external" {
                    // [external:address] format
                    Ok(AddressIdentifier::External { address: parts[1].to_string() })
                } else if self.is_valid_address_type(parts[0]) {
                    // p2tr:0 format
                    let address_type = AddressType::from_str(parts[0])?;
                    let index = parts[1].parse::<u32>()
                        .map_err(|_| AlkanesError::Parse("Invalid address index".to_string()))?;
                    Ok(AddressIdentifier::SelfWallet { address_type, index })
                } else {
                    Err(AlkanesError::Parse(format!("Unknown identifier format: {identifier}")))
                }
            },
            3 => {
                if parts[0] == "self" && self.is_valid_address_type(parts[1]) {
                    // [self:p2tr:0] format
                    let address_type = AddressType::from_str(parts[1])?;
                    let index = parts[2].parse::<u32>()
                        .map_err(|_| AlkanesError::Parse("Invalid address index".to_string()))?;
                    Ok(AddressIdentifier::SelfWallet { address_type, index })
                } else {
                    Err(AlkanesError::Parse(format!("Unknown identifier format: {identifier}")))
                }
            },
            _ => Err(AlkanesError::Parse(format!("Invalid identifier format: {identifier}"))),
        }
    }
    
    /// Check if string is a valid address type
    fn is_valid_address_type(&self, s: &str) -> bool {
        matches!(s.to_lowercase().as_str(), "p2pk" | "p2tr" | "p2pkh" | "p2sh" | "p2wpkh" | "p2wsh" | "p2sh-p2wpkh" | "p2sh-p2wsh"
            | "subfrost-alkanes" | "subfrost-brc20")
    }
    
    /// Resolve a single identifier to an address
    pub async fn resolve_identifier(&mut self, identifier: &str) -> Result<String> {
        // Check cache first
        if let Some(cached) = self.cache.get(identifier) {
            return Ok(cached.clone());
        }

        let parsed = self.parse_identifier(identifier)?;

        let address = match parsed {
            AddressIdentifier::SelfWallet { ref address_type, index } => {
                let type_str = address_type.as_str().to_lowercase();
                // Handle subfrost dynamic signer addresses
                if type_str.starts_with("subfrost-") {
                    self.resolve_subfrost_signer(&type_str, index).await?
                } else {
                    crate::traits::AddressResolver::get_address(&self.provider, address_type.as_str(), index).await?
                }
            },
            AddressIdentifier::External { address } => address,
            AddressIdentifier::Raw { address } => {
                // We don't validate raw addresses here. Validation will happen
                // when the address is actually used to construct a script pubkey.
                address
            },
        };

        // Cache the result
        self.cache.insert(identifier.to_string(), address.clone());

        Ok(address)
    }

    /// Resolve a subfrost dynamic signer address by calling the contract's
    /// GetSignerAddress opcode (103) via simulate, then computing P2TR.
    ///
    /// Address types:
    /// - `subfrost-alkanes:N` → simulate [32:N] with input [103]
    /// - `subfrost-brc20:N` → simulate BRC20 prog frBTC at equivalent ID
    async fn resolve_subfrost_signer(&self, type_str: &str, index: u32) -> Result<String> {
        use crate::traits::AlkanesProvider;

        // Determine the contract to call based on the address type
        let contract_id = match type_str {
            "subfrost-alkanes" => format!("32:{}", index),
            "subfrost-brc20" => format!("32:{}", index), // Same contract, different protocol
            _ => return Err(AlkanesError::AddressResolution(
                format!("Unknown subfrost address type: {}", type_str)
            )),
        };

        log::info!("Resolving subfrost signer address for {} via simulate on {}", type_str, contract_id);

        // Build simulate context with opcode 103 (GetSignerAddress)
        let context = crate::proto::alkanes::MessageContextParcel {
            alkanes: vec![],
            transaction: vec![],
            block: vec![],
            height: 0,
            txindex: 0,
            calldata: {
                use alkanes_support::cellpack::Cellpack;
                use alkanes_support::id::AlkaneId;
                let parts: Vec<&str> = contract_id.split(':').collect();
                let block = parts[0].parse::<u128>().unwrap_or(32);
                let tx = parts[1].parse::<u128>().unwrap_or(0);
                Cellpack {
                    target: AlkaneId { block, tx },
                    inputs: vec![103], // GetSignerAddress opcode
                }.encipher()
            },
            vout: 0,
            pointer: 0,
            refund_pointer: 0,
        };

        let sim_result = self.provider.simulate(&contract_id, &context, None).await
            .map_err(|e| AlkanesError::AddressResolution(
                format!("Failed to simulate GetSignerAddress on {}: {}", contract_id, e)
            ))?;

        // The simulate returns a hex string (protobuf-encoded SimulateResponse).
        // We need to decode it to get the execution data.
        let result_hex = sim_result.as_str().unwrap_or("");
        let result_hex = result_hex.strip_prefix("0x").unwrap_or(result_hex);

        // Decode the protobuf SimulateResponse
        let result_bytes = hex::decode(result_hex)
            .map_err(|e| AlkanesError::AddressResolution(
                format!("Failed to decode simulate response: {}", e)
            ))?;

        // The SimulateResponse has: execution.data which contains the signer pubkey
        let sim_response = crate::proto::alkanes::SimulateResponse::decode(&*result_bytes)
            .map_err(|e| AlkanesError::AddressResolution(
                format!("Failed to decode SimulateResponse protobuf: {}", e)
            ))?;

        let data_hex = sim_response.execution
            .as_ref()
            .map(|e| hex::encode(&e.data))
            .unwrap_or_default();
        if data_hex.len() < 64 {
            return Err(AlkanesError::AddressResolution(
                format!("GetSignerAddress returned invalid data ({}): expected 32-byte pubkey", data_hex)
            ));
        }

        let pubkey_bytes = hex::decode(&data_hex[..64])
            .map_err(|e| AlkanesError::AddressResolution(
                format!("Failed to decode signer pubkey: {}", e)
            ))?;

        // Compute P2TR address from x-only pubkey
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let xonly = bitcoin::key::UntweakedPublicKey::from_slice(&pubkey_bytes)
            .map_err(|e| AlkanesError::AddressResolution(
                format!("Invalid x-only pubkey: {}", e)
            ))?;

        use bitcoin::key::TapTweak;
        let (tweaked, _) = xonly.tap_tweak(&secp, None);
        let script = bitcoin::ScriptBuf::new_p2tr_tweaked(tweaked);

        // Convert to address using network from provider
        use crate::traits::StorageProvider;
        let network = self.provider.get_network();
        let address = bitcoin::Address::from_script(&script, network)
            .map_err(|e| AlkanesError::AddressResolution(
                format!("Failed to derive address from signer script: {}", e)
            ))?;

        let addr_str = address.to_string();
        log::info!("Resolved {} → {}", type_str, addr_str);
        Ok(addr_str)
    }
    
    /// Parse address range specification (e.g., "p2tr:0-1000", "p2sh:0-500", "p2tr:50")
    pub fn parse_address_range(&self, range_spec: &str) -> Result<(String, u32, u32)> {
        let parts: Vec<&str> = range_spec.split(':').collect();
        if parts.len() != 2 {
            return Err(AlkanesError::Parse(format!("Invalid range specification. Expected format: address_type:start-end or address_type:index, got: {}", range_spec)));
        }
        
        let address_type = parts[0].to_string();
        let range_str = parts[1];

        if range_str.contains('-') {
            // Handle range format: start-end
            let range_parts: Vec<&str> = range_str.split('-').collect();
            if range_parts.len() != 2 {
                return Err(AlkanesError::Parse(format!("Invalid range format. Expected start-end, got: {}", range_str)));
            }
            
            let start_index = range_parts[0].parse::<u32>()
                .map_err(|_| AlkanesError::Parse(format!("Invalid start index: {}", range_parts[0])))?;
            let end_index = range_parts[1].parse::<u32>()
                .map_err(|_| AlkanesError::Parse(format!("Invalid end index: {}", range_parts[1])))?;
            
            if end_index <= start_index {
                return Err(AlkanesError::Parse(format!("End index must be greater than start index: {}-{}", start_index, end_index)));
            }
            
            let count = end_index - start_index;
            Ok((address_type, start_index, count))
        } else {
            // Handle single index format: just the index
            let index = range_str.parse::<u32>()
                .map_err(|_| AlkanesError::Parse(format!("Invalid index: {}", range_str)))?;
            Ok((address_type, index, 1))
        }
    }

    /// Resolve all identifiers in a string
    pub async fn resolve_all_identifiers(&mut self, input: &str) -> Result<String> {
        let identifiers = self.find_identifiers(input);
        
        if identifiers.is_empty() {
            // If there are no identifiers, we assume it's a raw address and
            // return it as is. Validation will happen downstream.
            return Ok(input.to_string());
        }
        
        let mut result = Vec::new();
        
        for identifier in identifiers {
            // Check if this is a range identifier
            let clean_id = identifier.trim_start_matches('[').trim_end_matches(']');
            
            if clean_id.contains(':') && self.is_shorthand_identifier(clean_id) {
                // Try to parse as a range
                match self.parse_address_range(clean_id) {
                    Ok((address_type, start_index, count)) => {
                        // Resolve all addresses in the range
                        for i in 0..count {
                            let index = start_index + i;
                            let at_lower = address_type.to_lowercase();
                            let address = if at_lower.starts_with("subfrost-") {
                                self.resolve_subfrost_signer(&at_lower, index).await?
                            } else {
                                crate::traits::AddressResolver::get_address(&self.provider, &address_type, index).await?
                            };
                            result.push(address);
                        }
                    }
                    Err(_) => {
                        // Fall back to single address resolution
                        let address = self.resolve_identifier(&identifier).await?;
                        result.push(address);
                    }
                }
            } else {
                let address = self.resolve_identifier(&identifier).await?;
                result.push(address);
            }
        }
        
        // If we resolved multiple addresses, return them comma-separated
        // If we resolved one address, return it as is
        // This maintains backwards compatibility
        if result.len() == 1 {
            Ok(result[0].clone())
        } else if result.is_empty() {
            Ok(input.to_string())
        } else {
            Ok(result.join(","))
        }
    }
    
    /// Get address for specific type and index
    pub async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        crate::traits::AddressResolver::get_address(&self.provider, address_type, index).await
    }
    
    /// List available address identifiers
    pub async fn list_identifiers(&self) -> Result<Vec<String>> {
        self.provider.list_identifiers().await
    }
    
    
    /// Clear the address cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
    
    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            (self.cache.len(), self.cache.capacity())
        }
        #[cfg(target_arch = "wasm32")]
        {
            // BTreeMap doesn't have capacity, so just return length twice
            (self.cache.len(), self.cache.len())
        }
    }
}

/// Standalone address resolver for environments without full provider
#[cfg(not(target_arch = "wasm32"))]
pub struct StandaloneAddressResolver {
    addresses: HashMap<String, String>,
    network: Network,
}

#[cfg(not(target_arch = "wasm32"))]
impl StandaloneAddressResolver {
    /// Create a new standalone address resolver
    pub fn new(network: Network) -> Self {
        Self {
            addresses: HashMap::new(),
            network,
        }
    }
    
    /// Add an address mapping
    pub fn add_address(&mut self, identifier: &str, address: &str) {
        self.addresses.insert(identifier.to_string(), address.to_string());
    }
    
    /// Resolve identifier using local mappings
    pub fn resolve(&self, identifier: &str) -> Result<String> {
        self.addresses.get(identifier)
            .cloned()
            .ok_or_else(|| AlkanesError::AddressResolution(
                format!("Unknown address identifier: {identifier}")
            ))
    }
    
    /// Check if identifier exists
    pub fn contains(&self, identifier: &str) -> bool {
        self.addresses.contains_key(identifier)
    }
}

/// Utility functions for address operations
pub mod utils {
    use super::*;
    
    /// Extract address from script
    pub fn extract_address_from_script(script: &bitcoin::ScriptBuf, network: Network) -> Option<String> {
        bitcoin::Address::from_script(script, network)
            .ok()
            .map(|addr| addr.to_string())
    }
    
    /// Get script type description
    pub fn get_script_type_description(script: &bitcoin::ScriptBuf) -> String {
        if script.is_p2pkh() {
            "P2PKH (Legacy)".to_string()
        } else if script.is_p2sh() {
            "P2SH (Script Hash)".to_string()
        } else if script.is_p2tr() {
            "P2TR (Taproot)".to_string()
        } else if script.is_witness_program() {
            "Witness Program (SegWit)".to_string()
        } else {
            "Unknown".to_string()
        }
    }
    
    /// Check if address is a raw Bitcoin address (not an identifier)
    pub fn is_raw_bitcoin_address(addr: &str) -> bool {
        !addr.contains('[') && !addr.contains(':') && (
            addr.starts_with('1') || 
            addr.starts_with('3') || 
            addr.starts_with("bc1") || 
            addr.starts_with("tb1") || 
            addr.starts_with("bcrt1")
        )
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_shorthand_identifier() {
        let resolver = StandaloneAddressResolver::new(Network::Regtest);
        let resolver = AddressResolver::new(resolver);
        
        assert!(resolver.is_shorthand_identifier("p2tr:0"));
        assert!(resolver.is_shorthand_identifier("p2wpkh:5"));
        assert!(resolver.is_shorthand_identifier("p2tr"));
        assert!(!resolver.is_shorthand_identifier("invalid:0"));
        assert!(!resolver.is_shorthand_identifier("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"));
    }
    
    #[test]
    fn test_find_identifiers() {
        let resolver = StandaloneAddressResolver::new(Network::Regtest);
        let resolver = AddressResolver::new(resolver);
        
        let identifiers = resolver.find_identifiers("Send to [self:p2tr:0] and [external:bc1q...]");
        assert_eq!(identifiers.len(), 2);
        assert!(identifiers.contains(&"[self:p2tr:0]".to_string()));
        assert!(identifiers.contains(&"[external:bc1q...]".to_string()));
        
        let identifiers = resolver.find_identifiers("p2tr:0");
        assert_eq!(identifiers.len(), 1);
        assert!(identifiers.contains(&"p2tr:0".to_string()));
    }
    
    #[test]
    fn test_utils() {
        assert!(utils::is_raw_bitcoin_address("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"));
        assert!(utils::is_raw_bitcoin_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"));
        assert!(!utils::is_raw_bitcoin_address("p2tr:0"));
        assert!(!utils::is_raw_bitcoin_address("[self:p2tr:0]"));
    }
    
    #[test]
    fn test_standalone_resolver() {
        let mut resolver = StandaloneAddressResolver::new(Network::Regtest);
        resolver.add_address("p2tr:0", "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kyxztk9");
        
        assert!(resolver.contains("p2tr:0"));
        assert_eq!(resolver.resolve("p2tr:0").unwrap(), "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kyxztk9");
        assert!(resolver.resolve("unknown").is_err());
    }
}

// Trait implementations for StandaloneAddressResolver (only when not web-compat)
#[cfg(not(target_arch = "wasm32"))]
mod standalone_impls {
    use super::*;
    use async_trait::async_trait;
    use crate::traits::{
        JsonRpcProvider, StorageProvider, CryptoProvider, TimeProvider, LogProvider,
        WalletProvider, BitcoinRpcProvider, MetashrewRpcProvider, MetashrewProvider,
        EsploraProvider, EspoProvider, RunestoneProvider, AlkanesProvider, MonitorProvider,
        KeystoreProvider, OrdProvider, SendParams, UtxoInfo, TransactionInfo,
        FeeEstimate, FeeRates, WalletBalance, WalletConfig, WalletInfo, AddressInfo,
        BlockEvent, KeystoreAddress, KeystoreInfo,
    };
    use crate::network::NetworkParams;
    use crate::ord::{
        AddressInfo as OrdAddressInfo, Block as OrdBlock, Blocks as OrdBlocks,
        Children as OrdChildren, Inscription as OrdInscription, Inscriptions as OrdInscriptions,
        Output as OrdOutput, ParentInscriptions as OrdParents, SatResponse as OrdSat,
        RuneInfo as OrdRuneInfo, Runes as OrdRunes, TxInfo as OrdTxInfo,
    };

    #[async_trait(?Send)]
    impl JsonRpcProvider for StandaloneAddressResolver {
    async fn call(&self, _url: &str, _method: &str, _params: serde_json::Value, _id: u64) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support RPC calls".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl StorageProvider for StandaloneAddressResolver {
    async fn read(&self, _key: &str) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support storage".to_string()))
    }
    async fn write(&self, _key: &str, _data: &[u8]) -> Result<()> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support storage".to_string()))
    }
    async fn exists(&self, _key: &str) -> Result<bool> { Ok(false) }
    async fn delete(&self, _key: &str) -> Result<()> { Ok(()) }
    async fn list_keys(&self, _prefix: &str) -> Result<Vec<String>> { Ok(vec![]) }
    fn storage_type(&self) -> &'static str { "none" }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl NetworkProvider for StandaloneAddressResolver {
    async fn get(&self, _url: &str) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support network operations".to_string()))
    }
    async fn post(&self, _url: &str, _body: &[u8], _content_type: &str) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support network operations".to_string()))
    }
    async fn is_reachable(&self, _url: &str) -> bool { false }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl CryptoProvider for StandaloneAddressResolver {
    fn random_bytes(&self, _len: usize) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support crypto operations".to_string()))
    }
    fn sha256(&self, _data: &[u8]) -> Result<[u8; 32]> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support crypto operations".to_string()))
    }
    fn sha3_256(&self, _data: &[u8]) -> Result<[u8; 32]> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support crypto operations".to_string()))
    }
    async fn encrypt_aes_gcm(&self, _data: &[u8], _key: &[u8], _nonce: &[u8]) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support crypto operations".to_string()))
    }
    async fn decrypt_aes_gcm(&self, _data: &[u8], _key: &[u8], _nonce: &[u8]) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support crypto operations".to_string()))
    }
    async fn pbkdf2_derive(&self, _password: &[u8], _salt: &[u8], _iterations: u32, _key_len: usize) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support crypto operations".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl TimeProvider for StandaloneAddressResolver {
    fn now_secs(&self) -> u64 { 0 }
    fn now_millis(&self) -> u64 { 0 }
    #[cfg(feature = "native-deps")]
    async fn sleep_ms(&self, ms: u64) {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await
    }

    #[cfg(not(feature = "native-deps"))]
    async fn sleep_ms(&self, ms: u64) {
        #[cfg(target_arch = "wasm32")]
        {
            gloo_timers::future::sleep(std::time::Duration::from_millis(ms)).await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ms;
            unimplemented!("sleep_ms is not implemented for non-wasm targets without native-deps feature")
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl LogProvider for StandaloneAddressResolver {
    fn debug(&self, _message: &str) {}
    fn info(&self, _message: &str) {}
    fn warn(&self, _message: &str) {}
    fn error(&self, _message: &str) {}
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl WalletProvider for StandaloneAddressResolver {
    async fn create_wallet(&mut self, _config: WalletConfig, _mnemonic: Option<String>, _passphrase: Option<String>) -> Result<WalletInfo> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn load_wallet(&mut self, _config: WalletConfig, _passphrase: Option<String>) -> Result<WalletInfo> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn get_balance(&self, _addresses: Option<Vec<String>>) -> Result<WalletBalance> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn get_address(&self) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn get_addresses(&self, _count: u32) -> Result<Vec<AddressInfo>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn send(&mut self, _params: SendParams) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn get_utxos(&self, _include_frozen: bool, _addresses: Option<Vec<String>>) -> Result<Vec<(bitcoin::OutPoint, UtxoInfo)>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn get_history(&self, _count: u32, _address: Option<String>) -> Result<Vec<TransactionInfo>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn freeze_utxo(&self, _utxo: String, _reason: Option<String>) -> Result<()> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn unfreeze_utxo(&self, _utxo: String) -> Result<()> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn create_transaction(&self, _params: SendParams) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn sign_transaction(&mut self, _tx_hex: String) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn broadcast_transaction(&self, _tx_hex: String) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn estimate_fee(&self, _target: u32) -> Result<FeeEstimate> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn get_fee_rates(&self) -> Result<FeeRates> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn sync(&self) -> Result<()> { Ok(()) }
    async fn backup(&self) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    async fn get_mnemonic(&self) -> Result<Option<String>> { Ok(None) }
    fn get_network(&self) -> Network { self.network }
    
    async fn get_internal_key(&self) -> Result<(bitcoin::XOnlyPublicKey, (bitcoin::bip32::Fingerprint, bitcoin::bip32::DerivationPath))> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }

    async fn get_internal_key_with_secret(&self) -> Result<(bitcoin::XOnlyPublicKey, bitcoin::secp256k1::SecretKey, (bitcoin::bip32::Fingerprint, bitcoin::bip32::DerivationPath))> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }

    async fn sign_psbt(&mut self, _psbt: &bitcoin::psbt::Psbt) -> Result<bitcoin::psbt::Psbt> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    
    async fn get_keypair(&self) -> Result<bitcoin::secp256k1::Keypair> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wallet operations".to_string()))
    }
    fn set_passphrase(&mut self, _passphrase: Option<String>) {
        // No-op for StandaloneAddressResolver
    }
    async fn get_last_used_address_index(&self) -> Result<u32> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support get_last_used_address_index".to_string()))
    }

    async fn get_enriched_utxos(&self, _addresses: Option<Vec<String>>) -> Result<Vec<crate::provider::EnrichedUtxo>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support get_enriched_utxos".to_string()))
    }

    async fn get_all_balances(&self, _addresses: Option<Vec<String>>) -> Result<crate::provider::AllBalances> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support get_all_balances".to_string()))
    }

    async fn get_master_public_key(&self) -> Result<Option<String>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support get_master_public_key".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl crate::traits::AddressResolver for StandaloneAddressResolver {
    async fn resolve_all_identifiers(&self, input: &str) -> Result<String> {
        Ok(input.to_string()) // No-op for standalone
    }
    fn contains_identifiers(&self, _input: &str) -> bool { false }
    async fn get_address(&self, _address_type: &str, _index: u32) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support address generation".to_string()))
    }
    async fn list_identifiers(&self) -> Result<Vec<String>> {
        Ok(self.addresses.keys().cloned().collect())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl BitcoinRpcProvider for StandaloneAddressResolver {
    async fn get_block_count(&self) -> Result<u64> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn generate_to_address(&self, _nblocks: u32, _address: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn generate_future(&self, _address: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn subfrost_thieve(&self, _address: &str, _amount: u64) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn get_blockchain_info(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn get_transaction_hex(&self, _txid: &str) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn get_block(&self, _hash: &str, _raw: bool) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn get_block_hash(&self, _height: u64) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn send_raw_transaction(&self, _tx_hex: &str) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn send_raw_transactions(&self, _tx_hexes: &[String]) -> Result<Vec<String>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn get_mempool_info(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    async fn estimate_smart_fee(&self, _target: u32) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
    
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64> {
        Err(AlkanesError::NotImplemented("get_esplora_blocks_tip_height not implemented for StandaloneAddressResolver".to_string()))
    }
    
    async fn trace_transaction(&self, _txid: &str, _vout: u32, _block: Option<&str>, _tx: Option<&str>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("trace_transaction not implemented for StandaloneAddressResolver".to_string()))
    }
    async fn get_new_address(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }

    async fn get_network_info(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }

    async fn get_raw_transaction(&self, _txid: &str, _block_hash: Option<&str>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }

    async fn get_block_header(&self, _hash: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }

    async fn get_block_stats(&self, _hash: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }

    async fn get_chain_tips(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }

    async fn get_raw_mempool(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }

    async fn get_tx_out(&self, _txid: &str, _vout: u32, _include_mempool: bool) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }

    async fn decode_raw_transaction(&self, _hex: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }

    async fn decode_psbt(&self, _psbt: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Bitcoin RPC".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl MetashrewRpcProvider for StandaloneAddressResolver {
    async fn get_metashrew_height(&self) -> Result<u64> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Metashrew RPC".to_string()))
    }
    async fn get_state_root(&self, _height: serde_json::Value) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Metashrew RPC".to_string()))
    }
    async fn get_contract_meta(&self, _block: &str, _tx: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Metashrew RPC".to_string()))
    }
    async fn trace_outpoint(&self, _txid: &str, _vout: u32) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Metashrew RPC".to_string()))
    }
    async fn get_spendables_by_address(&self, _address: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Metashrew RPC".to_string()))
    }
    async fn get_protorunes_by_address(
        &self,
        _address: &str,
        _block_tag: Option<String>,
        _protocol_tag: u128,
    ) -> Result<crate::alkanes::protorunes::ProtoruneWalletResponse> {
        Err(AlkanesError::NotImplemented(
            "StandaloneAddressResolver does not support Metashrew RPC".to_string(),
        ))
    }
    async fn get_protorunes_by_outpoint(
        &self,
        _txid: &str,
        _vout: u32,
        _block_tag: Option<String>,
        _protocol_tag: u128,
    ) -> Result<crate::alkanes::protorunes::ProtoruneOutpointResponse> {
        Err(AlkanesError::NotImplemented(
            "StandaloneAddressResolver does not support Metashrew RPC".to_string(),
        ))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl EsploraProvider for StandaloneAddressResolver {
    async fn get_blocks_tip_hash(&self) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_blocks_tip_height(&self) -> Result<u64> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_blocks(&self, _start_height: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_block_by_height(&self, _height: u64) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_block(&self, _hash: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_block_status(&self, _hash: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_block_txids(&self, _hash: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_block_header(&self, _hash: &str) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_block_raw(&self, _hash: &str) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_block_txid(&self, _hash: &str, _index: u32) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_block_txs(&self, _hash: &str, _start_index: Option<u32>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_address_info(&self, _address: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_address_utxo(&self, _address: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_address_txs(&self, _address: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_address_txs_chain(&self, _address: &str, _last_seen_txid: Option<&str>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_address_txs_mempool(&self, _address: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_address_prefix(&self, _prefix: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_tx(&self, _txid: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_tx_hex(&self, _txid: &str) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_tx_raw(&self, _txid: &str) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_tx_status(&self, _txid: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_tx_merkle_proof(&self, _txid: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_tx_merkleblock_proof(&self, _txid: &str) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_tx_outspend(&self, _txid: &str, _index: u32) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_tx_outspends(&self, _txid: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn broadcast(&self, _tx_hex: &str) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_mempool(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_mempool_txids(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_mempool_recent(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
    async fn get_fee_estimates(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Esplora API".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl EspoProvider for StandaloneAddressResolver {
    async fn get_espo_height(&self) -> Result<u64> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_address_balances(&self, _address: &str, _include_outpoints: bool) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_address_outpoints(&self, _address: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_outpoint_balances(&self, _outpoint: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_holders(&self, _alkane_id: &str, _page: u64, _limit: u64) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_holders_count(&self, _alkane_id: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_keys(&self, _alkane_id: &str, _page: u64, _limit: u64) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn ping(&self) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn ammdata_ping(&self) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_candles(
        &self,
        _pool: &str,
        _timeframe: Option<&str>,
        _side: Option<&str>,
        _limit: Option<u64>,
        _page: Option<u64>,
    ) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_trades(
        &self,
        _pool: &str,
        _limit: Option<u64>,
        _page: Option<u64>,
        _side: Option<&str>,
        _filter_side: Option<&str>,
        _sort: Option<&str>,
        _dir: Option<&str>,
    ) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_pools(
        &self,
        _limit: Option<u64>,
        _page: Option<u64>,
    ) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn find_best_swap_path(
        &self,
        _token_in: &str,
        _token_out: &str,
        _mode: Option<&str>,
        _amount_in: Option<&str>,
        _amount_out: Option<&str>,
        _amount_out_min: Option<&str>,
        _amount_in_max: Option<&str>,
        _available_in: Option<&str>,
        _fee_bps: Option<u64>,
        _max_hops: Option<u64>,
    ) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_best_mev_swap(
        &self,
        _token: &str,
        _fee_bps: Option<u64>,
        _max_hops: Option<u64>,
    ) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_amm_factories(&self, _page: Option<u64>, _limit: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_all_alkanes(&self, _page: Option<u64>, _limit: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_info(&self, _alkane_id: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_block_summary(&self, _height: u64) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_circulating_supply(&self, _alkane_id: &str, _height: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_transfer_volume(&self, _alkane_id: &str, _page: Option<u64>, _limit: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_total_received(&self, _alkane_id: &str, _page: Option<u64>, _limit: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_address_activity(&self, _address: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_balances(&self, _alkane_id: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_balance_metashrew(&self, _owner: &str, _target: &str, _height: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_balance_txs(&self, _alkane_id: &str, _page: Option<u64>, _limit: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_balance_txs_by_token(&self, _owner: &str, _token: &str, _page: Option<u64>, _limit: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_block_traces(&self, _height: u64) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_tx_summary(&self, _txid: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_block_txs(&self, _height: u64, _page: Option<u64>, _limit: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_address_txs(&self, _address: &str, _page: Option<u64>, _limit: Option<u64>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_address_transactions(&self, _address: &str, _page: Option<u64>, _limit: Option<u64>, _only_alkane_txs: Option<bool>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_latest_traces(&self) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_mempool_traces(&self, _page: Option<u64>, _limit: Option<u64>, _address: Option<&str>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_wrap_events_all(&self, _count: Option<u64>, _offset: Option<u64>, _successful: Option<bool>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_wrap_events_by_address(&self, _address: &str, _count: Option<u64>, _offset: Option<u64>, _successful: Option<bool>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_unwrap_events_all(&self, _count: Option<u64>, _offset: Option<u64>, _successful: Option<bool>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_unwrap_events_by_address(&self, _address: &str, _count: Option<u64>, _offset: Option<u64>, _successful: Option<bool>) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_series_id_from_alkane_id(&self, _alkane_id: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_series_ids_from_alkane_ids(&self, _alkane_ids: &[&str]) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_id_from_series_id(&self, _series_id: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
    async fn get_alkane_ids_from_series_ids(&self, _series_ids: &[&str]) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Espo API".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl RunestoneProvider for StandaloneAddressResolver {
    async fn decode_runestone(&self, _tx: &bitcoin::Transaction) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support runestone operations".to_string()))
    }
    async fn format_runestone_with_decoded_messages(&self, _tx: &bitcoin::Transaction) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support runestone operations".to_string()))
    }
    async fn analyze_runestone(&self, _txid: &str) -> Result<serde_json::Value> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support runestone operations".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl OrdProvider for StandaloneAddressResolver {
    async fn get_inscription(&self, _inscription_id: &str) -> Result<OrdInscription> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }

    async fn get_inscriptions_in_block(&self, _block_hash: &str) -> Result<OrdInscriptions> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_ord_address_info(&self, _address: &str) -> Result<OrdAddressInfo> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_block_info(&self, _query: &str) -> Result<OrdBlock> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_ord_block_count(&self) -> Result<u64> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_ord_blocks(&self) -> Result<OrdBlocks> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_children(&self, _inscription_id: &str, _page: Option<u32>) -> Result<OrdChildren> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_content(&self, _inscription_id: &str) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_inscriptions(&self, _page: Option<u32>) -> Result<OrdInscriptions> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_output(&self, _output: &str) -> Result<OrdOutput> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_parents(&self, _inscription_id: &str, _page: Option<u32>) -> Result<OrdParents> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_rune(&self, _rune: &str) -> Result<OrdRuneInfo> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_runes(&self, _page: Option<u32>) -> Result<OrdRunes> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_sat(&self, _sat: u64) -> Result<OrdSat> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
    async fn get_tx_info(&self, _txid: &str) -> Result<OrdTxInfo> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support ord operations".to_string()))
    }
}


#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl MonitorProvider for StandaloneAddressResolver {
    async fn monitor_blocks(&self, _start: Option<u64>) -> Result<()> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support monitoring".to_string()))
    }
    async fn get_block_events(&self, _height: u64) -> Result<Vec<BlockEvent>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support monitoring".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl KeystoreProvider for StandaloneAddressResolver {
    async fn get_address(&self, _address_type: &str, _index: u32) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support keystore operations".to_string()))
    }
    async fn derive_addresses(&self, _master_public_key: &str, _network_params: &NetworkParams, _script_types: &[&str], _start_index: u32, _count: u32) -> Result<Vec<KeystoreAddress>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support keystore operations".to_string()))
    }
    
    async fn get_default_addresses(&self, _master_public_key: &str, _network_params: &NetworkParams) -> Result<Vec<KeystoreAddress>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support keystore operations".to_string()))
    }
    
    fn parse_address_range(&self, _range_spec: &str) -> Result<(String, u32, u32)> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support keystore operations".to_string()))
    }
    
    async fn get_keystore_info(&self, _master_fingerprint: &str, _created_at: u64, _version: &str) -> Result<KeystoreInfo> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support keystore operations".to_string()))
    }

    async fn derive_address_from_path(&self, _master_public_key: &str, _path: &bitcoin::bip32::DerivationPath, _script_type: &str, _network_params: &NetworkParams) -> Result<KeystoreAddress> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support keystore operations".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl AlkanesProvider for StandaloneAddressResolver {
    async fn execute(
        &mut self,
        _params: crate::alkanes::types::EnhancedExecuteParams,
    ) -> Result<crate::alkanes::types::ExecutionState> {
        Err(AlkanesError::NotImplemented(
            "StandaloneAddressResolver does not support alkanes operations".to_string(),
        ))
    }

    async fn execute_full(
        &mut self,
        _params: crate::alkanes::types::EnhancedExecuteParams,
    ) -> Result<crate::alkanes::types::EnhancedExecuteResult> {
        Err(AlkanesError::NotImplemented(
            "StandaloneAddressResolver does not support alkanes operations".to_string(),
        ))
    }

    async fn resume_execution(
        &mut self,
        _state: crate::alkanes::types::ReadyToSignTx,
        _params: &crate::alkanes::types::EnhancedExecuteParams,
    ) -> Result<crate::alkanes::types::EnhancedExecuteResult> {
        Err(AlkanesError::NotImplemented(
            "StandaloneAddressResolver does not support alkanes operations".to_string(),
        ))
    }

    async fn resume_commit_execution(
        &mut self,
        _state: crate::alkanes::types::ReadyToSignCommitTx,
    ) -> Result<crate::alkanes::types::ExecutionState> {
        Err(AlkanesError::NotImplemented(
            "StandaloneAddressResolver does not support alkanes operations".to_string(),
        ))
    }

    async fn resume_reveal_execution(
        &mut self,
        _state: crate::alkanes::types::ReadyToSignRevealTx,
    ) -> Result<crate::alkanes::types::EnhancedExecuteResult> {
        Err(AlkanesError::NotImplemented(
            "StandaloneAddressResolver does not support alkanes operations".to_string(),
        ))
    }

    async fn protorunes_by_address(
        &self,
        _address: &str,
        _block_tag: Option<String>,
        _protocol_tag: u128,
    ) -> Result<crate::alkanes::protorunes::ProtoruneWalletResponse> {
        Err(AlkanesError::NotImplemented(
            "StandaloneAddressResolver does not support alkanes operations".to_string(),
        ))
    }

    async fn protorunes_by_outpoint(
        &self,
        _txid: &str,
        _vout: u32,
        _block_tag: Option<String>,
        _protocol_tag: u128,
    ) -> Result<crate::alkanes::protorunes::ProtoruneOutpointResponse> {
        Err(AlkanesError::NotImplemented(
            "StandaloneAddressResolver does not support alkanes operations".to_string(),
        ))
    }

    async fn simulate(&self, _contract_id: &str, _context: &crate::proto::alkanes::MessageContextParcel, _block_tag: Option<String>) -> Result<crate::JsonValue> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn view(&self, _contract_id: &str, _view_fn: &str, _params: Option<&[u8]>, _block_tag: Option<String>) -> Result<crate::JsonValue> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn tx_script(
        &self,
        _wasm_bytes: &[u8],
        _inputs: Vec<u128>,
        _block_tag: Option<String>,
    ) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn trace(&self, _outpoint: &str) -> Result<crate::proto::alkanes::Trace> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn trace_protostones(&self, _txid: &str) -> Result<Option<Vec<crate::JsonValue>>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn get_block(&self, _height: u64) -> Result<crate::proto::alkanes::BlockResponse> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn sequence(&self, _block_tag: Option<String>) -> Result<crate::JsonValue> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn spendables_by_address(&self, _address: &str) -> Result<crate::JsonValue> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn trace_block(&self, _height: u64) -> Result<crate::proto::alkanes::AlkanesBlockTraceEvent> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn get_bytecode(&self, _alkane_id: &str, _block_tag: Option<String>) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn meta(&self, _alkane_id: &str, _block_tag: Option<String>) -> Result<Vec<u8>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn inspect(&self, _target: &str, _config: crate::alkanes::AlkanesInspectConfig) -> Result<crate::alkanes::AlkanesInspectResult> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }

    async fn get_balance(&self, _address: Option<&str>) -> Result<Vec<crate::alkanes::AlkaneBalance>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }
    async fn pending_unwraps(&self, _block_tag: Option<String>) -> Result<Vec<crate::alkanes::PendingUnwrap>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support alkanes operations".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Clone for StandaloneAddressResolver {
    fn clone(&self) -> Self {
        Self {
            addresses: self.addresses.clone(),
            network: self.network,
        }
    }
}

// Stub implementation of LuaScriptExecutor for StandaloneAddressResolver
#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl crate::lua_script::LuaScriptExecutor for StandaloneAddressResolver {
    async fn execute_lua_script(
        &self,
        _script: &crate::lua_script::LuaScript,
        _args: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        Err(AlkanesError::Other("LuaScriptExecutor not available in StandaloneAddressResolver".to_string()))
    }

    async fn lua_evalsaved(
        &self,
        _script_hash: &str,
        _args: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        Err(AlkanesError::Other("LuaScriptExecutor not available in StandaloneAddressResolver".to_string()))
    }

    async fn lua_evalscript(
        &self,
        _script_content: &str,
        _args: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        Err(AlkanesError::Other("LuaScriptExecutor not available in StandaloneAddressResolver".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl DeezelProvider for StandaloneAddressResolver {
    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        None
    }
    fn get_esplora_api_url(&self) -> Option<String> {
        None
    }
    fn get_ord_server_url(&self) -> Option<String> {
        None
    }
    fn get_metashrew_rpc_url(&self) -> Option<String> {
        None
    }
    fn get_brc20_prog_rpc_url(&self) -> Option<String> {
        None
    }
    fn provider_name(&self) -> &str {
        "StandaloneAddressResolver"
    }
    async fn initialize(&self) -> Result<()> { Ok(()) }
    async fn shutdown(&self) -> Result<()> { Ok(()) }
    fn clone_box(&self) -> Box<dyn DeezelProvider> {
        Box::new(self.clone())
    }
    fn secp(&self) -> &bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All> {
        unimplemented!()
    }
    async fn get_utxo(&self, _outpoint: &bitcoin::OutPoint) -> Result<Option<bitcoin::TxOut>> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support get_utxo".to_string()))
    }
    async fn sign_taproot_script_spend(&self, _sighash: bitcoin::secp256k1::Message, _ephemeral_secret: Option<bitcoin::secp256k1::SecretKey>) -> Result<bitcoin::secp256k1::schnorr::Signature> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support sign_taproot_script_spend".to_string()))
    }

    async fn wrap(&mut self, _amount: u64, _address: Option<String>, _fee_rate: Option<f32>) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support wrap".to_string()))
    }

    async fn unwrap(&mut self, _amount: u64, _address: Option<String>) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support unwrap".to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl MetashrewProvider for StandaloneAddressResolver {
    async fn get_height(&self) -> Result<u64> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Metashrew operations".to_string()))
    }
    async fn get_block_hash(&self, _height: u64) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Metashrew operations".to_string()))
    }
    async fn get_state_root(&self, _height: serde_json::Value) -> Result<String> {
        Err(AlkanesError::NotImplemented("StandaloneAddressResolver does not support Metashrew operations".to_string()))
    }
}
}
