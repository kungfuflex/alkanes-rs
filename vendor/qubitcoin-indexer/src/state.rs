//! Append-only key-value state layout compatible with metashrew.
//!
//! Matches the exact key layout used by metashrew-runtime so that
//! existing WASM indexer modules work without modification.
//!
//! Layout (from metashrew-runtime key_utils / runtime.rs):
//!   key ++ u32::MAX_le  -> length (u32 LE count of entries)
//!   key ++ index_le32   -> value at index (0-based)
//!
//! Height tracking:
//!   __HEIGHT__  -> u32 LE (current indexer tip height)

/// The key used to store the current indexer tip height.
pub const HEIGHT_KEY: &[u8] = b"__HEIGHT__";

/// The key used to store the WASM binary hash for integrity checks.
pub const WASM_HASH_KEY: &[u8] = b"__WASM_HASH__";

/// Build the key for the length counter: `key ++ u32::MAX_le`.
///
/// Matches `db_make_length_key` in metashrew-runtime which appends
/// `u32::MAX` in little-endian as the length sentinel.
pub fn length_key(key: &[u8]) -> Vec<u8> {
    let mut k = Vec::with_capacity(key.len() + 4);
    k.extend_from_slice(key);
    k.extend_from_slice(&u32::MAX.to_le_bytes());
    k
}

/// Build the key for entry N: `key ++ N_le32`.
///
/// Matches `db_make_list_key` in metashrew-runtime which appends
/// the index as u32 little-endian.
pub fn index_key(key: &[u8], index: u32) -> Vec<u8> {
    let mut k = Vec::with_capacity(key.len() + 4);
    k.extend_from_slice(key);
    k.extend_from_slice(&index.to_le_bytes());
    k
}

/// Build the key that records at which height an entry was appended.
/// Used for rollback: `key ++ "/__height__/" ++ N_le32`.
pub fn entry_height_key(key: &[u8], index: u32) -> Vec<u8> {
    let mut k = Vec::with_capacity(key.len() + 12 + 4);
    k.extend_from_slice(key);
    k.extend_from_slice(b"/__height__/");
    k.extend_from_slice(&index.to_le_bytes());
    k
}

/// Reorg marker: the height at or above which entries may be orphaned.
/// When set, read operations validate entries against canonical block hashes.
pub const REORG_HEIGHT_KEY: &[u8] = b"__REORG_HEIGHT__";

/// Prefix for canonical block hash mapping: `"__H2H__/" ++ height_le32` → `hash[0..8]`.
/// Used during deferred rollback to distinguish canonical vs orphaned entries.
pub const HEIGHT_TO_HASH_PREFIX: &[u8] = b"__H2H__/";

/// Build the key for canonical block hash at a height.
pub fn height_to_hash_key(height: u32) -> Vec<u8> {
    let mut k = Vec::with_capacity(HEIGHT_TO_HASH_PREFIX.len() + 4);
    k.extend_from_slice(HEIGHT_TO_HASH_PREFIX);
    k.extend_from_slice(&height.to_le_bytes());
    k
}

/// Key for the set of logical keys modified at a given height.
/// Format: `"__keyset__/" ++ height_le32`
///
/// Used by `append_batch` to record which keys changed at each height,
/// enabling O(K) rollback (where K = keys modified) instead of full DB scan.
pub fn height_keyset_key(height: u32) -> Vec<u8> {
    let mut k = Vec::with_capacity(11 + 4);
    k.extend_from_slice(b"__keyset__/");
    k.extend_from_slice(&height.to_le_bytes());
    k
}

/// Encode a list of keys as `[count_u32_le, len1_u32_le, key1, len2_u32_le, key2, ...]`.
pub fn encode_key_set(keys: &[Vec<u8>]) -> Vec<u8> {
    let total: usize = 4 + keys.iter().map(|k| 4 + k.len()).sum::<usize>();
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(&(keys.len() as u32).to_le_bytes());
    for key in keys {
        buf.extend_from_slice(&(key.len() as u32).to_le_bytes());
        buf.extend_from_slice(key);
    }
    buf
}

/// Decode a key set produced by [`encode_key_set`].
pub fn decode_key_set(data: &[u8]) -> Vec<Vec<u8>> {
    if data.len() < 4 {
        return vec![];
    }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut keys = Vec::with_capacity(count);
    let mut pos = 4;
    for _ in 0..count {
        if pos + 4 > data.len() {
            break;
        }
        let len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
            as usize;
        pos += 4;
        if pos + len > data.len() {
            break;
        }
        keys.push(data[pos..pos + len].to_vec());
        pos += len;
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_length_key_uses_u32_max() {
        let key = b"mykey";
        let result = length_key(key);
        let mut expected = b"mykey".to_vec();
        expected.extend_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(result, expected);
        assert_eq!(result.len(), 5 + 4); // key + 4 bytes
    }

    #[test]
    fn test_length_key_empty() {
        let result = length_key(b"");
        assert_eq!(result, u32::MAX.to_le_bytes().to_vec());
    }

    #[test]
    fn test_index_key() {
        let key = b"mykey";
        let result = index_key(key, 0);
        let mut expected = b"mykey".to_vec();
        expected.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(result, expected);
    }

    #[test]
    fn test_index_key_nonzero() {
        let key = b"foo";
        let result = index_key(key, 42);
        let mut expected = b"foo".to_vec();
        expected.extend_from_slice(&42u32.to_le_bytes());
        assert_eq!(result, expected);
    }

    #[test]
    fn test_entry_height_key() {
        let key = b"mykey";
        let result = entry_height_key(key, 5);
        let mut expected = b"mykey/__height__/".to_vec();
        expected.extend_from_slice(&5u32.to_le_bytes());
        assert_eq!(result, expected);
    }

    #[test]
    fn test_length_and_index_keys_distinct() {
        // length key uses u32::MAX, index keys use 0..u32::MAX-1,
        // so they should never collide.
        let key = b"test";
        let lk = length_key(key);
        let ik = index_key(key, 0);
        assert_ne!(lk, ik);
        // Even the highest valid index should differ from the length key.
        let ik_max = index_key(key, u32::MAX - 1);
        assert_ne!(lk, ik_max);
    }

    #[test]
    fn test_constants() {
        assert_eq!(HEIGHT_KEY, b"__HEIGHT__");
        assert_eq!(WASM_HASH_KEY, b"__WASM_HASH__");
    }

    #[test]
    fn test_height_keyset_key() {
        let k = height_keyset_key(42);
        assert!(k.starts_with(b"__keyset__/"));
        assert_eq!(&k[11..], &42u32.to_le_bytes());
    }

    #[test]
    fn test_encode_decode_key_set_roundtrip() {
        let keys = vec![b"alpha".to_vec(), b"beta".to_vec(), b"gamma".to_vec()];
        let encoded = encode_key_set(&keys);
        let decoded = decode_key_set(&encoded);
        assert_eq!(decoded, keys);
    }

    #[test]
    fn test_encode_decode_empty() {
        let keys: Vec<Vec<u8>> = vec![];
        let encoded = encode_key_set(&keys);
        let decoded = decode_key_set(&encoded);
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_decode_truncated() {
        let decoded = decode_key_set(&[0, 0, 0]); // too short
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_reorg_height_key_constant() {
        assert_eq!(REORG_HEIGHT_KEY, b"__REORG_HEIGHT__");
    }

    #[test]
    fn test_height_to_hash_prefix_constant() {
        assert_eq!(HEIGHT_TO_HASH_PREFIX, b"__H2H__/");
    }

    #[test]
    fn test_height_to_hash_key_format() {
        let k = height_to_hash_key(42);
        assert!(k.starts_with(b"__H2H__/"));
        assert_eq!(&k[8..], &42u32.to_le_bytes());
    }

    #[test]
    fn test_height_to_hash_key_different_heights() {
        let k1 = height_to_hash_key(100);
        let k2 = height_to_hash_key(200);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_height_to_hash_key_no_collision_with_length_key() {
        // height_to_hash starts with "__H2H__/", length key ends with u32::MAX.
        let h = height_to_hash_key(0);
        let l = length_key(b"somekey");
        assert_ne!(h, l);
    }
}
