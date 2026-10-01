
//! Network and RPC configuration for deezel.

use crate::commands::Commands;

use bitcoin::Network;

use serde::{Deserialize, Serialize};

use thiserror::Error;



#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum RpcError {
    #[error("Missing RPC URL for command: {0:?}")]
    MissingRpcUrl(Commands),
    #[error("RPC error {code}: {message}")]
    JsonRpcError { code: i64, message: String },
}

use bech32::Hrp;
use bitcoin::Script;
use metashrew_support::address::{AddressEncoding, Payload};
use std::sync::{Arc, PoisonError, RwLock};

/// Distinguishes the underlying blockchain type for address encoding,
/// derivation paths, and RPC routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChainType {
    Bitcoin,
    Zcash,
}

impl Default for ChainType {
    fn default() -> Self { ChainType::Bitcoin }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkParams {
    pub network: Network,
    pub magic: [u8; 4],
    pub default_port: u16,
    pub rpc_port: u16,
    pub bech32_hrp: String,
    pub bech32_prefix: String,
    pub p2pkh_prefix: u8,
    pub p2sh_prefix: u8,
    pub bitcoin_rpc_url: Option<String>,
    pub metashrew_rpc_url: Option<String>,
    pub esplora_url: Option<String>,
    /// Blockchain type — determines address encoding, derivation paths, signing.
    #[serde(default)]
    pub chain_type: ChainType,
    /// Zcash 2-byte P2PKH version prefix (e.g. [0x1c, 0xb8] for t1...).
    /// When present, overrides the single-byte `p2pkh_prefix` for Base58Check encoding.
    #[serde(default)]
    pub p2pkh_prefix_bytes: Option<Vec<u8>>,
    /// Zcash 2-byte P2SH version prefix (e.g. [0x1c, 0xbd] for t3...).
    #[serde(default)]
    pub p2sh_prefix_bytes: Option<Vec<u8>>,
}

impl NetworkParams {
    /// Check that the address prefixes can produce unambiguous addresses.
    ///
    /// Bitcoin-family chains need a valid lowercase bech32 HRP and distinct
    /// one-byte base58 prefixes. Zcash has no bech32 encoding and instead needs
    /// distinct, non-empty two-byte base58 prefixes.
    pub fn validate(&self) -> Result<(), AlkanesError> {
        let invalid = |msg: String| AlkanesError::Configuration(format!("invalid network params: {}", msg));
        match self.chain_type {
            ChainType::Bitcoin => {
                let mut hrps = vec![("bech32_hrp", &self.bech32_hrp)];
                if !self.bech32_prefix.is_empty() {
                    hrps.push(("bech32_prefix", &self.bech32_prefix));
                }
                for (field, value) in hrps {
                    let hrp = Hrp::parse(value)
                        .map_err(|e| invalid(format!("{} {:?}: {}", field, value, e)))?;
                    if hrp.to_lowercase() != *value {
                        return Err(invalid(format!("{} {:?} must be lowercase", field, value)));
                    }
                }
                if self.p2pkh_prefix == self.p2sh_prefix {
                    return Err(invalid(format!(
                        "p2pkh and p2sh prefixes must differ (both are {:#04x})",
                        self.p2pkh_prefix
                    )));
                }
            }
            ChainType::Zcash => {
                let pkh = self.p2pkh_prefix_bytes.as_deref().filter(|b| !b.is_empty())
                    .ok_or_else(|| invalid("Zcash network needs p2pkh_prefix_bytes".to_string()))?;
                let sh = self.p2sh_prefix_bytes.as_deref().filter(|b| !b.is_empty())
                    .ok_or_else(|| invalid("Zcash network needs p2sh_prefix_bytes".to_string()))?;
                if pkh == sh {
                    return Err(invalid("Zcash p2pkh and p2sh prefix bytes must differ".to_string()));
                }
            }
        }
        Ok(())
    }
}

// The active configuration is shared as an `Arc` snapshot behind a lock.
// Readers never borrow the global itself, so replacing it cannot invalidate a
// value someone is still using, and concurrent native callers are synchronised.
// The critical sections only move an `Option<Arc<_>>`, which a panic cannot
// leave half-written, so a poisoned lock is still safe to use.
static NETWORK: RwLock<Option<Arc<NetworkParams>>> = RwLock::new(None);

/// Validate and install `params` as the active network configuration,
/// replacing any previous one. Snapshots taken earlier keep their values.
/// On error the current configuration is left unchanged.
pub fn set_network(params: impl Into<Arc<NetworkParams>>) -> Result<(), AlkanesError> {
    let params = params.into();
    params.validate()?;
    let previous = NETWORK.write().unwrap_or_else(PoisonError::into_inner).replace(params);
    drop(previous); // outside the lock
    Ok(())
}

/// A snapshot of the active configuration.
///
/// # Panics
/// If no configuration has been installed; use [`get_network_option`] to
/// handle that case.
pub fn get_network() -> Arc<NetworkParams> {
    get_network_option()
        .expect("network not configured: call set_network() before get_network()")
}

/// A snapshot of the active configuration, or `None` before initialisation.
pub fn get_network_option() -> Option<Arc<NetworkParams>> {
    NETWORK.read().unwrap_or_else(PoisonError::into_inner).clone()
}

pub fn to_address_str(script: &Script) -> Result<String, anyhow::Error> {
    let config = get_network_option()
        .ok_or_else(|| anyhow::anyhow!("network not configured: call set_network() first"))?;
    let payload = Payload::from_script(script)?;

    // Zcash transparent addresses use 2-byte version prefixes in Base58Check.
    // Handle this separately since AddressEncoding only supports 1-byte prefixes.
    if config.chain_type == ChainType::Zcash {
        return zcash_address_str(&config, &payload);
    }

    Ok(AddressEncoding {
        p2pkh_prefix: config.p2pkh_prefix,
        p2sh_prefix: config.p2sh_prefix,
        hrp: Hrp::parse_unchecked(&config.bech32_hrp),
        payload: &payload,
    }
    .to_string())
}

/// Encode a Zcash transparent address with 2-byte version prefix Base58Check.
/// Produces t1... (P2PKH) or t3... (P2SH) addresses on mainnet.
fn zcash_address_str(config: &NetworkParams, payload: &Payload) -> Result<String, anyhow::Error> {
    use bitcoin::base58;

    match payload {
        Payload::PubkeyHash(hash) => {
            let prefix = config.p2pkh_prefix_bytes.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Zcash network missing p2pkh_prefix_bytes"))?;
            let mut prefixed = Vec::with_capacity(prefix.len() + 20);
            prefixed.extend_from_slice(prefix);
            prefixed.extend_from_slice(&hash[..]);
            Ok(base58::encode_check(&prefixed))
        }
        Payload::ScriptHash(hash) => {
            let prefix = config.p2sh_prefix_bytes.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Zcash network missing p2sh_prefix_bytes"))?;
            let mut prefixed = Vec::with_capacity(prefix.len() + 20);
            prefixed.extend_from_slice(prefix);
            prefixed.extend_from_slice(&hash[..]);
            Ok(base58::encode_check(&prefixed))
        }
        Payload::WitnessProgram(_) => {
            Err(anyhow::anyhow!("Zcash does not support SegWit/Taproot witness programs"))
        }
        _ => Err(anyhow::anyhow!("Unsupported payload type for Zcash address encoding")),
    }
}




use crate::AlkanesError;
use clap::Args;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeezelNetwork(pub Network);

impl FromStr for DeezelNetwork {
    type Err = AlkanesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(DeezelNetwork(Network::Bitcoin)),
            "testnet" => Ok(DeezelNetwork(Network::Testnet)),
            "signet" => Ok(DeezelNetwork(Network::Signet)),
            "regtest" => Ok(DeezelNetwork(Network::Regtest)),
            // Zcash networks reuse Bitcoin Network enum — differentiation is via ChainType
            "zcash" | "zcash-mainnet" => Ok(DeezelNetwork(Network::Bitcoin)),
            "zcash-testnet" => Ok(DeezelNetwork(Network::Testnet)),
            "zcash-regtest" => Ok(DeezelNetwork(Network::Regtest)),
            _ => Err(AlkanesError::InvalidParameters(format!("Invalid network: {}", s))),
        }
    }
}

#[derive(Args, Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Network provider
    #[arg(short = 'p', long, default_value = "regtest")]
    pub provider: String,

    /// Bitcoin RPC URL (defaults based on provider if not provided)
    #[arg(long)]
    pub bitcoin_rpc_url: Option<String>,

    /// JSON-RPC URL (defaults based on network if not provided)
    #[arg(long)]
    pub jsonrpc_url: Option<String>,

    /// Titan API URL (alternative to jsonrpc_url, uses REST API)
    #[arg(long)]
    pub titan_api_url: Option<String>,

    /// Esplora API URL (overrides JSON-RPC for Esplora calls, enables REST)
    #[arg(long)]
    pub esplora_url: Option<String>,

    /// Ord API URL (overrides JSON-RPC for ord calls, enables REST)
    #[arg(long)]
    pub ord_url: Option<String>,

    /// Metashrew RPC URL (overrides JSON-RPC for metashrew calls)
    #[arg(long)]
    pub metashrew_rpc_url: Option<String>,

    /// BRC20-Prog RPC URL (for querying BRC20-Prog contracts, defaults based on network)
    #[arg(long)]
    pub brc20_prog_rpc_url: Option<String>,

    /// Data API URL (for analytics and indexing data, defaults based on network)
    #[arg(long)]
    pub data_api_url: Option<String>,

    /// ESPO RPC URL (for alkanes balance indexer, defaults to jsonrpc_url + /espo)
    #[arg(long)]
    pub espo_rpc_url: Option<String>,

    /// Qubitcoin RPC URL — single-process mode where qubitcoind provides all indexers
    /// via secondaryview/secondaryheight. Overrides jsonrpc_url, metashrew_rpc_url,
    /// esplora_url, and ord_url. Uses "alkanes", "esplora", and "brc20-prog" labels.
    #[arg(long)]
    pub qubitcoin_rpc_url: Option<String>,

    /// Quzec RPC URL — single-process mode where quzec provides all indexers
    /// for Zcash networks. Same pattern as qubitcoin_rpc_url but for Zcash chains.
    #[arg(long)]
    pub quzec_rpc_url: Option<String>,

    /// Subfrost API Key (optional, can also be set via SUBFROST_API_KEY environment variable)
    #[arg(long)]
    pub subfrost_api_key: Option<String>,

    /// RPC timeout in seconds
    #[arg(long, default_value = "600")]
    pub timeout_seconds: u64,

    /// Custom headers for JSON-RPC requests (can be specified multiple times)
    /// Format: "Header-Name: Header-Value" (e.g., "Host: signet.subfrost.io")
    #[arg(long = "jsonrpc-header", value_name = "HEADER")]
    pub jsonrpc_headers: Vec<String>,
}

/// Type of RPC backend to use
#[derive(Debug, Clone, PartialEq)]
pub enum RpcBackendType {
    JsonRpc,
    Rest,
}

/// RPC target for different service types
#[derive(Debug, Clone)]
pub struct RpcTarget {
    pub url: String,
    pub backend_type: RpcBackendType,
}

impl RpcConfig {
    /// Validate that only one backend is configured (jsonrpc_url OR titan_api_url)
    pub fn validate(&self) -> Result<(), AlkanesError> {
        if self.jsonrpc_url.is_some() && self.titan_api_url.is_some() {
            return Err(AlkanesError::Configuration(
                "Cannot specify both --jsonrpc-url and --titan-api-url. Please choose one backend.".to_string()
            ));
        }
        Ok(())
    }

    /// Returns true if using Titan REST API as backend
    pub fn using_titan_api(&self) -> bool {
        self.titan_api_url.is_some()
    }

    /// Get the effective JSON-RPC URL (jsonrpc_url or default based on provider)
    fn get_effective_jsonrpc_url(&self) -> Option<String> {
        self.jsonrpc_url.clone()
    }
    
    /// Get effective Subfrost API key (from flag or environment variable)
    pub fn get_subfrost_api_key(&self) -> Option<String> {
        self.subfrost_api_key.clone().or_else(|| {
            std::env::var("SUBFROST_API_KEY").ok()
        })
    }

    /// Get parsed JSON-RPC headers as (name, value) pairs
    /// Headers should be in format "Header-Name: Header-Value"
    pub fn get_jsonrpc_headers(&self) -> Vec<(String, String)> {
        self.jsonrpc_headers
            .iter()
            .filter_map(|header| {
                let parts: Vec<&str> = header.splitn(2, ':').collect();
                if parts.len() == 2 {
                    Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
                } else {
                    log::warn!("Invalid header format (expected 'Name: Value'): {}", header);
                    None
                }
            })
            .collect()
    }
    
    /// Get default JSON-RPC URL for the network
    fn get_default_jsonrpc_url(&self) -> String {
        match self.provider.as_str() {
            "mainnet" => "https://mainnet.subfrost.io/v4/jsonrpc".to_string(),
            "signet" => "https://signet.subfrost.io/v4/jsonrpc".to_string(),
            "subfrost-regtest" => "https://regtest.subfrost.io/v4/jsonrpc".to_string(),
            "zcash" | "zcash-mainnet" => "http://localhost:8232".to_string(),
            "zcash-testnet" => "http://localhost:18232".to_string(),
            "zcash-regtest" => "http://localhost:18232".to_string(),
            _ => "http://localhost:18888".to_string(), // regtest
        }
    }
    
    /// Get default BRC20-Prog RPC URL for the network
    /// For mainnet/signet: use brc20.build URLs
    /// For everything else: derive from jsonrpc_url with /brc20-prog path appended
    pub fn get_default_brc20_prog_rpc_url(&self) -> Option<String> {
        match self.provider.as_str() {
            "mainnet" => Some("https://rpc.brc20.build".to_string()),
            "signet" => Some("https://rpc-signet.brc20.build".to_string()),
            _ => {
                // Derive from jsonrpc_url with /brc20-prog path appended
                self.get_effective_jsonrpc_url()
                    .or_else(|| Some(self.get_default_jsonrpc_url()))
                    .map(|url| {
                        let base = url.trim_end_matches('/');
                        format!("{}/brc20-prog", base)
                    })
            }
        }
    }
    
    /// Get default Data API URL for the network
    pub fn get_default_data_api_url(&self) -> String {
        match self.provider.as_str() {
            "mainnet" => "https://mainnet.subfrost.io/v4/api".to_string(),
            "signet" => "https://signet.subfrost.io/v4/api".to_string(),
            "subfrost-regtest" => "https://regtest.subfrost.io/v4/api".to_string(),
            _ => "http://localhost:3000".to_string(), // regtest
        }
    }
    
    /// Get the Data API target
    /// Priority: data_api_url > default based on network
    pub fn get_data_api_target(&self) -> RpcTarget {
        let url = self.data_api_url.clone().unwrap_or_else(|| self.get_default_data_api_url());
        RpcTarget {
            url,
            backend_type: RpcBackendType::JsonRpc,
        }
    }
    
    /// Returns true if operating in qubitcoin single-process mode.
    pub fn is_qubitcoin_mode(&self) -> bool {
        self.qubitcoin_rpc_url.is_some()
    }

    /// Returns true if operating in quzec single-process mode (Zcash).
    pub fn is_quzec_mode(&self) -> bool {
        self.quzec_rpc_url.is_some()
    }

    /// Returns true if this is a Zcash network provider.
    pub fn is_zcash(&self) -> bool {
        self.provider.starts_with("zcash")
    }

    /// Returns the unified single-process RPC URL if in qubitcoin or quzec mode.
    fn get_unified_rpc_url(&self) -> Option<&String> {
        self.qubitcoin_rpc_url.as_ref().or(self.quzec_rpc_url.as_ref())
    }

    /// Get the RPC target for Bitcoin Core operations
    /// Priority: qubitcoin_rpc_url > bitcoin_rpc_url > jsonrpc_url (JSONRPC translation) > default
    pub fn get_bitcoin_rpc_target(&self) -> RpcTarget {
        if let Some(url) = self.get_unified_rpc_url() {
            return RpcTarget { url: url.clone(), backend_type: RpcBackendType::JsonRpc };
        }
        if let Some(ref url) = self.bitcoin_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else if let Some(url) = self.get_effective_jsonrpc_url() {
            RpcTarget {
                url,
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_jsonrpc_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get the RPC target for Metashrew operations (alkanes.wasm view functions)
    /// Priority: metashrew_rpc_url > jsonrpc_url > default jsonrpc
    pub fn get_metashrew_rpc_target(&self) -> RpcTarget {
        if let Some(url) = self.get_unified_rpc_url() {
            return RpcTarget { url: url.clone(), backend_type: RpcBackendType::JsonRpc };
        }
        if let Some(ref url) = self.metashrew_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else if let Some(url) = self.get_effective_jsonrpc_url() {
            RpcTarget {
                url,
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_jsonrpc_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get the RPC target for Esplora operations
    /// Priority: esplora_url (REST) > jsonrpc_url (JSONRPC translation) > default jsonrpc
    pub fn get_esplora_rpc_target(&self) -> RpcTarget {
        if let Some(url) = self.get_unified_rpc_url() {
            return RpcTarget { url: url.clone(), backend_type: RpcBackendType::JsonRpc };
        }
        if let Some(ref url) = self.esplora_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::Rest,
            }
        } else if let Some(url) = self.get_effective_jsonrpc_url() {
            RpcTarget {
                url,
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_jsonrpc_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get the RPC target for Ord operations
    /// Priority: ord_url (REST) > jsonrpc_url (JSONRPC translation) > default jsonrpc
    pub fn get_ord_rpc_target(&self) -> RpcTarget {
        if let Some(url) = self.get_unified_rpc_url() {
            return RpcTarget { url: url.clone(), backend_type: RpcBackendType::JsonRpc };
        }
        if let Some(ref url) = self.ord_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::Rest,
            }
        } else if let Some(url) = self.get_effective_jsonrpc_url() {
            RpcTarget {
                url,
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_jsonrpc_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get default ESPO RPC URL for the network
    /// Derives from jsonrpc_url with /espo path appended
    pub fn get_default_espo_rpc_url(&self) -> Option<String> {
        self.get_effective_jsonrpc_url()
            .or_else(|| Some(self.get_default_jsonrpc_url()))
            .map(|url| {
                let base = url.trim_end_matches('/');
                format!("{}/espo", base)
            })
    }

    /// Get the RPC target for ESPO operations (alkanes balance indexer)
    /// Priority: espo_rpc_url > jsonrpc_url + /espo > default jsonrpc + /espo
    pub fn get_espo_rpc_target(&self) -> RpcTarget {
        if let Some(ref url) = self.espo_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else if let Some(url) = self.get_default_espo_rpc_url() {
            RpcTarget {
                url,
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: format!("{}/espo", self.get_default_jsonrpc_url()),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }

    /// Get the RPC target for Alkanes operations (view functions, protorunes, etc.)
    /// Priority: titan_api_url (REST) > jsonrpc_url (JSONRPC) > default jsonrpc
    pub fn get_alkanes_rpc_target(&self) -> RpcTarget {
        if let Some(ref url) = self.titan_api_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::Rest,
            }
        } else if let Some(url) = self.get_effective_jsonrpc_url() {
            RpcTarget {
                url,
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_jsonrpc_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get the RPC target for Wallet operations (used by alkanes execute)
    /// Priority: titan_api_url (REST) > jsonrpc_url (JSONRPC) > default jsonrpc
    /// Note: Wallet operations like send use esplora/bitcoin backends separately
    pub fn get_wallet_rpc_target(&self) -> RpcTarget {
        self.get_alkanes_rpc_target()
    }
}

/// Get default BRC20-Prog RPC URL for a given network
pub fn get_default_brc20_prog_rpc_url(network: bitcoin::Network) -> String {
    match network {
        bitcoin::Network::Bitcoin => "https://rpc.brc20.build".to_string(),
        bitcoin::Network::Signet => "https://rpc-signet.brc20.build".to_string(),
        bitcoin::Network::Regtest => "http://localhost:3002".to_string(),
        _ => "https://rpc-signet.brc20.build".to_string(), // Default to signet for other networks
    }
}



impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            provider: "regtest".to_string(),
            bitcoin_rpc_url: None,
            jsonrpc_url: None,
            titan_api_url: None,
            esplora_url: None,
            ord_url: None,
            metashrew_rpc_url: Some("http://localhost:18888".to_string()),
            brc20_prog_rpc_url: None,
            data_api_url: None,
            espo_rpc_url: None,
            qubitcoin_rpc_url: None,
            quzec_rpc_url: None,
            subfrost_api_key: None,
            timeout_seconds: 600,
            jsonrpc_headers: Vec::new(),
        }
    }
}

impl Default for NetworkParams {
    fn default() -> Self {
        Self {
            network: Network::Regtest,
            magic: [0x00; 4],
            default_port: 0,
            rpc_port: 0,
            bech32_hrp: String::new(),
            bech32_prefix: String::new(),
            p2pkh_prefix: 0,
            p2sh_prefix: 0,
            bitcoin_rpc_url: None,
            metashrew_rpc_url: None,
            esplora_url: None,
            chain_type: ChainType::Bitcoin,
            p2pkh_prefix_bytes: None,
            p2sh_prefix_bytes: None,
        }
    }
}

impl NetworkParams {
    pub fn regtest() -> Self {
        Self {
            network: Network::Regtest,
            magic: [0xfa, 0xbf, 0xb5, 0xda],
            default_port: 18444,
            rpc_port: 18443,
            bech32_hrp: "bcrt".to_string(),
            bech32_prefix: "bcrt".to_string(),
            p2pkh_prefix: 0x6f,
            p2sh_prefix: 0xc4,
            bitcoin_rpc_url: Some("http://localhost:18443".to_string()),
            metashrew_rpc_url: Some("http://localhost:18888".to_string()),
            esplora_url: None,
            ..Default::default()
        }
    }

    pub fn from_network_str(network: &str) -> Result<Self, AlkanesError> {
        match network {
            "regtest" => Ok(Self::regtest()),
            // "bitcoin" is the Display output of bitcoin::Network::Bitcoin,
            // so accept it as an alias for "mainnet"
            "bitcoin" => Ok(Self {
                network: Network::Bitcoin,
                magic: [0xf9, 0xbe, 0xb4, 0xd9],
                default_port: 8333,
                rpc_port: 8332,
                bech32_hrp: "bc".to_string(),
                bech32_prefix: "bc".to_string(),
                p2pkh_prefix: 0x00,
                p2sh_prefix: 0x05,
                bitcoin_rpc_url: None,
                metashrew_rpc_url: None,
                esplora_url: None,
                ..Default::default()
            }),
            "subfrost-regtest" => Ok(Self {
                network: Network::Regtest,
                magic: [0xfa, 0xbf, 0xb5, 0xda],
                default_port: 18444,
                rpc_port: 18443,
                bech32_hrp: "bcrt".to_string(),
                bech32_prefix: "bcrt".to_string(),
                p2pkh_prefix: 0x6f,
                p2sh_prefix: 0xc4,
                bitcoin_rpc_url: Some("https://regtest.subfrost.io/v4/jsonrpc".to_string()),
                metashrew_rpc_url: Some("https://regtest.subfrost.io/v4/jsonrpc".to_string()),
                esplora_url: None,  // Subfrost uses JSON-RPC esplora_* methods, not REST API
                ..Default::default()
            }),
            "mainnet" => Ok(Self {
                network: Network::Bitcoin,
                magic: [0xf9, 0xbe, 0xb4, 0xd9],
                default_port: 8333,
                rpc_port: 8332,
                bech32_hrp: "bc".to_string(),
                bech32_prefix: "bc".to_string(),
                p2pkh_prefix: 0x00,
                p2sh_prefix: 0x05,
                bitcoin_rpc_url: None,
                metashrew_rpc_url: None,
                esplora_url: None,
                ..Default::default()
            }),
            "testnet" => Ok(Self {
                network: Network::Testnet,
                magic: [0x0b, 0x11, 0x09, 0x07],
                default_port: 18333,
                rpc_port: 18332,
                bech32_hrp: "tb".to_string(),
                bech32_prefix: "tb".to_string(),
                p2pkh_prefix: 0x6f,
                p2sh_prefix: 0xc4,
                bitcoin_rpc_url: None,
                metashrew_rpc_url: None,
                esplora_url: None,
                ..Default::default()
            }),
            "signet" => Ok(Self {
                network: Network::Signet,
                magic: [0x0a, 0x03, 0xcf, 0x40],
                default_port: 38333,
                rpc_port: 38332,
                bech32_hrp: "tb".to_string(),
                bech32_prefix: "tb".to_string(),
                p2pkh_prefix: 0x6f,
                p2sh_prefix: 0xc4,
                bitcoin_rpc_url: None,
                metashrew_rpc_url: None,
                esplora_url: None,
                ..Default::default()
            }),
            // Zcash networks — transparent addresses use 2-byte Base58Check prefixes
            "zcash" | "zcash-mainnet" => Ok(Self {
                network: Network::Bitcoin,
                magic: [0x24, 0xe9, 0x27, 0x64],
                default_port: 8233,
                rpc_port: 8232,
                bech32_hrp: String::new(), // Zcash doesn't use bech32
                bech32_prefix: String::new(),
                p2pkh_prefix: 0x1c, // first byte only — full prefix in p2pkh_prefix_bytes
                p2sh_prefix: 0x1c,
                bitcoin_rpc_url: None,
                metashrew_rpc_url: None,
                esplora_url: None,
                chain_type: ChainType::Zcash,
                p2pkh_prefix_bytes: Some(vec![0x1c, 0xb8]), // t1...
                p2sh_prefix_bytes: Some(vec![0x1c, 0xbd]),  // t3...
            }),
            "zcash-testnet" => Ok(Self {
                network: Network::Testnet,
                magic: [0xfa, 0x1a, 0xf9, 0xbf],
                default_port: 18233,
                rpc_port: 18232,
                bech32_hrp: String::new(),
                bech32_prefix: String::new(),
                p2pkh_prefix: 0x1d,
                p2sh_prefix: 0x1c,
                bitcoin_rpc_url: None,
                metashrew_rpc_url: None,
                esplora_url: None,
                chain_type: ChainType::Zcash,
                p2pkh_prefix_bytes: Some(vec![0x1d, 0x25]), // tm...
                p2sh_prefix_bytes: Some(vec![0x1c, 0xba]),  // t2...
            }),
            "zcash-regtest" => Ok(Self {
                network: Network::Regtest,
                magic: [0xaa, 0xe8, 0x3f, 0x5f],
                default_port: 18344,
                rpc_port: 18232,
                bech32_hrp: String::new(),
                bech32_prefix: String::new(),
                p2pkh_prefix: 0x1d,
                p2sh_prefix: 0x1c,
                bitcoin_rpc_url: Some("http://localhost:18232".to_string()),
                metashrew_rpc_url: Some("http://localhost:18888".to_string()),
                esplora_url: None,
                chain_type: ChainType::Zcash,
                p2pkh_prefix_bytes: Some(vec![0x1d, 0x25]), // tm... (same as testnet)
                p2sh_prefix_bytes: Some(vec![0x1c, 0xba]),  // t2...
            }),
            _ => Err(AlkanesError::InvalidParameters(format!("Unknown network: {}", network))),
        }
    }

    pub fn from_magic_str(magic_str: &str) -> Result<(u8, u8, String), AlkanesError> {
        let parts: Vec<&str> = magic_str.split(',').collect();
        if parts.len() != 3 {
            return Err(AlkanesError::InvalidParameters(
                "Magic string must be in format: p2pkh_prefix,p2sh_prefix,bech32_hrp".to_string()
            ));
        }
        
        let p2pkh = parts[0].trim().strip_prefix("0x").unwrap_or(parts[0].trim());
        let p2sh = parts[1].trim().strip_prefix("0x").unwrap_or(parts[1].trim());
        let bech32_hrp = parts[2].trim().to_string();
        
        let p2pkh_prefix = u8::from_str_radix(p2pkh, 16)
            .map_err(|e| AlkanesError::InvalidParameters(format!("Invalid p2pkh prefix: {}", e)))?;
        let p2sh_prefix = u8::from_str_radix(p2sh, 16)
            .map_err(|e| AlkanesError::InvalidParameters(format!("Invalid p2sh prefix: {}", e)))?;
        
        Ok((p2pkh_prefix, p2sh_prefix, bech32_hrp))
    }

    pub fn with_custom_magic(network: Network, p2pkh_prefix: u8, p2sh_prefix: u8, bech32_hrp: String) -> Self {
        let base = match network {
            Network::Bitcoin => Self::from_network_str("mainnet").unwrap(),
            Network::Testnet => Self::from_network_str("testnet").unwrap(),
            Network::Signet => Self::from_network_str("signet").unwrap(),
            Network::Regtest => Self::regtest(),
            _ => Self::regtest(),
        };
        
        Self {
            p2pkh_prefix,
            p2sh_prefix,
            bech32_hrp: bech32_hrp.clone(),
            bech32_prefix: bech32_hrp,
            ..base
        }
    }

    pub fn supported_networks() -> Vec<String> {
        vec![
            "mainnet".to_string(), "testnet".to_string(), "signet".to_string(), "regtest".to_string(),
            "zcash".to_string(), "zcash-testnet".to_string(), "zcash-regtest".to_string(),
        ]
    }
}

#[cfg(test)]
mod global_network_tests {
    use super::*;

    // These tests exercise the process-wide slot; they hold a mutex so they do
    // not interleave with each other under the parallel test runner.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn snapshot_survives_replacement_and_invalid_params_are_rejected() {
        let _g = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        set_network(NetworkParams::from_network_str("mainnet").unwrap()).unwrap();
        let snapshot = get_network();
        let hrp: &str = &snapshot.bech32_hrp;

        set_network(NetworkParams::from_network_str("zcash").unwrap()).unwrap();
        set_network(NetworkParams::regtest()).unwrap();
        assert_eq!(hrp, "bc");
        assert_eq!(snapshot.network, Network::Bitcoin);
        assert_eq!(get_network().bech32_hrp, "bcrt");

        // Rejected configurations leave the current one in place.
        assert!(set_network(NetworkParams::default()).is_err());
        assert!(set_network(NetworkParams::with_custom_magic(Network::Regtest, 0x05, 0x05, "bcrt".into())).is_err());
        assert!(set_network(NetworkParams::with_custom_magic(Network::Regtest, 0x6f, 0xc4, "BCRT".into())).is_err());
        let mut zcash = NetworkParams::from_network_str("zcash").unwrap();
        zcash.p2sh_prefix_bytes = None;
        assert!(set_network(zcash).is_err());
        assert_eq!(get_network().bech32_hrp, "bcrt");
    }

    #[test]
    fn every_builtin_network_validates() {
        for name in NetworkParams::supported_networks().iter().chain(["bitcoin".to_string(), "subfrost-regtest".to_string()].iter()) {
            NetworkParams::from_network_str(name).unwrap().validate()
                .unwrap_or_else(|e| panic!("{}: {}", name, e));
        }
    }

    #[test]
    fn concurrent_native_callers() {
        let _g = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let a = NetworkParams::from_network_str("mainnet").unwrap();
        let b = NetworkParams::regtest();
        set_network(a.clone()).unwrap();
        std::thread::scope(|s| {
            for i in 0..2 {
                let (a, b) = (a.clone(), b.clone());
                s.spawn(move || {
                    for n in 0..2_000 {
                        set_network(if (n + i) % 2 == 0 { a.clone() } else { b.clone() }).unwrap();
                    }
                });
            }
            for _ in 0..4 {
                s.spawn(|| {
                    for _ in 0..10_000 {
                        let snap = get_network();
                        let consistent = (snap.bech32_hrp == "bc" && snap.p2pkh_prefix == 0x00)
                            || (snap.bech32_hrp == "bcrt" && snap.p2pkh_prefix == 0x6f);
                        assert!(consistent, "torn config: {:?}", snap);
                    }
                });
            }
        });
    }
}
