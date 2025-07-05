#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gpu_tracking::*;
    use bitcoin::hashes::Hash;
    use bitcoin::{Transaction, TxIn, TxOut, OutPoint, ScriptBuf};
    use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
    use metashrew_support::KeyValuePointer;
    use protorune_support::protostone::Protostone;

    fn create_test_transaction(txid_bytes: [u8; 32]) -> Transaction {
        Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ZERO,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![TxOut {
                value: bitcoin::Amount::from_sat(1000),
                script_pubkey: ScriptBuf::new(),
            }],
        }
    }

    #[test]
    fn test_tracked_atomic_pointer() {
        let atomic = AtomicPointer::default();
        let tracker = StorageTracker::new(
            bitcoin::Txid::from_byte_array([1; 32]),
            0,
            0,
        );
        let mut tracked = TrackedAtomicPointer::new(atomic, tracker);

        // Test that operations are tracked
        let test_key = IndexPointer::wrap(&vec![1, 2, 3]);
        let mut derived = tracked.derive(&test_key);
        
        // This should record a read operation
        let _value = derived.get();
        
        // This should record a write operation
        derived.set(std::sync::Arc::new(vec![4, 5, 6]));

        // Verify tracker recorded the operations
        let final_tracker = tracked.get_tracker().unwrap();
        assert!(final_tracker.read_slots.contains(&vec![1, 2, 3]));
        assert!(final_tracker.write_slots.contains(&vec![1, 2, 3]));
    }

    #[test]
    fn test_cellpack_parallelization_detection() {
        use protorune_support::utils::encode_varint_list;
        
        let txid = bitcoin::Txid::from_byte_array([1; 32]);

        // Test cellpack starting with 2 (parallelizable)
        let cellpack_2_data = encode_varint_list(&vec![2u128, 100u128, 77u128]);
        let tracker = StorageTracker::from_calldata(txid, 0, 0, &cellpack_2_data, 50).unwrap();
        assert!(tracker.is_parallelizable);
        assert!(!tracker.creates_new_alkane);

        // Test cellpack starting with 4 (parallelizable)
        let cellpack_4_data = encode_varint_list(&vec![4u128, 200u128, 88u128]);
        let tracker = StorageTracker::from_calldata(txid, 0, 0, &cellpack_4_data, 50).unwrap();
        assert!(tracker.is_parallelizable);
        assert!(!tracker.creates_new_alkane);

        // Test cellpack starting with 1 (not parallelizable)
        let cellpack_1_data = encode_varint_list(&vec![1u128, 100u128, 77u128]);
        let tracker = StorageTracker::from_calldata(txid, 0, 0, &cellpack_1_data, 50).unwrap();
        assert!(!tracker.is_parallelizable);

        // Test new alkane creation (not parallelizable)
        let cellpack_new_data = encode_varint_list(&vec![2u128, 100u128, 77u128]);
        let tracker = StorageTracker::from_calldata(txid, 0, 0, &cellpack_new_data, 2).unwrap(); // height = target block
        assert!(!tracker.is_parallelizable);
        assert!(tracker.creates_new_alkane);
    }

    #[test]
    fn test_storage_profile_generation() {
        use protorune_support::utils::encode_varint_list;
        
        let txid = bitcoin::Txid::from_byte_array([1; 32]);
        let cellpack_data = encode_varint_list(&vec![2u128, 100u128, 77u128]);
        let tracker = StorageTracker::from_calldata(txid, 0, 0, &cellpack_data, 50).unwrap();
        
        // Should have a storage profile key
        let profile_key = tracker.get_storage_profile_key();
        assert!(profile_key.is_some());
        
        let key = profile_key.unwrap();
        assert!(key.starts_with("/storagepaths/"));
        assert!(key.contains("/77")); // opcode
    }

    #[test]
    fn test_dependency_analysis() {
        let mut analyzer = DependencyAnalyzer::new(100);

        let txid1 = bitcoin::Txid::from_byte_array([1; 32]);
        let txid2 = bitcoin::Txid::from_byte_array([2; 32]);
        let txid3 = bitcoin::Txid::from_byte_array([3; 32]);

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
    fn test_conflicting_transactions() {
        let mut analyzer = DependencyAnalyzer::new(100);

        let txid1 = bitcoin::Txid::from_byte_array([1; 32]);
        let txid2 = bitcoin::Txid::from_byte_array([2; 32]);

        let mut tracker1 = StorageTracker::new(txid1, 0, 0);
        let mut tracker2 = StorageTracker::new(txid2, 1, 0);

        // Mark as parallelizable
        tracker1.is_parallelizable = true;
        tracker2.is_parallelizable = true;

        // Set up conflicting operations (write-read conflict)
        tracker1.record_write(b"shared_key");
        tracker2.record_read(b"shared_key");

        analyzer.add_tracker(tracker1);
        analyzer.add_tracker(tracker2);

        analyzer.filter_parallelizable();
        analyzer.build_conflict_matrix();
        let groups = analyzer.find_parallel_groups();

        // Should be in separate groups due to conflict
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 1);
        assert_eq!(groups[1].len(), 1);
    }

    #[test]
    fn test_global_dependency_tracking() {
        // Initialize global tracking with height
        init_block_dependency_tracking(100);

        let txid1 = bitcoin::Txid::from_byte_array([1; 32]);
        let txid2 = bitcoin::Txid::from_byte_array([2; 32]);

        let mut tracker1 = StorageTracker::new(txid1, 0, 0);
        let mut tracker2 = StorageTracker::new(txid2, 1, 0);

        // Mark as parallelizable
        tracker1.is_parallelizable = true;
        tracker2.is_parallelizable = true;

        tracker1.record_read(b"key1");
        tracker2.record_read(b"key2");

        // Add trackers to global analysis
        add_tracker_to_block(tracker1).unwrap();
        add_tracker_to_block(tracker2).unwrap();

        // Get analysis results
        let stats = get_block_dependency_analysis().unwrap();
        assert_eq!(stats.total_trackers, 2);
        assert_eq!(stats.parallelizable_trackers, 2);
        assert_eq!(stats.total_conflicts, 0);
        assert_eq!(stats.parallel_groups, 1);

        // Clean up
        clear_block_dependency_tracking();
    }
}