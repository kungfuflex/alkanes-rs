//! Pool details types and parsing for AMM pools

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use std::vec::Vec;
#[cfg(target_arch = "wasm32")]
use alloc::vec::Vec;

/// Pool information combining ID and optional details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolInfo {
    pub pool_id_block: u64,
    pub pool_id_tx: u64,
    pub details: Option<PoolDetails>,
}

/// Pool details information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolDetails {
    pub token_a_block: u64,
    pub token_a_tx: u64,
    pub token_b_block: u64,
    pub token_b_tx: u64,
    pub reserve_a: u128,
    pub reserve_b: u128,
    pub total_supply: u128,
    pub pool_name: String,
}

impl PoolDetails {
    /// Parse PoolDetails from bytes returned by the pool_details (opcode 999) simulation
    /// 
    /// Format:
    /// - token_a: 16 bytes (block) + 16 bytes (tx)
    /// - token_b: 16 bytes (block) + 16 bytes (tx)
    /// - reserve_a: 16 bytes (u128)
    /// - reserve_b: 16 bytes (u128)
    /// - total_supply: 16 bytes (u128)
    /// - name_length: 4 bytes (u32)
    /// - name: variable length string
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        // Minimum size: 32 (token_a) + 32 (token_b) + 16 (reserve_a) + 16 (reserve_b) + 16 (total_supply) + 4 (name_length) = 116 bytes
        if bytes.len() < 116 {
            return Err(anyhow!("Invalid bytes length for PoolDetails: expected at least 116 bytes, got {}", bytes.len()));
        }

        let mut offset = 0;

        // Read token_a (32 bytes: 16 bytes block + 16 bytes tx)
        let token_a_block_u128 = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?);
        offset += 16;
        let token_a_tx_u128 = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?);
        offset += 16;

        // Read token_b (32 bytes: 16 bytes block + 16 bytes tx)
        let token_b_block_u128 = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?);
        offset += 16;
        let token_b_tx_u128 = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?);
        offset += 16;

        // Read reserve_a (16 bytes for u128)
        let reserve_a = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?);
        offset += 16;

        // Read reserve_b (16 bytes for u128)
        let reserve_b = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?);
        offset += 16;

        // Read total_supply (16 bytes for u128)
        let total_supply = u128::from_le_bytes(bytes[offset..offset + 16].try_into()?);
        offset += 16;

        // Read pool_name length (4 bytes for u32)
        let name_length = u32::from_le_bytes(bytes[offset..offset + 4].try_into()?) as usize;
        offset += 4;

        // Check if we have enough bytes for the name
        if bytes.len() < offset + name_length {
            return Err(anyhow!(
                "Invalid bytes length for pool_name: expected {} more bytes, got {}",
                name_length,
                bytes.len() - offset
            ));
        }

        // Read pool_name
        let pool_name = String::from_utf8(bytes[offset..offset + name_length].to_vec())?;

        Ok(PoolDetails {
            token_a_block: token_a_block_u128 as u64,
            token_a_tx: token_a_tx_u128 as u64,
            token_b_block: token_b_block_u128 as u64,
            token_b_tx: token_b_tx_u128 as u64,
            reserve_a,
            reserve_b,
            total_supply,
            pool_name,
        })
    }

    /// Parse PoolDetails from hex-encoded data (with or without 0x prefix)
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let hex_data = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        let bytes = hex::decode(hex_data)?;
        Self::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pool_details() {
        // Create test data
        let mut bytes = Vec::new();
        
        // token_a: block=2, tx=0
        bytes.extend_from_slice(&2u128.to_le_bytes());
        bytes.extend_from_slice(&0u128.to_le_bytes());
        
        // token_b: block=32, tx=0
        bytes.extend_from_slice(&32u128.to_le_bytes());
        bytes.extend_from_slice(&0u128.to_le_bytes());
        
        // reserves and total supply
        bytes.extend_from_slice(&1000000u128.to_le_bytes());
        bytes.extend_from_slice(&2000000u128.to_le_bytes());
        bytes.extend_from_slice(&1414213u128.to_le_bytes());
        
        // pool name
        let name = "DIESEL / frBTC LP";
        bytes.extend_from_slice(&(name.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        
        let pool_details = PoolDetails::from_bytes(&bytes).unwrap();
        
        assert_eq!(pool_details.token_a_block, 2);
        assert_eq!(pool_details.token_a_tx, 0);
        assert_eq!(pool_details.token_b_block, 32);
        assert_eq!(pool_details.token_b_tx, 0);
        assert_eq!(pool_details.reserve_a, 1000000);
        assert_eq!(pool_details.reserve_b, 2000000);
        assert_eq!(pool_details.total_supply, 1414213);
        assert_eq!(pool_details.pool_name, name);
    }
}
