
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
static mut _NETWORK: Option<NetworkParams> = None;

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
    pub bitcoin_rpc_url: String,
    pub metashrew_rpc_url: String,
    pub esplora_url: Option<String>,
}

#[allow(static_mut_refs)]
pub fn set_network(params: NetworkParams) {
    unsafe {
        _NETWORK = Some(params);
    }
}

#[allow(static_mut_refs)]
pub fn get_network() -> &'static NetworkParams {
    unsafe { _NETWORK.as_ref().unwrap() }
}

#[allow(static_mut_refs)]
pub fn get_network_option() -> Option<&'static NetworkParams> {
    unsafe { _NETWORK.as_ref().clone() }
}

pub fn to_address_str(script: &Script) -> Result<String, anyhow::Error> {
    let config = get_network();
    Ok(AddressEncoding {
        p2pkh_prefix: config.p2pkh_prefix,
        p2sh_prefix: config.p2sh_prefix,
        hrp: Hrp::parse_unchecked(&config.bech32_hrp),
        payload: &Payload::from_script(script)?,
    }
    .to_string())
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

    /// Sandshrew RPC URL (defaults based on network if not provided)
    #[arg(long)]
    pub sandshrew_rpc_url: Option<String>,

    /// Titan API URL (alternative to sandshrew_rpc_url, uses REST API)
    #[arg(long)]
    pub titan_api_url: Option<String>,

    /// Esplora API URL (overrides Sandshrew for Esplora calls, enables REST)
    #[arg(long)]
    pub esplora_url: Option<String>,

    /// Ord API URL (overrides Sandshrew for ord calls, enables REST)
    #[arg(long)]
    pub ord_url: Option<String>,

    /// Metashrew RPC URL (overrides Sandshrew for metashrew calls)
    #[arg(long)]
    pub metashrew_rpc_url: Option<String>,

    /// RPC timeout in seconds
    #[arg(long, default_value = "600")]
    pub timeout_seconds: u64,
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
    /// Validate that only one backend is configured (sandshrew_rpc_url OR titan_api_url)
    pub fn validate(&self) -> Result<(), AlkanesError> {
        if self.sandshrew_rpc_url.is_some() && self.titan_api_url.is_some() {
            return Err(AlkanesError::Configuration(
                "Cannot specify both --sandshrew-rpc-url and --titan-api-url. Please choose one backend.".to_string()
            ));
        }
        Ok(())
    }

    /// Returns true if using Titan REST API as backend
    pub fn using_titan_api(&self) -> bool {
        self.titan_api_url.is_some()
    }
    
    /// Get default Sandshrew RPC URL for the network
    fn get_default_sandshrew_url(&self) -> String {
        match self.provider.as_str() {
            "mainnet" => "https://mainnet.sandshrew.io/v2/lasereyes".to_string(),
            "testnet" => "https://testnet.sandshrew.io/v2/lasereyes".to_string(),
            "signet" => "https://signet.sandshrew.io/v2/lasereyes".to_string(),
            _ => "http://localhost:18888".to_string(), // regtest
        }
    }
    
    /// Get the RPC target for Bitcoin Core operations
    /// Priority: bitcoin_rpc_url > sandshrew_rpc_url (JSONRPC translation) > default
    pub fn get_bitcoin_rpc_target(&self) -> RpcTarget {
        if let Some(ref url) = self.bitcoin_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else if let Some(ref url) = self.sandshrew_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_sandshrew_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get the RPC target for Metashrew operations (alkanes.wasm view functions)
    /// Priority: metashrew_rpc_url > sandshrew_rpc_url > default sandshrew
    pub fn get_metashrew_rpc_target(&self) -> RpcTarget {
        if let Some(ref url) = self.metashrew_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else if let Some(ref url) = self.sandshrew_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_sandshrew_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get the RPC target for Esplora operations
    /// Priority: esplora_url (REST) > sandshrew_rpc_url (JSONRPC translation) > default sandshrew
    pub fn get_esplora_rpc_target(&self) -> RpcTarget {
        if let Some(ref url) = self.esplora_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::Rest,
            }
        } else if let Some(ref url) = self.sandshrew_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_sandshrew_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get the RPC target for Ord operations
    /// Priority: ord_url (REST) > sandshrew_rpc_url (JSONRPC translation) > default sandshrew
    pub fn get_ord_rpc_target(&self) -> RpcTarget {
        if let Some(ref url) = self.ord_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::Rest,
            }
        } else if let Some(ref url) = self.sandshrew_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_sandshrew_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get the RPC target for Alkanes operations (view functions, protorunes, etc.)
    /// Priority: titan_api_url (REST) > sandshrew_rpc_url (JSONRPC) > default sandshrew
    pub fn get_alkanes_rpc_target(&self) -> RpcTarget {
        if let Some(ref url) = self.titan_api_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::Rest,
            }
        } else if let Some(ref url) = self.sandshrew_rpc_url {
            RpcTarget {
                url: url.clone(),
                backend_type: RpcBackendType::JsonRpc,
            }
        } else {
            RpcTarget {
                url: self.get_default_sandshrew_url(),
                backend_type: RpcBackendType::JsonRpc,
            }
        }
    }
    
    /// Get the RPC target for Wallet operations (used by alkanes execute)
    /// Priority: titan_api_url (REST) > sandshrew_rpc_url (JSONRPC) > default sandshrew
    /// Note: Wallet operations like send use esplora/bitcoin backends separately
    pub fn get_wallet_rpc_target(&self) -> RpcTarget {
        self.get_alkanes_rpc_target()
    }
}



impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            provider: "regtest".to_string(),
            bitcoin_rpc_url: None,
            sandshrew_rpc_url: Some("http://localhost:18443".to_string()),
            titan_api_url: None,
            esplora_url: None,
            ord_url: None,
            metashrew_rpc_url: Some("http://localhost:18888".to_string()),
            timeout_seconds: 600,
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
            bitcoin_rpc_url: "http://localhost:18443".to_string(),
            metashrew_rpc_url: "http://localhost:18888".to_string(),
            esplora_url: None,
        }
    }

    pub fn from_network_str(network: &str) -> Result<Self, AlkanesError> {
        match network {
            "regtest" => Ok(Self::regtest()),
            "mainnet" => Ok(Self {
                network: Network::Bitcoin,
                magic: [0xf9, 0xbe, 0xb4, 0xd9],
                default_port: 8333,
                rpc_port: 8332,
                bech32_hrp: "bc".to_string(),
                bech32_prefix: "bc".to_string(),
                p2pkh_prefix: 0x00,
                p2sh_prefix: 0x05,
                bitcoin_rpc_url: "http://localhost:8332".to_string(),
                metashrew_rpc_url: "http://localhost:8888".to_string(),
                esplora_url: None,
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
                bitcoin_rpc_url: "http://localhost:18332".to_string(),
                metashrew_rpc_url: "http://localhost:18888".to_string(),
                esplora_url: None,
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
                bitcoin_rpc_url: "http://localhost:38332".to_string(),
                metashrew_rpc_url: "http://localhost:18888".to_string(),
                esplora_url: None,
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
        vec!["mainnet".to_string(), "testnet".to_string(), "signet".to_string(), "regtest".to_string()]
    }
}
