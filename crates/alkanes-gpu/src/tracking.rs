//! Storage dependency tracking and parallel group detection.
//!
//! Analyzes a block's contract messages to find which ones can execute
//! in parallel on GPU without conflicting storage access. Messages that
//! read/write overlapping storage keys must be serialized; messages with
//! disjoint storage can run in parallel as a GPU shard.

use std::collections::{BTreeMap, BTreeSet};

/// Tracks storage operations for a single contract message.
#[derive(Debug, Clone)]
pub struct StorageTracker {
    /// Transaction index in the block
    pub tx_index: usize,
    /// Message index within the transaction (protostone index)
    pub msg_index: usize,
    /// Target contract: (block, tx)
    pub target: (u128, u128),
    /// Opcode (first input after target in cellpack)
    pub opcode: Option<u128>,
    /// Calldata bytes for this message
    pub calldata: Vec<u8>,
    /// Storage keys read during execution
    pub read_keys: BTreeSet<Vec<u8>>,
    /// Storage keys written during execution
    pub write_keys: BTreeSet<Vec<u8>>,
    /// Whether this message is eligible for GPU execution
    pub gpu_eligible: bool,
}

impl StorageTracker {
    pub fn new(tx_index: usize, msg_index: usize, target: (u128, u128)) -> Self {
        // Only cellpacks targeting existing contracts (block 2 or 4) are GPU-eligible.
        // Block 1 (create), 3 (reserved), 5/6 (deployment) must be sequential.
        let gpu_eligible = target.0 == 2 || target.0 == 4;
        Self {
            tx_index,
            msg_index,
            target,
            opcode: None,
            calldata: Vec::new(),
            read_keys: BTreeSet::new(),
            write_keys: BTreeSet::new(),
            gpu_eligible,
        }
    }

    /// Record a storage read.
    pub fn record_read(&mut self, key: Vec<u8>) {
        self.read_keys.insert(key);
    }

    /// Record a storage write.
    pub fn record_write(&mut self, key: Vec<u8>) {
        self.write_keys.insert(key);
    }

    /// Check if this message conflicts with another.
    ///
    /// Conflicts:
    ///   - Write-After-Write (WAW): both write the same key
    ///   - Read-After-Write (RAW): one reads what the other writes
    ///   - Write-After-Read (WAR): one writes what the other reads
    ///   - Same transaction: always conflicts (ordering matters)
    pub fn conflicts_with(&self, other: &StorageTracker) -> bool {
        // Same transaction — must preserve ordering
        if self.tx_index == other.tx_index {
            return true;
        }

        // WAW: both write same key
        if !self.write_keys.is_disjoint(&other.write_keys) {
            return true;
        }

        // RAW: self reads what other writes
        if !self.read_keys.is_disjoint(&other.write_keys) {
            return true;
        }

        // WAR: self writes what other reads
        if !self.write_keys.is_disjoint(&other.read_keys) {
            return true;
        }

        false
    }
}

/// Analyzes dependencies across all messages in a block to find
/// parallel execution groups.
pub struct DependencyAnalyzer {
    trackers: Vec<StorageTracker>,
}

/// Statistics about parallelization potential.
#[derive(Debug, Clone)]
pub struct DependencyStats {
    pub total_messages: usize,
    pub gpu_eligible: usize,
    pub parallel_groups: usize,
    pub largest_group: usize,
    pub total_conflicts: usize,
}

impl DependencyAnalyzer {
    pub fn new() -> Self {
        Self {
            trackers: Vec::new(),
        }
    }

    /// Add a tracked message to the analyzer.
    pub fn add_tracker(&mut self, tracker: StorageTracker) {
        self.trackers.push(tracker);
    }

    /// Compute parallel groups of non-conflicting GPU-eligible messages.
    ///
    /// Uses a greedy graph coloring approach:
    /// 1. Build a conflict graph (adjacency list) among GPU-eligible messages
    /// 2. Find connected components — messages in different components can't conflict
    /// 3. Each connected component becomes a group that must serialize internally
    ///    but the component as a whole runs as one GPU shard
    ///
    /// Returns groups of tracker indices that can execute in parallel.
    pub fn compute_parallel_groups(&self) -> Vec<Vec<usize>> {
        let eligible: Vec<usize> = self
            .trackers
            .iter()
            .enumerate()
            .filter(|(_, t)| t.gpu_eligible)
            .map(|(i, _)| i)
            .collect();

        if eligible.is_empty() {
            return Vec::new();
        }

        // Build conflict adjacency sets among eligible messages
        let n = eligible.len();
        let mut adj: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];

        for i in 0..n {
            for j in (i + 1)..n {
                if self.trackers[eligible[i]].conflicts_with(&self.trackers[eligible[j]]) {
                    adj[i].insert(j);
                    adj[j].insert(i);
                }
            }
        }

        // Find connected components via DFS
        let mut visited = vec![false; n];
        let mut groups: Vec<Vec<usize>> = Vec::new();

        for start in 0..n {
            if visited[start] {
                continue;
            }
            let mut component: Vec<usize> = Vec::new();
            let mut stack = vec![start];
            while let Some(node) = stack.pop() {
                if visited[node] {
                    continue;
                }
                visited[node] = true;
                component.push(eligible[node]);
                for &neighbor in &adj[node] {
                    if !visited[neighbor] {
                        stack.push(neighbor);
                    }
                }
            }
            groups.push(component);
        }

        groups
    }

    /// Get statistics about the parallelization analysis.
    pub fn stats(&self) -> DependencyStats {
        let groups = self.compute_parallel_groups();
        let gpu_eligible = self.trackers.iter().filter(|t| t.gpu_eligible).count();

        // Count conflicts
        let mut total_conflicts = 0;
        for i in 0..self.trackers.len() {
            for j in (i + 1)..self.trackers.len() {
                if self.trackers[i].gpu_eligible
                    && self.trackers[j].gpu_eligible
                    && self.trackers[i].conflicts_with(&self.trackers[j])
                {
                    total_conflicts += 1;
                }
            }
        }

        DependencyStats {
            total_messages: self.trackers.len(),
            gpu_eligible,
            parallel_groups: groups.len(),
            largest_group: groups.iter().map(|g| g.len()).max().unwrap_or(0),
            total_conflicts,
        }
    }

    /// Get a reference to the trackers.
    pub fn trackers(&self) -> &[StorageTracker] {
        &self.trackers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_conflict_different_keys() {
        let mut a = StorageTracker::new(0, 0, (2, 1));
        a.record_read(b"key_a".to_vec());
        a.record_write(b"key_a".to_vec());

        let mut b = StorageTracker::new(1, 0, (2, 2));
        b.record_read(b"key_b".to_vec());
        b.record_write(b"key_b".to_vec());

        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn test_conflict_write_write() {
        let mut a = StorageTracker::new(0, 0, (2, 1));
        a.record_write(b"shared_key".to_vec());

        let mut b = StorageTracker::new(1, 0, (2, 2));
        b.record_write(b"shared_key".to_vec());

        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn test_conflict_read_write() {
        let mut a = StorageTracker::new(0, 0, (2, 1));
        a.record_read(b"shared_key".to_vec());

        let mut b = StorageTracker::new(1, 0, (2, 2));
        b.record_write(b"shared_key".to_vec());

        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn test_conflict_same_tx() {
        let a = StorageTracker::new(0, 0, (2, 1));
        let b = StorageTracker::new(0, 1, (2, 2));
        assert!(a.conflicts_with(&b)); // same tx_index = conflict
    }

    #[test]
    fn test_not_gpu_eligible() {
        let a = StorageTracker::new(0, 0, (1, 0)); // block 1 = create
        assert!(!a.gpu_eligible);

        let b = StorageTracker::new(0, 0, (3, 0)); // block 3 = reserved
        assert!(!b.gpu_eligible);

        let c = StorageTracker::new(0, 0, (2, 5)); // block 2 = eligible
        assert!(c.gpu_eligible);

        let d = StorageTracker::new(0, 0, (4, 5)); // block 4 = eligible
        assert!(d.gpu_eligible);
    }

    #[test]
    fn test_parallel_groups_no_conflicts() {
        let mut analyzer = DependencyAnalyzer::new();

        let mut a = StorageTracker::new(0, 0, (2, 1));
        a.record_write(b"key_a".to_vec());

        let mut b = StorageTracker::new(1, 0, (2, 2));
        b.record_write(b"key_b".to_vec());

        let mut c = StorageTracker::new(2, 0, (2, 3));
        c.record_write(b"key_c".to_vec());

        analyzer.add_tracker(a);
        analyzer.add_tracker(b);
        analyzer.add_tracker(c);

        let groups = analyzer.compute_parallel_groups();
        // 3 independent messages — each in its own group (no conflicts)
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn test_parallel_groups_with_conflict_chain() {
        let mut analyzer = DependencyAnalyzer::new();

        // A writes key_x, B reads key_x and writes key_y, C reads key_y
        // A→B→C conflict chain → all in one group
        let mut a = StorageTracker::new(0, 0, (2, 1));
        a.record_write(b"key_x".to_vec());

        let mut b = StorageTracker::new(1, 0, (2, 2));
        b.record_read(b"key_x".to_vec());
        b.record_write(b"key_y".to_vec());

        let mut c = StorageTracker::new(2, 0, (2, 3));
        c.record_read(b"key_y".to_vec());

        // D is independent
        let mut d = StorageTracker::new(3, 0, (2, 4));
        d.record_write(b"key_z".to_vec());

        analyzer.add_tracker(a);
        analyzer.add_tracker(b);
        analyzer.add_tracker(c);
        analyzer.add_tracker(d);

        let groups = analyzer.compute_parallel_groups();
        // A,B,C in one component; D alone
        assert_eq!(groups.len(), 2);

        let stats = analyzer.stats();
        assert_eq!(stats.total_messages, 4);
        assert_eq!(stats.gpu_eligible, 4);
        assert_eq!(stats.parallel_groups, 2);
        assert_eq!(stats.largest_group, 3);
    }

    #[test]
    fn test_mixed_eligible_ineligible() {
        let mut analyzer = DependencyAnalyzer::new();

        let mut a = StorageTracker::new(0, 0, (2, 1)); // eligible
        a.record_write(b"key_a".to_vec());

        let b = StorageTracker::new(1, 0, (1, 0)); // NOT eligible (create)

        let mut c = StorageTracker::new(2, 0, (4, 5)); // eligible
        c.record_write(b"key_c".to_vec());

        analyzer.add_tracker(a);
        analyzer.add_tracker(b);
        analyzer.add_tracker(c);

        let stats = analyzer.stats();
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.gpu_eligible, 2);
        assert_eq!(stats.parallel_groups, 2); // a and c are independent
    }
}
