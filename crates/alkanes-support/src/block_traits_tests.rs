//! Unit tests for BlockLike and TransactionLike trait implementations
//!
//! These tests verify that the abstractions work correctly at a fundamental level,
//! testing each trait method and conversion function.

#[cfg(test)]
mod tests {
    use crate::block_traits::{BlockLike, TransactionLike};
    use bitcoin::hashes::Hash as HashTrait;
    use bitcoin::{
        absolute::LockTime, block::Header, transaction::Version, Amount, Block, BlockHash,
        CompactTarget, OutPoint, ScriptBuf, Transaction, TxIn, TxMerkleNode, TxOut, Txid, Witness,
    };

    /// Create a simple test transaction with known values
    fn create_test_transaction() -> Transaction {
        Transaction {
            version: Version(2),
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![
                TxOut {
                    value: Amount::from_sat(5000000000),
                    script_pubkey: ScriptBuf::from_hex(
                        "76a914f54a5851e9372b87810a8e60cdd2e7cfd80b6e3188ac",
                    )
                    .unwrap(),
                },
                TxOut {
                    value: Amount::from_sat(1000000),
                    script_pubkey: ScriptBuf::from_hex("a914abcdef1234567890abcdef1234567890abcdef87")
                        .unwrap(),
                },
            ],
        }
    }

    /// Create a simple test block with known values
    fn create_test_block() -> Block {
        let tx1 = create_test_transaction();
        let mut tx2 = create_test_transaction();
        tx2.output[0].value = Amount::from_sat(2500000000);

        Block {
            header: Header {
                version: bitcoin::block::Version::from_consensus(1),
                prev_blockhash: BlockHash::all_zeros(),
                merkle_root: TxMerkleNode::all_zeros(),
                time: 1234567890,
                bits: CompactTarget::from_consensus(0x1d00ffff),
                nonce: 12345,
            },
            txdata: vec![tx1, tx2],
        }
    }

    #[test]
    fn test_transaction_like_txid() {
        let tx = create_test_transaction();
        let txid1 = tx.txid();
        let txid2 = tx.compute_txid();
        assert_eq!(
            txid1, txid2,
            "TransactionLike::txid() should match Transaction::compute_txid()"
        );
    }

    #[test]
    fn test_transaction_like_inputs() {
        let tx = create_test_transaction();
        let inputs = tx.inputs();
        assert_eq!(inputs.len(), 1, "Should have 1 input");
        assert_eq!(
            inputs.len(),
            tx.input.len(),
            "inputs() should match input field"
        );
        assert!(
            inputs[0].previous_output.is_null(),
            "Test transaction should be coinbase"
        );
    }

    #[test]
    fn test_transaction_like_outputs() {
        let tx = create_test_transaction();
        let outputs = tx.outputs();
        assert_eq!(outputs.len(), 2, "Should have 2 outputs");
        assert_eq!(
            outputs.len(),
            tx.output.len(),
            "outputs() should match output field"
        );
        assert_eq!(
            outputs[0].value,
            Amount::from_sat(5000000000),
            "First output value should match"
        );
        assert_eq!(
            outputs[1].value,
            Amount::from_sat(1000000),
            "Second output value should match"
        );
    }

    #[test]
    fn test_transaction_like_is_coinbase() {
        let tx = create_test_transaction();
        assert!(tx.is_coinbase(), "Test transaction should be coinbase");

        // Create non-coinbase transaction
        let mut non_coinbase = tx.clone();
        non_coinbase.input[0].previous_output = OutPoint {
            txid: Txid::all_zeros(),
            vout: 0,
        };
        assert!(
            !non_coinbase.is_coinbase(),
            "Transaction with non-null outpoint should not be coinbase"
        );
    }

    #[test]
    fn test_transaction_like_version() {
        let tx = create_test_transaction();
        assert_eq!(tx.version(), 2, "Version should be 2");
    }

    #[test]
    fn test_transaction_like_to_bitcoin_tx() {
        let tx = create_test_transaction();
        let converted = tx.to_bitcoin_tx();

        // For Bitcoin::Transaction, this should be a clone
        assert_eq!(
            converted.compute_txid(),
            tx.compute_txid(),
            "Converted transaction should have same txid"
        );
        assert_eq!(
            converted.input.len(),
            tx.input.len(),
            "Should preserve input count"
        );
        assert_eq!(
            converted.output.len(),
            tx.output.len(),
            "Should preserve output count"
        );
        assert_eq!(
            converted.version.0, tx.version.0,
            "Should preserve version"
        );
    }

    #[test]
    fn test_block_like_block_hash() {
        let block = create_test_block();
        let hash1 = block.block_hash();
        let hash2 = Block::block_hash(&block);
        assert_eq!(
            hash1, hash2,
            "BlockLike::block_hash() should match Block::block_hash()"
        );
    }

    #[test]
    fn test_block_like_transactions() {
        let block = create_test_block();
        let txs = block.transactions();
        assert_eq!(txs.len(), 2, "Should have 2 transactions");
        assert_eq!(
            txs.len(),
            block.txdata.len(),
            "transactions() should match txdata field"
        );
    }

    #[test]
    fn test_block_like_header() {
        let block = create_test_block();
        let header = block.header();
        assert_eq!(header.time, 1234567890, "Header time should match");
        assert_eq!(header.nonce, 12345, "Header nonce should match");
        assert_eq!(
            header.version,
            bitcoin::block::Version::from_consensus(1),
            "Header version should match"
        );
    }

    #[test]
    fn test_block_like_to_bitcoin_block() {
        let block = create_test_block();
        let converted = block.to_bitcoin_block();

        // For Bitcoin::Block, this should be a clone
        assert_eq!(
            converted.block_hash(),
            block.block_hash(),
            "Converted block should have same hash"
        );
        assert_eq!(
            converted.txdata.len(),
            block.txdata.len(),
            "Should preserve transaction count"
        );
        assert_eq!(
            converted.header.time, block.header.time,
            "Should preserve header"
        );

        // Verify transactions are properly converted
        for (i, tx) in converted.txdata.iter().enumerate() {
            assert_eq!(
                tx.compute_txid(),
                block.txdata[i].compute_txid(),
                "Transaction {} txid should match",
                i
            );
            assert_eq!(
                tx.input.len(),
                block.txdata[i].input.len(),
                "Transaction {} input count should match",
                i
            );
            assert_eq!(
                tx.output.len(),
                block.txdata[i].output.len(),
                "Transaction {} output count should match",
                i
            );
        }
    }

    #[test]
    fn test_to_bitcoin_block_preserves_vfsize() {
        let block = create_test_block();

        // Calculate vfsize of original block
        let original_vfsize: u64 = block
            .txdata
            .iter()
            .map(|tx| {
                use bitcoin::consensus::Encodable;
                let mut buf = Vec::new();
                tx.consensus_encode(&mut buf).unwrap();
                buf.len() as u64
            })
            .sum();

        // Convert and calculate vfsize of converted block
        let converted = block.to_bitcoin_block();
        let converted_vfsize: u64 = converted
            .txdata
            .iter()
            .map(|tx| {
                use bitcoin::consensus::Encodable;
                let mut buf = Vec::new();
                tx.consensus_encode(&mut buf).unwrap();
                buf.len() as u64
            })
            .sum();

        assert_eq!(
            original_vfsize, converted_vfsize,
            "Converted block should have same vfsize as original"
        );
        assert!(
            converted_vfsize > 0,
            "vfsize should be non-zero (got {})",
            converted_vfsize
        );
    }

    #[test]
    fn test_empty_block_to_bitcoin_block() {
        let empty_block = Block {
            header: Header {
                version: bitcoin::block::Version::from_consensus(1),
                prev_blockhash: BlockHash::all_zeros(),
                merkle_root: TxMerkleNode::all_zeros(),
                time: 1234567890,
                bits: CompactTarget::from_consensus(0x1d00ffff),
                nonce: 12345,
            },
            txdata: vec![],
        };

        let converted = empty_block.to_bitcoin_block();
        assert_eq!(
            converted.txdata.len(),
            0,
            "Empty block should remain empty"
        );
        assert_eq!(
            converted.block_hash(),
            empty_block.block_hash(),
            "Header should be preserved"
        );
    }

    #[test]
    fn test_transaction_roundtrip() {
        let tx = create_test_transaction();
        let converted = tx.to_bitcoin_tx();

        // Verify all fields are preserved
        assert_eq!(converted.version, tx.version);
        assert_eq!(converted.lock_time, tx.lock_time);
        assert_eq!(converted.input, tx.input);
        assert_eq!(converted.output, tx.output);
    }

    #[test]
    fn test_block_roundtrip() {
        let block = create_test_block();
        let converted = block.to_bitcoin_block();

        // Verify all fields are preserved
        assert_eq!(converted.header, block.header);
        assert_eq!(converted.txdata.len(), block.txdata.len());

        // Verify each transaction
        for (i, tx) in converted.txdata.iter().enumerate() {
            assert_eq!(*tx, block.txdata[i], "Transaction {} should match", i);
        }
    }

    #[test]
    fn test_multiple_outputs_preserved() {
        let mut tx = create_test_transaction();
        // Add more outputs
        for i in 0..10 {
            tx.output.push(TxOut {
                value: Amount::from_sat(i * 1000),
                script_pubkey: ScriptBuf::new(),
            });
        }

        let converted = tx.to_bitcoin_tx();
        assert_eq!(
            converted.output.len(),
            12,
            "Should have 12 outputs (2 original + 10 added)"
        );
        for (i, out) in converted.output.iter().enumerate() {
            assert_eq!(
                out.value, tx.output[i].value,
                "Output {} value should match",
                i
            );
        }
    }

    #[test]
    fn test_multiple_inputs_preserved() {
        let mut tx = create_test_transaction();
        // Add more inputs
        for i in 0..10 {
            tx.input.push(TxIn {
                previous_output: OutPoint {
                    txid: Txid::all_zeros(),
                    vout: i,
                },
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: Witness::new(),
            });
        }

        let converted = tx.to_bitcoin_tx();
        assert_eq!(
            converted.input.len(),
            11,
            "Should have 11 inputs (1 original + 10 added)"
        );
        for (i, inp) in converted.input.iter().enumerate() {
            assert_eq!(
                inp.previous_output, tx.input[i].previous_output,
                "Input {} should match",
                i
            );
        }
    }
}
