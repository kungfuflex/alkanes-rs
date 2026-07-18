use alkanes_trace_transform::*;
use serde_json::json;

/// Integration test: Simulate a swap transaction and verify all tracking
#[test]
fn test_complete_swap_transaction() {
    // Setup
    let mut backend = InMemoryBackend::new();
    let mut pipeline = TransformPipeline::new();
    
    // Create context for the transaction
    let context = types::TransactionContext {
        txid: "swap_tx_123".to_string(),
        block_height: 100,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("bc1qswapper".to_string()),
                script_pubkey: "".to_string(),
                value: 1000,
            },
            types::VoutInfo {
                index: 1,
                address: Some("bc1qpool".to_string()),
                script_pubkey: "".to_string(),
                value: 2000,
            },
        ],
    };
    
    // Add balance tracker
    let balance_extractor = ValueTransferExtractor::with_context(context.clone());
    pipeline.add_extractor(balance_extractor);
    pipeline.add_tracker::<BalanceTracker, InMemoryBackend>(BalanceTracker::new());
    
    // Simulate value_transfer events from a swap
    // User sends 1000 of token A (4:10) to pool
    let trace1 = types::TraceEvent {
        event_type: "value_transfer".to_string(),
        vout: 0,
        alkane_address_block: "4".to_string(),
        alkane_address_tx: "10".to_string(),
        data: json!({
            "redirect_to": 1,
            "transfers": [
                {
                    "id": {"block": 4, "tx": 10},
                    "amount": "1000"
                }
            ]
        }),
    };
    
    // Pool sends 900 of token B (4:20) to user
    let trace2 = types::TraceEvent {
        event_type: "value_transfer".to_string(),
        vout: 1,
        alkane_address_block: "4".to_string(),
        alkane_address_tx: "20".to_string(),
        data: json!({
            "redirect_to": 0,
            "transfers": [
                {
                    "id": {"block": 4, "tx": 20},
                    "amount": "900"
                }
            ]
        }),
    };
    
    // Process traces
    pipeline.process_trace(&mut backend, &trace1).unwrap();
    pipeline.process_trace(&mut backend, &trace2).unwrap();
    
    // Verify balances were tracked
    
    // Pool should have +1000 of token A
    let pool_balance_key = format!("balance:bc1qpool:{}", types::AlkaneId::new(4, 10).to_string());
    let pool_balance_bytes = backend.get("address_balances", pool_balance_key.as_bytes()).unwrap();
    assert!(pool_balance_bytes.is_some(), "Pool should have token A balance");
    
    let pool_balance: AddressBalance = serde_json::from_slice(&pool_balance_bytes.unwrap()).unwrap();
    assert_eq!(pool_balance.total_amount, 1000);
    
    // User should have +900 of token B
    let user_balance_key = format!("balance:bc1qswapper:{}", types::AlkaneId::new(4, 20).to_string());
    let user_balance_bytes = backend.get("address_balances", user_balance_key.as_bytes()).unwrap();
    assert!(user_balance_bytes.is_some(), "User should have token B balance");
    
    let user_balance: AddressBalance = serde_json::from_slice(&user_balance_bytes.unwrap()).unwrap();
    assert_eq!(user_balance.total_amount, 900);
    
    // Verify UTXO-level tracking
    let pool_utxo_key = format!("utxo:swap_tx_123:1:{}", types::AlkaneId::new(4, 10).to_string());
    let pool_utxo_bytes = backend.get("utxo_balances", pool_utxo_key.as_bytes()).unwrap();
    assert!(pool_utxo_bytes.is_some(), "Pool UTXO should be tracked");
}

/// Test multiple sequential swaps to verify accumulation
#[test]
fn test_sequential_swaps() {
    let mut backend = InMemoryBackend::new();
    let mut pipeline = TransformPipeline::new();
    
    // First swap
    let context1 = types::TransactionContext {
        txid: "tx1".to_string(),
        block_height: 100,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("bc1quser".to_string()),
                script_pubkey: "".to_string(),
                value: 1000,
            },
        ],
    };
    
    let extractor1 = ValueTransferExtractor::with_context(context1);
    pipeline.add_extractor(extractor1);
    pipeline.add_tracker::<BalanceTracker, InMemoryBackend>(BalanceTracker::new());
    
    let trace1 = types::TraceEvent {
        event_type: "value_transfer".to_string(),
        vout: 0,
        alkane_address_block: "4".to_string(),
        alkane_address_tx: "10".to_string(),
        data: json!({
            "redirect_to": 0,
            "transfers": [
                {
                    "id": {"block": 4, "tx": 10},
                    "amount": "1000"
                }
            ]
        }),
    };
    
    pipeline.process_trace(&mut backend, &trace1).unwrap();
    
    // Second swap (different transaction, same user)
    let context2 = types::TransactionContext {
        txid: "tx2".to_string(),
        block_height: 101,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("bc1quser".to_string()),
                script_pubkey: "".to_string(),
                value: 2000,
            },
        ],
    };
    
    // We need to update the extractor context for the second transaction
    // In a real implementation, this would be handled by the pipeline
    let extractor2 = ValueTransferExtractor::with_context(context2);
    
    let trace2 = types::TraceEvent {
        event_type: "value_transfer".to_string(),
        vout: 0,
        alkane_address_block: "4".to_string(),
        alkane_address_tx: "10".to_string(),
        data: json!({
            "redirect_to": 0,
            "transfers": [
                {
                    "id": {"block": 4, "tx": 10},
                    "amount": "500"
                }
            ]
        }),
    };
    
    // Process second trace
    if let Ok(Some(changes)) = extractor2.extract(&trace2) {
        let mut tracker = BalanceTracker::new();
        tracker.update(&mut backend, changes).unwrap();
    }
    
    // Verify accumulated balance
    let balance_key = format!("balance:bc1quser:{}", types::AlkaneId::new(4, 10).to_string());
    let balance_bytes = backend.get("address_balances", balance_key.as_bytes()).unwrap().unwrap();
    let balance: AddressBalance = serde_json::from_slice(&balance_bytes).unwrap();
    
    // Should have 1000 + 500 = 1500
    assert_eq!(balance.total_amount, 1500);
}

/// Test AMM tracker with trade events
#[test]
fn test_amm_trade_tracking_integration() {
    let mut backend = InMemoryBackend::new();
    let mut tracker = AmmTracker::with_intervals(vec!["1h".to_string()]);
    
    let timestamp = chrono::DateTime::from_timestamp(1609459200, 0).unwrap();
    let pool_id = types::AlkaneId::new(4, 100);
    
    // Simulate a series of trades
    let trades = vec![
        TradeEvent {
            txid: "trade1".to_string(),
            vout: 0,
            pool_id: pool_id.clone(),
            token0_id: types::AlkaneId::new(4, 10),
            token1_id: types::AlkaneId::new(4, 20),
            amount0_in: 1000,
            amount1_in: 0,
            amount0_out: 0,
            amount1_out: 900,
            reserve0_after: 101000,
            reserve1_after: 99100,
            timestamp,
            block_height: 100,
        },
        TradeEvent {
            txid: "trade2".to_string(),
            vout: 0,
            pool_id: pool_id.clone(),
            token0_id: types::AlkaneId::new(4, 10),
            token1_id: types::AlkaneId::new(4, 20),
            amount0_in: 0,
            amount1_in: 1000,
            amount0_out: 980,
            amount1_out: 0,
            reserve0_after: 100020,
            reserve1_after: 100100,
            timestamp: timestamp + chrono::Duration::minutes(10),
            block_height: 100,
        },
        TradeEvent {
            txid: "trade3".to_string(),
            vout: 0,
            pool_id: pool_id.clone(),
            token0_id: types::AlkaneId::new(4, 10),
            token1_id: types::AlkaneId::new(4, 20),
            amount0_in: 500,
            amount1_in: 0,
            amount0_out: 0,
            amount1_out: 498,
            reserve0_after: 100520,
            reserve1_after: 99602,
            timestamp: timestamp + chrono::Duration::minutes(20),
            block_height: 100,
        },
    ];
    
    tracker.update(&mut backend, trades).unwrap();
    
    // Verify trades were stored
    let all_trades = backend.scan("trades").unwrap();
    assert_eq!(all_trades.len(), 3, "Should have 3 trades stored");
    
    // Verify reserves were updated
    let all_reserves = backend.scan("reserves").unwrap();
    assert_eq!(all_reserves.len(), 3, "Should have 3 reserve snapshots");
    
    // Verify candle was aggregated
    let all_candles = backend.scan("candles").unwrap();
    assert_eq!(all_candles.len(), 1, "Should have 1 hourly candle");
    
    // Verify candle data
    let (_, candle_bytes) = &all_candles[0];
    let candle: Candle = serde_json::from_slice(candle_bytes).unwrap();
    assert_eq!(candle.trade_count, 3);
    assert_eq!(candle.interval, "1h");
    assert!(candle.high >= candle.low);
    assert!(candle.volume0 > 0);
    assert!(candle.volume1 > 0);
}

/// Test receive_intent event with incoming_alkanes array
/// This simulates the exact data structure from actual alkanes transactions
#[test]
fn test_receive_intent_with_incoming_alkanes() {
    // Setup
    let mut backend = InMemoryBackend::new();
    let mut pipeline = TransformPipeline::new();
    
    // Create context for a transaction where user receives alkanes
    let context = types::TransactionContext {
        txid: "receive_tx_abc123".to_string(),
        block_height: 485,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("bcrt1p705x8h5dy67x7tgdu6wv2crq333sdx6h776vc929rxcdlxs5wj2s7h296c".to_string()),
                script_pubkey: "5120f3e863de8d26bc6f2d0de69cc560608c63069b57f7b4cc154519b0df9a147495".to_string(),
                value: 546,
            },
        ],
    };
    
    // Add balance tracker
    let balance_extractor = ValueTransferExtractor::with_context(context.clone());
    pipeline.add_extractor(balance_extractor);
    pipeline.add_tracker::<BalanceTracker, InMemoryBackend>(BalanceTracker::new());
    
    // Simulate receive_intent event with incoming_alkanes array
    // This matches the exact structure from alkanes runtime
    let trace = types::TraceEvent {
        event_type: "receive_intent".to_string(),
        vout: 0,
        alkane_address_block: "".to_string(), // receive_intent doesn't have alkane address in top level
        alkane_address_tx: "".to_string(),
        data: json!({
            "incoming_alkanes": [
                {
                    "id": {
                        "block": 2,
                        "tx": 0
                    },
                    "value": {
                        "lo": 5000000000_i64,
                        "hi": 0
                    }
                },
                {
                    "id": {
                        "block": 32,
                        "tx": 0
                    },
                    "value": {
                        "lo": 164,
                        "hi": 0
                    }
                }
            ]
        }),
    };
    
    // Process trace
    pipeline.process_trace(&mut backend, &trace).unwrap();
    
    // Verify balances were tracked for DIESEL (2:0)
    let diesel_balance_key = format!("balance:bcrt1p705x8h5dy67x7tgdu6wv2crq333sdx6h776vc929rxcdlxs5wj2s7h296c:{}", 
        types::AlkaneId::new(2, 0).to_string());
    let diesel_balance_bytes = backend.get("address_balances", diesel_balance_key.as_bytes()).unwrap();
    assert!(diesel_balance_bytes.is_some(), "Should have DIESEL balance");
    
    let diesel_balance: AddressBalance = serde_json::from_slice(&diesel_balance_bytes.unwrap()).unwrap();
    assert_eq!(diesel_balance.total_amount, 5000000000);
    assert_eq!(diesel_balance.alkane_id, types::AlkaneId::new(2, 0));
    
    // Verify balances for frBTC (32:0)
    let frbtc_balance_key = format!("balance:bcrt1p705x8h5dy67x7tgdu6wv2crq333sdx6h776vc929rxcdlxs5wj2s7h296c:{}", 
        types::AlkaneId::new(32, 0).to_string());
    let frbtc_balance_bytes = backend.get("address_balances", frbtc_balance_key.as_bytes()).unwrap();
    assert!(frbtc_balance_bytes.is_some(), "Should have frBTC balance");
    
    let frbtc_balance: AddressBalance = serde_json::from_slice(&frbtc_balance_bytes.unwrap()).unwrap();
    assert_eq!(frbtc_balance.total_amount, 164);
    assert_eq!(frbtc_balance.alkane_id, types::AlkaneId::new(32, 0));
    
    // Verify UTXO-level tracking
    let diesel_utxo_key = format!("utxo:receive_tx_abc123:0:{}", types::AlkaneId::new(2, 0).to_string());
    let diesel_utxo_bytes = backend.get("utxo_balances", diesel_utxo_key.as_bytes()).unwrap();
    assert!(diesel_utxo_bytes.is_some(), "DIESEL UTXO should be tracked");
    
    let frbtc_utxo_key = format!("utxo:receive_tx_abc123:0:{}", types::AlkaneId::new(32, 0).to_string());
    let frbtc_utxo_bytes = backend.get("utxo_balances", frbtc_utxo_key.as_bytes()).unwrap();
    assert!(frbtc_utxo_bytes.is_some(), "frBTC UTXO should be tracked");
}

/// Test receive_intent with empty incoming_alkanes
#[test]
fn test_receive_intent_empty_array() {
    let mut backend = InMemoryBackend::new();
    let mut pipeline = TransformPipeline::new();
    
    let context = types::TransactionContext {
        txid: "empty_receive".to_string(),
        block_height: 100,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("bc1qtest".to_string()),
                script_pubkey: "".to_string(),
                value: 1000,
            },
        ],
    };
    
    let extractor = ValueTransferExtractor::with_context(context);
    pipeline.add_extractor(extractor);
    pipeline.add_tracker::<BalanceTracker, InMemoryBackend>(BalanceTracker::new());
    
    let trace = types::TraceEvent {
        event_type: "receive_intent".to_string(),
        vout: 0,
        alkane_address_block: "".to_string(),
        alkane_address_tx: "".to_string(),
        data: json!({
            "incoming_alkanes": []
        }),
    };
    
    // Should not error with empty array
    let result = pipeline.process_trace(&mut backend, &trace);
    assert!(result.is_ok(), "Empty incoming_alkanes should not error");
    
    // Should not create any balances
    let all_balances = backend.scan("address_balances").unwrap();
    assert_eq!(all_balances.len(), 0, "Should have no balances for empty array");
}

/// Test multiple receive_intent events accumulating balance
#[test]
fn test_receive_intent_accumulation() {
    let mut backend = InMemoryBackend::new();
    let mut pipeline = TransformPipeline::new();
    
    let address = "bcrt1qtest".to_string();
    
    // First receive
    let context1 = types::TransactionContext {
        txid: "tx1".to_string(),
        block_height: 100,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some(address.clone()),
                script_pubkey: "script1".to_string(),
                value: 546,
            },
        ],
    };
    
    let extractor1 = ValueTransferExtractor::with_context(context1);
    pipeline.add_extractor(extractor1);
    pipeline.add_tracker::<BalanceTracker, InMemoryBackend>(BalanceTracker::new());
    
    let trace1 = types::TraceEvent {
        event_type: "receive_intent".to_string(),
        vout: 0,
        alkane_address_block: "".to_string(),
        alkane_address_tx: "".to_string(),
        data: json!({
            "incoming_alkanes": [
                {
                    "id": {"block": 2, "tx": 0},
                    "value": {"lo": 1000, "hi": 0}
                }
            ]
        }),
    };
    
    pipeline.process_trace(&mut backend, &trace1).unwrap();
    
    // Second receive (same alkane, same address)
    let context2 = types::TransactionContext {
        txid: "tx2".to_string(),
        block_height: 101,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some(address.clone()),
                script_pubkey: "script2".to_string(),
                value: 546,
            },
        ],
    };
    
    let extractor2 = ValueTransferExtractor::with_context(context2);
    
    let trace2 = types::TraceEvent {
        event_type: "receive_intent".to_string(),
        vout: 0,
        alkane_address_block: "".to_string(),
        alkane_address_tx: "".to_string(),
        data: json!({
            "incoming_alkanes": [
                {
                    "id": {"block": 2, "tx": 0},
                    "value": {"lo": 500, "hi": 0}
                }
            ]
        }),
    };
    
    // Process second trace
    if let Ok(Some(changes)) = extractor2.extract(&trace2) {
        let mut tracker = BalanceTracker::new();
        tracker.update(&mut backend, changes).unwrap();
    }
    
    // Verify accumulated balance (1000 + 500 = 1500)
    let balance_key = format!("balance:{}:{}", address, types::AlkaneId::new(2, 0).to_string());
    let balance_bytes = backend.get("address_balances", balance_key.as_bytes()).unwrap().unwrap();
    let balance: AddressBalance = serde_json::from_slice(&balance_bytes).unwrap();
    
    assert_eq!(balance.total_amount, 1500);
    
    // Verify both UTXOs exist
    let utxo1_key = format!("utxo:tx1:0:{}", types::AlkaneId::new(2, 0).to_string());
    let utxo2_key = format!("utxo:tx2:0:{}", types::AlkaneId::new(2, 0).to_string());
    
    assert!(backend.get("utxo_balances", utxo1_key.as_bytes()).unwrap().is_some());
    assert!(backend.get("utxo_balances", utxo2_key.as_bytes()).unwrap().is_some());
}

/// Test value_transfer with STRING format for block/tx/value (as emitted by protostone.rs)
/// This matches the exact format from alkanes_pb::alkanes_trace_event::Event::ValueTransfer
#[test]
fn test_value_transfer_with_string_ids() {
    let mut backend = InMemoryBackend::new();
    let mut pipeline = TransformPipeline::new();

    let context = types::TransactionContext {
        txid: "string_test_tx".to_string(),
        block_height: 200,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("bc1qsender".to_string()),
                script_pubkey: "".to_string(),
                value: 546,
            },
            types::VoutInfo {
                index: 1,
                address: Some("bc1qreceiver".to_string()),
                script_pubkey: "".to_string(),
                value: 546,
            },
        ],
    };

    let extractor = ValueTransferExtractor::with_context(context);
    pipeline.add_extractor(extractor);
    pipeline.add_tracker::<BalanceTracker, InMemoryBackend>(BalanceTracker::new());

    // Use STRING format for block, tx, and value (matches protostone.rs convert_trace_to_events)
    let trace = types::TraceEvent {
        event_type: "value_transfer".to_string(),
        vout: 0,
        alkane_address_block: "".to_string(),
        alkane_address_tx: "".to_string(),
        data: json!({
            "redirect_to": 1,
            "transfers": [
                {
                    "id": {
                        "block": "2",    // STRING, not number
                        "tx": "65523"    // STRING, not number
                    },
                    "value": "5000000000"  // STRING, not number
                }
            ]
        }),
    };

    pipeline.process_trace(&mut backend, &trace).unwrap();

    // Verify balance was tracked correctly
    let balance_key = format!("balance:bc1qreceiver:{}", types::AlkaneId::new(2, 65523).to_string());
    let balance_bytes = backend.get("address_balances", balance_key.as_bytes()).unwrap();
    assert!(balance_bytes.is_some(), "Should have balance for receiver with string IDs");

    let balance: AddressBalance = serde_json::from_slice(&balance_bytes.unwrap()).unwrap();
    assert_eq!(balance.total_amount, 5000000000);
    assert_eq!(balance.alkane_id.block, 2);
    assert_eq!(balance.alkane_id.tx, 65523);
}

/// Test receive_intent with "transfers" field name (as emitted by protostone.rs)
#[test]
fn test_receive_intent_with_transfers_field() {
    let mut backend = InMemoryBackend::new();
    let mut pipeline = TransformPipeline::new();

    // Note: For receive_intent, the vout is usually a shadow vout (tx.output.len() + 1 + i)
    // We need to provide a matching physical vout in the context for this test
    let context = types::TransactionContext {
        txid: "receive_transfers_test".to_string(),
        block_height: 300,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("bc1qtest".to_string()),
                script_pubkey: "".to_string(),
                value: 546,
            },
        ],
    };

    let extractor = ValueTransferExtractor::with_context(context);
    pipeline.add_extractor(extractor);
    pipeline.add_tracker::<BalanceTracker, InMemoryBackend>(BalanceTracker::new());

    // Use "transfers" field (not "incoming_alkanes") - matches protostone.rs format
    let trace = types::TraceEvent {
        event_type: "receive_intent".to_string(),
        vout: 0,  // Using physical vout for this test
        alkane_address_block: "".to_string(),
        alkane_address_tx: "".to_string(),
        data: json!({
            "transfers": [
                {
                    "id": {
                        "block": "4",
                        "tx": "100"
                    },
                    "value": "999"
                }
            ]
        }),
    };

    pipeline.process_trace(&mut backend, &trace).unwrap();

    // Verify balance was tracked
    let balance_key = format!("balance:bc1qtest:{}", types::AlkaneId::new(4, 100).to_string());
    let balance_bytes = backend.get("address_balances", balance_key.as_bytes()).unwrap();
    assert!(balance_bytes.is_some(), "Should have balance from receive_intent with transfers field");

    let balance: AddressBalance = serde_json::from_slice(&balance_bytes.unwrap()).unwrap();
    assert_eq!(balance.total_amount, 999);
}

/// Test reset functionality
#[test]
fn test_pipeline_reset() {
    let mut backend = InMemoryBackend::new();
    let mut pipeline = TransformPipeline::new();
    
    let context = types::TransactionContext {
        txid: "test_tx".to_string(),
        block_height: 100,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("bc1qtest".to_string()),
                script_pubkey: "".to_string(),
                value: 1000,
            },
        ],
    };
    
    let extractor = ValueTransferExtractor::with_context(context);
    pipeline.add_extractor(extractor);
    pipeline.add_tracker::<BalanceTracker, InMemoryBackend>(BalanceTracker::new());
    
    // Process some traces
    let trace = types::TraceEvent {
        event_type: "value_transfer".to_string(),
        vout: 0,
        alkane_address_block: "4".to_string(),
        alkane_address_tx: "10".to_string(),
        data: json!({
            "redirect_to": 0,
            "transfers": [
                {
                    "id": {"block": 4, "tx": 10},
                    "amount": "1000"
                }
            ]
        }),
    };
    
    pipeline.process_trace(&mut backend, &trace).unwrap();
    
    // Verify data exists
    let before_reset = backend.scan("address_balances").unwrap();
    assert!(!before_reset.is_empty(), "Should have balance data");
    
    // Reset
    pipeline.reset(&mut backend).unwrap();
    
    // Verify data is cleared
    let after_reset = backend.scan("address_balances").unwrap();
    assert!(after_reset.is_empty(), "Balance data should be cleared");
}
