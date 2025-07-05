//! GPU ABI Structures for Vulkan Compute Pipeline
//!
//! This module defines fixed-size data structures for communication between
//! CPU and GPU during parallel message processing. All structures must be
//! compatible with SPIR-V and Vulkan compute shaders.

use anyhow::Result;
use bitcoin::{Block, Transaction, Txid};
use bitcoin::hashes::Hash;
use std::collections::BTreeMap;

/// Enhanced size constraints for GPU compatibility with two-bucket system
pub const MAX_MESSAGE_SIZE: usize = 4096;
pub const MAX_CALLDATA_SIZE: usize = 1024;
pub const MAX_KV_PAIRS: usize = 1024;
pub const MAX_RETURN_DATA_SIZE: usize = 1024;
pub const MAX_SHARD_SIZE: usize = 64; // Messages per GPU shard

/// Size constraints for GPU pipeline eligibility
pub const MAX_KEY_SIZE: usize = 256;           // Keys must be <= 256 bytes
pub const MAX_SMALL_VALUE_SIZE: usize = 256;   // Small values bucket: <= 256 bytes
pub const MAX_LARGE_VALUE_SIZE: usize = 512 * 1024; // Large values bucket: <= 512KB

/// GPU memory and clustering constraints
pub const MAX_GPU_MEMORY_PER_SHARD: usize = 64 * 1024 * 1024; // 64MB per shard
pub const MIN_SHARD_SIZE_FOR_GPU: usize = 4; // Minimum messages to justify GPU

/// Fixed-size representation of a MessageContextParcel for GPU execution
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuMessageInput {
    /// Transaction ID (32 bytes)
    pub txid: [u8; 32],
    /// Transaction index in block
    pub txindex: u32,
    /// Block height
    pub height: u64,
    /// Virtual output index
    pub vout: u32,
    /// Pointer for output allocation
    pub pointer: u32,
    /// Refund pointer
    pub refund_pointer: u32,
    /// Calldata length
    pub calldata_len: u32,
    /// Calldata bytes (fixed size for GPU)
    pub calldata: [u8; MAX_CALLDATA_SIZE],
    /// Runtime balance data length
    pub runtime_balance_len: u32,
    /// Runtime balance data
    pub runtime_balance_data: [u8; 512],
    /// Input runes data length
    pub input_runes_len: u32,
    /// Input runes data
    pub input_runes_data: [u8; 512],
}

impl Default for GpuMessageInput {
    fn default() -> Self {
        Self {
            txid: [0; 32],
            txindex: 0,
            height: 0,
            vout: 0,
            pointer: 0,
            refund_pointer: 0,
            calldata_len: 0,
            calldata: [0; MAX_CALLDATA_SIZE],
            runtime_balance_len: 0,
            runtime_balance_data: [0; 512],
            input_runes_len: 0,
            input_runes_data: [0; 512],
        }
    }
}

/// Fixed-size key-value pair for GPU storage operations (small values bucket)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuKvPair {
    /// Key length (max 256 bytes)
    pub key_len: u32,
    /// Key data (fixed size)
    pub key: [u8; MAX_KEY_SIZE],
    /// Value length (max 256 bytes for small bucket)
    pub value_len: u32,
    /// Value data (small bucket - fixed size)
    pub value: [u8; MAX_SMALL_VALUE_SIZE],
    /// Operation type: 0=read, 1=write, 2=delete
    pub operation: u32,
    /// Bucket type: 0=small, 1=large (for conflict detection)
    pub bucket_type: u32,
}

impl Default for GpuKvPair {
    fn default() -> Self {
        Self {
            key_len: 0,
            key: [0; MAX_KEY_SIZE],
            value_len: 0,
            value: [0; MAX_SMALL_VALUE_SIZE],
            operation: 0,
            bucket_type: 0, // Small bucket by default
        }
    }
}

/// Large value K/V pair for 512KB values bucket
#[repr(C)]
#[derive(Debug, Clone)]
pub struct GpuLargeKvPair {
    /// Key length (max 256 bytes)
    pub key_len: u32,
    /// Key data (fixed size)
    pub key: [u8; MAX_KEY_SIZE],
    /// Value length (max 512KB for large bucket)
    pub value_len: u32,
    /// Value data (large bucket - heap allocated for efficiency)
    pub value: Vec<u8>, // Will be serialized differently for GPU transfer
    /// Operation type: 0=read, 1=write, 2=delete
    pub operation: u32,
}

impl Default for GpuLargeKvPair {
    fn default() -> Self {
        Self {
            key_len: 0,
            key: [0; MAX_KEY_SIZE],
            value_len: 0,
            value: Vec::new(),
            operation: 0,
        }
    }
}

/// Conflict detection result for shard merging
#[derive(Debug, Clone)]
pub struct ShardConflict {
    /// Shard IDs that have conflicts
    pub conflicting_shards: Vec<u32>,
    /// Conflicting keys
    pub conflicting_keys: Vec<Vec<u8>>,
    /// Whether conflict requires merging
    pub requires_merge: bool,
}

/// Shard execution statistics
#[derive(Debug, Clone)]
pub struct ShardExecutionStats {
    /// Shard ID
    pub shard_id: u32,
    /// Number of messages processed
    pub message_count: u32,
    /// Number of K/V operations
    pub kv_operation_count: u32,
    /// Execution time in microseconds
    pub execution_time_us: u64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Whether execution was successful
    pub success: bool,
    /// Error message if failed
    pub error_message: String,
}

/// GPU execution context containing K/V store slice
#[repr(C)]
#[derive(Debug, Clone)]
pub struct GpuExecutionContext {
    /// Number of K/V pairs in context
    pub kv_count: u32,
    /// K/V pairs for this shard
    pub kv_pairs: [GpuKvPair; MAX_KV_PAIRS],
    /// Shard ID for debugging
    pub shard_id: u32,
    /// Block height
    pub height: u64,
}

impl Default for GpuExecutionContext {
    fn default() -> Self {
        Self {
            kv_count: 0,
            kv_pairs: [GpuKvPair::default(); MAX_KV_PAIRS],
            shard_id: 0,
            height: 0,
        }
    }
}

/// GPU execution shard containing messages and context
#[repr(C)]
#[derive(Debug, Clone)]
pub struct GpuExecutionShard {
    /// Number of messages in this shard
    pub message_count: u32,
    /// Messages to execute
    pub messages: [GpuMessageInput; MAX_SHARD_SIZE],
    /// Execution context (K/V store slice)
    pub context: GpuExecutionContext,
}

impl Default for GpuExecutionShard {
    fn default() -> Self {
        Self {
            message_count: 0,
            messages: [GpuMessageInput::default(); MAX_SHARD_SIZE],
            context: GpuExecutionContext::default(),
        }
    }
}

/// GPU execution results from a shard
#[repr(C)]
#[derive(Debug, Clone)]
pub struct GpuExecutionResult {
    /// Number of K/V updates
    pub kv_update_count: u32,
    /// K/V updates to apply
    pub kv_updates: [GpuKvPair; MAX_KV_PAIRS],
    /// Number of return data entries
    pub return_data_count: u32,
    /// Return data for each message
    pub return_data: [GpuReturnData; MAX_SHARD_SIZE],
    /// Execution status: 0=success, 1=error
    pub status: u32,
    /// Error message length (if status != 0)
    pub error_len: u32,
    /// Error message
    pub error_message: [u8; 256],
}

impl Default for GpuExecutionResult {
    fn default() -> Self {
        Self {
            kv_update_count: 0,
            kv_updates: [GpuKvPair::default(); MAX_KV_PAIRS],
            return_data_count: 0,
            return_data: [GpuReturnData::default(); MAX_SHARD_SIZE],
            status: 0,
            error_len: 0,
            error_message: [0; 256],
        }
    }
}

/// Return data from a single message execution
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuReturnData {
    /// Message index in shard
    pub message_index: u32,
    /// Success flag: 0=failed, 1=success
    pub success: u32,
    /// Return data length
    pub data_len: u32,
    /// Return data
    pub data: [u8; MAX_RETURN_DATA_SIZE],
}

impl Default for GpuReturnData {
    fn default() -> Self {
        Self {
            message_index: 0,
            success: 0,
            data_len: 0,
            data: [0; MAX_RETURN_DATA_SIZE],
        }
    }
}

/// Conversion utilities for CPU-GPU data transfer
pub mod conversion {
    use super::*;
    use crate::message::MessageContextParcel;
    use protorune_support::rune_transfer::RuneTransfer;
    use protorune_support::balance_sheet::BalanceSheet;
    use metashrew_core::index_pointer::AtomicPointer;

    /// Convert MessageContextParcel to GPU format
    pub fn message_parcel_to_gpu(parcel: &MessageContextParcel) -> Result<GpuMessageInput> {
        let mut gpu_input = GpuMessageInput::default();
        
        // Copy transaction ID
        gpu_input.txid = parcel.transaction.compute_txid().to_byte_array();
        gpu_input.txindex = parcel.txindex;
        gpu_input.height = parcel.height;
        gpu_input.vout = parcel.vout;
        gpu_input.pointer = parcel.pointer;
        gpu_input.refund_pointer = parcel.refund_pointer;
        
        // Copy calldata (truncate if too large)
        let calldata_len = std::cmp::min(parcel.calldata.len(), MAX_CALLDATA_SIZE);
        gpu_input.calldata_len = calldata_len as u32;
        gpu_input.calldata[..calldata_len].copy_from_slice(&parcel.calldata[..calldata_len]);
        
        // Serialize runtime balances
        let runtime_balance_bytes = serialize_balance_sheet(&parcel.runtime_balances)?;
        let balance_len = std::cmp::min(runtime_balance_bytes.len(), 512);
        gpu_input.runtime_balance_len = balance_len as u32;
        gpu_input.runtime_balance_data[..balance_len].copy_from_slice(&runtime_balance_bytes[..balance_len]);
        
        // Serialize input runes
        let runes_bytes = serialize_rune_transfers(&parcel.runes)?;
        let runes_len = std::cmp::min(runes_bytes.len(), 512);
        gpu_input.input_runes_len = runes_len as u32;
        gpu_input.input_runes_data[..runes_len].copy_from_slice(&runes_bytes[..runes_len]);
        
        Ok(gpu_input)
    }

    /// Serialize balance sheet to bytes
    fn serialize_balance_sheet(sheet: &BalanceSheet<AtomicPointer>) -> Result<Vec<u8>> {
        // TODO: Implement proper serialization
        // For now, return empty bytes
        Ok(vec![])
    }

    /// Serialize rune transfers to bytes
    fn serialize_rune_transfers(transfers: &[RuneTransfer]) -> Result<Vec<u8>> {
        // TODO: Implement proper serialization
        // For now, return empty bytes
        Ok(vec![])
    }

    /// Create GPU execution context from K/V pairs
    pub fn create_gpu_context(
        kv_pairs: &BTreeMap<Vec<u8>, Vec<u8>>,
        shard_id: u32,
        height: u64,
    ) -> Result<GpuExecutionContext> {
        let mut context = GpuExecutionContext::default();
        context.shard_id = shard_id;
        context.height = height;
        
        let mut count = 0;
        for (key, value) in kv_pairs.iter() {
            if count >= MAX_KV_PAIRS {
                break;
            }
            
            let mut kv_pair = GpuKvPair::default();
            
            // Copy key (truncate if too large)
            let key_len = std::cmp::min(key.len(), 256);
            kv_pair.key_len = key_len as u32;
            kv_pair.key[..key_len].copy_from_slice(&key[..key_len]);
            
            // Copy value (truncate if too large)
            let value_len = std::cmp::min(value.len(), 1024);
            kv_pair.value_len = value_len as u32;
            kv_pair.value[..value_len].copy_from_slice(&value[..value_len]);
            
            kv_pair.operation = 0; // Read operation
            
            context.kv_pairs[count] = kv_pair;
            count += 1;
        }
        
        context.kv_count = count as u32;
        Ok(context)
    }

    /// Create execution shard from messages and context
    pub fn create_execution_shard(
        messages: &[GpuMessageInput],
        context: GpuExecutionContext,
    ) -> Result<GpuExecutionShard> {
        let mut shard = GpuExecutionShard::default();
        shard.context = context;
        
        let message_count = std::cmp::min(messages.len(), MAX_SHARD_SIZE);
        shard.message_count = message_count as u32;
        
        for (i, message) in messages.iter().take(message_count).enumerate() {
            shard.messages[i] = *message;
        }
        
        Ok(shard)
    }

    /// Extract K/V updates from GPU result
    pub fn extract_kv_updates(result: &GpuExecutionResult) -> Result<BTreeMap<Vec<u8>, Vec<u8>>> {
        let mut updates = BTreeMap::new();
        
        for i in 0..result.kv_update_count as usize {
            if i >= MAX_KV_PAIRS {
                break;
            }
            
            let kv_pair = &result.kv_updates[i];
            
            // Extract key
            let key_len = std::cmp::min(kv_pair.key_len as usize, 256);
            let key = kv_pair.key[..key_len].to_vec();
            
            // Extract value
            let value_len = std::cmp::min(kv_pair.value_len as usize, 1024);
            let value = kv_pair.value[..value_len].to_vec();
            
            // Only process write operations
            if kv_pair.operation == 1 {
                updates.insert(key, value);
            }
        }
        
        Ok(updates)
    }

    /// Extract return data from GPU result
    pub fn extract_return_data(result: &GpuExecutionResult) -> Result<Vec<Vec<u8>>> {
        let mut return_data = Vec::new();
        
        for i in 0..result.return_data_count as usize {
            if i >= MAX_SHARD_SIZE {
                break;
            }
            
            let ret_data = &result.return_data[i];
            let data_len = std::cmp::min(ret_data.data_len as usize, MAX_RETURN_DATA_SIZE);
            let data = ret_data.data[..data_len].to_vec();
            
            return_data.push(data);
        }
        
        Ok(return_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::conversion::*;

    #[test]
    fn test_gpu_structures_size() {
        // Ensure structures are reasonable size for GPU
        assert!(std::mem::size_of::<GpuMessageInput>() < 8192);
        assert!(std::mem::size_of::<GpuExecutionShard>() < 1024 * 1024); // 1MB max
        assert!(std::mem::size_of::<GpuExecutionResult>() < 1024 * 1024); // 1MB max
    }

    #[test]
    fn test_gpu_context_creation() {
        let mut kv_pairs = BTreeMap::new();
        kv_pairs.insert(b"key1".to_vec(), b"value1".to_vec());
        kv_pairs.insert(b"key2".to_vec(), b"value2".to_vec());
        
        let context = create_gpu_context(&kv_pairs, 0, 100).unwrap();
        assert_eq!(context.kv_count, 2);
        assert_eq!(context.shard_id, 0);
        assert_eq!(context.height, 100);
    }

    #[test]
    fn test_execution_shard_creation() {
        let messages = vec![GpuMessageInput::default(); 5];
        let context = GpuExecutionContext::default();
        
        let shard = create_execution_shard(&messages, context).unwrap();
        assert_eq!(shard.message_count, 5);
    }

    #[test]
    fn test_kv_updates_extraction() {
        let mut result = GpuExecutionResult::default();
        result.kv_update_count = 1;
        
        // Set up a write operation
        result.kv_updates[0].key_len = 4;
        result.kv_updates[0].key[..4].copy_from_slice(b"test");
        result.kv_updates[0].value_len = 5;
        result.kv_updates[0].value[..5].copy_from_slice(b"value");
        result.kv_updates[0].operation = 1; // Write
        
        let updates = extract_kv_updates(&result).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates.get(b"test".as_slice()), Some(&b"value".to_vec()));
    }
}

/// Utility functions for GPU pipeline validation and conflict detection
impl GpuKvPair {
    /// Check if key-value pair is eligible for GPU processing
    pub fn is_gpu_eligible(key: &[u8], value: &[u8]) -> bool {
        key.len() <= MAX_KEY_SIZE &&
        (value.len() <= MAX_SMALL_VALUE_SIZE || value.len() <= MAX_LARGE_VALUE_SIZE)
    }
    
    /// Determine bucket type for key-value pair
    pub fn determine_bucket_type(value: &[u8]) -> u32 {
        if value.len() <= MAX_SMALL_VALUE_SIZE {
            0 // Small bucket
        } else {
            1 // Large bucket
        }
    }
    
    /// Create from key-value data with validation
    pub fn from_kv_data(key: &[u8], value: &[u8], operation: u32) -> Result<Self> {
        if key.len() > MAX_KEY_SIZE {
            return Err(anyhow::anyhow!("Key size {} exceeds maximum {}", key.len(), MAX_KEY_SIZE));
        }
        
        if value.len() > MAX_SMALL_VALUE_SIZE {
            return Err(anyhow::anyhow!("Value size {} exceeds small bucket maximum {}", value.len(), MAX_SMALL_VALUE_SIZE));
        }
        
        let mut kv_pair = Self::default();
        kv_pair.key_len = key.len() as u32;
        kv_pair.key[..key.len()].copy_from_slice(key);
        kv_pair.value_len = value.len() as u32;
        kv_pair.value[..value.len()].copy_from_slice(value);
        kv_pair.operation = operation;
        kv_pair.bucket_type = Self::determine_bucket_type(value);
        
        Ok(kv_pair)
    }
    
    /// Get key as slice
    pub fn key_slice(&self) -> &[u8] {
        &self.key[..self.key_len as usize]
    }
    
    /// Get value as slice
    pub fn value_slice(&self) -> &[u8] {
        &self.value[..self.value_len as usize]
    }
    
    /// Check if this K/V pair conflicts with another (same key, both writes)
    pub fn conflicts_with(&self, other: &Self) -> bool {
        // Only writes can conflict
        if self.operation != 1 || other.operation != 1 {
            return false;
        }
        
        // Check if keys match
        self.key_slice() == other.key_slice()
    }
}

impl GpuLargeKvPair {
    /// Create from key-value data with validation
    pub fn from_kv_data(key: &[u8], value: &[u8], operation: u32) -> Result<Self> {
        if key.len() > MAX_KEY_SIZE {
            return Err(anyhow::anyhow!("Key size {} exceeds maximum {}", key.len(), MAX_KEY_SIZE));
        }
        
        if value.len() > MAX_LARGE_VALUE_SIZE {
            return Err(anyhow::anyhow!("Value size {} exceeds large bucket maximum {}", value.len(), MAX_LARGE_VALUE_SIZE));
        }
        
        let mut kv_pair = Self::default();
        kv_pair.key_len = key.len() as u32;
        kv_pair.key[..key.len()].copy_from_slice(key);
        kv_pair.value_len = value.len() as u32;
        kv_pair.value = value.to_vec();
        kv_pair.operation = operation;
        
        Ok(kv_pair)
    }
    
    /// Get key as slice
    pub fn key_slice(&self) -> &[u8] {
        &self.key[..self.key_len as usize]
    }
    
    /// Check if this K/V pair conflicts with another (same key, both writes)
    pub fn conflicts_with(&self, other: &Self) -> bool {
        // Only writes can conflict
        if self.operation != 1 || other.operation != 1 {
            return false;
        }
        
        // Check if keys match
        self.key_slice() == other.key_slice()
    }
}

/// Conflict detection utilities
impl ShardConflict {
    /// Create new conflict detection result
    pub fn new() -> Self {
        Self {
            conflicting_shards: Vec::new(),
            conflicting_keys: Vec::new(),
            requires_merge: false,
        }
    }
    
    /// Add conflicting shard
    pub fn add_conflict(&mut self, shard_id: u32, key: Vec<u8>) {
        if !self.conflicting_shards.contains(&shard_id) {
            self.conflicting_shards.push(shard_id);
        }
        if !self.conflicting_keys.contains(&key) {
            self.conflicting_keys.push(key);
        }
        self.requires_merge = true;
    }
    
    /// Check if there are any conflicts
    pub fn has_conflicts(&self) -> bool {
        self.requires_merge && !self.conflicting_shards.is_empty()
    }
}

impl Default for ShardConflict {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ShardExecutionStats {
    fn default() -> Self {
        Self {
            shard_id: 0,
            message_count: 0,
            kv_operation_count: 0,
            execution_time_us: 0,
            memory_usage_bytes: 0,
            success: false,
            error_message: String::new(),
        }
    }
}

/// GPU pipeline validation utilities
pub mod validation {
    use super::*;
    
    /// Check if a cellpack is eligible for GPU processing based on size constraints
    pub fn is_cellpack_gpu_eligible(kv_operations: &[(Vec<u8>, Vec<u8>, u32)]) -> bool {
        for (key, value, _operation) in kv_operations {
            if !GpuKvPair::is_gpu_eligible(key, value) {
                return false;
            }
        }
        true
    }
    
    /// Estimate memory usage for a shard
    pub fn estimate_shard_memory_usage(
        message_count: usize,
        kv_operation_count: usize,
        avg_value_size: usize,
    ) -> usize {
        let message_memory = message_count * std::mem::size_of::<GpuMessageInput>();
        let kv_memory = kv_operation_count * (MAX_KEY_SIZE + avg_value_size);
        let overhead = 1024 * 1024; // 1MB overhead
        
        message_memory + kv_memory + overhead
    }
    
    /// Check if shard fits within GPU memory constraints
    pub fn shard_fits_in_gpu_memory(
        message_count: usize,
        kv_operation_count: usize,
        avg_value_size: usize,
    ) -> bool {
        let estimated_memory = estimate_shard_memory_usage(message_count, kv_operation_count, avg_value_size);
        estimated_memory <= MAX_GPU_MEMORY_PER_SHARD
    }
    
    /// Detect conflicts between two shards
    pub fn detect_shard_conflicts(
        shard1_kvs: &[GpuKvPair],
        shard2_kvs: &[GpuKvPair],
    ) -> ShardConflict {
        let mut conflict = ShardConflict::new();
        
        for kv1 in shard1_kvs {
            for kv2 in shard2_kvs {
                if kv1.conflicts_with(kv2) {
                    conflict.add_conflict(0, kv1.key_slice().to_vec()); // shard IDs would be passed separately
                    break;
                }
            }
        }
        
        conflict
    }
}