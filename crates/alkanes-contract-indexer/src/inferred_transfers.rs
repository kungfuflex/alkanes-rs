//! Inferred Value Transfer Module
//!
//! This module infers where alkane value ends up based on protorune rules when
//! no explicit ValueTransfer event is emitted. This happens when a protocol message
//! does a ReturnContext without explicitly transferring value.
//!
//! Rules implemented (matching ./crates/protorune):
//! 1. On success (ReturnContext): value goes to protostone's `pointer` field
//! 2. If no `pointer`: value goes to `default_output` (first non-OP_RETURN output)
//! 3. On failure (RevertContext): value goes to protostone's `refund_pointer` field
//! 4. If no `refund_pointer`: value goes to `default_output`

use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;

/// Represents a decoded protostone with its routing information
#[derive(Debug, Clone)]
pub struct ProtostoneRouting {
    /// The shadow vout for this protostone (tx.output.len() + 1 + index)
    pub shadow_vout: u32,
    /// The protostone index (0-based)
    pub protostone_index: usize,
    /// Where value goes on success (pointer field from protostone)
    pub pointer: Option<u32>,
    /// Where value goes on failure (refund field from protostone)
    pub refund_pointer: Option<u32>,
    /// Default output if no pointer/refund specified
    pub default_output: u32,
}

/// Represents a trace event with additional context
#[derive(Debug, Clone)]
pub struct TraceEventContext {
    pub event_type: String,
    pub vout: i32,
    pub data: JsonValue,
}

/// Result of analyzing traces for a transaction
#[derive(Debug)]
pub struct InferredTransfers {
    /// Inferred value transfers keyed by destination vout
    pub transfers: Vec<InferredTransfer>,
}

#[derive(Debug, Clone)]
pub struct InferredTransfer {
    /// Source shadow vout (protostone)
    pub from_vout: u32,
    /// Destination physical vout
    pub to_vout: u32,
    /// Alkane transfers
    pub alkanes: Vec<AlkaneTransfer>,
}

#[derive(Debug, Clone)]
pub struct AlkaneTransfer {
    pub block: i32,
    pub tx: i64,
    pub value: u128,
}

/// Infer value transfers from trace events and protostone data
pub fn infer_value_transfers(
    traces: &[TraceEventContext],
    protostones: &[ProtostoneRouting],
    num_tx_outputs: usize,
) -> InferredTransfers {
    let mut transfers = Vec::new();

    // Group traces by vout (shadow vout)
    let mut traces_by_vout: HashMap<i32, Vec<&TraceEventContext>> = HashMap::new();
    for trace in traces {
        traces_by_vout.entry(trace.vout).or_default().push(trace);
    }

    // Check if we already have explicit value_transfer events
    let has_value_transfers = traces.iter().any(|t| t.event_type == "value_transfer");

    // If there are explicit value_transfers, we don't need to infer
    if has_value_transfers {
        tracing::debug!("infer_value_transfers: explicit value_transfer events found, skipping inference");
        return InferredTransfers { transfers };
    }

    // Process each protostone
    for routing in protostones {
        let shadow_vout = routing.shadow_vout as i32;
        let vout_traces = traces_by_vout.get(&shadow_vout).map(|v| v.as_slice()).unwrap_or(&[]);

        // Find receive_intent for this protostone (shows what came in)
        let receive_intent = vout_traces.iter()
            .find(|t| t.event_type == "receive_intent");

        // Find the exit context (return event) for this protostone
        let exit_context = vout_traces.iter()
            .find(|t| t.event_type == "return");

        // If no receive_intent, no alkanes came in, nothing to infer
        let intent = match receive_intent {
            Some(i) => i,
            None => continue,
        };

        // Parse incoming alkanes from receive_intent
        let incoming_alkanes = parse_incoming_alkanes(&intent.data);
        if incoming_alkanes.is_empty() {
            continue;
        }

        // Determine destination based on exit status
        let destination_vout = if let Some(exit) = exit_context {
            let status = exit.data.get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            match status {
                "success" => {
                    // On success, value goes to pointer (or default_output)
                    routing.pointer.unwrap_or(routing.default_output)
                }
                "failure" => {
                    // On failure, value goes to refund_pointer (or default_output)
                    routing.refund_pointer.unwrap_or(routing.default_output)
                }
                _ => {
                    tracing::warn!("Unknown exit status '{}' for protostone at vout {}", status, shadow_vout);
                    continue;
                }
            }
        } else {
            // No exit context found - this shouldn't happen for completed traces
            // Default to pointer or default_output
            tracing::warn!("No exit context for protostone at vout {}, using pointer or default", shadow_vout);
            routing.pointer.unwrap_or(routing.default_output)
        };

        // Validate destination is a physical output
        if destination_vout as usize >= num_tx_outputs {
            // Destination is another protostone (virtual output)
            // For now, we skip these as they'll be handled by that protostone's processing
            tracing::debug!(
                "Destination vout {} is virtual (num_outputs={}), skipping",
                destination_vout, num_tx_outputs
            );
            continue;
        }

        // Create inferred transfer
        let transfer = InferredTransfer {
            from_vout: routing.shadow_vout,
            to_vout: destination_vout,
            alkanes: incoming_alkanes,
        };

        tracing::info!(
            "Inferred transfer: shadow_vout {} -> physical_vout {} ({} alkanes)",
            routing.shadow_vout, destination_vout, transfer.alkanes.len()
        );

        transfers.push(transfer);
    }

    InferredTransfers { transfers }
}

/// Parse incoming alkanes from receive_intent data
fn parse_incoming_alkanes(data: &JsonValue) -> Vec<AlkaneTransfer> {
    let mut result = Vec::new();

    // Try different field names
    let transfers = data.get("transfers")
        .or_else(|| data.get("incoming_alkanes"))
        .or_else(|| data.get("incomingAlkanes"))
        .and_then(|v| v.as_array());

    if let Some(arr) = transfers {
        for item in arr {
            if let Some(transfer) = parse_single_alkane_transfer(item) {
                result.push(transfer);
            }
        }
    }

    result
}

/// Parse a single alkane transfer from JSON
fn parse_single_alkane_transfer(item: &JsonValue) -> Option<AlkaneTransfer> {
    let id_obj = item.get("id")?;

    // block and tx can be strings or numbers
    let block: i32 = id_obj.get("block")
        .and_then(|v| {
            v.as_str().and_then(|s| s.parse().ok())
                .or_else(|| v.as_i64().map(|n| n as i32))
        })?;

    let tx: i64 = id_obj.get("tx")
        .and_then(|v| {
            v.as_str().and_then(|s| s.parse().ok())
                .or_else(|| v.as_i64())
        })?;

    // value can be a string, number, or U128 object with lo/hi
    let value: u128 = item.get("value")
        .and_then(|v| {
            // Try string first
            v.as_str().and_then(|s| s.parse().ok())
                // Then try as number
                .or_else(|| v.as_u64().map(|n| n as u128))
                .or_else(|| v.as_i64().map(|n| n as u128))
                // Then try U128 format with lo/hi
                .or_else(|| {
                    let lo = v.get("lo")?.as_u64()? as u128;
                    let hi = v.get("hi").and_then(|h| h.as_u64()).unwrap_or(0) as u128;
                    Some(lo | (hi << 64))
                })
        })?;

    Some(AlkaneTransfer { block, tx, value })
}

/// Extract protostone routing info from decoded protostones JSON
pub fn extract_protostone_routing(
    decoded_protostones: &[JsonValue],
    num_tx_outputs: usize,
) -> Vec<ProtostoneRouting> {
    let mut result = Vec::new();
    let default_output = find_default_output_from_tx_outputs(num_tx_outputs);

    for (index, proto) in decoded_protostones.iter().enumerate() {
        let shadow_vout = (num_tx_outputs as u32) + 1 + (index as u32);

        // Extract pointer - can be number or string
        let pointer = proto.get("pointer")
            .and_then(|v| {
                v.as_u64().map(|n| n as u32)
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            });

        // Extract refund pointer
        let refund_pointer = proto.get("refund")
            .or_else(|| proto.get("refund_pointer"))
            .and_then(|v| {
                v.as_u64().map(|n| n as u32)
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            });

        result.push(ProtostoneRouting {
            shadow_vout,
            protostone_index: index,
            pointer,
            refund_pointer,
            default_output,
        });

        tracing::debug!(
            "Protostone {}: shadow_vout={}, pointer={:?}, refund={:?}, default={}",
            index, shadow_vout, pointer, refund_pointer, default_output
        );
    }

    result
}

/// Find the default output (first non-OP_RETURN output)
/// Since we don't have access to script_pubkeys here, we assume output 0 for now
/// The caller should provide this from transaction analysis
fn find_default_output_from_tx_outputs(num_outputs: usize) -> u32 {
    // Default to output 0 - this matches protorune's behavior when
    // all outputs are OP_RETURN or when we can't determine
    0
}

/// Convert inferred transfers to the JSON format used by value_transfer events
pub fn inferred_transfers_to_trace_events(
    inferred: &InferredTransfers,
) -> Vec<JsonValue> {
    let mut events = Vec::new();

    for transfer in &inferred.transfers {
        let alkane_transfers: Vec<JsonValue> = transfer.alkanes.iter()
            .map(|a| {
                json!({
                    "id": {
                        "block": a.block.to_string(),
                        "tx": a.tx.to_string(),
                    },
                    "value": a.value.to_string(),
                })
            })
            .collect();

        events.push(json!({
            "event": "value_transfer",
            "vout": transfer.from_vout,
            "data": {
                "transfers": alkane_transfers,
                "redirect_to": transfer.to_vout,
                "inferred": true,  // Mark as inferred, not from actual trace
            },
        }));
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_alkane_transfer_string_values() {
        let item = json!({
            "id": {
                "block": "2",
                "tx": "100"
            },
            "value": "1000000"
        });

        let transfer = parse_single_alkane_transfer(&item).unwrap();
        assert_eq!(transfer.block, 2);
        assert_eq!(transfer.tx, 100);
        assert_eq!(transfer.value, 1000000);
    }

    #[test]
    fn test_parse_single_alkane_transfer_number_values() {
        let item = json!({
            "id": {
                "block": 2,
                "tx": 100
            },
            "value": 1000000
        });

        let transfer = parse_single_alkane_transfer(&item).unwrap();
        assert_eq!(transfer.block, 2);
        assert_eq!(transfer.tx, 100);
        assert_eq!(transfer.value, 1000000);
    }

    #[test]
    fn test_infer_value_transfers_success() {
        let traces = vec![
            TraceEventContext {
                event_type: "receive_intent".to_string(),
                vout: 3, // shadow vout
                data: json!({
                    "transfers": [{
                        "id": { "block": "2", "tx": "100" },
                        "value": "5000"
                    }]
                }),
            },
            TraceEventContext {
                event_type: "return".to_string(),
                vout: 3,
                data: json!({
                    "status": "success"
                }),
            },
        ];

        let protostones = vec![
            ProtostoneRouting {
                shadow_vout: 3,
                protostone_index: 0,
                pointer: Some(1), // Value goes to vout 1 on success
                refund_pointer: Some(0),
                default_output: 0,
            },
        ];

        let result = infer_value_transfers(&traces, &protostones, 2);
        assert_eq!(result.transfers.len(), 1);
        assert_eq!(result.transfers[0].to_vout, 1);
        assert_eq!(result.transfers[0].alkanes[0].block, 2);
        assert_eq!(result.transfers[0].alkanes[0].tx, 100);
        assert_eq!(result.transfers[0].alkanes[0].value, 5000);
    }

    #[test]
    fn test_infer_value_transfers_failure() {
        let traces = vec![
            TraceEventContext {
                event_type: "receive_intent".to_string(),
                vout: 3,
                data: json!({
                    "transfers": [{
                        "id": { "block": "2", "tx": "100" },
                        "value": "5000"
                    }]
                }),
            },
            TraceEventContext {
                event_type: "return".to_string(),
                vout: 3,
                data: json!({
                    "status": "failure"
                }),
            },
        ];

        let protostones = vec![
            ProtostoneRouting {
                shadow_vout: 3,
                protostone_index: 0,
                pointer: Some(1),
                refund_pointer: Some(0), // Value goes to vout 0 on failure
                default_output: 0,
            },
        ];

        let result = infer_value_transfers(&traces, &protostones, 2);
        assert_eq!(result.transfers.len(), 1);
        assert_eq!(result.transfers[0].to_vout, 0); // Refund pointer
    }

    #[test]
    fn test_infer_value_transfers_no_pointer_uses_default() {
        let traces = vec![
            TraceEventContext {
                event_type: "receive_intent".to_string(),
                vout: 3,
                data: json!({
                    "transfers": [{
                        "id": { "block": "2", "tx": "100" },
                        "value": "5000"
                    }]
                }),
            },
            TraceEventContext {
                event_type: "return".to_string(),
                vout: 3,
                data: json!({
                    "status": "success"
                }),
            },
        ];

        let protostones = vec![
            ProtostoneRouting {
                shadow_vout: 3,
                protostone_index: 0,
                pointer: None, // No pointer specified
                refund_pointer: None,
                default_output: 0, // Uses default
            },
        ];

        let result = infer_value_transfers(&traces, &protostones, 2);
        assert_eq!(result.transfers.len(), 1);
        assert_eq!(result.transfers[0].to_vout, 0); // Default output
    }

    #[test]
    fn test_skip_when_value_transfer_exists() {
        let traces = vec![
            TraceEventContext {
                event_type: "receive_intent".to_string(),
                vout: 3,
                data: json!({
                    "transfers": [{
                        "id": { "block": "2", "tx": "100" },
                        "value": "5000"
                    }]
                }),
            },
            TraceEventContext {
                event_type: "value_transfer".to_string(), // Explicit transfer exists
                vout: 3,
                data: json!({
                    "transfers": [],
                    "redirect_to": 1
                }),
            },
        ];

        let protostones = vec![
            ProtostoneRouting {
                shadow_vout: 3,
                protostone_index: 0,
                pointer: Some(1),
                refund_pointer: Some(0),
                default_output: 0,
            },
        ];

        let result = infer_value_transfers(&traces, &protostones, 2);
        assert_eq!(result.transfers.len(), 0); // No inferred transfers when explicit exists
    }
}
