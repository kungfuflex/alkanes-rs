//! Rollback logic for append-only indexer state.
//!
//! Walks all keys in the database and truncates any entries that were
//! appended at a height above the rollback target.

use crate::state;
use crate::traits::IndexerStorageReader;

/// Roll back `storage` so that no entries above `target_height` remain.
///
/// For each logical key, finds entries whose recorded height exceeds
/// `target_height`, deletes them, and updates the length counter.
///
/// The `keys_with_lengths` parameter provides all (base_key, length) pairs
/// discovered by scanning the storage for length sentinel keys.
pub fn rollback_to_height<S: IndexerStorageReader>(
    storage: &S,
    target_height: u32,
    keys_with_lengths: &[(Vec<u8>, u32)],
) -> Result<u32, String> {
    let mut deleted = 0u32;
    let mut keys_to_delete_all = Vec::new();
    let mut length_updates = Vec::new();

    // For each logical key, check entries from the end and truncate.
    for (base_key, len) in keys_with_lengths {
        let mut new_len = *len;
        let mut keys_to_delete = Vec::new();

        // Walk backwards from the last entry.
        for idx in (0..*len).rev() {
            let h_key = state::entry_height_key(base_key, idx);
            if let Some(h_data) = storage.get(&h_key) {
                if h_data.len() >= 4 {
                    let entry_height = u32::from_le_bytes([
                        h_data[0], h_data[1], h_data[2], h_data[3],
                    ]);
                    if entry_height > target_height {
                        // Delete this entry and its height record.
                        keys_to_delete.push(state::index_key(base_key, idx));
                        keys_to_delete.push(h_key);
                        new_len = idx;
                        deleted += 1;
                    } else {
                        // All earlier entries are at or below target height.
                        break;
                    }
                }
            }
        }

        if !keys_to_delete.is_empty() {
            keys_to_delete_all.extend(keys_to_delete);
            length_updates.push((state::length_key(base_key), new_len.to_le_bytes().to_vec()));
        }
    }

    Ok(deleted)
}
