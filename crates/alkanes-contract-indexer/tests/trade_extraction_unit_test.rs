/// Unit test for parse_trade_from_intent with actual block 467 data
/// This isolates the exact bug without needing Docker

use serde_json::json;

// Copy the exact data structures from the indexer
#[derive(Debug, Clone)]
struct TraceEvent {
    pub event_type: String,
    pub vout: i32,
    pub alkane_address_block: String,
    pub alkane_address_tx: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone)]
struct TransactionContext {
    pub txid: String,
    pub block_height: i32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AlkaneId {
    pub block: i32,
    pub tx: i64,
}

impl AlkaneId {
    fn new(block: i32, tx: i64) -> Self {
        Self { block, tx }
    }
}

#[derive(Debug)]
struct TradeEvent {
    pub txid: String,
    pub vout: i32,
    pub pool_id: AlkaneId,
    pub token0_id: AlkaneId,
    pub token1_id: AlkaneId,
    pub amount0_in: u128,
    pub amount1_in: u128,
    pub amount0_out: u128,
    pub amount1_out: u128,
    pub reserve0_after: u128,
    pub reserve1_after: u128,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub block_height: i32,
}

/// This is the EXACT function from transform_integration.rs
fn parse_trade_from_intent(
    context: &TransactionContext,
    intent: &TraceEvent,
    transfers: &[&&TraceEvent],
    vout: i32,
    pool_id: AlkaneId,
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
    
    let mut token0_id: Option<AlkaneId> = None;
    let mut token1_id: Option<AlkaneId> = None;
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
            token0_id = Some(AlkaneId::new(block, tx));
            amount0_in = amount;
        } else if i == 1 {
            token1_id = Some(AlkaneId::new(block, tx));
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
                
                let alkane_id = AlkaneId::new(block, tx);
                println!("    Parsed output: {:?}, amount={}", alkane_id, amount);
                
                if Some(&alkane_id) == token0_id.as_ref() {
                    amount0_out += amount;
                    println!("    Matched token0, amount0_out now: {}", amount0_out);
                } else if Some(&alkane_id) == token1_id.as_ref() {
                    amount1_out += amount;
                    println!("    Matched token1, amount1_out now: {}", amount1_out);
                } else {
                    // This is a NEW token not in inputs - assign it to token1
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
fn test_parse_trade_with_block_467_data() {
    // EXACT data from block 467, vout 5
    let context = TransactionContext {
        txid: "324711e8bbd2e1991a73e5a1d856a99842ced8f223cf7b9ea4b706dcc1ee5997".to_string(),
        block_height: 467,
        timestamp: chrono::Utc::now(),
    };
    
    let intent = TraceEvent {
        event_type: "receive_intent".to_string(),
        vout: 5,
        alkane_address_block: "4".to_string(),
        alkane_address_tx: "65522".to_string(),
        data: json!({
            "transfers": [
                {"id": {"block": "2", "tx": "0"}, "value": "1000000"}
            ]
        }),
    };
    
    let vt1 = TraceEvent {
        event_type: "value_transfer".to_string(),
        vout: 5,
        alkane_address_block: "4".to_string(),
        alkane_address_tx: "65522".to_string(),
        data: json!({
            "transfers": [
                {"id": {"block": "2", "tx": "0"}, "value": "4999000000"},
                {"id": {"block": "32", "tx": "0"}, "value": "163"}
            ],
            "redirect_to": 0
        }),
    };
    
    let vt2 = TraceEvent {
        event_type: "value_transfer".to_string(),
        vout: 5,
        alkane_address_block: "4".to_string(),
        alkane_address_tx: "65522".to_string(),
        data: json!({
            "transfers": [
                {"id": {"block": "2", "tx": "0"}, "value": "302000000"},
                {"id": {"block": "32", "tx": "0"}, "value": "49673"}
            ],
            "redirect_to": -1
        }),
    };
    
    let transfers_refs: Vec<&TraceEvent> = vec![&vt1, &vt2];
    let transfers: Vec<&&TraceEvent> = transfers_refs.iter().collect();
    
    let pool_id = AlkaneId::new(4, 65522);
    
    println!("\n==================== STARTING TEST ====================");
    let result = parse_trade_from_intent(&context, &intent, &transfers, 5, pool_id);
    
    match &result {
        Some(trade) => {
            println!("\n==================== SUCCESS ====================");
            println!("Trade extracted successfully!");
            println!("  Token0 (DIESEL): {:?}", trade.token0_id);
            println!("  Token1 (frBTC): {:?}", trade.token1_id);
            println!("  Amount in: DIESEL={}, frBTC={}", trade.amount0_in, trade.amount1_in);
            println!("  Amount out: DIESEL={}, frBTC={}", trade.amount0_out, trade.amount1_out);
            println!("  Reserves after: DIESEL={}, frBTC={}", trade.reserve0_after, trade.reserve1_after);
        },
        None => {
            println!("\n==================== FAILED ====================");
            println!("parse_trade_from_intent returned None!");
        }
    }
    
    assert!(result.is_some(), "Should successfully parse trade from block 467 data");
    
    let trade = result.unwrap();
    assert_eq!(trade.token0_id.block, 2);
    assert_eq!(trade.token0_id.tx, 0);
    assert_eq!(trade.amount0_in, 1000000);
}
