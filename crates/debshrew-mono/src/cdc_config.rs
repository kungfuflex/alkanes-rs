//! # CDC Configuration Management
//!
//! ## PURPOSE
//! Defines configuration structures for Change Data Capture functionality,
//! including relationship mappings, Kafka settings, and schema management.
//!
//! ## KEY CONCEPTS
//! - CDCRelationship: Maps k/v patterns to relational table structures
//! - OperationType: Defines how changes are tracked (CREATE/UPDATE/DELETE)
//! - ValueMapping: Specifies how to extract fields from k/v values
//! - KafkaConfig: Kafka producer and topic configuration

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Complete CDC configuration for debshrew-mono
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CDCConfig {
    /// List of relationship mappings between k/v pairs and tables
    pub relationships: Vec<CDCRelationship>,
    /// Kafka producer configuration
    pub kafka_config: KafkaConfig,
    /// Optional schema registry configuration
    pub schema_registry: Option<SchemaRegistryConfig>,
    /// Global CDC settings
    pub global_settings: GlobalCDCSettings,
}

/// Defines how k/v pairs map to relational table structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CDCRelationship {
    /// Unique name for this relationship
    pub name: String,
    /// Regex pattern to match k/v keys
    pub key_pattern: String,
    /// Target table name in the destination system
    pub table_name: String,
    /// How to extract primary key from k/v key
    pub key_mapping: KeyMapping,
    /// How to parse and map the k/v value
    pub value_mapping: ValueMapping,
    /// Type of CDC operations to generate
    pub operation_type: OperationType,
    /// Optional filtering conditions
    pub filters: Option<Vec<FilterCondition>>,
    /// Custom transformation rules
    pub transformations: Option<Vec<TransformationRule>>,
}

/// Defines how CDC operations are tracked for a relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    /// Only CREATE operations (append-only)
    CreateOnly,
    /// CREATE and UPDATE operations (track changes)
    CreateUpdate,
    /// CREATE, UPDATE, and DELETE operations (full CDC)
    CreateUpdateDelete,
    /// Custom operation logic
    Custom(String),
}

/// Defines how to extract primary key fields from k/v keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMapping {
    /// Names of primary key fields in the target table
    pub primary_key_fields: Vec<String>,
    /// Regex pattern with capture groups for extracting PK values
    pub extraction_pattern: String,
    /// Optional key transformations
    pub transformations: Option<Vec<KeyTransformation>>,
}

/// Defines how to parse and map k/v values to table fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueMapping {
    /// List of field mappings
    pub fields: Vec<FieldMapping>,
    /// Format of the k/v value
    pub value_format: ValueFormat,
    /// Optional default values for missing fields
    pub defaults: Option<HashMap<String, serde_json::Value>>,
}

/// Supported value formats for k/v values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueFormat {
    /// JSON-encoded values
    Json,
    /// Protocol Buffer encoded values
    Protobuf { schema_name: String },
    /// Raw binary values
    Raw,
    /// Custom parsing logic
    Custom { parser_name: String },
}

/// Maps a field from k/v value to table column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    /// Target column name
    pub name: String,
    /// JSONPath or field path in the value
    pub path: String,
    /// SQL data type for the field
    pub data_type: String,
    /// Whether the field is nullable
    pub nullable: bool,
    /// Optional field transformations
    pub transformations: Option<Vec<FieldTransformation>>,
}

/// Filtering conditions for CDC events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterCondition {
    /// Field to filter on
    pub field: String,
    /// Filter operator
    pub operator: FilterOperator,
    /// Filter value
    pub value: serde_json::Value,
}

/// Supported filter operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
    Regex,
}

/// Transformation rules for field values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformationRule {
    /// Target field name
    pub field: String,
    /// Transformation type
    pub transformation: TransformationType,
}

/// Types of transformations that can be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransformationType {
    /// Convert to uppercase
    Uppercase,
    /// Convert to lowercase
    Lowercase,
    /// Apply regex replacement
    RegexReplace { pattern: String, replacement: String },
    /// Custom transformation function
    Custom { function_name: String },
}

/// Key-specific transformations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyTransformation {
    /// Field index in the extracted key
    pub field_index: usize,
    /// Transformation to apply
    pub transformation: TransformationType,
}

/// Field-specific transformations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldTransformation {
    /// Transformation to apply
    pub transformation: TransformationType,
}

/// Kafka producer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    /// Kafka bootstrap servers
    pub bootstrap_servers: String,
    /// Topic prefix for CDC events
    pub topic_prefix: String,
    /// Producer acknowledgment settings
    pub acks: String,
    /// Compression type (none, gzip, snappy, lz4, zstd)
    pub compression: Option<String>,
    /// Batch size for producer
    pub batch_size: Option<usize>,
    /// Linger time in milliseconds
    pub linger_ms: Option<u64>,
    /// Additional Kafka producer properties
    pub additional_properties: Option<HashMap<String, String>>,
}

/// Schema registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaRegistryConfig {
    /// Schema registry URL
    pub url: String,
    /// Authentication settings
    pub auth: Option<SchemaRegistryAuth>,
    /// Subject naming strategy
    pub subject_naming_strategy: SubjectNamingStrategy,
}

/// Schema registry authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaRegistryAuth {
    /// Username for basic auth
    pub username: String,
    /// Password for basic auth
    pub password: String,
}

/// Subject naming strategies for schema registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubjectNamingStrategy {
    /// Use topic name as subject
    TopicName,
    /// Use table name as subject
    TableName,
    /// Custom naming pattern
    Custom { pattern: String },
}

/// Global CDC settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalCDCSettings {
    /// Maximum number of events to buffer before flushing
    pub max_buffer_size: usize,
    /// Maximum time to wait before flushing events (milliseconds)
    pub flush_interval_ms: u64,
    /// Number of retry attempts for failed Kafka sends
    pub retry_attempts: u32,
    /// Backoff time between retries (milliseconds)
    pub retry_backoff_ms: u64,
    /// Whether to enable CDC event deduplication
    pub enable_deduplication: bool,
    /// Whether to include block metadata in events
    pub include_block_metadata: bool,
}

impl Default for CDCConfig {
    fn default() -> Self {
        Self {
            relationships: Vec::new(),
            kafka_config: KafkaConfig::default(),
            schema_registry: None,
            global_settings: GlobalCDCSettings::default(),
        }
    }
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            topic_prefix: "metashrew.cdc".to_string(),
            acks: "all".to_string(),
            compression: Some("gzip".to_string()),
            batch_size: Some(1000),
            linger_ms: Some(100),
            additional_properties: None,
        }
    }
}

impl Default for GlobalCDCSettings {
    fn default() -> Self {
        Self {
            max_buffer_size: 1000,
            flush_interval_ms: 1000,
            retry_attempts: 3,
            retry_backoff_ms: 1000,
            enable_deduplication: true,
            include_block_metadata: true,
        }
    }
}

impl CDCConfig {
    /// Load CDC configuration from a YAML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow!("Failed to read CDC config file: {}", e))?;
        
        serde_yaml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse CDC config: {}", e))
    }

    /// Save CDC configuration to a YAML file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self)
            .map_err(|e| anyhow!("Failed to serialize CDC config: {}", e))?;
        
        std::fs::write(path.as_ref(), content)
            .map_err(|e| anyhow!("Failed to write CDC config file: {}", e))
    }

    /// Validate the configuration for consistency and completeness
    pub fn validate(&self) -> Result<()> {
        // Check for duplicate relationship names
        let mut names = std::collections::HashSet::new();
        for relationship in &self.relationships {
            if !names.insert(&relationship.name) {
                return Err(anyhow!("Duplicate relationship name: {}", relationship.name));
            }
        }

        // Validate each relationship
        for relationship in &self.relationships {
            relationship.validate()?;
        }

        // Validate Kafka configuration
        self.kafka_config.validate()?;

        Ok(())
    }

    /// Get relationship by name
    pub fn get_relationship(&self, name: &str) -> Option<&CDCRelationship> {
        self.relationships.iter().find(|r| r.name == name)
    }

    /// Get all relationships that match a given key
    pub fn get_matching_relationships(&self, key: &[u8]) -> Vec<&CDCRelationship> {
        let key_str = String::from_utf8_lossy(key);
        self.relationships
            .iter()
            .filter(|r| {
                if let Ok(regex) = regex::Regex::new(&r.key_pattern) {
                    regex.is_match(&key_str)
                } else {
                    false
                }
            })
            .collect()
    }
}

impl CDCRelationship {
    /// Validate the relationship configuration
    pub fn validate(&self) -> Result<()> {
        // Validate regex pattern
        regex::Regex::new(&self.key_pattern)
            .map_err(|e| anyhow!("Invalid key pattern '{}': {}", self.key_pattern, e))?;

        // Validate extraction pattern
        regex::Regex::new(&self.key_mapping.extraction_pattern)
            .map_err(|e| anyhow!("Invalid extraction pattern '{}': {}", self.key_mapping.extraction_pattern, e))?;

        // Validate field mappings
        for field in &self.value_mapping.fields {
            field.validate()?;
        }

        Ok(())
    }

    /// Extract primary key values from a k/v key
    pub fn extract_primary_key(&self, key: &[u8]) -> Result<HashMap<String, String>> {
        let key_str = String::from_utf8_lossy(key);
        let regex = regex::Regex::new(&self.key_mapping.extraction_pattern)?;
        
        if let Some(captures) = regex.captures(&key_str) {
            let mut result = HashMap::new();
            
            for (i, field_name) in self.key_mapping.primary_key_fields.iter().enumerate() {
                if let Some(capture) = captures.get(i + 1) {
                    result.insert(field_name.clone(), capture.as_str().to_string());
                } else {
                    return Err(anyhow!("Missing capture group {} for field '{}'", i + 1, field_name));
                }
            }
            
            Ok(result)
        } else {
            Err(anyhow!("Key '{}' does not match extraction pattern '{}'", key_str, self.key_mapping.extraction_pattern))
        }
    }
}

impl FieldMapping {
    /// Validate the field mapping
    pub fn validate(&self) -> Result<()> {
        // Basic validation - could be extended with more sophisticated checks
        if self.name.is_empty() {
            return Err(anyhow!("Field name cannot be empty"));
        }
        
        if self.path.is_empty() {
            return Err(anyhow!("Field path cannot be empty"));
        }
        
        Ok(())
    }
}

impl KafkaConfig {
    /// Validate the Kafka configuration
    pub fn validate(&self) -> Result<()> {
        if self.bootstrap_servers.is_empty() {
            return Err(anyhow!("Kafka bootstrap servers cannot be empty"));
        }
        
        if self.topic_prefix.is_empty() {
            return Err(anyhow!("Kafka topic prefix cannot be empty"));
        }
        
        Ok(())
    }

    /// Get the full topic name for a table
    pub fn get_topic_name(&self, table_name: &str) -> String {
        format!("{}.{}", self.topic_prefix, table_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdc_config_default() {
        let config = CDCConfig::default();
        assert_eq!(config.kafka_config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.kafka_config.topic_prefix, "metashrew.cdc");
    }

    #[test]
    fn test_relationship_validation() {
        let relationship = CDCRelationship {
            name: "test".to_string(),
            key_pattern: "^address:(.+)$".to_string(),
            table_name: "addresses".to_string(),
            key_mapping: KeyMapping {
                primary_key_fields: vec!["address".to_string()],
                extraction_pattern: "^address:(.+)$".to_string(),
                transformations: None,
            },
            value_mapping: ValueMapping {
                fields: vec![FieldMapping {
                    name: "balance".to_string(),
                    path: "$.balance".to_string(),
                    data_type: "bigint".to_string(),
                    nullable: false,
                    transformations: None,
                }],
                value_format: ValueFormat::Json,
                defaults: None,
            },
            operation_type: OperationType::CreateUpdate,
            filters: None,
            transformations: None,
        };

        assert!(relationship.validate().is_ok());
    }

    #[test]
    fn test_primary_key_extraction() {
        let relationship = CDCRelationship {
            name: "test".to_string(),
            key_pattern: "^address:(.+)$".to_string(),
            table_name: "addresses".to_string(),
            key_mapping: KeyMapping {
                primary_key_fields: vec!["address".to_string()],
                extraction_pattern: "^address:(.+)$".to_string(),
                transformations: None,
            },
            value_mapping: ValueMapping {
                fields: vec![],
                value_format: ValueFormat::Json,
                defaults: None,
            },
            operation_type: OperationType::CreateUpdate,
            filters: None,
            transformations: None,
        };

        let key = b"address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let extracted = relationship.extract_primary_key(key).unwrap();
        
        assert_eq!(extracted.get("address").unwrap(), "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa");
    }
}