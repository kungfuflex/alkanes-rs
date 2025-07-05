//! GPU Clustering Pipeline for Conflict Detection and Shard Merging
//!
//! This module implements the iterative clustering pipeline that:
//! 1. Detects conflicts between shards (overlapping K/V writes)
//! 2. Merges conflicting shards preserving transaction order
//! 3. Iteratively clusters until GPU memory limits are reached
//! 4. Falls back to WASM for remaining oversized shards
//!
//! The goal is to maximize GPU utilization while maintaining perfect
//! equivalence to WASM execution behavior.

use anyhow::Result;
use std::collections::HashSet;
use log::{info, debug, warn};

use crate::gpu_abi::{
    GpuExecutionShard, GpuKvPair,
    MAX_GPU_MEMORY_PER_SHARD, MIN_SHARD_SIZE_FOR_GPU
};

/// Clustering pipeline statistics
#[derive(Debug, Clone)]
pub struct ClusteringStats {
    /// Initial number of shards
    pub initial_shard_count: usize,
    /// Number of clustering passes performed
    pub clustering_passes: usize,
    /// Final number of GPU shards
    pub final_gpu_shard_count: usize,
    /// Number of shards falling back to WASM
    pub wasm_fallback_count: usize,
    /// Total conflicts detected
    pub total_conflicts_detected: usize,
    /// Total merges performed
    pub total_merges_performed: usize,
    /// Total memory usage estimate
    pub total_memory_usage_bytes: usize,
    /// Clustering time in microseconds
    pub clustering_time_us: u64,
}

impl Default for ClusteringStats {
    fn default() -> Self {
        Self {
            initial_shard_count: 0,
            clustering_passes: 0,
            final_gpu_shard_count: 0,
            wasm_fallback_count: 0,
            total_conflicts_detected: 0,
            total_merges_performed: 0,
            total_memory_usage_bytes: 0,
            clustering_time_us: 0,
        }
    }
}

/// Shard with metadata for clustering
#[derive(Debug, Clone)]
pub struct ClusteringShard {
    /// The GPU execution shard
    pub shard: GpuExecutionShard,
    /// Original transaction ordering indices
    pub transaction_order: Vec<usize>,
    /// K/V write operations for conflict detection
    pub write_operations: Vec<GpuKvPair>,
    /// Estimated memory usage
    pub memory_usage_bytes: usize,
    /// Whether this shard is eligible for GPU processing
    pub gpu_eligible: bool,
}

impl ClusteringShard {
    /// Create new clustering shard from GPU execution shard
    pub fn new(shard: GpuExecutionShard, transaction_order: Vec<usize>) -> Self {
        let mut write_operations = Vec::new();
        let mut memory_usage = 0;
        let mut gpu_eligible = true;
        
        // Extract write operations from shard context
        for i in 0..shard.context.kv_count as usize {
            let kv_pair = &shard.context.kv_pairs[i];
            if kv_pair.operation == 1 { // Write operation
                write_operations.push(*kv_pair);
            }
            memory_usage += kv_pair.key_len as usize + kv_pair.value_len as usize;
        }
        
        // Check GPU eligibility
        for kv_pair in &write_operations {
            if !GpuKvPair::is_gpu_eligible(kv_pair.key_slice(), kv_pair.value_slice()) {
                gpu_eligible = false;
                break;
            }
        }
        
        // Add base memory overhead
        memory_usage += std::mem::size_of::<GpuExecutionShard>();
        
        Self {
            shard,
            transaction_order,
            write_operations,
            memory_usage_bytes: memory_usage,
            gpu_eligible,
        }
    }
    
    /// Merge with another clustering shard, preserving transaction order
    pub fn merge_with(&mut self, other: ClusteringShard) -> Result<()> {
        // Merge transaction orders while preserving original ordering
        let mut merged_order = self.transaction_order.clone();
        merged_order.extend(other.transaction_order);
        merged_order.sort(); // Preserve original transaction order
        self.transaction_order = merged_order;
        
        // Merge write operations
        self.write_operations.extend(other.write_operations);
        
        // Update memory usage
        self.memory_usage_bytes += other.memory_usage_bytes;
        
        // Update GPU eligibility
        self.gpu_eligible = self.gpu_eligible && other.gpu_eligible;
        
        // Merge shard messages (up to MAX_SHARD_SIZE)
        let current_count = self.shard.message_count as usize;
        let other_count = other.shard.message_count as usize;
        let total_count = current_count + other_count;
        
        if total_count > crate::gpu_abi::MAX_SHARD_SIZE {
            warn!("Merged shard exceeds MAX_SHARD_SIZE, truncating");
            let available_slots = crate::gpu_abi::MAX_SHARD_SIZE - current_count;
            let messages_to_copy = std::cmp::min(other_count, available_slots);
            
            for i in 0..messages_to_copy {
                self.shard.messages[current_count + i] = other.shard.messages[i];
            }
            self.shard.message_count = crate::gpu_abi::MAX_SHARD_SIZE as u32;
        } else {
            for i in 0..other_count {
                self.shard.messages[current_count + i] = other.shard.messages[i];
            }
            self.shard.message_count = total_count as u32;
        }
        
        // Merge K/V context
        let current_kv_count = self.shard.context.kv_count as usize;
        let other_kv_count = other.shard.context.kv_count as usize;
        let total_kv_count = current_kv_count + other_kv_count;
        
        if total_kv_count > crate::gpu_abi::MAX_KV_PAIRS {
            warn!("Merged shard K/V pairs exceed MAX_KV_PAIRS, truncating");
            let available_kv_slots = crate::gpu_abi::MAX_KV_PAIRS - current_kv_count;
            let kvs_to_copy = std::cmp::min(other_kv_count, available_kv_slots);
            
            for i in 0..kvs_to_copy {
                self.shard.context.kv_pairs[current_kv_count + i] = other.shard.context.kv_pairs[i];
            }
            self.shard.context.kv_count = crate::gpu_abi::MAX_KV_PAIRS as u32;
        } else {
            for i in 0..other_kv_count {
                self.shard.context.kv_pairs[current_kv_count + i] = other.shard.context.kv_pairs[i];
            }
            self.shard.context.kv_count = total_kv_count as u32;
        }
        
        Ok(())
    }
    
    /// Check if this shard fits within GPU memory constraints
    pub fn fits_in_gpu_memory(&self) -> bool {
        self.memory_usage_bytes <= MAX_GPU_MEMORY_PER_SHARD && self.gpu_eligible
    }
    
    /// Check if this shard is large enough to justify GPU processing
    pub fn justifies_gpu_processing(&self) -> bool {
        self.shard.message_count as usize >= MIN_SHARD_SIZE_FOR_GPU
    }
}

/// GPU clustering pipeline for conflict detection and shard merging
pub struct GpuClusteringPipeline {
    /// Current clustering shards
    shards: Vec<ClusteringShard>,
    /// Clustering statistics
    stats: ClusteringStats,
}

impl GpuClusteringPipeline {
    /// Create new clustering pipeline
    pub fn new(initial_shards: Vec<GpuExecutionShard>) -> Self {
        let start_time = std::time::Instant::now();
        
        let mut clustering_shards = Vec::new();
        for (i, shard) in initial_shards.into_iter().enumerate() {
            let transaction_order = (0..shard.message_count as usize).collect();
            clustering_shards.push(ClusteringShard::new(shard, transaction_order));
        }
        
        let mut stats = ClusteringStats::default();
        stats.initial_shard_count = clustering_shards.len();
        
        info!("Initialized clustering pipeline with {} shards", clustering_shards.len());
        
        Self {
            shards: clustering_shards,
            stats,
        }
    }
    
    /// Run the complete clustering pipeline
    pub fn run_clustering(&mut self) -> Result<ClusteringStats> {
        let start_time = std::time::Instant::now();
        
        info!("Starting GPU clustering pipeline with {} initial shards", self.shards.len());
        
        let mut pass_count = 0;
        let mut conflicts_found = true;
        
        // Iterative clustering until no more conflicts or memory limits reached
        while conflicts_found && pass_count < 10 { // Max 10 passes to prevent infinite loops
            pass_count += 1;
            info!("Clustering pass {}", pass_count);
            
            conflicts_found = self.perform_clustering_pass()?;
            
            if conflicts_found {
                info!("Pass {} completed with conflicts resolved, {} shards remaining", 
                      pass_count, self.shards.len());
            } else {
                info!("Pass {} completed with no conflicts, clustering finished", pass_count);
            }
        }
        
        // Separate GPU-eligible and WASM fallback shards
        let (gpu_shards, wasm_shards): (Vec<_>, Vec<_>) = self.shards.iter()
            .partition(|shard| shard.fits_in_gpu_memory() && shard.justifies_gpu_processing());
        
        self.stats.clustering_passes = pass_count;
        self.stats.final_gpu_shard_count = gpu_shards.len();
        self.stats.wasm_fallback_count = wasm_shards.len();
        self.stats.clustering_time_us = start_time.elapsed().as_micros() as u64;
        
        // Calculate total memory usage
        self.stats.total_memory_usage_bytes = self.shards.iter()
            .map(|s| s.memory_usage_bytes)
            .sum();
        
        info!("Clustering completed: {} GPU shards, {} WASM fallback shards, {} passes",
              self.stats.final_gpu_shard_count, self.stats.wasm_fallback_count, self.stats.clustering_passes);
        
        Ok(self.stats.clone())
    }
    
    /// Perform a single clustering pass
    fn perform_clustering_pass(&mut self) -> Result<bool> {
        let mut conflicts_detected = false;
        let mut merges_performed = 0;
        
        // Build conflict detection matrix
        let conflict_matrix = self.build_conflict_matrix()?;
        
        // Find shards to merge based on conflicts
        let merge_groups = self.find_merge_groups(&conflict_matrix)?;
        
        if !merge_groups.is_empty() {
            conflicts_detected = true;
            
            // Perform merges
            let mut new_shards = Vec::new();
            let mut merged_indices = HashSet::new();
            
            for merge_group in merge_groups {
                if merge_group.len() < 2 {
                    continue;
                }
                
                // Merge all shards in the group
                let mut merged_shard = self.shards[merge_group[0]].clone();
                merged_indices.insert(merge_group[0]);
                
                for &shard_idx in &merge_group[1..] {
                    if merged_indices.contains(&shard_idx) {
                        continue;
                    }
                    
                    let other_shard = self.shards[shard_idx].clone();
                    merged_shard.merge_with(other_shard)?;
                    merged_indices.insert(shard_idx);
                    merges_performed += 1;
                }
                
                new_shards.push(merged_shard);
            }
            
            // Add non-merged shards
            for (i, shard) in self.shards.iter().enumerate() {
                if !merged_indices.contains(&i) {
                    new_shards.push(shard.clone());
                }
            }
            
            self.shards = new_shards;
            self.stats.total_merges_performed += merges_performed;
            
            debug!("Clustering pass merged {} shards, {} shards remaining", 
                   merges_performed, self.shards.len());
        }
        
        Ok(conflicts_detected)
    }
    
    /// Build conflict detection matrix between all shards
    fn build_conflict_matrix(&mut self) -> Result<Vec<Vec<bool>>> {
        let shard_count = self.shards.len();
        let mut conflict_matrix = vec![vec![false; shard_count]; shard_count];
        
        for i in 0..shard_count {
            for j in (i + 1)..shard_count {
                let has_conflict = self.detect_conflict_between_shards(i, j)?;
                conflict_matrix[i][j] = has_conflict;
                conflict_matrix[j][i] = has_conflict;
                
                if has_conflict {
                    self.stats.total_conflicts_detected += 1;
                    debug!("Conflict detected between shards {} and {}", i, j);
                }
            }
        }
        
        Ok(conflict_matrix)
    }
    
    /// Detect conflicts between two shards
    fn detect_conflict_between_shards(&self, shard1_idx: usize, shard2_idx: usize) -> Result<bool> {
        let shard1 = &self.shards[shard1_idx];
        let shard2 = &self.shards[shard2_idx];
        
        // Check for overlapping write operations
        for kv1 in &shard1.write_operations {
            for kv2 in &shard2.write_operations {
                if kv1.conflicts_with(kv2) {
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
    
    /// Find groups of shards that should be merged together
    fn find_merge_groups(&self, conflict_matrix: &[Vec<bool>]) -> Result<Vec<Vec<usize>>> {
        let shard_count = self.shards.len();
        let mut visited = vec![false; shard_count];
        let mut merge_groups = Vec::new();
        
        for i in 0..shard_count {
            if visited[i] {
                continue;
            }
            
            let mut group = Vec::new();
            let mut stack = vec![i];
            
            while let Some(current) = stack.pop() {
                if visited[current] {
                    continue;
                }
                
                visited[current] = true;
                group.push(current);
                
                // Find all shards that conflict with current shard
                for j in 0..shard_count {
                    if !visited[j] && conflict_matrix[current][j] {
                        // Check if merging would exceed memory limits
                        let combined_memory = self.shards[current].memory_usage_bytes + 
                                            self.shards[j].memory_usage_bytes;
                        
                        if combined_memory <= MAX_GPU_MEMORY_PER_SHARD {
                            stack.push(j);
                        } else {
                            debug!("Skipping merge of shards {} and {} due to memory constraints", 
                                   current, j);
                        }
                    }
                }
            }
            
            if group.len() > 1 {
                merge_groups.push(group);
            }
        }
        
        Ok(merge_groups)
    }
    
    /// Get GPU-eligible shards
    pub fn get_gpu_shards(&self) -> Vec<GpuExecutionShard> {
        self.shards.iter()
            .filter(|shard| shard.fits_in_gpu_memory() && shard.justifies_gpu_processing())
            .map(|shard| shard.shard.clone())
            .collect()
    }
    
    /// Get WASM fallback shards
    pub fn get_wasm_fallback_shards(&self) -> Vec<GpuExecutionShard> {
        self.shards.iter()
            .filter(|shard| !shard.fits_in_gpu_memory() || !shard.justifies_gpu_processing())
            .map(|shard| shard.shard.clone())
            .collect()
    }
    
    /// Get clustering statistics
    pub fn get_stats(&self) -> &ClusteringStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_abi::{GpuExecutionContext, GpuMessageInput};
    
    fn create_test_shard(message_count: u32, kv_count: u32) -> GpuExecutionShard {
        let mut shard = GpuExecutionShard::default();
        shard.message_count = message_count;
        shard.context.kv_count = kv_count;
        shard
    }
    
    #[test]
    fn test_clustering_shard_creation() {
        let shard = create_test_shard(5, 10);
        let transaction_order = vec![0, 1, 2, 3, 4];
        
        let clustering_shard = ClusteringShard::new(shard, transaction_order);
        assert_eq!(clustering_shard.transaction_order.len(), 5);
        assert!(clustering_shard.memory_usage_bytes > 0);
    }
    
    #[test]
    fn test_shard_merging() {
        let shard1 = create_test_shard(3, 5);
        let shard2 = create_test_shard(2, 3);
        
        let mut clustering_shard1 = ClusteringShard::new(shard1, vec![0, 1, 2]);
        let clustering_shard2 = ClusteringShard::new(shard2, vec![3, 4]);
        
        clustering_shard1.merge_with(clustering_shard2).unwrap();
        
        assert_eq!(clustering_shard1.shard.message_count, 5);
        assert_eq!(clustering_shard1.transaction_order, vec![0, 1, 2, 3, 4]);
    }
    
    #[test]
    fn test_clustering_pipeline_initialization() {
        let shards = vec![
            create_test_shard(5, 10),
            create_test_shard(3, 8),
            create_test_shard(7, 15),
        ];
        
        let pipeline = GpuClusteringPipeline::new(shards);
        assert_eq!(pipeline.shards.len(), 3);
        assert_eq!(pipeline.stats.initial_shard_count, 3);
    }
    
    #[test]
    fn test_conflict_detection() {
        let shards = vec![
            create_test_shard(2, 2),
            create_test_shard(2, 2),
        ];
        
        let mut pipeline = GpuClusteringPipeline::new(shards);
        let conflict_matrix = pipeline.build_conflict_matrix().unwrap();
        
        assert_eq!(conflict_matrix.len(), 2);
        assert_eq!(conflict_matrix[0].len(), 2);
    }
}