
//! Network and RPC configuration for deezel.

use crate::commands::Commands;

use bitcoin::{Network, Script};
use bech32::Hrp;
use metashrew_support::address::{AddressEncoding, Payload};

use serde::{Deserialize, Serialize};

use thiserror::Error;



#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum RpcError {
    #[error("Missing RPC URL for command: {0:?}")]
    MissingRpcUrl(Commands),
    #[error("RPC error {code}: {message}")]
    JsonRpcError { code: i64, message: String },
}

static mut _NETWORK: Option<NetworkParams> = None;

impl NetworkParams {
    pub fn mainnet() -> Self {
        Self {
            network: Network::Bitcoin,
            magic: [0xf9, 0xbe, 0xb4, 0xd9],
            default_port: 8333,
            rpc_port: 8332,
            bech32_hrp: "bc".to_string(),
            bech32_prefix: "bc".to_string(),
            p2pkh_prefix: 0,
            p2sh_prefix: 5,
        }
    }

    pub fn testnet() -> Self {
        Self {
            network: Network::Testnet,
            magic: [0x0b, 0x11, 0x09, 0x07],
            default_port: 18333,
            rpc_port: 18332,
            bech32_hrp: "tb".to_string(),
            bech32_prefix: "tb".to_string(),
            p2pkh_prefix: 111,
            p2sh_prefix: 196,
        }
    }

    pub fn signet() -> Self {
        Self {
            network: Network::Signet,
            magic: [0x0a, 0x03, 0xcf, 0x40],
            default_port: 38333,
            rpc_port: 38332,
            bech32_hrp: "tb".to_string(),
            bech32_prefix: "tb".to_string(),
            p2pkh_prefix: 111,
            p2sh_prefix: 196,
        }
    }

    pub fn regtest() -> Self {
        Self {
            network: Network::Regtest,
            magic: [0xfa, 0xbf, 0xb5, 0xda],
            default_port: 18444,
            rpc_port: 18443,
            bech32_hrp: "bcrt".to_string(),
            bech32_prefix: "bcrt".to_string(),
            p2pkh_prefix: 111,
            p2sh_prefix: 196,
        }
    }

    pub fn with_custom_magic(network: Network, p2pkh_prefix: u8, p2sh_prefix: u8, bech32_hrp: String) -> Self {
        let mut params = match network {
            Network::Bitcoin => Self::mainnet(),
            Network::Testnet => Self::testnet(),
            Network::Signet => Self::signet(),
            Network::Regtest => Self::regtest(),
            _ => Self::regtest(),
        };
        params.p2pkh_prefix = p2pkh_prefix;
        params.p2sh_prefix = p2sh_prefix;
        params.bech32_hrp = bech32_hrp;
        params
    }

    pub fn from_magic_str(s: &str) -> Result<(u8, u8, String), AlkanesError> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 3 {
            return Err(AlkanesError::InvalidParameters("Invalid magic string format".to_string()));
        }
        let p2pkh_prefix = u8::from_str_radix(parts[0].trim_start_matches("0x"), 16).map_err(|_| AlkanesError::InvalidParameters("Invalid p2pkh prefix".to_string()))?;
        let p2sh_prefix = u8::from_str_radix(parts[1].trim_start_matches("0x"), 16).map_err(|_| AlkanesError::InvalidParameters("Invalid p2sh prefix".to_string()))?;
        let bech32_hrp = parts[2].to_string();
        Ok((p2pkh_prefix, p2sh_prefix, bech32_hrp))
    }

    pub fn supported_networks() -> Vec<&'static str> {
        vec!["mainnet", "testnet", "signet", "regtest"]
    }

    pub fn from_network_str(s: &str) -> Result<Self, AlkanesError> {
        match s {
            "mainnet" => Ok(Self::mainnet()),
            "testnet" => Ok(Self::testnet()),
            "signet" => Ok(Self::signet()),
            "regtest" => Ok(Self::regtest()),
            _ => Err(AlkanesError::InvalidParameters(format!("Invalid network: {}", s))),
        }
    }
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
pub struct AlkanesNetwork(pub Network);

impl FromStr for AlkanesNetwork {
    type Err = AlkanesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(AlkanesNetwork(Network::Bitcoin)),
            "testnet" => Ok(AlkanesNetwork(Network::Testnet)),
            "signet" => Ok(AlkanesNetwork(Network::Signet)),
            "regtest" => Ok(AlkanesNetwork(Network::Regtest)),
            _ => Err(AlkanesError::InvalidParameters(format!("Invalid network: {}", s))),
        }
    }
}

impl std::fmt::Display for AlkanesNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self.0 {
            Network::Bitcoin => "mainnet",
            Network::Testnet => "testnet",
            Network::Signet => "signet",
            Network::Regtest => "regtest",
            _ => "unknown",
        })
    }
}

#[derive(Args, Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Network provider
    #[arg(short, long, default_value = "regtest")]
    pub network: AlkanesNetwork,

    /// Sandshrew RPC URL (defaults based on network if not provided)
    #[arg(long)]
    pub sandshrew_rpc_url: Option<String>,

    /// Esplora API URL (overrides Sandshrew for Esplora calls, enables REST)
    #[arg(long)]
    pub esplora_url: Option<String>,

    /// Ord API URL (overrides Sandshrew for ord calls, enables REST)
    #[arg(long)]
    pub ord_url: Option<String>,

    /// Metashrew RPC URL (overrides Sandshrew for metashrew calls)
    #[arg(long)]
    pub metashrew_rpc_url: Option<String>,

    /// Bitcoin RPC URL (overrides Sandshrew for bitcoind calls)
    #[arg(long)]
    pub bitcoin_rpc_url: Option<String>,

    /// RPC timeout in seconds
    #[arg(long, default_value = "600")]
    pub timeout_seconds: u64,
}



impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            network: AlkanesNetwork(Network::Regtest),
            sandshrew_rpc_url: Some("http://localhost:18443".to_string()),
            esplora_url: None,
            ord_url: None,
            metashrew_rpc_url: Some("http://localhost:18888".to_string()),
            bitcoin_rpc_url: None,
            timeout_seconds: 600,
        }
    }
}
