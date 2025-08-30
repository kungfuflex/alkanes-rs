//! # Schema Manager
//!
//! ## PURPOSE
//! Manages schema evolution and registry integration for CDC events.
//! Handles schema generation, validation, and compatibility checking.

use anyhow::{anyhow, Result};
use serde_json;
use std::collections::HashMap;

/// Manages schemas for CDC events and tables
pub struct SchemaManager {
    /// Cache of generated schemas
    schema_cache: HashMap<String, serde_json::Value>,
    /// Schema registry client (optional)
    #[allow(dead_code)]
    registry_client: Option<SchemaRegistryClient>,
}

/// Schema registry client for external schema management
pub struct SchemaRegistryClient {
    #[allow(dead_code)]
    base_url: String,
    #[allow(dead_code)]
    auth: Option<(String, String)>,
}

impl SchemaManager {
    /// Create a new schema manager
    pub fn new() -> Self {
        Self {
            schema_cache: HashMap::new(),
            registry_client: None,
        }
    }

    /// Create a schema manager with registry integration
    pub fn with_registry(registry_url: String, auth: Option<(String, String)>) -> Self {
        let registry_client = Some(SchemaRegistryClient {
            base_url: registry_url,
            auth,
        });

        Self {
            schema_cache: HashMap::new(),
            registry_client,
        }
    }

    /// Generate a basic Avro schema for a table
    pub fn generate_avro_schema(&mut self, table_name: &str, fields: &[SchemaField]) -> Result<serde_json::Value> {
        // Check cache first
        if let Some(cached_schema) = self.schema_cache.get(table_name) {
            return Ok(cached_schema.clone());
        }

        // Generate new schema
        let mut avro_fields = Vec::new();
        
        for field in fields {
            avro_fields.push(serde_json::json!({
                "name": field.name,
                "type": self.sql_to_avro_type(&field.sql_type),
                "optional": field.nullable,
                "doc": field.description.as_deref().unwrap_or("")
            }));
        }

        let schema = serde_json::json!({
            "type": "record",
            "name": format!("{}Value", table_name),
            "namespace": "debshrew.cdc",
            "fields": avro_fields,
            "doc": format!("Schema for table {}", table_name)
        });

        // Cache the schema
        self.schema_cache.insert(table_name.to_string(), schema.clone());

        Ok(schema)
    }

    /// Convert SQL types to Avro types
    fn sql_to_avro_type(&self, sql_type: &str) -> serde_json::Value {
        match sql_type.to_lowercase().as_str() {
            "bigint" | "int8" => serde_json::json!("long"),
            "integer" | "int4" | "int" => serde_json::json!("int"),
            "smallint" | "int2" => serde_json::json!("int"),
            "boolean" | "bool" => serde_json::json!("boolean"),
            "real" | "float4" => serde_json::json!("float"),
            "double" | "float8" => serde_json::json!("double"),
            "text" | "varchar" | "char" => serde_json::json!("string"),
            "bytea" | "blob" => serde_json::json!("bytes"),
            "timestamp" | "timestamptz" => serde_json::json!({
                "type": "long",
                "logicalType": "timestamp-millis"
            }),
            "date" => serde_json::json!({
                "type": "int",
                "logicalType": "date"
            }),
            "decimal" | "numeric" => serde_json::json!({
                "type": "bytes",
                "logicalType": "decimal",
                "precision": 10,
                "scale": 2
            }),
            _ => serde_json::json!("string"), // Default to string for unknown types
        }
    }

    /// Validate schema compatibility
    pub fn validate_compatibility(&self, old_schema: &serde_json::Value, new_schema: &serde_json::Value) -> Result<bool> {
        // Basic compatibility check - in production you'd want more sophisticated validation
        let old_fields = self.extract_fields(old_schema)?;
        let new_fields = self.extract_fields(new_schema)?;

        // Check that all old fields are still present
        for old_field in &old_fields {
            if !new_fields.iter().any(|f| f.name == old_field.name) {
                return Ok(false); // Field was removed - not compatible
            }
        }

        // Check that field types haven't changed incompatibly
        for old_field in &old_fields {
            if let Some(new_field) = new_fields.iter().find(|f| f.name == old_field.name) {
                if !self.are_types_compatible(&old_field.field_type, &new_field.field_type) {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Extract fields from a schema
    fn extract_fields(&self, schema: &serde_json::Value) -> Result<Vec<SchemaFieldInfo>> {
        let fields_array = schema["fields"].as_array()
            .ok_or_else(|| anyhow!("Schema missing fields array"))?;

        let mut fields = Vec::new();
        for field in fields_array {
            let name = field["name"].as_str()
                .ok_or_else(|| anyhow!("Field missing name"))?
                .to_string();
            
            let field_type = field["type"].clone();
            
            fields.push(SchemaFieldInfo {
                name,
                field_type,
            });
        }

        Ok(fields)
    }

    /// Check if two types are compatible
    fn are_types_compatible(&self, old_type: &serde_json::Value, new_type: &serde_json::Value) -> bool {
        // Simplified compatibility check
        old_type == new_type
    }

    /// Clear the schema cache
    pub fn clear_cache(&mut self) {
        self.schema_cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.schema_cache.len(), self.schema_cache.capacity())
    }
}

/// Schema field definition
#[derive(Debug, Clone)]
pub struct SchemaField {
    pub name: String,
    pub sql_type: String,
    pub nullable: bool,
    pub description: Option<String>,
}

/// Internal schema field information
#[derive(Debug, Clone)]
struct SchemaFieldInfo {
    name: String,
    field_type: serde_json::Value,
}

impl Default for SchemaManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avro_schema_generation() {
        let mut manager = SchemaManager::new();
        
        let fields = vec![
            SchemaField {
                name: "id".to_string(),
                sql_type: "bigint".to_string(),
                nullable: false,
                description: Some("Primary key".to_string()),
            },
            SchemaField {
                name: "name".to_string(),
                sql_type: "varchar".to_string(),
                nullable: true,
                description: None,
            },
            SchemaField {
                name: "balance".to_string(),
                sql_type: "decimal".to_string(),
                nullable: false,
                description: Some("Account balance".to_string()),
            },
        ];

        let schema = manager.generate_avro_schema("accounts", &fields).unwrap();
        
        assert_eq!(schema["type"], "record");
        assert_eq!(schema["name"], "accountsValue");
        assert_eq!(schema["namespace"], "debshrew.cdc");
        
        let schema_fields = schema["fields"].as_array().unwrap();
        assert_eq!(schema_fields.len(), 3);
        
        // Check first field
        assert_eq!(schema_fields[0]["name"], "id");
        assert_eq!(schema_fields[0]["type"], "long");
        assert_eq!(schema_fields[0]["optional"], false);
    }

    #[test]
    fn test_sql_to_avro_type_conversion() {
        let manager = SchemaManager::new();
        
        assert_eq!(manager.sql_to_avro_type("bigint"), serde_json::json!("long"));
        assert_eq!(manager.sql_to_avro_type("integer"), serde_json::json!("int"));
        assert_eq!(manager.sql_to_avro_type("boolean"), serde_json::json!("boolean"));
        assert_eq!(manager.sql_to_avro_type("varchar"), serde_json::json!("string"));
        assert_eq!(manager.sql_to_avro_type("unknown"), serde_json::json!("string"));
    }

    #[test]
    fn test_schema_caching() {
        let mut manager = SchemaManager::new();
        
        let fields = vec![
            SchemaField {
                name: "id".to_string(),
                sql_type: "bigint".to_string(),
                nullable: false,
                description: None,
            },
        ];

        // Generate schema twice
        let schema1 = manager.generate_avro_schema("test_table", &fields).unwrap();
        let schema2 = manager.generate_avro_schema("test_table", &fields).unwrap();
        
        // Should be the same (cached)
        assert_eq!(schema1, schema2);
        
        let (cache_size, _) = manager.cache_stats();
        assert_eq!(cache_size, 1);
    }

    #[test]
    fn test_compatibility_validation() {
        let manager = SchemaManager::new();
        
        let old_schema = serde_json::json!({
            "type": "record",
            "fields": [
                {"name": "id", "type": "long"},
                {"name": "name", "type": "string"}
            ]
        });
        
        let compatible_schema = serde_json::json!({
            "type": "record",
            "fields": [
                {"name": "id", "type": "long"},
                {"name": "name", "type": "string"},
                {"name": "email", "type": "string"}  // Added field - compatible
            ]
        });
        
        let incompatible_schema = serde_json::json!({
            "type": "record",
            "fields": [
                {"name": "id", "type": "long"}
                // Removed "name" field - incompatible
            ]
        });
        
        assert!(manager.validate_compatibility(&old_schema, &compatible_schema).unwrap());
        assert!(!manager.validate_compatibility(&old_schema, &incompatible_schema).unwrap());
    }
}