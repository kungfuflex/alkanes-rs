// crates/rockshrew-mono/src/tests/snapshot_sync_test.rs

/*
 * Chadson v69.0.0
 *
 * This file contains the integration tests for snapshot and repo mode synchronization.
 *
 * Purpose:
 * - To verify the correctness of the snapshot-based synchronization mechanism in `rockshrew-mono`.
 * - To simulate real-world scenarios involving two `rockshrew-mono` instances:
 *   1. A "snapshot" instance that acts as the source of truth.
 *   2. A "repo" instance that syncs from the snapshot.
 * - This test performs a direct key-value copy from the underlying RocksDB database
 *   to simulate the transfer of a snapshot's state data.
 *
 * Core Logic:
 * - The tests will use in-memory `RocksDB` instances to simulate the two nodes.
 * - The snapshot instance will be populated with a predefined set of data using SMTHelper.
 * - The raw key-value pairs will be iterated from the snapshot instance's RocksDB.
 * - These key-value pairs will be inserted into the repo instance's RocksDB.
 * - The SMT state root of both instances will be compared to ensure they are identical.
 */

use crate::smt_helper::SMTHelper;
use metashrew_runtime::traits::KeyValueStoreLike;
use rockshrew_runtime::adapter::RocksDBRuntimeAdapter;
use tempfile::tempdir;

#[tokio::test]
async fn test_snapshot_sync_basic() {
    // 1. Setup snapshot instance and populate it with some data
    let snapshot_dir = tempdir().unwrap();
    let snapshot_db =
        RocksDBRuntimeAdapter::open_optimized(snapshot_dir.path().to_str().unwrap().to_string())
            .unwrap();
    let mut snapshot_instance = SMTHelper::new(snapshot_db.clone());

    for i in 0..10u32 {
        snapshot_instance
            .put(&i.to_be_bytes(), &[i as u8; 32], i)
            .unwrap();
    }
    let snapshot_root = snapshot_instance.get_current_state_root().unwrap();

    // 2. Create a "snapshot" by copying all raw key-value pairs from the database
    let iter = snapshot_db.db.iterator(rocksdb::IteratorMode::Start);
    let snapshot_data: Vec<(Box<[u8]>, Box<[u8]>)> = iter.map(|item| item.unwrap()).collect();

    // 3. Setup a new, empty repo instance
    let repo_dir = tempdir().unwrap();
    let mut repo_db =
        RocksDBRuntimeAdapter::open_optimized(repo_dir.path().to_str().unwrap().to_string())
            .unwrap();

    // 4. "Sync" the repo instance by applying the snapshot data
    for (key, value) in snapshot_data {
        repo_db.put(&key, &value).unwrap();
    }

    // 5. Verify the state of the repo instance matches the original
    let repo_instance = SMTHelper::new(repo_db.clone());
    let repo_root = repo_instance.get_current_state_root().unwrap();

    assert_eq!(
        snapshot_root, repo_root,
        "Synced state root should match snapshot state root"
    );
}
