#[cfg(test)]
mod tests {
    use crate::message::MessageContext;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};

    use crate::message::MessageContextParcel;
    use crate::test_helpers::{self as helpers, clear};
    use crate::Protorune;
    use anyhow::Result;
    use metashrew_core::index_pointer::AtomicPointer;
    use protorune_support::rune_transfer::RuneTransfer;

    use bitcoin::{OutPoint, Transaction, TxIn, TxOut, Amount, Sequence, Witness};
    use bitcoin::blockdata::script::ScriptBuf;
    use bitcoin::blockdata::transaction::Version;
    use ordinals::{Edict, RuneId, Runestone};
    use wasm_bindgen_test::*;

    struct MyMessageContext(());

    impl MessageContext for MyMessageContext {
        fn handle(
            _parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let ar: Vec<RuneTransfer> = vec![];
            Ok((ar, BalanceSheet::default()))
        }
        fn protocol_tag() -> u128 {
            100
        }
    }

    /// Test the bug where OP_RETURN at vout 0 causes remainder to go to wrong output
    ///
    /// Simulates the real-world transaction bb910271... with:
    /// - 8,356 runes incoming (using simplified numbers)
    /// - Edict: Give 500 runes to each output (edict.output == num_outputs)
    /// - vout 0: OP_RETURN
    /// - vouts 1-16: Regular outputs (should each get 500)
    /// - vout 17: Should get remainder (356)
    ///
    /// Bug: vout 16 gets remainder, vout 17 gets 500 (swapped!)
    #[wasm_bindgen_test]
    fn test_many_outputs_with_op_return_at_index_0() {
        clear();

        // Step 1: Create initial block with rune etching (8356 runes minted)
        let config = helpers::RunesTestingConfig {
            rune_etch_height: 840000,
            rune_etch_vout: 0,
            rune_name: Some("ALKAMIST".to_string()),
            rune_symbol: Some("A".to_string()),
            address1: helpers::ADDRESS1(),
            address2: helpers::ADDRESS2(),
            rune_pointer: Some(0), // Initial mint goes to vout 0
        };

        let etch_tx = helpers::create_rune_etching_transaction(&config);
        let etch_block = helpers::create_block_with_txs(vec![etch_tx.clone()]);

        let _ = Protorune::index_block::<MyMessageContext>(
            etch_block.clone(),
            config.rune_etch_height,
        );

        // Step 2: Create a transaction with 18 outputs total:
        // - vout 0: OP_RETURN (runestone)
        // - vouts 1-17: Regular outputs
        // Total available: 8356 runes
        // Edict: amount=500 per output, output=18 (special "distribute to all")
        let input_outpoint = OutPoint {
            txid: etch_tx.compute_txid(),
            vout: 0,
        };

        let txin = TxIn {
            previous_output: input_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        };

        // Create runestone with edict
        let rune_id = RuneId::new(config.rune_etch_height, config.rune_etch_vout).unwrap();
        let runestone: ScriptBuf = (Runestone {
            etching: None,
            pointer: None,
            edicts: vec![Edict {
                id: rune_id,
                amount: 500, // 500 runes each
                output: 18, // == number of outputs (special case: distribute to all)
            }],
            mint: None,
            protocol: None,
        })
        .encipher();

        let op_return = TxOut {
            value: Amount::from_sat(0),
            script_pubkey: runestone,
        };

        // Create 17 regular outputs (vouts 1-17)
        let address1 = helpers::get_address(&config.address1);
        let script_pubkey = address1.script_pubkey();

        let mut outputs = vec![op_return]; // vout 0
        for _ in 0..17 {
            outputs.push(TxOut {
                value: Amount::from_sat(1_000_000),
                script_pubkey: script_pubkey.clone(),
            });
        }

        let transfer_tx = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: outputs,
        };

        let transfer_block = helpers::create_block_with_txs(vec![etch_tx.clone(), transfer_tx.clone()]);

        let _ = Protorune::index_block::<MyMessageContext>(
            transfer_block.clone(),
            config.rune_etch_height + 1,
        );

        // Step 3: Verify balances
        let protorune_id = ProtoruneRuneId {
            block: config.rune_etch_height as u128,
            tx: config.rune_etch_vout as u128,
        };

        // Check vout 16 (should have 500 runes)
        let balance_vout_16 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: transfer_tx.compute_txid(),
                vout: 16,
            },
            vec![protorune_id],
        )[0];

        // Check vout 17 (should have remainder: 356 runes)
        let balance_vout_17 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: transfer_tx.compute_txid(),
                vout: 17,
            },
            vec![protorune_id],
        )[0];

        // Expected amounts
        let expected_vout_16 = 500u128; // 500 runes
        let expected_vout_17 = 356u128; // 356 runes (remainder: 8356 - 16*500 = 356)

        // These assertions will FAIL with the current buggy code
        // because vout 16 and 17 are swapped
        assert_eq!(
            balance_vout_16, expected_vout_16,
            "BUG: vout 16 should have {} but has {}",
            expected_vout_16, balance_vout_16
        );
        assert_eq!(
            balance_vout_17, expected_vout_17,
            "BUG: vout 17 should have {} (remainder) but has {}",
            expected_vout_17, balance_vout_17
        );
    }
}
