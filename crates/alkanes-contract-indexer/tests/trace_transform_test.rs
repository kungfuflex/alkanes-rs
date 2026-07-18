/// Test the trace transform integration with real event formats from the database
use serde_json::json;

#[test]
fn test_value_transfer_event_parsing() {
    // This is the ACTUAL format we're getting from the database
    let event_data = json!({
        "transfers": [
            {"id": {"tx": "1", "block": "2"}, "value": "5"}
        ],
        "redirect_to": 0
    });
    
    // Extract the transfers array
    let transfers = event_data.get("transfers")
        .and_then(|v| v.as_array())
        .expect("Should have transfers array");
    
    assert_eq!(transfers.len(), 1);
    
    // Parse the first transfer
    let transfer = &transfers[0];
    let id = transfer.get("id").expect("Should have id");
    let block_str = id.get("block").and_then(|v| v.as_str()).expect("Should have block");
    let tx_str = id.get("tx").and_then(|v| v.as_str()).expect("Should have tx");
    let value_str = transfer.get("value").and_then(|v| v.as_str()).expect("Should have value");
    
    println!("✅ Parsed transfer:");
    println!("   - Alkane: {}:{}", block_str, tx_str);
    println!("   - Value: {}", value_str);
    
    // Parse to integers
    let block: u32 = block_str.parse().expect("Block should parse to u32");
    let tx: u64 = tx_str.parse().expect("TX should parse to u64");
    let value: u128 = value_str.parse().expect("Value should parse to u128");
    
    assert_eq!(block, 2);
    assert_eq!(tx, 1);
    assert_eq!(value, 5);
}

#[test]
fn test_receive_intent_event_parsing() {
    // This is the ACTUAL format for receive_intent events
    let event_data = json!({
        "transfers": []
    });
    
    let transfers = event_data.get("transfers")
        .and_then(|v| v.as_array())
        .expect("Should have transfers array");
    
    assert_eq!(transfers.len(), 0);
    println!("✅ ReceiveIntent event has {} transfers (empty is valid for mints)", transfers.len());
}

#[test]
fn test_trade_extraction_logic() {
    // Simulate what the transform code should do:
    // Find receive_intent + value_transfer pairs for trades
    
    let events = vec![
        json!({
            "eventType": "receive_intent",
            "vout": 4,
            "data": {
                "transfers": [
                    {"id": {"block": "2", "tx": "0"}, "value": "1000"}
                ]
            },
            "alkaneAddressBlock": "4",
            "alkaneAddressTx": "100"
        }),
        json!({
            "eventType": "value_transfer",
            "vout": 4,
            "data": {
                "transfers": [
                    {"id": {"block": "4", "tx": "20"}, "value": "900"}
                ],
                "redirect_to": 0
            },
            "alkaneAddressBlock": "4",
            "alkaneAddressTx": "100"
        }),
    ];
    
    // Group by vout
    let mut by_vout: std::collections::HashMap<i32, Vec<&serde_json::Value>> = std::collections::HashMap::new();
    for event in &events {
        let vout = event.get("vout").and_then(|v| v.as_i64()).unwrap() as i32;
        by_vout.entry(vout).or_insert_with(Vec::new).push(event);
    }
    
    println!("✅ Grouped events by vout:");
    for (vout, events) in &by_vout {
        println!("   vout {}: {} events", vout, events.len());
    }
    
    // Look for receive_intent + value_transfer pairs
    for (vout, vout_events) in by_vout {
        let receive_intent = vout_events.iter()
            .find(|e| e.get("eventType").and_then(|v| v.as_str()) == Some("receive_intent"));
        
        let value_transfers: Vec<_> = vout_events.iter()
            .filter(|e| e.get("eventType").and_then(|v| v.as_str()) == Some("value_transfer"))
            .collect();
        
        if let Some(intent) = receive_intent {
            if !value_transfers.is_empty() {
                println!("✅ Found potential trade at vout {}:", vout);
                println!("   - ReceiveIntent: {:?}", intent.get("data"));
                println!("   - ValueTransfers: {}", value_transfers.len());
                
                // Extract pool ID
                let pool_block = intent.get("alkaneAddressBlock").and_then(|v| v.as_str()).unwrap_or("");
                let pool_tx = intent.get("alkaneAddressTx").and_then(|v| v.as_str()).unwrap_or("");
                println!("   - Pool ID: {}:{}", pool_block, pool_tx);
                
                assert_eq!(pool_block, "4");
                assert_eq!(pool_tx, "100");
            }
        }
    }
}

#[test]
fn test_balance_tracking_logic() {
    // Test the value_transfer -> balance tracking logic
    
    let value_transfer_event = json!({
        "eventType": "value_transfer",
        "vout": 0,
        "data": {
            "transfers": [
                {"id": {"block": "2", "tx": "1"}, "value": "100"}
            ],
            "redirect_to": 0
        },
        "alkaneAddressBlock": "",
        "alkaneAddressTx": ""
    });
    
    // Extract transfer info
    let vout = value_transfer_event.get("vout").and_then(|v| v.as_i64()).unwrap();
    let data = value_transfer_event.get("data").unwrap();
    let redirect_to = data.get("redirect_to").and_then(|v| v.as_u64()).unwrap();
    let transfers = data.get("transfers").and_then(|v| v.as_array()).unwrap();
    
    println!("✅ Processing value_transfer:");
    println!("   - vout: {}", vout);
    println!("   - redirect_to: {}", redirect_to);
    println!("   - transfers: {}", transfers.len());
    
    for (i, transfer) in transfers.iter().enumerate() {
        let id = transfer.get("id").unwrap();
        let block = id.get("block").and_then(|v| v.as_str()).unwrap();
        let tx = id.get("tx").and_then(|v| v.as_str()).unwrap();
        let value = transfer.get("value").and_then(|v| v.as_str()).unwrap();
        
        println!("   Transfer {}:", i);
        println!("     - Alkane: {}:{}", block, tx);
        println!("     - Value: {}", value);
        println!("     - Destination vout: {}", redirect_to);
        
        // This is where we'd create a BalanceChange
        // The address would come from the transaction's vout[redirect_to]
    }
    
    assert_eq!(transfers.len(), 1);
}

#[test]
fn test_actual_db_event_format() {
    // This is the EXACT format from our database query
    let db_event = json!({
        "id": "acb1c079-08ef-4c18-b924-291237a9e2dd",
        "transactionId": "8e866b17cc0abf08fadf0f77f78370b1f9f611325615baeadb5fb055199d28b4",
        "vout": 4,
        "blockHeight": 438,
        "alkaneAddressBlock": "",
        "alkaneAddressTx": "",
        "eventType": "value_transfer",
        "data": {
            "transfers": [
                {"id": {"tx": "1", "block": "2"}, "value": "5"}
            ],
            "redirect_to": 0
        },
        "createdAt": "2025-12-01 19:04:36.582253+00",
        "updatedAt": "2025-12-01 19:04:36.582253+00"
    });
    
    // Verify we can extract all needed fields
    let event_type = db_event.get("eventType").and_then(|v| v.as_str()).unwrap();
    let vout = db_event.get("vout").and_then(|v| v.as_i64()).unwrap() as i32;
    let data = db_event.get("data").unwrap();
    let txid = db_event.get("transactionId").and_then(|v| v.as_str()).unwrap();
    let block_height = db_event.get("blockHeight").and_then(|v| v.as_i64()).unwrap() as i32;
    
    println!("✅ Successfully parsed DB event:");
    println!("   - Type: {}", event_type);
    println!("   - TXID: {}", txid);
    println!("   - Block: {}", block_height);
    println!("   - Vout: {}", vout);
    println!("   - Data: {}", data);
    
    assert_eq!(event_type, "value_transfer");
    assert_eq!(vout, 4);
    assert_eq!(block_height, 438);
    
    // Verify data structure
    let redirect_to = data.get("redirect_to").and_then(|v| v.as_u64()).unwrap();
    let transfers = data.get("transfers").and_then(|v| v.as_array()).unwrap();
    
    assert_eq!(redirect_to, 0);
    assert_eq!(transfers.len(), 1);
    
    println!("✅ All fields accessible - format is compatible!");
}
