use crate::helpers::rpc::{resilient_call, resilient_provider_call};
use alkanes_cli_common::traits::{
    BitcoinRpcProvider, DeezelProvider, JsonRpcProvider, MetashrewRpcProvider,
};
use anyhow::{Context, Result, bail};
use serde_json::Value as JsonValue;
use serde_json::json;
use std::env;
use tracing::warn;

// Resolve block hash by height via Bitcoin RPC provider
pub async fn get_block_hash<P>(provider: &P, height: u64) -> Result<String>
where
    P: BitcoinRpcProvider + DeezelProvider + Send + Sync,
{
    let hash = <P as BitcoinRpcProvider>::get_block_hash(provider, height).await?;
    Ok(hash)
}

fn canonical_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn parse_block_txids(block: &JsonValue, expected_hash: &str) -> Result<Vec<String>> {
    let returned_hash = block
        .get("hash")
        .and_then(JsonValue::as_str)
        .context("Bitcoin Core block response is missing hash")?;
    if returned_hash != expected_hash || !canonical_hash(returned_hash) {
        bail!("Bitcoin Core block response does not match the requested canonical hash");
    }
    let encoded = block
        .get("tx")
        .and_then(JsonValue::as_array)
        .context("Bitcoin Core block response tx must be an array")?;
    if encoded.is_empty() {
        bail!("Bitcoin Core block response cannot have an empty transaction list");
    }

    let mut txids = Vec::with_capacity(encoded.len());
    let mut seen = std::collections::HashSet::with_capacity(encoded.len());
    for (index, value) in encoded.iter().enumerate() {
        let txid = value
            .as_str()
            .with_context(|| format!("Bitcoin Core block tx {index} is not a txid string"))?;
        if !canonical_hash(txid) {
            bail!("Bitcoin Core block tx {index} is not canonical lowercase hexadecimal");
        }
        if !seen.insert(txid) {
            bail!("Bitcoin Core block response contains duplicate transaction {txid}");
        }
        txids.push(txid.to_owned());
    }
    if let Some(declared) = block.get("nTx") {
        let declared = declared
            .as_u64()
            .context("Bitcoin Core block response nTx must be a non-negative integer")?;
        if declared != txids.len() as u64 {
            bail!("Bitcoin Core block response nTx does not match its transaction array");
        }
    }
    Ok(txids)
}

// Get the authoritative transaction list directly from Bitcoin Core.
pub async fn get_block_txids<P>(provider: &P, block_hash: &str) -> Result<Vec<String>>
where
    P: BitcoinRpcProvider + Send + Sync,
{
    if !canonical_hash(block_hash) {
        bail!("requested block hash is not canonical lowercase hexadecimal");
    }
    let block = resilient_provider_call("get_block", || provider.get_block(block_hash, false))
        .await?;
    parse_block_txids(&block, block_hash)
}

// Fetch tx infos for a list of txids concurrently (batch size controls max in-flight)
pub async fn get_transactions_info<P>(
    provider: &P,
    txids: &[String],
    batch_size: usize,
) -> Result<Vec<JsonValue>>
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
                let res = match resilient_call(
                    provider_ref,
                    &url_inner,
                    "esplora_tx",
                    json!([txid.clone()]),
                    1,
                )
                .await
                {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!(%txid, error = %e, "esplora_tx failed after retries");
                        None
                    }
                };
                (txid, res)
            }
        })
        .buffered(batch_size)
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
    let Some(vout) = tx_json.get("vout").and_then(|v| v.as_array()) else {
        return false;
    };
    for o in vout {
        if let Some(t) = o.get("scriptpubkey_type").and_then(|v| v.as_str()) {
            if t.eq_ignore_ascii_case("op_return") {
                return true;
            }
        }
        if let Some(asm) = o.get("scriptpubkey_asm").and_then(|v| v.as_str()) {
            if asm.starts_with("OP_RETURN") {
                return true;
            }
        }
        if let Some(spk) = o.get("scriptpubkey").and_then(|v| v.as_str()) {
            if spk.starts_with("6a") {
                return true;
            }
        }
    }
    false
}

// Returns the canonical chain tip height  from Metashrew's reported height,
pub async fn canonical_tip_height<P: MetashrewRpcProvider>(provider: &P) -> Result<u64> {
    let h =
        resilient_provider_call("get_metashrew_height", || provider.get_metashrew_height()).await?;
    if h == 0 {
        return Err(anyhow::anyhow!("unexpected metashrew height 0"));
    }
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: char) -> String {
        std::iter::repeat(byte).take(64).collect()
    }

    #[test]
    fn core_block_transaction_list_is_strict_and_complete() {
        let block_hash = hash('a');
        let valid = json!({
            "hash": block_hash,
            "nTx": 2,
            "tx": [hash('b'), hash('c')]
        });
        assert_eq!(parse_block_txids(&valid, &hash('a')).unwrap().len(), 2);

        let malformed = [
            json!({"hash": hash('a'), "nTx": 2, "tx": [hash('b'), 1]}),
            json!({"hash": hash('a'), "nTx": 2, "tx": [hash('b')]}),
            json!({"hash": hash('a'), "tx": [hash('b'), hash('b')]}),
            json!({"hash": hash('d'), "tx": [hash('b')]}),
        ];
        for block in malformed {
            assert!(parse_block_txids(&block, &hash('a')).is_err());
        }
    }
}
