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
}
