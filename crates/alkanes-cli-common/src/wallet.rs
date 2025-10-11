use crate::{Result, AlkanesError};
use alloc::{string::ToString, format};
use crate::traits::{AlkanesProvider, WalletProvider};
use crate::types::{WalletBalance, EnrichedUtxo, WalletConfig, SendParams, FeeEstimate, FeeRates, AddressInfo, TransactionInfo, TransactionInput, TransactionOutput, UtxoInfo};
use crate::network::NetworkParams;
use bitcoin::Network;
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use std::{vec, vec::Vec, string::String};
#[cfg(target_arch = "wasm32")]
use alloc::{vec, vec::Vec, string::String};

fn network_to_params(network: bitcoin::Network) -> NetworkParams {
    match network {
        bitcoin::Network::Bitcoin => NetworkParams {
            network: bitcoin::Network::Bitcoin,
            magic: [0xf9, 0xbe, 0xb4, 0xd9],
            default_port: 8333,
            rpc_port: 8332,
            bech32_hrp: "bc".to_string(),
            bech32_prefix: "bc".to_string(),
            p2pkh_prefix: 0,
            p2sh_prefix: 5,
        },
        bitcoin::Network::Testnet => NetworkParams {
            network: bitcoin::Network::Testnet,
            magic: [0x0b, 0x11, 0x09, 0x07],
            default_port: 18333,
            rpc_port: 18332,
            bech32_hrp: "tb".to_string(),
            bech32_prefix: "tb".to_string(),
            p2pkh_prefix: 111,
            p2sh_prefix: 196,
        },
        bitcoin::Network::Signet => NetworkParams {
            network: bitcoin::Network::Signet,
            magic: [0x0a, 0x03, 0xcf, 0x40],
            default_port: 38333,
            rpc_port: 38332,
            bech32_hrp: "tb".to_string(),
            bech32_prefix: "tb".to_string(),
            p2pkh_prefix: 111,
            p2sh_prefix: 196,
        },
        bitcoin::Network::Regtest => NetworkParams {
            network: bitcoin::Network::Regtest,
            magic: [0xfa, 0xbf, 0xb5, 0xda],
            default_port: 18444,
            rpc_port: 18443,
            bech32_hrp: "bcrt".to_string(),
            bech32_prefix: "bcrt".to_string(),
            p2pkh_prefix: 111,
            p2sh_prefix: 196,
        },
        _ => NetworkParams {
            network: bitcoin::Network::Bitcoin,
            magic: [0xf9, 0xbe, 0xb4, 0xd9],
            default_port: 8333,
            rpc_port: 8332,
            bech32_hrp: "bc".to_string(),
            bech32_prefix: "bc".to_string(),
            p2pkh_prefix: 0,
            p2sh_prefix: 5,
        },
    }
}

/// Wallet manager that works with any provider
pub struct Wallet<P: AlkanesProvider> {
    provider: P,
    _config: WalletConfig,
}

impl<P: AlkanesProvider> Wallet<P> {
    /// Create a new wallet manager
    pub fn new(provider: P, config: WalletConfig) -> Self {
        Self { provider, _config: config }
    }
    
    pub async fn create(
        mut provider: P,
        config: WalletConfig,
        mnemonic: Option<String>,
        passphrase: Option<String>,
    ) -> Result<Self> {
        let params = network_to_params(config.network);
        crate::network::set_network(params.clone());
        let trait_config = crate::types::WalletConfig {
            wallet_path: config.wallet_path.clone(),
            bitcoin_rpc_url: config.bitcoin_rpc_url.clone(),
            metashrew_rpc_url: config.metashrew_rpc_url.clone(),
            network: config.network,
            network_params: Some(params),
        };
        let wallet_info = provider.create_wallet(trait_config, mnemonic, passphrase).await?;
        provider.info(&format!("Created wallet with address: {}", wallet_info.address));
        
        Ok(Self { provider, _config: config })
    }
    
    /// Load an existing wallet
    pub async fn load(mut provider: P, config: WalletConfig, passphrase: Option<String>) -> Result<Self> {
        let params = network_to_params(config.network);
        crate::network::set_network(params.clone());
        let trait_config = crate::types::WalletConfig {
            wallet_path: config.wallet_path.clone(),
            bitcoin_rpc_url: config.bitcoin_rpc_url.clone(),
            metashrew_rpc_url: config.metashrew_rpc_url.clone(),
            network: config.network,
            network_params: Some(params),
        };
        let _wallet_info = provider.load_wallet(trait_config, passphrase).await?;
        Ok(Self { provider, _config: config })
    }
    /// Load wallet with passphrase
    pub async fn load_with_passphrase(
        provider: P,
        config: WalletConfig,
        passphrase: &str,
    ) -> Result<Self> {
        Self::load(provider, config, Some(passphrase.to_string())).await
    }
    
    /// Get wallet balance
    pub async fn get_balance(&self) -> Result<WalletBalance> {
        self.provider.get_balance(None).await
    }
    
    /// Get wallet address
    pub async fn get_address(&self) -> Result<String> {
        self.provider.get_address().await
    }
    
    /// Get multiple addresses
    pub async fn get_addresses(&self, count: u32) -> Result<Vec<AddressInfo>> {
        self.provider.get_addresses(count).await
    }
    
    /// Send Bitcoin transaction
    pub async fn send(&mut self, params: SendParams) -> Result<String> {
        self.provider.send(params).await
    }
    
    /// Get UTXOs
    pub async fn get_utxos(&self) -> Result<Vec<UtxoInfo>> {
        self.provider.get_utxos(false, None).await.map(|utxos| utxos.into_iter().map(|(_, utxo)| utxo).collect())
    }
    
    /// Get enriched UTXOs (with additional metadata)
    pub async fn get_enriched_utxos(&self) -> Result<Vec<EnrichedUtxo>> {
        self.provider.get_enriched_utxos(None).await
    }
    
    /// Get UTXOs for a specific address
    pub async fn get_enriched_utxos_for_address(&self, address: &str) -> Result<Vec<EnrichedUtxo>> {
        self.provider.get_enriched_utxos(Some(vec![address.to_string()])).await
    }
    
    /// Get transaction history
    pub async fn get_history(&self, count: u32, address: Option<String>) -> Result<Vec<TransactionInfo>> {
        self.provider.get_history(count, address).await
    }
    
    /// Freeze UTXO
    pub async fn freeze_utxo(&self, utxo: String, reason: Option<String>) -> Result<()> {
        self.provider.freeze_utxo(utxo, reason).await
    }
    
    /// Unfreeze UTXO
    pub async fn unfreeze_utxo(&self, utxo: String) -> Result<()> {
        self.provider.unfreeze_utxo(utxo).await
    }
    
    /// Create transaction without broadcasting
    pub async fn create_transaction(&self, params: SendParams) -> Result<String> {
        self.provider.create_transaction(params).await
    }
    
    /// Sign transaction
    pub async fn sign_transaction(&mut self, tx_hex: String) -> Result<String> {
        self.provider.sign_transaction(tx_hex).await
    }
    
    /// Broadcast transaction
    pub async fn broadcast_transaction(&self, tx_hex: String) -> Result<String> {
        self.provider.broadcast_transaction(tx_hex).await
    }
    
    /// Estimate fee
    pub async fn estimate_fee(&self, target: u32) -> Result<FeeEstimate> {
        self.provider.estimate_fee(target).await
    }
    
    /// Get current fee rates
    pub async fn get_fee_rates(&self) -> Result<FeeRates> {
        self.provider.get_fee_rates().await
    }
    
    /// Synchronize wallet
    pub async fn sync(&self) -> Result<()> {
        self.provider.sync().await
    }
    
    /// Backup wallet
    pub async fn backup(&self) -> Result<String> {
        self.provider.backup().await
    }
    
    /// Get mnemonic
    pub async fn get_mnemonic(&self) -> Result<Option<String>> {
        self.provider.get_mnemonic().await
    }
    
    /// Get network
    pub fn get_network(&self) -> Network {
        self.provider.get_network()
    }
    
    /// Get internal key for wallet
    pub async fn get_internal_key(&self) -> Result<bitcoin::XOnlyPublicKey> {
        self.provider.get_internal_key().await.map(|(key, _)| key)
    }
    
    /// Sign PSBT
    pub async fn sign_psbt(&mut self, psbt: &bitcoin::psbt::Psbt) -> Result<bitcoin::psbt::Psbt> {
        self.provider.sign_psbt(psbt).await
    }
    
    /// Get keypair for wallet
    pub async fn get_keypair(&self) -> Result<bitcoin::secp256k1::Keypair> {
        self.provider.get_keypair().await
    }
}

/// Address type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AddressType {
    P2PKH,
    P2SH,
    P2WPKH,
    P2WSH,
    P2TR,
}

impl AddressType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            AddressType::P2PKH => "p2pkh",
            AddressType::P2SH => "p2sh",
            AddressType::P2WPKH => "p2wpkh",
            AddressType::P2WSH => "p2wsh",
            AddressType::P2TR => "p2tr",
        }
    }
    
}

impl core::str::FromStr for AddressType {
    type Err = AlkanesError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "p2pkh" => Ok(AddressType::P2PKH),
            "p2sh" => Ok(AddressType::P2SH),
            "p2wpkh" => Ok(AddressType::P2WPKH),
            "p2wsh" => Ok(AddressType::P2WSH),
            "p2tr" => Ok(AddressType::P2TR),
            _ => Err(AlkanesError::Parse(format!("Unknown address type: {{s}}"))),
        }
    }
}

/// Derivation path utilities
pub mod derivation {
    use super::*;
    
    /// Get derivation path for address type and network
    pub fn get_derivation_path(address_type: &AddressType, network: Network, account: u32, change: u32, index: u32) -> String {
        let coin_type = match network {
            Network::Bitcoin => 0,
            _ => 1, // Testnet, Signet, Regtest
        };
        
        let purpose = match address_type {
            AddressType::P2PKH => 44,
            AddressType::P2SH => 49,
            AddressType::P2WPKH => 84,
            AddressType::P2WSH => 84,
            AddressType::P2TR => 86,
        };
        
        format!("m/{{purpose}}'/{{coin_type}}'/{{account}}'/{{change}}/{{index}}")
    }
    
    /// Parse derivation path
    pub fn parse_derivation_path(path: &str) -> Result<(u32, u32, u32, u32, u32)> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() != 6 || parts[0] != "m" {
            return Err(AlkanesError::Parse("Invalid derivation path format".to_string()));
        }
        
        let purpose = parts[1].trim_end_matches('\'').parse::<u32>()
            .map_err(|_| AlkanesError::Parse("Invalid purpose in derivation path".to_string()))?;
        let coin_type = parts[2].trim_end_matches('\'').parse::<u32>()
            .map_err(|_| AlkanesError::Parse("Invalid coin type in derivation path".to_string()))?;
        let account = parts[3].trim_end_matches('\'').parse::<u32>()
            .map_err(|_| AlkanesError::Parse("Invalid account in derivation path".to_string()))?;
        let change = parts[4].parse::<u32>()
            .map_err(|_| AlkanesError::Parse("Invalid change in derivation path".to_string()))?;
        let index = parts[5].parse::<u32>()
            .map_err(|_| AlkanesError::Parse("Invalid index in derivation path".to_string()))?;
        
        Ok((purpose, coin_type, account, change, index))
    }
    
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;
    
    #[test]
    fn test_balance_calculations() {
        let balance = WalletBalance {
            confirmed: 100000,
            pending: 75000,
        };
        
        assert_eq!(balance.confirmed, 100000);
        assert_eq!(balance.pending, 75000);
    }
    
    #[test]
    fn test_address_type_parsing() {
        assert!(matches!(AddressType::from_str("p2tr").unwrap(), AddressType::P2TR));
        assert!(matches!(AddressType::from_str("P2WPKH").unwrap(), AddressType::P2WPKH));
        assert!(AddressType::from_str("invalid").is_err());
    }
    
    #[test]
    fn test_derivation_path() {
        let path = derivation::get_derivation_path(&AddressType::P2TR, Network::Bitcoin, 0, 0, 0);
        assert_eq!(path, "m/86'/0'/0'/0/0");
        
        let path = derivation::get_derivation_path(&AddressType::P2WPKH, Network::Testnet, 0, 1, 5);
        assert_eq!(path, "m/84'/1'/0'/1/5");
    }
    
    #[test]
    fn test_parse_derivation_path() {
        let (purpose, coin_type, account, change, index) = 
            derivation::parse_derivation_path("m/86'/0'/0'/0/0").unwrap();
        assert_eq!(purpose, 86);
        assert_eq!(coin_type, 0);
        assert_eq!(account, 0);
        assert_eq!(change, 0);
        assert_eq!(index, 0);
        
        assert!(derivation::parse_derivation_path("invalid").is_err());
    }
}
