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
                value: 1000,
            },
            types::VoutInfo {
                index: 1,
                address: Some("bc1qpool".to_string()),
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
