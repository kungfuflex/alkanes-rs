//! Types for bridge deposit operations.

use serde::{Deserialize, Serialize};

/// Parameters for depositing stablecoins into the subfrost vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDepositParams {
    /// Amount of stablecoin to deposit (in stablecoin's native decimals, e.g., 6 for USDC)
    pub amount: u64,
    /// Stablecoin type
    pub stablecoin: Stablecoin,
    /// EVM RPC URL (e.g., http://localhost:8545 for anvil)
    pub evm_rpc_url: String,
    /// Private key for EVM transaction (hex, 0x-prefixed)
    pub evm_private_key: String,
    /// Vault contract address (overridable; defaults per network)
    pub vault_address: Option<String>,
    /// Stablecoin token address (overridable; defaults per network)
    pub token_address: Option<String>,
    /// Raw protostones bytes (hex) to include in the payment record.
    /// The signal engine will use these to build the Bitcoin TX.
    /// If empty, a simple frUSD mint is performed.
    pub protostones_hex: Option<String>,
    /// Bitcoin TxOuts to include (value_sats, scriptPubKey_hex).
    /// These are additional outputs the user wants in the mint TX.
    pub outputs: Vec<BridgeTxOut>,
    /// Chain ID (default: auto-detect from RPC)
    pub chain_id: Option<u64>,
}

/// A Bitcoin output to include in the bridge payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTxOut {
    pub value_sats: u64,
    pub script_pubkey_hex: String,
}

/// Supported stablecoins
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stablecoin {
    USDC,
    USDT,
}

impl Stablecoin {
    pub fn decimals(&self) -> u8 {
        match self {
            Stablecoin::USDC => 6,
            Stablecoin::USDT => 6,
        }
    }
}

/// Result of a bridge deposit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDepositResult {
    /// EVM transaction hash of the deposit
    pub tx_hash: String,
    /// Payment ID from the vault (from PaymentQueued event)
    pub payment_id: Option<u64>,
    /// frUSD amount minted (in 18-decimal wei)
    pub frusd_amount: String,
    /// Net stablecoin amount after protocol fee
    pub net_amount: u64,
}

/// Default contract addresses per network
pub struct DefaultAddresses;

impl DefaultAddresses {
    pub fn usdc_address(network: &str) -> &'static str {
        match network {
            "mainnet" => "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", // Ethereum mainnet USDC
            "regtest" | "testnet" => "0x9e545e3c0baab3e08cdfd552c960a1050f373042", // Anvil deployed
            _ => "0x9e545e3c0baab3e08cdfd552c960a1050f373042",
        }
    }

    pub fn usdt_address(network: &str) -> &'static str {
        match network {
            "mainnet" => "0xdAC17F958D2ee523a2206206994597C13D831ec7", // Ethereum mainnet USDT
            "regtest" | "testnet" => "0x9e545e3c0baab3e08cdfd552c960a1050f373042", // Same mock
            _ => "0x9e545e3c0baab3e08cdfd552c960a1050f373042",
        }
    }

    pub fn usdt_vault_address(network: &str) -> &'static str {
        match network {
            "mainnet" => "0x0000000000000000000000000000000000000000", // TBD
            "regtest" | "testnet" => "0x4826533B4897376654Bb4d4AD88B7faFD0C98528",
            _ => "0x4826533B4897376654Bb4d4AD88B7faFD0C98528",
        }
    }

    pub fn usdc_vault_address(network: &str) -> &'static str {
        match network {
            "mainnet" => "0x0000000000000000000000000000000000000000", // TBD
            "regtest" | "testnet" => "0x70e0bA845a1A0F2DA3359C97E0285013525FFC49",
            _ => "0x70e0bA845a1A0F2DA3359C97E0285013525FFC49",
        }
    }
}
