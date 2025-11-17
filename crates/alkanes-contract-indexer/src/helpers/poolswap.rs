use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::info;

use crate::db::transactions::{replace_pool_swaps, get_decoded_protostones_by_txid_vout};
use crate::db::pools::get_pool_tokens_for_pairs;
use std::collections::HashSet;

// Internal decoding struct (currently unused externally)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DecodedSwap {
    pub pool_block: String,
    pub pool_tx: String,
    pub sold_block: String,
    pub sold_tx: String,
    pub bought_block: String,
    pub bought_tx: String,
    pub sold_amount: f64,
    pub bought_amount: f64,
    pub seller_address: Option<String>,
}

pub(crate) fn hex_to_dec_u128_str(hex_str: &str) -> Result<String> {
    let s = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let val = u128::from_str_radix(s, 16).map_err(|e| anyhow!("invalid hex: {}: {}", hex_str, e))?;
    Ok(val.to_string())
}

fn u128_from_json(obj: &JsonValue) -> Option<u128> {
    match (obj.get("hi"), obj.get("lo")) {
        (Some(hi), Some(lo)) => Some(((hi.as_u64()? as u128) << 64) | (lo.as_u64()? as u128)),
        _ => None,
    }
}

fn hex_to_u128(hex_str: &str) -> Option<u128> {
    let s = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    u128::from_str_radix(s, 16).ok()
}

fn value_u128_from_json(v: &JsonValue) -> Option<u128> {
    if let Some(obj) = v.as_object() {
        if obj.contains_key("hi") && obj.contains_key("lo") {
            return u128_from_json(v);
        }
    }
    if let Some(s) = v.as_str() {
        if s.starts_with("0x") || s.chars().any(|c| matches!(c, 'a'..='f' | 'A'..='F')) {
            return hex_to_u128(s);
        }
        return s.parse::<u128>().ok();
    }
    v.as_u64().map(|x| x as u128)
}

fn calculate_token_total(alkanes: &[JsonValue], target_block: &str, target_tx: &str) -> u128 {
    let mut total: u128 = 0;
    for a in alkanes {
        if let Some(id) = a.get("id") {
            let b = id.get("block").and_then(value_u128_from_json).unwrap_or(0).to_string();
            let t = id.get("tx").and_then(value_u128_from_json).unwrap_or(0).to_string();
            if b == target_block && t == target_tx {
                if let Some(v) = a.get("value").and_then(value_u128_from_json) {
                    total = total.saturating_add(v);
                }
            }
        }
    }
    total
}

fn is_delegate_invoke(event: &JsonValue) -> bool {
    if event.get("eventType").and_then(|v| v.as_str()) != Some("invoke") { return false; }
    let data = event.get("data").and_then(|v| v.as_object()).cloned();
    if data.is_none() { return false; }
    let data = data.unwrap();
    if data.get("type").and_then(|v| v.as_str()) != Some("delegatecall") { return false; }
    let inputs = data.get("context").and_then(|v| v.get("inputs")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if inputs.is_empty() { return false; }
    if let Some(opcode_hex) = inputs.get(0).and_then(|v| v.as_str()) { // hex string
        return hex_to_dec_u128_str(opcode_hex).ok().as_deref() == Some("3");
    }
    false
}

fn find_matching_return<'a>(invoke_idx: usize, events: &'a [JsonValue], invoke_vout: i64) -> Option<&'a JsonValue> {
    for i in (invoke_idx+1)..events.len() {
        let ev = &events[i];
        if ev.get("eventType").and_then(|v| v.as_str()) == Some("return") && ev.get("vout").and_then(|v| v.as_i64()) == Some(invoke_vout) {
            return Some(ev);
        }
    }
    None
}

fn find_matching_return_strict<'a>(
    invoke_idx: usize,
    events: &'a [JsonValue],
    invoke_vout: i64,
    expected_block: &str,
    expected_tx: &str,
    allowed_amounts: &[u128],
) -> Option<&'a JsonValue> {
    for i in (invoke_idx+1)..events.len() {
        let ev = &events[i];
        if ev.get("eventType").and_then(|v| v.as_str()) != Some("return") { continue; }
        if ev.get("vout").and_then(|v| v.as_i64()) != Some(invoke_vout) { continue; }
        let alks = ev
            .get("data").and_then(|d| d.get("response")).and_then(|r| r.get("alkanes")).and_then(|a| a.as_array());
        if let Some(arr) = alks {
            for a in arr {
                let id = a.get("id");
                if let Some(id) = id {
                    let b = id.get("block").and_then(value_u128_from_json).unwrap_or(0).to_string();
                    let t = id.get("tx").and_then(value_u128_from_json).unwrap_or(0).to_string();
                    if b == expected_block && t == expected_tx {
                        if allowed_amounts.is_empty() {
                            return Some(ev);
                        }
                        if let Some(v) = a.get("value").and_then(value_u128_from_json) {
                            if allowed_amounts.iter().any(|x| *x == v) { return Some(ev); }
                        }
                    }
                }
            }
        }
    }
    None
}

pub async fn index_pool_swaps_for_block(
    db: &PgPool,
    block_height: i32,
    results: &[(String, i32, DateTime<Utc>, JsonValue, Vec<JsonValue>)]
    // (transactionId, transactionIndex, timestamp, tx_json, trace_events_json)
) -> Result<()> {
    // Preload decoded protostones for all txids in this block once
    let txids: Vec<String> = results.iter().map(|r| r.0.clone()).collect();
    let decoded_by_tx_vout = get_decoded_protostones_by_txid_vout(db, &txids).await?;
    // Collect unique pool (block, tx) pairs across all delegatecall invoke events
    let mut unique_pools: HashSet<(String, String)> = HashSet::new();
    for (_txid, _tx_idx, _timestamp, _tx_json, events) in results.iter() {
        for ev in events {
            if !is_delegate_invoke(ev) { continue; }
            let pool_block = ev.get("alkaneAddressBlock").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let pool_tx = ev.get("alkaneAddressTx").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !pool_block.is_empty() && !pool_tx.is_empty() {
                unique_pools.insert((pool_block, pool_tx));
            }
        }
    }

    // Batch fetch token pairs for all pools in this block
    let token_map = if !unique_pools.is_empty() {
        let pairs: Vec<(String, String)> = unique_pools.into_iter().collect();
        let mut dbtx = db.begin().await?;
        let m = get_pool_tokens_for_pairs(&mut dbtx, &pairs).await?;
        dbtx.commit().await?;
        m
    } else { std::collections::HashMap::new() };

    // Group by txid to delete-then-insert swaps per txid in one shot
    let txids: Vec<String> = results.iter().map(|r| r.0.clone()).collect();
    let mut swap_rows: Vec<(String, i32, i32, String, String, String, String, String, String, f64, f64, Option<String>, DateTime<Utc>, bool)> = Vec::new();

    for (txid, tx_idx, timestamp, _tx_json, events) in results {
        for (i, ev) in events.iter().enumerate() {
            if !is_delegate_invoke(ev) { continue; }
            let pool_block = ev.get("alkaneAddressBlock").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let pool_tx = ev.get("alkaneAddressTx").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if pool_block.is_empty() || pool_tx.is_empty() { continue; }

            // Lookup token pair from pre-fetched map
            let Some(((token0_block, token0_tx), (token1_block, token1_tx))) = token_map.get(&(pool_block.clone(), pool_tx.clone())).cloned() else { continue };

            // Incoming alkanes on invoke
            let incoming: Vec<JsonValue> = ev
                .get("data").and_then(|d| d.get("context")).and_then(|c| c.get("incomingAlkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();

            // Ensure incoming contains one of the pool tokens
            let has_t0_incoming = calculate_token_total(&incoming, &token0_block, &token0_tx) > 0;
            let has_t1_incoming = calculate_token_total(&incoming, &token1_block, &token1_tx) > 0;
            if !has_t0_incoming && !has_t1_incoming { continue; }

            // Extract desired amounts from inputs[1] or inputs[2] if present
            let inputs: Vec<String> = ev
                .get("data").and_then(|d| d.get("context")).and_then(|c| c.get("inputs")).and_then(|a| a.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let mut allowed_amounts: Vec<u128> = Vec::new();
            if inputs.len() > 1 { if let Some(v) = hex_to_u128(&inputs[1]) { allowed_amounts.push(v); } }
            if inputs.len() > 2 { if let Some(v) = hex_to_u128(&inputs[2]) { allowed_amounts.push(v); } }

            // Find matching return event with token expectation and amount guard
            let invoke_vout = ev.get("vout").and_then(|v| v.as_i64()).unwrap_or(-1);
            let (expected_b, expected_t) = if has_t0_incoming { (&token1_block, &token1_tx) } else { (&token0_block, &token0_tx) };
            let ret = find_matching_return_strict(i, events, invoke_vout, expected_b, expected_t, &allowed_amounts)
                .or_else(|| find_matching_return(i, events, invoke_vout));
            let success = ret.is_some();
            let outgoing: Vec<JsonValue> = ret
                .map(|r| r.get("data").and_then(|d| d.get("response")).and_then(|c| c.get("alkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default())
                .unwrap_or_default();

            let t0_in = calculate_token_total(&incoming, &token0_block, &token0_tx);
            let t0_out = calculate_token_total(&outgoing, &token0_block, &token0_tx);
            let t1_in = calculate_token_total(&incoming, &token1_block, &token1_tx);
            let t1_out = calculate_token_total(&outgoing, &token1_block, &token1_tx);

            // Decide sold/bought
            let (sold_block, sold_tx, bought_block, bought_tx, sold_amount_u128, bought_amount_u128) = if t0_out == 0 && t1_in == 0 {
                (token0_block.clone(), token0_tx.clone(), token1_block.clone(), token1_tx.clone(), t0_in, t1_out)
            } else {
                (token1_block.clone(), token1_tx.clone(), token0_block.clone(), token0_tx.clone(), t1_in, t0_out)
            };

            let amounts_valid = sold_amount_u128 > 0 && bought_amount_u128 > 0;

            // Determine sellerAddress from DecodedProtostone.pointer_destination.address for this txid+vout
            let seller_address: Option<String> = decoded_by_tx_vout
                .get(txid)
                .and_then(|by_vout| by_vout.get(&(invoke_vout as i32)))
                .and_then(|items| {
                    // Search any decoded object at this vout for pointer_destination.address
                    for (_idx, d) in items {
                        if let Some(addr) = d
                            .get("pointer_destination")
                            .and_then(|pd| pd.get("address"))
                            .and_then(|v| v.as_str())
                        {
                            return Some(addr.to_string());
                        }
                    }
                    None
                });

            // Push row; if not successful or amounts invalid, push zeros and successful=false
            swap_rows.push((
                txid.clone(),
                block_height,
                *tx_idx,
                pool_block.clone(),
                pool_tx.clone(),
                sold_block,
                sold_tx,
                bought_block,
                bought_tx,
                if success && amounts_valid { sold_amount_u128 as f64 } else { 0.0 },
                if success && amounts_valid { bought_amount_u128 as f64 } else { 0.0 },
                seller_address,
                *timestamp,
                success && amounts_valid,
            ));
        }
    }

    // Single transaction for delete+insert
    let mut dbtx = db.begin().await?;
    replace_pool_swaps(&mut dbtx, &txids, &swap_rows).await?;
    dbtx.commit().await?;
    info!(swaps = swap_rows.len(), "indexed pool swaps for block");
    Ok(())
}


