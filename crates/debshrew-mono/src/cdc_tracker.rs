//! # CDC Event Tracker
//!
//! ## PURPOSE
//! Core CDC functionality that intercepts k/v updates during block processing,
//! transforms them into relational CDC events, and buffers them for Kafka streaming.
//!
//! ## KEY CONCEPTS
//! - CDCTracker: Main component that processes k/v updates and generates CDC events
//! - CDCEvent: Structured representation of a database change event
//! - Event buffering: Collects events during block processing for atomic flushing
//! - Debezium compatibility: Events formatted for Debezium consumption

use crate::cdc_config::{CDCConfig, CDCRelationship, OperationType, ValueFormat};
use crate::kafka_producer::KafkaProducer;
use crate::relationship_matcher::RelationshipMatcher;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use metashrew_runtime::KVTrackerFn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Main CDC tracker that processes k/v updates and generates CDC events
#[derive(Clone)]
pub struct CDCTracker {
    /// CDC configuration
    config: CDCConfig,
    /// Kafka producer for streaming events
    kafka_producer: Arc<Mutex<KafkaProducer>>,
    /// Matcher for finding relationships that apply to keys
    relationship_matcher: RelationshipMatcher,
    /// Buffer for events during current block processing
    event_buffer: Arc<Mutex<Vec<CDCEvent>>>,
    /// Current block height being processed
    current_height: Arc<Mutex<u32>>,
    /// Current block hash being processed
    current_block_hash: Arc<Mutex<Option<String>>>,
    /// Previous values cache for UPDATE detection
    previous_values: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

/// Structured representation of a CDC event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CDCEvent {
    /// Name of the relationship that generated this event
    pub relationship_name: String,
    /// Target table name
    pub table_name: String,
    /// Type of CDC operation
    pub operation: CDCOperation,
    /// Previous value (for UPDATE and DELETE operations)
    pub before: Option<serde_json::Value>,
    /// New value (for CREATE and UPDATE operations)
    pub after: Option<serde_json::Value>,
    /// Primary key values
    pub primary_key: serde_json::Value,
    /// Block height where this change occurred
    pub block_height: u32,
    /// Block hash where this change occurred
    pub block_hash: Option<String>,
    /// Timestamp when the event was created
    pub timestamp: DateTime<Utc>,
    /// Unique transaction ID for this event
    pub transaction_id: String,
    /// Source key that triggered this event
    pub source_key: String,
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

/// Types of CDC operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CDCOperation {
    /// Record creation
    Create,
    /// Record update
    Update,
    /// Record deletion
    Delete,
}

/// Debezium-compatible event structure
#[derive(Debug, Serialize)]
pub struct DebeziumEvent {
    /// Schema information
    pub schema: serde_json::Value,
    /// Event payload
    pub payload: DebeziumPayload,
}

/// Debezium event payload
#[derive(Debug, Serialize)]
pub struct DebeziumPayload {
    /// Previous value (for UPDATE and DELETE)
    pub before: Option<serde_json::Value>,
    /// New value (for CREATE and UPDATE)
    pub after: Option<serde_json::Value>,
    /// Source metadata
    pub source: DebeziumSource,
    /// Operation type
    pub op: String,
    /// Timestamp in milliseconds
    pub ts_ms: i64,
    /// Transaction information
    pub transaction: Option<DebeziumTransaction>,
}

/// Debezium source metadata
#[derive(Debug, Serialize)]
pub struct DebeziumSource {
    /// Connector version
    pub version: String,
    /// Connector name
    pub connector: String,
    /// Source name
    pub name: String,
    /// Timestamp in milliseconds
    pub ts_ms: i64,
    /// Snapshot indicator
    pub snapshot: String,
    /// Database name
    pub db: String,
    /// Table name
    pub table: String,
    /// Server ID
    pub server_id: u32,
    /// File name (block identifier)
    pub file: String,
    /// Position in file
    pub pos: u64,
    /// Row number
    pub row: u32,
}

/// Debezium transaction information
#[derive(Debug, Serialize)]
pub struct DebeziumTransaction {
    /// Transaction ID
    pub id: String,
    /// Total order within transaction
    pub total_order: u64,
    /// Data collection order
    pub data_collection_order: u64,
}

impl CDCTracker {
    /// Create a new CDC tracker with the given configuration
    pub async fn new(config: CDCConfig) -> Result<Self> {
        let kafka_producer = KafkaProducer::new(&config.kafka_config).await?;
        let relationship_matcher = RelationshipMatcher::new(&config.relationships);

        Ok(Self {
            config,
            kafka_producer: Arc::new(Mutex::new(kafka_producer)),
            relationship_matcher,
            event_buffer: Arc::new(Mutex::new(Vec::new())),
            current_height: Arc::new(Mutex::new(0)),
            current_block_hash: Arc::new(Mutex::new(None)),
            previous_values: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Set the current block height being processed
    pub fn set_current_height(&self, height: u32) {
        if let Ok(mut current_height) = self.current_height.lock() {
            *current_height = height;
        }
    }

    /// Set the current block hash being processed
    pub fn set_current_block_hash(&self, block_hash: String) {
        if let Ok(mut current_block_hash) = self.current_block_hash.lock() {
            *current_block_hash = Some(block_hash);
        }
    }

    /// Create a KV tracker function that can be used with the runtime
    pub fn create_kv_tracker(&self) -> KVTrackerFn {
        let tracker = self.clone();
        Box::new(move |key: Vec<u8>, value: Vec<u8>| {
            if let Err(e) = tracker.track_kv_change(key, value) {
                log::error!("CDC tracking error: {}", e);
            }
        })
    }

    /// Process a k/v change and generate CDC events
    fn track_kv_change(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        // Find matching relationships for this key
        let matches = self.relationship_matcher.find_matches(&key);
        
        if matches.is_empty() {
            return Ok(()); // No relationships match this key
        }

        // Get current context
        let height = self.current_height.lock().unwrap().clone();
        let block_hash = self.current_block_hash.lock().unwrap().clone();

        // Process each matching relationship
        for relationship in matches {
            let event = self.create_cdc_event(relationship, &key, &value, height, &block_hash)?;
            
            // Add to event buffer
            if let Ok(mut buffer) = self.event_buffer.lock() {
                buffer.push(event);
            }
        }

        // Update previous values cache for future UPDATE detection
        if let Ok(mut cache) = self.previous_values.lock() {
            cache.insert(key, value);
        }

        Ok(())
    }

    /// Create a CDC event from a k/v change
    fn create_cdc_event(
        &self,
        relationship: &CDCRelationship,
        key: &[u8],
        value: &[u8],
        height: u32,
        block_hash: &Option<String>,
    ) -> Result<CDCEvent> {
        // Extract primary key from k/v key
        let primary_key = self.extract_primary_key(relationship, key)?;
        
        // Parse value according to relationship mapping
        let after_value = self.parse_value(relationship, value)?;
        
        // Determine operation type
        let (operation, before_value) = self.determine_operation(relationship, key, &after_value)?;
        
        // Generate unique transaction ID
        let transaction_id = format!("{}:{}", height, Uuid::new_v4());
        
        Ok(CDCEvent {
            relationship_name: relationship.name.clone(),
            table_name: relationship.table_name.clone(),
            operation,
            before: before_value,
            after: Some(after_value),
            primary_key: serde_json::to_value(primary_key)?,
            block_height: height,
            block_hash: block_hash.clone(),
            timestamp: Utc::now(),
            transaction_id,
            source_key: String::from_utf8_lossy(key).to_string(),
            metadata: None,
        })
    }

    /// Extract primary key values from a k/v key using the relationship mapping
    fn extract_primary_key(
        &self,
        relationship: &CDCRelationship,
        key: &[u8],
    ) -> Result<HashMap<String, String>> {
        relationship.extract_primary_key(key)
    }

    /// Parse k/v value according to the relationship's value mapping
    fn parse_value(
        &self,
        relationship: &CDCRelationship,
        value: &[u8],
    ) -> Result<serde_json::Value> {
        match &relationship.value_mapping.value_format {
            ValueFormat::Json => {
                serde_json::from_slice(value)
                    .map_err(|e| anyhow!("Failed to parse JSON value: {}", e))
            }
            ValueFormat::Raw => {
                // For raw values, create a simple object with the hex-encoded value
                Ok(serde_json::json!({
                    "raw_value": hex::encode(value),
                    "raw_length": value.len()
                }))
            }
            ValueFormat::Protobuf { schema_name: _ } => {
                // TODO: Implement protobuf parsing
                Err(anyhow!("Protobuf parsing not yet implemented"))
            }
            ValueFormat::Custom { parser_name: _ } => {
                // TODO: Implement custom parser registry
                Err(anyhow!("Custom parsers not yet implemented"))
            }
        }
    }

    /// Determine the CDC operation type based on previous values and configuration
    fn determine_operation(
        &self,
        relationship: &CDCRelationship,
        key: &[u8],
        _after_value: &serde_json::Value,
    ) -> Result<(CDCOperation, Option<serde_json::Value>)> {
        match &relationship.operation_type {
            OperationType::CreateOnly => {
                // Always CREATE for append-only relationships
                Ok((CDCOperation::Create, None))
            }
            OperationType::CreateUpdate => {
                // Check if we have a previous value for this key
                if let Ok(cache) = self.previous_values.lock() {
                    if let Some(previous_value) = cache.get(key) {
                        // Parse previous value
                        let before_value = self.parse_value(relationship, previous_value)?;
                        Ok((CDCOperation::Update, Some(before_value)))
                    } else {
                        Ok((CDCOperation::Create, None))
                    }
                } else {
                    Ok((CDCOperation::Create, None))
                }
            }
            OperationType::CreateUpdateDelete => {
                // TODO: Implement DELETE detection (e.g., tombstone values)
                // For now, treat as CreateUpdate
                if let Ok(cache) = self.previous_values.lock() {
                    if let Some(previous_value) = cache.get(key) {
                        let before_value = self.parse_value(relationship, previous_value)?;
                        Ok((CDCOperation::Update, Some(before_value)))
                    } else {
                        Ok((CDCOperation::Create, None))
                    }
                } else {
                    Ok((CDCOperation::Create, None))
                }
            }
            OperationType::Custom(_) => {
                // TODO: Implement custom operation logic
                Err(anyhow!("Custom operation types not yet implemented"))
            }
        }
    }

    /// Flush all buffered events to Kafka
    pub async fn flush_events(&self) -> Result<()> {
        let events = {
            let mut buffer = self.event_buffer.lock().unwrap();
            let events = buffer.clone();
            buffer.clear();
            events
        };

        if events.is_empty() {
            return Ok(());
        }

        log::info!("Flushing {} CDC events to Kafka", events.len());

        let kafka_producer = self.kafka_producer.lock().unwrap();
        for event in events {
            kafka_producer.send_event(&event, &self.config).await?;
        }

        Ok(())
    }

    /// Get the number of buffered events
    pub fn get_buffer_size(&self) -> usize {
        self.event_buffer.lock().unwrap().len()
    }

    /// Clear the event buffer (useful for testing)
    pub fn clear_buffer(&self) {
        self.event_buffer.lock().unwrap().clear();
    }

    /// Convert a CDC event to Debezium format
    pub fn to_debezium_event(&self, event: &CDCEvent) -> Result<DebeziumEvent> {
        let schema = self.generate_schema_for_table(&event.table_name)?;
        
        let payload = DebeziumPayload {
            before: event.before.clone(),
            after: event.after.clone(),
            source: DebeziumSource {
                version: "1.0.0".to_string(),
                connector: "debshrew".to_string(),
                name: event.relationship_name.clone(),
                ts_ms: event.timestamp.timestamp_millis(),
                snapshot: "false".to_string(),
                db: "metashrew".to_string(),
                table: event.table_name.clone(),
                server_id: 0,
                file: format!("block-{}", event.block_height),
                pos: 0,
                row: 0,
            },
            op: match event.operation {
                CDCOperation::Create => "c",
                CDCOperation::Update => "u",
                CDCOperation::Delete => "d",
            }.to_string(),
            ts_ms: event.timestamp.timestamp_millis(),
            transaction: Some(DebeziumTransaction {
                id: event.transaction_id.clone(),
                total_order: 1,
                data_collection_order: 1,
            }),
        };

        Ok(DebeziumEvent { schema, payload })
    }

    /// Generate a basic schema for a table (simplified implementation)
    fn generate_schema_for_table(&self, table_name: &str) -> Result<serde_json::Value> {
        // Find the relationship for this table
        let relationship = self.config.relationships
            .iter()
            .find(|r| r.table_name == table_name)
            .ok_or_else(|| anyhow!("No relationship found for table: {}", table_name))?;

        // Generate a basic Avro-style schema
        let mut fields = Vec::new();
        
        // Add primary key fields
        for pk_field in &relationship.key_mapping.primary_key_fields {
            fields.push(serde_json::json!({
                "name": pk_field,
                "type": "string",
                "optional": false
            }));
        }
        
        // Add value fields
        for field in &relationship.value_mapping.fields {
            fields.push(serde_json::json!({
                "name": field.name,
                "type": self.sql_type_to_avro_type(&field.data_type),
                "optional": field.nullable
            }));
        }

        Ok(serde_json::json!({
            "type": "struct",
            "fields": fields,
            "optional": false,
            "name": format!("{}.{}.Value", self.config.kafka_config.topic_prefix, table_name)
        }))
    }

    /// Convert SQL data types to Avro types (simplified mapping)
    fn sql_type_to_avro_type(&self, sql_type: &str) -> &'static str {
        match sql_type.to_lowercase().as_str() {
            "bigint" | "int8" => "long",
            "integer" | "int4" | "int" => "int",
            "smallint" | "int2" => "int",
            "boolean" | "bool" => "boolean",
            "real" | "float4" => "float",
            "double" | "float8" => "double",
            "text" | "varchar" | "char" => "string",
            "bytea" | "blob" => "bytes",
            _ => "string", // Default to string for unknown types
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdc_config::{FieldMapping, KeyMapping, ValueMapping};

    fn create_test_relationship() -> CDCRelationship {
        CDCRelationship {
            name: "test_addresses".to_string(),
            key_pattern: "^address:(.+)$".to_string(),
            table_name: "addresses".to_string(),
            key_mapping: KeyMapping {
                primary_key_fields: vec!["address".to_string()],
                extraction_pattern: "^address:(.+)$".to_string(),
                transformations: None,
            },
            value_mapping: ValueMapping {
                fields: vec![
                    FieldMapping {
                        name: "balance".to_string(),
                        path: "$.balance".to_string(),
                        data_type: "bigint".to_string(),
                        nullable: false,
                        transformations: None,
                    }
                ],
                value_format: ValueFormat::Json,
                defaults: None,
            },
            operation_type: OperationType::CreateUpdate,
            filters: None,
            transformations: None,
        }
    }

    #[test]
    fn test_primary_key_extraction() {
        let relationship = create_test_relationship();
        let tracker = CDCTracker {
            config: CDCConfig::default(),
            kafka_producer: Arc::new(Mutex::new(KafkaProducer::mock())),
            relationship_matcher: RelationshipMatcher::new(&[]),
            event_buffer: Arc::new(Mutex::new(Vec::new())),
            current_height: Arc::new(Mutex::new(0)),
            current_block_hash: Arc::new(Mutex::new(None)),
            previous_values: Arc::new(Mutex::new(HashMap::new())),
        };

        let key = b"address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let pk = tracker.extract_primary_key(&relationship, key).unwrap();
        
        assert_eq!(pk.get("address").unwrap(), "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa");
    }

    #[test]
    fn test_value_parsing() {
        let relationship = create_test_relationship();
        let tracker = CDCTracker {
            config: CDCConfig::default(),
            kafka_producer: Arc::new(Mutex::new(KafkaProducer::mock())),
            relationship_matcher: RelationshipMatcher::new(&[]),
            event_buffer: Arc::new(Mutex::new(Vec::new())),
            current_height: Arc::new(Mutex::new(0)),
            current_block_hash: Arc::new(Mutex::new(None)),
            previous_values: Arc::new(Mutex::new(HashMap::new())),
        };

        let value = br#"{"balance": 1000000, "tx_count": 5}"#;
        let parsed = tracker.parse_value(&relationship, value).unwrap();
        
        assert_eq!(parsed["balance"], 1000000);
        assert_eq!(parsed["tx_count"], 5);
    }
}