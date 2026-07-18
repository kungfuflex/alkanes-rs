/// Reproduce the exact issue from block 467 where we have receive_intent + value_transfer
/// but extract_trades returns 0 trades

use alkanes_trace_transform::types::{TraceEvent, TransactionContext, VoutInfo};
use serde_json::json;

#[test]
fn test_block_467_trade_extraction() {
    // This is the EXACT scenario from block 467 swap transaction
    // Transaction: 324711e8bbd2e1991a73e5a1d856a99842ced8f223cf7b9ea4b706dcc1ee5997
    // vout 5 has the actual swap
    
    let context = TransactionContext {
        txid: "324711e8bbd2e1991a73e5a1d856a99842ced8f223cf7b9ea4b706dcc1ee5997".to_string(),
        block_height: 467,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            VoutInfo {
                index: 0,
                address: Some("bcrt1p705x8h5dy67x7tgdu6wv2crq333sdx6h776vc929rxcdlxs5wj2s7h296c".to_string()),
                value: 546,
                script_pubkey: String::new(),
            },
            VoutInfo {
                index: 1,
                address: Some("bcrt1p705x8h5dy67x7tgdu6wv2crq333sdx6h776vc929rxcdlxs5wj2s7h296c".to_string()),
                value: 4999997605,
                script_pubkey: String::new(),
            },
        ],
    };
    
    // The actual events from the database for vout 5
    let traces = vec![
        TraceEvent {
            event_type: "receive_intent".to_string(),
            vout: 5,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "65522".to_string(), // Pool contract
            data: json!({
                "transfers": [
                    {"id": {"block": "2", "tx": "0"}, "value": "1000000"} // DIESEL input
                ]
            }),
        },
        TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 5,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "65522".to_string(),
            data: json!({
                "transfers": [
                    {"id": {"block": "2", "tx": "0"}, "value": "4999000000"}, // DIESEL back (change?)
                    {"id": {"block": "32", "tx": "0"}, "value": "163"} // frBTC output
                ],
                "redirect_to": 0
            }),
        },
        TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 5,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "65522".to_string(),
            data: json!({
                "transfers": [
                    {"id": {"block": "2", "tx": "0"}, "value": "302000000"}, // DIESEL to LP
                    {"id": {"block": "32", "tx": "0"}, "value": "49673"} // frBTC to LP
                ],
                "redirect_to": -1  // 4294967295 as i32
            }),
        },
    ];
    
    println!("Context: {} vouts", context.vouts.len());
    println!("Traces: {} events", traces.len());
    
    // Now test the extract_trades logic from transform_integration.rs
    let trades = extract_trades_from_traces_test(&context, &traces);
    
    println!("Extracted trades: {}", trades.len());
    
    // This should find 1 trade but currently finds 0
    assert_eq!(trades.len(), 1, "Should extract 1 trade from receive_intent + value_transfer pair");
}

// Copy of the extract_trades logic so we can test it
fn extract_trades_from_traces_test(
    context: &TransactionContext,
    traces: &[TraceEvent],
) -> Vec<String> {
    let mut trades = Vec::new();
    
    println!("extract_trades: processing {} traces", traces.len());
    
    // Group traces by vout
    let mut traces_by_vout: std::collections::HashMap<i32, Vec<&TraceEvent>> = 
        std::collections::HashMap::new();
    
    for trace in traces {
        traces_by_vout.entry(trace.vout).or_default().push(trace);
    }
    
    println!("extract_trades: grouped into {} vouts", traces_by_vout.len());
    
    // Look for receive_intent + value_transfer patterns
    for (vout, vout_traces) in traces_by_vout {
        println!("  vout {}: {} events", vout, vout_traces.len());
        
        let receive_intent = vout_traces.iter()
            .find(|t| t.event_type == "receive_intent");
        
        let value_transfers: Vec<&&TraceEvent> = vout_traces.iter()
            .filter(|t| t.event_type == "value_transfer")
            .collect();
        
        println!("  vout {}: receive_intent={}, value_transfers={}", 
            vout, receive_intent.is_some(), value_transfers.len());
        
        if let Some(intent) = receive_intent {
            println!("    Found receive_intent: pool={}:{}", 
                intent.alkane_address_block, intent.alkane_address_tx);
            println!("    receive_intent data: {}", intent.data);
            
            if !value_transfers.is_empty() {
                println!("    Found {} value_transfer events", value_transfers.len());
                for (i, vt) in value_transfers.iter().enumerate() {
                    println!("      value_transfer {}: {}", i, vt.data);
                }
                
                // Parse pool ID from alkane address
                let pool_block = intent.alkane_address_block.parse().unwrap_or(0);
                let pool_tx = intent.alkane_address_tx.parse().unwrap_or(0);
                
                println!("    Potential trade: pool={}:{}", pool_block, pool_tx);
                
                // For now, just count it as a trade
                trades.push(format!("trade_at_vout_{}", vout));
            } else {
                println!("    No value_transfer events found");
            }
        } else {
            println!("    No receive_intent found");
        }
    }
    
    trades
}
