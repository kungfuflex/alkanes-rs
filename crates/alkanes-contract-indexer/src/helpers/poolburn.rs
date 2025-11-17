use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::info;

use crate::db::pools::get_pool_tokens_for_pairs;
use crate::db::transactions::{get_decoded_protostones_by_txid_vout, replace_pool_burns};

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

fn is_delegate_invoke_burn(event: &JsonValue) -> bool {
    if event.get("eventType").and_then(|v| v.as_str()) != Some("invoke") { return false; }
    let data = event.get("data").and_then(|v| v.as_object()).cloned();
    if data.is_none() { return false; }
    let data = data.unwrap();
    if data.get("type").and_then(|v| v.as_str()) != Some("delegatecall") { return false; }
    let inputs = data.get("context").and_then(|v| v.get("inputs")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if inputs.is_empty() { return false; }
    if let Some(opcode_hex) = inputs.get(0).and_then(|v| v.as_str()) {
        // opcode 0x2 indicates burn (remove liquidity)
        return hex_to_u128(opcode_hex) == Some(2);
    }
    false
}

pub async fn index_pool_burns_for_block(
    db: &PgPool,
    block_height: i32,
    results: &[(String, i32, DateTime<Utc>, JsonValue, Vec<JsonValue>)]
    // (transactionId, transactionIndex, timestamp, tx_json, trace_events_json)
) -> Result<()> {
    let txids: Vec<String> = results.iter().map(|r| r.0.clone()).collect();
    let decoded_by_tx_vout = get_decoded_protostones_by_txid_vout(db, &txids).await?;

    // Collect unique pool ids across candidate events
    let mut unique_pools: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for (_txid, _tx_idx, _timestamp, _tx_json, events) in results.iter() {
        for ev in events {
            if !is_delegate_invoke_burn(ev) { continue; }
            let pool_block = ev.get("alkaneAddressBlock").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let pool_tx = ev.get("alkaneAddressTx").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !pool_block.is_empty() && !pool_tx.is_empty() {
                unique_pools.insert((pool_block, pool_tx));
            }
        }
    }

    // Batch fetch token pairs for pools in this block
    let token_map = if !unique_pools.is_empty() {
        let pairs: Vec<(String, String)> = unique_pools.into_iter().collect();
        let mut dbtx = db.begin().await?;
        let m = get_pool_tokens_for_pairs(&mut dbtx, &pairs).await?;
        dbtx.commit().await?;
        m
    } else { std::collections::HashMap::new() };

    // Gather burn rows across all txs
    let mut burn_rows: Vec<(
        String, i32, i32, String, String, String, String, String, String, String, String, String, Option<String>, DateTime<Utc>, bool
    )> = Vec::new();

    for (txid, tx_idx, timestamp, _tx_json, events) in results.iter() {
        // Ensure deterministic ordering: by vout asc, invoke before return
        let mut ordered: Vec<(usize, JsonValue)> = events.iter().cloned().enumerate().collect();
        ordered.sort_by(|a, b| {
            let av = a.1.get("vout").and_then(|v| v.as_i64()).unwrap_or(i64::MIN);
            let bv = b.1.get("vout").and_then(|v| v.as_i64()).unwrap_or(i64::MIN);
            if av != bv { return av.cmp(&bv); }
            let ae = a.1.get("eventType").and_then(|v| v.as_str()).unwrap_or("");
            let be = b.1.get("eventType").and_then(|v| v.as_str()).unwrap_or("");
            let aw = if ae == "invoke" { 0 } else if ae == "return" { 1 } else { 2 };
            let bw = if be == "invoke" { 0 } else if be == "return" { 1 } else { 2 };
            aw.cmp(&bw)
        });
        let ordered_events: Vec<JsonValue> = ordered.iter().map(|t| t.1.clone()).collect();

        for (i, ev) in ordered.iter().enumerate() {
            let ev = &ev.1;
            if ev.get("eventType").and_then(|v| v.as_str()) != Some("invoke") { continue; }
            if !is_delegate_invoke_burn(ev) { continue; }

            let pool_block = ev.get("alkaneAddressBlock").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let pool_tx = ev.get("alkaneAddressTx").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if pool_block.is_empty() || pool_tx.is_empty() { continue; }

            // Lookup token pair
            let Some(((token0_block, token0_tx), (token1_block, token1_tx))) = token_map.get(&(pool_block.clone(), pool_tx.clone())).cloned() else { continue };

            let invoke_vout = ev.get("vout").and_then(|v| v.as_i64()).unwrap_or(-1);

            // Incoming alkanes on invoke
            let incoming: Vec<JsonValue> = ev
                .get("data").and_then(|d| d.get("context")).and_then(|c| c.get("incomingAlkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();

            let incoming_lp = calculate_token_total(&incoming, &pool_block, &pool_tx);

            // Pick matching return where tokens net-positive out and LP net burned
            let mut chosen_idx: Option<usize> = None;
            let mut best_score: (u128, usize) = (u128::MAX, 0);
            for j in (i+1)..ordered_events.len() {
                let evr = &ordered_events[j];
                if evr.get("eventType").and_then(|v| v.as_str()) != Some("return") { continue; }
                if evr.get("vout").and_then(|v| v.as_i64()) != Some(invoke_vout) { continue; }
                let outgoing: Vec<JsonValue> = evr
                    .get("data").and_then(|d| d.get("response")).and_then(|c| c.get("alkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();

                let t0_in_total = calculate_token_total(&incoming, &token0_block, &token0_tx);
                let t1_in_total = calculate_token_total(&incoming, &token1_block, &token1_tx);
                let t0_out_total = calculate_token_total(&outgoing, &token0_block, &token0_tx);
                let t1_out_total = calculate_token_total(&outgoing, &token1_block, &token1_tx);
                let lp_out_total = calculate_token_total(&outgoing, &pool_block, &pool_tx);

                let t0_net_out = t0_out_total.saturating_sub(t0_in_total);
                let t1_net_out = t1_out_total.saturating_sub(t1_in_total);
                if t0_net_out == 0 || t1_net_out == 0 { continue; }

                if incoming_lp > 0 && lp_out_total >= incoming_lp { continue; }

                // Prefer minimal lp_out_total, latest on tie
                let score = (lp_out_total, j);
                let better = match chosen_idx { None => true, Some(_) => score.0 < best_score.0 || (score.0 == best_score.0 && score.1 > best_score.1) };
                if better { chosen_idx = Some(j); best_score = score; }
            }
            let success = chosen_idx.is_some();
            let outgoing: Vec<JsonValue> = chosen_idx
                .map(|j| &ordered_events[j])
                .map(|ret| ret.get("data").and_then(|d| d.get("response")).and_then(|c| c.get("alkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default())
                .unwrap_or_default();

            // Compute net amounts
            let t0_in_total = calculate_token_total(&incoming, &token0_block, &token0_tx);
            let t1_in_total = calculate_token_total(&incoming, &token1_block, &token1_tx);
            let t0_out_total = calculate_token_total(&outgoing, &token0_block, &token0_tx);
            let t1_out_total = calculate_token_total(&outgoing, &token1_block, &token1_tx);
            let lp_out_total = calculate_token_total(&outgoing, &pool_block, &pool_tx);

            let token0_amount_u128 = t0_out_total.saturating_sub(t0_in_total);
            let token1_amount_u128 = t1_out_total.saturating_sub(t1_in_total);
            let lp_amount_u128 = incoming_lp.saturating_sub(lp_out_total);
            let amounts_valid = token0_amount_u128 > 0 && token1_amount_u128 > 0 && lp_amount_u128 > 0;

            // Optional burner address via decoded protostone at this vout
            let burner_address: Option<String> = decoded_by_tx_vout
                .get(txid)
                .and_then(|by_vout| by_vout.get(&(invoke_vout as i32)))
                .and_then(|items| {
                    for (_idx, d) in items {
                        if let Some(addr) = d.get("pointer_destination").and_then(|pd| pd.get("address")).and_then(|v| v.as_str()) {
                            return Some(addr.to_string());
                        }
                    }
                    None
                });

            burn_rows.push((
                txid.clone(),
                block_height,
                *tx_idx,
                pool_block.clone(),
                pool_tx.clone(),
                if success && amounts_valid { lp_amount_u128.to_string() } else { "0".to_string() },
                token0_block.clone(),
                token0_tx.clone(),
                token1_block.clone(),
                token1_tx.clone(),
                if success && amounts_valid { token0_amount_u128.to_string() } else { "0".to_string() },
                if success && amounts_valid { token1_amount_u128.to_string() } else { "0".to_string() },
                burner_address,
                *timestamp,
                success && amounts_valid,
            ));
        }
    }

    if !burn_rows.is_empty() {
        let mut dbtx = db.begin().await?;
        replace_pool_burns(&mut dbtx, &txids, &burn_rows).await?;
        dbtx.commit().await?;
    }
    info!(burns = burn_rows.len(), "indexed pool burns for block");
    Ok(())
}



