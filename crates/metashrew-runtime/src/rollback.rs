//! SMT Rollback Trait and Implementation
//!
//! This module provides a trait for properly rolling back Sparse Merkle Tree (SMT) data
//! during blockchain reorganizations. Both RocksDB and in-memory storage adapters must
//! implement this trait to ensure consistent reorg handling.

use anyhow::Result;
use log::{debug, info, warn};
use crate::smt::{MANIFEST_PREFIX, SMT_ROOT_PREFIX, deserialize_key_manifest};

/// Trait for rolling back SMT data during blockchain reorganizations
pub trait SmtRollback {
    /// Iterate over all keys in storage (streaming, memory-efficient)
    fn iter_keys<F>(&self, callback: F) -> Result<()>
    where
        F: FnMut(&[u8]) -> Result<()>;

    /// Delete a key from storage
    fn delete_key(&mut self, key: &[u8]) -> Result<()>;

    /// Put a key-value pair into storage
    fn put_key(&mut self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Get value for a key
    fn get_value(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
}

/// Parse height from an SMT update key
///
/// SMT keys are stored as: `base_key/index`
/// SMT values are stored as: `height:data`
///
/// This function extracts the height from the value.
fn parse_height_from_smt_value(value: &[u8]) -> Option<u32> {
    let value_str = String::from_utf8_lossy(value);
    if let Some(colon_pos) = value_str.find(':') {
        let height_str = &value_str[..colon_pos];
        height_str.parse::<u32>().ok()
    } else {
        None
    }
}

/// Roll back SMT data to a specific height
///
/// This is the correct implementation that BOTH RocksDB and MemStore should use.
///
/// The rollback process:
/// 1. Delete metadata keys (block_hash_*, state_root_*, smt:root:*) for heights > rollback_height
/// 2. Roll back append-only SMT data structures (keys with /length suffix)
/// 3. This ensures all WASM-indexed data is properly cleaned up during reorgs
pub fn rollback_smt_data<S: SmtRollback>(
    storage: &mut S,
    rollback_height: u32,
    current_height: u32,
) -> Result<()> {
    info!("Starting SMT rollback from height {} to height {}", current_height, rollback_height);

    if rollback_height >= current_height {
        debug!("Rollback height {} >= current height {}, nothing to do", rollback_height, current_height);
        return Ok(());
    }

    // --- Step 1: Collect metadata keys to delete (streaming) ---
    let mut metadata_keys_to_delete = Vec::new();
    storage.iter_keys(|key| {
        let key_str = String::from_utf8_lossy(key);

        // Check for metadata keys and parse their height
        let metadata_height = if let Some(stripped) = key_str.strip_prefix("block_hash_") {
            stripped.parse::<u32>().ok()
        } else if let Some(stripped) = key_str.strip_prefix("state_root_") {
            stripped.parse::<u32>().ok()
        } else if let Some(stripped) = key_str.strip_prefix("smt:root:") {
            stripped.parse::<u32>().ok()
        } else {
            None
        };

        if let Some(h) = metadata_height {
            if h > rollback_height {
                metadata_keys_to_delete.push(key.to_vec());
            }
        }
        Ok(())
    })?;

    // Delete metadata keys
    for key in &metadata_keys_to_delete {
        storage.delete_key(key)?;
    }
    info!("Deleted {} metadata keys", metadata_keys_to_delete.len());

    // --- Step 2: Collect base keys for SMT structures (streaming) ---
    let length_suffix = b"/length";
    let mut base_keys = Vec::new(); // Use Vec instead of HashSet to save memory
    let max_keys_to_collect = 100_000; // Safety limit to prevent OOM

    info!("Scanning for SMT structures to roll back...");
    let mut total_length_keys_found = 0;
    storage.iter_keys(|key| {
        if key.ends_with(length_suffix) {
            total_length_keys_found += 1;
            let base_key = &key[..key.len() - length_suffix.len()];

            // Only collect up to max limit to prevent OOM
            if base_keys.len() < max_keys_to_collect {
                base_keys.push(base_key.to_vec());
            }

            // Log progress every 10000 keys
            if total_length_keys_found % 10000 == 0 {
                info!("Found {} SMT /length keys so far...", total_length_keys_found);
            }
        }
        Ok(())
    })?;

    if total_length_keys_found > max_keys_to_collect {
        warn!("WARNING: Found {} SMT structures, but can only process {} at a time due to memory limits.",
              total_length_keys_found, max_keys_to_collect);
        warn!("Consider implementing multi-pass rollback for very large databases.");
    }

    // Deduplicate and sort for consistent processing
    base_keys.sort_unstable();
    base_keys.dedup();

    info!("Collected {} unique SMT structures to process (found {} total /length keys)",
          base_keys.len(), total_length_keys_found);

    // Process structures in batches to limit memory usage
    let batch_size = 100; // Process 100 structures at a time
    let mut smt_structures_rolled_back = 0;

    for (batch_idx, batch) in base_keys.chunks(batch_size).enumerate() {
        info!("Processing batch {} ({} structures)...", batch_idx + 1, batch.len());

        for base_key in batch {
            let mut length_key = base_key.clone();
            length_key.extend_from_slice(length_suffix);

            // Read the current length
            let old_length = if let Some(length_bytes) = storage.get_value(&length_key)? {
                String::from_utf8_lossy(&length_bytes).parse::<u32>().unwrap_or(0)
            } else {
                continue;
            };

            // Collect valid updates (height <= rollback_height)
            let mut valid_updates = Vec::new();
            for i in 0..old_length {
                let update_key_suffix = format!("/{}", i);
                let mut update_key = base_key.clone();
                update_key.extend_from_slice(update_key_suffix.as_bytes());

                if let Some(update_data) = storage.get_value(&update_key)? {
                    if let Some(update_height) = parse_height_from_smt_value(&update_data) {
                        if update_height <= rollback_height {
                            valid_updates.push((i, update_data));
                        } else {
                            debug!("Removing SMT update at height {} (> {})", update_height, rollback_height);
                        }
                    }
                }
            }

            // Remove all old entries
            for i in 0..old_length {
                let update_key_suffix = format!("/{}", i);
                let mut update_key = base_key.clone();
                update_key.extend_from_slice(update_key_suffix.as_bytes());
                storage.delete_key(&update_key)?;
            }

            // Re-insert valid entries with compacted indices
            for (new_index, (_, update_data)) in valid_updates.iter().enumerate() {
                let update_key_suffix = format!("/{}", new_index);
                let mut update_key = base_key.clone();
                update_key.extend_from_slice(update_key_suffix.as_bytes());
                storage.put_key(&update_key, update_data)?;
            }

            // Update or remove the length key
            let new_length = valid_updates.len() as u32;
            if new_length > 0 {
                storage.put_key(&length_key, new_length.to_string().as_bytes())?;
                debug!("SMT structure {} compacted from {} to {} entries", String::from_utf8_lossy(&base_key), old_length, new_length);
            } else {
                storage.delete_key(&length_key)?;
                debug!("SMT structure {} completely removed (no valid entries)", String::from_utf8_lossy(&base_key));
            }
            smt_structures_rolled_back += 1;

            if smt_structures_rolled_back % 1000 == 0 {
                info!("Rolled back {} SMT structures so far...", smt_structures_rolled_back);
            }
        }
    }

    info!("Successfully rolled back {} SMT data structures to height {}", smt_structures_rolled_back, rollback_height);
    Ok(())
}

/// Fast rollback using per-height manifests.
///
/// Instead of scanning all keys in the database, reads the manifest for each
/// rolled-back height to find exactly which keys were modified, then rolls
/// back only those keys. Falls back to `rollback_smt_data()` if manifests
/// are missing (e.g., blocks indexed before the upgrade).
///
/// Returns Ok(true) if fast rollback succeeded, Ok(false) if fallback is needed.
pub fn rollback_with_manifests<S: SmtRollback>(
    storage: &mut S,
    rollback_height: u32,
    current_height: u32,
) -> Result<bool> {
    if rollback_height >= current_height {
        return Ok(true);
    }

    info!(
        "Attempting fast manifest-based rollback from height {} to {}",
        current_height, rollback_height
    );

    // Check if manifests exist for all heights in the rollback range
    for h in (rollback_height + 1)..=current_height {
        let manifest_key = format!("{}{}", MANIFEST_PREFIX, h).into_bytes();
        if storage.get_value(&manifest_key)?.is_none() {
            warn!(
                "Manifest missing for height {}. Falling back to full rollback.",
                h
            );
            return Ok(false);
        }
    }

    // All manifests exist — do the fast rollback
    let mut total_keys_rolled_back = 0;

    for h in ((rollback_height + 1)..=current_height).rev() {
        let manifest_key = format!("{}{}", MANIFEST_PREFIX, h).into_bytes();
        let manifest_data = storage.get_value(&manifest_key)?.unwrap();
        let keys = deserialize_key_manifest(&manifest_data);

        for key in &keys {
            // For each key modified at this height, trim append-only entries
            // above the rollback height
            let length_key = [key.as_slice(), b"/length"].concat();
            if let Some(length_bytes) = storage.get_value(&length_key)? {
                let length = String::from_utf8_lossy(&length_bytes)
                    .parse::<u32>()
                    .unwrap_or(0);

                // Walk backward from the end to find entries above rollback_height
                let mut new_length = length;
                for i in (0..length).rev() {
                    let update_key =
                        [key.as_slice(), b"/", i.to_string().as_bytes()].concat();
                    if let Some(update_data) = storage.get_value(&update_key)? {
                        if let Some(entry_height) = parse_height_from_smt_value(&update_data) {
                            if entry_height > rollback_height {
                                storage.delete_key(&update_key)?;
                                new_length = i;
                            } else {
                                break; // entries are ordered by height, stop early
                            }
                        }
                    }
                }

                if new_length != length {
                    if new_length > 0 {
                        storage.put_key(&length_key, new_length.to_string().as_bytes())?;
                    } else {
                        storage.delete_key(&length_key)?;
                    }
                }
            }
            total_keys_rolled_back += 1;
        }

        // Delete manifest and metadata for this height
        storage.delete_key(&manifest_key)?;

        let root_key = format!("{}{}",SMT_ROOT_PREFIX, h).into_bytes();
        storage.delete_key(&root_key)?;

        let hash_key = format!("block_hash_{}", h).into_bytes();
        storage.delete_key(&hash_key)?;

        let state_key = format!("state_root_{}", h).into_bytes();
        storage.delete_key(&state_key)?;
    }

    info!(
        "Fast rollback complete: rolled back {} keys across {} heights",
        total_keys_rolled_back,
        current_height - rollback_height
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockStorage {
        data: HashMap<Vec<u8>, Vec<u8>>,
    }

    impl SmtRollback for MockStorage {
        fn iter_keys<F>(&self, mut callback: F) -> Result<()>
        where
            F: FnMut(&[u8]) -> Result<()>,
        {
            for key in self.data.keys() {
                callback(key)?;
            }
            Ok(())
        }

        fn delete_key(&mut self, key: &[u8]) -> Result<()> {
            self.data.remove(key);
            Ok(())
        }

        fn put_key(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
            self.data.insert(key.to_vec(), value.to_vec());
            Ok(())
        }

        fn get_value(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
            Ok(self.data.get(key).cloned())
        }
    }

    #[test]
    fn test_rollback_metadata_keys() {
        let mut storage = MockStorage {
            data: HashMap::new(),
        };

        // Insert metadata for heights 3, 4, 5
        storage.data.insert(b"block_hash_3".to_vec(), b"hash3".to_vec());
        storage.data.insert(b"block_hash_4".to_vec(), b"hash4".to_vec());
        storage.data.insert(b"block_hash_5".to_vec(), b"hash5".to_vec());
        storage.data.insert(b"smt:root:3".to_vec(), b"root3".to_vec());
        storage.data.insert(b"smt:root:4".to_vec(), b"root4".to_vec());
        storage.data.insert(b"smt:root:5".to_vec(), b"root5".to_vec());

        // Rollback to height 3
        rollback_smt_data(&mut storage, 3, 5).unwrap();

        // Heights 4 and 5 should be deleted, height 3 should remain
        assert!(storage.data.contains_key(b"block_hash_3".as_ref()));
        assert!(!storage.data.contains_key(b"block_hash_4".as_ref()));
        assert!(!storage.data.contains_key(b"block_hash_5".as_ref()));
        assert!(storage.data.contains_key(b"smt:root:3".as_ref()));
        assert!(!storage.data.contains_key(b"smt:root:4".as_ref()));
        assert!(!storage.data.contains_key(b"smt:root:5".as_ref()));
    }

    #[test]
    fn test_parse_height_from_smt_value() {
        assert_eq!(parse_height_from_smt_value(b"123:data"), Some(123));
        assert_eq!(parse_height_from_smt_value(b"0:data"), Some(0));
        assert_eq!(parse_height_from_smt_value(b"noheight"), None);
        assert_eq!(parse_height_from_smt_value(b":data"), None);
    }

    #[test]
    fn test_manifest_rollback_basic() {
        use crate::smt::{serialize_key_manifest, MANIFEST_PREFIX, SMT_ROOT_PREFIX};

        let mut storage = MockStorage {
            data: HashMap::new(),
        };

        // Simulate indexing 3 blocks (heights 1, 2, 3)
        // Block 1: modifies key_a, key_b
        storage.data.insert(b"key_a/length".to_vec(), b"1".to_vec());
        storage.data.insert(b"key_a/0".to_vec(), b"1:aa".to_vec());
        storage.data.insert(b"key_b/length".to_vec(), b"1".to_vec());
        storage.data.insert(b"key_b/0".to_vec(), b"1:bb".to_vec());
        let manifest1 = serialize_key_manifest(&[b"key_a", b"key_b"]);
        storage.data.insert(format!("{}1", MANIFEST_PREFIX).into_bytes(), manifest1);
        storage.data.insert(format!("{}1", SMT_ROOT_PREFIX).into_bytes(), b"root1".to_vec());
        storage.data.insert(b"block_hash_1".to_vec(), b"hash1".to_vec());

        // Block 2: modifies key_a, key_c
        storage.data.insert(b"key_a/length".to_vec(), b"2".to_vec());
        storage.data.insert(b"key_a/1".to_vec(), b"2:aa2".to_vec());
        storage.data.insert(b"key_c/length".to_vec(), b"1".to_vec());
        storage.data.insert(b"key_c/0".to_vec(), b"2:cc".to_vec());
        let manifest2 = serialize_key_manifest(&[b"key_a", b"key_c"]);
        storage.data.insert(format!("{}2", MANIFEST_PREFIX).into_bytes(), manifest2);
        storage.data.insert(format!("{}2", SMT_ROOT_PREFIX).into_bytes(), b"root2".to_vec());
        storage.data.insert(b"block_hash_2".to_vec(), b"hash2".to_vec());

        // Block 3: modifies key_b
        storage.data.insert(b"key_b/length".to_vec(), b"2".to_vec());
        storage.data.insert(b"key_b/1".to_vec(), b"3:bb3".to_vec());
        let manifest3 = serialize_key_manifest(&[b"key_b"]);
        storage.data.insert(format!("{}3", MANIFEST_PREFIX).into_bytes(), manifest3);
        storage.data.insert(format!("{}3", SMT_ROOT_PREFIX).into_bytes(), b"root3".to_vec());
        storage.data.insert(b"block_hash_3".to_vec(), b"hash3".to_vec());

        // Rollback to height 1 (undo blocks 2 and 3)
        let result = rollback_with_manifests(&mut storage, 1, 3).unwrap();
        assert!(result, "fast rollback should succeed with manifests");

        // key_a should be back to length 1 (only block 1 entry)
        assert_eq!(storage.data.get(b"key_a/length".as_ref()), Some(&b"1".to_vec()));
        assert!(storage.data.contains_key(b"key_a/0".as_ref())); // block 1 entry kept
        assert!(!storage.data.contains_key(b"key_a/1".as_ref())); // block 2 entry removed

        // key_b should be back to length 1 (block 3 entry removed)
        assert_eq!(storage.data.get(b"key_b/length".as_ref()), Some(&b"1".to_vec()));
        assert!(storage.data.contains_key(b"key_b/0".as_ref())); // block 1 entry kept
        assert!(!storage.data.contains_key(b"key_b/1".as_ref())); // block 3 entry removed

        // key_c should be completely removed (only existed from block 2)
        assert!(!storage.data.contains_key(b"key_c/length".as_ref()));
        assert!(!storage.data.contains_key(b"key_c/0".as_ref()));

        // Metadata for heights 2 and 3 should be gone
        assert!(!storage.data.contains_key(format!("{}2", SMT_ROOT_PREFIX).as_bytes()));
        assert!(!storage.data.contains_key(format!("{}3", SMT_ROOT_PREFIX).as_bytes()));
        assert!(!storage.data.contains_key(b"block_hash_2".as_ref()));
        assert!(!storage.data.contains_key(b"block_hash_3".as_ref()));

        // Height 1 metadata should remain
        assert!(storage.data.contains_key(format!("{}1", SMT_ROOT_PREFIX).as_bytes()));
        assert!(storage.data.contains_key(b"block_hash_1".as_ref()));

        // Manifests for 2 and 3 should be deleted
        assert!(!storage.data.contains_key(format!("{}2", MANIFEST_PREFIX).as_bytes()));
        assert!(!storage.data.contains_key(format!("{}3", MANIFEST_PREFIX).as_bytes()));
    }

    #[test]
    fn test_manifest_rollback_falls_back_without_manifests() {
        let mut storage = MockStorage {
            data: HashMap::new(),
        };

        // No manifests — should return false for fallback
        storage.data.insert(b"block_hash_2".to_vec(), b"hash2".to_vec());
        let result = rollback_with_manifests(&mut storage, 1, 2).unwrap();
        assert!(!result, "should fall back when manifests are missing");
    }
}
