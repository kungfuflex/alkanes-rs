use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, info};
// Import from crate root (inferred_transfers is a sibling module to helpers)
use crate::inferred_transfers::{
    TraceEventContext, ProtostoneRouting,
    infer_value_transfers,
};

#[derive(Debug, Clone)]
pub struct BalanceChange {
    pub alkane_id_block: i32,
    pub alkane_id_tx: i64,
    pub amount: String,
}

#[derive(Debug, Clone)]
pub struct OutpointBalance {
    pub outpoint_txid: String,
    pub outpoint_vout: i32,
    pub address: String,
    pub changes: Vec<BalanceChange>,
}

/// Extract balance changes from a transaction's trace events
/// This function now supports inferring value destinations when no explicit
/// value_transfer events exist, by using the protostone routing rules.
pub fn extract_balance_changes(
    tx: &JsonValue,
    trace_events: &[super::protostone::TraceEventItem],
) -> Result<Vec<OutpointBalance>> {
    let mut outpoint_balances: HashMap<(String, i32), OutpointBalance> = HashMap::new();

    // Get transaction outputs for address resolution
    let outputs = tx.get("vout")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Missing vout"))?;

    let txid = tx.get("txid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing txid"))?
        .to_string();

    let num_outputs = outputs.len();

    // Check if we have explicit value_transfer events
    let has_value_transfers = trace_events.iter()
        .any(|e| e.event_type == "value_transfer");

    // Process explicit value_transfer events
    for event in trace_events {
        if event.event_type == "value_transfer" {
            process_value_transfer_event(
                &txid, event, outputs, &mut outpoint_balances
            )?;
        }
    }

    // If no value_transfer events, try to infer from receive_intent + return
    if !has_value_transfers {
        let inferred = infer_balance_changes_from_traces(
            &txid, trace_events, outputs, num_outputs
        )?;

        for ob in inferred {
            let key = (ob.outpoint_txid.clone(), ob.outpoint_vout);
            outpoint_balances.insert(key, ob);
        }
    }

    Ok(outpoint_balances.into_values().collect())
}

/// Process an explicit value_transfer event
fn process_value_transfer_event(
    txid: &str,
    event: &super::protostone::TraceEventItem,
    outputs: &[JsonValue],
    outpoint_balances: &mut HashMap<(String, i32), OutpointBalance>,
) -> Result<()> {
    let vout = event.vout;
    let data = &event.data;
    let empty_vec = vec![];
    let transfers = data.get("transfers")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);
    let redirect_to = data.get("redirect_to")
        .and_then(|v| v.as_i64())
        .unwrap_or(vout as i64) as i32;

    // Get address for target vout
    if let Some(address) = get_address_from_output(outputs, redirect_to) {
        let key = (txid.to_string(), redirect_to);
        let entry = outpoint_balances.entry(key).or_insert_with(|| {
            OutpointBalance {
                outpoint_txid: txid.to_string(),
                outpoint_vout: redirect_to,
                address: address.clone(),
                changes: Vec::new(),
            }
        });

        for transfer in transfers {
            if let Some(change) = parse_balance_change(transfer) {
                entry.changes.push(change);
            }
        }
    }

    Ok(())
}

/// Parse a single balance change from transfer JSON
fn parse_balance_change(transfer: &JsonValue) -> Option<BalanceChange> {
    let alkane_id = transfer.get("id")
        .or_else(|| transfer.get("alkaneId"))?;

    // block and tx can be strings or numbers
    let block: i32 = alkane_id.get("block")
        .and_then(|v| {
            v.as_str().and_then(|s| s.parse().ok())
                .or_else(|| v.as_i64().map(|n| n as i32))
        })?;

    let tx_num: i64 = alkane_id.get("tx")
        .and_then(|v| {
            v.as_str().and_then(|s| s.parse().ok())
                .or_else(|| v.as_i64())
        })?;

    // amount/value can be string or number
    let amount = transfer.get("value")
        .or_else(|| transfer.get("amount"))
        .and_then(|v| {
            v.as_str().map(|s| s.to_string())
                .or_else(|| v.as_u64().map(|n| n.to_string()))
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        })
        .unwrap_or_else(|| "0".to_string());

    if block > 0 {
        Some(BalanceChange {
            alkane_id_block: block,
            alkane_id_tx: tx_num,
            amount,
        })
    } else {
        None
    }
}

/// Infer balance changes when no explicit value_transfer events exist
fn infer_balance_changes_from_traces(
    txid: &str,
    trace_events: &[super::protostone::TraceEventItem],
    outputs: &[JsonValue],
    num_outputs: usize,
) -> Result<Vec<OutpointBalance>> {
    let mut result = Vec::new();

    // Convert trace events to our internal format
    let traces: Vec<TraceEventContext> = trace_events.iter()
        .map(|e| TraceEventContext {
            event_type: e.event_type.clone(),
            vout: e.vout,
            data: e.data.clone(),
        })
        .collect();

    // Find all decoded protostones from the trace events
    // We need to extract routing info from the traces themselves
    let protostone_routing = extract_protostone_routing_from_traces(&traces, num_outputs);

    if protostone_routing.is_empty() {
        debug!("No protostone routing info found, cannot infer transfers");
        return Ok(result);
    }

    // Infer value transfers
    let inferred = infer_value_transfers(&traces, &protostone_routing, num_outputs);

    if !inferred.transfers.is_empty() {
        info!(
            "Inferred {} value transfers for tx {} (no explicit value_transfer events)",
            inferred.transfers.len(), txid
        );
    }

    // Convert inferred transfers to OutpointBalance
    for transfer in inferred.transfers {
        let to_vout = transfer.to_vout as i32;

        if let Some(address) = get_address_from_output(outputs, to_vout) {
            let changes: Vec<BalanceChange> = transfer.alkanes.iter()
                .filter_map(|a| {
                    if a.block > 0 {
                        Some(BalanceChange {
                            alkane_id_block: a.block,
                            alkane_id_tx: a.tx,
                            amount: a.value.to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            if !changes.is_empty() {
                result.push(OutpointBalance {
                    outpoint_txid: txid.to_string(),
                    outpoint_vout: to_vout,
                    address,
                    changes,
                });
            }
        }
    }

    Ok(result)
}

/// Extract protostone routing info from trace events
/// This creates ProtostoneRouting entries based on the shadow vouts we see in traces
fn extract_protostone_routing_from_traces(
    traces: &[TraceEventContext],
    num_tx_outputs: usize,
) -> Vec<ProtostoneRouting> {
    let mut routing = Vec::new();
    let mut seen_shadow_vouts: HashMap<i32, bool> = HashMap::new();

    // Shadow vouts start at num_tx_outputs + 1
    let shadow_vout_start = (num_tx_outputs as i32) + 1;

    // Find unique shadow vouts from traces
    for trace in traces {
        if trace.vout >= shadow_vout_start {
            seen_shadow_vouts.insert(trace.vout, true);
        }
    }

    // For each shadow vout, try to extract routing info from invoke event
    for shadow_vout in seen_shadow_vouts.keys() {
        let protostone_index = (*shadow_vout as usize) - num_tx_outputs - 1;

        // Look for invoke event to get pointer/refund from context
        let invoke = traces.iter()
            .find(|t| t.vout == *shadow_vout && t.event_type == "invoke");

        // Extract pointer from invoke context if available
        let (pointer, refund_pointer) = if let Some(inv) = invoke {
            let context = inv.data.get("context");
            // The protostone fields might be in the decoded protostone, not the trace
            // For now, use default behavior
            (None, None)
        } else {
            (None, None)
        };

        // Calculate default output (first non-OP_RETURN)
        let default_output = 0u32; // TODO: analyze outputs to find first non-OP_RETURN

        routing.push(ProtostoneRouting {
            shadow_vout: *shadow_vout as u32,
            protostone_index,
            pointer,
            refund_pointer,
            default_output,
        });
    }

    routing
}

fn get_address_from_output(outputs: &[JsonValue], vout: i32) -> Option<String> {
    outputs.get(vout as usize)
        .and_then(|out| out.get("scriptPubKey"))
        .and_then(|spk| spk.get("address"))
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
}

/// Upsert UTXO balances into database
pub async fn upsert_utxo_balances(
    pool: &PgPool,
    block_height: i32,
    outpoint_balances: &[OutpointBalance],
) -> Result<()> {
    if outpoint_balances.is_empty() {
        return Ok(());
    }
    
    let mut tx = pool.begin().await?;
    
    for ob in outpoint_balances {
        for change in &ob.changes {
            sqlx::query(
                r#"
                insert into "AlkaneBalanceUtxo" 
                ("address", "outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx", "amount", "blockHeight")
                values ($1, $2, $3, $4, $5, $6, $7)
                on conflict ("outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx")
                do update set 
                    "amount" = excluded."amount",
                    "updatedAt" = now()
                "#
            )
            .bind(&ob.address)
            .bind(&ob.outpoint_txid)
            .bind(ob.outpoint_vout)
            .bind(change.alkane_id_block)
            .bind(change.alkane_id_tx)
            .bind(&change.amount)
            .bind(block_height)
            .execute(&mut *tx)
            .await?;
        }
    }
    
    tx.commit().await?;
    debug!("Upserted {} UTXO balances", outpoint_balances.len());
    Ok(())
}

/// Update aggregate address balances
pub async fn update_address_balances(
    pool: &PgPool,
    outpoint_balances: &[OutpointBalance],
) -> Result<()> {
    if outpoint_balances.is_empty() {
        return Ok(());
    }
    
    // Group by address and alkane
    let mut aggregates: HashMap<(String, i32, i64), i128> = HashMap::new();
    
    for ob in outpoint_balances {
        for change in &ob.changes {
            let key = (ob.address.clone(), change.alkane_id_block, change.alkane_id_tx);
            let amount: i128 = change.amount.parse().unwrap_or(0);
            *aggregates.entry(key).or_insert(0) += amount;
        }
    }
    
    let mut dbtx = pool.begin().await?;
    let agg_count = aggregates.len();
    
    for ((address, block, tx_num), amount) in aggregates.into_iter() {
        if amount != 0 {
            sqlx::query(
                r#"
                insert into "AlkaneBalance"
                ("address", "alkaneIdBlock", "alkaneIdTx", "amount")
                values ($1, $2, $3, $4)
                on conflict ("address", "alkaneIdBlock", "alkaneIdTx")
                do update set
                    "amount" = (
                        (coalesce("AlkaneBalance"."amount", '0')::numeric + $4::numeric)::text
                    ),
                    "updatedAt" = now()
                "#
            )
            .bind(&address)
            .bind(block)
            .bind(tx_num)
            .bind(amount.to_string())
            .execute(&mut *dbtx)
            .await?;
        }
    }
    
    dbtx.commit().await?;
    debug!("Updated {} address balances", agg_count);
    Ok(())
}

/// Refresh holder materialized data for modified alkanes
pub async fn refresh_holders_for_block(
    pool: &PgPool,
    outpoint_balances: &[OutpointBalance],
) -> Result<()> {
    if outpoint_balances.is_empty() {
        return Ok(());
    }
    
    // Collect unique alkanes that were modified
    let mut alkanes: HashMap<(i32, i64), ()> = HashMap::new();
    for ob in outpoint_balances {
        for change in &ob.changes {
            alkanes.insert((change.alkane_id_block, change.alkane_id_tx), ());
        }
    }
    
    for ((block, tx_num), _) in alkanes {
        refresh_holders(pool, block, tx_num).await?;
    }
    
    Ok(())
}

/// Refresh holder materialized data for an alkane
async fn refresh_holders(
    pool: &PgPool,
    alkane_id_block: i32,
    alkane_id_tx: i64,
) -> Result<()> {
    // Delete old entries
    sqlx::query(
        r#"delete from "AlkaneHolder" where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2"#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .execute(pool)
    .await?;
    
    // Insert fresh data
    sqlx::query(
        r#"
        insert into "AlkaneHolder" ("alkaneIdBlock", "alkaneIdTx", "address", "totalAmount")
        select "alkaneIdBlock", "alkaneIdTx", "address", "amount"
        from "AlkaneBalance"
        where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2
          and "amount"::numeric > 0
        "#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .execute(pool)
    .await?;
    
    // Update count
    sqlx::query(
        r#"
        insert into "AlkaneHolderCount" ("alkaneIdBlock", "alkaneIdTx", "count")
        select $1, $2, count(*)
        from "AlkaneHolder"
        where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2
        on conflict ("alkaneIdBlock", "alkaneIdTx")
        do update set "count" = excluded."count", "lastUpdated" = now()
        "#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .execute(pool)
    .await?;
    
    debug!("Refreshed holders for alkane {}:{}", alkane_id_block, alkane_id_tx);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_trace_event(
        vout: i32,
        event_type: &str,
        data: serde_json::Value,
        alkane_block: &str,
        alkane_tx: &str,
    ) -> super::super::protostone::TraceEventItem {
        super::super::protostone::TraceEventItem {
            vout,
            event_type: event_type.to_string(),
            data,
            alkane_address_block: alkane_block.to_string(),
            alkane_address_tx: alkane_tx.to_string(),
        }
    }

    #[test]
    fn test_extract_balance_changes_simple() {
        let tx_json = json!({
            "txid": "abc123def456",
            "vout": [{"scriptPubKey": {"address": "bc1ptest123"}}]
        });

        let trace_events = vec![
            create_test_trace_event(
                0, "value_transfer",
                json!({"transfers": [{"id": {"block": 840000, "tx": 123}, "amount": "1000000"}]}),
                "840000", "100"
            )
        ];

        let result = extract_balance_changes(&tx_json, &trace_events);
        assert!(result.is_ok());
        let balances = result.unwrap();
        assert_eq!(balances.len(), 1);
        assert_eq!(balances[0].address, "bc1ptest123");
    }
}
