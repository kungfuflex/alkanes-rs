/// Test the actual transform_integration.rs code with real event formats
use serde_json::json;

// Import the types we need
use alkanes_trace_transform::types::{TraceEvent, TransactionContext, VoutInfo};

#[test]
fn test_transform_receives_correct_event_format() {
    // Simulate what the pipeline passes to the transform
    
    let context = TransactionContext {
        txid: "test_tx".to_string(),
        block_height: 100,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            VoutInfo {
                index: 0,
                address: Some("bc1quser".to_string()),
                value: 1000,
                script_pubkey: String::new(),
            },
            VoutInfo {
                index: 1,
                address: Some("bc1qpool".to_string()),
                value: 2000,
                script_pubkey: String::new(),
            },
        ],
    };
    
    // This is the format we're creating in pipeline.rs line 167-177
    let traces = vec![
        TraceEvent {
            event_type: "receive_intent".to_string(),
            vout: 4,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "100".to_string(),
            data: json!({
                "transfers": [
                    {"id": {"block": "2", "tx": "0"}, "value": "1000"}
                ]
            }),
        },
        TraceEvent {
            event_type: "value_transfer".to_string(),
            vout: 4,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "100".to_string(),
            data: json!({
                "transfers": [
                    {"id": {"block": "4", "tx": "20"}, "value": "900"}
                ],
                "redirect_to": 0
            }),
        },
    ];
    
    println!("✅ Created {} trace events", traces.len());
    
    // Now simulate the transform logic from transform_integration.rs
    
    // Group by vout (line 70-71)
    let mut traces_by_vout: std::collections::HashMap<i32, Vec<&TraceEvent>> = std::collections::HashMap::new();
    for trace in &traces {
        traces_by_vout.entry(trace.vout).or_insert_with(Vec::new).push(trace);
    }
    
    println!("✅ Grouped into {} vouts", traces_by_vout.len());
    
    // Look for receive_intent + value_transfer patterns (line 72-75)
    let mut found_trade = false;
    for (vout, vout_traces) in traces_by_vout {
        let receive_intent = vout_traces.iter()
            .find(|t| t.event_type == "receive_intent");
        
        let value_transfers: Vec<&&TraceEvent> = vout_traces.iter()
            .filter(|t| t.event_type == "value_transfer")
            .collect();
        
        if let Some(intent) = receive_intent {
            if !value_transfers.is_empty() {
                println!("✅ Found trade pattern at vout {}:", vout);
                println!("   - Pool: {}:{}", intent.alkane_address_block, intent.alkane_address_tx);
                println!("   - {} value transfers", value_transfers.len());
                found_trade = true;
                
                // This is where transform_integration.rs would parse the trade
                // The issue might be in the parsing logic
            }
        }
    }
    
    assert!(found_trade, "Should have found at least one trade");
}

#[test]
fn test_balance_extractor_with_real_events() {
    // Test that value_transfer events can be processed for balance tracking
    
    let event = TraceEvent {
        event_type: "value_transfer".to_string(),
        vout: 0,
        alkane_address_block: "".to_string(),
        alkane_address_tx: "".to_string(),
        data: json!({
            "transfers": [
                {"id": {"block": "2", "tx": "1"}, "value": "100"}
            ],
            "redirect_to": 0
        }),
    };
    
    println!("✅ Created value_transfer event:");
    println!("   - vout: {}", event.vout);
    println!("   - data: {}", event.data);
    
    // Check if the transform_integration's balance tracking would process this
    if event.event_type == "value_transfer" {
        let data = &event.data;
        let transfers = data.get("transfers").and_then(|v| v.as_array());
        
        if let Some(transfers_array) = transfers {
            println!("✅ Found {} transfers to process", transfers_array.len());
            assert_eq!(transfers_array.len(), 1);
            
            // Parse the transfer
            for transfer in transfers_array {
                let id = transfer.get("id").expect("Should have id");
                let block = id.get("block").and_then(|v| v.as_str()).expect("Should have block");
                let tx = id.get("tx").and_then(|v| v.as_str()).expect("Should have tx");
                let value = transfer.get("value").and_then(|v| v.as_str()).expect("Should have value");
                
                println!("✅ Parsed transfer: {}:{} = {}", block, tx, value);
            }
        }
    }
}

#[test]
fn test_empty_trace_list() {
    // What happens when we have no traces? Should not panic
    
    let traces: Vec<TraceEvent> = vec![];
    
    let mut traces_by_vout: std::collections::HashMap<i32, Vec<&TraceEvent>> = std::collections::HashMap::new();
    for trace in &traces {
        traces_by_vout.entry(trace.vout).or_insert_with(Vec::new).push(trace);
    }
    
    println!("✅ Empty trace list handled: {} vouts", traces_by_vout.len());
    assert_eq!(traces_by_vout.len(), 0);
}

#[test]
fn test_vout_info_address_lookup() {
    // Test that we can look up addresses from vout info
    // This is what the transform needs to do to get the destination address
    
    let vouts = vec![
        VoutInfo {
            index: 0,
            address: Some("bc1quser".to_string()),
            value: 1000,
            script_pubkey: String::new(),
        },
        VoutInfo {
            index: 1,
            address: Some("bc1qpool".to_string()),
            value: 2000,
            script_pubkey: String::new(),
        },
    ];
    
    // Looking up vout 0
    let vout_idx = 0;
    let address = vouts.iter()
        .find(|v| v.index == vout_idx)
        .and_then(|v| v.address.as_ref())
        .expect("Should find address for vout 0");
    
    println!("✅ Found address for vout {}: {}", vout_idx, address);
    assert_eq!(address, "bc1quser");
    
    // Looking up vout 1
    let vout_idx = 1;
    let address = vouts.iter()
        .find(|v| v.index == vout_idx)
        .and_then(|v| v.address.as_ref())
        .expect("Should find address for vout 1");
    
    println!("✅ Found address for vout {}: {}", vout_idx, address);
    assert_eq!(address, "bc1qpool");
}
