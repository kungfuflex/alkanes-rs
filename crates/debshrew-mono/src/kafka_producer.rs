//! # Kafka Producer for CDC Events
//!
//! ## PURPOSE
//! Handles streaming of CDC events to Kafka topics in Debezium-compatible format.
//! Provides reliable delivery with retry logic and proper error handling.

use crate::cdc_config::{CDCConfig, KafkaConfig};
use crate::cdc_tracker::{CDCEvent, DebeziumEvent};
use anyhow::{anyhow, Result};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use serde_json;
use std::time::Duration;

/// Kafka producer for streaming CDC events
pub struct KafkaProducer {
    producer: FutureProducer,
    config: KafkaConfig,
}

impl KafkaProducer {
    /// Create a new Kafka producer with the given configuration
    pub async fn new(config: &KafkaConfig) -> Result<Self> {
        let mut client_config = ClientConfig::new();
        
        // Basic configuration
        client_config
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("acks", &config.acks)
            .set("retries", "3")
            .set("retry.backoff.ms", "1000")
            .set("delivery.timeout.ms", "30000");

        // Optional compression
        if let Some(compression) = &config.compression {
            client_config.set("compression.type", compression);
        }

        // Optional batch size
        if let Some(batch_size) = config.batch_size {
            client_config.set("batch.size", &batch_size.to_string());
        }

        // Optional linger time
        if let Some(linger_ms) = config.linger_ms {
            client_config.set("linger.ms", &linger_ms.to_string());
        }

        // Additional properties
        if let Some(additional_props) = &config.additional_properties {
            for (key, value) in additional_props {
                client_config.set(key, value);
            }
        }

        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| anyhow!("Failed to create Kafka producer: {}", e))?;

        Ok(Self {
            producer,
            config: config.clone(),
        })
    }

    /// Send a CDC event to Kafka
    pub async fn send_event(&self, event: &CDCEvent, cdc_config: &CDCConfig) -> Result<()> {
        // Convert to Debezium format
        let debezium_event = self.to_debezium_event(event)?;
        
        // Serialize the event
        let message = serde_json::to_string(&debezium_event)
            .map_err(|e| anyhow!("Failed to serialize CDC event: {}", e))?;
        
        // Create primary key for Kafka message key
        let key = serde_json::to_string(&event.primary_key)
            .map_err(|e| anyhow!("Failed to serialize primary key: {}", e))?;

        // Determine topic name
        let topic = self.config.get_topic_name(&event.table_name);

        // Create Kafka record
        let record = FutureRecord::to(&topic)
            .key(&key)
            .payload(&message)
            .headers(self.create_headers(event)?);

        // Send with retry logic
        let mut attempts = 0;
        let max_attempts = cdc_config.global_settings.retry_attempts;
        let retry_backoff = Duration::from_millis(cdc_config.global_settings.retry_backoff_ms);

        while attempts < max_attempts {
            match self.producer.send(record.clone(), Timeout::After(Duration::from_secs(10))).await {
                Ok((partition, offset)) => {
                    log::debug!(
                        "Successfully sent CDC event to topic '{}', partition {}, offset {}",
                        topic, partition, offset
                    );
                    return Ok(());
                }
                Err((kafka_error, _)) => {
                    attempts += 1;
                    log::warn!(
                        "Failed to send CDC event (attempt {}/{}): {}",
                        attempts, max_attempts, kafka_error
                    );
                    
                    if attempts < max_attempts {
                        tokio::time::sleep(retry_backoff).await;
                    } else {
                        return Err(anyhow!("Failed to send CDC event after {} attempts: {}", max_attempts, kafka_error));
                    }
                }
            }
        }

        Ok(())
    }

    /// Convert CDC event to Debezium format
    fn to_debezium_event(&self, event: &CDCEvent) -> Result<DebeziumEvent> {
        use crate::cdc_tracker::{DebeziumEvent, DebeziumPayload, DebeziumSource, DebeziumTransaction};

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
                crate::cdc_tracker::CDCOperation::Create => "c",
                crate::cdc_tracker::CDCOperation::Update => "u",
                crate::cdc_tracker::CDCOperation::Delete => "d",
            }.to_string(),
            ts_ms: event.timestamp.timestamp_millis(),
            transaction: Some(DebeziumTransaction {
                id: event.transaction_id.clone(),
                total_order: 1,
                data_collection_order: 1,
            }),
        };

        // Generate basic schema
        let schema = self.generate_basic_schema(&event.table_name)?;

        Ok(DebeziumEvent { schema, payload })
    }

    /// Create Kafka headers for the message
    fn create_headers(&self, event: &CDCEvent) -> Result<rdkafka::message::OwnedHeaders> {
        let mut headers = rdkafka::message::OwnedHeaders::new();
        
        headers = headers
            .insert(rdkafka::message::Header {
                key: "debshrew.source.table",
                value: Some(&event.table_name),
            })
            .insert(rdkafka::message::Header {
                key: "debshrew.source.relationship",
                value: Some(&event.relationship_name),
            })
            .insert(rdkafka::message::Header {
                key: "debshrew.block.height",
                value: Some(&event.block_height.to_string()),
            })
            .insert(rdkafka::message::Header {
                key: "debshrew.operation",
                value: Some(&format!("{:?}", event.operation)),
            });

        if let Some(block_hash) = &event.block_hash {
            headers = headers.insert(rdkafka::message::Header {
                key: "debshrew.block.hash",
                value: Some(block_hash),
            });
        }

        Ok(headers)
    }

    /// Generate a basic schema for the table
    fn generate_basic_schema(&self, table_name: &str) -> Result<serde_json::Value> {
        // This is a simplified schema - in production, you'd want more sophisticated schema management
        Ok(serde_json::json!({
            "type": "struct",
            "fields": [
                {
                    "name": "id",
                    "type": "string",
                    "optional": false
                }
            ],
            "optional": false,
            "name": format!("{}.{}.Value", self.config.topic_prefix, table_name)
        }))
    }

    /// Create a mock producer for testing
    #[cfg(test)]
    pub fn mock() -> Self {
        // Create a mock producer that doesn't actually send to Kafka
        let config = KafkaConfig::default();
        
        // This is a simplified mock - in real tests you'd use a proper mock
        let client_config = ClientConfig::new();
        let producer: FutureProducer = client_config.create().unwrap();
        
        Self { producer, config }
    }

    /// Flush any pending messages
    pub async fn flush(&self, timeout: Duration) -> Result<()> {
        self.producer.flush(Timeout::After(timeout))
            .map_err(|e| anyhow!("Failed to flush Kafka producer: {}", e))
    }
}

impl Drop for KafkaProducer {
    fn drop(&mut self) {
        // Attempt to flush any remaining messages
        let _ = self.producer.flush(Timeout::After(Duration::from_secs(5)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdc_tracker::{CDCEvent, CDCOperation};
    use chrono::Utc;

    fn create_test_event() -> CDCEvent {
        CDCEvent {
            relationship_name: "test_addresses".to_string(),
            table_name: "addresses".to_string(),
            operation: CDCOperation::Create,
            before: None,
            after: Some(serde_json::json!({"balance": 1000000})),
            primary_key: serde_json::json!({"address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"}),
            block_height: 100,
            block_hash: Some("00000000000000000007316856900e76b4f7a9139cfbfba89842c8d196cd5f91".to_string()),
            timestamp: Utc::now(),
            transaction_id: "100:test-tx-id".to_string(),
            source_key: "address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
            metadata: None,
        }
    }

    #[test]
    fn test_debezium_event_conversion() {
        let producer = KafkaProducer::mock();
        let event = create_test_event();
        
        let debezium_event = producer.to_debezium_event(&event).unwrap();
        
        assert_eq!(debezium_event.payload.op, "c");
        assert_eq!(debezium_event.payload.source.table, "addresses");
        assert_eq!(debezium_event.payload.source.file, "block-100");
        assert!(debezium_event.payload.after.is_some());
        assert!(debezium_event.payload.before.is_none());
    }

    #[test]
    fn test_headers_creation() {
        let producer = KafkaProducer::mock();
        let event = create_test_event();
        
        let headers = producer.create_headers(&event).unwrap();
        
        // Verify headers are created (detailed verification would require accessing header values)
        assert!(headers.count() > 0);
    }
}