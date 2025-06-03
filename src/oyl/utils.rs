//! OYL Protocol Utility Functions
//! 
//! This module provides utility functions for data processing, calculations,
//! and common operations used throughout the OYL protocol integration.

use alkanes_support::id::AlkaneId;
use alkanes_support::proto::oyl::*;
use crate::oyl::{OylError, OylResult, constants::*};
use crate::view::{call_view, call_multiview};
use metashrew_support::index_pointer::KeyValuePointer;
use std::collections::HashMap;
use std::io::Cursor;
use bitcoin::Block;
use protobuf::Message;

/// Token utility functions
pub struct TokenUtils;

impl TokenUtils {
    /// Get token metadata by calling opcodes
    pub fn get_token_metadata(token_id: &AlkaneId) -> OylResult<(String, String, u128)> {
        // Get name (opcode 99)
        let name_data = call_view(token_id, &vec![opcodes::TOKEN_GET_NAME], DEFAULT_VIEW_FUEL)
            .map_err(|e| OylError::OpcodeCallFailed(format!("Failed to get token name: {}", e)))?;
        let name = String::from_utf8(name_data)
            .unwrap_or_else(|_| format!("{},{}", token_id.block, token_id.tx));
        
        // Get symbol (opcode 100)
        let symbol_data = call_view(token_id, &vec![opcodes::TOKEN_GET_SYMBOL], DEFAULT_VIEW_FUEL)
            .map_err(|e| OylError::OpcodeCallFailed(format!("Failed to get token symbol: {}", e)))?;
        let symbol = String::from_utf8(symbol_data)
            .unwrap_or_else(|_| format!("{},{}", token_id.block, token_id.tx));
        
        // Get total supply (opcode 101)
        let supply_data = call_view(token_id, &vec![opcodes::TOKEN_GET_TOTAL_SUPPLY], DEFAULT_VIEW_FUEL)
            .map_err(|e| OylError::OpcodeCallFailed(format!("Failed to get token supply: {}", e)))?;
        let total_supply = if supply_data.len() >= 16 {
            u128::from_le_bytes(supply_data[0..16].try_into().unwrap_or([0u8; 16]))
        } else {
            0
        };
        
        Ok((name, symbol, total_supply))
    }
    
    /// Get multiple token metadata efficiently
    pub fn get_multiple_token_metadata(token_ids: &[AlkaneId]) -> OylResult<Vec<(String, String, u128)>> {
        let mut calls = Vec::new();
        
        // Prepare batch calls for all tokens
        for token_id in token_ids {
            calls.push((*token_id, vec![opcodes::TOKEN_GET_NAME]));
            calls.push((*token_id, vec![opcodes::TOKEN_GET_SYMBOL]));
            calls.push((*token_id, vec![opcodes::TOKEN_GET_TOTAL_SUPPLY]));
        }
        
        let call_ids: Vec<AlkaneId> = calls.iter().map(|(id, _)| *id).collect();
        let call_inputs: Vec<Vec<u128>> = calls.iter().map(|(_, inputs)| inputs.clone()).collect();
        
        let results = call_multiview(&call_ids, &call_inputs, DEFAULT_VIEW_FUEL)
            .map_err(|e| OylError::OpcodeCallFailed(format!("Batch call failed: {}", e)))?;
        
        // Process results in groups of 3 (name, symbol, supply)
        let mut metadata = Vec::new();
        let mut cursor = Cursor::new(results);
        
        for _ in 0..token_ids.len() {
            // Read name
            let name_len = u32::from_le_bytes({
                let mut bytes = [0u8; 4];
                std::io::Read::read_exact(&mut cursor, &mut bytes)
                    .map_err(|e| OylError::SerializationError(e.to_string()))?;
                bytes
            }) as usize;
            let mut name_bytes = vec![0u8; name_len];
            std::io::Read::read_exact(&mut cursor, &mut name_bytes)
                .map_err(|e| OylError::SerializationError(e.to_string()))?;
            let name = String::from_utf8(name_bytes).unwrap_or_default();
            
            // Read symbol
            let symbol_len = u32::from_le_bytes({
                let mut bytes = [0u8; 4];
                std::io::Read::read_exact(&mut cursor, &mut bytes)
                    .map_err(|e| OylError::SerializationError(e.to_string()))?;
                bytes
            }) as usize;
            let mut symbol_bytes = vec![0u8; symbol_len];
            std::io::Read::read_exact(&mut cursor, &mut symbol_bytes)
                .map_err(|e| OylError::SerializationError(e.to_string()))?;
            let symbol = String::from_utf8(symbol_bytes).unwrap_or_default();
            
            // Read supply
            let supply_len = u32::from_le_bytes({
                let mut bytes = [0u8; 4];
                std::io::Read::read_exact(&mut cursor, &mut bytes)
                    .map_err(|e| OylError::SerializationError(e.to_string()))?;
                bytes
            }) as usize;
            let mut supply_bytes = vec![0u8; supply_len];
            std::io::Read::read_exact(&mut cursor, &mut supply_bytes)
                .map_err(|e| OylError::SerializationError(e.to_string()))?;
            let total_supply = if supply_bytes.len() >= 16 {
                u128::from_le_bytes(supply_bytes[0..16].try_into().unwrap_or([0u8; 16]))
            } else {
                0
            };
            
            metadata.push((name, symbol, total_supply));
        }
        
        Ok(metadata)
    }
    
    /// Calculate token holder percentage
    pub fn calculate_holder_percentage(holder_balance: u128, total_supply: u128) -> f64 {
        if total_supply == 0 {
            0.0
        } else {
            (holder_balance as f64 / total_supply as f64) * 100.0
        }
    }
    
    /// Sort token pairs for consistent storage keys
    pub fn sort_token_pair(token_a: &AlkaneId, token_b: &AlkaneId) -> (AlkaneId, AlkaneId) {
        if token_a < token_b {
            (*token_a, *token_b)
        } else {
            (*token_b, *token_a)
        }
    }
}

/// Pool utility functions
pub struct PoolUtils;

impl PoolUtils {
    /// Get pool details by calling opcode 999
    pub fn get_pool_details(pool_id: &AlkaneId) -> OylResult<PoolDetails> {
        let details_data = call_view(pool_id, &vec![opcodes::POOL_GET_DETAILS], DEFAULT_VIEW_FUEL)
            .map_err(|e| OylError::OpcodeCallFailed(format!("Failed to get pool details: {}", e)))?;
        
        // Parse the pool details from the response
        Self::parse_pool_details(&details_data)
    }
    
    /// Parse pool details from raw bytes
    fn parse_pool_details(data: &[u8]) -> OylResult<PoolDetails> {
        if data.len() < 80 { // Minimum size for pool details
            return Err(OylError::SerializationError("Pool details data too short".to_string()));
        }
        
        let mut cursor = Cursor::new(data);
        
        // Parse token_a (32 bytes)
        let mut token_a_bytes = [0u8; 32];
        std::io::Read::read_exact(&mut cursor, &mut token_a_bytes)
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        let token_a = AlkaneId {
            block: u128::from_le_bytes(token_a_bytes[0..16].try_into().unwrap()),
            tx: u128::from_le_bytes(token_a_bytes[16..32].try_into().unwrap()),
        };
        
        // Parse token_b (32 bytes)
        let mut token_b_bytes = [0u8; 32];
        std::io::Read::read_exact(&mut cursor, &mut token_b_bytes)
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        let token_b = AlkaneId {
            block: u128::from_le_bytes(token_b_bytes[0..16].try_into().unwrap()),
            tx: u128::from_le_bytes(token_b_bytes[16..32].try_into().unwrap()),
        };
        
        // Parse reserves (16 bytes each)
        let mut reserve_a_bytes = [0u8; 16];
        std::io::Read::read_exact(&mut cursor, &mut reserve_a_bytes)
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        let reserve_a = u128::from_le_bytes(reserve_a_bytes);
        
        let mut reserve_b_bytes = [0u8; 16];
        std::io::Read::read_exact(&mut cursor, &mut reserve_b_bytes)
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        let reserve_b = u128::from_le_bytes(reserve_b_bytes);
        
        // Parse total supply (16 bytes)
        let mut total_supply_bytes = [0u8; 16];
        std::io::Read::read_exact(&mut cursor, &mut total_supply_bytes)
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        let total_supply = u128::from_le_bytes(total_supply_bytes);
        
        // Parse pool name (remaining bytes)
        let mut name_bytes = Vec::new();
        std::io::Read::read_to_end(&mut cursor, &mut name_bytes)
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        let pool_name = String::from_utf8(name_bytes).unwrap_or_default();
        
        Ok(PoolDetails {
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            total_supply,
            pool_name,
        })
    }
    
    /// Get all pools from factory
    pub fn get_all_pools_from_factory(factory_id: &AlkaneId) -> OylResult<Vec<AlkaneId>> {
        let pools_data = call_view(factory_id, &vec![opcodes::FACTORY_GET_ALL_POOLS], DEFAULT_VIEW_FUEL)
            .map_err(|e| OylError::OpcodeCallFailed(format!("Failed to get all pools: {}", e)))?;
        
        Self::parse_pool_list(&pools_data)
    }
    
    /// Parse pool list from factory response
    fn parse_pool_list(data: &[u8]) -> OylResult<Vec<AlkaneId>> {
        if data.len() < 16 {
            return Ok(Vec::new());
        }
        
        let mut cursor = Cursor::new(data);
        
        // Read pool count
        let mut count_bytes = [0u8; 16];
        std::io::Read::read_exact(&mut cursor, &mut count_bytes)
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        let pool_count = u128::from_le_bytes(count_bytes) as usize;
        
        let mut pools = Vec::new();
        
        // Read each pool ID (32 bytes each)
        for _ in 0..pool_count {
            let mut pool_bytes = [0u8; 32];
            if std::io::Read::read_exact(&mut cursor, &mut pool_bytes).is_ok() {
                let pool_id = AlkaneId {
                    block: u128::from_le_bytes(pool_bytes[0..16].try_into().unwrap()),
                    tx: u128::from_le_bytes(pool_bytes[16..32].try_into().unwrap()),
                };
                pools.push(pool_id);
            }
        }
        
        Ok(pools)
    }
}

/// Price calculation utilities
pub struct PriceUtils;

impl PriceUtils {
    /// Calculate price using constant product formula
    pub fn calculate_price(reserve_a: u128, reserve_b: u128) -> OylResult<f64> {
        if reserve_a == 0 {
            return Ok(0.0);
        }
        
        Ok(reserve_b as f64 / reserve_a as f64)
    }
    
    /// Calculate weighted average price across multiple pools
    pub fn calculate_weighted_average_price(pools: &[(u128, u128, u128)]) -> OylResult<f64> {
        if pools.is_empty() {
            return Ok(0.0);
        }
        
        let mut total_liquidity = 0u128;
        let mut weighted_sum = 0.0;
        
        for &(reserve_a, reserve_b, _) in pools {
            if reserve_a > 0 {
                let liquidity = reserve_a + reserve_b;
                let price = reserve_b as f64 / reserve_a as f64;
                
                weighted_sum += price * liquidity as f64;
                total_liquidity += liquidity;
            }
        }
        
        if total_liquidity == 0 {
            Ok(0.0)
        } else {
            Ok(weighted_sum / total_liquidity as f64)
        }
    }
    
    /// Calculate price change percentage
    pub fn calculate_price_change(old_price: f64, new_price: f64) -> f64 {
        if old_price == 0.0 {
            0.0
        } else {
            ((new_price - old_price) / old_price) * 100.0
        }
    }
    
    /// Calculate market cap
    pub fn calculate_market_cap(price: f64, circulating_supply: u128) -> f64 {
        price * circulating_supply as f64
    }
    
    /// Calculate fully diluted valuation
    pub fn calculate_fdv(price: f64, max_supply: u128) -> f64 {
        price * max_supply as f64
    }
}

/// Time utilities
pub struct TimeUtils;

impl TimeUtils {
    /// Get timestamp from block
    pub fn get_block_timestamp(block: &Block) -> u64 {
        block.header.time as u64
    }
    
    /// Create time buckets for different intervals
    pub fn create_time_bucket(timestamp: u64, interval: &str) -> OylResult<u64> {
        let bucket_size = match interval {
            "1h" => HOUR_SECONDS,
            "1d" => DAY_SECONDS,
            "1w" => WEEK_SECONDS,
            "1m" => MONTH_SECONDS,
            _ => return Err(OylError::InvalidRequest(format!("Invalid interval: {}", interval))),
        };
        
        Ok(timestamp / bucket_size)
    }
    
    /// Get time range for queries
    pub fn get_time_range(start_time: u64, end_time: u64, interval: &str) -> OylResult<Vec<u64>> {
        let bucket_size = match interval {
            "1h" => HOUR_SECONDS,
            "1d" => DAY_SECONDS,
            "1w" => WEEK_SECONDS,
            "1m" => MONTH_SECONDS,
            _ => return Err(OylError::InvalidRequest(format!("Invalid interval: {}", interval))),
        };
        
        let start_bucket = start_time / bucket_size;
        let end_bucket = end_time / bucket_size;
        
        let mut buckets = Vec::new();
        for bucket in start_bucket..=end_bucket {
            buckets.push(bucket * bucket_size);
        }
        
        Ok(buckets)
    }
    
    /// Convert timestamp to ISO 8601 string
    pub fn timestamp_to_iso8601(timestamp: u64) -> String {
        // Simple conversion - in production, use a proper datetime library
        format!("{}T00:00:00Z", timestamp / DAY_SECONDS * DAY_SECONDS)
    }
}

/// Validation utilities
pub struct ValidationUtils;

impl ValidationUtils {
    /// Validate AlkaneId
    pub fn validate_alkane_id(id: &AlkaneId) -> OylResult<()> {
        if id.block == 0 && id.tx == 0 {
            return Err(OylError::InvalidRequest("Invalid AlkaneId: cannot be zero".to_string()));
        }
        Ok(())
    }
    
    /// Validate address
    pub fn validate_address(address: &[u8]) -> OylResult<()> {
        if address.is_empty() {
            return Err(OylError::InvalidRequest("Address cannot be empty".to_string()));
        }
        if address.len() > 64 {
            return Err(OylError::InvalidRequest("Address too long".to_string()));
        }
        Ok(())
    }
    
    /// Validate pagination parameters
    pub fn validate_pagination(limit: u32, cursor: &[u8]) -> OylResult<u64> {
        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(OylError::InvalidRequest(
                format!("Invalid limit: must be between 1 and {}", MAX_PAGE_SIZE)
            ));
        }
        
        let start_index = if cursor.is_empty() {
            0
        } else {
            crate::oyl::storage::PaginationUtils::cursor_to_index(cursor)?
        };
        
        Ok(start_index)
    }
    
    /// Validate time range
    pub fn validate_time_range(start_time: u64, end_time: u64) -> OylResult<()> {
        if start_time >= end_time {
            return Err(OylError::InvalidRequest("Start time must be before end time".to_string()));
        }
        
        let max_range = 365 * DAY_SECONDS; // 1 year
        if end_time - start_time > max_range {
            return Err(OylError::InvalidRequest("Time range too large (max 1 year)".to_string()));
        }
        
        Ok(())
    }
}

/// Conversion utilities
pub struct ConversionUtils;

impl ConversionUtils {
    /// Convert protobuf TokenId to AlkaneId
    pub fn token_id_to_alkane_id(token_id: &TokenId) -> OylResult<AlkaneId> {
        Ok(AlkaneId {
            block: token_id.block.as_ref()
                .ok_or_else(|| OylError::InvalidRequest("Missing block in TokenId".to_string()))?
                .clone().into(),
            tx: token_id.tx.as_ref()
                .ok_or_else(|| OylError::InvalidRequest("Missing tx in TokenId".to_string()))?
                .clone().into(),
        })
    }
    
    /// Convert AlkaneId to protobuf TokenId
    pub fn alkane_id_to_token_id(alkane_id: &AlkaneId) -> TokenId {
        let mut token_id = TokenId::new();
        token_id.block = protobuf::MessageField::some(alkane_id.block.into());
        token_id.tx = protobuf::MessageField::some(alkane_id.tx.into());
        token_id
    }
    
    /// Convert protobuf PoolId to AlkaneId
    pub fn pool_id_to_alkane_id(pool_id: &PoolId) -> OylResult<AlkaneId> {
        Ok(AlkaneId {
            block: pool_id.block.as_ref()
                .ok_or_else(|| OylError::InvalidRequest("Missing block in PoolId".to_string()))?
                .clone().into(),
            tx: pool_id.tx.as_ref()
                .ok_or_else(|| OylError::InvalidRequest("Missing tx in PoolId".to_string()))?
                .clone().into(),
        })
    }
    
    /// Convert AlkaneId to protobuf PoolId
    pub fn alkane_id_to_pool_id(alkane_id: &AlkaneId) -> PoolId {
        let mut pool_id = PoolId::new();
        pool_id.block = protobuf::MessageField::some(alkane_id.block.into());
        pool_id.tx = protobuf::MessageField::some(alkane_id.tx.into());
        pool_id
    }
}

/// Pool details structure for internal use
#[derive(Debug, Clone)]
pub struct PoolDetails {
    pub token_a: AlkaneId,
    pub token_b: AlkaneId,
    pub reserve_a: u128,
    pub reserve_b: u128,
    pub total_supply: u128,
    pub pool_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sort_token_pair() {
        let token_a = AlkaneId { block: 2, tx: 100 };
        let token_b = AlkaneId { block: 2, tx: 50 };
        
        let (sorted_a, sorted_b) = TokenUtils::sort_token_pair(&token_a, &token_b);
        assert!(sorted_a.tx < sorted_b.tx);
    }
    
    #[test]
    fn test_calculate_price() {
        let price = PriceUtils::calculate_price(1000, 2000).unwrap();
        assert_eq!(price, 2.0);
    }
    
    #[test]
    fn test_calculate_price_change() {
        let change = PriceUtils::calculate_price_change(100.0, 110.0);
        assert_eq!(change, 10.0);
    }
    
    #[test]
    fn test_time_bucket() {
        let timestamp = 3661; // 1 hour and 1 second
        let bucket = TimeUtils::create_time_bucket(timestamp, "1h").unwrap();
        assert_eq!(bucket, 1); // Should be in the second hour bucket
    }
    
    #[test]
    fn test_validate_pagination() {
        let result = ValidationUtils::validate_pagination(50, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        
        let invalid_result = ValidationUtils::validate_pagination(0, &[]);
        assert!(invalid_result.is_err());
    }
}