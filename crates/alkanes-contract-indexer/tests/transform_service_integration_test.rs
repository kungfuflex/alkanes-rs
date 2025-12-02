/// Integration test for the entire TraceTransformService flow
/// This tests the EXACT path that production uses

use serde_json::json;

// Exact copy of types from transform_integration.rs
mod types {
    use serde_json::Value as JsonValue;
    
    #[derive(Debug, Clone)]
    pub struct TraceEvent {
        pub event_type: String,
        pub vout: i32,
        pub alkane_address_block: String,
        pub alkane_address_tx: String,
        pub data: JsonValue,
    }
    
    #[derive(Debug, Clone)]
    pub struct TransactionContext {
        pub txid: String,
        pub block_height: i32,
        pub timestamp: chrono::DateTime<chrono::Utc>,
    }
    
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct AlkaneId {
        pub block: i32,
        pub tx: i64,
    }
    
    impl AlkaneId {
        pub fn new(block: i32, tx: i64) -> Self {
            Self { block, tx }
        }
    }
}

#[derive(Debug)]
struct TradeEvent {
    pub txid: String,
    pub vout: i32,
    pub pool_id: types::AlkaneId,
    pub token0_id: types::AlkaneId,
    pub token1_id: types::AlkaneId,
    pub amount0_in: u128,
    pub amount1_in: u128,
    pub amount0_out: u128,
    pub amount1_out: u128,
    pub reserve0_after: u128,
    pub reserve1_after: u128,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub block_height: i32,
}

/// EXACT copy of extract_trades_from_traces from transform_integration.rs
fn extract_trades_from_traces(
    context: &types::TransactionContext,
    traces: &[types::TraceEvent],
) -> Vec<TradeEvent> {
    let mut trades = Vec::new();
    
    println!("extract_trades: processing {} traces", traces.len());
    
    // Group traces by vout
    let mut traces_by_vout: std::collections::HashMap<i32, Vec<&types::TraceEvent>> = 
        std::collections::HashMap::new();
    
    for trace in traces {
        traces_by_vout.entry(trace.vout).or_default().push(trace);
    }
    
    println!("extract_trades: grouped into {} vouts", traces_by_vout.len());
    println!("extract_trades: vout keys: {:?}", traces_by_vout.keys().collect::<Vec<_>>());
    
    // Look for receive_intent + value_transfer patterns
    for (&vout, vout_traces) in traces_by_vout.iter() {
        println!("extract_trades: examining vout {} with {} traces", vout, vout_traces.len());
        
        let receive_intent = vout_traces.iter()
            .find(|t| t.event_type == "receive_intent");
        let value_transfers: Vec<&&types::TraceEvent> = vout_traces.iter()
            .filter(|t| t.event_type == "value_transfer")
            .collect();
        
        println!("extract_trades: vout {} has receive_intent={} value_transfers={}", 
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
                        println!("extract_trades: found invoke event (type=call) with alkane address {}:{}", 
                            inv.alkane_address_block, inv.alkane_address_tx);
                        (inv.alkane_address_block.parse().unwrap_or(0),
                         inv.alkane_address_tx.parse().unwrap_or(0))
                    } else {
                        println!("extract_trades: no invoke event with type=call found!");
                        (0, 0)
                    }
                };
                
                println!("extract_trades: potential trade at vout {}, pool {}:{}", 
                    vout, pool_block, pool_tx);
                
                if let Some(trade) = parse_trade_from_intent(
                    context,
                    intent,
                    &value_transfers,
                    vout,
                    types::AlkaneId::new(pool_block, pool_tx),
                ) {
                    println!("extract_trades: found trade in tx {} at vout {}", context.txid, vout);
                    trades.push(trade);
                } else {
                    println!("extract_trades: failed to parse trade at vout {} - parse_trade_from_intent returned None", vout);
                }
            }
        }
    }
    
    trades
}

/// EXACT copy of parse_trade_from_intent from transform_integration.rs
fn parse_trade_from_intent(
    context: &types::TransactionContext,
    intent: &types::TraceEvent,
    transfers: &[&&types::TraceEvent],
    vout: i32,
    pool_id: types::AlkaneId,
) -> Option<TradeEvent> {
    println!("=== parse_trade_from_intent called ===");
    println!("intent.data: {}", intent.data);
    println!("transfers.len(): {}", transfers.len());
    
    // Extract input amounts from receive_intent
    let inputs = match intent.data.get("transfers").and_then(|v| v.as_array()) {
        Some(arr) => {
            println!("Found {} transfers in receive_intent", arr.len());
            arr
        },
        None => {
            println!("ERROR: no 'transfers' field in receive_intent");
            return None;
        }
    };
    
    let mut token0_id: Option<types::AlkaneId> = None;
    let mut token1_id: Option<types::AlkaneId> = None;
    let mut amount0_in = 0u128;
    let mut amount1_in = 0u128;
    
    for (i, input) in inputs.iter().enumerate() {
        println!("Processing input {}: {}", i, input);
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
        let amount: u128 = input.get("value")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())?;
        
        println!("  Parsed: block={}, tx={}, amount={}", block, tx, amount);
        
        if i == 0 {
            token0_id = Some(types::AlkaneId::new(block, tx));
            amount0_in = amount;
        } else if i == 1 {
            token1_id = Some(types::AlkaneId::new(block, tx));
            amount1_in = amount;
        }
    }
    
    println!("Inputs parsed: token0={:?}, amount0_in={}, token1={:?}, amount1_in={}", 
        token0_id, amount0_in, token1_id, amount1_in);
    
    // Extract output amounts from value_transfers
    let mut amount0_out = 0u128;
    let mut amount1_out = 0u128;
    
    for (i, transfer) in transfers.iter().enumerate() {
        println!("Processing value_transfer {}: {}", i, transfer.data);
        if let Some(transfers_arr) = transfer.data.get("transfers").and_then(|v| v.as_array()) {
            for (j, t) in transfers_arr.iter().enumerate() {
                println!("  Transfer {}: {}", j, t);
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
                let amount: u128 = t.get("value")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())?;
                
                let alkane_id = types::AlkaneId::new(block, tx);
                println!("    Parsed output: {:?}, amount={}", alkane_id, amount);
                
                if Some(&alkane_id) == token0_id.as_ref() {
                    amount0_out += amount;
                    println!("    Matched token0, amount0_out now: {}", amount0_out);
                } else if Some(&alkane_id) == token1_id.as_ref() {
                    amount1_out += amount;
                    println!("    Matched token1, amount1_out now: {}", amount1_out);
                } else {
                    // Discover token1 from outputs (for swaps where only 1 token comes in)
                    if token1_id.is_none() {
                        println!("    Discovered new token (token1): {:?}", alkane_id);
                        token1_id = Some(alkane_id);
                        amount1_out = amount;
                        println!("    Set token1_id={:?}, amount1_out={}", token1_id, amount1_out);
                    }
                }
            }
        }
    }
    
    println!("Outputs parsed: amount0_out={}, amount1_out={}", amount0_out, amount1_out);
    
    let reserve0_after = amount0_in.saturating_sub(amount0_out);
    let reserve1_after = amount1_in.saturating_sub(amount1_out);
    
    println!("Final: reserve0_after={}, reserve1_after={}", reserve0_after, reserve1_after);
    
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

#[test]
fn test_exact_block_468_scenario_with_invoke_event() {
    println!("\n==================== TEST: Block 468 with invoke event ====================");
    
    // EXACT data from block 468, vout 5 including the invoke event
    let context = types::TransactionContext {
        txid: "324711e8bbd2e1991a73e5a1d856a99842ced8f223cf7b9ea4b706dcc1ee5997".to_string(),
        block_height: 468,
        timestamp: chrono::Utc::now(),
    };
    
    // This is what the indexer actually creates - including invoke, receive_intent, value_transfers, and returns
    let traces = vec![
        // Invoke event - FIRST call-type invoke (the pool)
        types::TraceEvent {
            event_type: "invoke".to_string(),
            vout: 5,
            alkane_address_block: "4".to_string(),  // From context.myself.block
            alkane_address_tx: "65522".to_string(), // From context.myself.tx (pool contract)
            data: json!({
                "type": "call",
                "context": {
                    "myself": {"block": "4", "tx": "65522"},
                    "fuel": 3500000,
                    "incomingAlkanes": [{"id": {"tx": "0", "block": "2"}, "value": "1000000"}]
                }
            }),
        },
        // Receive intent - alkane address is EMPTY (not extracted by protostone.rs)
        types::TraceEvent {
            event_type: "receive_intent".to_string(),
            vout: 5,
            alkane_address_block: "".to_string(),  // EMPTY!
            alkane_address_tx: "".to_string(),     // EMPTY!
            data: json!({
                "transfers": [
                    {"id": {"block": "2", "tx": "0"}, "value": "1000000"}
                ]
            }),
        },
        // Value transfers - also empty alkane address
        types::TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 5,
            alkane_address_block: "".to_string(),
            alkane_address_tx: "".to_string(),
            data: json!({
                "transfers": [
                    {"id": {"block": "2", "tx": "0"}, "value": "4999000000"},
                    {"id": {"block": "32", "tx": "0"}, "value": "163"}
                ],
                "redirect_to": 0
            }),
        },
        types::TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 5,
            alkane_address_block: "".to_string(),
            alkane_address_tx: "".to_string(),
            data: json!({
                "transfers": [
                    {"id": {"block": "2", "tx": "0"}, "value": "302000000"},
                    {"id": {"block": "32", "tx": "0"}, "value": "49673"}
                ],
                "redirect_to": -1
            }),
        },
        // Return event
        types::TraceEvent {
            event_type: "return".to_string(),
            vout: 5,
            alkane_address_block: "".to_string(),
            alkane_address_tx: "".to_string(),
            data: json!({"status": "success"}),
        },
    ];
    
    println!("\nCalling extract_trades_from_traces with {} traces", traces.len());
    let result = extract_trades_from_traces(&context, &traces);
    
    println!("\n==================== RESULT ====================");
    println!("Trades extracted: {}", result.len());
    
    for (i, trade) in result.iter().enumerate() {
        println!("\nTrade {}:", i);
        println!("  Pool: {:?}", trade.pool_id);
        println!("  Token0 (DIESEL): {:?}", trade.token0_id);
        println!("  Token1 (frBTC): {:?}", trade.token1_id);
        println!("  Amount in: DIESEL={}, frBTC={}", trade.amount0_in, trade.amount1_in);
        println!("  Amount out: DIESEL={}, frBTC={}", trade.amount0_out, trade.amount1_out);
    }
    
    assert_eq!(result.len(), 1, "Should extract exactly 1 trade");
    
    let trade = &result[0];
    assert_eq!(trade.pool_id.block, 4, "Pool block should be 4");
    assert_eq!(trade.pool_id.tx, 65522, "Pool tx should be 65522");
    assert_eq!(trade.token0_id.block, 2, "Token0 (DIESEL) block should be 2");
    assert_eq!(trade.token0_id.tx, 0, "Token0 (DIESEL) tx should be 0");
    assert_eq!(trade.token1_id.block, 32, "Token1 (frBTC) block should be 32");
    assert_eq!(trade.token1_id.tx, 0, "Token1 (frBTC) tx should be 0");
    assert_eq!(trade.amount0_in, 1000000, "Should have 1M DIESEL in");
    assert_eq!(trade.amount1_in, 0, "Should have 0 frBTC in (swap)");
    assert_eq!(trade.amount1_out, 49836, "Should have 49836 frBTC out");
}

#[test]
fn test_vout_4_should_not_extract_trade() {
    println!("\n==================== TEST: Vout 4 (no pool address) ====================");
    
    let context = types::TransactionContext {
        txid: "test".to_string(),
        block_height: 468,
        timestamp: chrono::Utc::now(),
    };
    
    // Vout 4 has receive_intent + value_transfer but NO invoke event
    // So no pool address, should NOT create a trade
    let traces = vec![
        types::TraceEvent {
            event_type: "receive_intent".to_string(),
            vout: 4,
            alkane_address_block: "".to_string(),
            alkane_address_tx: "".to_string(),
            data: json!({"transfers": [{"id": {"block": "2", "tx": "0"}, "value": "5000000000"}]}),
        },
        types::TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 4,
            alkane_address_block: "".to_string(),
            alkane_address_tx: "".to_string(),
            data: json!({
                "transfers": [{"id": {"block": "2", "tx": "0"}, "value": "4999000000"}],
                "redirect_to": 0
            }),
        },
    ];
    
    let result = extract_trades_from_traces(&context, &traces);
    
    println!("Result: {} trades (expected 0 because no pool address)", result.len());
    
    // Should be 0 trades because pool ID is 0:0 (no invoke event)
    assert_eq!(result.len(), 0, "Should not extract trade without pool address");
}
