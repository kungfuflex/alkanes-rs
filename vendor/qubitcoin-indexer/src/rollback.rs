//! Rollback logic for append-only indexer state.
//!
//! Two rollback strategies:
//!
//! 1. **Deferred (zero-mutation):** Sets a reorg marker and deletes only
//!    metadata. The service stays live — reads transparently filter orphaned
//!    entries using canonical block hash validation. Background pruning
//!    cleans up later. Takes <100ms.
//!
//! 2. **Immediate (keyset-based):** Uses per-height key sets to surgically
//!    delete entries above the target height. Used for explicit admin
//!    rollback when the indexer is paused.
//!
//! Falls back to a full DB scan for legacy data without key sets.

use crate::state;
use crate::storage::IndexerStorage;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Deferred rollback (zero-mutation, service stays live)
// ---------------------------------------------------------------------------

/// Deferred rollback: set a reorg marker and delete only metadata.
///
/// The service stays responsive — `get_latest_canonical()` transparently
/// skips orphaned entries above the reorg marker. Background pruning
/// (`prune_orphaned`) can clean up the stale data later.
///
/// Returns the number of metadata keys deleted (block hashes, state roots,
/// keysets for heights above target).
pub fn rollback_deferred(storage: &IndexerStorage, target_height: u32) -> Result<u32, String> {
    let current_tip = storage.tip_height();
    if current_tip <= target_height {
        return Ok(0);
    }

    let mut batch = rocksdb::WriteBatch::default();
    let mut deleted = 0u32;

    // Set the reorg marker (uses min with existing for cascading reorgs).
    let reorg_h = target_height + 1;
    let effective = storage
        .reorg_height()
        .map(|existing| existing.min(reorg_h))
        .unwrap_or(reorg_h);
    batch.put(state::REORG_HEIGHT_KEY, &effective.to_le_bytes());

    // Delete metadata for heights above target (but keep keysets for prune_orphaned).
    for h in (target_height + 1)..=current_tip {
        // Delete canonical hash mapping (will be re-written on new chain).
        batch.delete(state::height_to_hash_key(h));
        // NOTE: keysets are intentionally kept for prune_orphaned() to use.
        deleted += 1;
    }

    // Reset tip height.
    batch.put(state::HEIGHT_KEY, &target_height.to_le_bytes());

    storage.write_raw_batch(batch)?;
    Ok(deleted)
}

/// Background prune: delete orphaned entries above the reorg marker.
///
/// Walks keysets for heights >= reorg_height and deletes non-canonical
/// entries. Clears the reorg marker when done.
///
/// Designed to be called from a spawned async task so the service
/// continues serving during cleanup.
pub fn prune_orphaned(storage: &IndexerStorage) -> Result<u32, String> {
    let reorg_h = match storage.reorg_height() {
        Some(h) => h,
        None => return Ok(0), // no reorg pending
    };

    let tip = storage.tip_height();
    let mut deleted = 0u32;
    let mut batch = rocksdb::WriteBatch::default();
    let mut length_cache: HashMap<Vec<u8>, u32> = HashMap::new();

    // Walk from reorg_height upward to find orphaned entries.
    // We scan keysets that may still be in the DB (they might have been
    // deleted by rollback_deferred, or they might exist for heights that
    // were re-indexed after the reorg).
    for h in reorg_h..=tip.max(reorg_h + 10000) {
        let keyset_key = state::height_keyset_key(h);
        if let Some(keyset_data) = storage.get(&keyset_key) {
            let keys = state::decode_key_set(&keyset_data);
            for key in &keys {
                // Check if this entry is canonical.
                let current_len = length_cache
                    .get(key)
                    .copied()
                    .unwrap_or_else(|| storage.get_length(key));
                if current_len == 0 {
                    continue;
                }

                let idx = current_len - 1;
                let h_key = state::entry_height_key(key, idx);
                if let Some(h_data) = storage.get(&h_key) {
                    if h_data.len() >= 4 {
                        let entry_height =
                            u32::from_le_bytes([h_data[0], h_data[1], h_data[2], h_data[3]]);
                        if entry_height >= reorg_h {
                            // Check canonicity via blockhash8.
                            let is_canonical = if h_data.len() >= 12 {
                                let entry_hash8 = &h_data[4..12];
                                storage
                                    .get_canonical_hash(entry_height)
                                    .map(|ch| ch.len() >= 8 && &ch[..8] == entry_hash8)
                                    .unwrap_or(false)
                            } else {
                                true // old format, assume canonical
                            };

                            if !is_canonical {
                                batch.delete(state::index_key(key, idx));
                                batch.delete(&h_key);
                                let len_key = state::length_key(key);
                                batch.put(&len_key, &idx.to_le_bytes());
                                length_cache.insert(key.clone(), idx);
                                deleted += 1;
                            }
                        }
                    }
                }
            }
            // Clean up the keyset after processing.
            batch.delete(&keyset_key);
        }
    }

    // Clear the reorg marker.
    batch.delete(state::REORG_HEIGHT_KEY);

    storage.write_raw_batch(batch)?;
    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Immediate rollback (keyset-based, for admin operations)
// ---------------------------------------------------------------------------

/// Roll back `storage` so that no entries above `target_height` remain.
///
/// Prefers the fast path via per-height key sets. Falls back to a full
/// DB scan for heights where no key set exists (legacy data).
pub fn rollback_to_height(storage: &IndexerStorage, target_height: u32) -> Result<u32, String> {
    let current_tip = storage.tip_height();
    if current_tip <= target_height {
        storage.set_tip_height(target_height)?;
        return Ok(0);
    }

    let mut deleted = 0u32;
    let mut batch = rocksdb::WriteBatch::default();
    let mut needs_legacy_scan = false;
    // Track in-flight length changes (same key may be modified at multiple heights).
    let mut length_cache: HashMap<Vec<u8>, u32> = HashMap::new();

    // Walk from current tip down to target+1, using per-height key sets.
    for h in (target_height + 1..=current_tip).rev() {
        let keyset_key = state::height_keyset_key(h);
        if let Some(keyset_data) = storage.get(&keyset_key) {
            let keys = state::decode_key_set(&keyset_data);
            for key in &keys {
                let current_len = length_cache
                    .get(key)
                    .copied()
                    .unwrap_or_else(|| storage.get_length(key));
                if current_len > 0 {
                    let idx = current_len - 1;
                    batch.delete(state::index_key(key, idx));
                    batch.delete(state::entry_height_key(key, idx));
                    let len_key = state::length_key(key);
                    batch.put(&len_key, &idx.to_le_bytes());
                    length_cache.insert(key.clone(), idx);
                    deleted += 1;
                }
            }
            batch.delete(&keyset_key);
        } else {
            // No keyset for this height — need legacy full-scan fallback.
            needs_legacy_scan = true;
            break;
        }
    }

    // Commit what we have so far.
    batch.put(state::HEIGHT_KEY, &target_height.to_le_bytes());
    storage.write_raw_batch(batch)?;

    // If any heights lacked key set data, fall back to legacy scan.
    if needs_legacy_scan {
        let legacy_deleted = rollback_legacy(storage, target_height)?;
        deleted += legacy_deleted;
    }

    Ok(deleted)
}

/// Legacy rollback: full DB scan for entries above `target_height`.
///
/// This is the slow path used for data written before per-height key sets
/// were introduced. Scans all length sentinels and walks entries backwards.
fn rollback_legacy(storage: &IndexerStorage, target_height: u32) -> Result<u32, String> {
    let mut deleted = 0u32;
    let length_sentinel = u32::MAX.to_le_bytes();
    let iter = storage.raw_iterator();

    let mut length_keys: Vec<(Vec<u8>, u32)> = Vec::new();

    for item in iter {
        let (key, value) = item.map_err(|e| format!("iterator error: {}", e))?;
        if key.len() >= 4 && key.ends_with(&length_sentinel) {
            if value.len() >= 4 {
                let len = u32::from_le_bytes([value[0], value[1], value[2], value[3]]);
                let base_key = key[..key.len() - 4].to_vec();
                length_keys.push((base_key, len));
            }
        }
    }

    for (base_key, len) in &length_keys {
        let mut new_len = *len;
        let mut keys_to_delete = Vec::new();

        for idx in (0..*len).rev() {
            let h_key = state::entry_height_key(&base_key, idx);
            if let Some(h_data) = storage.get(&h_key) {
                if h_data.len() >= 4 {
                    let entry_height =
                        u32::from_le_bytes([h_data[0], h_data[1], h_data[2], h_data[3]]);
                    if entry_height > target_height {
                        keys_to_delete.push(state::index_key(&base_key, idx));
                        keys_to_delete.push(h_key);
                        new_len = idx;
                        deleted += 1;
                    } else {
                        break;
                    }
                }
            }
        }

        if !keys_to_delete.is_empty() {
            storage.delete_batch(&keys_to_delete)?;
            let len_key = state::length_key(&base_key);
            storage.put(&len_key, &new_len.to_le_bytes())?;
        }
    }

    storage.set_tip_height(target_height)?;
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::IndexerStorage;

    fn temp_storage() -> (IndexerStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = IndexerStorage::open(dir.path()).unwrap();
        (storage, dir)
    }

    #[test]
    fn test_rollback_empty_db() {
        let (storage, _dir) = temp_storage();
        let deleted = rollback_to_height(&storage, 100).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(storage.tip_height(), 100);
    }

    #[test]
    fn test_rollback_with_keyset_fast_path() {
        let (storage, _dir) = temp_storage();

        // Use append_batch which writes per-height key sets.
        storage
            .append_batch(&[(b"counter".to_vec(), b"val_100".to_vec())], 100)
            .unwrap();
        storage
            .append_batch(&[(b"counter".to_vec(), b"val_200".to_vec())], 200)
            .unwrap();
        storage
            .append_batch(&[(b"counter".to_vec(), b"val_300".to_vec())], 300)
            .unwrap();

        assert_eq!(storage.get_length(b"counter"), 3);

        // Roll back to 150 — removes entries at 200 and 300.
        let deleted = rollback_to_height(&storage, 150).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(storage.get_length(b"counter"), 1);
        assert_eq!(
            storage.get_latest(b"counter"),
            Some(b"val_100".to_vec())
        );
        assert_eq!(storage.tip_height(), 150);
    }

    #[test]
    fn test_rollback_legacy_fallback() {
        let (storage, _dir) = temp_storage();

        // Use individual append (no keyset written — simulates legacy data).
        storage.append(b"counter", b"val_100", 100).unwrap();
        storage.append(b"counter", b"val_200", 200).unwrap();
        storage.append(b"counter", b"val_300", 300).unwrap();
        storage.set_tip_height(300).unwrap();

        let deleted = rollback_to_height(&storage, 150).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(storage.get_length(b"counter"), 1);
        assert_eq!(
            storage.get_latest(b"counter"),
            Some(b"val_100".to_vec())
        );
    }

    #[test]
    fn test_rollback_keeps_entries_at_target() {
        let (storage, _dir) = temp_storage();

        storage
            .append_batch(&[(b"key".to_vec(), b"val_10".to_vec())], 10)
            .unwrap();
        storage
            .append_batch(&[(b"key".to_vec(), b"val_20".to_vec())], 20)
            .unwrap();

        let deleted = rollback_to_height(&storage, 20).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(storage.get_length(b"key"), 2);
    }

    #[test]
    fn test_rollback_multiple_keys() {
        let (storage, _dir) = temp_storage();

        storage
            .append_batch(
                &[
                    (b"alpha".to_vec(), b"a1".to_vec()),
                    (b"beta".to_vec(), b"b1".to_vec()),
                ],
                10,
            )
            .unwrap();
        storage
            .append_batch(
                &[
                    (b"alpha".to_vec(), b"a2".to_vec()),
                    (b"beta".to_vec(), b"b2".to_vec()),
                ],
                25,
            )
            .unwrap();

        let deleted = rollback_to_height(&storage, 18).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(storage.get_length(b"alpha"), 1);
        assert_eq!(storage.get_latest(b"alpha"), Some(b"a1".to_vec()));
        assert_eq!(storage.get_length(b"beta"), 1);
        assert_eq!(storage.get_latest(b"beta"), Some(b"b1".to_vec()));
    }

    #[test]
    fn test_rollback_to_zero() {
        let (storage, _dir) = temp_storage();

        storage
            .append_batch(&[(b"data".to_vec(), b"v1".to_vec())], 1)
            .unwrap();
        storage
            .append_batch(&[(b"data".to_vec(), b"v2".to_vec())], 2)
            .unwrap();

        let deleted = rollback_to_height(&storage, 0).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(storage.get_length(b"data"), 0);
        assert_eq!(storage.get_latest(b"data"), None);
        assert_eq!(storage.tip_height(), 0);
    }

    #[test]
    fn test_rollback_noop_when_below_target() {
        let (storage, _dir) = temp_storage();

        storage
            .append_batch(&[(b"x".to_vec(), b"v".to_vec())], 50)
            .unwrap();

        let deleted = rollback_to_height(&storage, 100).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(storage.get_length(b"x"), 1);
    }

    // -- Deferred rollback tests -------------------------------------------

    #[test]
    fn test_deferred_rollback_sets_reorg_marker() {
        let (storage, _dir) = temp_storage();

        let hash8 = b"12345678";
        storage
            .append_batch_with_hash(
                &[(b"key".to_vec(), b"v1".to_vec())],
                10,
                Some(hash8),
            )
            .unwrap();
        storage
            .append_batch_with_hash(
                &[(b"key".to_vec(), b"v2".to_vec())],
                20,
                Some(b"abcdefgh"),
            )
            .unwrap();

        // Deferred rollback to height 15.
        let deleted = rollback_deferred(&storage, 15).unwrap();
        assert!(deleted > 0);

        // Reorg marker should be set.
        assert_eq!(storage.reorg_height(), Some(16)); // target + 1

        // Tip rolled back.
        assert_eq!(storage.tip_height(), 15);

        // Data is still physically present (not mutated).
        assert_eq!(storage.get_length(b"key"), 2);
    }

    #[test]
    fn test_deferred_rollback_cascading_uses_min() {
        let (storage, _dir) = temp_storage();

        storage
            .append_batch(&[(b"k".to_vec(), b"v".to_vec())], 100)
            .unwrap();

        // First reorg to height 80.
        rollback_deferred(&storage, 80).unwrap();
        assert_eq!(storage.reorg_height(), Some(81));

        // Second reorg to height 90 — should keep 81 (min).
        storage.set_tip_height(100).unwrap(); // simulate re-indexing
        rollback_deferred(&storage, 90).unwrap();
        assert_eq!(storage.reorg_height(), Some(81)); // min(81, 91)

        // Third reorg to height 70 — should update to 71.
        storage.set_tip_height(100).unwrap();
        rollback_deferred(&storage, 70).unwrap();
        assert_eq!(storage.reorg_height(), Some(71)); // min(81, 71)
    }

    #[test]
    fn test_reorg_aware_read_filters_orphaned() {
        let (storage, _dir) = temp_storage();

        let hash_a = b"AAAAAAAA";
        let hash_b = b"BBBBBBBB";

        // Block 10 with hash A.
        storage
            .append_batch_with_hash(
                &[(b"counter".to_vec(), b"val_10".to_vec())],
                10,
                Some(hash_a),
            )
            .unwrap();

        // Block 20 with hash B.
        storage
            .append_batch_with_hash(
                &[(b"counter".to_vec(), b"val_20".to_vec())],
                20,
                Some(hash_b),
            )
            .unwrap();

        // Without reorg, latest is val_20.
        assert_eq!(
            storage.get_latest_canonical(b"counter"),
            Some(b"val_20".to_vec())
        );

        // Deferred rollback to 15: marks height >= 16 as suspect.
        rollback_deferred(&storage, 15).unwrap();

        // Now set canonical hash for height 20 to something DIFFERENT from hash_b.
        // This simulates the new chain having a different block at height 20.
        storage
            .set_canonical_hash(20, b"CCCCCCCC")
            .unwrap();

        // Reorg-aware read should skip the orphaned val_20 and return val_10.
        assert_eq!(
            storage.get_latest_canonical(b"counter"),
            Some(b"val_10".to_vec())
        );
    }

    // -- prune_orphaned tests ----------------------------------------------

    #[test]
    fn test_prune_orphaned_no_reorg_pending() {
        let (storage, _dir) = temp_storage();
        storage.append(b"key", b"val", 10).unwrap();
        let pruned = prune_orphaned(&storage).unwrap();
        assert_eq!(pruned, 0);
    }

    #[test]
    fn test_prune_orphaned_clears_reorg_marker() {
        let (storage, _dir) = temp_storage();

        storage
            .append_batch_with_hash(&[(b"k".to_vec(), b"v1".to_vec())], 10, Some(b"AAAAAAAA"))
            .unwrap();
        storage
            .append_batch_with_hash(&[(b"k".to_vec(), b"v2".to_vec())], 20, Some(b"BBBBBBBB"))
            .unwrap();

        // Deferred rollback to 15.
        rollback_deferred(&storage, 15).unwrap();
        assert!(storage.reorg_height().is_some());

        // Set canonical hash for height 20 to a DIFFERENT hash.
        storage.set_canonical_hash(20, b"CCCCCCCC").unwrap();

        // Prune should delete the non-canonical entry and clear the marker.
        let pruned = prune_orphaned(&storage).unwrap();
        assert!(pruned > 0);
        assert!(storage.reorg_height().is_none());
    }

    #[test]
    fn test_deferred_rollback_empty_db() {
        let (storage, _dir) = temp_storage();
        let deleted = rollback_deferred(&storage, 100).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_full_workflow_deferred_rollback_reindex_prune() {
        let (storage, _dir) = temp_storage();

        // Index blocks 10, 20, 30.
        storage
            .append_batch_with_hash(&[(b"x".to_vec(), b"a".to_vec())], 10, Some(b"11111111"))
            .unwrap();
        storage
            .append_batch_with_hash(&[(b"x".to_vec(), b"b".to_vec())], 20, Some(b"22222222"))
            .unwrap();
        storage
            .append_batch_with_hash(&[(b"x".to_vec(), b"c".to_vec())], 30, Some(b"33333333"))
            .unwrap();

        assert_eq!(storage.get_length(b"x"), 3);

        // Deferred rollback to 15 — data stays, marker set.
        rollback_deferred(&storage, 15).unwrap();
        assert_eq!(storage.reorg_height(), Some(16));
        assert_eq!(storage.tip_height(), 15);
        // Physical data still present.
        assert_eq!(storage.get_length(b"x"), 3);

        // Reorg-aware read should return "a" (entries at 20, 30 are orphaned).
        // Set canonical hashes to different values so they fail validation.
        storage.set_canonical_hash(20, b"ZZZZZZZZ").unwrap();
        storage.set_canonical_hash(30, b"YYYYYYYY").unwrap();
        assert_eq!(
            storage.get_latest_canonical(b"x"),
            Some(b"a".to_vec())
        );

        // Background prune cleans up.
        let pruned = prune_orphaned(&storage).unwrap();
        assert!(pruned >= 1);
        assert!(storage.reorg_height().is_none());
    }
}
