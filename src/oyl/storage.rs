//! OYL Protocol Storage Layer
//! 
//! This module defines storage tables and access patterns for OYL protocol data.
//! It provides efficient indexing and lookup capabilities for tokens, pools, positions,
//! activities, and time-series data.

use alkanes_support::id::AlkaneId;
use metashrew_support::index_pointer::{KeyValuePointer, IndexPointer};
use std::sync::LazyLock;
use crate::oyl::{OylError, OylResult};
use std::sync::Arc;

// ============================================================================
// TOKEN STORAGE TABLES
// ============================================================================

/// Token metadata storage: /oyl/tokens/{token_id} -> TokenInfo
pub static TOKEN_INFO: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/tokens/"));

/// Token holders by token: /oyl/token_holders/{token_id}/{holder_id} -> balance
pub static TOKEN_HOLDERS: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/token_holders/"));

/// Holders by token (list): /oyl/holders_list/{token_id} -> list of holder_ids
pub static HOLDERS_LIST: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/holders_list/"));

/// Holder count by token: /oyl/holder_count/{token_id} -> count
pub static HOLDER_COUNT: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/holder_count/"));

/// Tokens held by address: /oyl/tokens_by_holder/{holder_id} -> list of token_ids
pub static TOKENS_BY_HOLDER: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/tokens_by_holder/"));

// ============================================================================
// POOL STORAGE TABLES
// ============================================================================

/// Pool information: /oyl/pools/{pool_id} -> PoolInfo
pub static POOL_INFO: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/pools/"));

/// All pools list: /oyl/all_pools/{index} -> pool_id
pub static ALL_POOLS: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/all_pools/"));

/// Pool count: /oyl/pool_count -> count
pub static POOL_COUNT: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/pool_count"));

/// Pools by token: /oyl/pools_by_token/{token_id} -> list of pool_ids
pub static POOLS_BY_TOKEN: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/pools_by_token/"));

/// Pool pair mapping: /oyl/pool_pairs/{token_a_id}/{token_b_id} -> pool_id
pub static POOL_PAIRS: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/pool_pairs/"));

// ============================================================================
// POSITION STORAGE TABLES
// ============================================================================

/// Positions by address: /oyl/positions/{address} -> list of positions
pub static POSITIONS_BY_ADDRESS: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/positions/"));

/// Positions by pool: /oyl/pool_positions/{pool_id} -> list of positions
pub static POSITIONS_BY_POOL: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/pool_positions/"));

/// Position details: /oyl/position_details/{address}/{pool_id} -> Position
pub static POSITION_DETAILS: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/position_details/"));

// ============================================================================
// ACTIVITY STORAGE TABLES
// ============================================================================

/// Activities by token: /oyl/activities_by_token/{token_id}/{timestamp} -> ActivityEvent
pub static ACTIVITIES_BY_TOKEN: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/activities_by_token/"));

/// Activities by address: /oyl/activities_by_address/{address}/{timestamp} -> ActivityEvent
pub static ACTIVITIES_BY_ADDRESS: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/activities_by_address/"));

/// Activities by pool: /oyl/activities_by_pool/{pool_id}/{timestamp} -> ActivityEvent
pub static ACTIVITIES_BY_POOL: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/activities_by_pool/"));

/// Activities by type: /oyl/activities_by_type/{activity_type}/{timestamp} -> ActivityEvent
pub static ACTIVITIES_BY_TYPE: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/activities_by_type/"));

/// Activity index by transaction: /oyl/activities_by_tx/{tx_hash} -> list of activities
pub static ACTIVITIES_BY_TX: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/activities_by_tx/"));

// ============================================================================
// TIME SERIES STORAGE TABLES
// ============================================================================

/// Price history: /oyl/price_history/{token_id}/{timestamp} -> Price
pub static PRICE_HISTORY: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/price_history/"));

/// Volume history by token: /oyl/volume_history_token/{token_id}/{timestamp} -> Volume
pub static VOLUME_HISTORY_TOKEN: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/volume_history_token/"));

/// Volume history by pool: /oyl/volume_history_pool/{pool_id}/{timestamp} -> Volume
pub static VOLUME_HISTORY_POOL: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/volume_history_pool/"));

/// TVL history by pool: /oyl/tvl_history/{pool_id}/{timestamp} -> TVL
pub static TVL_HISTORY: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/tvl_history/"));

/// Global metrics by timestamp: /oyl/global_metrics/{timestamp} -> GlobalMetrics
pub static GLOBAL_METRICS: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/global_metrics/"));

// ============================================================================
// CACHE TABLES
// ============================================================================

/// Token metadata cache: /oyl/cache/token_meta/{token_id} -> cached metadata
pub static TOKEN_META_CACHE: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/cache/token_meta/"));

/// Pool details cache: /oyl/cache/pool_details/{pool_id} -> cached pool details
pub static POOL_DETAILS_CACHE: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/cache/pool_details/"));

/// Price cache: /oyl/cache/prices/{token_id} -> cached price data
pub static PRICE_CACHE: LazyLock<IndexPointer> =
    LazyLock::new(|| IndexPointer::from_keyword("/oyl/cache/prices/"));

/// Storage tables for OYL protocol data
pub struct OylTables;

impl OylTables {
    // This struct is now just a namespace for organization
    // All static tables are defined above
}

/// Storage access utilities
pub struct StorageUtils;

impl StorageUtils {
    /// Convert AlkaneId to storage key bytes
    pub fn alkane_id_to_key(id: &AlkaneId) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend_from_slice(&id.block.to_le_bytes());
        key.extend_from_slice(&id.tx.to_le_bytes());
        key
    }
    
    /// Convert address bytes to storage key
    pub fn address_to_key(address: &[u8]) -> Vec<u8> {
        address.to_vec()
    }
    
    /// Convert timestamp to storage key
    pub fn timestamp_to_key(timestamp: u64) -> Vec<u8> {
        timestamp.to_le_bytes().to_vec()
    }
    
    /// Convert transaction hash to storage key
    pub fn tx_hash_to_key(tx_hash: &[u8]) -> Vec<u8> {
        tx_hash.to_vec()
    }
    
    /// Create a compound key from multiple components
    pub fn compound_key(components: &[&[u8]]) -> Vec<u8> {
        let mut key = Vec::new();
        for (i, component) in components.iter().enumerate() {
            if i > 0 {
                key.push(0xFF); // Separator byte
            }
            key.extend_from_slice(component);
        }
        key
    }
    
    /// Get the next available index for a list
    pub fn get_next_index(pointer: &IndexPointer, prefix: &[u8]) -> u64 {
        let count_key = [prefix, b"_count"].concat();
        let current_count = pointer.select(&count_key).get_value::<u64>();
        let next_index = current_count;
        pointer.select(&count_key).set_value::<u64>(current_count + 1);
        next_index
    }
    
    /// Append item to a list with automatic indexing
    pub fn append_to_list(pointer: &IndexPointer, prefix: &[u8], item: &[u8]) -> OylResult<u64> {
        let index = Self::get_next_index(pointer, prefix);
        let item_key = [prefix, &index.to_le_bytes()].concat();
        pointer.select(&item_key).set(Arc::new(item.to_vec()));
        Ok(index)
    }
    
    /// Get items from a list with pagination
    pub fn get_list_items(
        pointer: &IndexPointer, 
        prefix: &[u8], 
        start_index: u64, 
        limit: u32
    ) -> OylResult<Vec<Vec<u8>>> {
        let mut items = Vec::new();
        let count_key = [prefix, b"_count"].concat();
        let total_count = pointer.select(&count_key).get_value::<u64>();
        
        let end_index = std::cmp::min(start_index + limit as u64, total_count);
        
        for i in start_index..end_index {
            let item_key = [prefix, &i.to_le_bytes()].concat();
            let item_data = pointer.select(&item_key).get();
            if !item_data.is_empty() {
                items.push(item_data.as_ref().clone());
            }
        }
        
        Ok(items)
    }
    
    /// Check if a key exists in storage
    pub fn key_exists(pointer: &IndexPointer, key: &[u8]) -> bool {
        !pointer.select(&key.to_vec()).get().is_empty()
    }
    
    /// Get the total count of items in a list
    pub fn get_list_count(pointer: &IndexPointer, prefix: &[u8]) -> u64 {
        let count_key = [prefix, b"_count"].concat();
        pointer.select(&count_key).get_value::<u64>()
    }
    
    /// Create a time-bucketed key for time series data
    pub fn time_bucket_key(timestamp: u64, bucket_size: u64) -> Vec<u8> {
        let bucket = timestamp / bucket_size;
        bucket.to_le_bytes().to_vec()
    }
    
    /// Get time series data within a range
    pub fn get_time_series_range(
        pointer: &IndexPointer,
        prefix: &[u8],
        start_time: u64,
        end_time: u64,
        bucket_size: u64
    ) -> OylResult<Vec<(u64, Vec<u8>)>> {
        let mut results = Vec::new();
        
        let start_bucket = start_time / bucket_size;
        let end_bucket = end_time / bucket_size;
        
        for bucket in start_bucket..=end_bucket {
            let bucket_key = [prefix, &bucket.to_le_bytes()].concat();
            let data = pointer.select(&bucket_key).get();
            if !data.is_empty() {
                results.push((bucket * bucket_size, data.as_ref().clone()));
            }
        }
        
        Ok(results)
    }
}

/// Pagination utilities
pub struct PaginationUtils;

impl PaginationUtils {
    /// Create a cursor from an index
    pub fn index_to_cursor(index: u64) -> Vec<u8> {
        index.to_le_bytes().to_vec()
    }
    
    /// Parse cursor to get index
    pub fn cursor_to_index(cursor: &[u8]) -> OylResult<u64> {
        if cursor.len() != 8 {
            return Err(OylError::PaginationError("Invalid cursor format".to_string()));
        }
        
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(cursor);
        Ok(u64::from_le_bytes(bytes))
    }
    
    /// Create pagination info
    pub fn create_pagination_info(
        current_index: u64,
        limit: u32,
        total_count: u64
    ) -> (bool, Vec<u8>) {
        let has_more = current_index + (limit as u64) < total_count;
        let next_cursor = if has_more {
            Self::index_to_cursor(current_index + (limit as u64))
        } else {
            Vec::new()
        };
        
        (has_more, next_cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_alkane_id_to_key() {
        let id = AlkaneId { block: 123, tx: 456 };
        let key = StorageUtils::alkane_id_to_key(&id);
        assert_eq!(key.len(), 32); // 16 bytes for block + 16 bytes for tx
    }
    
    #[test]
    fn test_compound_key() {
        let key = StorageUtils::compound_key(&[b"prefix", b"middle", b"suffix"]);
        assert!(key.contains(&0xFF)); // Should contain separator
    }
    
    #[test]
    fn test_time_bucket_key() {
        let timestamp = 1640995200; // 2022-01-01 00:00:00 UTC
        let bucket_size = 3600; // 1 hour
        let key = StorageUtils::time_bucket_key(timestamp, bucket_size);
        assert_eq!(key.len(), 8);
    }
    
    #[test]
    fn test_pagination_cursor() {
        let index = 12345u64;
        let cursor = PaginationUtils::index_to_cursor(index);
        let parsed_index = PaginationUtils::cursor_to_index(&cursor).unwrap();
        assert_eq!(index, parsed_index);
    }
}