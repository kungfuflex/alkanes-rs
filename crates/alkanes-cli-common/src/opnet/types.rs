use serde::{Deserialize, Serialize};

/// Block information returned by opshrew getblockbynumber/getblockbyhash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpnetBlockInfo {
    pub height: u32,
    pub hash: String,
    pub timestamp: u64,
    pub tx_count: u32,
    pub opnet_tx_count: u32,
    pub gas_used: u64,
}

/// Transaction info returned by opshrew gettransactionbyhash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpnetTxInfo {
    pub height: u32,
    pub tx_index: u32,
    pub tx_type: OpnetTxType,
    pub contract_address: String,
}

/// OP_NET transaction type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OpnetTxType {
    Generic,
    Deployment,
    Interaction,
}

impl From<u8> for OpnetTxType {
    fn from(v: u8) -> Self {
        match v {
            1 => OpnetTxType::Deployment,
            2 => OpnetTxType::Interaction,
            _ => OpnetTxType::Generic,
        }
    }
}

/// Transaction receipt returned by opshrew gettransactionreceipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpnetReceipt {
    pub success: bool,
    pub gas_used: u64,
    pub exit_data: Vec<u8>,
    pub event_count: u32,
}

/// Gas information returned by opshrew gas view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpnetGasInfo {
    pub height: u32,
    pub gas_used: u64,
    pub block_hash: String,
}
