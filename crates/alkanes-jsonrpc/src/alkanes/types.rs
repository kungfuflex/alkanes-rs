//! JSON request/response types for alkanes_* methods

use serde::{Deserialize, Serialize};

/// Alkane ID in JSON format (block:tx string or object)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AlkaneIdJson {
    /// String format: "block:tx"
    String(String),
    /// Object format: { block: number, tx: number }
    Object { block: u128, tx: u128 },
}

impl AlkaneIdJson {
    pub fn to_parts(&self) -> anyhow::Result<(u128, u128)> {
        match self {
            AlkaneIdJson::String(s) => {
                let parts: Vec<&str> = s.split(':').collect();
                if parts.len() != 2 {
                    anyhow::bail!("Invalid alkane ID format: expected 'block:tx'");
                }
                Ok((parts[0].parse()?, parts[1].parse()?))
            }
            AlkaneIdJson::Object { block, tx } => Ok((*block, *tx)),
        }
    }
}

/// Alkane transfer in JSON format
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AlkaneTransferJson {
    pub id: AlkaneIdJson,
    pub value: String, // String to handle large u128 values
}

// ============================================================================
// Simulate Request/Response
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulateRequest {
    /// Target contract ID ("block:tx" format)
    pub target: AlkaneIdJson,
    /// Input opcodes/arguments
    #[serde(default)]
    pub inputs: Vec<serde_json::Value>,
    /// Alkane transfers to include
    #[serde(default)]
    pub alkanes: Vec<AlkaneTransferJson>,
    /// Transaction hex (optional)
    #[serde(default)]
    pub transaction: String,
    /// Block height (optional, defaults to current)
    #[serde(default)]
    pub height: u64,
    /// Block hex (optional)
    #[serde(default)]
    pub block: String,
    /// Transaction index
    #[serde(default)]
    pub txindex: u32,
    /// Output vout
    #[serde(default)]
    pub vout: u32,
    /// Pointer
    #[serde(default)]
    pub pointer: u32,
    /// Refund pointer
    #[serde(default)]
    pub refund_pointer: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulateResponse {
    pub status: i32,
    pub gas_used: u64,
    pub execution: ExecutionResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionResult {
    pub alkanes: Vec<AlkaneTransferOutput>,
    pub storage: Vec<StorageSlot>,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlkaneTransferOutput {
    pub id: AlkaneIdOutput,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlkaneIdOutput {
    pub block: String,
    pub tx: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageSlot {
    pub key: String,
    pub value: String,
}

// ============================================================================
// Trace Request/Response
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct TraceRequest {
    pub txid: String,
    pub vout: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceEvent {
    pub event: String,
    pub data: serde_json::Value,
}

// ============================================================================
// TraceBlock Request/Response
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct TraceBlockRequest {
    pub block: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceBlockItem {
    pub outpoint: OutpointJson,
    pub trace: Vec<TraceEvent>,
}

// ============================================================================
// Bytecode Request/Response
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct BytecodeRequest {
    pub block: u128,
    pub tx: u128,
}

// ============================================================================
// Block Request/Response
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct BlockRequest {
    pub height: u32,
}

// ============================================================================
// Inventory Request/Response
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct InventoryRequest {
    pub block: u128,
    pub tx: u128,
}

// ============================================================================
// Storage Request/Response
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct StorageRequest {
    pub id: AlkaneIdJson,
    /// Path as hex bytes or string
    pub path: String,
}

// ============================================================================
// Address Queries
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressRequest {
    pub address: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtorunesAddressRequest {
    pub address: String,
    pub protocol_tag: String, // String to handle large u128 values
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletOutput {
    pub outpoints: Vec<OutpointResponseJson>,
    #[serde(rename = "balanceSheet")]
    pub balance_sheet: Vec<RuneBalanceJson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutpointResponseJson {
    pub outpoint: OutpointJson,
    pub balances: Vec<RuneBalanceJson>,
    pub output: Option<OutputJson>,
    pub height: u32,
    pub txindex: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutpointJson {
    pub txid: String,
    pub vout: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputJson {
    pub script: String,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuneBalanceJson {
    pub rune: RuneJson,
    pub balance: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuneJson {
    pub rune_id: RuneIdJson,
    pub name: String,
    pub divisibility: u32,
    pub spacers: u32,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuneIdJson {
    pub height: String,
    pub txindex: String,
}

// ============================================================================
// Height Queries
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct HeightRequest {
    pub height: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtorunesHeightRequest {
    pub height: u64,
    pub protocol_tag: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunesResponse {
    pub runes: Vec<RuneJson>,
}

// ============================================================================
// Outpoint Queries
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutpointRequest {
    pub txid: String,
    pub vout: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtorunesOutpointRequest {
    pub txid: String,
    pub vout: u32,
    pub protocol_tag: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutpointBalancesResponse {
    pub balances: Vec<TokenBalanceJson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenBalanceJson {
    pub token: TokenInfoJson,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenInfoJson {
    pub id: AlkaneIdOutput,
    pub name: String,
    pub symbol: String,
}

// ============================================================================
// AlkaneId to Outpoint
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct AlkaneIdToOutpointRequest {
    pub block: u128,
    pub tx: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlkaneIdToOutpointResponse {
    pub txid: String,
    pub vout: u32,
}

// ============================================================================
// Transaction By ID
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionByIdRequest {
    pub txid: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionResponse {
    pub transaction: String,
    pub height: u32,
}

// ============================================================================
// Runtime
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRequest {
    pub protocol_tag: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeResponse {
    pub balances: Vec<RuneBalanceJson>,
}

// ============================================================================
// Unwraps
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct UnwrapsRequest {
    pub block: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaymentJson {
    pub spendable: OutpointJson,
    pub output: String,
    pub fulfilled: bool,
}
