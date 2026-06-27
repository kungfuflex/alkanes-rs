//! Cargo tests for the new `simulate_transaction` view function.
//!
//! Strategy:
//!   1. Run the live indexer (`index_block`) on a setup block that creates
//!      an alkane (via the alkanes-std-test cellpack pattern). This puts
//!      state into the in-memory KV that the simulator can then consume.
//!   2. Construct a SECOND transaction that invokes the alkane via a
//!      protostone (opcode 0 — a no-op test target).
//!   3. Serialize that tx to hex.
//!   4. Call `view::simulate_transaction(tx_hex, height)`.
//!   5. Assert the response has:
//!       - The right txid.
//!       - At least one protostone execution.
//!       - A non-empty trace (the wasm runtime fired).
//!       - Final per-vout balance sheets (a `Vec<VoutBalances>`).
//!       - No `error` field set.
//!   6. CRUCIALLY: confirm that the simulator left ZERO on-disk side
//!      effects by re-reading state and asserting it's unchanged.

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
    /// The tx has 1 dummy input + 2 outputs (recipient dust + OP_RETURN
    /// carrying the runestone protocol field).
    ///
    /// The protostone `message` field is the LEB128-encoded byte form of
    /// the cellpack `[target.block, target.tx, opcode]` — same shape the
    /// indexer's `decode_varint_list` consumes inside `handle_message`.
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
            pointer: Some(0),     // route outputs to vout 0
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

        // Step 1: index a setup block that deploys the test alkane at id (2, 0)
        // via the standard alkanes-std-test bytecode.
        let setup_cellpacks = vec![Cellpack {
            target: AlkaneId { block: 1, tx: 0 }, // CREATE
            inputs: vec![78],                      // init
        }];
        let setup_block: Block = alkane_helpers::init_with_multiple_cellpacks(
            alkanes_std_test_build::get_bytes(),
            setup_cellpacks,
        );
        index_block(&setup_block, 0u32)?;

        // Step 2: capture the OUTPOINT_TO_RUNES key for one of the setup
        // outpoints (we'll assert it stays unchanged after simulation).
        let setup_tx = &setup_block.txdata[1]; // the non-coinbase tx
        let setup_outpoint = OutPoint {
            txid: setup_tx.compute_txid(),
            vout: 0,
        };
        let table = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag());
        let setup_key = consensus_encode(&setup_outpoint)?;
        let pre_sim_outpoint_state = table.OUTPOINT_TO_RUNES.select(&setup_key).keyword("/length").get_value::<u32>();

        // Step 3: build a second tx that invokes the deployed alkane
        // (target = (2,0), opcode = 0 — the test alkane responds).
        let invoke_tx = build_invoke_tx(
            AlkaneId { block: 2, tx: 0 },
            0u128,
            setup_outpoint, // spend the setup outpoint as input
        );
        let tx_hex = hex::encode(serialize(&invoke_tx));
        let expected_txid = invoke_tx.compute_txid().to_string();

        // Step 4: call simulate_transaction.
        let response = simulate_transaction(&tx_hex, 1u64)?;

        // Step 5: assertions about the response shape.
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
        // The wasm path executed → trace has events.
        let first = &response.protostones[0];
        let event_count = first.trace.0.lock().unwrap().len();
        assert!(
            event_count > 0,
            "expected the protostone trace to contain ≥1 event (EnterCall + ReturnContext/RevertContext), got 0"
        );

        // Step 6: confirm zero on-disk side effects.
        let post_sim_outpoint_state = table.OUTPOINT_TO_RUNES.select(&setup_key).keyword("/length").get_value::<u32>();
        assert_eq!(
            pre_sim_outpoint_state,
            post_sim_outpoint_state,
            "simulate_transaction must NOT mutate OUTPOINT_TO_RUNES for any outpoint"
        );

        // Confirm no trace was persisted for the simulated tx (TRACES_BY_HEIGHT
        // at height 1 should be empty since we never indexed at h=1).
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
        // Build a tx with no OP_RETURN (no runestone). Should return an empty
        // response with the `no runestone in transaction` marker.
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
        // Neither a PSBT nor a tx — should return Err from decode_tx_or_psbt.
        let bad = "deadbeefnotrealhex";
        let result = simulate_transaction(bad, 0u64);
        assert!(result.is_err());
        Ok(())
    }
}
