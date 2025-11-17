use anyhow::Result;
use clap::Parser;
use dotenvy::dotenv;
use tracing_subscriber::{fmt, EnvFilter};
use tracing::info;
use chrono::{TimeZone, Utc};

#[derive(Parser, Debug)]
#[command(name = "swaps", about = "Run PoolSwap indexer for a specific block")] 
struct Cli {
    /// Block height to process
    #[arg(long)]
    height: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(env_filter).init();

    let cli = Cli::parse();
    let cfg = alkanes_contract_indexer::config::AppConfig::from_env()?;
    let pool = alkanes_contract_indexer::db::connect(&cfg.database_url, 5).await?;
    let provider = alkanes_contract_indexer::provider::build_provider(
        cfg.bitcoin_rpc_url.clone(),
        cfg.sandshrew_rpc_url.clone(),
        cfg.esplora_url.clone(),
        cfg.network_provider.clone(),
    ).await?;

    // Fetch txids and tx infos for the block
    let block_hash = alkanes_contract_indexer::helpers::block::get_block_hash(&provider, cli.height).await?;
    let txids = alkanes_contract_indexer::helpers::block::get_block_txids(&provider, &block_hash).await?;
    let txs = alkanes_contract_indexer::helpers::block::get_transactions_info(&provider, &txids, 25).await?;
    let op_return_txs: Vec<_> = txs.iter().filter(|tx| alkanes_contract_indexer::helpers::block::tx_has_op_return(tx)).cloned().collect();
    if op_return_txs.is_empty() { info!(height = cli.height, "no OP_RETURN txs in block"); return Ok(()); }

    // Decode and trace
    let results = alkanes_contract_indexer::helpers::protostone::decode_and_trace_for_block(&provider, &op_return_txs, 32, 16).await?;

    // Write basic tx/protostone/trace rows (for consistency with pipeline)
    let mut tx_rows: Vec<(i32, String, i32, bool, bool, serde_json::Value)> = Vec::with_capacity(results.len());
    let mut all_txids: Vec<String> = Vec::with_capacity(results.len());
    let mut protostone_rows: Vec<(String, i32, i32, i32, serde_json::Value)> = Vec::new();
    let mut event_rows: Vec<(String, i32, i32, String, serde_json::Value, String, String)> = Vec::new();
    for (tx_index, r) in results.iter().enumerate() {
        let txid = r.transaction_id.clone();
        all_txids.push(txid.clone());
        tx_rows.push((
            cli.height as i32,
            txid.clone(),
            tx_index as i32,
            r.has_trace,
            r.trace_succeed,
            r.transaction_json.clone(),
        ));
        for d in &r.decoded_protostones {
            protostone_rows.push((txid.clone(), d.vout, d.protostone_index, cli.height as i32, d.decoded.clone()));
        }
        for e in &r.trace_events {
            event_rows.push((
                txid.clone(),
                cli.height as i32,
                e.vout,
                e.event_type.clone(),
                e.data.clone(),
                e.alkane_address_block.clone(),
                e.alkane_address_tx.clone(),
            ));
        }
    }
    let mut dbtx = pool.begin().await?;
    alkanes_contract_indexer::db::transactions::upsert_alkane_transactions(&mut dbtx, &tx_rows).await?;
    alkanes_contract_indexer::db::transactions::replace_decoded_protostones(&mut dbtx, &all_txids, &protostone_rows).await?;
    alkanes_contract_indexer::db::transactions::replace_trace_events(&mut dbtx, &all_txids, &event_rows).await?;
    dbtx.commit().await?;

    // Build inputs and run swap indexer
    let mut swap_inputs: Vec<(String, i32, chrono::DateTime<Utc>, serde_json::Value, Vec<serde_json::Value>)> = Vec::new();
    for (tx_index, r) in results.iter().enumerate() {
        let ts_opt = r.transaction_json.get("status").and_then(|s| s.get("block_time")).and_then(|v| v.as_i64());
        let ts = ts_opt.and_then(|secs| Utc.timestamp_opt(secs, 0).single()).unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap());
        let trace_events_json: Vec<serde_json::Value> = r.trace_events.iter().map(|e| {
            serde_json::json!({
                "vout": e.vout,
                "eventType": e.event_type,
                "data": e.data,
                "alkaneAddressBlock": e.alkane_address_block,
                "alkaneAddressTx": e.alkane_address_tx,
            })
        }).collect();
        swap_inputs.push((r.transaction_id.clone(), tx_index as i32, ts, r.transaction_json.clone(), trace_events_json));
    }
    alkanes_contract_indexer::helpers::poolswap::index_pool_swaps_for_block(&pool, cli.height as i32, &swap_inputs).await?;

    info!(height = cli.height, "pool swap indexing complete");
    Ok(())
}


