//! Tests to detect potential data loss in BlockLike/TransactionLike trait implementations
//!
//! These tests verify that witness data and other critical fields are preserved when
//! using the generic trait abstractions, particularly for WASM deployment detection.

#[cfg(test)]
mod tests {
    use crate::block_traits::{BlockLike, TransactionLike};
    use crate::envelope::RawEnvelope;
    use bitcoin::hashes::Hash as HashTrait;
    use bitcoin::{
        absolute::LockTime, block::Header, opcodes, script, transaction::Version, Amount, Block,
        BlockHash, CompactTarget, OutPoint, ScriptBuf, Transaction, TxIn, TxMerkleNode, TxOut,
        Txid, Witness,
    };

    /// Create a transaction with a tapscript envelope (simulating WASM deployment)
    fn create_transaction_with_envelope() -> Transaction {
        // Create a witness with a BIN envelope (used for alkane deployments)
        let mut witness = Witness::new();

        // Build a reveal script with BIN protocol ID
        let reveal_script = script::Builder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"BIN") // Protocol ID
            .push_slice::<&script::PushBytes>((&[]).try_into().unwrap()) // BODY_TAG (empty)
            .push_slice(b"test wasm payload data") // Simulated WASM payload
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script();

        // Push the reveal script as the tapscript
        witness.push(reveal_script.as_bytes());
        // Push empty control block (simplified for testing)
        witness.push(&[0u8; 33]); // Minimal control block

        Transaction {
            version: Version(2),
            lock_time: LockTime::from_consensus(12345), // Non-zero lock_time
            input: vec![TxIn {
                previous_output: OutPoint {
                    txid: Txid::all_zeros(),
                    vout: 0,
                },
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness,
            }],
            output: vec![TxOut {
                value: Amount::from_sat(1000),
                script_pubkey: ScriptBuf::new(),
            }],
        }
    }

    /// Create a block with envelope transactions
    fn create_block_with_envelopes() -> Block {
        Block {
            header: Header {
                version: bitcoin::block::Version::from_consensus(1),
                prev_blockhash: BlockHash::all_zeros(),
                merkle_root: TxMerkleNode::all_zeros(),
                time: 1234567890,
                bits: CompactTarget::from_consensus(0x1d00ffff),
                nonce: 12345,
            },
            txdata: vec![create_transaction_with_envelope()],
        }
    }

    // ==================== CRITICAL: Witness Data Preservation ====================

    #[test]
    fn test_witness_data_preserved_in_inputs() {
        let tx = create_transaction_with_envelope();
        let inputs = tx.inputs();

        assert_eq!(inputs.len(), 1, "Should have 1 input");
        assert!(!inputs[0].witness.is_empty(), "Witness should not be empty");

        // Verify witness has at least 2 elements (script + control block)
        assert!(
            inputs[0].witness.len() >= 2,
            "Witness should have at least 2 elements, got {}",
            inputs[0].witness.len()
        );
    }

    #[test]
    fn test_witness_tapscript_extraction() {
        let tx = create_transaction_with_envelope();
        let inputs = tx.inputs();

        // Verify tapscript can be extracted from the witness
        let tapscript = inputs[0].witness.tapscript();
        assert!(tapscript.is_some(), "Should be able to extract tapscript from witness");

        let script = tapscript.unwrap();
        // Verify the script contains our BIN protocol marker
        let script_bytes = script.as_bytes();
        assert!(
            script_bytes.windows(3).any(|w| w == b"BIN"),
            "Tapscript should contain BIN protocol ID"
        );
    }

    #[test]
    fn test_to_bitcoin_tx_preserves_witness() {
        let tx = create_transaction_with_envelope();
        let converted = tx.to_bitcoin_tx();

        // Critical: witness must be preserved
        assert_eq!(
            converted.input[0].witness.len(),
            tx.input[0].witness.len(),
            "to_bitcoin_tx() must preserve witness length"
        );

        // Verify witness content is identical
        for (i, (orig, conv)) in tx.input[0].witness.iter()
            .zip(converted.input[0].witness.iter())
            .enumerate()
        {
            assert_eq!(
                orig, conv,
                "Witness element {} must be identical after conversion",
                i
            );
        }
    }

    #[test]
    fn test_to_bitcoin_tx_preserves_tapscript() {
        let tx = create_transaction_with_envelope();
        let converted = tx.to_bitcoin_tx();

        // Extract tapscript from both and compare
        let orig_tapscript = tx.input[0].witness.tapscript();
        let conv_tapscript = converted.input[0].witness.tapscript();

        assert_eq!(
            orig_tapscript.is_some(),
            conv_tapscript.is_some(),
            "Tapscript availability must match"
        );

        if let (Some(orig), Some(conv)) = (orig_tapscript, conv_tapscript) {
            assert_eq!(
                orig.as_bytes(),
                conv.as_bytes(),
                "Tapscript content must be identical after conversion"
            );
        }
    }

    // ==================== CRITICAL: Envelope Extraction ====================

    #[test]
    fn test_envelope_extraction_from_original_tx() {
        let tx = create_transaction_with_envelope();
        let envelopes = RawEnvelope::from_transaction(&tx);

        assert!(
            !envelopes.is_empty(),
            "Should extract at least one envelope from transaction with BIN marker"
        );
    }

    #[test]
    fn test_envelope_extraction_from_converted_tx() {
        let tx = create_transaction_with_envelope();
        let converted = tx.to_bitcoin_tx();

        // Extract envelopes from BOTH original and converted
        let orig_envelopes = RawEnvelope::from_transaction(&tx);
        let conv_envelopes = RawEnvelope::from_transaction(&converted);

        assert_eq!(
            orig_envelopes.len(),
            conv_envelopes.len(),
            "Envelope count must match between original and converted transaction"
        );

        // Verify envelope content matches
        for (i, (orig, conv)) in orig_envelopes.iter().zip(conv_envelopes.iter()).enumerate() {
            assert_eq!(
                orig.payload, conv.payload,
                "Envelope {} payload must match after conversion",
                i
            );
            assert_eq!(
                orig.input, conv.input,
                "Envelope {} input index must match after conversion",
                i
            );
        }
    }

    #[test]
    fn test_envelope_extraction_through_block_like() {
        let block = create_block_with_envelopes();

        // Extract envelopes from original block transactions
        let orig_envelopes: Vec<RawEnvelope> = block.txdata.iter()
            .flat_map(|tx| RawEnvelope::from_transaction(tx))
            .collect();

        // Convert block using trait and extract envelopes
        let converted_block = block.to_bitcoin_block();
        let conv_envelopes: Vec<RawEnvelope> = converted_block.txdata.iter()
            .flat_map(|tx| RawEnvelope::from_transaction(tx))
            .collect();

        assert_eq!(
            orig_envelopes.len(),
            conv_envelopes.len(),
            "Block-level envelope count must match after to_bitcoin_block()"
        );
    }

    // ==================== CRITICAL: Lock Time Preservation ====================

    #[test]
    fn test_lock_time_preserved() {
        let tx = create_transaction_with_envelope();
        let converted = tx.to_bitcoin_tx();

        // The default trait impl sets lock_time to ZERO, but Bitcoin impl should clone
        assert_eq!(
            tx.lock_time, converted.lock_time,
            "lock_time must be preserved (got {:?} vs {:?})",
            tx.lock_time, converted.lock_time
        );
    }

    #[test]
    fn test_non_zero_lock_time_preserved() {
        let mut tx = create_transaction_with_envelope();
        tx.lock_time = LockTime::from_consensus(500000);

        let converted = tx.to_bitcoin_tx();

        assert_eq!(
            tx.lock_time, converted.lock_time,
            "Non-zero lock_time must be preserved"
        );
    }

    // ==================== CRITICAL: TxId Consistency ====================

    #[test]
    fn test_txid_matches_after_conversion() {
        let tx = create_transaction_with_envelope();

        let orig_txid = tx.compute_txid();
        let trait_txid = tx.txid(); // Via TransactionLike trait
        let converted = tx.to_bitcoin_tx();
        let conv_txid = converted.compute_txid();

        assert_eq!(
            orig_txid, trait_txid,
            "TransactionLike::txid() must match compute_txid()"
        );
        assert_eq!(
            orig_txid, conv_txid,
            "Converted transaction must have same txid as original"
        );
    }

    #[test]
    fn test_wtxid_matches_after_conversion() {
        let tx = create_transaction_with_envelope();

        let orig_wtxid = tx.compute_wtxid();
        let converted = tx.to_bitcoin_tx();
        let conv_wtxid = converted.compute_wtxid();

        assert_eq!(
            orig_wtxid, conv_wtxid,
            "wtxid must match after conversion (witness data affects wtxid)"
        );
    }

    // ==================== Block-Level Tests ====================

    #[test]
    fn test_block_transactions_preserve_witness() {
        let block = create_block_with_envelopes();
        let txs = block.transactions();

        for (i, tx) in txs.iter().enumerate() {
            for (j, input) in tx.inputs().iter().enumerate() {
                if !input.witness.is_empty() {
                    // Verify witness is accessible through trait
                    let tapscript = input.witness.tapscript();
                    assert!(
                        tapscript.is_some() || input.witness.len() < 2,
                        "Transaction {} input {} should have extractable tapscript if witness has 2+ elements",
                        i, j
                    );
                }
            }
        }
    }

    #[test]
    fn test_block_to_bitcoin_block_preserves_all_witnesses() {
        let block = create_block_with_envelopes();
        let converted = block.to_bitcoin_block();

        for (i, (orig_tx, conv_tx)) in block.txdata.iter()
            .zip(converted.txdata.iter())
            .enumerate()
        {
            for (j, (orig_input, conv_input)) in orig_tx.input.iter()
                .zip(conv_tx.input.iter())
                .enumerate()
            {
                assert_eq!(
                    orig_input.witness.len(),
                    conv_input.witness.len(),
                    "Transaction {} input {} witness length must match",
                    i, j
                );

                for (k, (orig_elem, conv_elem)) in orig_input.witness.iter()
                    .zip(conv_input.witness.iter())
                    .enumerate()
                {
                    assert_eq!(
                        orig_elem, conv_elem,
                        "Transaction {} input {} witness element {} must match",
                        i, j, k
                    );
                }
            }
        }
    }

    // ==================== Trait Method Consistency ====================

    #[test]
    fn test_inputs_returns_reference_not_copy() {
        let tx = create_transaction_with_envelope();

        // Get inputs through trait
        let inputs = tx.inputs();

        // Verify it's the same data (not a copy that lost witness)
        assert_eq!(
            inputs[0].witness.len(),
            tx.input[0].witness.len(),
            "inputs() must return reference to actual input data with witness"
        );
    }

    #[test]
    fn test_generic_function_preserves_envelope_extraction() {
        fn extract_envelopes_generic<T: TransactionLike>(tx: &T) -> Vec<RawEnvelope> {
            // This simulates how the indexer might use the trait
            let bitcoin_tx = tx.to_bitcoin_tx();
            RawEnvelope::from_transaction(&bitcoin_tx)
        }

        let tx = create_transaction_with_envelope();

        // Direct extraction
        let direct_envelopes = RawEnvelope::from_transaction(&tx);

        // Through generic function
        let generic_envelopes = extract_envelopes_generic(&tx);

        assert_eq!(
            direct_envelopes.len(),
            generic_envelopes.len(),
            "Envelope extraction through generic function must match direct extraction"
        );
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_empty_witness_handled() {
        let tx = Transaction {
            version: Version(2),
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: Witness::new(), // Empty witness
            }],
            output: vec![TxOut {
                value: Amount::from_sat(1000),
                script_pubkey: ScriptBuf::new(),
            }],
        };

        let converted = tx.to_bitcoin_tx();
        assert!(
            converted.input[0].witness.is_empty(),
            "Empty witness should remain empty after conversion"
        );
    }

    #[test]
    fn test_multiple_inputs_with_different_witnesses() {
        let mut witness1 = Witness::new();
        witness1.push(b"witness1_data");

        let mut witness2 = Witness::new();
        witness2.push(b"witness2_data");
        witness2.push(b"witness2_more");

        let tx = Transaction {
            version: Version(2),
            lock_time: LockTime::ZERO,
            input: vec![
                TxIn {
                    previous_output: OutPoint { txid: Txid::all_zeros(), vout: 0 },
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::MAX,
                    witness: witness1,
                },
                TxIn {
                    previous_output: OutPoint { txid: Txid::all_zeros(), vout: 1 },
                    script_sig: ScriptBuf::new(),
                    sequence: bitcoin::Sequence::MAX,
                    witness: witness2,
                },
            ],
            output: vec![TxOut {
                value: Amount::from_sat(1000),
                script_pubkey: ScriptBuf::new(),
            }],
        };

        let converted = tx.to_bitcoin_tx();

        assert_eq!(
            converted.input[0].witness.len(), 1,
            "First input should have 1 witness element"
        );
        assert_eq!(
            converted.input[1].witness.len(), 2,
            "Second input should have 2 witness elements"
        );
    }

    // ==================== Comparison with Default Trait Implementation ====================

    /// This test verifies what the DEFAULT trait implementation would do
    /// (i.e., what happens for non-Bitcoin types like Zcash)
    #[test]
    fn test_default_impl_loses_lock_time() {
        // Simulate what the default implementation does
        fn simulate_default_to_bitcoin_tx<T: TransactionLike>(tx: &T) -> Transaction {
            Transaction {
                version: bitcoin::transaction::Version(tx.version()),
                lock_time: LockTime::ZERO, // DEFAULT IMPL ALWAYS SETS ZERO
                input: tx.inputs().to_vec(),
                output: tx.outputs().to_vec(),
            }
        }

        let tx = create_transaction_with_envelope();
        let default_converted = simulate_default_to_bitcoin_tx(&tx);

        // This demonstrates the data loss in the default implementation
        assert_ne!(
            tx.lock_time,
            default_converted.lock_time,
            "Default implementation DOES lose lock_time (expected for this test)"
        );

        // But Bitcoin's override should preserve it
        let bitcoin_converted = tx.to_bitcoin_tx();
        assert_eq!(
            tx.lock_time,
            bitcoin_converted.lock_time,
            "Bitcoin's to_bitcoin_tx() implementation should preserve lock_time"
        );
    }

    #[test]
    fn test_default_impl_preserves_witness_through_inputs() {
        // Even the default impl should preserve witness because inputs() returns &[TxIn]
        // and TxIn contains witness
        fn simulate_default_to_bitcoin_tx<T: TransactionLike>(tx: &T) -> Transaction {
            Transaction {
                version: bitcoin::transaction::Version(tx.version()),
                lock_time: LockTime::ZERO,
                input: tx.inputs().to_vec(), // This clones the full TxIn including witness
                output: tx.outputs().to_vec(),
            }
        }

        let tx = create_transaction_with_envelope();
        let default_converted = simulate_default_to_bitcoin_tx(&tx);

        // Witness should still be preserved because TxIn.clone() includes witness
        assert_eq!(
            tx.input[0].witness.len(),
            default_converted.input[0].witness.len(),
            "Even default impl preserves witness through inputs().to_vec()"
        );
    }
}
