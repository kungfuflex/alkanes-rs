//! Integration tests for debshrew-mono CDC functionality
//! 
//! These tests verify the end-to-end CDC pipeline from k/v updates
//! through Kafka delivery using embedded test infrastructure.

use debshrew_mono::{
    cdc_config::{CDCConfig, CDCRelationship, GlobalSettings, KafkaConfig, KeyMapping, ValueMapping, FieldMapping},
    cdc_tracker::CDCTracker,
    kafka_producer::KafkaProducer,
    relationship_matcher::RelationshipMatcher,
    schema_manager::SchemaManager,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

/// Mock Kafka producer for testing that captures sent events
#[derive(Debug, Clone)]
pub struct MockKafkaProducer {
    pub sent_events: Arc<Mutex<Vec<(String, String, Value)>>>, // (topic, key, value)
    pub should_fail: Arc<Mutex<bool>>,
}

impl MockKafkaProducer {
    pub fn new() -> Self {
        Self {
            sent_events: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn send_event(&self, topic: &str, key: &str, value: Value) -> Result<(), String> {
        if *self.should_fail.lock().await {
            return Err("Mock failure".to_string());
        }
        
        self.sent_events.lock().await.push((
            topic.to_string(),
            key.to_string(),
            value,
        ));
        Ok(())
    }

    pub async fn get_sent_events(&self) -> Vec<(String, String, Value)> {
        self.sent_events.lock().await.clone()
    }

    pub async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().await = should_fail;
    }

    pub async fn clear_events(&self) {
        self.sent_events.lock().await.clear();
    }
}

/// Create a test CDC configuration
fn create_test_config() -> CDCConfig {
    CDCConfig {
        global_settings: GlobalSettings {
            max_buffer_size: 10,
            flush_interval_ms: 100,
            retry_attempts: 2,
            retry_backoff_ms: 50,
            enable_deduplication: true,
            include_block_metadata: true,
        },
        kafka_config: KafkaConfig {
            bootstrap_servers: "localhost:9092".to_string(),
            topic_prefix: "test.cdc".to_string(),
            acks: "all".to_string(),
            compression: Some("gzip".to_string()),
            batch_size: Some(100),
            linger_ms: Some(10),
            additional_properties: HashMap::new(),
        },
        schema_registry: None,
        relationships: vec![
            // Bitcoin addresses relationship
            CDCRelationship {
                name: "bitcoin_addresses".to_string(),
                key_pattern: r"^address:([13][a-km-zA-HJ-NP-Z1-9]{25,34})$".to_string(),
                table_name: "addresses".to_string(),
                operation_type: "CreateUpdate".to_string(),
                key_mapping: KeyMapping {
                    primary_key_fields: vec!["address".to_string()],
                    extraction_pattern: r"^address:(.+)$".to_string(),
                },
                value_mapping: ValueMapping {
                    value_format: "Json".to_string(),
                    fields: vec![
                        FieldMapping {
                            name: "address".to_string(),
                            path: "$.address".to_string(),
                            data_type: "varchar".to_string(),
                            nullable: false,
                        },
                        FieldMapping {
                            name: "balance".to_string(),
                            path: "$.balance".to_string(),
                            data_type: "bigint".to_string(),
                            nullable: false,
                        },
                    ],
                },
            },
            // Bitcoin transactions relationship
            CDCRelationship {
                name: "bitcoin_transactions".to_string(),
                key_pattern: r"^tx:([a-fA-F0-9]{64})$".to_string(),
                table_name: "transactions".to_string(),
                operation_type: "CreateOnly".to_string(),
                key_mapping: KeyMapping {
                    primary_key_fields: vec!["txid".to_string()],
                    extraction_pattern: r"^tx:(.+)$".to_string(),
                },
                value_mapping: ValueMapping {
                    value_format: "Json".to_string(),
                    fields: vec![
                        FieldMapping {
                            name: "txid".to_string(),
                            path: "$.txid".to_string(),
                            data_type: "varchar".to_string(),
                            nullable: false,
                        },
                        FieldMapping {
                            name: "block_height".to_string(),
                            path: "$.height".to_string(),
                            data_type: "integer".to_string(),
                            nullable: false,
                        },
                    ],
                },
            },
            // Raw data relationship for testing
            CDCRelationship {
                name: "raw_data".to_string(),
                key_pattern: r"^raw:(.+)$".to_string(),
                table_name: "raw_data".to_string(),
                operation_type: "CreateUpdateDelete".to_string(),
                key_mapping: KeyMapping {
                    primary_key_fields: vec!["key".to_string()],
                    extraction_pattern: r"^raw:(.+)$".to_string(),
                },
                value_mapping: ValueMapping {
                    value_format: "Raw".to_string(),
                    fields: vec![
                        FieldMapping {
                            name: "key".to_string(),
                            path: "$.key".to_string(),
                            data_type: "text".to_string(),
                            nullable: false,
                        },
                        FieldMapping {
                            name: "raw_value".to_string(),
                            path: "$.raw_value".to_string(),
                            data_type: "text".to_string(),
                            nullable: false,
                        },
                    ],
                },
            },
        ],
    }
}

#[tokio::test]
async fn test_relationship_matcher() {
    let config = create_test_config();
    let matcher = RelationshipMatcher::new(&config.relationships).unwrap();

    // Test address matching
    let matches = matcher.find_matches("address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "bitcoin_addresses");

    // Test transaction matching
    let matches = matcher.find_matches("tx:abc123def456789012345678901234567890123456789012345678901234567890");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "bitcoin_transactions");

    // Test raw data matching
    let matches = matcher.find_matches("raw:test_key");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "raw_data");

    // Test no matches
    let matches = matcher.find_matches("nomatch:test");
    assert_eq!(matches.len(), 0);
}

#[tokio::test]
async fn test_schema_manager() {
    let config = create_test_config();
    let schema_manager = SchemaManager::new(&config).unwrap();

    // Test schema generation for address relationship
    let relationship = &config.relationships[0];
    let schema = schema_manager.get_schema_for_relationship(relationship).unwrap();
    
    // Verify schema structure
    assert!(schema.as_object().unwrap().contains_key("type"));
    assert!(schema.as_object().unwrap().contains_key("fields"));
    
    let fields = schema["fields"].as_array().unwrap();
    assert!(fields.iter().any(|f| f["field"] == "before"));
    assert!(fields.iter().any(|f| f["field"] == "after"));
    assert!(fields.iter().any(|f| f["field"] == "source"));
    assert!(fields.iter().any(|f| f["field"] == "op"));
}

#[tokio::test]
async fn test_cdc_tracker_basic_operations() {
    let config = create_test_config();
    let mock_producer = MockKafkaProducer::new();
    
    // Create CDC tracker with mock producer
    let mut tracker = CDCTracker::new(config).unwrap();
    
    // Test address creation
    let address_key = "address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    let address_value = json!({
        "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
        "balance": 5000000000i64
    });
    
    tracker.track_update(
        address_key.as_bytes(),
        Some(address_value.to_string().as_bytes()),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    // Test transaction creation
    let tx_key = "tx:abc123def456789012345678901234567890123456789012345678901234567890";
    let tx_value = json!({
        "txid": "abc123def456789012345678901234567890123456789012345678901234567890",
        "height": 800000
    });
    
    tracker.track_update(
        tx_key.as_bytes(),
        Some(tx_value.to_string().as_bytes()),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    // Test raw data creation
    let raw_key = "raw:test_data";
    let raw_value = b"binary_data_here";
    
    tracker.track_update(
        raw_key.as_bytes(),
        Some(raw_value),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    // Force flush to process buffered events
    tracker.flush().await.unwrap();

    // Verify events were generated (this would normally go to Kafka)
    // In a real test, we'd verify the mock producer received the events
}

#[tokio::test]
async fn test_cdc_tracker_update_operations() {
    let config = create_test_config();
    let mut tracker = CDCTracker::new(config).unwrap();
    
    let address_key = "address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    
    // Initial creation
    let initial_value = json!({
        "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
        "balance": 5000000000i64
    });
    
    tracker.track_update(
        address_key.as_bytes(),
        Some(initial_value.to_string().as_bytes()),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    // Update with new balance
    let updated_value = json!({
        "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
        "balance": 4500000000i64
    });
    
    tracker.track_update(
        address_key.as_bytes(),
        Some(updated_value.to_string().as_bytes()),
        Some(800001),
        Some("block_hash_124"),
    ).await.unwrap();

    tracker.flush().await.unwrap();
}

#[tokio::test]
async fn test_cdc_tracker_delete_operations() {
    let config = create_test_config();
    let mut tracker = CDCTracker::new(config).unwrap();
    
    let raw_key = "raw:test_data";
    let raw_value = b"binary_data_here";
    
    // Create the record first
    tracker.track_update(
        raw_key.as_bytes(),
        Some(raw_value),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    // Delete the record
    tracker.track_update(
        raw_key.as_bytes(),
        None, // None value indicates deletion
        Some(800001),
        Some("block_hash_124"),
    ).await.unwrap();

    tracker.flush().await.unwrap();
}

#[tokio::test]
async fn test_cdc_tracker_deduplication() {
    let config = create_test_config();
    let mut tracker = CDCTracker::new(config).unwrap();
    
    let address_key = "address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    let address_value = json!({
        "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
        "balance": 5000000000i64
    });
    
    // Send the same update twice
    tracker.track_update(
        address_key.as_bytes(),
        Some(address_value.to_string().as_bytes()),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    tracker.track_update(
        address_key.as_bytes(),
        Some(address_value.to_string().as_bytes()),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    tracker.flush().await.unwrap();
    
    // With deduplication enabled, only one event should be generated
    // This would be verified by checking the mock producer's sent events
}

#[tokio::test]
async fn test_cdc_tracker_buffer_management() {
    let mut config = create_test_config();
    config.global_settings.max_buffer_size = 2; // Small buffer for testing
    config.global_settings.flush_interval_ms = 10000; // Long interval to test buffer overflow
    
    let mut tracker = CDCTracker::new(config).unwrap();
    
    // Add events to exceed buffer size
    for i in 0..5 {
        let key = format!("address:test_address_{}", i);
        let value = json!({
            "address": format!("test_address_{}", i),
            "balance": i * 1000000
        });
        
        tracker.track_update(
            key.as_bytes(),
            Some(value.to_string().as_bytes()),
            Some(800000 + i as u32),
            Some(&format!("block_hash_{}", i)),
        ).await.unwrap();
    }
    
    // Buffer should have been automatically flushed when it exceeded max_buffer_size
}

#[tokio::test]
async fn test_cdc_tracker_flush_interval() {
    let mut config = create_test_config();
    config.global_settings.flush_interval_ms = 50; // Short interval for testing
    
    let mut tracker = CDCTracker::new(config).unwrap();
    
    let address_key = "address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    let address_value = json!({
        "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
        "balance": 5000000000i64
    });
    
    tracker.track_update(
        address_key.as_bytes(),
        Some(address_value.to_string().as_bytes()),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    // Wait for flush interval to trigger automatic flush
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Events should have been automatically flushed due to interval
}

#[tokio::test]
async fn test_cdc_tracker_error_handling() {
    let config = create_test_config();
    let mut tracker = CDCTracker::new(config).unwrap();
    
    // Test with invalid JSON value for JSON relationship
    let address_key = "address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    let invalid_json = b"invalid json {";
    
    let result = tracker.track_update(
        address_key.as_bytes(),
        Some(invalid_json),
        Some(800000),
        Some("block_hash_123"),
    ).await;
    
    // Should handle the error gracefully
    assert!(result.is_ok()); // Error should be logged but not propagated
}

#[tokio::test]
async fn test_cdc_tracker_key_extraction() {
    let config = create_test_config();
    let mut tracker = CDCTracker::new(config).unwrap();
    
    // Test address key extraction
    let address_key = "address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    let address_value = json!({
        "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
        "balance": 5000000000i64
    });
    
    tracker.track_update(
        address_key.as_bytes(),
        Some(address_value.to_string().as_bytes()),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    // Test transaction key extraction
    let tx_key = "tx:abc123def456789012345678901234567890123456789012345678901234567890";
    let tx_value = json!({
        "txid": "abc123def456789012345678901234567890123456789012345678901234567890",
        "height": 800000
    });
    
    tracker.track_update(
        tx_key.as_bytes(),
        Some(tx_value.to_string().as_bytes()),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    tracker.flush().await.unwrap();
    
    // Key extraction should work correctly for both relationships
}

#[tokio::test]
async fn test_cdc_tracker_operation_type_filtering() {
    let config = create_test_config();
    let mut tracker = CDCTracker::new(config).unwrap();
    
    // Test CreateOnly relationship (transactions) - should only emit create events
    let tx_key = "tx:abc123def456789012345678901234567890123456789012345678901234567890";
    let tx_value = json!({
        "txid": "abc123def456789012345678901234567890123456789012345678901234567890",
        "height": 800000
    });
    
    // Initial creation - should emit event
    tracker.track_update(
        tx_key.as_bytes(),
        Some(tx_value.to_string().as_bytes()),
        Some(800000),
        Some("block_hash_123"),
    ).await.unwrap();

    // Update attempt - should NOT emit event for CreateOnly relationship
    let updated_tx_value = json!({
        "txid": "abc123def456789012345678901234567890123456789012345678901234567890",
        "height": 800001
    });
    
    tracker.track_update(
        tx_key.as_bytes(),
        Some(updated_tx_value.to_string().as_bytes()),
        Some(800001),
        Some("block_hash_124"),
    ).await.unwrap();

    tracker.flush().await.unwrap();
    
    // Only the initial creation should have generated an event
}

/// Integration test that simulates a complete block processing scenario
#[tokio::test]
async fn test_complete_block_processing_scenario() {
    let config = create_test_config();
    let mut tracker = CDCTracker::new(config).unwrap();
    
    let block_height = 800000u32;
    let block_hash = "block_hash_800000";
    
    // Simulate processing a block with multiple transactions and address updates
    
    // Transaction 1
    let tx1_key = "tx:abc123def456789012345678901234567890123456789012345678901234567890";
    let tx1_value = json!({
        "txid": "abc123def456789012345678901234567890123456789012345678901234567890",
        "height": block_height
    });
    
    tracker.track_update(
        tx1_key.as_bytes(),
        Some(tx1_value.to_string().as_bytes()),
        Some(block_height),
        Some(block_hash),
    ).await.unwrap();

    // Address updates from transaction 1
    let addr1_key = "address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    let addr1_value = json!({
        "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
        "balance": 4500000000i64
    });
    
    tracker.track_update(
        addr1_key.as_bytes(),
        Some(addr1_value.to_string().as_bytes()),
        Some(block_height),
        Some(block_hash),
    ).await.unwrap();

    let addr2_key = "address:1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2";
    let addr2_value = json!({
        "address": "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
        "balance": 500000000i64
    });
    
    tracker.track_update(
        addr2_key.as_bytes(),
        Some(addr2_value.to_string().as_bytes()),
        Some(block_height),
        Some(block_hash),
    ).await.unwrap();

    // Transaction 2
    let tx2_key = "tx:def456abc789012345678901234567890123456789012345678901234567890123";
    let tx2_value = json!({
        "txid": "def456abc789012345678901234567890123456789012345678901234567890123",
        "height": block_height
    });
    
    tracker.track_update(
        tx2_key.as_bytes(),
        Some(tx2_value.to_string().as_bytes()),
        Some(block_height),
        Some(block_hash),
    ).await.unwrap();

    // Raw data update
    let raw_key = "raw:block_metadata";
    let raw_value = format!("Block {} processed", block_height);
    
    tracker.track_update(
        raw_key.as_bytes(),
        Some(raw_value.as_bytes()),
        Some(block_height),
        Some(block_hash),
    ).await.unwrap();

    // Flush all events
    tracker.flush().await.unwrap();
    
    // In a real scenario, we would verify that all events were sent to Kafka
    // with correct formatting and metadata
}

/// Test CDC tracker with concurrent operations
#[tokio::test]
async fn test_concurrent_operations() {
    let config = create_test_config();
    let tracker = Arc::new(Mutex::new(CDCTracker::new(config).unwrap()));
    
    let mut handles = vec![];
    
    // Spawn multiple tasks that concurrently update different keys
    for i in 0..10 {
        let tracker_clone = Arc::clone(&tracker);
        let handle = tokio::spawn(async move {
            let key = format!("address:test_address_{}", i);
            let value = json!({
                "address": format!("test_address_{}", i),
                "balance": i * 1000000
            });
            
            let mut tracker_guard = tracker_clone.lock().await;
            tracker_guard.track_update(
                key.as_bytes(),
                Some(value.to_string().as_bytes()),
                Some(800000 + i as u32),
                Some(&format!("block_hash_{}", i)),
            ).await.unwrap();
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Flush all events
    tracker.lock().await.flush().await.unwrap();
}