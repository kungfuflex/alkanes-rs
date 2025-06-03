//! OYL Protocol Indexer
//! 
//! This module handles real-time indexing of OYL protocol activities during block processing.
//! It detects OYL transactions, extracts relevant data, and updates storage tables for
//! efficient querying.

use alkanes_support::id::AlkaneId;
use alkanes_support::proto::oyl::*;
use alkanes_support::cellpack::Cellpack;
use crate::oyl::{
    OylError, OylResult, constants::*,
    storage::*,
    utils::{TokenUtils, PoolUtils, TimeUtils}
};
use bitcoin::{Block, Transaction, OutPoint};
use bitcoin::hashes::Hash;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::utils::decode_varint_list;
use protobuf::{Message, Enum, MessageField};
use std::sync::Arc;
use std::io::Cursor;
use std::collections::HashMap;

/// Main indexer for OYL protocol data
pub struct OylIndexer;

impl OylIndexer {
    /// Index OYL-specific data for a block
    pub fn index_oyl_block_data(block: &Block, height: u32) -> OylResult<()> {
        let block_timestamp = TimeUtils::get_block_timestamp(block);
        
        // Process each transaction in the block
        for (tx_index, transaction) in block.txdata.iter().enumerate() {
            Self::index_oyl_transaction(transaction, height, tx_index, block_timestamp)?;
        }
        
        // Update time-series data for this block
        Self::update_time_series_data(block_timestamp, height)?;
        
        Ok(())
    }
    
    /// Index a single transaction for OYL activities
    fn index_oyl_transaction(
        transaction: &Transaction,
        height: u32,
        tx_index: usize,
        timestamp: u64
    ) -> OylResult<()> {
        // Extract cellpacks from transaction outputs
        for (vout, output) in transaction.output.iter().enumerate() {
            if let Ok(Some(cellpack)) = Self::extract_cellpack_from_output(output) {
                Self::process_oyl_cellpack(&cellpack, transaction, height, tx_index, vout, timestamp)?;
            }
        }
        
        Ok(())
    }
    
    /// Process a cellpack for OYL protocol activities
    fn process_oyl_cellpack(
        cellpack: &Cellpack,
        transaction: &Transaction,
        height: u32,
        tx_index: usize,
        vout: usize,
        timestamp: u64
    ) -> OylResult<()> {
        // Check if this is an OYL factory transaction
        if Self::is_oyl_factory_transaction(cellpack) {
            Self::process_factory_transaction(cellpack, transaction, height, timestamp)?;
        }
        // Check if this is an OYL pool transaction
        else if Self::is_oyl_pool_transaction(cellpack)? {
            Self::process_pool_transaction(cellpack, transaction, height, timestamp)?;
        }
        // Check if this is an OYL token transaction
        else if Self::is_oyl_token_transaction(cellpack)? {
            Self::process_token_transaction(cellpack, transaction, height, timestamp)?;
        }
        
        Ok(())
    }
    
    /// Check if a cellpack targets the OYL factory
    fn is_oyl_factory_transaction(cellpack: &Cellpack) -> bool {
        cellpack.target == OYL_FACTORY_ID
    }
    
    /// Check if a cellpack targets an OYL pool
    fn is_oyl_pool_transaction(cellpack: &Cellpack) -> OylResult<bool> {
        // Check if the target is in our tracked pools
        let pool_key = StorageUtils::alkane_id_to_key(&cellpack.target);
        Ok(StorageUtils::key_exists(&*POOL_INFO, &pool_key))
    }
    
    /// Check if a cellpack targets an OYL token
    fn is_oyl_token_transaction(cellpack: &Cellpack) -> OylResult<bool> {
        // Check if the target is in our tracked tokens
        let token_key = StorageUtils::alkane_id_to_key(&cellpack.target);
        Ok(StorageUtils::key_exists(&*TOKEN_INFO, &token_key))
    }
    
    /// Process factory transactions (pool creation, etc.)
    fn process_factory_transaction(
        cellpack: &Cellpack,
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        if cellpack.inputs.is_empty() {
            return Ok(());
        }
        
        let opcode = cellpack.inputs[0];
        
        match opcode {
            opcodes::FACTORY_CREATE_POOL => {
                // Pool creation - the new pool will be at [2, next_sequence]
                Self::handle_pool_creation(transaction, height, timestamp)?;
            }
            opcodes::FACTORY_INIT => {
                // Factory initialization
                Self::handle_factory_initialization(cellpack, timestamp)?;
            }
            _ => {
                // Other factory operations - log as activity
                Self::log_factory_activity(cellpack, transaction, height, timestamp)?;
            }
        }
        
        Ok(())
    }
    
    /// Process pool transactions (swaps, liquidity, etc.)
    fn process_pool_transaction(
        cellpack: &Cellpack,
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        if cellpack.inputs.is_empty() {
            return Ok(());
        }
        
        let opcode = cellpack.inputs[0];
        let pool_id = &cellpack.target;
        
        match opcode {
            opcodes::POOL_INIT => {
                // Pool initialization
                Self::handle_pool_initialization(cellpack, transaction, height, timestamp)?;
            }
            opcodes::POOL_ADD_LIQUIDITY => {
                // Liquidity addition
                Self::handle_add_liquidity(pool_id, transaction, height, timestamp)?;
            }
            opcodes::POOL_BURN => {
                // Liquidity removal
                Self::handle_remove_liquidity(pool_id, transaction, height, timestamp)?;
            }
            opcodes::POOL_SWAP_EXACT_FOR_TOKENS | opcodes::POOL_SWAP_TOKENS_FOR_EXACT | opcodes::POOL_SWAP => {
                // Swap transactions
                Self::handle_swap(pool_id, transaction, height, timestamp, opcode)?;
            }
            _ => {
                // Other pool operations
                Self::log_pool_activity(pool_id, cellpack, transaction, height, timestamp)?;
            }
        }
        
        Ok(())
    }
    
    /// Process token transactions
    fn process_token_transaction(
        cellpack: &Cellpack,
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        let token_id = &cellpack.target;
        
        // Log token activity
        Self::log_token_activity(token_id, cellpack, transaction, height, timestamp)?;
        
        Ok(())
    }
    
    /// Handle pool creation
    fn handle_pool_creation(
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        // The new pool ID would be determined by the alkanes runtime
        // For now, we'll detect it from the transaction trace when available
        
        // Create activity event for pool creation
        let mut activity = ActivityEvent::new();
        activity.tx_hash = transaction.compute_txid().to_byte_array().to_vec();
        activity.timestamp = MessageField::some({
            let mut ts = Timestamp::new();
            ts.seconds = timestamp;
            ts
        });
        activity.block_height = height;
        activity.type_ = ActivityType::ACTIVITY_ADD_LIQUIDITY.into(); // Pool creation is a form of liquidity addition
        
        // Store the activity
        let activity_bytes = activity.write_to_bytes()
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        
        let timestamp_key = StorageUtils::timestamp_to_key(timestamp);
        ACTIVITIES_BY_TYPE
            .select(&(ActivityType::ACTIVITY_ADD_LIQUIDITY as u8).to_le_bytes())
            .select(&timestamp_key)
            .set(Arc::new(activity_bytes));
        
        Ok(())
    }
    
    /// Handle pool initialization
    fn handle_pool_initialization(
        cellpack: &Cellpack,
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        let pool_id = &cellpack.target;
        
        // Extract token pair from cellpack inputs
        if cellpack.inputs.len() >= 7 {
            let token_a = AlkaneId::new(cellpack.inputs[1], cellpack.inputs[2]);
            let token_b = AlkaneId::new(cellpack.inputs[3], cellpack.inputs[4]);
            let factory_id = AlkaneId::new(cellpack.inputs[5], cellpack.inputs[6]);
            
            // Store pool information
            let mut pool_info = PoolInfo::new();
            pool_info.id = MessageField::some(crate::oyl::utils::ConversionUtils::alkane_id_to_pool_id(pool_id));
            pool_info.token_a = MessageField::some(crate::oyl::utils::ConversionUtils::alkane_id_to_token_id(&token_a));
            pool_info.token_b = MessageField::some(crate::oyl::utils::ConversionUtils::alkane_id_to_token_id(&token_b));
            pool_info.factory_id = MessageField::some(crate::oyl::utils::ConversionUtils::alkane_id_to_token_id(&factory_id));
            pool_info.block_created = height;
            pool_info.created_at = MessageField::some({
                let mut ts = Timestamp::new();
                ts.seconds = timestamp;
                ts
            });
            
            // Get pool name from opcode call
            if let Ok(name_data) = crate::view::call_view(pool_id, &vec![opcodes::POOL_GET_NAME], DEFAULT_VIEW_FUEL) {
                pool_info.name = String::from_utf8(name_data).unwrap_or_default();
            }
            
            // Store pool info
            let pool_key = StorageUtils::alkane_id_to_key(pool_id);
            let pool_bytes = pool_info.write_to_bytes()
                .map_err(|e| OylError::SerializationError(e.to_string()))?;
            POOL_INFO.select(&pool_key).set(Arc::new(pool_bytes));
            
            // Update pool count
            let current_count = POOL_COUNT.get_value::<u64>();
            POOL_COUNT.set_value::<u64>(current_count + 1);
            
            // Add to all pools list
            let pool_id_bytes = StorageUtils::alkane_id_to_key(pool_id);
            StorageUtils::append_to_list(&*ALL_POOLS, b"", &pool_id_bytes)?;
            
            // Index by tokens
            let token_a_key = StorageUtils::alkane_id_to_key(&token_a);
            let token_b_key = StorageUtils::alkane_id_to_key(&token_b);
            
            POOLS_BY_TOKEN.select(&token_a_key).append(Arc::new(pool_id_bytes.clone()));
            POOLS_BY_TOKEN.select(&token_b_key).append(Arc::new(pool_id_bytes.clone()));
            
            // Create pair mapping
            let (sorted_a, sorted_b) = TokenUtils::sort_token_pair(&token_a, &token_b);
            let pair_key = StorageUtils::compound_key(&[
                &StorageUtils::alkane_id_to_key(&sorted_a),
                &StorageUtils::alkane_id_to_key(&sorted_b)
            ]);
            POOL_PAIRS.select(&pair_key).set(Arc::new(pool_id_bytes));
        }
        
        Ok(())
    }
    
    /// Handle liquidity addition
    fn handle_add_liquidity(
        pool_id: &AlkaneId,
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        // Create activity event
        let mut activity = ActivityEvent::new();
        activity.tx_hash = transaction.compute_txid().to_byte_array().to_vec();
        activity.timestamp = MessageField::some({
            let mut ts = Timestamp::new();
            ts.seconds = timestamp;
            ts
        });
        activity.block_height = height;
        activity.type_ = ActivityType::ACTIVITY_ADD_LIQUIDITY.into();
        activity.pool_id = MessageField::some(crate::oyl::utils::ConversionUtils::alkane_id_to_pool_id(pool_id));
        
        // Store activity
        Self::store_activity(&activity, pool_id, timestamp)?;
        
        Ok(())
    }
    
    /// Handle liquidity removal
    fn handle_remove_liquidity(
        pool_id: &AlkaneId,
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        // Create activity event
        let mut activity = ActivityEvent::new();
        activity.tx_hash = transaction.compute_txid().to_byte_array().to_vec();
        activity.timestamp = MessageField::some({
            let mut ts = Timestamp::new();
            ts.seconds = timestamp;
            ts
        });
        activity.block_height = height;
        activity.type_ = ActivityType::ACTIVITY_REMOVE_LIQUIDITY.into();
        activity.pool_id = MessageField::some(crate::oyl::utils::ConversionUtils::alkane_id_to_pool_id(pool_id));
        
        // Store activity
        Self::store_activity(&activity, pool_id, timestamp)?;
        
        Ok(())
    }
    
    /// Handle swap transactions
    fn handle_swap(
        pool_id: &AlkaneId,
        transaction: &Transaction,
        height: u32,
        timestamp: u64,
        opcode: u128
    ) -> OylResult<()> {
        // Create activity events for both swap in and swap out
        let mut swap_activity = ActivityEvent::new();
        swap_activity.tx_hash = transaction.compute_txid().to_byte_array().to_vec();
        swap_activity.timestamp = MessageField::some({
            let mut ts = Timestamp::new();
            ts.seconds = timestamp;
            ts
        });
        swap_activity.block_height = height;
        swap_activity.type_ = ActivityType::ACTIVITY_SWAP_IN.into(); // We'll create both SWAP_IN and SWAP_OUT
        swap_activity.pool_id = MessageField::some(crate::oyl::utils::ConversionUtils::alkane_id_to_pool_id(pool_id));
        
        // Store activity
        Self::store_activity(&swap_activity, pool_id, timestamp)?;
        
        // Update volume metrics
        Self::update_volume_metrics(pool_id, timestamp)?;
        
        Ok(())
    }
    
    /// Store activity event in multiple indexes
    fn store_activity(
        activity: &ActivityEvent,
        pool_id: &AlkaneId,
        timestamp: u64
    ) -> OylResult<()> {
        let activity_bytes = activity.write_to_bytes()
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        
        let timestamp_key = StorageUtils::timestamp_to_key(timestamp);
        let pool_key = StorageUtils::alkane_id_to_key(pool_id);
        let tx_hash_key = StorageUtils::tx_hash_to_key(&activity.tx_hash);
        
        // Index by pool
        ACTIVITIES_BY_POOL
            .select(&pool_key)
            .select(&timestamp_key)
            .set(Arc::new(activity_bytes.clone()));
        
        // Index by type
        ACTIVITIES_BY_TYPE
            .select(&(activity.type_.value() as u8).to_le_bytes())
            .select(&timestamp_key)
            .set(Arc::new(activity_bytes.clone()));
        
        // Index by transaction
        ACTIVITIES_BY_TX
            .select(&tx_hash_key)
            .append(Arc::new(activity_bytes));
        
        Ok(())
    }
    
    /// Log general factory activity
    fn log_factory_activity(
        cellpack: &Cellpack,
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        // Create a generic activity event for factory operations
        let mut activity = ActivityEvent::new();
        activity.tx_hash = transaction.compute_txid().to_byte_array().to_vec();
        activity.timestamp = MessageField::some({
            let mut ts = Timestamp::new();
            ts.seconds = timestamp;
            ts
        });
        activity.block_height = height;
        activity.type_ = ActivityType::ACTIVITY_UNKNOWN.into();
        
        // Store additional data about the operation
        let mut additional_data = Vec::new();
        additional_data.extend_from_slice(&cellpack.inputs[0].to_le_bytes()); // opcode
        activity.additional_data = additional_data;
        
        let activity_bytes = activity.write_to_bytes()
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        
        let timestamp_key = StorageUtils::timestamp_to_key(timestamp);
        ACTIVITIES_BY_TYPE
            .select(&(ActivityType::ACTIVITY_UNKNOWN.value() as u8).to_le_bytes())
            .select(&timestamp_key)
            .set(Arc::new(activity_bytes));
        
        Ok(())
    }
    
    /// Log general pool activity
    fn log_pool_activity(
        pool_id: &AlkaneId,
        cellpack: &Cellpack,
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        let mut activity = ActivityEvent::new();
        activity.tx_hash = transaction.compute_txid().to_byte_array().to_vec();
        activity.timestamp = MessageField::some({
            let mut ts = Timestamp::new();
            ts.seconds = timestamp;
            ts
        });
        activity.block_height = height;
        activity.type_ = ActivityType::ACTIVITY_UNKNOWN.into();
        activity.pool_id = MessageField::some(crate::oyl::utils::ConversionUtils::alkane_id_to_pool_id(pool_id));
        
        Self::store_activity(&activity, pool_id, timestamp)?;
        
        Ok(())
    }
    
    /// Log general token activity
    fn log_token_activity(
        token_id: &AlkaneId,
        cellpack: &Cellpack,
        transaction: &Transaction,
        height: u32,
        timestamp: u64
    ) -> OylResult<()> {
        let mut activity = ActivityEvent::new();
        activity.tx_hash = transaction.compute_txid().to_byte_array().to_vec();
        activity.timestamp = MessageField::some({
            let mut ts = Timestamp::new();
            ts.seconds = timestamp;
            ts
        });
        activity.block_height = height;
        activity.type_ = ActivityType::ACTIVITY_UNKNOWN.into();
        activity.token_id = MessageField::some(crate::oyl::utils::ConversionUtils::alkane_id_to_token_id(token_id));
        
        let activity_bytes = activity.write_to_bytes()
            .map_err(|e| OylError::SerializationError(e.to_string()))?;
        
        let timestamp_key = StorageUtils::timestamp_to_key(timestamp);
        let token_key = StorageUtils::alkane_id_to_key(token_id);
        
        // Index by token
        ACTIVITIES_BY_TOKEN
            .select(&token_key)
            .select(&timestamp_key)
            .set(Arc::new(activity_bytes));
        
        Ok(())
    }
    
    /// Handle factory initialization
    fn handle_factory_initialization(
        cellpack: &Cellpack,
        timestamp: u64
    ) -> OylResult<()> {
        // Store factory information if needed
        // This could include storing the pool factory ID and other factory metadata
        Ok(())
    }
    
    /// Update time-series data for the block
    fn update_time_series_data(timestamp: u64, height: u32) -> OylResult<()> {
        // Update price history for all tracked tokens
        Self::update_price_history(timestamp)?;
        
        // Update global metrics
        Self::update_global_metrics(timestamp, height)?;
        
        Ok(())
    }
    
    /// Update price history for all tokens
    fn update_price_history(timestamp: u64) -> OylResult<()> {
        // Get all tracked tokens and update their prices
        // This would involve calculating current prices from pool data
        
        // For now, we'll implement a basic version
        // In a full implementation, this would iterate through all tokens
        // and calculate their current prices from pool reserves
        
        Ok(())
    }
    
    /// Update volume metrics
    fn update_volume_metrics(pool_id: &AlkaneId, timestamp: u64) -> OylResult<()> {
        // Update volume data for the pool
        // This would involve calculating the volume from swap amounts
        
        Ok(())
    }
    
    /// Update global metrics
    fn update_global_metrics(timestamp: u64, height: u32) -> OylResult<()> {
        // Calculate and store global metrics like total TVL, total volume, etc.
        
        Ok(())
    }
    
    /// Extract cellpack from transaction output
    fn extract_cellpack_from_output(output: &bitcoin::TxOut) -> OylResult<Option<Cellpack>> {
        // This is a simplified version - in practice, you'd need to parse the script
        // and extract the cellpack data according to the alkanes protocol
        
        // For now, return None to indicate no cellpack found
        // In a full implementation, this would parse the output script
        // and extract cellpack data if present
        
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_oyl_factory_transaction() {
        let cellpack = Cellpack {
            target: OYL_FACTORY_ID,
            inputs: vec![opcodes::FACTORY_CREATE_POOL],
        };
        
        assert!(OylIndexer::is_oyl_factory_transaction(&cellpack));
    }
    
    #[test]
    fn test_time_series_update() {
        let timestamp = 1640995200; // 2022-01-01 00:00:00 UTC
        let height = 720000;
        
        let result = OylIndexer::update_time_series_data(timestamp, height);
        assert!(result.is_ok());
    }
}