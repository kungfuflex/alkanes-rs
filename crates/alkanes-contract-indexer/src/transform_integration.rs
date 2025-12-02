use alkanes_trace_transform::*;
use anyhow::Result;
use sqlx::PgPool;

/// Optimized trace transform service using direct table writes
pub struct TraceTransformService {
    pool: PgPool,
    balance_processor: OptimizedBalanceProcessor,
    amm_tracker: OptimizedAmmTracker,
}

impl TraceTransformService {
    pub fn new(pool: PgPool) -> Self {
        let balance_processor = OptimizedBalanceProcessor::new(pool.clone());
        let amm_tracker = OptimizedAmmTracker::new(pool.clone());
        
        Self {
            pool,
            balance_processor,
            amm_tracker,
        }
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
        
        // Update extractor context
        self.balance_processor = OptimizedBalanceProcessor::with_context(self.pool.clone(), context.clone());
        
        // Process each trace
        for trace in &traces {
            tracing::info!("Transform: processing {} event at vout {}", trace.event_type, trace.vout);
            // Process balance changes
            if let Err(e) = self.balance_processor.process_trace(trace).await {
                tracing::warn!("Transform: balance processing failed for {} at vout {}: {:?}", trace.event_type, trace.vout, e);
            }
        }
        
        // Extract and process AMM trades
        let trades = extract_trades_from_traces(&context, &traces);
        let trade_count = trades.len();
        tracing::info!("Transform: extracted {} trades", trade_count);
        if !trades.is_empty() {
            self.amm_tracker.process_trades(trades).await?;
            tracing::debug!("Transform: processed {} trades", trade_count);
        }
        
        Ok(())
    }
}

/// Extract trades from traces (correlate receive_intent with value_transfer)
fn extract_trades_from_traces(
    context: &types::TransactionContext,
    traces: &[types::TraceEvent],
) -> Vec<TradeEvent> {
    let mut trades = Vec::new();
    
    tracing::info!("extract_trades: processing {} traces", traces.len());
    
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
                    // Fall back to invoke event which has the alkane address
                    // Look for "call" type invoke (not delegatecall or staticcall)
                    let invoke = vout_traces.iter().find(|t| {
                        t.event_type == "invoke" && 
                        t.data.get("type").and_then(|v| v.as_str()) == Some("call")
                    });
                    if let Some(inv) = invoke {
                        tracing::info!("extract_trades: found invoke event (type=call) with alkane address {}:{}", 
                            inv.alkane_address_block, inv.alkane_address_tx);
                        (inv.alkane_address_block.parse().unwrap_or(0),
                         inv.alkane_address_tx.parse().unwrap_or(0))
                    } else {
                        tracing::info!("extract_trades: no invoke event with type=call found!");
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
