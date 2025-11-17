use anyhow::Result;
use deezel_common::traits::{BitcoinRpcProvider, JsonRpcProvider, DeezelProvider, MetashrewRpcProvider};
use serde_json::Value as JsonValue;
use serde_json::json;
use std::env;
use crate::helpers::rpc::{resilient_call, resilient_provider_call};
use tracing::warn;

// Resolve block hash by height via Bitcoin RPC provider
pub async fn get_block_hash<P>(provider: &P, height: u64) -> Result<String>
where
	P: BitcoinRpcProvider + DeezelProvider + Send + Sync,
{
	let hash = <P as BitcoinRpcProvider>::get_block_hash(provider, height).await?;
	Ok(hash)
}

// Get txids for a block via JSON-RPC method `esplora_block::txids`
pub async fn get_block_txids<P>(provider: &P, block_hash: &str) -> Result<Vec<String>>
where
	P: JsonRpcProvider + DeezelProvider + Send + Sync,
{
	let url = env::var("SANDSHREW_RPC_URL")
		.ok()
		.or_else(|| provider.get_bitcoin_rpc_url())
		.unwrap_or_else(|| "http://localhost:18888".to_string());
    let txids_val = resilient_call(provider, &url, "esplora_block::txids", json!([block_hash]), 1).await?;
	let txids: Vec<String> = txids_val
		.as_array()
		.map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
		.unwrap_or_default();
	Ok(txids)
}

// Fetch tx infos for a list of txids concurrently (batch size controls max in-flight)
pub async fn get_transactions_info<P>(provider: &P, txids: &[String], batch_size: usize) -> Result<Vec<JsonValue>>
where
	P: JsonRpcProvider + DeezelProvider + Send + Sync,
{
	use futures::stream::{self, StreamExt};
	let url = env::var("SANDSHREW_RPC_URL")
		.ok()
		.or_else(|| provider.get_bitcoin_rpc_url())
		.unwrap_or_else(|| "http://localhost:18888".to_string());
    let results: Vec<(String, Option<JsonValue>)> = stream::iter(txids.iter().cloned())
        .map(|txid| {
            let url_inner = url.clone();
            let provider_ref = provider;
            async move {
                let res = match resilient_call(provider_ref, &url_inner, "esplora_tx", json!([txid.clone()]), 1).await {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!(%txid, error = %e, "esplora_tx failed after retries");
                        None
                    }
                };
                (txid, res)
            }
        })
        .buffer_unordered(batch_size)
        .collect()
        .await;

    // If any tx fetch failed, fail the block so it can be retried rather than silently dropping txs.
    let mut failed: Vec<String> = Vec::new();
    let mut ok_vals: Vec<JsonValue> = Vec::with_capacity(results.len());
    for (txid, val_opt) in results.into_iter() {
        match val_opt {
            Some(v) => ok_vals.push(v),
            None => failed.push(txid),
        }
    }
    if !failed.is_empty() {
        let sample: Vec<String> = failed.iter().take(5).cloned().collect();
        return Err(anyhow::anyhow!(
            "esplora_tx failed for {} txids (sample: {})",
            failed.len(),
            sample.join(", ")
        ));
    }
    Ok(ok_vals)
}

// Determine if a transaction JSON has any OP_RETURN outputs
pub fn tx_has_op_return(tx_json: &JsonValue) -> bool {
	let Some(vout) = tx_json.get("vout").and_then(|v| v.as_array()) else { return false };
	for o in vout {
		if let Some(t) = o.get("scriptpubkey_type").and_then(|v| v.as_str()) {
			if t.eq_ignore_ascii_case("op_return") { return true; }
		}
		if let Some(asm) = o.get("scriptpubkey_asm").and_then(|v| v.as_str()) {
			if asm.starts_with("OP_RETURN") { return true; }
		}
		if let Some(spk) = o.get("scriptpubkey").and_then(|v| v.as_str()) {
			if spk.starts_with("6a") { return true; }
		}
	}
	false
}

// Returns the canonical chain tip height  from Metashrew's reported height,
pub async fn canonical_tip_height<P: MetashrewRpcProvider>(provider: &P) -> Result<u64> {
    let h = resilient_provider_call("get_metashrew_height", || provider.get_metashrew_height()).await?;
	if h == 0 {
		return Err(anyhow::anyhow!("unexpected metashrew height 0"));
	}
	Ok(h)
}


