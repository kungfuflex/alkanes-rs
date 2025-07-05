//! Automatic GPU Parallel Processing for Alkanes Protocol Messages
//!
//! This module implements automatic storage dependency tracking and conflict analysis
//! to enable safe parallel execution of protocol messages on GPU without requiring
//! special markers. It analyzes cellpacks starting with 2 or 4, tracks storage
//! patterns by WASM hash and opcode, and builds dependency groups.
//!
//! Key features:
//! - Automatic parallelization for cellpacks [2, *] and [4, *]
//! - Storage pattern tracking by WASM hash and opcode
//! - Exclusion of transactions that create new alkanes
//! - Dependency analysis based on actual storage access patterns

use anyhow::Result;
use alkanes_support::cellpack::Cellpack;
use bitcoin::Txid;
use metashrew_core::index_pointer::AtomicPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::utils::decode_varint_list;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::sync::{Arc, Mutex};

/// Tracks storage operations for a single protocol message execution
#[derive(Debug, Clone)]
pub struct StorageTracker {
    /// Transaction ID this tracker belongs to
    pub txid: Txid,
    /// Transaction index in the block
    pub tx_index: usize,
    /// Protostone index within the transaction
    pub protostone_index: usize,
    /// Parsed cellpack if this is a protocol message
    pub cellpack: Option<Cellpack>,
    /// WASM hash for the target alkane (if resolved)
    pub wasm_hash: Option<Vec<u8>>,
    /// Opcode from cellpack (first input after target)
    pub opcode: Option<u128>,
    /// Storage slots that are read during execution
    pub read_slots: BTreeSet<Vec<u8>>,
    /// Storage slots that are written during execution
    pub write_slots: BTreeSet<Vec<u8>>,
    /// Whether this message can be parallelized
    pub is_parallelizable: bool,
    /// Whether this transaction creates a new alkane
    pub creates_new_alkane: bool,
    /// Whether this tracker is currently recording operations
    pub is_recording: bool,
}

impl StorageTracker {
    pub fn new(txid: Txid, tx_index: usize, protostone_index: usize) -> Self {
        Self {
            txid,
            tx_index,
            protostone_index,
            cellpack: None,
            wasm_hash: None,
            opcode: None,
            read_slots: BTreeSet::new(),
            write_slots: BTreeSet::new(),
            is_parallelizable: false,
            creates_new_alkane: false,
            is_recording: true,
        }
    }

    /// Create a tracker from calldata, analyzing if it's parallelizable
    pub fn from_calldata(
        txid: Txid, 
        tx_index: usize, 
        protostone_index: usize, 
        calldata: &[u8],
        height: u64,
    ) -> Result<Self> {
        let mut tracker = Self::new(txid, tx_index, protostone_index);
        
        if calldata.is_empty() {
            return Ok(tracker);
        }

        // Try to parse cellpack
        match decode_varint_list(&mut Cursor::new(calldata.to_vec())) {
            Ok(values) => {
                if let Ok(cellpack) = Cellpack::try_from(values) {
                    tracker.analyze_cellpack(cellpack, height)?;
                }
            }
            Err(_) => {
                // Not a valid cellpack, not parallelizable
            }
        }

        Ok(tracker)
    }

    /// Analyze a cellpack to determine if it's parallelizable
    fn analyze_cellpack(&mut self, cellpack: Cellpack, height: u64) -> Result<()> {
        self.cellpack = Some(cellpack.clone());

        // Only cellpacks starting with 2 or 4 can be parallelized
        let target_block = cellpack.target.block;
        if target_block != 2 && target_block != 4 {
            return Ok(());
        }

        // Check if this creates a new alkane (instantiation in current block)
        if cellpack.target.block == height as u128 {
            self.creates_new_alkane = true;
            return Ok(());
        }

        // Extract opcode (first input after target)
        if !cellpack.inputs.is_empty() {
            self.opcode = Some(cellpack.inputs[0]);
        }

        // TODO: Resolve WASM hash for the target alkane
        // This would require looking up the alkane's WASM bytecode
        // For now, we'll use the target as a placeholder
        self.wasm_hash = Some(format!("{:032x}{:032x}", cellpack.target.block, cellpack.target.tx).into_bytes());

        // Mark as parallelizable if all conditions are met
        self.is_parallelizable = !self.creates_new_alkane;

        Ok(())
    }

    /// Record a storage read operation
    pub fn record_read(&mut self, key: &[u8]) {
        if self.is_recording {
            self.read_slots.insert(key.to_vec());
        }
    }

    /// Record a storage write operation
    pub fn record_write(&mut self, key: &[u8]) {
        if self.is_recording {
            self.write_slots.insert(key.to_vec());
        }
    }

    /// Check if this tracker conflicts with another
    pub fn conflicts_with(&self, other: &StorageTracker) -> bool {
        // Two transactions conflict if:
        // 1. One writes to a slot the other reads
        // 2. One writes to a slot the other writes
        // 3. They are in the same transaction (can't parallelize within a tx)
        
        if self.txid == other.txid {
            return true; // Same transaction - always conflicts
        }

        // Check for read-write conflicts
        for write_slot in &self.write_slots {
            if other.read_slots.contains(write_slot) || other.write_slots.contains(write_slot) {
                return true;
            }
        }

        // Check for write-read conflicts
        for read_slot in &self.read_slots {
            if other.write_slots.contains(read_slot) {
                return true;
            }
        }

        false
    }

    /// Get all storage slots accessed (read or write)
    pub fn all_accessed_slots(&self) -> BTreeSet<Vec<u8>> {
        let mut all_slots = self.read_slots.clone();
        all_slots.extend(self.write_slots.iter().cloned());
        all_slots
    }

    /// Get a storage profile key for this tracker
    pub fn get_storage_profile_key(&self) -> Option<String> {
        if let (Some(wasm_hash), Some(opcode)) = (&self.wasm_hash, &self.opcode) {
            Some(format!("/storagepaths/{}/{}", 
                hex::encode(wasm_hash), 
                opcode
            ))
        } else {
            None
        }
    }
}

/// Profiles storage access patterns for a specific WASM contract and opcode
#[derive(Debug, Clone)]
pub struct WasmStorageProfile {
    /// WASM bytecode hash
    pub wasm_hash: Vec<u8>,
    /// Opcode being executed
    pub opcode: u128,
    /// Known storage slots that are typically read
    pub known_read_slots: BTreeSet<Vec<u8>>,
    /// Known storage slots that are typically written
    pub known_write_slots: BTreeSet<Vec<u8>>,
    /// Number of times this pattern has been observed
    pub access_count: u64,
    /// Last time this pattern was updated
    pub last_updated_height: u64,
}

impl WasmStorageProfile {
    pub fn new(wasm_hash: Vec<u8>, opcode: u128, height: u64) -> Self {
        Self {
            wasm_hash,
            opcode,
            known_read_slots: BTreeSet::new(),
            known_write_slots: BTreeSet::new(),
            access_count: 0,
            last_updated_height: height,
        }
    }

    /// Update the profile with new storage access patterns
    pub fn update_from_tracker(&mut self, tracker: &StorageTracker, height: u64) {
        self.known_read_slots.extend(tracker.read_slots.iter().cloned());
        self.known_write_slots.extend(tracker.write_slots.iter().cloned());
        self.access_count += 1;
        self.last_updated_height = height;
    }

    /// Predict if two profiles might conflict based on known patterns
    pub fn might_conflict_with(&self, other: &WasmStorageProfile) -> bool {
        // Check for potential read-write conflicts
        for write_slot in &self.known_write_slots {
            if other.known_read_slots.contains(write_slot) || other.known_write_slots.contains(write_slot) {
                return true;
            }
        }

        // Check for potential write-read conflicts
        for read_slot in &self.known_read_slots {
            if other.known_write_slots.contains(read_slot) {
                return true;
            }
        }

        false
    }
}

/// Enhanced AtomicPointer that tracks storage operations for GPU dependency analysis
#[derive(Debug, Clone)]
pub struct TrackedAtomicPointer {
    /// The underlying AtomicPointer
    pub atomic: AtomicPointer,
    /// Storage tracker for this execution context
    pub tracker: Arc<Mutex<StorageTracker>>,
}

impl TrackedAtomicPointer {
    pub fn new(atomic: AtomicPointer, tracker: StorageTracker) -> Self {
        Self {
            atomic,
            tracker: Arc::new(Mutex::new(tracker)),
        }
    }

    /// Create a derived pointer that shares the same tracker
    pub fn derive(&self, pointer: &metashrew_core::index_pointer::IndexPointer) -> Self {
        Self {
            atomic: self.atomic.derive(pointer),
            tracker: self.tracker.clone(),
        }
    }

    /// Record a read operation and delegate to underlying AtomicPointer
    pub fn get(&self) -> Arc<Vec<u8>> {
        let key = self.atomic.unwrap();
        if let Ok(mut tracker) = self.tracker.lock() {
            tracker.record_read(&key);
        }
        self.atomic.get()
    }

    /// Record a write operation and delegate to underlying AtomicPointer
    pub fn set(&mut self, value: Arc<Vec<u8>>) {
        let key = self.atomic.unwrap();
        if let Ok(mut tracker) = self.tracker.lock() {
            tracker.record_write(&key);
        }
        self.atomic.set(value);
    }

    /// Get the underlying AtomicPointer for operations that don't need tracking
    pub fn inner(&mut self) -> &mut AtomicPointer {
        &mut self.atomic
    }

    /// Get a copy of the current storage tracker
    pub fn get_tracker(&self) -> Result<StorageTracker> {
        self.tracker
            .lock()
            .map(|t| t.clone())
            .map_err(|e| anyhow::anyhow!("Failed to lock tracker: {}", e))
    }
}

/// Analyzes storage dependencies across multiple protocol messages to find parallelizable groups
#[derive(Debug)]
pub struct DependencyAnalyzer {
    /// All storage trackers from the current block
    trackers: Vec<StorageTracker>,
    /// Storage profiles indexed by WASM hash and opcode
    storage_profiles: BTreeMap<String, WasmStorageProfile>,
    /// Conflict matrix: conflicts[i][j] = true if tracker i conflicts with tracker j
    conflict_matrix: Vec<Vec<bool>>,
    /// Current block height
    current_height: u64,
}

impl DependencyAnalyzer {
    pub fn new(height: u64) -> Self {
        Self {
            trackers: Vec::new(),
            storage_profiles: BTreeMap::new(),
            conflict_matrix: Vec::new(),
            current_height: height,
        }
    }

    /// Add a storage tracker to the analysis
    pub fn add_tracker(&mut self, tracker: StorageTracker) {
        // Update storage profiles if this tracker has a profile key
        if let Some(profile_key) = tracker.get_storage_profile_key() {
            let profile = self.storage_profiles.entry(profile_key.clone()).or_insert_with(|| {
                WasmStorageProfile::new(
                    tracker.wasm_hash.clone().unwrap_or_default(),
                    tracker.opcode.unwrap_or(0),
                    self.current_height,
                )
            });
            profile.update_from_tracker(&tracker, self.current_height);
        }

        self.trackers.push(tracker);
    }

    /// Filter trackers to only include parallelizable ones
    pub fn filter_parallelizable(&mut self) {
        self.trackers.retain(|tracker| tracker.is_parallelizable);
    }

    /// Build the conflict matrix between all trackers
    pub fn build_conflict_matrix(&mut self) {
        let n = self.trackers.len();
        self.conflict_matrix = vec![vec![false; n]; n];

        for i in 0..n {
            for j in 0..n {
                if i != j {
                    self.conflict_matrix[i][j] = self.trackers[i].conflicts_with(&self.trackers[j]);
                }
            }
        }
    }

    /// Find groups of non-conflicting trackers that can be executed in parallel
    pub fn find_parallel_groups(&self) -> Vec<Vec<usize>> {
        let n = self.trackers.len();
        let mut assigned = vec![false; n];
        let mut groups = Vec::new();

        for i in 0..n {
            if assigned[i] {
                continue;
            }

            let mut group = vec![i];
            assigned[i] = true;

            // Find all trackers that don't conflict with any in the current group
            for j in (i + 1)..n {
                if assigned[j] {
                    continue;
                }

                let conflicts_with_group = group.iter().any(|&group_member| {
                    self.conflict_matrix[group_member][j]
                });

                if !conflicts_with_group {
                    group.push(j);
                    assigned[j] = true;
                }
            }

            groups.push(group);
        }

        groups
    }

    /// Get statistics about the dependency analysis
    pub fn get_stats(&self) -> DependencyStats {
        let total_trackers = self.trackers.len();
        let parallelizable_trackers = self.trackers.iter()
            .filter(|t| t.is_parallelizable)
            .count();
        let total_conflicts = self.conflict_matrix.iter()
            .flat_map(|row| row.iter())
            .filter(|&&conflict| conflict)
            .count() / 2; // Divide by 2 since matrix is symmetric

        let groups = self.find_parallel_groups();
        let largest_group_size = groups.iter().map(|g| g.len()).max().unwrap_or(0);

        DependencyStats {
            total_trackers,
            parallelizable_trackers,
            total_conflicts,
            parallel_groups: groups.len(),
            largest_group_size,
            parallelization_ratio: if total_trackers > 0 {
                parallelizable_trackers as f64 / total_trackers as f64
            } else {
                0.0
            },
            storage_profiles_count: self.storage_profiles.len(),
        }
    }

    /// Get storage profiles for analysis
    pub fn get_storage_profiles(&self) -> &BTreeMap<String, WasmStorageProfile> {
        &self.storage_profiles
    }
}

/// Statistics about dependency analysis results
#[derive(Debug, Clone)]
pub struct DependencyStats {
    pub total_trackers: usize,
    pub parallelizable_trackers: usize,
    pub total_conflicts: usize,
    pub parallel_groups: usize,
    pub largest_group_size: usize,
    pub parallelization_ratio: f64,
    pub storage_profiles_count: usize,
}

/// Global storage for tracking dependencies across the entire block
static mut BLOCK_DEPENDENCY_ANALYZER: Option<Mutex<DependencyAnalyzer>> = None;

/// Initialize the global dependency analyzer for a new block
pub fn init_block_dependency_tracking(height: u64) {
    unsafe {
        BLOCK_DEPENDENCY_ANALYZER = Some(Mutex::new(DependencyAnalyzer::new(height)));
    }
}

/// Add a tracker to the global dependency analyzer
pub fn add_tracker_to_block(tracker: StorageTracker) -> Result<()> {
    unsafe {
        if let Some(ref analyzer_mutex) = BLOCK_DEPENDENCY_ANALYZER {
            let mut analyzer = analyzer_mutex.lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock dependency analyzer: {}", e))?;
            analyzer.add_tracker(tracker);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Block dependency tracking not initialized"))
        }
    }
}

/// Get the current dependency analysis results
pub fn get_block_dependency_analysis() -> Result<DependencyStats> {
    unsafe {
        if let Some(ref analyzer_mutex) = BLOCK_DEPENDENCY_ANALYZER {
            let mut analyzer = analyzer_mutex.lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock dependency analyzer: {}", e))?;
            analyzer.filter_parallelizable();
            analyzer.build_conflict_matrix();
            Ok(analyzer.get_stats())
        } else {
            Err(anyhow::anyhow!("Block dependency tracking not initialized"))
        }
    }
}

/// Get parallel execution groups for GPU processing
pub fn get_parallel_groups() -> Result<Vec<Vec<usize>>> {
    unsafe {
        if let Some(ref analyzer_mutex) = BLOCK_DEPENDENCY_ANALYZER {
            let mut analyzer = analyzer_mutex.lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock dependency analyzer: {}", e))?;
            analyzer.filter_parallelizable();
            analyzer.build_conflict_matrix();
            Ok(analyzer.find_parallel_groups())
        } else {
            Err(anyhow::anyhow!("Block dependency tracking not initialized"))
        }
    }
}

/// Get storage profiles for analysis
pub fn get_storage_profiles() -> Result<BTreeMap<String, WasmStorageProfile>> {
    unsafe {
        if let Some(ref analyzer_mutex) = BLOCK_DEPENDENCY_ANALYZER {
            let analyzer = analyzer_mutex.lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock dependency analyzer: {}", e))?;
            Ok(analyzer.get_storage_profiles().clone())
        } else {
            Err(anyhow::anyhow!("Block dependency tracking not initialized"))
        }
    }
}

/// Clear the global dependency analyzer (called at end of block processing)
pub fn clear_block_dependency_tracking() {
    unsafe {
        BLOCK_DEPENDENCY_ANALYZER = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    #[test]
    fn test_storage_tracker_conflicts() {
        let txid1 = Txid::from_byte_array([1; 32]);
        let txid2 = Txid::from_byte_array([2; 32]);

        let mut tracker1 = StorageTracker::new(txid1, 0, 0);
        let mut tracker2 = StorageTracker::new(txid2, 1, 0);

        // No conflicts initially
        assert!(!tracker1.conflicts_with(&tracker2));

        // Add non-conflicting operations
        tracker1.record_read(b"key1");
        tracker2.record_read(b"key2");
        assert!(!tracker1.conflicts_with(&tracker2));

        // Add conflicting operations (write-read conflict)
        tracker1.record_write(b"key2");
        assert!(tracker1.conflicts_with(&tracker2));
    }

    #[test]
    fn test_same_transaction_always_conflicts() {
        let txid = Txid::from_byte_array([1; 32]);
        let tracker1 = StorageTracker::new(txid, 0, 0);
        let tracker2 = StorageTracker::new(txid, 0, 1);

        // Same transaction should always conflict
        assert!(tracker1.conflicts_with(&tracker2));
    }

    #[test]
    fn test_dependency_analyzer() {
        let mut analyzer = DependencyAnalyzer::new(100);

        let txid1 = Txid::from_byte_array([1; 32]);
        let txid2 = Txid::from_byte_array([2; 32]);
        let txid3 = Txid::from_byte_array([3; 32]);

        let mut tracker1 = StorageTracker::new(txid1, 0, 0);
        let mut tracker2 = StorageTracker::new(txid2, 1, 0);
        let mut tracker3 = StorageTracker::new(txid3, 2, 0);

        // Mark as parallelizable
        tracker1.is_parallelizable = true;
        tracker2.is_parallelizable = true;
        tracker3.is_parallelizable = true;

        // Set up non-conflicting operations
        tracker1.record_read(b"key1");
        tracker2.record_read(b"key2");
        tracker3.record_read(b"key3");

        analyzer.add_tracker(tracker1);
        analyzer.add_tracker(tracker2);
        analyzer.add_tracker(tracker3);

        analyzer.filter_parallelizable();
        analyzer.build_conflict_matrix();
        let groups = analyzer.find_parallel_groups();

        // All should be in one group since no conflicts
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 3);
    }

    #[test]
    fn test_cellpack_parallelization_rules() {
        let txid = Txid::from_byte_array([1; 32]);

        // Test cellpack starting with 2 (parallelizable)
        let calldata_2 = vec![2u8, 100u8, 77u8]; // [2, 100, 77] as bytes
        let tracker = StorageTracker::from_calldata(txid, 0, 0, &calldata_2, 50).unwrap();
        // Should be parallelizable since target block (2) != current height (50)
        assert!(tracker.is_parallelizable);

        // Test cellpack starting with 4 (parallelizable)
        let calldata_4 = vec![4u8, 200u8, 88u8]; // [4, 200, 88] as bytes
        let tracker = StorageTracker::from_calldata(txid, 0, 0, &calldata_4, 50).unwrap();
        assert!(tracker.is_parallelizable);

        // Test cellpack starting with 1 (not parallelizable)
        let calldata_1 = vec![1u8, 100u8, 77u8]; // [1, 100, 77] as bytes
        let tracker = StorageTracker::from_calldata(txid, 0, 0, &calldata_1, 50).unwrap();
        assert!(!tracker.is_parallelizable);

        // Test new alkane creation (not parallelizable)
        let calldata_new = vec![2u8, 100u8, 77u8]; // [2, 100, 77] as bytes
        let tracker = StorageTracker::from_calldata(txid, 0, 0, &calldata_new, 2).unwrap(); // height = target block
        assert!(!tracker.is_parallelizable);
        assert!(tracker.creates_new_alkane);
    }
}