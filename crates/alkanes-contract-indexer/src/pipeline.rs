use anyhow::Result;
use alkanes_cli_sys::SystemAlkanes as ConcreteProvider;
use alkanes_cli_common::traits::{DeezelProvider, JsonRpcProvider, BitcoinRpcProvider};
use sqlx::PgPool;
use tracing::{info, warn};
use crate::helpers::pools::{fetch_and_upsert_pools_for_tip};
use crate::helpers::notify::{notify_pools_processed, publish_block_processed};
use crate::helpers::block::{get_block_hash as helper_get_block_hash, get_block_txids as helper_get_block_txids, get_transactions_info as helper_get_transactions_info, tx_has_op_return};
use crate::helpers::protostone::decode_and_trace_for_block;
use crate::helpers::protostone::TxDecodeTraceResult;
use crate::db::transactions::{upsert_alkane_transactions, replace_trace_events, replace_decoded_protostones};
use crate::helpers::poolswap::index_pool_swaps_for_block;
use crate::helpers::poolcreate::index_pool_creations_for_block;
use crate::helpers::poolmint::index_pool_mints_for_block;
use crate::helpers::poolburn::index_pool_burns_for_block;
use crate::helpers::subfrost::{index_subfrost_wraps_for_block, index_subfrost_unwraps_for_block};
use crate::db::transactions::replace_pool_creations;
use chrono::{TimeZone, Utc};
use chrono::DateTime;
use std::time::Instant;
use crate::db::blocks::upsert_processed_block;
use crate::transform_integration::{TraceTransformService, convert_trace_event, convert_transaction_context};
use alkanes_trace_transform::types::VoutInfo;

#[derive(Clone, Debug)]
pub struct BlockContext {
	pub height: u64,
	pub emit_publish: bool,
}

#[derive(Clone, Debug)]
pub struct Pipeline {
	pool: PgPool,
	factory_block_id: String,
	factory_tx_id: String,
}

impl Pipeline {
	pub fn new(pool: PgPool, factory_block_id: String, factory_tx_id: String) -> Self {
		Self { pool, factory_block_id, factory_tx_id }
	}

	// Runs on every new tip height (even during catch-up)
	pub async fn fetch_pools_for_tip(&self, provider: &ConcreteProvider, tip_height: u64) -> Result<()> {
		let res = fetch_and_upsert_pools_for_tip(
			provider,
			&self.pool,
			&self.factory_block_id,
			&self.factory_tx_id,
			tip_height,
		).await;
		if res.is_ok() {
			notify_pools_processed(tip_height).await;
		}
		res
	}

	// Sequential per-block processing (historical and then following tip)
	pub async fn process_block_sequential<P>(&self, provider: &P, ctx: BlockContext) -> Result<()>
	where
		P: DeezelProvider + JsonRpcProvider + BitcoinRpcProvider + Send + Sync,
	{
		// Resolve block hash via bitcoind and print/log it
		let block_hash = helper_get_block_hash(provider, ctx.height).await?;
		info!(height = ctx.height, %block_hash, "resolved block hash");

		// Fetch txids for the block via JSON-RPC helper
		let txids = helper_get_block_txids(provider, &block_hash).await?;
		info!(height = ctx.height, count = txids.len(), "esplora_block::txids fetched");

		// Fetch tx infos concurrently using helper and maintain order
		let txs = helper_get_transactions_info(provider, &txids, 25).await?;
		info!(height = ctx.height, txs = txs.len(), "esplora_tx fetched");

		// Filter for OP_RETURN outputs
		let opret_count: usize = txs.iter().filter(|tx| tx_has_op_return(tx)).count();
		info!(height = ctx.height, op_return_txs = opret_count, "OP_RETURN transactions in block");

		// Build filtered list of OP_RETURN transactions only
		let op_return_txs: Vec<_> = txs.iter().filter(|tx| tx_has_op_return(tx)).cloned().collect();

		// Decode+trace protostones for this block (only OP_RETURN txs) with timing
		if !op_return_txs.is_empty() {
			let count = op_return_txs.len();
			let t0 = Instant::now();
			info!(height = ctx.height, op_return_txs = count, "decode_and_trace_for_block: start");
			let results: Vec<TxDecodeTraceResult> = decode_and_trace_for_block(provider, &op_return_txs, 32, 16).await?;

			// Prepare batch payloads
			let mut tx_rows: Vec<(i32, String, i32, bool, bool, serde_json::Value)> = Vec::with_capacity(results.len());
			let mut all_txids: Vec<String> = Vec::with_capacity(results.len());
            let mut protostone_rows: Vec<(String, i32, i32, i32, serde_json::Value)> = Vec::new();
            let mut event_rows: Vec<(String, i32, i32, String, serde_json::Value, String, String)> = Vec::new();

			for (tx_index, r) in results.iter().enumerate() {
				let txid = r.transaction_id.clone();
				all_txids.push(txid.clone());
				tx_rows.push((
					ctx.height as i32,
					txid.clone(),
					tx_index as i32,
					r.has_trace,
					r.trace_succeed,
					r.transaction_json.clone(),
				));
                for d in &r.decoded_protostones {
                    protostone_rows.push((txid.clone(), d.vout, d.protostone_index, ctx.height as i32, d.decoded.clone()));
				}
				for e in &r.trace_events {
					event_rows.push((
						txid.clone(),
                        ctx.height as i32,
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
			info!(height = ctx.height, op_return_txs = count, elapsed_ms, "decode_and_trace_for_block: done");

			// Process traces through transform pipeline
			let transform_t0 = std::time::Instant::now();
			let mut transform_service = TraceTransformService::new(self.pool.clone());
			// Load known pools at start of each block processing
			if let Err(e) = transform_service.load_existing_pools().await {
				warn!("Failed to load existing pools: {:?}", e);
			}
			info!("Transform pipeline: processing {} transactions from block {}", results.len(), ctx.height);
			for r in &results {
				info!("Transform pipeline: tx {} has {} trace_events", r.transaction_id, r.trace_events.len());
				if !r.trace_events.is_empty() {
					// Convert transaction info to context
					let tx_info = txs.iter().find(|tx| {
						tx.get("txid").and_then(|v| v.as_str()).unwrap_or("") == r.transaction_id
					});
					
					if let Some(tx) = tx_info {
						let timestamp = tx.get("status")
							.and_then(|s| s.get("block_time"))
							.and_then(|bt| bt.as_i64())
							.and_then(|bt| Utc.timestamp_opt(bt, 0).single())
							.unwrap_or_else(|| Utc::now());
						
						let vouts: Vec<VoutInfo> = tx.get("vout")
							.and_then(|v| v.as_array())
							.map(|arr| {
								arr.iter().enumerate().map(|(i, v)| {
									VoutInfo {
										index: i as i32,
										address: v.get("scriptpubkey_address").and_then(|a| a.as_str()).map(|s| s.to_string()),
										script_pubkey: v.get("scriptpubkey").and_then(|s| s.as_str()).unwrap_or("").to_string(),
										value: v.get("value").and_then(|val| val.as_u64()).unwrap_or(0),
									}
								}).collect()
							})
							.unwrap_or_default();
						
						let context = convert_transaction_context(
							r.transaction_id.clone(),
							ctx.height as i32,
							timestamp,
							vouts,
						);
						
						let traces: Vec<alkanes_trace_transform::types::TraceEvent> = r.trace_events.iter().map(|e| {
							convert_trace_event(
								e.event_type.clone(),
								e.vout,
								e.alkane_address_block.clone(),
								e.alkane_address_tx.clone(),
								e.data.clone(),
							)
						}).collect();
						
						if let Err(e) = transform_service.process_transaction(context, traces).await {
							warn!(txid = %r.transaction_id, error = ?e, "transform processing failed");
						}
					}
				}
			}
			let transform_elapsed_ms = transform_t0.elapsed().as_millis() as u64;
			info!(height = ctx.height, elapsed_ms = transform_elapsed_ms, "trace transform processing: done");

			// Extract and index balances from trace events
			let balance_t0 = std::time::Instant::now();
			let mut all_outpoint_balances = Vec::new();
			for r in &results {
				if !r.trace_events.is_empty() {
					match crate::helpers::balance_tracker::extract_balance_changes(
						&r.transaction_json,
						&r.trace_events
					) {
						Ok(balances) => all_outpoint_balances.extend(balances),
						Err(e) => warn!(txid = %r.transaction_id, error = ?e, "balance extraction failed"),
					}
				}
			}

			if !all_outpoint_balances.is_empty() {
				crate::helpers::balance_tracker::upsert_utxo_balances(
					&self.pool,
					ctx.height as i32,
					&all_outpoint_balances
				).await?;
				crate::helpers::balance_tracker::update_address_balances(
					&self.pool,
					&all_outpoint_balances
				).await?;
				crate::helpers::balance_tracker::refresh_holders_for_block(
					&self.pool,
					&all_outpoint_balances
				).await?;
			}
			let balance_elapsed_ms = balance_t0.elapsed().as_millis() as u64;
			info!(height = ctx.height, balance_updates = all_outpoint_balances.len(), elapsed_ms = balance_elapsed_ms, "balance indexing: done");

			// Extract and index storage changes from trace events
			let storage_t0 = std::time::Instant::now();
			let mut all_storage_changes = Vec::new();
			for r in &results {
				if !r.trace_events.is_empty() {
					match crate::helpers::storage_tracker::extract_storage_changes(
						&r.transaction_json,
						&r.trace_events
					) {
						Ok(changes) => all_storage_changes.extend(changes),
						Err(e) => warn!(txid = %r.transaction_id, error = ?e, "storage extraction failed"),
					}
				}
			}

			if !all_storage_changes.is_empty() {
				crate::helpers::storage_tracker::upsert_storage_changes(
					&self.pool,
					ctx.height as i32,
					&all_storage_changes
				).await?;
			}
			let storage_elapsed_ms = storage_t0.elapsed().as_millis() as u64;
			info!(height = ctx.height, storage_updates = all_storage_changes.len(), elapsed_ms = storage_elapsed_ms, "storage indexing: done");

			// Extract and index AMM trade events
			let amm_t0 = std::time::Instant::now();
			let mut all_trades = Vec::new();
			for r in &results {
				if !r.trace_events.is_empty() {
					let ts_opt = r.transaction_json.get("status")
						.and_then(|s| s.get("block_time"))
						.and_then(|v| v.as_i64());
					let ts = ts_opt
						.and_then(|secs| chrono::Utc.timestamp_opt(secs, 0).single())
						.unwrap_or_else(|| chrono::Utc.timestamp_opt(0, 0).single().unwrap());
					
					match crate::helpers::amm_tracker::extract_trade_events(
						&r.transaction_json,
						&r.trace_events,
						ts,
						ctx.height as i32
					) {
						Ok(trades) => all_trades.extend(trades),
						Err(e) => warn!(txid = %r.transaction_id, error = ?e, "trade extraction failed"),
					}
				}
			}

			if !all_trades.is_empty() {
				crate::helpers::amm_tracker::insert_trade_events(&self.pool, &all_trades).await?;
			}
			
			// Extract reserve snapshots from storage changes
			if !all_storage_changes.is_empty() {
				let ts_opt = results.first()
					.and_then(|r| r.transaction_json.get("status"))
					.and_then(|s| s.get("block_time"))
					.and_then(|v| v.as_i64());
				let ts = ts_opt
					.and_then(|secs| chrono::Utc.timestamp_opt(secs, 0).single())
					.unwrap_or_else(|| chrono::Utc.timestamp_opt(0, 0).single().unwrap());
				
				let reserves = crate::helpers::amm_tracker::extract_reserves_from_storage(
					&all_storage_changes,
					ts,
					ctx.height as i32
				);
				
				if !reserves.is_empty() {
					crate::helpers::amm_tracker::insert_reserve_snapshots(&self.pool, &reserves).await?;
				}
			}
			
			let amm_elapsed_ms = amm_t0.elapsed().as_millis() as u64;
			info!(height = ctx.height, trade_events = all_trades.len(), elapsed_ms = amm_elapsed_ms, "AMM indexing: done");

			// Aggregate candles periodically (every 10 blocks)
			if ctx.height % 10 == 0 && !all_trades.is_empty() {
				let candle_t0 = std::time::Instant::now();
				let start_time = chrono::Utc.timestamp_opt((ctx.height as i64 - 600) * 600, 0).single()
					.unwrap_or_else(|| chrono::Utc::now());
				let end_time = chrono::Utc::now();
				
				if let Err(e) = crate::helpers::amm_tracker::aggregate_candles(&self.pool, start_time, end_time).await {
					warn!(height = ctx.height, error = ?e, "candle aggregation failed");
				} else {
					let candle_elapsed_ms = candle_t0.elapsed().as_millis() as u64;
					info!(height = ctx.height, elapsed_ms = candle_elapsed_ms, "candle aggregation: done");
				}
			}

            // Build inputs for PoolSwap / PoolCreation / PoolMint / PoolBurn indexers and run them
			let mut swap_inputs: Vec<(String, i32, chrono::DateTime<Utc>, serde_json::Value, Vec<serde_json::Value>)> = Vec::new();
			let mut creation_inputs: Vec<(String, i32, chrono::DateTime<Utc>, serde_json::Value, Vec<serde_json::Value>)> = Vec::new();
            let mut mint_inputs: Vec<(String, i32, chrono::DateTime<Utc>, serde_json::Value, Vec<serde_json::Value>)> = Vec::new();
            let mut burn_inputs: Vec<(String, i32, chrono::DateTime<Utc>, serde_json::Value, Vec<serde_json::Value>)> = Vec::new();
            let mut subfrost_inputs: Vec<(String, i32, chrono::DateTime<Utc>, serde_json::Value, Vec<serde_json::Value>)> = Vec::new();
			for (tx_index, r) in results.iter().enumerate() {
				let ts_opt = r.transaction_json
					.get("status").and_then(|s| s.get("block_time")).and_then(|v| v.as_i64());
				let ts = ts_opt
					.and_then(|secs| Utc.timestamp_opt(secs, 0).single())
					.unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap());
				let trace_events_json: Vec<serde_json::Value> = r.trace_events.iter().map(|e| {
					serde_json::json!({
						"vout": e.vout,
						"eventType": e.event_type,
						"data": e.data,
						"alkaneAddressBlock": e.alkane_address_block,
						"alkaneAddressTx": e.alkane_address_tx,
					})
				}).collect();
                swap_inputs.push((r.transaction_id.clone(), tx_index as i32, ts, r.transaction_json.clone(), trace_events_json.clone()));
                creation_inputs.push((r.transaction_id.clone(), tx_index as i32, ts, r.transaction_json.clone(), trace_events_json.clone()));
                mint_inputs.push((r.transaction_id.clone(), tx_index as i32, ts, r.transaction_json.clone(), trace_events_json.clone()));
                burn_inputs.push((r.transaction_id.clone(), tx_index as i32, ts, r.transaction_json.clone(), trace_events_json.clone()));
                subfrost_inputs.push((r.transaction_id.clone(), tx_index as i32, ts, r.transaction_json.clone(), trace_events_json));
			}
			index_pool_swaps_for_block(&self.pool, ctx.height as i32, &swap_inputs).await?;

			let creations = index_pool_creations_for_block(&self.pool, ctx.height as i32, &creation_inputs).await?;
			if !creations.is_empty() {
				let mut dbtx = self.pool.begin().await?;
				replace_pool_creations(&mut dbtx, &all_txids, &creations).await?;
				dbtx.commit().await?;
			}

            // Index pool mints
            index_pool_mints_for_block(&self.pool, ctx.height as i32, &mint_inputs).await?;

            // Index pool burns
            index_pool_burns_for_block(&self.pool, ctx.height as i32, &burn_inputs).await?;

            // Index Subfrost wraps and unwraps
            index_subfrost_wraps_for_block(&self.pool, ctx.height as i32, &subfrost_inputs).await?;
            index_subfrost_unwraps_for_block(&self.pool, ctx.height as i32, &subfrost_inputs).await?;
		}

		// Determine block timestamp: use first tx's block_time if present, else now()
		let block_ts: DateTime<Utc> = txs.iter()
			.filter_map(|tx| tx.get("status").and_then(|s| s.get("block_time")).and_then(|v| v.as_i64()))
			.next()
			.and_then(|secs| Utc.timestamp_opt(secs, 0).single())
			.unwrap_or_else(|| Utc::now());


		// Record processed block marker
		upsert_processed_block(&self.pool, ctx.height as i32, &block_hash, block_ts).await?;
		info!(height = ctx.height, %block_hash, "recorded ProcessedBlocks entry");

		// Notify downstream services via Redis pub-sub only for realtime blocks (not during catch-up)
		if ctx.emit_publish {
			publish_block_processed(ctx.height).await;
		}

		Ok(())
	}
}


