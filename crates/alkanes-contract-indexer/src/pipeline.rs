use crate::canonical_commit::{self, PrepareOutcome};
use crate::db::transactions::replace_pool_creations;
use crate::db::transactions::{
    replace_decoded_protostones, replace_trace_events, upsert_alkane_transactions,
};
use crate::helpers::block::{
    canonical_tip_height, get_block_hash as helper_get_block_hash,
    get_block_txids as helper_get_block_txids,
    get_transactions_info as helper_get_transactions_info, tx_has_op_return,
};
use crate::helpers::notify::{notify_pools_processed, publish_block_processed};
use crate::helpers::poolburn::index_pool_burns_for_block;
use crate::helpers::poolcreate::index_pool_creations_for_block;
use crate::helpers::poolmint::index_pool_mints_for_block;
use crate::helpers::pools::fetch_and_upsert_pools_for_tip;
use crate::helpers::poolswap::index_pool_swaps_for_block;
use crate::helpers::protostone::TxDecodeTraceResult;
use crate::helpers::protostone::decode_and_trace_for_block;
use crate::helpers::subfrost::{index_subfrost_unwraps_for_block, index_subfrost_wraps_for_block};
use crate::transform_integration::{
    TraceTransformService, convert_trace_event, convert_transaction_context,
};
use alkanes_cli_common::traits::{
    BitcoinRpcProvider, DeezelProvider, JsonRpcProvider, MetashrewRpcProvider,
};
use alkanes_cli_sys::SystemAlkanes as ConcreteProvider;
use alkanes_trace_transform::types::VoutInfo;
use anyhow::{Context, Result, anyhow};
use chrono::DateTime;
use chrono::{TimeZone, Utc};
use sqlx::PgPool;
use std::collections::HashSet;
use std::time::Instant;
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct BlockContext {
    pub height: u64,
    pub emit_publish: bool,
}

fn spent_outpoints_from_transactions(txs: &[serde_json::Value]) -> Result<Vec<(String, i32)>> {
    let mut outpoints = Vec::new();
    for tx in txs {
        let txid = tx
            .get("txid")
            .and_then(|value| value.as_str())
            .unwrap_or("<unknown>");
        let inputs = tx
            .get("vin")
            .and_then(|value| value.as_array())
            .with_context(|| format!("transaction {txid} vin must be an array"))?;
        for input in inputs {
            let is_coinbase = input
                .get("is_coinbase")
                .map(|value| value.as_bool().context("vin.is_coinbase must be boolean"))
                .transpose()?
                .unwrap_or(false);
            if is_coinbase {
                continue;
            }
            let previous_txid = input
                .get("txid")
                .and_then(|value| value.as_str())
                .context("non-coinbase vin is missing txid")?;
            if previous_txid.len() != 64
                || !previous_txid
                    .as_bytes()
                    .iter()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
            {
                return Err(anyhow!(
                    "non-coinbase vin txid is not canonical lowercase hexadecimal"
                ));
            }
            let previous_vout = input
                .get("vout")
                .and_then(|value| value.as_u64())
                .context("non-coinbase vin is missing a non-negative vout")?;
            outpoints.push((
                previous_txid.to_owned(),
                i32::try_from(previous_vout).context("vin.vout exceeds PostgreSQL INTEGER")?,
            ));
        }
    }
    Ok(outpoints)
}

fn canonical_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn verify_source_transactions(
    txids: &[String],
    txs: &[serde_json::Value],
    expected_height: u64,
    expected_block_hash: &str,
) -> Result<DateTime<Utc>> {
    if txids.len() != txs.len() {
        return Err(anyhow!(
            "source block declared {} transactions but {} bodies were returned",
            txids.len(),
            txs.len()
        ));
    }
    let expected: HashSet<&str> = txids.iter().map(String::as_str).collect();
    if expected.len() != txids.len() {
        return Err(anyhow!("source block transaction list contains duplicates"));
    }
    if !canonical_hash(expected_block_hash) || txids.iter().any(|txid| !canonical_hash(txid)) {
        return Err(anyhow!(
            "source block or transaction hash is not canonical lowercase hexadecimal"
        ));
    }
    let mut observed = HashSet::with_capacity(txs.len());
    let mut block_timestamp = None;
    for (tx_index, tx) in txs.iter().enumerate() {
        let txid = tx
            .get("txid")
            .and_then(|value| value.as_str())
            .context("source transaction body is missing txid")?;
        if !canonical_hash(txid) {
            return Err(anyhow!(
                "source transaction body contains a non-canonical txid"
            ));
        }
        if !observed.insert(txid) {
            return Err(anyhow!(
                "source transaction bodies contain duplicate txid {txid}"
            ));
        }

        let vin = tx
            .get("vin")
            .and_then(serde_json::Value::as_array)
            .with_context(|| format!("source transaction {txid} vin must be an array"))?;
        if vin.is_empty() {
            return Err(anyhow!("source transaction {txid} cannot have an empty vin"));
        }
        let vout = tx
            .get("vout")
            .and_then(serde_json::Value::as_array)
            .with_context(|| format!("source transaction {txid} vout must be an array"))?;
        if vout.is_empty() {
            return Err(anyhow!("source transaction {txid} cannot have an empty vout"));
        }
        for (vout_index, output) in vout.iter().enumerate() {
            let script = output
                .get("scriptpubkey")
                .and_then(serde_json::Value::as_str)
                .with_context(|| {
                    format!("source transaction {txid} vout {vout_index} is missing scriptpubkey")
                })?;
            if script.len() % 2 != 0
                || !script
                    .as_bytes()
                    .iter()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
            {
                return Err(anyhow!(
                    "source transaction {txid} vout {vout_index} has non-canonical scriptpubkey"
                ));
            }
            output
                .get("value")
                .and_then(serde_json::Value::as_u64)
                .with_context(|| {
                    format!(
                        "source transaction {txid} vout {vout_index} value must be a non-negative u64"
                    )
                })?;
        }

        let status = tx
            .get("status")
            .context("source transaction is missing confirmation status")?;
        if status.get("confirmed").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(anyhow!("source transaction {txid} is not confirmed"));
        }
        if status
            .get("block_height")
            .and_then(serde_json::Value::as_u64)
            != Some(expected_height)
        {
            return Err(anyhow!(
                "source transaction {txid} does not belong to the requested block height"
            ));
        }
        if status
            .get("block_hash")
            .and_then(serde_json::Value::as_str)
            != Some(expected_block_hash)
        {
            return Err(anyhow!(
                "source transaction {txid} does not belong to the requested block hash"
            ));
        }
        let timestamp_seconds = status
            .get("block_time")
            .and_then(serde_json::Value::as_i64)
            .with_context(|| format!("source transaction {txid} is missing status.block_time"))?;
        let timestamp = Utc
            .timestamp_opt(timestamp_seconds, 0)
            .single()
            .with_context(|| format!("source transaction {txid} has invalid status.block_time"))?;
        if block_timestamp
            .replace(timestamp)
            .is_some_and(|previous| previous != timestamp)
        {
            return Err(anyhow!(
                "source transaction {tx_index} reports a different block timestamp"
            ));
        }
    }
    if observed != expected {
        return Err(anyhow!(
            "source transaction bodies do not exactly match the block txid set"
        ));
    }
    block_timestamp.context("source block contains no transaction timestamp")
}

#[derive(Clone, Debug)]
pub struct Pipeline {
    pool: PgPool,
    factory_block_id: String,
    factory_tx_id: String,
}

impl Pipeline {
    pub fn new(pool: PgPool, factory_block_id: String, factory_tx_id: String) -> Self {
        Self {
            pool,
            factory_block_id,
            factory_tx_id,
        }
    }

    /// Compare the last published commitment with Bitcoin Core and atomically remove a forked
    /// suffix. This runs even when caught up, so a same-height replacement cannot go unnoticed.
    pub async fn reconcile_canonical<P>(&self, provider: &P) -> Result<Option<u64>>
    where
        P: BitcoinRpcProvider + MetashrewRpcProvider + DeezelProvider + Send + Sync,
    {
        let mut serial_guard = self.pool.begin().await?;
        canonical_commit::acquire_serial_lock(&mut serial_guard).await?;
        let Some(tip) = canonical_commit::latest_commit(&self.pool).await? else {
            serial_guard.commit().await?;
            return Ok(None);
        };
        let source_tip_height = canonical_tip_height(provider).await?;
        if tip.height <= source_tip_height
            && helper_get_block_hash(provider, tip.height).await? == tip.block_hash
        {
            serial_guard.commit().await?;
            return Ok(None);
        }

        let commits = canonical_commit::commits_descending(&self.pool).await?;
        let mut ancestor = None;
        for commit in &commits {
            if commit.height > source_tip_height {
                continue;
            }
            let canonical_hash = helper_get_block_hash(provider, commit.height).await?;
            if canonical_hash == commit.block_hash {
                ancestor = Some((commit.height, commit.block_hash.clone()));
                break;
            }
        }
        let epoch = canonical_commit::rollback_to_common_ancestor(&self.pool, ancestor).await?;
        serial_guard.commit().await?;
        Ok(Some(epoch))
    }

    // Runs on every new tip height (even during catch-up)
    pub async fn fetch_pools_for_tip(
        &self,
        provider: &ConcreteProvider,
        tip_height: u64,
    ) -> Result<()> {
        let res = fetch_and_upsert_pools_for_tip(
            provider,
            &self.pool,
            &self.factory_block_id,
            &self.factory_tx_id,
            tip_height,
        )
        .await;
        if res.is_ok() {
            notify_pools_processed(tip_height).await;
        }
        res
    }

    /// Sequential per-block processing (historical and then following tip)
    /// Returns the block hash on success for position tracking
    pub async fn process_block_sequential<P>(
        &self,
        provider: &P,
        ctx: BlockContext,
    ) -> Result<String>
    where
        P: DeezelProvider
            + JsonRpcProvider
            + BitcoinRpcProvider
            + alkanes_cli_common::traits::AlkanesProvider
            + alkanes_cli_common::traits::EsploraProvider
            + Send
            + Sync,
    {
        // The poller and catch-up coordinator run concurrently. A transaction-scoped advisory lock
        // serializes the complete staging/publish interval, including failed attempts.
        let mut serial_guard = self.pool.begin().await?;
        canonical_commit::acquire_serial_lock(&mut serial_guard).await?;
        let transform_height =
            i32::try_from(ctx.height).map_err(|_| anyhow!("block height exceeds transform i32"))?;

        // Resolve block hash via bitcoind and print/log it
        let block_hash = helper_get_block_hash(provider, ctx.height).await?;
        let previous_block_hash = if ctx.height == 0 {
            canonical_commit::ZERO_BLOCK_HASH.to_owned()
        } else {
            helper_get_block_hash(provider, ctx.height - 1).await?
        };
        match canonical_commit::prepare_height(
            &self.pool,
            ctx.height,
            &block_hash,
            &previous_block_hash,
        )
        .await?
        {
            PrepareOutcome::AlreadyCommitted => {
                serial_guard.commit().await?;
                return Ok(block_hash);
            }
            PrepareOutcome::Ready => {}
        }
        info!(height = ctx.height, %block_hash, "resolved block hash");

        // Fetch txids for the block via JSON-RPC helper
        let txids = helper_get_block_txids(provider, &block_hash).await?;
        info!(
            height = ctx.height,
            count = txids.len(),
            "esplora_block::txids fetched"
        );

        // Fetch tx infos concurrently using helper and maintain order
        let txs = helper_get_transactions_info(provider, &txids, 25).await?;
        let block_ts = verify_source_transactions(&txids, &txs, ctx.height, &block_hash)?;
        info!(height = ctx.height, txs = txs.len(), "esplora_tx fetched");
        let spent_outpoints = spent_outpoints_from_transactions(&txs)?;
        canonical_commit::stage_spent_outpoints(&self.pool, ctx.height, &spent_outpoints).await?;
        info!(
            height = ctx.height,
            spends = spent_outpoints.len(),
            "Bitcoin inputs staged for canonical inventory publication"
        );

        // Filter for OP_RETURN outputs
        let opret_count: usize = txs.iter().filter(|tx| tx_has_op_return(tx)).count();
        info!(
            height = ctx.height,
            op_return_txs = opret_count,
            "OP_RETURN transactions in block"
        );

        // Build filtered list of OP_RETURN transactions only
        let op_return_txs: Vec<_> = txs
            .iter()
            .filter(|tx| tx_has_op_return(tx))
            .cloned()
            .collect();

        // Decode+trace protostones for this block (only OP_RETURN txs) with timing
        if !op_return_txs.is_empty() {
            let count = op_return_txs.len();
            let t0 = Instant::now();
            info!(
                height = ctx.height,
                op_return_txs = count,
                "decode_and_trace_for_block: start"
            );
            let results: Vec<TxDecodeTraceResult> =
                decode_and_trace_for_block(provider, &op_return_txs, 32, 16).await?;

            // Prepare batch payloads
            let mut tx_rows: Vec<(i32, String, i32, bool, bool, serde_json::Value)> =
                Vec::with_capacity(results.len());
            let mut all_txids: Vec<String> = Vec::with_capacity(results.len());
            let mut protostone_rows: Vec<(String, i32, i32, i32, serde_json::Value)> = Vec::new();
            let mut event_rows: Vec<(String, i32, i32, String, serde_json::Value, String, String)> =
                Vec::new();

            for (tx_index, r) in results.iter().enumerate() {
                let txid = r.transaction_id.clone();
                all_txids.push(txid.clone());
                tx_rows.push((
                    transform_height,
                    txid.clone(),
                    tx_index as i32,
                    r.has_trace,
                    r.trace_succeed,
                    r.transaction_json.clone(),
                ));
                for d in &r.decoded_protostones {
                    protostone_rows.push((
                        txid.clone(),
                        d.vout,
                        d.protostone_index,
                        transform_height,
                        d.decoded.clone(),
                    ));
                }
                for e in &r.trace_events {
                    event_rows.push((
                        txid.clone(),
                        transform_height,
                        e.vout,
                        e.event_type.clone(),
                        e.data.clone(),
                        e.alkane_address_block.clone(),
                        e.alkane_address_tx.clone(),
                    ));
                }
            }

            // Write in a single transaction
            let mut dbtx = self.pool.begin().await?;
            upsert_alkane_transactions(&mut dbtx, &tx_rows).await?;
            replace_decoded_protostones(&mut dbtx, &all_txids, &protostone_rows).await?;
            replace_trace_events(&mut dbtx, &all_txids, &event_rows).await?;
            dbtx.commit().await?;

            let elapsed_ms = t0.elapsed().as_millis() as u64;
            info!(
                height = ctx.height,
                op_return_txs = count,
                elapsed_ms,
                "decode_and_trace_for_block: done"
            );

            // Process traces through transform pipeline
            let transform_t0 = std::time::Instant::now();
            let mut transform_service = TraceTransformService::new(self.pool.clone());
            // Load known pools at start of each block processing
            transform_service.load_existing_pools().await?;
            info!(
                "Transform pipeline: processing {} transactions from block {}",
                results.len(),
                ctx.height
            );
            for r in &results {
                info!(
                    "Transform pipeline: tx {} has {} trace_events",
                    r.transaction_id,
                    r.trace_events.len()
                );
                if !r.trace_events.is_empty() {
                    // Convert transaction info to context
                    let tx_info = txs.iter().find(|tx| {
                        tx.get("txid").and_then(|v| v.as_str()).unwrap_or("") == r.transaction_id
                    });

                    let tx = tx_info.with_context(|| {
                        format!(
                            "traced transaction {} is missing from its source block",
                            r.transaction_id
                        )
                    })?;
                    let timestamp_seconds = tx
                        .get("status")
                        .and_then(|status| status.get("block_time"))
                        .and_then(|value| value.as_i64())
                        .context("confirmed transaction is missing status.block_time")?;
                    let timestamp = Utc
                        .timestamp_opt(timestamp_seconds, 0)
                        .single()
                        .context("confirmed transaction has an invalid status.block_time")?;
                    let source_vouts = tx
                        .get("vout")
                        .and_then(|value| value.as_array())
                        .context("confirmed transaction vout must be an array")?;
                    let vouts: Vec<VoutInfo> = source_vouts
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            Ok(VoutInfo {
                                index: i32::try_from(index)
                                    .context("transaction has too many outputs")?,
                                address: value
                                    .get("scriptpubkey_address")
                                    .map(|address| {
                                        address
                                            .as_str()
                                            .context("scriptpubkey_address must be text")
                                            .map(str::to_owned)
                                    })
                                    .transpose()?,
                                script_pubkey: value
                                    .get("scriptpubkey")
                                    .and_then(|script| script.as_str())
                                    .context("vout.scriptpubkey must be text")?
                                    .to_owned(),
                                value: value
                                    .get("value")
                                    .and_then(|amount| amount.as_u64())
                                    .context("vout.value must be a non-negative u64")?,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let context = convert_transaction_context(
                        r.transaction_id.clone(),
                        transform_height,
                        timestamp,
                        vouts,
                    );
                    let traces: Vec<alkanes_trace_transform::types::TraceEvent> = r
                        .trace_events
                        .iter()
                        .map(|e| {
                            convert_trace_event(
                                e.event_type.clone(),
                                e.vout,
                                e.alkane_address_block.clone(),
                                e.alkane_address_tx.clone(),
                                e.data.clone(),
                            )
                        })
                        .collect();
                    transform_service
                        .process_transaction(context, traces)
                        .await?;
                }
            }
            let transform_elapsed_ms = transform_t0.elapsed().as_millis() as u64;
            info!(
                height = ctx.height,
                elapsed_ms = transform_elapsed_ms,
                "trace transform processing: done"
            );

            // Extract and index balances from trace events
            let balance_t0 = std::time::Instant::now();
            let mut all_outpoint_balances = Vec::new();
            for r in &results {
                if !r.trace_events.is_empty() {
                    let balances = crate::helpers::balance_tracker::extract_balance_changes(
                        &r.transaction_json,
                        &r.trace_events,
                    )?;
                    all_outpoint_balances.extend(balances);
                }
            }

            if !all_outpoint_balances.is_empty() {
                crate::helpers::balance_tracker::upsert_utxo_balances(
                    &self.pool,
                    transform_height,
                    &all_outpoint_balances,
                )
                .await?;
                crate::helpers::balance_tracker::update_address_balances(
                    &self.pool,
                    &all_outpoint_balances,
                )
                .await?;
                crate::helpers::balance_tracker::refresh_holders_for_block(
                    &self.pool,
                    &all_outpoint_balances,
                )
                .await?;
            }
            let balance_elapsed_ms = balance_t0.elapsed().as_millis() as u64;
            info!(
                height = ctx.height,
                balance_updates = all_outpoint_balances.len(),
                elapsed_ms = balance_elapsed_ms,
                "balance indexing: done"
            );

            // Extract and index storage changes from trace events
            let storage_t0 = std::time::Instant::now();
            let mut all_storage_changes = Vec::new();
            for r in &results {
                if !r.trace_events.is_empty() {
                    match crate::helpers::storage_tracker::extract_storage_changes(
                        &r.transaction_json,
                        &r.trace_events,
                    ) {
                        Ok(changes) => all_storage_changes.extend(changes),
                        Err(e) => {
                            warn!(txid = %r.transaction_id, error = ?e, "storage extraction failed")
                        }
                    }
                }
            }

            if !all_storage_changes.is_empty() {
                crate::helpers::storage_tracker::upsert_storage_changes(
                    &self.pool,
                    transform_height,
                    &all_storage_changes,
                )
                .await?;
            }
            let storage_elapsed_ms = storage_t0.elapsed().as_millis() as u64;
            info!(
                height = ctx.height,
                storage_updates = all_storage_changes.len(),
                elapsed_ms = storage_elapsed_ms,
                "storage indexing: done"
            );

            // Extract and index AMM trade events
            let amm_t0 = std::time::Instant::now();
            let mut all_trades = Vec::new();
            for r in &results {
                if !r.trace_events.is_empty() {
                    let ts_opt = r
                        .transaction_json
                        .get("status")
                        .and_then(|s| s.get("block_time"))
                        .and_then(|v| v.as_i64());
                    let ts = ts_opt
                        .and_then(|secs| chrono::Utc.timestamp_opt(secs, 0).single())
                        .unwrap_or_else(|| chrono::Utc.timestamp_opt(0, 0).single().unwrap());

                    match crate::helpers::amm_tracker::extract_trade_events(
                        &r.transaction_json,
                        &r.trace_events,
                        ts,
                        transform_height,
                    ) {
                        Ok(trades) => all_trades.extend(trades),
                        Err(e) => {
                            warn!(txid = %r.transaction_id, error = ?e, "trade extraction failed")
                        }
                    }
                }
            }

            if !all_trades.is_empty() {
                crate::helpers::amm_tracker::insert_trade_events(&self.pool, &all_trades).await?;
            }

            // Extract reserve snapshots from storage changes
            if !all_storage_changes.is_empty() {
                let ts_opt = results
                    .first()
                    .and_then(|r| r.transaction_json.get("status"))
                    .and_then(|s| s.get("block_time"))
                    .and_then(|v| v.as_i64());
                let ts = ts_opt
                    .and_then(|secs| chrono::Utc.timestamp_opt(secs, 0).single())
                    .unwrap_or_else(|| chrono::Utc.timestamp_opt(0, 0).single().unwrap());

                let reserves = crate::helpers::amm_tracker::extract_reserves_from_storage(
                    &all_storage_changes,
                    ts,
                    transform_height,
                );

                if !reserves.is_empty() {
                    crate::helpers::amm_tracker::insert_reserve_snapshots(&self.pool, &reserves)
                        .await?;
                }
            }

            let amm_elapsed_ms = amm_t0.elapsed().as_millis() as u64;
            info!(
                height = ctx.height,
                trade_events = all_trades.len(),
                elapsed_ms = amm_elapsed_ms,
                "AMM indexing: done"
            );

            // Aggregate candles periodically (every 10 blocks)
            if ctx.height % 10 == 0 && !all_trades.is_empty() {
                let candle_t0 = std::time::Instant::now();
                let start_time = chrono::Utc
                    .timestamp_opt((ctx.height as i64 - 600) * 600, 0)
                    .single()
                    .unwrap_or_else(|| chrono::Utc::now());
                let end_time = chrono::Utc::now();

                if let Err(e) =
                    crate::helpers::amm_tracker::aggregate_candles(&self.pool, start_time, end_time)
                        .await
                {
                    warn!(height = ctx.height, error = ?e, "candle aggregation failed");
                } else {
                    let candle_elapsed_ms = candle_t0.elapsed().as_millis() as u64;
                    info!(
                        height = ctx.height,
                        elapsed_ms = candle_elapsed_ms,
                        "candle aggregation: done"
                    );
                }
            }

            // Build inputs for PoolSwap / PoolCreation / PoolMint / PoolBurn indexers and run them
            let mut swap_inputs: Vec<(
                String,
                i32,
                chrono::DateTime<Utc>,
                serde_json::Value,
                Vec<serde_json::Value>,
            )> = Vec::new();
            let mut creation_inputs: Vec<(
                String,
                i32,
                chrono::DateTime<Utc>,
                serde_json::Value,
                Vec<serde_json::Value>,
            )> = Vec::new();
            let mut mint_inputs: Vec<(
                String,
                i32,
                chrono::DateTime<Utc>,
                serde_json::Value,
                Vec<serde_json::Value>,
            )> = Vec::new();
            let mut burn_inputs: Vec<(
                String,
                i32,
                chrono::DateTime<Utc>,
                serde_json::Value,
                Vec<serde_json::Value>,
            )> = Vec::new();
            let mut subfrost_inputs: Vec<(
                String,
                i32,
                chrono::DateTime<Utc>,
                serde_json::Value,
                Vec<serde_json::Value>,
            )> = Vec::new();
            for (tx_index, r) in results.iter().enumerate() {
                // All source transactions were proven to belong to this block and to carry this
                // exact timestamp before any transforms ran. Do not reparse with an epoch fallback.
                let ts = block_ts;
                let trace_events_json: Vec<serde_json::Value> = r
                    .trace_events
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "vout": e.vout,
                            "eventType": e.event_type,
                            "data": e.data,
                            "alkaneAddressBlock": e.alkane_address_block,
                            "alkaneAddressTx": e.alkane_address_tx,
                        })
                    })
                    .collect();
                swap_inputs.push((
                    r.transaction_id.clone(),
                    tx_index as i32,
                    ts,
                    r.transaction_json.clone(),
                    trace_events_json.clone(),
                ));
                creation_inputs.push((
                    r.transaction_id.clone(),
                    tx_index as i32,
                    ts,
                    r.transaction_json.clone(),
                    trace_events_json.clone(),
                ));
                mint_inputs.push((
                    r.transaction_id.clone(),
                    tx_index as i32,
                    ts,
                    r.transaction_json.clone(),
                    trace_events_json.clone(),
                ));
                burn_inputs.push((
                    r.transaction_id.clone(),
                    tx_index as i32,
                    ts,
                    r.transaction_json.clone(),
                    trace_events_json.clone(),
                ));
                subfrost_inputs.push((
                    r.transaction_id.clone(),
                    tx_index as i32,
                    ts,
                    r.transaction_json.clone(),
                    trace_events_json,
                ));
            }
            index_pool_swaps_for_block(&self.pool, transform_height, &swap_inputs).await?;

            let creations =
                index_pool_creations_for_block(&self.pool, transform_height, &creation_inputs)
                    .await?;
            if !creations.is_empty() {
                let mut dbtx = self.pool.begin().await?;
                replace_pool_creations(&mut dbtx, &all_txids, &creations).await?;
                dbtx.commit().await?;
            }

            // Index pool mints
            index_pool_mints_for_block(&self.pool, transform_height, &mint_inputs).await?;

            // Index pool burns
            index_pool_burns_for_block(&self.pool, transform_height, &burn_inputs).await?;

            // Index Subfrost wraps and unwraps
            index_subfrost_wraps_for_block(&self.pool, transform_height, &subfrost_inputs).await?;
            index_subfrost_unwraps_for_block(&self.pool, transform_height, &subfrost_inputs)
                .await?;
        }

        // Do not publish a block that ceased to be canonical while its transforms were running.
        if helper_get_block_hash(provider, ctx.height).await? != block_hash
            || (ctx.height > 0
                && helper_get_block_hash(provider, ctx.height - 1).await? != previous_block_hash)
        {
            return Err(anyhow!(
                "Bitcoin Core canonical hash changed before Marketplace publication"
            ));
        }

        canonical_commit::publish_height(
            &self.pool,
            ctx.height,
            &block_hash,
            &previous_block_hash,
            block_ts,
        )
        .await?;
        serial_guard.commit().await?;
        info!(height = ctx.height, %block_hash, "published canonical commitment, ProcessedBlocks, and position atomically");

        // Notify downstream services via Redis pub-sub only for realtime blocks (not during catch-up)
        if ctx.emit_publish {
            publish_block_processed(ctx.height).await;
        }

        // Return the block hash for position tracking
        Ok(block_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hash(byte: char) -> String {
        std::iter::repeat(byte).take(64).collect()
    }

    fn source_tx(txid: &str, block_hash: &str) -> serde_json::Value {
        json!({
            "txid": txid,
            "vin": [{"is_coinbase": true}],
            "vout": [{"scriptpubkey": "51", "value": 1}],
            "status": {
                "confirmed": true,
                "block_height": 100,
                "block_hash": block_hash,
                "block_time": 1_700_000_000
            }
        })
    }

    #[test]
    fn spend_staging_extracts_inputs_and_skips_coinbase() {
        let previous = std::iter::repeat('a').take(64).collect::<String>();
        let txs = vec![
            json!({"txid": "coinbase", "vin": [{"is_coinbase": true}]}),
            json!({"txid": "spend", "vin": [{
                "is_coinbase": false,
                "txid": previous,
                "vout": 7
            }]}),
        ];

        let outpoints = spent_outpoints_from_transactions(&txs).unwrap();
        assert_eq!(
            outpoints,
            vec![(std::iter::repeat('a').take(64).collect(), 7)]
        );
    }

    #[test]
    fn malformed_spend_input_fails_closed() {
        let malformed = vec![json!({
            "txid": "spend",
            "vin": [{"is_coinbase": false, "txid": "ABC", "vout": -1}]
        })];
        assert!(spent_outpoints_from_transactions(&malformed).is_err());
        assert!(spent_outpoints_from_transactions(&[json!({"txid": "missing"})]).is_err());
    }

    #[test]
    fn source_transaction_set_must_match_exactly() {
        let block_hash = hash('d');
        let a = hash('a');
        let b = hash('b');
        let c = hash('c');
        let expected = vec![a.clone(), b.clone()];
        assert!(
            verify_source_transactions(
                &expected,
                &[source_tx(&b, &block_hash), source_tx(&a, &block_hash)],
                100,
                &block_hash,
            )
            .is_ok()
        );
        assert!(
            verify_source_transactions(
                &expected,
                &[source_tx(&a, &block_hash), source_tx(&c, &block_hash)],
                100,
                &block_hash,
            )
            .is_err()
        );
        assert!(
            verify_source_transactions(
                &expected,
                &[source_tx(&a, &block_hash), source_tx(&a, &block_hash)],
                100,
                &block_hash,
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_or_wrong_block_source_transaction_fails_closed() {
        let block_hash = hash('d');
        let txid = hash('a');
        let expected = vec![txid.clone()];

        let mut missing_output = source_tx(&txid, &block_hash);
        missing_output["vout"] = json!([]);
        assert!(
            verify_source_transactions(&expected, &[missing_output], 100, &block_hash).is_err()
        );

        let mut bad_script = source_tx(&txid, &block_hash);
        bad_script["vout"][0]["scriptpubkey"] = json!("ABC");
        assert!(
            verify_source_transactions(&expected, &[bad_script], 100, &block_hash).is_err()
        );

        let mut wrong_block = source_tx(&txid, &block_hash);
        wrong_block["status"]["block_hash"] = json!(hash('e'));
        assert!(
            verify_source_transactions(&expected, &[wrong_block], 100, &block_hash).is_err()
        );

        let mut missing_time = source_tx(&txid, &block_hash);
        missing_time["status"].as_object_mut().unwrap().remove("block_time");
        assert!(
            verify_source_transactions(&expected, &[missing_time], 100, &block_hash).is_err()
        );
    }
}
