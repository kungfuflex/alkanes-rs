//! Sparse Merkle Tree (SHA-256) for computing state roots.
//!
//! Provides a per-height state root hash over the indexer's key-value state.
//! The SMT uses SHA-256 as the hash function and computes roots over
//! the current set of key-value pairs.

use sha2::{Digest, Sha256};

/// Key for storing the SMT root at a given height.
pub fn smt_root_key(height: u32) -> Vec<u8> {
    let mut k = Vec::with_capacity(12);
    k.extend_from_slice(b"__SMT__/");
    k.extend_from_slice(&height.to_le_bytes());
    k
}

/// Compute a SHA-256 state root from a set of key-value pairs.
///
/// This is a simplified "sorted hash list" approach: pairs are sorted by key,
/// then iteratively hashed together to produce a single root.
pub fn compute_state_root(pairs: &[(Vec<u8>, Vec<u8>)]) -> [u8; 32] {
    if pairs.is_empty() {
        return [0u8; 32];
    }

    // Sort pairs by key for deterministic ordering.
    let mut sorted: Vec<&(Vec<u8>, Vec<u8>)> = pairs.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    // Hash each key-value pair into a leaf hash.
    let mut hashes: Vec<[u8; 32]> = sorted
        .iter()
        .map(|(k, v)| {
            let mut hasher = Sha256::new();
            hasher.update(&(k.len() as u32).to_le_bytes());
            hasher.update(k);
            hasher.update(&(v.len() as u32).to_le_bytes());
            hasher.update(v);
            hasher.finalize().into()
        })
        .collect();

    // Build a Merkle tree by iteratively combining pairs.
    while hashes.len() > 1 {
        let mut next_level = Vec::with_capacity((hashes.len() + 1) / 2);
        for chunk in hashes.chunks(2) {
            if chunk.len() == 2 {
                let mut hasher = Sha256::new();
                hasher.update(chunk[0]);
                hasher.update(chunk[1]);
                next_level.push(hasher.finalize().into());
            } else {
                // Odd element: promote to next level.
                next_level.push(chunk[0]);
            }
        }
        hashes = next_level;
    }

    hashes[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smt_root_key() {
        let key = smt_root_key(100);
        let mut expected = b"__SMT__/".to_vec();
        expected.extend_from_slice(&100u32.to_le_bytes());
        assert_eq!(key, expected);
    }

    #[test]
    fn test_empty_state_root() {
        let root = compute_state_root(&[]);
        assert_eq!(root, [0u8; 32]);
    }

    #[test]
    fn test_single_pair_root() {
        let pairs = vec![(b"key".to_vec(), b"value".to_vec())];
        let root = compute_state_root(&pairs);
        // Should be a valid SHA-256 hash, not all zeros.
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn test_deterministic_root() {
        let pairs = vec![
            (b"a".to_vec(), b"1".to_vec()),
            (b"b".to_vec(), b"2".to_vec()),
        ];
        let root1 = compute_state_root(&pairs);
        let root2 = compute_state_root(&pairs);
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_order_independent() {
        let pairs_ab = vec![
            (b"a".to_vec(), b"1".to_vec()),
            (b"b".to_vec(), b"2".to_vec()),
        ];
        let pairs_ba = vec![
            (b"b".to_vec(), b"2".to_vec()),
            (b"a".to_vec(), b"1".to_vec()),
        ];
        // Should produce the same root regardless of input order.
        assert_eq!(
            compute_state_root(&pairs_ab),
            compute_state_root(&pairs_ba)
        );
    }

    #[test]
    fn test_different_data_different_root() {
        let pairs1 = vec![(b"key".to_vec(), b"value1".to_vec())];
        let pairs2 = vec![(b"key".to_vec(), b"value2".to_vec())];
        assert_ne!(compute_state_root(&pairs1), compute_state_root(&pairs2));
    }

    #[test]
    fn test_odd_number_of_pairs() {
        let pairs = vec![
            (b"a".to_vec(), b"1".to_vec()),
            (b"b".to_vec(), b"2".to_vec()),
            (b"c".to_vec(), b"3".to_vec()),
        ];
        let root = compute_state_root(&pairs);
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn test_four_pairs() {
        let pairs = vec![
            (b"a".to_vec(), b"1".to_vec()),
            (b"b".to_vec(), b"2".to_vec()),
            (b"c".to_vec(), b"3".to_vec()),
            (b"d".to_vec(), b"4".to_vec()),
        ];
        let root = compute_state_root(&pairs);
        assert_ne!(root, [0u8; 32]);
        // With 4 leaves, tree should have 2 levels of hashing.
        // Verify determinism.
        assert_eq!(root, compute_state_root(&pairs));
    }
}
