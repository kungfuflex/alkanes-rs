//! Encoding functions: JSON -> Protobuf hex strings

use super::types::*;
use alkanes_cli_common::proto::alkanes as alkanes_pb;
use alkanes_cli_common::proto::protorune as protorune_pb;
use anyhow::{Context, Result};
use prost::Message;

/// Convert u128 to protobuf uint128
fn to_uint128(v: u128) -> alkanes_pb::Uint128 {
    alkanes_pb::Uint128 {
        lo: (v & 0xFFFFFFFFFFFFFFFF) as u64,
        hi: (v >> 64) as u64,
    }
}

/// Convert u128 to protorune protobuf uint128
fn to_protorune_uint128(v: u128) -> protorune_pb::Uint128 {
    protorune_pb::Uint128 {
        lo: (v & 0xFFFFFFFFFFFFFFFF) as u64,
        hi: (v >> 64) as u64,
    }
}

/// Parse a value that can be a number or string representation of u128
fn parse_u128_value(v: &serde_json::Value) -> Result<u128> {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_u64() {
                Ok(i as u128)
            } else if let Some(i) = n.as_i64() {
                Ok(i as u128)
            } else {
                anyhow::bail!("Number too large for u128")
            }
        }
        serde_json::Value::String(s) => s.parse().context("Invalid u128 string"),
        _ => anyhow::bail!("Expected number or string for u128 value"),
    }
}

/// Encode cellpack: [target_block, target_tx, ...inputs]
fn encode_cellpack(target: &AlkaneIdJson, inputs: &[serde_json::Value]) -> Result<Vec<u8>> {
    use alkanes_support::cellpack::Cellpack;

    let (block, tx) = target.to_parts()?;

    let mut values: Vec<u128> = vec![block, tx];
    for input in inputs {
        values.push(parse_u128_value(input)?);
    }

    let cellpack = Cellpack::try_from(values)?;
    Ok(cellpack.encipher())
}

/// Convert AlkaneTransferJson to protobuf
fn to_pb_alkane_transfer(t: &AlkaneTransferJson) -> Result<alkanes_pb::AlkaneTransfer> {
    let (block, tx) = t.id.to_parts()?;
    let value: u128 = t.value.parse().context("Invalid alkane transfer value")?;

    Ok(alkanes_pb::AlkaneTransfer {
        id: Some(alkanes_pb::AlkaneId {
            block: Some(to_uint128(block)),
            tx: Some(to_uint128(tx)),
        }),
        value: Some(to_uint128(value)),
    })
}

// ============================================================================
// Simulate
// ============================================================================

pub fn encode_simulate_request(req: &SimulateRequest) -> Result<String> {
    let calldata = encode_cellpack(&req.target, &req.inputs)?;

    let alkanes: Result<Vec<_>> = req.alkanes.iter().map(to_pb_alkane_transfer).collect();

    let parcel = alkanes_pb::MessageContextParcel {
        alkanes: alkanes?,
        transaction: hex::decode(&req.transaction).unwrap_or_default(),
        block: hex::decode(&req.block).unwrap_or_default(),
        height: req.height,
        txindex: req.txindex,
        calldata,
        vout: req.vout,
        pointer: req.pointer,
        refund_pointer: req.refund_pointer,
    };

    Ok(format!("0x{}", hex::encode(parcel.encode_to_vec())))
}

// ============================================================================
// Trace
// ============================================================================

pub fn encode_trace_request(req: &TraceRequest) -> Result<String> {
    let txid = hex::decode(&req.txid).context("Invalid txid hex")?;

    let outpoint = protorune_pb::Outpoint {
        txid,
        vout: req.vout,
    };

    Ok(format!("0x{}", hex::encode(outpoint.encode_to_vec())))
}

// ============================================================================
// TraceBlock
// ============================================================================

pub fn encode_traceblock_request(req: &TraceBlockRequest) -> Result<String> {
    let request = alkanes_pb::TraceBlockRequest {
        block: req.block,
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// Bytecode
// ============================================================================

pub fn encode_bytecode_request(req: &BytecodeRequest) -> Result<String> {
    let request = alkanes_pb::BytecodeRequest {
        id: Some(alkanes_pb::AlkaneId {
            block: Some(to_uint128(req.block)),
            tx: Some(to_uint128(req.tx)),
        }),
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// Block
// ============================================================================

pub fn encode_block_request(req: &BlockRequest) -> Result<String> {
    let request = alkanes_pb::BlockRequest {
        height: req.height,
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// Inventory
// ============================================================================

pub fn encode_inventory_request(req: &InventoryRequest) -> Result<String> {
    let request = alkanes_pb::AlkaneInventoryRequest {
        id: Some(alkanes_pb::AlkaneId {
            block: Some(to_uint128(req.block)),
            tx: Some(to_uint128(req.tx)),
        }),
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// Storage
// ============================================================================

pub fn encode_storage_request(req: &StorageRequest) -> Result<String> {
    let (block, tx) = req.id.to_parts()?;

    // Path can be hex (with 0x prefix) or UTF-8 string
    let path = if req.path.starts_with("0x") {
        hex::decode(&req.path[2..]).context("Invalid path hex")?
    } else {
        req.path.as_bytes().to_vec()
    };

    let request = alkanes_pb::AlkaneStorageRequest {
        id: Some(alkanes_pb::AlkaneId {
            block: Some(to_uint128(block)),
            tx: Some(to_uint128(tx)),
        }),
        path,
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// Address Queries
// ============================================================================

fn encode_address_to_wallet(address: &str) -> Result<Vec<u8>> {
    use bitcoin::Address;
    use std::str::FromStr;

    // Try to parse as Bitcoin address
    let addr = Address::from_str(address)
        .map_err(|e| anyhow::anyhow!("Invalid address: {}", e))?
        .assume_checked();

    Ok(addr.script_pubkey().to_bytes())
}

pub fn encode_runesbyaddress_request(req: &AddressRequest) -> Result<String> {
    let wallet = encode_address_to_wallet(&req.address)?;

    let request = protorune_pb::WalletRequest {
        wallet,
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

pub fn encode_protorunesbyaddress_request(req: &ProtorunesAddressRequest) -> Result<String> {
    let wallet = encode_address_to_wallet(&req.address)?;
    let protocol_tag: u128 = req.protocol_tag.parse().context("Invalid protocol_tag")?;

    let request = protorune_pb::ProtorunesWalletRequest {
        wallet,
        protocol_tag: Some(to_protorune_uint128(protocol_tag)),
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// Height Queries
// ============================================================================

pub fn encode_runesbyheight_request(req: &HeightRequest) -> Result<String> {
    let request = protorune_pb::RunesByHeightRequest {
        height: req.height,
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

pub fn encode_protorunesbyheight_request(req: &ProtorunesHeightRequest) -> Result<String> {
    let protocol_tag: u128 = req.protocol_tag.parse().context("Invalid protocol_tag")?;

    let request = protorune_pb::ProtorunesByHeightRequest {
        height: req.height,
        protocol_tag: Some(to_protorune_uint128(protocol_tag)),
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// Outpoint Queries
// ============================================================================

pub fn encode_runesbyoutpoint_request(req: &OutpointRequest) -> Result<String> {
    let txid = hex::decode(&req.txid).context("Invalid txid hex")?;

    let request = protorune_pb::Outpoint {
        txid,
        vout: req.vout,
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

pub fn encode_protorunesbyoutpoint_request(req: &ProtorunesOutpointRequest) -> Result<String> {
    let txid = hex::decode(&req.txid).context("Invalid txid hex")?;
    let protocol_tag: u128 = req.protocol_tag.parse().context("Invalid protocol_tag")?;

    let request = protorune_pb::OutpointWithProtocol {
        txid,
        vout: req.vout,
        protocol: Some(to_protorune_uint128(protocol_tag)),
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// AlkaneId to Outpoint
// ============================================================================

pub fn encode_alkaneid_to_outpoint_request(req: &AlkaneIdToOutpointRequest) -> Result<String> {
    let request = alkanes_pb::AlkaneIdToOutpointRequest {
        id: Some(alkanes_pb::AlkaneId {
            block: Some(to_uint128(req.block)),
            tx: Some(to_uint128(req.tx)),
        }),
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// Transaction By ID
// ============================================================================

pub fn encode_transactionbyid_request(req: &TransactionByIdRequest) -> Result<String> {
    let txid = hex::decode(&req.txid).context("Invalid txid hex")?;

    // Just the raw txid bytes
    Ok(format!("0x{}", hex::encode(txid)))
}

// ============================================================================
// Runtime
// ============================================================================

pub fn encode_runtime_request(req: &RuntimeRequest) -> Result<String> {
    let protocol_tag: u128 = req.protocol_tag.parse().context("Invalid protocol_tag")?;

    let request = protorune_pb::RuntimeInput {
        protocol_tag: Some(to_protorune_uint128(protocol_tag)),
    };

    Ok(format!("0x{}", hex::encode(request.encode_to_vec())))
}

// ============================================================================
// Unwraps
// ============================================================================

/// Note: unwraps doesn't use a request message - the height is passed as block_tag
pub fn encode_unwraps_request(_req: &UnwrapsRequest) -> Result<String> {
    // Empty input - height is passed as block_tag parameter
    Ok("0x".to_string())
}
