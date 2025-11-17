use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::info;

use crate::db::transactions::{replace_subfrost_wraps, replace_subfrost_unwraps, get_decoded_protostones_by_txid_vout};

fn hex_to_u128(hex_str: &str) -> Option<u128> {
    let s = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    u128::from_str_radix(s, 16).ok()
}

fn value_u128_from_json(v: &JsonValue) -> Option<u128> {
    if let Some(obj) = v.as_object() {
        if obj.contains_key("hi") && obj.contains_key("lo") {
            let hi = obj.get("hi")?.as_u64()? as u128;
            let lo = obj.get("lo")?.as_u64()? as u128;
            return Some((hi << 64) | lo);
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

fn is_subfrost_wrap_invoke(event: &JsonValue) -> bool {
    if event.get("eventType").and_then(|v| v.as_str()) != Some("invoke") { return false; }
    let data = event.get("data");
    let inputs = data.and_then(|d| d.get("context")).and_then(|c| c.get("inputs")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if inputs.is_empty() { return false; }
    // opcode 0x4d (77) indicates wrap (mint) on Subfrost
    if let Some(opcode_str) = inputs.get(0).and_then(|v| v.as_str()) {
        if hex_to_u128(opcode_str) != Some(77) { return false; }
    } else { return false; }
    // Ensure this call targets Subfrost alkaneId 32:0 via flattened address fields
    let blk_dec = event.get("alkaneAddressBlock").and_then(|v| v.as_str()).unwrap_or("");
    let tx_dec = event.get("alkaneAddressTx").and_then(|v| v.as_str()).unwrap_or("");
    blk_dec == "32" && tx_dec == "0"
}

fn is_subfrost_unwrap_invoke(event: &JsonValue) -> bool {
    if event.get("eventType").and_then(|v| v.as_str()) != Some("invoke") { return false; }
    let data = event.get("data");
    let inputs = data.and_then(|d| d.get("context")).and_then(|c| c.get("inputs")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if inputs.is_empty() { return false; }
    // opcode 0x4e (78) indicates unwrap (redeem) on Subfrost
    if let Some(opcode_str) = inputs.get(0).and_then(|v| v.as_str()) {
        if hex_to_u128(opcode_str) != Some(78) { return false; }
    } else { return false; }
    // Ensure this call targets Subfrost alkaneId 32:0 via flattened address fields
    let blk_dec = event.get("alkaneAddressBlock").and_then(|v| v.as_str()).unwrap_or("");
    let tx_dec = event.get("alkaneAddressTx").and_then(|v| v.as_str()).unwrap_or("");
    blk_dec == "32" && tx_dec == "0"
}

pub async fn index_subfrost_wraps_for_block(
    db: &PgPool,
    block_height: i32,
    results: &[(String, i32, DateTime<Utc>, JsonValue, Vec<JsonValue>)]
) -> Result<()> {
    let txids: Vec<String> = results.iter().map(|r| r.0.clone()).collect();
    // Preload decoded protostones for address extraction (pointer_destination.address)
    let decoded_by_tx_vout = get_decoded_protostones_by_txid_vout(db, &txids).await?;
    let mut wrap_rows: Vec<(String, i32, i32, Option<String>, String, bool, DateTime<Utc>)> = Vec::new();

    for (txid, tx_idx, timestamp, _tx_json, events) in results.iter() {
        // deterministically order by vout asc then invoke before return
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
            if !is_subfrost_wrap_invoke(ev) { continue; }
            let invoke_vout = ev.get("vout").and_then(|v| v.as_i64()).unwrap_or(-1);

            // find matching successful return with Subfrost token 0x20:0 in response.alkanes
            let mut success = false;
            let mut amount_u128: u128 = 0;
            for j in (i+1)..ordered_events.len() {
                let maybe_ret = &ordered_events[j];
                if maybe_ret.get("eventType").and_then(|v| v.as_str()) != Some("return") { continue; }
                if maybe_ret.get("vout").and_then(|v| v.as_i64()) != Some(invoke_vout) { continue; }
                let status_ok = maybe_ret
                    .get("data").and_then(|d| d.get("status")).and_then(|s| s.as_str())
                    .map(|s| s.eq_ignore_ascii_case("success") || s.eq_ignore_ascii_case("ok"))
                    .unwrap_or(false);
                if !status_ok { continue; }
                let alks = maybe_ret
                    .get("data").and_then(|d| d.get("response")).and_then(|r| r.get("alkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();
                for a in &alks {
                    let id = a.get("id");
                    if let Some(id) = id {
                        let b = id.get("block").and_then(|v| v.as_str());
                        let t = id.get("tx").and_then(|v| v.as_str());
                        if hex_to_u128(b.unwrap_or("")) == Some(0x20) && hex_to_u128(t.unwrap_or("")) == Some(0) {
                            if let Some(v) = a.get("value").and_then(value_u128_from_json) {
                                amount_u128 = amount_u128.saturating_add(v);
                                success = true;
                            }
                        }
                    }
                }
                if success { break; }
            }

            // Extract wrapper address from DecodedProtostone.pointer_destination.address at this vout
            let address: Option<String> = decoded_by_tx_vout
                .get(txid)
                .and_then(|by_vout| by_vout.get(&(invoke_vout as i32)))
                .and_then(|items| {
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
            let amount_str = if success { amount_u128.to_string() } else { "0".to_string() };
            wrap_rows.push((
                txid.clone(),
                block_height,
                *tx_idx,
                address,
                amount_str,
                success,
                *timestamp,
            ));
        }
    }

    if !wrap_rows.is_empty() {
        let mut dbtx = db.begin().await?;
        replace_subfrost_wraps(&mut dbtx, &txids, &wrap_rows).await?;
        dbtx.commit().await?;
    }
    info!(wraps = wrap_rows.len(), "indexed subfrost wraps for block");
    Ok(())
}

pub async fn index_subfrost_unwraps_for_block(
    db: &PgPool,
    block_height: i32,
    results: &[(String, i32, DateTime<Utc>, JsonValue, Vec<JsonValue>)]
) -> Result<()> {
    let txids: Vec<String> = results.iter().map(|r| r.0.clone()).collect();
    // Preload decoded protostones for address extraction (pointer_destination.address)
    let decoded_by_tx_vout = get_decoded_protostones_by_txid_vout(db, &txids).await?;
    let mut unwrap_rows: Vec<(String, i32, i32, Option<String>, String, bool, DateTime<Utc>)> = Vec::new();

    for (txid, tx_idx, timestamp, _tx_json, events) in results.iter() {
        // deterministically order by vout asc then invoke before return
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
            if !is_subfrost_unwrap_invoke(ev) { continue; }
            let invoke_vout = ev.get("vout").and_then(|v| v.as_i64()).unwrap_or(-1);

            // compute incoming amount of Subfrost token 0x20:0 from invoke's context.incomingAlkanes
            let mut incoming_u128: u128 = 0;
            let inv_alks = ev
                .get("data").and_then(|d| d.get("context")).and_then(|c| c.get("incomingAlkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();
            for a in &inv_alks {
                let id = a.get("id");
                if let Some(id) = id {
                    let b = id.get("block").and_then(|v| v.as_str());
                    let t = id.get("tx").and_then(|v| v.as_str());
                    if hex_to_u128(b.unwrap_or("")) == Some(0x20) && hex_to_u128(t.unwrap_or("")) == Some(0) {
                        if let Some(v) = a.get("value").and_then(value_u128_from_json) {
                            incoming_u128 = incoming_u128.saturating_add(v);
                        }
                    }
                }
            }

            // find matching successful return and compute outgoing Subfrost tokens from response.alkanes
            let mut success = false;
            let mut outgoing_u128: u128 = 0;
            for j in (i+1)..ordered_events.len() {
                let maybe_ret = &ordered_events[j];
                if maybe_ret.get("eventType").and_then(|v| v.as_str()) != Some("return") { continue; }
                if maybe_ret.get("vout").and_then(|v| v.as_i64()) != Some(invoke_vout) { continue; }
                let status_ok = maybe_ret
                    .get("data").and_then(|d| d.get("status")).and_then(|s| s.as_str())
                    .map(|s| s.eq_ignore_ascii_case("success") || s.eq_ignore_ascii_case("ok"))
                    .unwrap_or(false);
                if !status_ok { continue; }
                let alks = maybe_ret
                    .get("data").and_then(|d| d.get("response")).and_then(|r| r.get("alkanes")).and_then(|a| a.as_array()).cloned().unwrap_or_default();
                for a in &alks {
                    let id = a.get("id");
                    if let Some(id) = id {
                        let b = id.get("block").and_then(|v| v.as_str());
                        let t = id.get("tx").and_then(|v| v.as_str());
                        if hex_to_u128(b.unwrap_or("")) == Some(0x20) && hex_to_u128(t.unwrap_or("")) == Some(0) {
                            if let Some(v) = a.get("value").and_then(value_u128_from_json) {
                                outgoing_u128 = outgoing_u128.saturating_add(v);
                            }
                        }
                    }
                }
                success = true;
                break;
            }

            let net_u128 = if success { incoming_u128.saturating_sub(outgoing_u128) } else { 0 };

            // Extract address from DecodedProtostone.pointer_destination.address at this vout
            let address: Option<String> = decoded_by_tx_vout
                .get(txid)
                .and_then(|by_vout| by_vout.get(&(invoke_vout as i32)))
                .and_then(|items| {
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

            let amount_str = if success { net_u128.to_string() } else { "0".to_string() };
            unwrap_rows.push((
                txid.clone(),
                block_height,
                *tx_idx,
                address,
                amount_str,
                success,
                *timestamp,
            ));
        }
    }

    if !unwrap_rows.is_empty() {
        let mut dbtx = db.begin().await?;
        replace_subfrost_unwraps(&mut dbtx, &txids, &unwrap_rows).await?;
        dbtx.commit().await?;
    }
    info!(unwraps = unwrap_rows.len(), "indexed subfrost unwraps for block");
    Ok(())
}

