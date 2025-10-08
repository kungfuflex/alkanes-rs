use crate::{Result, DeezelError};
use alloc::{string::ToString, format};
use crate::traits::*;
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

/// Wallet configuration
#[derive(Debug, Clone)]
pub struct WalletConfig {
    pub wallet_path: String,
    pub network: Network,
    pub bitcoin_rpc_url: String,
    pub metashrew_rpc_url: String,
    pub network_params: Option<NetworkParams>,
}

/// Wallet manager that works with any provider
pub struct Wallet<P: DeezelProvider> {
    provider: P,
    _config: WalletConfig,
}

impl<P: DeezelProvider> Wallet<P> {
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
        let trait_config = crate::traits::WalletConfig {
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
        let trait_config = crate::traits::WalletConfig {
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
        crate::traits::WalletProvider::get_balance(&self.provider, None).await
    }
    
    /// Get wallet address
    pub async fn get_address(&self) -> Result<String> {
        crate::traits::WalletProvider::get_address(&self.provider).await
    }
    
    /// Get multiple addresses
    pub async fn get_addresses(&self, count: u32) -> Result<Vec<AddressInfo>> {
        let trait_addresses = self.provider.get_addresses(count).await?;
        Ok(trait_addresses.into_iter().map(|addr| AddressInfo {
            address: addr.address.clone(),
            index: addr.index,
            used: addr.used,
        }).collect())
    }
    
    /// Send Bitcoin transaction
    pub async fn send(&mut self, params: SendParams) -> Result<String> {
        let trait_params = crate::traits::SendParams {
            address: params.address,
            amount: params.amount,
            fee_rate: params.fee_rate,
            send_all: params.send_all,
            from: params.from,
            change_address: params.change_address,
            auto_confirm: params.auto_confirm,
        };
        self.provider.send(trait_params).await
    }
    
    /// Get UTXOs
    pub async fn get_utxos(&self) -> Result<Vec<UtxoInfo>> {
        let trait_utxos = self.provider.get_utxos(false, None).await?;
        let wallet_utxos = trait_utxos.into_iter().map(|(_, utxo)| UtxoInfo {
            txid: utxo.txid,
            vout: utxo.vout,
            amount: utxo.amount,
            address: utxo.address.clone(),
            script_pubkey: utxo.script_pubkey.unwrap_or_default(),
            confirmations: utxo.confirmations,
            frozen: utxo.frozen,
        }).collect();
        Ok(wallet_utxos)
    }
    
    /// Get enriched UTXOs (with additional metadata)
    pub async fn get_enriched_utxos(&self) -> Result<Vec<EnrichedUtxo>> {
        let utxos = self.provider.get_utxos(false, None).await?;
        let mut enriched = Vec::new();
        
        for (_, utxo) in utxos {
            enriched.push(EnrichedUtxo {
                utxo: UtxoInfo {
                    txid: utxo.txid.clone(),
                    vout: utxo.vout,
                    amount: utxo.amount,
                    address: utxo.address.clone(),
                    script_pubkey: utxo.script_pubkey.clone().unwrap_or_else(bitcoin::ScriptBuf::new),
                    confirmations: utxo.confirmations,
                    frozen: utxo.frozen,
                },
                freeze_reason: utxo.freeze_reason.clone(),
                block_height: utxo.block_height,
                has_inscriptions: utxo.has_inscriptions,
                has_runes: utxo.has_runes,
                has_alkanes: utxo.has_alkanes,
                is_coinbase: utxo.is_coinbase,
            });
        }
        
        Ok(enriched)
    }
    
    /// Get UTXOs for a specific address
    pub async fn get_enriched_utxos_for_address(&self, address: &str) -> Result<Vec<EnrichedUtxo>> {
        let utxos = self.provider.get_utxos(false, Some(vec![address.to_string()])).await?;
        let mut enriched = Vec::new();
        
        for (_, utxo) in utxos {
            enriched.push(EnrichedUtxo {
                utxo: UtxoInfo {
                    txid: utxo.txid.clone(),
                    vout: utxo.vout,
                    amount: utxo.amount,
                    address: utxo.address.clone(),
                    script_pubkey: utxo.script_pubkey.clone().unwrap_or_else(bitcoin::ScriptBuf::new),
                    confirmations: utxo.confirmations,
                    frozen: utxo.frozen,
                },
                freeze_reason: utxo.freeze_reason.clone(),
                block_height: utxo.block_height,
                has_inscriptions: utxo.has_inscriptions,
                has_runes: utxo.has_runes,
                has_alkanes: utxo.has_alkanes,
                is_coinbase: utxo.is_coinbase,
            });
        }
        
        Ok(enriched)
    }
    
    /// Get transaction history
    pub async fn get_history(&self, count: u32, address: Option<String>) -> Result<Vec<TransactionInfo>> {
        let trait_history = self.provider.get_history(count, address).await?;
        Ok(trait_history.into_iter().map(|tx| TransactionInfo {
            txid: tx.txid,
            block_height: tx.block_height,
            block_time: tx.block_time,
            confirmed: tx.confirmed,
            fee: tx.fee,
            inputs: tx.inputs.into_iter().map(|input| TransactionInput {
                txid: input.txid,
                vout: input.vout,
                address: input.address,
                amount: input.amount,
            }).collect(),
            outputs: tx.outputs.into_iter().map(|output| TransactionOutput {
                address: output.address,
                amount: output.amount,
                script_hex: hex::encode(output.script.as_bytes()),
            }).collect(),
        }).collect())
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
        let trait_params = crate::traits::SendParams {
            address: params.address,
            amount: params.amount,
            fee_rate: params.fee_rate,
            send_all: params.send_all,
            from: params.from,
            change_address: params.change_address,
            auto_confirm: params.auto_confirm,
        };
        self.provider.create_transaction(trait_params).await
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
        let trait_estimate = self.provider.estimate_fee(target).await?;
        Ok(FeeEstimate {
            fee_rate: trait_estimate.fee_rate,
            target_blocks: trait_estimate.target_blocks,
        })
    }
    
    /// Get current fee rates
    pub async fn get_fee_rates(&self) -> Result<FeeRates> {
        let trait_rates = self.provider.get_fee_rates().await?;
        Ok(FeeRates {
            slow: trait_rates.slow,
            medium: trait_rates.medium,
            fast: trait_rates.fast,
        })
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


/// Send transaction parameters
#[derive(Debug, Clone)]
pub struct SendParams {
    pub address: String,
    pub amount: u64,
    pub fee_rate: Option<f32>,
    pub send_all: bool,
    pub from: Option<Vec<String>>,
    pub change_address: Option<String>,
    pub auto_confirm: bool,
}

/// UTXO information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoInfo {
    pub txid: String,
    pub vout: u32,
    pub amount: u64,
    pub address: String,
    pub script_pubkey: bitcoin::ScriptBuf,
    pub confirmations: u32,
    pub frozen: bool,
}

impl From<crate::traits::UtxoInfo> for UtxoInfo {
    fn from(utxo: crate::traits::UtxoInfo) -> Self {
        Self {
            txid: utxo.txid,
            vout: utxo.vout,
            amount: utxo.amount,
            address: utxo.address,
            script_pubkey: utxo.script_pubkey.unwrap_or_default(),
            confirmations: utxo.confirmations,
            frozen: utxo.frozen,
        }
    }
}
/// Enriched UTXO with additional metadata
#[derive(Debug, Clone)]
pub struct EnrichedUtxo {
    pub utxo: UtxoInfo,
    pub freeze_reason: Option<String>,
    pub block_height: Option<u64>,
    pub has_inscriptions: bool,
    pub has_runes: bool,
    pub has_alkanes: bool,
    pub is_coinbase: bool,
}

/// Address information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInfo {
    pub address: String,
    pub index: u32,
    pub used: bool,
}

/// Transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub txid: String,
    pub block_height: Option<u64>,
    pub block_time: Option<u64>,
    pub confirmed: bool,
    pub fee: Option<u64>,
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<TransactionOutput>,
}

/// Transaction input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub txid: String,
    pub vout: u32,
    pub address: Option<String>,
    pub amount: Option<u64>,
}

/// Transaction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOutput {
    pub address: Option<String>,
    pub amount: u64,
    pub script_hex: String,
}

/// Fee estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    pub fee_rate: f32,
    pub target_blocks: u32,
}

/// Fee rates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeRates {
    pub fast: f32,
    pub medium: f32,
    pub slow: f32,
}

/// Wallet creation parameters
#[derive(Debug, Clone)]
pub struct WalletCreationParams {
    pub mnemonic: Option<String>,
    pub passphrase: Option<String>,
    pub derivation_path: Option<String>,
    pub network: Network,
}

/// Wallet information
#[derive(Debug, Clone)]
pub struct WalletInfo {
    pub address: String,
    pub network: Network,
    pub mnemonic: Option<String>,
    pub derivation_path: String,
}

/// Wallet statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletStats {
    pub total_balance: u64,
    pub confirmed_balance: u64,
    pub pending_balance: u64,
    pub total_utxos: usize,
    pub frozen_utxos: usize,
    pub total_transactions: usize,
    pub last_sync: Option<u64>,
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
    type Err = DeezelError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "p2pkh" => Ok(AddressType::P2PKH),
            "p2sh" => Ok(AddressType::P2SH),
            "p2wpkh" => Ok(AddressType::P2WPKH),
            "p2wsh" => Ok(AddressType::P2WSH),
            "p2tr" => Ok(AddressType::P2TR),
            _ => Err(DeezelError::Parse(format!("Unknown address type: {s}"))),
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
        
        format!("m/{purpose}'/{coin_type}'/{account}'/{change}/{index}")
    }
    
    /// Parse derivation path
    pub fn parse_derivation_path(path: &str) -> Result<(u32, u32, u32, u32, u32)> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() != 6 || parts[0] != "m" {
            return Err(DeezelError::Parse("Invalid derivation path format".to_string()));
        }
        
        let purpose = parts[1].trim_end_matches('\'').parse::<u32>()
            .map_err(|_| DeezelError::Parse("Invalid purpose in derivation path".to_string()))?;
        let coin_type = parts[2].trim_end_matches('\'').parse::<u32>()
            .map_err(|_| DeezelError::Parse("Invalid coin type in derivation path".to_string()))?;
        let account = parts[3].trim_end_matches('\'').parse::<u32>()
            .map_err(|_| DeezelError::Parse("Invalid account in derivation path".to_string()))?;
        let change = parts[4].parse::<u32>()
            .map_err(|_| DeezelError::Parse("Invalid change in derivation path".to_string()))?;
        let index = parts[5].parse::<u32>()
            .map_err(|_| DeezelError::Parse("Invalid index in derivation path".to_string()))?;
        
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