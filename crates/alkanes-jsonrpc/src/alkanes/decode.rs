//! Decoding functions: Protobuf hex strings -> JSON

use super::types::*;
use alkanes_cli_common::proto::alkanes as alkanes_pb;
use alkanes_cli_common::proto::protorune as protorune_pb;
use anyhow::{Context, Result};
use prost::Message;

/// Strip 0x prefix if present
fn strip_hex_prefix(s: &str) -> &str {
    s.strip_prefix("0x").unwrap_or(s)
}

/// Decode hex string to bytes
fn decode_hex(hex: &str) -> Result<Vec<u8>> {
    hex::decode(strip_hex_prefix(hex)).context("Invalid hex string")
}

/// Convert protobuf uint128 to string
fn from_uint128(v: Option<alkanes_pb::Uint128>) -> String {
    match v {
        Some(u) => {
            let value = (u.hi as u128) << 64 | (u.lo as u128);
            value.to_string()
        }
        None => "0".to_string(),
    }
}

/// Convert protorune uint128 to string
fn from_protorune_uint128(v: Option<protorune_pb::Uint128>) -> String {
    match v {
        Some(u) => {
            let value = (u.hi as u128) << 64 | (u.lo as u128);
            value.to_string()
        }
        None => "0".to_string(),
    }
}

/// Format storage key as human-readable path or hex
fn format_storage_key(key: &[u8]) -> String {
    const SEP: u8 = b'/';

    let parts: Vec<Vec<u8>> = key.split(|&b| b == SEP).map(|s| s.to_vec()).collect();

    let formatted: Vec<String> = parts
        .into_iter()
        .map(|part| {
            // Try to interpret as UTF-8, otherwise hex
            if let Ok(s) = std::str::from_utf8(&part) {
                if s.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return s.to_string();
                }
            }
            hex::encode(&part)
        })
        .collect();

    formatted.join("/")
}

// ============================================================================
// Simulate
// ============================================================================

pub fn decode_simulate_response(hex: &str) -> Result<SimulateResponse> {
    let bytes = decode_hex(hex)?;
    let resp = alkanes_pb::SimulateResponse::decode(bytes.as_slice())
        .context("Failed to decode SimulateResponse")?;

    let (status, execution) = if !resp.error.is_empty() {
        (
            1, // REVERT
            ExecutionResult {
                alkanes: vec![],
                storage: vec![],
                data: "0x".to_string(),
                error: Some(resp.error),
            },
        )
    } else {
        let exec = resp.execution.unwrap_or_default();
        (
            0, // SUCCESS
            ExecutionResult {
                alkanes: exec
                    .alkanes
                    .into_iter()
                    .map(|t| {
                        let id = t.id.unwrap_or_default();
                        AlkaneTransferOutput {
                            id: AlkaneIdOutput {
                                block: from_uint128(id.block),
                                tx: from_uint128(id.tx),
                            },
                            value: from_uint128(t.value),
                        }
                    })
                    .collect(),
                storage: exec
                    .storage
                    .into_iter()
                    .map(|s| StorageSlot {
                        key: format_storage_key(&s.key),
                        value: format!("0x{}", hex::encode(&s.value)),
                    })
                    .collect(),
                data: format!("0x{}", hex::encode(&exec.data)),
                error: None,
            },
        )
    };

    Ok(SimulateResponse {
        status,
        gas_used: resp.gas_used,
        execution,
    })
}

// ============================================================================
// Meta
// ============================================================================

pub fn decode_meta_response(hex: &str) -> Result<serde_json::Value> {
    let bytes = decode_hex(hex)?;

    if bytes.is_empty() {
        return Ok(serde_json::Value::Null);
    }

    // Meta response is UTF-8 JSON string
    let json_str =
        std::str::from_utf8(&bytes).context("Meta response is not valid UTF-8")?;

    serde_json::from_str(json_str).context("Meta response is not valid JSON")
}

// ============================================================================
// Trace
// ============================================================================

fn decode_call_type(call_type: i32) -> &'static str {
    match call_type {
        1 => "call",
        2 => "delegatecall",
        3 => "staticcall",
        _ => "unknowncall",
    }
}

fn decode_trace_event(event: alkanes_pb::AlkanesTraceEvent) -> TraceEvent {
    use alkanes_pb::alkanes_trace_event::Event;

    match event.event {
        Some(Event::EnterContext(enter)) => {
            let ctx = enter.context.unwrap_or_default();
            let inner = ctx.inner.unwrap_or_default();

            TraceEvent {
                event: "invoke".to_string(),
                data: serde_json::json!({
                    "type": decode_call_type(enter.call_type),
                    "context": {
                        "myself": {
                            "block": from_uint128(inner.myself.as_ref().and_then(|m| m.block.clone())),
                            "tx": from_uint128(inner.myself.as_ref().and_then(|m| m.tx.clone())),
                        },
                        "caller": {
                            "block": from_uint128(inner.caller.as_ref().and_then(|c| c.block.clone())),
                            "tx": from_uint128(inner.caller.as_ref().and_then(|c| c.tx.clone())),
                        },
                        "inputs": inner.inputs.iter().map(|i| from_uint128(Some(i.clone()))).collect::<Vec<_>>(),
                        "incomingAlkanes": inner.incoming_alkanes.iter().map(|t| {
                            let id = t.id.clone().unwrap_or_default();
                            serde_json::json!({
                                "id": {
                                    "block": from_uint128(id.block),
                                    "tx": from_uint128(id.tx),
                                },
                                "value": from_uint128(t.value.clone()),
                            })
                        }).collect::<Vec<_>>(),
                        "vout": inner.vout,
                    },
                    "fuel": ctx.fuel,
                }),
            }
        }
        Some(Event::ExitContext(exit)) => {
            let resp = exit.response.unwrap_or_default();

            TraceEvent {
                event: "return".to_string(),
                data: serde_json::json!({
                    "status": if exit.status == 0 { "success" } else { "revert" },
                    "response": {
                        "alkanes": resp.alkanes.iter().map(|t| {
                            let id = t.id.clone().unwrap_or_default();
                            serde_json::json!({
                                "id": {
                                    "block": from_uint128(id.block),
                                    "tx": from_uint128(id.tx),
                                },
                                "value": from_uint128(t.value.clone()),
                            })
                        }).collect::<Vec<_>>(),
                        "storage": resp.storage.iter().map(|s| {
                            serde_json::json!({
                                "key": format_storage_key(&s.key),
                                "value": format!("0x{}", hex::encode(&s.value)),
                            })
                        }).collect::<Vec<_>>(),
                        "data": format!("0x{}", hex::encode(&resp.data)),
                    },
                }),
            }
        }
        Some(Event::CreateAlkane(create)) => {
            let id = create.new_alkane.unwrap_or_default();

            TraceEvent {
                event: "create".to_string(),
                data: serde_json::json!({
                    "block": from_uint128(id.block),
                    "tx": from_uint128(id.tx),
                }),
            }
        }
        Some(Event::ReceiveIntent(intent)) => TraceEvent {
            event: "receiveIntent".to_string(),
            data: serde_json::json!({
                "incomingAlkanes": intent.incoming_alkanes.iter().map(|t| {
                    let id = t.id.clone().unwrap_or_default();
                    serde_json::json!({
                        "id": {
                            "block": from_uint128(id.block),
                            "tx": from_uint128(id.tx),
                        },
                        "value": from_uint128(t.value.clone()),
                    })
                }).collect::<Vec<_>>(),
            }),
        },
        Some(Event::ValueTransfer(transfer)) => TraceEvent {
            event: "valueTransfer".to_string(),
            data: serde_json::json!({
                "transfers": transfer.transfers.iter().map(|t| {
                    let id = t.id.clone().unwrap_or_default();
                    serde_json::json!({
                        "id": {
                            "block": from_uint128(id.block),
                            "tx": from_uint128(id.tx),
                        },
                        "value": from_uint128(t.value.clone()),
                    })
                }).collect::<Vec<_>>(),
                "redirectTo": transfer.redirect_to,
            }),
        },
        None => TraceEvent {
            event: "unknown".to_string(),
            data: serde_json::Value::Null,
        },
    }
}

pub fn decode_trace_response(hex: &str) -> Result<Vec<TraceEvent>> {
    let bytes = decode_hex(hex)?;
    let trace = alkanes_pb::AlkanesTrace::decode(bytes.as_slice())
        .context("Failed to decode AlkanesTrace")?;

    Ok(trace.events.into_iter().map(decode_trace_event).collect())
}

// ============================================================================
// TraceBlock
// ============================================================================

pub fn decode_traceblock_response(hex: &str) -> Result<Vec<TraceBlockItem>> {
    let bytes = decode_hex(hex)?;
    let resp = alkanes_pb::TraceBlockResponse::decode(bytes.as_slice())
        .context("Failed to decode TraceBlockResponse")?;

    Ok(resp
        .traces
        .into_iter()
        .map(|t| {
            let outpoint = t.outpoint.unwrap_or_default();
            let trace = t.trace.unwrap_or_default();

            TraceBlockItem {
                outpoint: OutpointJson {
                    txid: hex::encode(&outpoint.txid),
                    vout: outpoint.vout,
                },
                trace: trace.events.into_iter().map(decode_trace_event).collect(),
            }
        })
        .collect())
}

// ============================================================================
// Bytecode
// ============================================================================

pub fn decode_bytecode_response(hex: &str) -> Result<String> {
    // Bytecode response is just raw hex bytes
    Ok(hex.to_string())
}

// ============================================================================
// Block
// ============================================================================

pub fn decode_block_response(hex: &str) -> Result<String> {
    let bytes = decode_hex(hex)?;
    let resp = alkanes_pb::BlockResponse::decode(bytes.as_slice())
        .context("Failed to decode BlockResponse")?;

    Ok(format!("0x{}", hex::encode(&resp.block)))
}

// ============================================================================
// Inventory
// ============================================================================

pub fn decode_inventory_response(hex: &str) -> Result<Vec<AlkaneTransferOutput>> {
    let bytes = decode_hex(hex)?;
    let resp = alkanes_pb::AlkaneInventoryResponse::decode(bytes.as_slice())
        .context("Failed to decode AlkaneInventoryResponse")?;

    Ok(resp
        .alkanes
        .into_iter()
        .map(|t| {
            let id = t.id.unwrap_or_default();
            AlkaneTransferOutput {
                id: AlkaneIdOutput {
                    block: from_uint128(id.block),
                    tx: from_uint128(id.tx),
                },
                value: from_uint128(t.value),
            }
        })
        .collect())
}

// ============================================================================
// Storage
// ============================================================================

pub fn decode_storage_response(hex: &str) -> Result<String> {
    let bytes = decode_hex(hex)?;
    let resp = alkanes_pb::AlkaneStorageResponse::decode(bytes.as_slice())
        .context("Failed to decode AlkaneStorageResponse")?;

    Ok(format!("0x{}", hex::encode(&resp.value)))
}

// ============================================================================
// Address Queries (Wallet Response)
// ============================================================================

fn decode_rune_json(rune: &protorune_pb::Rune) -> RuneJson {
    let rune_id = rune.rune_id.clone().unwrap_or_default();

    RuneJson {
        rune_id: RuneIdJson {
            height: from_protorune_uint128(rune_id.height),
            txindex: from_protorune_uint128(rune_id.txindex),
        },
        name: rune.name.clone(),
        divisibility: rune.divisibility,
        spacers: rune.spacers,
        symbol: rune.symbol.clone(),
    }
}

fn decode_balance_sheet(sheet: &protorune_pb::BalanceSheet) -> Vec<RuneBalanceJson> {
    sheet
        .entries
        .iter()
        .map(|entry| RuneBalanceJson {
            rune: decode_rune_json(entry.rune.as_ref().unwrap_or(&protorune_pb::Rune::default())),
            balance: from_protorune_uint128(entry.balance.clone()),
        })
        .collect()
}

fn decode_outpoint_response(resp: &protorune_pb::OutpointResponse) -> OutpointResponseJson {
    let outpoint = resp.outpoint.clone().unwrap_or_default();
    let output = resp.output.clone();
    let balances = resp.balances.clone().unwrap_or_default();

    OutpointResponseJson {
        outpoint: OutpointJson {
            txid: hex::encode(&outpoint.txid),
            vout: outpoint.vout,
        },
        balances: decode_balance_sheet(&balances),
        output: output.map(|o| OutputJson {
            script: hex::encode(&o.script),
            value: o.value,
        }),
        height: resp.height,
        txindex: resp.txindex,
    }
}

pub fn decode_wallet_response(hex: &str) -> Result<WalletOutput> {
    let bytes = decode_hex(hex)?;
    let resp = protorune_pb::WalletResponse::decode(bytes.as_slice())
        .context("Failed to decode WalletResponse")?;

    let balance_sheet = resp.balances.unwrap_or_default();

    Ok(WalletOutput {
        outpoints: resp.outpoints.iter().map(decode_outpoint_response).collect(),
        balance_sheet: decode_balance_sheet(&balance_sheet),
    })
}

// ============================================================================
// Height Queries (Runes Response)
// ============================================================================

pub fn decode_runes_response(hex: &str) -> Result<RunesResponse> {
    let bytes = decode_hex(hex)?;
    let resp = protorune_pb::RunesResponse::decode(bytes.as_slice())
        .context("Failed to decode RunesResponse")?;

    Ok(RunesResponse {
        runes: resp.runes.iter().map(decode_rune_json).collect(),
    })
}

// ============================================================================
// Outpoint Queries
// ============================================================================

pub fn decode_outpoint_balances_response(hex: &str) -> Result<OutpointBalancesResponse> {
    let bytes = decode_hex(hex)?;
    let resp = protorune_pb::OutpointResponse::decode(bytes.as_slice())
        .context("Failed to decode OutpointResponse")?;

    let balances = resp.balances.unwrap_or_default();

    Ok(OutpointBalancesResponse {
        balances: balances
            .entries
            .iter()
            .map(|entry| {
                let rune = entry.rune.clone().unwrap_or_default();
                let rune_id = rune.rune_id.unwrap_or_default();

                TokenBalanceJson {
                    token: TokenInfoJson {
                        id: AlkaneIdOutput {
                            block: from_protorune_uint128(rune_id.height),
                            tx: from_protorune_uint128(rune_id.txindex),
                        },
                        name: rune.name,
                        symbol: rune.symbol,
                    },
                    value: from_protorune_uint128(entry.balance.clone()),
                }
            })
            .collect(),
    })
}

// ============================================================================
// AlkaneId to Outpoint
// ============================================================================

pub fn decode_alkaneid_to_outpoint_response(hex: &str) -> Result<AlkaneIdToOutpointResponse> {
    let bytes = decode_hex(hex)?;
    let resp = alkanes_pb::AlkaneIdToOutpointResponse::decode(bytes.as_slice())
        .context("Failed to decode AlkaneIdToOutpointResponse")?;

    Ok(AlkaneIdToOutpointResponse {
        txid: hex::encode(&resp.txid),
        vout: resp.vout,
    })
}

// ============================================================================
// Transaction By ID
// ============================================================================

pub fn decode_transaction_response(hex: &str) -> Result<TransactionResponse> {
    let bytes = decode_hex(hex)?;

    // The response format needs to match protowallet.decodeTransactionResult
    // This decodes a TransactionRecord protobuf
    // For now, we'll return the raw transaction hex and height 0
    // TODO: Implement proper TransactionRecord decoding

    Ok(TransactionResponse {
        transaction: format!("0x{}", hex::encode(&bytes)),
        height: 0,
    })
}

// ============================================================================
// Runtime
// ============================================================================

pub fn decode_runtime_response(hex: &str) -> Result<RuntimeResponse> {
    let bytes = decode_hex(hex)?;
    let resp = protorune_pb::Runtime::decode(bytes.as_slice())
        .context("Failed to decode Runtime")?;

    let balances = resp.balances.unwrap_or_default();

    Ok(RuntimeResponse {
        balances: decode_balance_sheet(&balances),
    })
}

// ============================================================================
// Unwraps
// ============================================================================

pub fn decode_unwraps_response(hex: &str) -> Result<Vec<PaymentJson>> {
    let bytes = decode_hex(hex)?;

    if bytes.is_empty() {
        return Ok(vec![]);
    }

    let resp = alkanes_pb::PendingUnwrapsResponse::decode(bytes.as_slice())
        .context("Failed to decode PendingUnwrapsResponse")?;

    Ok(resp
        .payments
        .into_iter()
        .map(|p| {
            let spendable = p.spendable.unwrap_or_default();

            PaymentJson {
                spendable: OutpointJson {
                    txid: hex::encode(&spendable.txid),
                    vout: spendable.vout,
                },
                output: hex::encode(&p.output),
                fulfilled: p.fulfilled,
            }
        })
        .collect())
}
