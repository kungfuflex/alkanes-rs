
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




use crate::DeezelError;
use clap::Args;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeezelNetwork(pub Network);

impl FromStr for DeezelNetwork {
    type Err = DeezelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(DeezelNetwork(Network::Bitcoin)),
            "testnet" => Ok(DeezelNetwork(Network::Testnet)),
            "signet" => Ok(DeezelNetwork(Network::Signet)),
            "regtest" => Ok(DeezelNetwork(Network::Regtest)),
            _ => Err(DeezelError::InvalidParameters(format!("Invalid network: {}", s))),
        }
    }
}

#[derive(Args, Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Network provider
    #[arg(short, long, default_value = "regtest")]
    pub network: DeezelNetwork,

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
            network: DeezelNetwork(Network::Regtest),
            sandshrew_rpc_url: Some("http://localhost:18443".to_string()),
            esplora_url: None,
            ord_url: None,
            metashrew_rpc_url: Some("http://localhost:18888".to_string()),
            bitcoin_rpc_url: None,
            timeout_seconds: 600,
        }
    }
}
