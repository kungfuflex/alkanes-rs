use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::{info, debug};

use crate::db::transactions::get_decoded_protostones_by_txid_vout;

// Reuse utility-style functions from poolswap via local copies to avoid circular deps.
// If these are moved to a shared util later, dedupe them.

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

fn is_delegate_invoke_create(event: &JsonValue) -> bool {
    if event.get("eventType").and_then(|v| v.as_str()) != Some("invoke") { return false; }
    let data = event.get("data").and_then(|v| v.as_object()).cloned();
    if data.is_none() { return false; }
    let data = data.unwrap();
    if data.get("type").and_then(|v| v.as_str()) != Some("delegatecall") { return false; }
    let inputs = data.get("context").and_then(|v| v.get("inputs")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if inputs.is_empty() { return false; }
    if let Some(opcode_hex) = inputs.get(0).and_then(|v| v.as_str()) {
        // opcode 0x0 indicates pool creation
        return hex_to_u128(opcode_hex) == Some(0);
    }
    false
}

fn find_last_success_return_with_lp<'a>(
    invoke_idx: usize,
    events: &'a [JsonValue],
    invoke_vout: i64,
    lp_block: &str,
    lp_tx: &str,
    incoming_lp: u128,
) -> Option<&'a JsonValue> {
    let mut candidate: Option<&JsonValue> = None;
    for i in (invoke_idx+1)..events.len() {
        let ev = &events[i];
        if ev.get("eventType").and_then(|v| v.as_str()) != Some("return") { continue; }
        if ev.get("vout").and_then(|v| v.as_i64()) != Some(invoke_vout) { continue; }
        let outgoing: Vec<JsonValue> = ev
            .get("data").and_then(|d| d.get("response")).and_then(|c| c.get("alkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();
        let lp_out = calculate_token_total(&outgoing, lp_block, lp_tx);
        if lp_out > incoming_lp {
            candidate = Some(ev);
        }
    }
    candidate
}

#[derive(Debug, Clone)]
pub struct DecodedPoolCreation {
    pub pool_block: String,
    pub pool_tx: String,
    pub token0_block: String,
    pub token0_tx: String,
    pub token1_block: String,
    pub token1_tx: String,
    pub token0_amount: String,
    pub token1_amount: String,
    pub token_supply: String,
    pub creator_address: Option<String>,
}

pub fn detect_pool_creations_for_tx(
    _txid: &str,
    _tx_index: i32,
    _timestamp: DateTime<Utc>,
    events: &[JsonValue],
) -> Vec<(DecodedPoolCreation, i64)> {
    let mut out: Vec<(DecodedPoolCreation, i64)> = Vec::new();

    // Ensure deterministic ordering identical to inspect: by vout asc, then invoke before return
    let mut ordered: Vec<(usize, JsonValue)> = events
        .iter()
        .cloned()
        .enumerate()
        .collect();
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

    // Materialize an events-only view to ensure stable borrowing for return matching
    let ordered_events: Vec<JsonValue> = ordered.iter().map(|t| t.1.clone()).collect();
    debug!(events = ordered_events.len(), "poolcreate: ordered events ready");

    for (i, ev) in ordered.iter().enumerate() {
        let ev = &ev.1;
        if ev.get("eventType").and_then(|v| v.as_str()) != Some("invoke") { continue; }
        if !is_delegate_invoke_create(ev) { debug!(index = i, "skip: not delegatecall create (inputs[0] != 0x0)"); continue; }

        let pool_block = ev.get("alkaneAddressBlock").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let pool_tx = ev.get("alkaneAddressTx").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if pool_block.is_empty() || pool_tx.is_empty() { debug!(index = i, "skip: missing pool id on invoke"); continue; }

        // Incoming alkanes on invoke
        let incoming: Vec<JsonValue> = ev
            .get("data").and_then(|d| d.get("context")).and_then(|c| c.get("incomingAlkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();
        debug!(index = i, incoming_len = incoming.len(), "incoming alkanes extracted");

        // Determine token0/token1 as the two non-LP ids present in incoming
        // Collect unique ids
        let mut ids: Vec<(String, String)> = Vec::new();
        for a in &incoming {
            if let Some(id) = a.get("id") {
                let b = id.get("block").and_then(value_u128_from_json).unwrap_or(0).to_string();
                let t = id.get("tx").and_then(value_u128_from_json).unwrap_or(0).to_string();
                if (b.as_str(), t.as_str()) != (pool_block.as_str(), pool_tx.as_str()) {
                    if !ids.iter().any(|(bb, tt)| bb == &b && tt == &t) {
                        ids.push((b, t));
                    }
                }
            }
        }
        if ids.len() < 2 { debug!(index = i, unique_non_lp_ids = ids.len(), "skip: less than two non-LP token ids in incoming"); continue; }
        let (token0_block, token0_tx) = ids[0].clone();
        let (token1_block, token1_tx) = ids[1].clone();
        debug!(index = i, token0_block = %token0_block, token0_tx = %token0_tx, token1_block = %token1_block, token1_tx = %token1_tx, "identified tokens");

        let invoke_vout = ev.get("vout").and_then(|v| v.as_i64()).unwrap_or(-1);
        let incoming_lp = calculate_token_total(&incoming, &pool_block, &pool_tx);

        // Select best-matching return: net LP minted, and minimal token outs (prefer latest on tie)
        let mut chosen_idx: Option<usize> = None;
        let mut best_score: (u128, u128, usize) = (u128::MAX, u128::MAX, 0);
        for j in (i+1)..ordered_events.len() {
            let evr = &ordered_events[j];
            if evr.get("eventType").and_then(|v| v.as_str()) != Some("return") { continue; }
            if evr.get("vout").and_then(|v| v.as_i64()) != Some(invoke_vout) { continue; }
            let outgoing_cand: Vec<JsonValue> = evr
                .get("data").and_then(|d| d.get("response")).and_then(|c| c.get("alkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();
            let lp_out = calculate_token_total(&outgoing_cand, &pool_block, &pool_tx);
            if lp_out <= incoming_lp { continue; }
            let t0_out_cand = calculate_token_total(&outgoing_cand, &token0_block, &token0_tx);
            let t1_out_cand = calculate_token_total(&outgoing_cand, &token1_block, &token1_tx);
            let score = (t0_out_cand, t1_out_cand, j);
            let better = match chosen_idx {
                None => true,
                Some(_) => score.0 < best_score.0 || (score.0 == best_score.0 && (score.1 < best_score.1 || (score.1 == best_score.1 && score.2 > best_score.2)))
            };
            if better { chosen_idx = Some(j); best_score = score; }
        }
        let Some(chosen_j) = chosen_idx else { debug!(index = i, invoke_vout, incoming_lp, "skip: no matching return with net LP minted"); continue };
        let ret = &ordered_events[chosen_j];
        debug!(index = i, chosen_return_index = chosen_j, "selected matching return");

        // Outgoing alkanes on return
        let outgoing: Vec<JsonValue> = ret
            .get("data").and_then(|d| d.get("response")).and_then(|c| c.get("alkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();

        // Compute net contributions
        let t0_in_total = calculate_token_total(&incoming, &token0_block, &token0_tx);
        let t1_in_total = calculate_token_total(&incoming, &token1_block, &token1_tx);
        let t0_out_total = calculate_token_total(&outgoing, &token0_block, &token0_tx);
        let t1_out_total = calculate_token_total(&outgoing, &token1_block, &token1_tx);
        let lp_out_total = calculate_token_total(&outgoing, &pool_block, &pool_tx);
        debug!(index = i, invoke_vout, incoming_lp, t0_in_total, t0_out_total, t1_in_total, t1_out_total, lp_out_total, "totals before net");

        let token0_amount_u128 = t0_in_total.saturating_sub(t0_out_total);
        let token1_amount_u128 = t1_in_total.saturating_sub(t1_out_total);
        let token_supply_u128 = lp_out_total.saturating_sub(incoming_lp);

        if token0_amount_u128 == 0 || token1_amount_u128 == 0 || token_supply_u128 == 0 { debug!(index = i, token0_amount_u128, token1_amount_u128, token_supply_u128, "skip: net amounts zero"); continue; }

        // Optional: try to read creator address from decoded protostones if wired later.
        let creator_address: Option<String> = None;

        debug!(index = i, %pool_block, %pool_tx, token0_amount_u128, token1_amount_u128, token_supply_u128, "poolcreate: decoded");
        out.push((DecodedPoolCreation {
            pool_block,
            pool_tx,
            token0_block,
            token0_tx,
            token1_block,
            token1_tx,
            token0_amount: token0_amount_u128.to_string(),
            token1_amount: token1_amount_u128.to_string(),
            token_supply: token_supply_u128.to_string(),
            creator_address,
        }, invoke_vout));
    }

    out
}

pub async fn index_pool_creations_for_block(
    db: &PgPool,
    block_height: i32,
    results: &[(String, i32, DateTime<Utc>, JsonValue, Vec<JsonValue>)]
) -> Result<Vec<(
    String,
    i32,
    i32,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    DateTime<Utc>,
)>> {
    // Preload decoded protostones for creator address extraction
    let txids: Vec<String> = results.iter().map(|r| r.0.clone()).collect();
    let decoded_by_tx_vout = get_decoded_protostones_by_txid_vout(db, &txids).await?;

    let mut rows: Vec<(
        String,
        i32,
        i32,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        DateTime<Utc>,
    )> = Vec::new();

    for (txid, tx_idx, timestamp, _tx_json, events) in results.iter() {
        for (pc, vout) in detect_pool_creations_for_tx(txid, *tx_idx, *timestamp, events) {
            // Try to find creator address using DecodedProtostone.pointer_destination.address at this vout
            let creator_address: Option<String> = decoded_by_tx_vout
                .get(txid)
                .and_then(|by_vout| by_vout.get(&(vout as i32)))
                .and_then(|items| {
                    for (_idx, d) in items {
                        if let Some(addr) = d.get("pointer_destination").and_then(|pd| pd.get("address")).and_then(|v| v.as_str()) {
                            return Some(addr.to_string());
                        }
                    }
                    None
                })
                .or(pc.creator_address.clone());

            rows.push((
                txid.clone(),
                block_height,
                *tx_idx,
                pc.pool_block,
                pc.pool_tx,
                pc.token0_block,
                pc.token0_tx,
                pc.token1_block,
                pc.token1_tx,
                pc.token0_amount,
                pc.token1_amount,
                pc.token_supply,
                creator_address,
                *timestamp,
            ));
        }
    }

    info!(creations = rows.len(), "decoded pool creations for block");
    Ok(rows)
}


