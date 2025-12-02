/// PostgreSQL Integration Test
/// 
/// This test uses the actual PostgreSQL container from docker-compose
/// to verify the complete production flow:
/// 1. ValueTransferExtractor extracts from receive_intent events
/// 2. OptimizedBalanceProcessor writes to PostgreSQL
/// 3. Data appears in TraceAlkaneBalance table
/// 
/// Run with: cargo test --package alkanes-trace-transform --test postgres_integration_test -- --ignored
/// 
/// Prerequisites:
/// - PostgreSQL container running: docker-compose up -d postgres
/// - DATABASE_URL env var set (or use default)

use alkanes_trace_transform::*;
use serde_json::json;
use sqlx::PgPool;

#[tokio::test]
#[ignore] // Requires PostgreSQL container
async fn test_receive_intent_with_real_postgres() {
    // Connect to the PostgreSQL container
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://alkanes_user:alkanes_pass@localhost:5432/alkanes_indexer".to_string());
    
    println!("Connecting to database: {}", database_url);
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL - is docker-compose up?");
    
    // Clean up any existing test data
    sqlx::query(r#"DELETE FROM "TraceAlkaneBalance" WHERE address = 'test_address_postgres'"#)
        .execute(&pool)
        .await
        .expect("Failed to clean up test data");
    
    sqlx::query(r#"DELETE FROM "TraceBalanceUtxo" WHERE address = 'test_address_postgres'"#)
        .execute(&pool)
        .await
        .expect("Failed to clean up UTXO data");
    
    println!("✓ Database connected and cleaned");
    
    // Create transaction context
    let context = types::TransactionContext {
        txid: "test_tx_postgres_123".to_string(),
        block_height: 485,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("test_address_postgres".to_string()),
                script_pubkey: "5120test".to_string(),
                value: 546,
            },
        ],
    };
    
    // Create trace event with receive_intent
    let trace = types::TraceEvent {
        event_type: "receive_intent".to_string(),
        vout: 0,
        alkane_address_block: "".to_string(),
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
    
    println!("✓ Test data prepared");
    
    // Create OptimizedBalanceProcessor (production code path)
    let mut processor = trackers::optimized_balance::OptimizedBalanceProcessor::with_context(
        pool.clone(),
        context.clone(),
    );
    
    println!("✓ OptimizedBalanceProcessor created");
    
    // Process the trace (this is what production does)
    processor.process_trace(&trace)
        .await
        .expect("Failed to process trace");
    
    println!("✓ Trace processed");
    
    // Verify data was written to TraceAlkaneBalance
    let balance_2_0: Option<(String,)> = sqlx::query_as(
        r#"SELECT balance::TEXT FROM "TraceAlkaneBalance"
           WHERE address = 'test_address_postgres'
           AND alkane_block = 2
           AND alkane_tx = 0"#
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to query TraceAlkaneBalance");
    
    assert!(balance_2_0.is_some(), "Expected balance for 2:0 (DIESEL) but got None");
    let (balance_str,) = balance_2_0.unwrap();
    let balance: u128 = balance_str.parse().expect("Failed to parse balance");
    assert_eq!(balance, 5000000000, "Expected 5000000000 DIESEL but got {}", balance);
    
    println!("✓ DIESEL balance verified: {}", balance);
    
    // Verify frBTC balance
    let balance_32_0: Option<(String,)> = sqlx::query_as(
        r#"SELECT balance::TEXT FROM "TraceAlkaneBalance"
           WHERE address = 'test_address_postgres'
           AND alkane_block = 32
           AND alkane_tx = 0"#
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to query frBTC balance");
    
    assert!(balance_32_0.is_some(), "Expected balance for 32:0 (frBTC) but got None");
    let (balance_str,) = balance_32_0.unwrap();
    let balance: u128 = balance_str.parse().expect("Failed to parse balance");
    assert_eq!(balance, 164, "Expected 164 frBTC but got {}", balance);
    
    println!("✓ frBTC balance verified: {}", balance);
    
    // Verify UTXO was created
    let utxo_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM "TraceBalanceUtxo"
           WHERE address = 'test_address_postgres'"#
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to count UTXOs");
    
    assert_eq!(utxo_count.0, 2, "Expected 2 UTXOs but got {}", utxo_count.0);
    
    println!("✓ UTXOs verified: {}", utxo_count.0);
    
    // Clean up
    sqlx::query(r#"DELETE FROM "TraceAlkaneBalance" WHERE address = 'test_address_postgres'"#)
        .execute(&pool)
        .await
        .expect("Failed to clean up");
    
    sqlx::query(r#"DELETE FROM "TraceBalanceUtxo" WHERE address = 'test_address_postgres'"#)
        .execute(&pool)
        .await
        .expect("Failed to clean up");
    
    println!("✓ Cleanup complete");
    println!("\n🎉 All checks passed! Production flow works correctly.");
}

#[tokio::test]
#[ignore] // Requires PostgreSQL container
async fn test_value_transfer_with_real_postgres() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://alkanes_user:alkanes_pass@localhost:5432/alkanes_indexer".to_string());
    
    println!("Testing value_transfer event with PostgreSQL...");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");
    
    // Clean up
    sqlx::query(r#"DELETE FROM "TraceAlkaneBalance" WHERE address = 'test_value_transfer'"#)
        .execute(&pool)
        .await
        .expect("Failed to clean up");
    
    // Create context
    let context = types::TransactionContext {
        txid: "test_value_tx_456".to_string(),
        block_height: 486,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("test_value_transfer".to_string()),
                script_pubkey: "script".to_string(),
                value: 1000,
            },
        ],
    };
    
    // Create value_transfer trace
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
    
    // Process
    let mut processor = trackers::optimized_balance::OptimizedBalanceProcessor::with_context(
        pool.clone(),
        context,
    );
    
    processor.process_trace(&trace)
        .await
        .expect("Failed to process value_transfer");
    
    // Verify
    let balance: Option<(String,)> = sqlx::query_as(
        r#"SELECT balance::TEXT FROM "TraceAlkaneBalance"
           WHERE address = 'test_value_transfer'
           AND alkane_block = 4
           AND alkane_tx = 10"#
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to query balance");
    
    assert!(balance.is_some(), "Expected balance for value_transfer but got None");
    let (balance_str,) = balance.unwrap();
    let balance_val: u128 = balance_str.parse().unwrap();
    assert_eq!(balance_val, 1000);
    
    // Clean up
    sqlx::query(r#"DELETE FROM "TraceAlkaneBalance" WHERE address = 'test_value_transfer'"#)
        .execute(&pool)
        .await
        .expect("Failed to clean up");
    
    println!("✓ value_transfer test passed!");
}

#[tokio::test]
#[ignore] // Requires PostgreSQL container
async fn test_multiple_receives_accumulate() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://alkanes_user:alkanes_pass@localhost:5432/alkanes_indexer".to_string());
    
    println!("Testing balance accumulation with PostgreSQL...");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");
    
    // Clean up
    sqlx::query(r#"DELETE FROM "TraceAlkaneBalance" WHERE address = 'test_accumulation'"#)
        .execute(&pool)
        .await
        .expect("Failed to clean up");
    
    // First receive
    let context1 = types::TransactionContext {
        txid: "accumulation_tx1".to_string(),
        block_height: 100,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("test_accumulation".to_string()),
                script_pubkey: "script".to_string(),
                value: 546,
            },
        ],
    };
    
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
    
    let mut processor1 = trackers::optimized_balance::OptimizedBalanceProcessor::with_context(
        pool.clone(),
        context1,
    );
    processor1.process_trace(&trace1).await.expect("Failed to process first trace");
    
    // Second receive
    let context2 = types::TransactionContext {
        txid: "accumulation_tx2".to_string(),
        block_height: 101,
        timestamp: chrono::Utc::now(),
        vouts: vec![
            types::VoutInfo {
                index: 0,
                address: Some("test_accumulation".to_string()),
                script_pubkey: "script".to_string(),
                value: 546,
            },
        ],
    };
    
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
    
    let mut processor2 = trackers::optimized_balance::OptimizedBalanceProcessor::with_context(
        pool.clone(),
        context2,
    );
    processor2.process_trace(&trace2).await.expect("Failed to process second trace");
    
    // Verify accumulated balance (1000 + 500 = 1500)
    let balance: Option<(String,)> = sqlx::query_as(
        r#"SELECT balance::TEXT FROM "TraceAlkaneBalance"
           WHERE address = 'test_accumulation'
           AND alkane_block = 2
           AND alkane_tx = 0"#
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to query balance");
    
    assert!(balance.is_some(), "Expected accumulated balance but got None");
    let (balance_str,) = balance.unwrap();
    let balance_val: u128 = balance_str.parse().unwrap();
    assert_eq!(balance_val, 1500, "Expected 1500 (1000+500) but got {}", balance_val);
    
    // Verify 2 UTXOs exist
    let utxo_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM "TraceBalanceUtxo"
           WHERE address = 'test_accumulation'
           AND alkane_block = 2
           AND alkane_tx = 0"#
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to count UTXOs");
    
    assert_eq!(utxo_count.0, 2, "Expected 2 UTXOs but got {}", utxo_count.0);
    
    // Clean up
    sqlx::query(r#"DELETE FROM "TraceAlkaneBalance" WHERE address = 'test_accumulation'"#)
        .execute(&pool)
        .await
        .expect("Failed to clean up");
    
    sqlx::query(r#"DELETE FROM "TraceBalanceUtxo" WHERE address = 'test_accumulation'"#)
        .execute(&pool)
        .await
        .expect("Failed to clean up");
    
    println!("✓ Accumulation test passed! Balance: 1500, UTXOs: 2");
}
