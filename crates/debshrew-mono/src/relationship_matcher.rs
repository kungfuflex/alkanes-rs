//! # Relationship Matcher
//!
//! ## PURPOSE
//! Efficiently matches k/v keys against configured CDC relationships using compiled regex patterns.
//! Provides fast lookup to determine which relationships apply to a given key.

use crate::cdc_config::CDCRelationship;
use regex::Regex;
use std::collections::HashMap;

/// Efficient matcher for finding relationships that apply to k/v keys
pub struct RelationshipMatcher {
    /// Compiled regex patterns for each relationship
    patterns: Vec<(Regex, usize)>,
    /// Lookup table from index to relationship
    relationships: Vec<CDCRelationship>,
    /// Cache for recently matched keys (optional optimization)
    cache: HashMap<Vec<u8>, Vec<usize>>,
}

impl RelationshipMatcher {
    /// Create a new relationship matcher from a list of relationships
    pub fn new(relationships: &[CDCRelationship]) -> Self {
        let mut patterns = Vec::new();
        let mut compiled_relationships = Vec::new();

        for (index, relationship) in relationships.iter().enumerate() {
            if let Ok(regex) = Regex::new(&relationship.key_pattern) {
                patterns.push((regex, index));
                compiled_relationships.push(relationship.clone());
            } else {
                log::warn!("Invalid regex pattern in relationship '{}': {}", 
                          relationship.name, relationship.key_pattern);
            }
        }

        Self {
            patterns,
            relationships: compiled_relationships,
            cache: HashMap::new(),
        }
    }

    /// Find all relationships that match the given key
    pub fn find_matches(&self, key: &[u8]) -> Vec<&CDCRelationship> {
        // Convert key to string for regex matching
        let key_str = String::from_utf8_lossy(key);
        
        let mut matches = Vec::new();
        
        for (regex, index) in &self.patterns {
            if regex.is_match(&key_str) {
                if let Some(relationship) = self.relationships.get(*index) {
                    matches.push(relationship);
                }
            }
        }
        
        matches
    }

    /// Find all relationship indices that match the given key (for caching)
    pub fn find_match_indices(&self, key: &[u8]) -> Vec<usize> {
        let key_str = String::from_utf8_lossy(key);
        
        self.patterns
            .iter()
            .filter_map(|(regex, index)| {
                if regex.is_match(&key_str) {
                    Some(*index)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get relationship by index
    pub fn get_relationship(&self, index: usize) -> Option<&CDCRelationship> {
        self.relationships.get(index)
    }

    /// Get the number of configured relationships
    pub fn len(&self) -> usize {
        self.relationships.len()
    }

    /// Check if any relationships are configured
    pub fn is_empty(&self) -> bool {
        self.relationships.is_empty()
    }

    /// Clear the internal cache (useful for memory management)
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.cache.len(), self.cache.capacity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdc_config::{KeyMapping, ValueMapping, ValueFormat, OperationType};

    fn create_test_relationships() -> Vec<CDCRelationship> {
        vec![
            CDCRelationship {
                name: "addresses".to_string(),
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
            },
            CDCRelationship {
                name: "transactions".to_string(),
                key_pattern: "^tx:(.+)$".to_string(),
                table_name: "transactions".to_string(),
                key_mapping: KeyMapping {
                    primary_key_fields: vec!["txid".to_string()],
                    extraction_pattern: "^tx:(.+)$".to_string(),
                    transformations: None,
                },
                value_mapping: ValueMapping {
                    fields: vec![],
                    value_format: ValueFormat::Json,
                    defaults: None,
                },
                operation_type: OperationType::CreateOnly,
                filters: None,
                transformations: None,
            },
            CDCRelationship {
                name: "utxos".to_string(),
                key_pattern: "^utxo:(.+):(.+)$".to_string(),
                table_name: "utxos".to_string(),
                key_mapping: KeyMapping {
                    primary_key_fields: vec!["txid".to_string(), "vout".to_string()],
                    extraction_pattern: "^utxo:(.+):(.+)$".to_string(),
                    transformations: None,
                },
                value_mapping: ValueMapping {
                    fields: vec![],
                    value_format: ValueFormat::Json,
                    defaults: None,
                },
                operation_type: OperationType::CreateUpdateDelete,
                filters: None,
                transformations: None,
            },
        ]
    }

    #[test]
    fn test_address_matching() {
        let relationships = create_test_relationships();
        let matcher = RelationshipMatcher::new(&relationships);

        let key = b"address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let matches = matcher.find_matches(key);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "addresses");
        assert_eq!(matches[0].table_name, "addresses");
    }

    #[test]
    fn test_transaction_matching() {
        let relationships = create_test_relationships();
        let matcher = RelationshipMatcher::new(&relationships);

        let key = b"tx:a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d";
        let matches = matcher.find_matches(key);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "transactions");
        assert_eq!(matches[0].table_name, "transactions");
    }

    #[test]
    fn test_utxo_matching() {
        let relationships = create_test_relationships();
        let matcher = RelationshipMatcher::new(&relationships);

        let key = b"utxo:a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d:0";
        let matches = matcher.find_matches(key);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "utxos");
        assert_eq!(matches[0].table_name, "utxos");
    }

    #[test]
    fn test_no_matches() {
        let relationships = create_test_relationships();
        let matcher = RelationshipMatcher::new(&relationships);

        let key = b"unknown:some_key";
        let matches = matcher.find_matches(key);

        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_multiple_matches() {
        // Create relationships with overlapping patterns
        let mut relationships = create_test_relationships();
        relationships.push(CDCRelationship {
            name: "all_keys".to_string(),
            key_pattern: ".*".to_string(), // Matches everything
            table_name: "all_data".to_string(),
            key_mapping: KeyMapping {
                primary_key_fields: vec!["key".to_string()],
                extraction_pattern: "(.*)".to_string(),
                transformations: None,
            },
            value_mapping: ValueMapping {
                fields: vec![],
                value_format: ValueFormat::Raw,
                defaults: None,
            },
            operation_type: OperationType::CreateOnly,
            filters: None,
            transformations: None,
        });

        let matcher = RelationshipMatcher::new(&relationships);

        let key = b"address:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let matches = matcher.find_matches(key);

        // Should match both "addresses" and "all_keys"
        assert_eq!(matches.len(), 2);
        
        let names: Vec<&str> = matches.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"addresses"));
        assert!(names.contains(&"all_keys"));
    }

    #[test]
    fn test_invalid_regex() {
        let mut relationships = create_test_relationships();
        relationships.push(CDCRelationship {
            name: "invalid_regex".to_string(),
            key_pattern: "[invalid".to_string(), // Invalid regex
            table_name: "invalid".to_string(),
            key_mapping: KeyMapping {
                primary_key_fields: vec!["key".to_string()],
                extraction_pattern: "(.*)".to_string(),
                transformations: None,
            },
            value_mapping: ValueMapping {
                fields: vec![],
                value_format: ValueFormat::Raw,
                defaults: None,
            },
            operation_type: OperationType::CreateOnly,
            filters: None,
            transformations: None,
        });

        let matcher = RelationshipMatcher::new(&relationships);

        // Should only have the valid relationships
        assert_eq!(matcher.len(), 3); // Original 3, invalid one is skipped
    }

    #[test]
    fn test_empty_relationships() {
        let matcher = RelationshipMatcher::new(&[]);

        assert!(matcher.is_empty());
        assert_eq!(matcher.len(), 0);

        let key = b"any:key";
        let matches = matcher.find_matches(key);
        assert_eq!(matches.len(), 0);
    }
}