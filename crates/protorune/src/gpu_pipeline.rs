//! GPU Pipeline for Parallel Alkanes Message Processing
//!
//! This module implements the connection between dependency analysis and actual GPU execution.
//! It takes parallelizable message groups from the dependency analyzer and executes them
//! on GPU hardware using the Vulkan runtime.

use crate::gpu_tracking::{StorageTracker, DependencyStats, get_block_dependency_analysis};
use crate::message::MessageContext;
use anyhow::{anyhow, Result};
use bitcoin::{Block, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Import GPU dependencies only when GPU feature is enabled
#[cfg(feature = "gpu")]
use metashrew_runtime::vulkan_runtime::{VulkanExecutionInput, VulkanExecutionResult};

#[cfg(feature = "gpu")]
use alkanes_gpu::gpu_types::{
    GpuExecutionShard, GpuExecutionResult, GpuMessageInput, GpuKvPair, GpuExecutionContext,
    MAX_SHARD_SIZE, MAX_KV_PAIRS,
};

/// Minimum batch size to justify GPU execution overhead
const MIN_GPU_BATCH_SIZE: usize = 4;

/// Maximum batch size for GPU execution
const MAX_GPU_BATCH_SIZE: usize = 1024;

/// Maximum calldata size per message
const MAX_CALLDATA_SIZE: usize = 256;

/// Maximum storage slots per batch
const MAX_STORAGE_SLOTS: usize = 4096;

/// GPU-compatible message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GpuMessage {
    /// Transaction ID (32 bytes)
    pub txid: [u8; 32],
    /// Transaction index in block
    pub tx_index: u32,
    /// Protostone index within transaction
    pub protostone_index: u32,
    /// Message calldata (fixed size)
    pub calldata: [u8; MAX_CALLDATA_SIZE],
    /// Actual calldata length
    pub calldata_len: u32,
    /// Target alkane block
    pub target_block: u64,
    /// Target alkane transaction
    pub target_tx: u64,
    /// Opcode from cellpack
    pub opcode: u128,
    /// Expected result offset in output buffer
    pub result_offset: u32,
}

/// GPU storage slot for pre-loaded state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GpuStorageSlot {
    /// Storage key (fixed size)
    pub key: [u8; 64],
    /// Actual key length
    pub key_len: u32,
    /// Storage value (fixed size)
    pub value: [u8; 1024],
    /// Actual value length
    pub value_len: u32,
}

/// GPU batch input structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GpuMessageBatch {
    /// Array of messages to process
    pub messages: Vec<GpuMessage>,
    /// Pre-loaded storage context
    pub storage_slots: Vec<GpuStorageSlot>,
    /// Block height for context
    pub block_height: u64,
}

/// GPU batch output structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GpuBatchResult {
    /// Success flags for each message (1 = success, 0 = failure)
    pub success_flags: Vec<u32>,
    /// Storage updates from GPU execution
    pub storage_updates: Vec<GpuStorageSlot>,
    /// Gas used per message
    pub gas_used: Vec<u64>,
}

/// Try to execute parallelizable message groups on GPU
pub fn try_execute_on_gpu(
    parallel_groups: Vec<Vec<usize>>,
    trackers: &[StorageTracker],
    block: &Block,
    height: u64,
) -> Result<bool> {
    // Filter groups that are worth GPU execution
    let gpu_worthy_groups: Vec<_> = parallel_groups
        .into_iter()
        .filter(|group| group.len() >= MIN_GPU_BATCH_SIZE)
        .collect();
    
    if gpu_worthy_groups.is_empty() {
        crate::alkane_log!("No GPU-worthy parallel groups found (min size: {})", MIN_GPU_BATCH_SIZE);
        return Ok(false);
    }
    
    crate::alkane_log!(
        "Found {} GPU-worthy parallel groups with {} total messages",
        gpu_worthy_groups.len(),
        gpu_worthy_groups.iter().map(|g| g.len()).sum::<usize>()
    );
    
    // Process each group separately (could be parallelized further)
    let mut total_gpu_executed = 0;
    
    for (group_idx, group) in gpu_worthy_groups.iter().enumerate() {
        match execute_group_on_gpu(group, trackers, block, height) {
            Ok(executed) => {
                if executed {
                    total_gpu_executed += group.len();
                    crate::alkane_log!(
                        "GPU group {} executed successfully ({} messages)",
                        group_idx,
                        group.len()
                    );
                } else {
                    crate::alkane_log!(
                        "GPU group {} fell back to CPU execution",
                        group_idx
                    );
                }
            }
            Err(e) => {
                crate::alkane_log!(
                    "GPU group {} execution failed: {}, falling back to CPU",
                    group_idx,
                    e
                );
                // Continue with other groups even if one fails
            }
        }
    }
    
    Ok(total_gpu_executed > 0)
}

/// Execute a single parallel group on GPU
fn execute_group_on_gpu(
    group: &[usize],
    trackers: &[StorageTracker],
    block: &Block,
    height: u64,
) -> Result<bool> {
    // Extract messages from the group
    let gpu_batch = serialize_group_for_gpu(group, trackers, block, height)?;
    
    if gpu_batch.messages.is_empty() {
        return Ok(false);
    }
    
    // Calculate expected output size
    let output_size = calculate_output_size(&gpu_batch);
    
    // Prepare GPU execution input
    let gpu_input = VulkanExecutionInput {
        shader_name: "alkanes_batch_processor".to_string(),
        input_data: serialize_gpu_batch(&gpu_batch)?,
        output_size,
    };
    
    // Execute on GPU via Vulkan runtime
    let result_bytes = metashrew_runtime::MetashrewRuntime::<()>::execute_gpu_work(
        &serde_json::to_vec(&gpu_input)?
    )?;
    
    // Parse GPU execution result
    let gpu_result: VulkanExecutionResult = serde_json::from_slice(&result_bytes)?;
    
    if !gpu_result.success {
        return Err(anyhow!(
            "GPU execution failed: {}",
            gpu_result.error_message.unwrap_or_else(|| "Unknown error".to_string())
        ));
    }
    
    // Process GPU results
    let batch_result = deserialize_gpu_result(&gpu_result.output_data)?;
    process_gpu_results(&batch_result, &gpu_batch)?;
    
    Ok(true)
}

/// Serialize a parallel group into GPU-compatible format
fn serialize_group_for_gpu(
    group: &[usize],
    trackers: &[StorageTracker],
    block: &Block,
    height: u64,
) -> Result<GpuMessageBatch> {
    let mut messages = Vec::new();
    let mut all_storage_slots = BTreeMap::new();
    
    for &tracker_idx in group {
        if tracker_idx >= trackers.len() {
            continue;
        }
        
        let tracker = &trackers[tracker_idx];
        
        // Skip non-parallelizable trackers
        if !tracker.is_parallelizable {
            continue;
        }
        
        // Find the transaction in the block
        let transaction = block.txdata.get(tracker.tx_index)
            .ok_or_else(|| anyhow!("Transaction index {} not found in block", tracker.tx_index))?;
        
        // Create GPU message
        let gpu_message = create_gpu_message(tracker, transaction, height)?;
        messages.push(gpu_message);
        
        // Collect storage slots accessed by this tracker
        for slot_key in tracker.all_accessed_slots() {
            if slot_key.len() <= 64 {
                // TODO: Load actual storage value for this key
                // For now, create empty slot as placeholder
                let storage_slot = GpuStorageSlot {
                    key: {
                        let mut key_array = [0u8; 64];
                        key_array[..slot_key.len()].copy_from_slice(&slot_key);
                        key_array
                    },
                    key_len: slot_key.len() as u32,
                    value: [0u8; 1024], // TODO: Load actual value
                    value_len: 0,
                };
                all_storage_slots.insert(slot_key, storage_slot);
            }
        }
    }
    
    // Limit storage slots to maximum
    let storage_slots: Vec<_> = all_storage_slots
        .into_values()
        .take(MAX_STORAGE_SLOTS)
        .collect();
    
    Ok(GpuMessageBatch {
        messages,
        storage_slots,
        block_height: height,
    })
}

/// Create a GPU message from a storage tracker
fn create_gpu_message(
    tracker: &StorageTracker,
    transaction: &Transaction,
    height: u64,
) -> Result<GpuMessage> {
    // Extract cellpack information
    let (target_block, target_tx, opcode) = if let Some(ref cellpack) = tracker.cellpack {
        (
            cellpack.target.block,
            cellpack.target.tx,
            cellpack.inputs.first().copied().unwrap_or(0),
        )
    } else {
        (0, 0, 0)
    };
    
    // Get transaction ID
    let txid = transaction.compute_txid();
    let mut txid_bytes = [0u8; 32];
    txid_bytes.copy_from_slice(&txid.to_byte_array());
    
    // Prepare calldata (this would come from the actual protostone message)
    let mut calldata = [0u8; MAX_CALLDATA_SIZE];
    let calldata_len = 0; // TODO: Extract actual calldata from protostone
    
    Ok(GpuMessage {
        txid: txid_bytes,
        tx_index: tracker.tx_index as u32,
        protostone_index: tracker.protostone_index as u32,
        calldata,
        calldata_len,
        target_block,
        target_tx,
        opcode,
        result_offset: 0, // Will be set during batch processing
    })
}

/// Calculate expected output size for GPU batch
fn calculate_output_size(batch: &GpuMessageBatch) -> usize {
    // Base size for batch result structure
    let base_size = std::mem::size_of::<u32>() * 3; // success_flags count, storage_updates count, gas_used count
    
    // Size for success flags (one u32 per message)
    let success_flags_size = batch.messages.len() * std::mem::size_of::<u32>();
    
    // Size for gas used (one u64 per message)
    let gas_used_size = batch.messages.len() * std::mem::size_of::<u64>();
    
    // Estimated size for storage updates (conservative estimate)
    let storage_updates_size = batch.storage_slots.len() * std::mem::size_of::<GpuStorageSlot>();
    
    base_size + success_flags_size + gas_used_size + storage_updates_size
}

/// Serialize GPU batch to bytes for Vulkan execution
fn serialize_gpu_batch(batch: &GpuMessageBatch) -> Result<Vec<u8>> {
    // For now, use JSON serialization
    // TODO: Use more efficient binary serialization for production
    serde_json::to_vec(batch).map_err(|e| anyhow!("Failed to serialize GPU batch: {}", e))
}

/// Deserialize GPU execution result
fn deserialize_gpu_result(result_data: &[u8]) -> Result<GpuBatchResult> {
    // For now, expect JSON format
    // TODO: Use binary deserialization to match GPU output format
    serde_json::from_slice(result_data)
        .map_err(|e| anyhow!("Failed to deserialize GPU result: {}", e))
}

/// Process GPU execution results and update storage
fn process_gpu_results(
    result: &GpuBatchResult,
    batch: &GpuMessageBatch,
) -> Result<()> {
    crate::alkane_log!(
        "Processing GPU results: {} messages, {} storage updates",
        result.success_flags.len(),
        result.storage_updates.len()
    );
    
    // Validate result consistency
    if result.success_flags.len() != batch.messages.len() {
        return Err(anyhow!(
            "GPU result mismatch: expected {} success flags, got {}",
            batch.messages.len(),
            result.success_flags.len()
        ));
    }
    
    // Process each message result
    for (i, &success) in result.success_flags.iter().enumerate() {
        let message = &batch.messages[i];
        let gas_used = result.gas_used.get(i).copied().unwrap_or(0);
        
        if success == 1 {
            crate::alkane_log!(
                "GPU message {}/{} succeeded (gas: {})",
                i + 1,
                batch.messages.len(),
                gas_used
            );
        } else {
            crate::alkane_log!(
                "GPU message {}/{} failed (txid: {})",
                i + 1,
                batch.messages.len(),
                hex::encode(&message.txid)
            );
        }
    }
    
    // TODO: Apply storage updates to the actual database
    // This would require integration with the AtomicPointer system
    for update in &result.storage_updates {
        if update.key_len > 0 && update.value_len > 0 {
            let key = &update.key[..update.key_len as usize];
            let value = &update.value[..update.value_len as usize];
            
            crate::alkane_log!(
                "GPU storage update: {} -> {} bytes",
                hex::encode(key),
                value.len()
            );
            
            // TODO: Apply to actual storage via AtomicPointer
        }
    }
    
    Ok(())
}

/// Check if GPU execution is worthwhile for the given dependency stats
pub fn should_use_gpu(stats: &DependencyStats) -> bool {
    // Use GPU if we have enough parallelizable messages
    stats.parallelizable_trackers >= MIN_GPU_BATCH_SIZE &&
    stats.parallel_groups > 0 &&
    stats.largest_group_size >= MIN_GPU_BATCH_SIZE
}

/// Get GPU execution statistics for monitoring
pub fn get_gpu_stats() -> GpuExecutionStats {
    // TODO: Implement actual statistics tracking
    GpuExecutionStats {
        total_batches_attempted: 0,
        total_batches_successful: 0,
        total_messages_processed: 0,
        average_batch_size: 0.0,
        average_execution_time_ms: 0.0,
        gpu_utilization_percent: 0.0,
    }
}

/// Statistics for GPU execution monitoring
#[derive(Debug, Clone)]
pub struct GpuExecutionStats {
    pub total_batches_attempted: u64,
    pub total_batches_successful: u64,
    pub total_messages_processed: u64,
    pub average_batch_size: f64,
    pub average_execution_time_ms: f64,
    pub gpu_utilization_percent: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;
    use bitcoin::Txid;
    use crate::gpu_tracking::StorageTracker;
    
    #[test]
    fn test_gpu_message_creation() {
        let txid = Txid::from_byte_array([1; 32]);
        let tracker = StorageTracker::new(txid, 0, 0);
        
        // Create a dummy transaction
        let transaction = bitcoin::Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        };
        
        let gpu_message = create_gpu_message(&tracker, &transaction, 100).unwrap();
        
        assert_eq!(gpu_message.txid, [1; 32]);
        assert_eq!(gpu_message.tx_index, 0);
        assert_eq!(gpu_message.protostone_index, 0);
    }
    
    #[test]
    fn test_should_use_gpu() {
        use crate::gpu_tracking::DependencyStats;
        
        // Not enough parallelizable messages
        let stats = DependencyStats {
            total_trackers: 10,
            parallelizable_trackers: 2,
            total_conflicts: 0,
            parallel_groups: 1,
            largest_group_size: 2,
            parallelization_ratio: 0.2,
            storage_profiles_count: 1,
        };
        assert!(!should_use_gpu(&stats));
        
        // Enough parallelizable messages
        let stats = DependencyStats {
            total_trackers: 20,
            parallelizable_trackers: 8,
            total_conflicts: 2,
            parallel_groups: 2,
            largest_group_size: 6,
            parallelization_ratio: 0.4,
            storage_profiles_count: 3,
        };
        assert!(should_use_gpu(&stats));
    }
    
    #[test]
    fn test_gpu_batch_serialization() {
        let batch = GpuMessageBatch {
            messages: vec![],
            storage_slots: vec![],
            block_height: 100,
        };
        
        let serialized = serialize_gpu_batch(&batch).unwrap();
        assert!(!serialized.is_empty());
        
        // Should be valid JSON
        let _: serde_json::Value = serde_json::from_slice(&serialized).unwrap();
    }
}

/// Try to execute a block on GPU using dependency analysis
pub fn try_execute_on_gpu<T: MessageContext>(
    block: &Block,
    height: u64,
) -> Result<bool> {
    // Get dependency analysis for the current block
    let stats = match get_block_dependency_analysis() {
        Ok(stats) => stats,
        Err(_) => {
            crate::alkane_log!("No dependency analysis available for GPU execution");
            return Ok(false);
        }
    };
    
    // Check if GPU execution is worthwhile
    if !should_use_gpu(&stats) {
        crate::alkane_log!(
            "GPU execution not worthwhile: {} parallelizable trackers, {} groups, largest group: {}",
            stats.parallelizable_trackers,
            stats.parallel_groups,
            stats.largest_group_size
        );
        return Ok(false);
    }
    
    crate::alkane_log!(
        "Attempting GPU execution: {} parallelizable trackers in {} groups",
        stats.parallelizable_trackers,
        stats.parallel_groups
    );
    
    // For now, simulate GPU execution success
    // TODO: Implement actual GPU execution with SPIR-V binary
    let gpu_success = execute_block_on_gpu::<T>(block, height, &stats)?;
    
    if gpu_success {
        crate::alkane_log!("Block {} successfully executed on GPU", height);
    } else {
        crate::alkane_log!("Block {} GPU execution failed, falling back to CPU", height);
    }
    
    Ok(gpu_success)
}

/// Execute a block on GPU using the alkanes-gpu SPIR-V binary
fn execute_block_on_gpu<T: MessageContext>(
    block: &Block,
    height: u64,
    stats: &DependencyStats,
) -> Result<bool> {
    // Get the SPIR-V binary from alkanes-gpu crate
    let spirv_binary = match alkanes_gpu::build_spirv_binary() {
        Ok(binary) => binary,
        Err(e) => {
            crate::alkane_log!("Failed to build SPIR-V binary: {}", e);
            return Ok(false);
        }
    };
    
    // Create GPU execution shard from block data
    let mut shard = GpuExecutionShard::default();
    shard.context.height = height;
    shard.context.shard_id = 0;
    
    // Extract parallelizable messages from block
    let message_count = extract_gpu_messages(block, &mut shard)?;
    shard.message_count = message_count;
    
    if message_count == 0 {
        crate::alkane_log!("No GPU-compatible messages found in block");
        return Ok(false);
    }
    
    // Prepare Vulkan execution input with real SPIR-V
    let gpu_input = VulkanExecutionInput {
        shader_name: "alkanes_pipeline".to_string(),
        input_data: serialize_gpu_shard(&shard)?,
        output_size: std::mem::size_of::<GpuExecutionResult>(),
    };
    
    // Execute on GPU via Vulkan runtime
    let result_bytes = match metashrew_runtime::MetashrewRuntime::<()>::execute_gpu_work(
        &serde_json::to_vec(&gpu_input)?
    ) {
        Ok(bytes) => bytes,
        Err(e) => {
            crate::alkane_log!("GPU execution failed: {}", e);
            return Ok(false);
        }
    };
    
    // Parse GPU execution result
    let gpu_result: VulkanExecutionResult = serde_json::from_slice(&result_bytes)?;
    
    if !gpu_result.success {
        crate::alkane_log!(
            "GPU execution failed: {}",
            gpu_result.error_message.unwrap_or_else(|| "Unknown error".to_string())
        );
        return Ok(false);
    }
    
    // Process GPU results
    let execution_result = deserialize_gpu_execution_result(&gpu_result.output_data)?;
    process_gpu_execution_results(&execution_result, &shard)?;
    
    Ok(true)
}

/// Extract GPU-compatible messages from block transactions
fn extract_gpu_messages(block: &Block, shard: &mut GpuExecutionShard) -> Result<u32> {
    let mut message_count = 0u32;
    
    for (tx_index, transaction) in block.txdata.iter().enumerate() {
        if message_count >= MAX_SHARD_SIZE as u32 {
            break;
        }
        
        // Check if transaction has alkanes messages
        // TODO: Implement actual message extraction from protostones
        if has_alkanes_messages(transaction) {
            let gpu_message = create_gpu_message_input(transaction, tx_index as u32, 0)?;
            shard.messages[message_count as usize] = gpu_message;
            message_count += 1;
        }
    }
    
    Ok(message_count)
}

/// Check if transaction contains alkanes messages
fn has_alkanes_messages(transaction: &Transaction) -> bool {
    // TODO: Implement actual alkanes message detection
    // For now, check if transaction has OP_RETURN outputs (potential runestones)
    transaction.output.iter().any(|output| output.script_pubkey.is_op_return())
}

/// Create GPU message input from transaction
fn create_gpu_message_input(
    transaction: &Transaction,
    tx_index: u32,
    protostone_index: u32,
) -> Result<GpuMessageInput> {
    let txid = transaction.compute_txid();
    let mut txid_bytes = [0u8; 32];
    txid_bytes.copy_from_slice(&txid.to_byte_array());
    
    // TODO: Extract actual calldata from protostone
    let mut calldata = [0u8; 2048];
    let calldata_len = 0;
    
    Ok(GpuMessageInput {
        txid: txid_bytes,
        txindex: tx_index,
        height: 0, // Will be set by caller
        vout: 0,
        pointer: 0,
        refund_pointer: 0,
        calldata_len,
        calldata,
        runtime_balance_len: 0,
        runtime_balance_data: [0; 512],
        input_runes_len: 0,
        input_runes_data: [0; 512],
    })
}

/// Serialize GPU shard for Vulkan execution
fn serialize_gpu_shard(shard: &GpuExecutionShard) -> Result<Vec<u8>> {
    // Use binary serialization for efficiency
    // For now, use JSON as fallback
    serde_json::to_vec(shard).map_err(|e| anyhow!("Failed to serialize GPU shard: {}", e))
}

/// Deserialize GPU execution result
fn deserialize_gpu_execution_result(result_data: &[u8]) -> Result<GpuExecutionResult> {
    // For now, expect JSON format
    serde_json::from_slice(result_data)
        .map_err(|e| anyhow!("Failed to deserialize GPU execution result: {}", e))
}

/// Process GPU execution results and apply to storage
fn process_gpu_execution_results(
    result: &GpuExecutionResult,
    shard: &GpuExecutionShard,
) -> Result<()> {
    crate::alkane_log!(
        "Processing GPU execution results: status={}, {} return data, {} KV updates",
        result.status,
        result.return_data_count,
        result.kv_update_count
    );
    
    if result.status != 0 {
        let error_msg = if result.error_len > 0 {
            String::from_utf8_lossy(&result.error_message[..result.error_len as usize])
        } else {
            "Unknown GPU execution error".into()
        };
        return Err(anyhow!("GPU execution failed: {}", error_msg));
    }
    
    // Process return data for each message
    for i in 0..result.return_data_count.min(MAX_SHARD_SIZE as u32) {
        let return_data = &result.return_data[i as usize];
        if return_data.success == 1 {
            crate::alkane_log!(
                "GPU message {} succeeded with {} bytes of data",
                return_data.message_index,
                return_data.data_len
            );
        } else {
            crate::alkane_log!(
                "GPU message {} failed",
                return_data.message_index
            );
        }
    }
    
    // Process KV updates
    for i in 0..result.kv_update_count.min(MAX_KV_PAIRS as u32) {
        let kv_update = &result.kv_updates[i as usize];
        if kv_update.key_len > 0 {
            let key = &kv_update.key[..kv_update.key_len as usize];
            let value = &kv_update.value[..kv_update.value_len as usize];
            
            crate::alkane_log!(
                "GPU KV update: {} -> {} bytes (op={})",
                hex::encode(key),
                value.len(),
                kv_update.operation
            );
            
            // TODO: Apply KV updates to actual storage via AtomicPointer
        }
    }
    
    Ok(())
}