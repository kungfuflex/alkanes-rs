//! Merkle tree computation for block transactions.
//! Maps to: src/consensus/merkle.h/cpp

use crate::transaction::TransactionRef;
use qubitcoin_crypto::hash::hash256;
use qubitcoin_primitives::Uint256;

/// Compute the Merkle root of a list of hashes.
///
/// This is the standard Bitcoin Merkle tree construction:
/// - If the number of items is odd, the last item is duplicated.
/// - Pairs of items are concatenated and hashed.
/// - Process repeats until one hash remains.
pub fn compute_merkle_root(mut hashes: Vec<Uint256>, mutated: &mut bool) -> Uint256 {
    *mutated = false;

    if hashes.is_empty() {
        return Uint256::ZERO;
    }

    while hashes.len() > 1 {
        // Check for duplicate pairs BEFORE odd-element duplication.
        // This matches Bitcoin Core's ComputeMerkleRoot ordering:
        // duplicates from padding should not trigger mutation detection.
        for i in (0..hashes.len() - (hashes.len() & 1)).step_by(2) {
            if hashes[i] == hashes[i + 1] {
                *mutated = true;
            }
        }
        if hashes.len() & 1 != 0 {
            hashes.push(*hashes.last().unwrap());
        }
        let mut next_level = Vec::with_capacity(hashes.len() / 2);
        for i in (0..hashes.len()).step_by(2) {
            let mut combined = [0u8; 64];
            combined[..32].copy_from_slice(hashes[i].as_bytes());
            combined[32..].copy_from_slice(hashes[i + 1].as_bytes());
            next_level.push(Uint256::from_bytes(hash256(&combined)));
        }
        hashes = next_level;
    }

    hashes[0]
}

/// Compute the Merkle root of block transactions (non-witness).
///
/// Uses the TXID (non-witness hash) of each transaction.
pub fn block_merkle_root(txs: &[TransactionRef], mutated: &mut bool) -> Uint256 {
    let hashes: Vec<Uint256> = txs.iter().map(|tx| tx.txid().into_uint256()).collect();
    compute_merkle_root(hashes, mutated)
}

/// Compute the witness Merkle root of block transactions.
///
/// Uses the WTXID of each transaction, except the coinbase which uses all-zeros.
pub fn block_witness_merkle_root(txs: &[TransactionRef], mutated: &mut bool) -> Uint256 {
    let mut hashes: Vec<Uint256> = Vec::with_capacity(txs.len());
    // Coinbase's wtxid is defined as all-zeros
    if !txs.is_empty() {
        hashes.push(Uint256::ZERO);
    }
    for tx in txs.iter().skip(1) {
        hashes.push(tx.wtxid().into_uint256());
    }
    compute_merkle_root(hashes, mutated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::*;
    use qubitcoin_primitives::{Amount, Txid};
    use qubitcoin_script::Script;
    use std::sync::Arc;

    fn make_simple_tx(id_byte: u8) -> TransactionRef {
        Arc::new(Transaction::new(
            1,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([id_byte; 32]), 0),
                Script::new(),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(Amount::from_sat(100), Script::new())],
            0,
        ))
    }

    #[test]
    fn test_single_tx_merkle_root() {
        let tx = make_simple_tx(1);
        let mut mutated = false;
        let root = block_merkle_root(&[tx.clone()], &mut mutated);
        assert!(!mutated);
        // For a single transaction, merkle root == txid
        assert_eq!(root, tx.txid().into_uint256());
    }

    #[test]
    fn test_two_tx_merkle_root() {
        let tx1 = make_simple_tx(1);
        let tx2 = make_simple_tx(2);
        let mut mutated = false;
        let root = block_merkle_root(&[tx1.clone(), tx2.clone()], &mut mutated);
        assert!(!mutated);

        // Manually compute: Hash(tx1_hash || tx2_hash)
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(tx1.txid().as_bytes());
        combined[32..].copy_from_slice(tx2.txid().as_bytes());
        let expected = Uint256::from_bytes(hash256(&combined));
        assert_eq!(root, expected);
    }

    #[test]
    fn test_duplicate_detection() {
        let tx = make_simple_tx(1);
        let mut mutated = false;
        // Odd number of txs: last is duplicated
        let root = block_merkle_root(&[tx.clone(), tx.clone()], &mut mutated);
        assert!(mutated); // duplicate detected
    }
}
