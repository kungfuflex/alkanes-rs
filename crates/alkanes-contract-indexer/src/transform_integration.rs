use alkanes_trace_transform::*;
use anyhow::Result;
use sqlx::{PgPool, Row};
use std::collections::HashSet;

/// Hardcoded factory ID that creates pools
const FACTORY_BLOCK: i32 = 4;
const FACTORY_TX: i64 = 65522;

/// Optimized trace transform service using direct table writes
pub struct TraceTransformService {
    pool: PgPool,
    balance_processor: OptimizedBalanceProcessor,
    amm_tracker: OptimizedAmmTracker,
    /// Registry of pool addresses created by the factory
    pub known_pools: HashSet<types::AlkaneId>,
}

impl TraceTransformService {
    pub fn new(pool: PgPool) -> Self {
        let balance_processor = OptimizedBalanceProcessor::new(pool.clone());
        let amm_tracker = OptimizedAmmTracker::new(pool.clone());
        
        Self {
            pool,
            balance_processor,
            amm_tracker,
            known_pools: HashSet::new(),
        }
    }
    
    /// Initialize by loading existing pools from the Pool table
    pub async fn load_existing_pools(&mut self) -> Result<()> {
        // Query all pools from the Pool table using dynamic query
        let pools = sqlx::query(
            r#"SELECT DISTINCT "poolBlockId", "poolTxId" FROM "Pool""#
        )
        .fetch_all(&self.pool)
        .await?;
        
        for row in pools {
            let block_str: Option<String> = row.try_get("poolBlockId").ok();
            let tx_str: Option<String> = row.try_get("poolTxId").ok();
            
            if let (Some(block_str), Some(tx_str)) = (block_str, tx_str) {
                if let (Ok(block), Ok(tx)) = (block_str.parse::<i32>(), tx_str.parse::<i64>()) {
                    let pool_id = types::AlkaneId::new(block, tx);
                    self.known_pools.insert(pool_id);
                    tracing::info!("Loaded existing pool: {}:{}", block, tx);
                }
            }
        }
        
        tracing::info!("Loaded {} existing pools from database", self.known_pools.len());
        Ok(())
    }
    
    /// Apply the trace transform schema
    pub async fn apply_schema(&self) -> Result<()> {
        schema::apply_schema(&self.pool).await
    }
    
    /// Process traces from a transaction
    pub async fn process_transaction(
        &mut self,
        context: types::TransactionContext,
        traces: Vec<types::TraceEvent>,
    ) -> Result<()> {
        tracing::info!("Transform: processing tx {} with {} traces", context.txid, traces.len());
        
        if !traces.is_empty() {
            // Count event types
            let mut event_counts = std::collections::HashMap::new();
            for trace in &traces {
                *event_counts.entry(trace.event_type.as_str()).or_insert(0) += 1;
            }
            tracing::info!("Transform: event types: {:?}", event_counts);
        }
        
        // First pass: Track ALL create events and factory pools
        for trace in &traces {
            if trace.event_type == "create" {
                // Extract alkane ID from create event data
                // For create events, the new alkane ID is in data.newAlkane
                let alkane_id_opt = trace.data.get("newAlkane").and_then(|v| {
                    let block = v.get("block")?.as_str()?.parse::<i32>().ok()?;
                    let tx = v.get("tx")?.as_str()?.parse::<i64>().ok()?;
                    Some(types::AlkaneId::new(block, tx))
                });
                
                if let Some(alkane_id) = alkane_id_opt {
                    // Insert into TraceAlkane registry
                    if let Err(e) = self.insert_alkane_registry(alkane_id.clone(), &context).await {
                        tracing::warn!("Failed to insert alkane {}:{} to registry: {:?}", alkane_id.block, alkane_id.tx, e);
                    } else {
                        tracing::info!("Registered alkane: {}:{}", alkane_id.block, alkane_id.tx);
                    }
                    
                    // Check if this create is from the factory
                    let factory_invoke = traces.iter().any(|t| {
                        t.vout == trace.vout &&
                        t.event_type == "invoke" &&
                        t.alkane_address_block.parse::<i32>().ok() == Some(FACTORY_BLOCK) &&
                        t.alkane_address_tx.parse::<i64>().ok() == Some(FACTORY_TX)
                    });
                    
                    if factory_invoke {
                        if self.known_pools.insert(alkane_id.clone()) {
                            tracing::info!("Discovered new pool created by factory: {}:{}", alkane_id.block, alkane_id.tx);
                        }
                    }
                }
            }
        }
        
        // Update extractor context for balance processing
        // The OptimizedBalanceProcessor uses ValueTransferExtractor which now handles
        // both receive_intent and value_transfer events automatically
        self.balance_processor = OptimizedBalanceProcessor::with_context(self.pool.clone(), context.clone());
        
        // Process each trace
        for trace in &traces {
            // Process balance changes
            if let Err(e) = self.balance_processor.process_trace(trace).await {
                tracing::warn!("Transform: balance processing failed for {} at vout {}: {:?}", trace.event_type, trace.vout, e);
            }
        }
        
        // Extract and process AMM trades
        let trades = extract_trades_from_traces(&context, &traces, &self.known_pools);
        let trade_count = trades.len();
        tracing::info!("Transform: extracted {} trades", trade_count);
        if !trades.is_empty() {
            self.amm_tracker.process_trades(trades).await?;
            tracing::debug!("Transform: processed {} trades", trade_count);
        }
        
        Ok(())
    }
    
    /// Insert alkane into registry (TraceAlkane table)
    async fn insert_alkane_registry(
        &self,
        alkane_id: types::AlkaneId,
        context: &types::TransactionContext,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "TraceAlkane" 
                (alkane_block, alkane_tx, created_at_block, created_at_tx, created_at_height, created_at_timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (alkane_block, alkane_tx) DO NOTHING
            "#
        )
        .bind(alkane_id.block as i32)
        .bind(alkane_id.tx as i64)
        .bind(context.block_height as i32)
        .bind(&context.txid)
        .bind(context.block_height as i32)
        .bind(context.timestamp)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Create UTXO balance entry from ReceiveIntent event
    async fn create_utxo_from_receive_intent(
        &self,
        trace: &types::TraceEvent,
        context: &types::TransactionContext,
    ) -> Result<()> {
        tracing::info!("create_utxo_from_receive_intent: full data = {}", serde_json::to_string_pretty(&trace.data).unwrap_or_else(|_| "error".to_string()));
        
        // Parse ReceiveIntent event structure: incoming_alkanes array
        let incoming_alkanes = trace.data.get("incoming_alkanes")
            .or_else(|| trace.data.get("incomingAlkanes"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("No incoming_alkanes array in receive_intent"))?;
        
        // Get target vout info (the vout where this event occurred)
        let vout = trace.vout;
        let target_vout = context.vouts.get(vout as usize)
            .ok_or_else(|| anyhow::anyhow!("vout {} out of range", vout))?;
        
        let address = target_vout.address.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No address for vout {}", vout))?;
        let script_pubkey = &target_vout.script_pubkey;
        
        // Process each incoming alkane
        for alkane in incoming_alkanes {
            let alkane_id = alkane.get("id")
                .ok_or_else(|| anyhow::anyhow!("No id in incoming alkane"))?;
            let alkane_block = alkane_id.get("block")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("No block in alkane id"))? as i32;
            let alkane_tx = alkane_id.get("tx")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("No tx in alkane id"))?;
            
            // Amount is in U128 format with "lo" field
            let amount = alkane.get("value")
                .and_then(|v| v.get("lo"))
                .and_then(|v| v.as_i64())
                .or_else(|| alkane.get("amount")
                    .and_then(|v| v.as_i64()))
                .ok_or_else(|| anyhow::anyhow!("No amount in incoming alkane"))? as i64;
            
            // Insert UTXO balance
            sqlx::query(
                r#"
                INSERT INTO "TraceUtxoBalance"
                    (tx_hash, vout, alkane_block, alkane_tx, amount, address, script_pubkey,
                     created_block, created_tx, created_timestamp)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (tx_hash, vout, alkane_block, alkane_tx) DO NOTHING
                "#
            )
            .bind(&context.txid)
            .bind(vout)
            .bind(alkane_block)
            .bind(alkane_tx)
            .bind(amount)
            .bind(address)
            .bind(script_pubkey)
            .bind(context.block_height as i32)
            .bind(&context.txid)
            .bind(context.timestamp)
            .execute(&self.pool)
            .await?;
            
            // Also update aggregate balance
            sqlx::query(
                r#"
                INSERT INTO "TraceAlkaneBalance"
                    (address, alkane_block, alkane_tx, balance, last_updated_block, last_updated_tx, last_updated_timestamp)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (address, alkane_block, alkane_tx)
                DO UPDATE SET
                    balance = "TraceAlkaneBalance".balance + $4,
                    last_updated_block = $5,
                    last_updated_tx = $6,
                    last_updated_timestamp = $7
                "#
            )
            .bind(address)
            .bind(alkane_block)
            .bind(alkane_tx)
            .bind(amount)
            .bind(context.block_height as i32)
            .bind(&context.txid)
            .bind(context.timestamp)
            .execute(&self.pool)
            .await?;
        }
        
        Ok(())
    }
    
    /// Create UTXO balance entry from ValueTransfer event
    async fn create_utxo_balance(
        &self,
        trace: &types::TraceEvent,
        context: &types::TransactionContext,
    ) -> Result<()> {
        // Parse ValueTransfer event structure
        let transfers = trace.data.get("transfers")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("No transfers array in value_transfer"))?;
        
        let redirect_to = trace.data.get("redirect_to")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("No redirect_to in value_transfer"))? as i32;
        
        // Get target vout info
        let target_vout = context.vouts.get(redirect_to as usize)
            .ok_or_else(|| anyhow::anyhow!("redirect_to {} out of range", redirect_to))?;
        
        let address = target_vout.address.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No address for vout {}", redirect_to))?;
        let script_pubkey = &target_vout.script_pubkey;
        
        // Process each transfer
        for transfer in transfers {
            let alkane_id = transfer.get("id")
                .ok_or_else(|| anyhow::anyhow!("No id in transfer"))?;
            let alkane_block = alkane_id.get("block")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("No block in alkane id"))? as i32;
            let alkane_tx = alkane_id.get("tx")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("No tx in alkane id"))?;
            
            // Amount is in U128 format with "lo" field
            let amount = transfer.get("value")
                .and_then(|v| v.get("lo"))
                .and_then(|v| v.as_i64())
                .or_else(|| transfer.get("amount")
                    .and_then(|v| v.as_i64()))
                .ok_or_else(|| anyhow::anyhow!("No amount in transfer"))? as i64;
            
            // Insert UTXO balance
            sqlx::query(
                r#"
                INSERT INTO "TraceUtxoBalance"
                    (tx_hash, vout, alkane_block, alkane_tx, amount, address, script_pubkey,
                     created_block, created_tx, created_timestamp)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (tx_hash, vout, alkane_block, alkane_tx) DO NOTHING
                "#
            )
            .bind(&context.txid)
            .bind(redirect_to)
            .bind(alkane_block)
            .bind(alkane_tx)
            .bind(amount)
            .bind(address)
            .bind(script_pubkey)
            .bind(context.block_height as i32)
            .bind(&context.txid)
            .bind(context.timestamp)
            .execute(&self.pool)
            .await?;
            
            // Also update aggregate balance
            sqlx::query(
                r#"
                INSERT INTO "TraceAlkaneBalance"
                    (address, alkane_block, alkane_tx, balance, last_updated_block, last_updated_tx, last_updated_timestamp)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (address, alkane_block, alkane_tx)
                DO UPDATE SET
                    balance = "TraceAlkaneBalance".balance + $4,
                    last_updated_block = $5,
                    last_updated_tx = $6,
                    last_updated_timestamp = $7
                "#
            )
            .bind(address)
            .bind(alkane_block)
            .bind(alkane_tx)
            .bind(amount)
            .bind(context.block_height as i32)
            .bind(&context.txid)
            .bind(context.timestamp)
            .execute(&self.pool)
            .await?;
        }
        
        Ok(())
    }
    
    /// Process balance changes from receive_intent and value_transfer events
    /// This method is being phased out in favor of using the library's ValueTransferExtractor
    /// which now handles both event types.
    async fn process_balance_change(
        &self,
        trace: &types::TraceEvent,
        context: &types::TransactionContext,
    ) -> Result<()> {
        // NOTE: This is legacy code. The balance_processor now handles both
        // receive_intent and value_transfer through ValueTransferExtractor.
        // Keeping this for backwards compatibility but it's no longer needed.
        return Ok(());
        
        // Handle ValueTransfer with UTXO tracking
        if trace.event_type == "value_transfer" {
            return self.create_utxo_balance(trace, context).await;
        }
        
        // Handle ReceiveIntent with UTXO tracking
        if trace.event_type == "receive_intent" {
            return self.create_utxo_from_receive_intent(trace, context).await;
        }
        
        // Extract alkane ID - skip if not present or empty
        if trace.alkane_address_block.is_empty() || trace.alkane_address_tx.is_empty() {
            return Ok(());
        }
        
        let alkane_block = trace.alkane_address_block.parse::<i32>()?;
        let alkane_tx = trace.alkane_address_tx.parse::<i64>()?;
        
        // Extract amount and addresses based on event type
        let (address, amount_change) = match trace.event_type.as_str() {
            "receive_intent" => {
                // Recipient receives alkanes
                let recipient = trace.data.get("recipient")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("No recipient in receive_intent"))?;
                
                let amount = trace.data.get("amount")
                    .or_else(|| trace.data.get("value"))
                    .and_then(|v| {
                        if let Some(s) = v.as_str() {
                            s.parse::<i64>().ok()
                        } else {
                            v.as_i64()
                        }
                    })
                    .ok_or_else(|| anyhow::anyhow!("No amount in receive_intent"))?;
                
                (recipient.to_string(), amount)
            }
            "value_transfer" => {
                // For value_transfer, we need both from and to
                // For now, just track the "to" address with positive change
                let to_addr = trace.data.get("to")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("No to in value_transfer"))?;
                
                let amount = trace.data.get("value")
                    .or_else(|| trace.data.get("amount"))
                    .and_then(|v| {
                        if let Some(s) = v.as_str() {
                            s.parse::<i64>().ok()
                        } else {
                            v.as_i64()
                        }
                    })
                    .ok_or_else(|| anyhow::anyhow!("No value in value_transfer"))?;
                
                (to_addr.to_string(), amount)
            }
            _ => return Ok(()),
        };
        
        // Upsert into TraceAlkaneBalance
        sqlx::query(
            r#"
            INSERT INTO "TraceAlkaneBalance"
                (address, alkane_block, alkane_tx, balance, last_updated_block, last_updated_tx, last_updated_timestamp)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (address, alkane_block, alkane_tx)
            DO UPDATE SET
                balance = "TraceAlkaneBalance".balance + $4,
                last_updated_block = $5,
                last_updated_tx = $6,
                last_updated_timestamp = $7
            "#
        )
        .bind(&address)
        .bind(alkane_block)
        .bind(alkane_tx)
        .bind(amount_change)
        .bind(context.block_height as i32)
        .bind(&context.txid)
        .bind(context.timestamp)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

/// Extract trades from traces (correlate receive_intent with value_transfer)
/// Only tracks swaps on pools in the known_pools registry (created by factory)
fn extract_trades_from_traces(
    context: &types::TransactionContext,
    traces: &[types::TraceEvent],
    known_pools: &HashSet<types::AlkaneId>,
) -> Vec<TradeEvent> {
    let mut trades = Vec::new();
    
    tracing::info!("extract_trades: processing {} traces with {} known pools", traces.len(), known_pools.len());
    
    // Group traces by vout
    let mut traces_by_vout: std::collections::HashMap<i32, Vec<&types::TraceEvent>> = 
        std::collections::HashMap::new();
    
    for trace in traces {
        traces_by_vout.entry(trace.vout).or_default().push(trace);
    }
    
    tracing::info!("extract_trades: grouped into {} vouts", traces_by_vout.len());
    for (&vout, traces) in traces_by_vout.iter() {
        tracing::info!("extract_trades: vout {} has {} traces", vout, traces.len());
    }
    
    // Look for receive_intent + value_transfer patterns
    for (&vout, vout_traces) in traces_by_vout.iter() {
        tracing::info!("extract_trades: examining vout {} with {} traces", vout, vout_traces.len());
        
        let receive_intent = vout_traces.iter()
            .find(|t| t.event_type == "receive_intent");
        let value_transfers: Vec<&&types::TraceEvent> = vout_traces.iter()
            .filter(|t| t.event_type == "value_transfer")
            .collect();
        
        tracing::info!("extract_trades: vout {} has receive_intent={} value_transfers={}", 
            vout, receive_intent.is_some(), value_transfers.len());
        
        if let Some(intent) = receive_intent {
            if !value_transfers.is_empty() {
                // Parse pool ID from alkane address - try intent first, fall back to invoke event
                let (pool_block, pool_tx) = if !intent.alkane_address_block.is_empty() {
                    (intent.alkane_address_block.parse().unwrap_or(0), 
                     intent.alkane_address_tx.parse().unwrap_or(0))
                } else {
                    // Find invoke event on a KNOWN POOL (created by factory)
                    // This ensures we track swaps on pools, not on the factory/router
                    let pool_invoke = vout_traces.iter().find(|t| {
                        if t.event_type != "invoke" {
                            return false;
                        }
                        if t.data.get("type").and_then(|v| v.as_str()) != Some("call") {
                            return false;
                        }
                        
                        // Parse the alkane address
                        let block = t.alkane_address_block.parse::<i32>().unwrap_or(0);
                        let tx = t.alkane_address_tx.parse::<i64>().unwrap_or(0);
                        let addr = types::AlkaneId::new(block, tx);
                        
                        // Check if this address is a known pool
                        known_pools.contains(&addr)
                    });
                    
                    if let Some(inv) = pool_invoke {
                        tracing::info!("extract_trades: found pool invoke ({}:{}) - this is a registered pool", 
                            inv.alkane_address_block, inv.alkane_address_tx);
                        (inv.alkane_address_block.parse().unwrap_or(0),
                         inv.alkane_address_tx.parse().unwrap_or(0))
                    } else {
                        tracing::info!("extract_trades: no invoke on known pools found (only factory/router invokes)");
                        (0, 0)
                    }
                };
                
                tracing::info!("extract_trades: potential trade at vout {}, pool {}:{}", 
                    vout, pool_block, pool_tx);
                
                if let Some(trade) = parse_trade_from_intent(
                    context,
                    intent,
                    &value_transfers,
                    vout,
                    types::AlkaneId::new(pool_block, pool_tx),
                ) {
                    tracing::info!("extract_trades: found trade in tx {} at vout {}", context.txid, vout);
                    trades.push(trade);
                } else {
                    tracing::info!("extract_trades: failed to parse trade at vout {} - parse_trade_from_intent returned None", vout);
                }
            }
        }
    }
    
    trades
}

/// Parse a trade event from receive_intent and value_transfers
fn parse_trade_from_intent(
    context: &types::TransactionContext,
    intent: &types::TraceEvent,
    transfers: &[&&types::TraceEvent],
    vout: i32,
    pool_id: types::AlkaneId,
) -> Option<TradeEvent> {
    // Extract input amounts from receive_intent
    // The field is "transfers" not "inputs"
    tracing::info!("parse_trade: intent data: {}", intent.data);
    let inputs = match intent.data.get("transfers").and_then(|v| v.as_array()) {
        Some(arr) => {
            tracing::info!("parse_trade: found {} transfers in receive_intent", arr.len());
            arr
        },
        None => {
            tracing::warn!("parse_trade: no 'transfers' field in receive_intent");
            return None;
        }
    };
    
    let mut token0_id: Option<types::AlkaneId> = None;
    let mut token1_id: Option<types::AlkaneId> = None;
    let mut amount0_in = 0u128;
    let mut amount1_in = 0u128;
    
    for (i, input) in inputs.iter().enumerate() {
        let id_obj = input.get("id")?;
        // block and tx can be either strings or numbers
        let block: i32 = id_obj.get("block")
            .and_then(|v| {
                v.as_str().and_then(|s| s.parse().ok())
                    .or_else(|| v.as_i64().map(|n| n as i32))
            })?;
        let tx: i64 = id_obj.get("tx")
            .and_then(|v| {
                v.as_str().and_then(|s| s.parse().ok())
                    .or_else(|| v.as_i64())
            })?;
        // The field is "value" not "amount"
        let amount: u128 = input.get("value")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())?;
        
        if i == 0 {
            token0_id = Some(types::AlkaneId::new(block, tx));
            amount0_in = amount;
        } else if i == 1 {
            token1_id = Some(types::AlkaneId::new(block, tx));
            amount1_in = amount;
        }
    }
    
    // Extract output amounts from value_transfers
    let mut amount0_out = 0u128;
    let mut amount1_out = 0u128;
    
    for transfer in transfers {
        if let Some(transfers_arr) = transfer.data.get("transfers").and_then(|v| v.as_array()) {
            for t in transfers_arr {
                let id_obj = t.get("id")?;
                // block and tx can be either strings or numbers
                let block: i32 = id_obj.get("block")
                    .and_then(|v| {
                        v.as_str().and_then(|s| s.parse().ok())
                            .or_else(|| v.as_i64().map(|n| n as i32))
                    })?;
                let tx: i64 = id_obj.get("tx")
                    .and_then(|v| {
                        v.as_str().and_then(|s| s.parse().ok())
                            .or_else(|| v.as_i64())
                    })?;
                // The field is "value" not "amount"
                let amount: u128 = t.get("value")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())?;
                
                let alkane_id = types::AlkaneId::new(block, tx);
                if Some(&alkane_id) == token0_id.as_ref() {
                    amount0_out += amount;
                } else if Some(&alkane_id) == token1_id.as_ref() {
                    amount1_out += amount;
                } else {
                    // Discover token1 from outputs (for swaps where only 1 token comes in)
                    if token1_id.is_none() {
                        tracing::info!("parse_trade: discovered token1 from output: {}:{}", block, tx);
                        token1_id = Some(alkane_id);
                        amount1_out = amount;
                    }
                }
            }
        }
    }
    
    // Calculate reserves (simplified - would need pool state)
    let reserve0_after = amount0_in.saturating_sub(amount0_out);
    let reserve1_after = amount1_in.saturating_sub(amount1_out);
    
    Some(TradeEvent {
        txid: context.txid.clone(),
        vout,
        pool_id,
        token0_id: token0_id?,
        token1_id: token1_id?,
        amount0_in,
        amount1_in,
        amount0_out,
        amount1_out,
        reserve0_after,
        reserve1_after,
        timestamp: context.timestamp,
        block_height: context.block_height,
    })
}

/// Convert from indexer's trace format to transform types
pub fn convert_trace_event(
    event_type: String,
    vout: i32,
    alkane_block: String,
    alkane_tx: String,
    data: serde_json::Value,
) -> types::TraceEvent {
    types::TraceEvent {
        event_type,
        vout,
        alkane_address_block: alkane_block,
        alkane_address_tx: alkane_tx,
        data,
    }
}

/// Convert transaction context
pub fn convert_transaction_context(
    txid: String,
    block_height: i32,
    timestamp: chrono::DateTime<chrono::Utc>,
    vouts: Vec<types::VoutInfo>,
) -> types::TransactionContext {
    types::TransactionContext {
        txid,
        block_height,
        timestamp,
        vouts,
    }
}
