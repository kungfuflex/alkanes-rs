//! Batch pools response parsing
//!
//! This module handles parsing the aggregated response from the batch get-all-pools WASM

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use std::vec::Vec;
#[cfg(target_arch = "wasm32")]
use alloc::vec::Vec;

use super::pool_details::PoolDetails;

/// Aggregated response from batch get-all-pools WASM
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BatchPoolsResponse {
    pub pool_count: usize,
    pub pools: Vec<PoolWithDetails>,
}

/// A single pool with its details
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PoolWithDetails {
    pub pool_id_block: u64,
    pub pool_id_tx: u64,
    pub details: Option<PoolDetails>,
}

impl BatchPoolsResponse {
    /// Parse the aggregated response from WASM execution
    ///
    /// Format: [pool_count(16)][pool0_data][pool1_data]...
    /// Each pool_data: [block(16)][tx(16)][detail_length(16)][details(variable)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 16 {
            return Err(anyhow!("Response too short: {} bytes", bytes.len()));
        }

        let mut offset = 0;

        // Read pool count
        let pool_count = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?) as usize;
        offset += 16;

        let mut pools = Vec::with_capacity(pool_count);

        // Parse each pool
        for i in 0..pool_count {
            if offset + 48 > bytes.len() {
                return Err(anyhow!("Incomplete pool data at index {}", i));
            }

            // Read pool ID
            let pool_block = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?) as u64;
            offset += 16;

            let pool_tx = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?) as u64;
            offset += 16;

            // Read detail length
            let detail_len = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?) as usize;
            offset += 16;

            // Parse details if present
            let details = if detail_len > 0 && offset + detail_len <= bytes.len() {
                match PoolDetails::from_bytes(&bytes[offset..offset + detail_len]) {
                    Ok(d) => Some(d),
                    Err(e) => {
                        log::warn!("Failed to parse details for pool {}:{}: {}", pool_block, pool_tx, e);
                        None
                    }
                }
            } else {
                None
            };

            offset += detail_len;

            pools.push(PoolWithDetails {
                pool_id_block: pool_block,
                pool_id_tx: pool_tx,
                details,
            });
        }

        Ok(BatchPoolsResponse {
            pool_count,
            pools,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_response() {
        // Create a response with 0 pools
        let mut data = Vec::new();
        data.extend_from_slice(&0u128.to_le_bytes()); // pool_count = 0

        let result = BatchPoolsResponse::from_bytes(&data);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.pool_count, 0);
        assert_eq!(response.pools.len(), 0);
    }

    #[test]
    fn test_parse_single_pool_no_details() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u128.to_le_bytes()); // pool_count = 1
        data.extend_from_slice(&4u128.to_le_bytes()); // pool_block = 4
        data.extend_from_slice(&65522u128.to_le_bytes()); // pool_tx = 65522
        data.extend_from_slice(&0u128.to_le_bytes()); // detail_len = 0

        let result = BatchPoolsResponse::from_bytes(&data);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.pool_count, 1);
        assert_eq!(response.pools.len(), 1);
        assert_eq!(response.pools[0].pool_id_block, 4);
        assert_eq!(response.pools[0].pool_id_tx, 65522);
        assert!(response.pools[0].details.is_none());
    }
}
