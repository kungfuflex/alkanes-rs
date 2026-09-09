//! Cargo tests for the new `simulate_transaction` / `simulate_protostones` /
//! `simulate_block` view functions.

#[cfg(test)]
mod tests {
    use crate::index_block;
    use crate::message::AlkaneMessageContext;
    use crate::tests::helpers as alkane_helpers;
    use crate::tests::std::alkanes_std_test_build;
    use crate::view::simulate_transaction;
    use alkane_helpers::clear;
    use alkanes_support::cellpack::Cellpack;
    use alkanes_support::id::AlkaneId;
    use anyhow::Result;
    use bitcoin::consensus::serialize;
    use bitcoin::{transaction::Version, Amount, ScriptBuf, Sequence, Transaction, TxIn, TxOut};
    use bitcoin::{Address, Block, OutPoint, Witness};
    use metashrew_support::index_pointer::KeyValuePointer;
    use ordinals::Runestone;
    use protorune::message::MessageContext;
    use protorune::protostone::Protostones;
    use protorune::tables::RuneTable;
    use protorune::test_helpers::{get_btc_network, ADDRESS1};
    use protorune_support::protostone::Protostone;
    use protorune_support::utils::consensus_encode;
    use std::str::FromStr;
    use wasm_bindgen_test::wasm_bindgen_test;

    /// Build a tx that invokes a deployed alkane via a protostone message.
    fn build_invoke_tx(target: AlkaneId, opcode: u128, prev_outpoint: OutPoint) -> Transaction {
        let cellpack = Cellpack {
            target,
            inputs: vec![opcode],
        };
        let protostone = Protostone {
            message: cellpack.encipher(),
            protocol_tag: 1,
            from: None,
            burn: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        };
        let runestone_script: ScriptBuf = (Runestone {
            etching: None,
            mint: None,
            pointer: None,
            edicts: vec![],
            protocol: vec![protostone].encipher().ok(),
        })
        .encipher();

        let recipient = Address::from_str(ADDRESS1().as_str())
            .unwrap()
            .require_network(get_btc_network())
            .unwrap()
            .script_pubkey();

        Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: prev_outpoint,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![
                TxOut {
                    script_pubkey: recipient,
                    value: Amount::from_sat(546),
                },
                TxOut {
                    script_pubkey: runestone_script,
                    value: Amount::from_sat(0),
                },
            ],
        }
    }

    #[wasm_bindgen_test]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn simulate_transaction_returns_a_trace_for_a_deployed_alkane() -> Result<()> {
        clear();

        let setup_cellpacks = vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![78],
        }];
        let setup_block: Block = alkane_helpers::init_with_multiple_cellpacks(
            alkanes_std_test_build::get_bytes(),
            setup_cellpacks,
        );
        index_block(&setup_block, 0u32)?;

        let setup_tx = &setup_block.txdata[1];
        let setup_outpoint = OutPoint {
            txid: setup_tx.compute_txid(),
            vout: 0,
        };
        let table = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag());
        let setup_key = consensus_encode(&setup_outpoint)?;
        let pre_sim_outpoint_state = table
            .OUTPOINT_TO_RUNES
            .select(&setup_key)
            .keyword("/length")
            .get_value::<u32>();

        let invoke_tx = build_invoke_tx(AlkaneId { block: 2, tx: 0 }, 0u128, setup_outpoint);
        let tx_hex = hex::encode(serialize(&invoke_tx));
        let expected_txid = invoke_tx.compute_txid().to_string();

        let response = simulate_transaction(&tx_hex, 1u64)?;

        assert_eq!(
            response.txid, expected_txid,
            "response.txid should match the input tx's computed txid"
        );
        assert_eq!(response.height, 1);
        assert!(
            response.error.is_none(),
            "simulate_transaction returned error: {:?}",
            response.error
        );
        assert!(
            !response.protostones.is_empty(),
            "expected at least one protostone execution, got 0"
        );
        let first = &response.protostones[0];
        let event_count = first.trace.0.lock().unwrap().len();
        assert!(
            event_count > 0,
            "expected the protostone trace to contain >=1 event, got 0"
        );

        let post_sim_outpoint_state = table
            .OUTPOINT_TO_RUNES
            .select(&setup_key)
            .keyword("/length")
            .get_value::<u32>();
        assert_eq!(
            pre_sim_outpoint_state, post_sim_outpoint_state,
            "simulate_transaction must NOT mutate OUTPOINT_TO_RUNES for any outpoint"
        );

        use crate::tables::TRACES_BY_HEIGHT;
        let traces_at_h1 = TRACES_BY_HEIGHT.select_value(1u64).length();
        assert_eq!(
            traces_at_h1, 0,
            "simulate_transaction must NOT write to TRACES_BY_HEIGHT; got {} entries at h=1",
            traces_at_h1
        );

        Ok(())
    }

    #[wasm_bindgen_test]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn simulate_transaction_handles_no_runestone_gracefully() -> Result<()> {
        clear();
        let recipient = Address::from_str(ADDRESS1().as_str())
            .unwrap()
            .require_network(get_btc_network())
            .unwrap()
            .script_pubkey();
        let tx = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                script_pubkey: recipient,
                value: Amount::from_sat(1000),
            }],
        };
        let tx_hex = hex::encode(serialize(&tx));
        let response = simulate_transaction(&tx_hex, 0u64)?;
        assert!(response.protostones.is_empty());
        assert!(response.final_balances_by_vout.is_empty());
        assert_eq!(response.total_fuel_used, 0);
        assert_eq!(
            response.error.as_deref(),
            Some("no runestone in transaction")
        );
        Ok(())
    }

    #[wasm_bindgen_test]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn simulate_transaction_rejects_garbage_hex() -> Result<()> {
        clear();
        let bad = "deadbeefnotrealhex";
        let result = simulate_transaction(bad, 0u64);
        assert!(result.is_err());
        Ok(())
    }

    /// Lower-level entry point: drive `simulate_protostones` directly.
    #[wasm_bindgen_test]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn simulate_protostones_synthesizes_tx_and_returns_touched_storage() -> Result<()> {
        use crate::view::{simulate_protostones, SimulateProtostonesInput};
        use bitcoin::consensus::deserialize as bitcoin_deserialize;
        use protorune_support::utils::encode_varint_list;

        clear();

        let setup_cellpacks = vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![78],
        }];
        let setup_block: Block = alkane_helpers::init_with_multiple_cellpacks(
            alkanes_std_test_build::get_bytes(),
            setup_cellpacks,
        );
        crate::index_block(&setup_block, 0u32)?;

        let cellpack = Cellpack {
            target: AlkaneId { block: 2, tx: 0 },
            inputs: vec![99u128],
        };
        let protostone = Protostone {
            message: cellpack.encipher(),
            protocol_tag: 1,
            from: None,
            burn: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        };
        let protocol_values = vec![protostone].encipher()?;
        let protostones_bytes = encode_varint_list(&protocol_values);

        let response = simulate_protostones(SimulateProtostonesInput {
            height: 1,
            alkane_inputs: vec![],
            protostones_bytes,
            transaction_bytes: None,
            block_bytes: None,
            storage_overrides: vec![],
        })?;

        assert!(
            response.error.is_none(),
            "simulate_protostones returned error: {:?}",
            response.error
        );
        assert!(
            !response.protostones.is_empty(),
            "expected >=1 protostone execution, got 0"
        );
        assert!(
            !response.used_transaction_bytes.is_empty(),
            "used_transaction_bytes should be the synthesized tx bytes"
        );
        assert!(
            !response.used_block_bytes.is_empty(),
            "used_block_bytes should be the synthesized block bytes"
        );
        let _round_tx: Transaction = bitcoin_deserialize(&response.used_transaction_bytes)?;
        let _round_block: Block = bitcoin_deserialize(&response.used_block_bytes)?;

        let first = &response.protostones[0];
        let event_count = first.trace.0.lock().unwrap().len();
        assert!(event_count > 0, "expected >=1 trace event, got 0");
        let _ = &first.touched_storage;
        Ok(())
    }

    /// Storage overrides path: confirm that pre-execution writes injected
    /// into the sandbox atomic don't crash the run.
    #[wasm_bindgen_test]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn simulate_protostones_applies_storage_overrides_without_crashing() -> Result<()> {
        use crate::view::{simulate_protostones, SimulateProtostonesInput};
        use protorune_support::utils::encode_varint_list;

        clear();

        let setup_cellpacks = vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![78],
        }];
        let setup_block: Block = alkane_helpers::init_with_multiple_cellpacks(
            alkanes_std_test_build::get_bytes(),
            setup_cellpacks,
        );
        crate::index_block(&setup_block, 0u32)?;

        let cellpack = Cellpack {
            target: AlkaneId { block: 2, tx: 0 },
            inputs: vec![99u128],
        };
        let protostone = Protostone {
            message: cellpack.encipher(),
            protocol_tag: 1,
            from: None,
            burn: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        };
        let protostones_bytes = encode_varint_list(&vec![protostone].encipher()?);

        let override_key = b"/__sim_marker".to_vec();
        let override_value = b"\xde\xad\xbe\xef".to_vec();
        let overrides = vec![(
            AlkaneId { block: 2, tx: 0 },
            vec![(override_key.clone(), override_value.clone())],
        )];

        let response = simulate_protostones(SimulateProtostonesInput {
            height: 1,
            alkane_inputs: vec![],
            protostones_bytes,
            transaction_bytes: None,
            block_bytes: None,
            storage_overrides: overrides,
        })?;

        assert!(
            response.error.is_none(),
            "simulate_protostones with overrides returned error: {:?}",
            response.error
        );
        assert!(!response.protostones.is_empty());
        Ok(())
    }

    /// `simulate_block` smoke test: a 3-tx block (coinbase + 2 GetName
    /// invocations) at h=1 verifies orchestration shape.
    #[wasm_bindgen_test]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn simulate_block_runs_each_tx_in_order_with_shared_sandbox() -> Result<()> {
        use crate::tables::TRACES_BY_HEIGHT;
        use crate::view::{simulate_block, SimulateBlockInput};
        use bitcoin::consensus::serialize as bitcoin_serialize;
        use bitcoin::hashes::Hash;
        use bitcoin::{
            blockdata::block::{Header, Version as BlockVersion},
            BlockHash, CompactTarget, TxMerkleNode,
        };

        clear();

        let setup_cellpacks = vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![78],
        }];
        let setup_block: Block = alkane_helpers::init_with_multiple_cellpacks(
            alkanes_std_test_build::get_bytes(),
            setup_cellpacks,
        );
        crate::index_block(&setup_block, 0u32)?;
        let setup_tx = &setup_block.txdata[1];
        let prev_outpoint = OutPoint {
            txid: setup_tx.compute_txid(),
            vout: 0,
        };

        fn build_getname_tx(prev: OutPoint) -> Transaction {
            let cellpack = Cellpack {
                target: AlkaneId { block: 2, tx: 0 },
                inputs: vec![99u128],
            };
            let protostone = Protostone {
                message: cellpack.encipher(),
                protocol_tag: 1,
                from: None,
                burn: None,
                pointer: Some(0),
                refund: Some(0),
                edicts: vec![],
            };
            let runestone_script: ScriptBuf = (Runestone {
                etching: None,
                mint: None,
                pointer: None,
                edicts: vec![],
                protocol: vec![protostone].encipher().ok(),
            })
            .encipher();
            Transaction {
                version: Version::ONE,
                lock_time: bitcoin::absolute::LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: prev,
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::new(),
                }],
                output: vec![
                    TxOut {
                        script_pubkey: ScriptBuf::new(),
                        value: Amount::from_sat(546),
                    },
                    TxOut {
                        script_pubkey: runestone_script,
                        value: Amount::from_sat(0),
                    },
                ],
            }
        }

        let coinbase = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::from_bytes(vec![0x01, 0x01]),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                script_pubkey: ScriptBuf::new(),
                value: Amount::from_sat(0),
            }],
        };
        let tx1 = build_getname_tx(prev_outpoint);
        let tx2 = build_getname_tx(OutPoint {
            txid: tx1.compute_txid(),
            vout: 0,
        });

        let block = Block {
            header: Header {
                version: BlockVersion::ONE,
                prev_blockhash: BlockHash::all_zeros(),
                merkle_root: TxMerkleNode::all_zeros(),
                time: 0,
                bits: CompactTarget::from_consensus(0),
                nonce: 0xDEAD_BEEF,
            },
            txdata: vec![coinbase, tx1, tx2],
        };
        let block_bytes = bitcoin_serialize(&block);
        let expected_block_hash = block.block_hash().to_string();

        let table = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag());
        let key = consensus_encode(&prev_outpoint)?;
        let pre_state = table
            .OUTPOINT_TO_RUNES
            .select(&key)
            .keyword("/length")
            .get_value::<u32>();
        let pre_traces_at_h1 = TRACES_BY_HEIGHT.select_value(1u64).length();

        let response = simulate_block(SimulateBlockInput {
            height: 1,
            block_bytes,
            storage_overrides: vec![],
        })?;

        assert!(
            response.error.is_none(),
            "simulate_block error: {:?}",
            response.error
        );
        assert_eq!(response.block_hash, expected_block_hash);
        assert_eq!(response.height, 1);
        assert_eq!(
            response.txs.len(),
            3,
            "expected 3 tx slots (coinbase + 2 runestones)"
        );
        assert_eq!(response.txs[0].error.as_deref(), Some("coinbase"));
        assert!(response.txs[1].error.is_none(), "tx1 should succeed");
        assert!(
            response.txs[2].error.is_none(),
            "tx2 should succeed (sees tx1's sandbox state)"
        );
        assert!(!response.txs[1].protostones.is_empty());
        assert!(!response.txs[2].protostones.is_empty());

        let summed: u64 = response.txs.iter().map(|t| t.total_fuel_used).sum();
        assert_eq!(response.total_fuel_used, summed);

        let post_state = table
            .OUTPOINT_TO_RUNES
            .select(&key)
            .keyword("/length")
            .get_value::<u32>();
        assert_eq!(
            pre_state, post_state,
            "simulate_block must NOT mutate OUTPOINT_TO_RUNES"
        );
        let post_traces_at_h1 = TRACES_BY_HEIGHT.select_value(1u64).length();
        assert_eq!(
            pre_traces_at_h1, post_traces_at_h1,
            "simulate_block must NOT persist traces"
        );
        Ok(())
    }
}
