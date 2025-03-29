#[cfg(test)]
mod tests {
    use crate::message::MessageContext;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};

    use crate::message::MessageContextParcel;
    use crate::test_helpers::{self as helpers, RunesTestingConfig, ADDRESS1, ADDRESS2};
    use crate::Protorune;
    use anyhow::Result;
    use metashrew::index_pointer::AtomicPointer;
    use protorune_support::rune_transfer::RuneTransfer;

    use bitcoin::OutPoint;

    use helpers::clear;
    #[allow(unused_imports)]
    use metashrew::{
        println,
        stdio::{stdout, Write},
    };
    use ordinals::{Edict, RuneId};

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

    ///
    /// EDICT TRANSFER TESTS
    /// refer to https://docs.ordinals.com/runes/specification.html#transferring
    /// for the proper spec that I am testing
    ///

    fn edict_test(
        config: RunesTestingConfig,
        edict_amount: Option<u128>,
        edict_output: Option<u32>,
        expected_address1_amount: u128,
        expected_address2_amount: u128,
    ) {
        let rune_id = RuneId::new(config.rune_etch_height, config.rune_etch_vout).unwrap();
        let edicts = match (edict_amount, edict_output) {
            (Some(amount), Some(output)) => vec![Edict {
                id: rune_id,
                amount,
                output,
            }],
            _ => Vec::new(),
        };

        let test_block = helpers::create_block_with_rune_transfer(&config, edicts);
        let _ =
            Protorune::index_block::<MyMessageContext>(test_block.clone(), config.rune_etch_height);
        let protorune_id = ProtoruneRuneId {
            block: config.rune_etch_height as u128,
            tx: config.rune_etch_vout as u128,
        };
        let stored_balance_address2 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![protorune_id],
        )[0];
        let stored_balance_address1 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[1].compute_txid(),
                vout: 1,
            },
            vec![protorune_id],
        )[0];
        assert_eq!(stored_balance_address1, expected_address1_amount);
        assert_eq!(stored_balance_address2, expected_address2_amount);

        // assert that original outpoint where runes were minted are not spenable anymore
        let stored_balance_address1_original = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![protorune_id],
        )[0];
        assert_eq!(0, stored_balance_address1_original);
    }

    /// normal transfer works
    #[wasm_bindgen_test]
    fn correct_balance_sheet_with_transfers() {
        clear();
        edict_test(
            RunesTestingConfig::default(),
            Some(200),
            Some(0),
            800 as u128,
            200 as u128,
        );
    }

    /// transferring more runes only transfers the amount remaining
    #[wasm_bindgen_test]
    fn correct_balance_sheet_transfer_too_much() {
        clear();
        edict_test(
            RunesTestingConfig::default(),
            Some(1200),
            Some(0),
            0 as u128,
            1000 as u128,
        );
    }

    /// Tests that transferring runes to an outpoint > num outpoints is a cenotaph.
    /// All runes input to a tx containing a cenotaph is burned
    #[wasm_bindgen_test]
    fn cenotaph_balance_sheet_transfer_bad_target() {
        clear();
        edict_test(RunesTestingConfig::default(), Some(200), Some(4), 0, 0);
    }

    /// Tests that transferring runes to an outpoint == OP_RETURN burns the runes.
    #[wasm_bindgen_test]
    fn correct_balance_sheet_transfer_target_op_return() {
        clear();
        edict_test(RunesTestingConfig::default(), Some(200), Some(2), 800, 0);
    }

    /// An edict with amount zero allocates all remaining units of rune id.
    #[wasm_bindgen_test]
    fn correct_balance_sheet_transfer_0() {
        clear();
        edict_test(RunesTestingConfig::default(), Some(0), Some(0), 0, 1000);
    }

    /// An edict with output == number of transaction outputs will
    /// allocates amount runes to each non-OP_RETURN output in order
    #[wasm_bindgen_test]
    fn correct_balance_sheet_equal_distribute_300() {
        clear();
        edict_test(RunesTestingConfig::default(), Some(300), Some(3), 700, 300);
    }

    /// An edict with output == number of transaction outputs
    /// and amount = 0 will equally distribute all remaining runes
    /// to each non-OP_RETURN output in order
    #[wasm_bindgen_test]
    fn correct_balance_sheet_equal_distribute_0() {
        clear();
        edict_test(RunesTestingConfig::default(), Some(0), Some(3), 500, 500);
    }

    /// No edict, all amount should go to pointer
    #[wasm_bindgen_test]
    fn no_edict_pointer_transfer() {
        clear();
        edict_test(RunesTestingConfig::default(), None, None, 1000, 0);
    }

    /// No edict, pointer None, transfer runes to first non op return output
    /// Address 2 has the first non op return output
    #[wasm_bindgen_test]
    fn no_edict_pointer_none() {
        clear();
        edict_test(
            RunesTestingConfig::default_with_pointer(None),
            None,
            None,
            0,
            1000,
        );
    }

    /// No edict, all amount should go to pointer, which is the runestone to distribute runes evenly
    #[wasm_bindgen_test]
    fn no_edict_pointer_burn() {
        clear();
        edict_test(
            RunesTestingConfig::default_with_pointer(Some(2)),
            None,
            None,
            0,
            0,
        );
    }

    /// No edict, all amount should go to pointer, which is equal to the number
    /// of transaction outputs. This is a cenotaph since the ordinals crate
    /// only considers pointer < number tx outputs valid
    #[wasm_bindgen_test]
    fn no_edict_pointer_cenotaph() {
        clear();
        edict_test(
            RunesTestingConfig::default_with_pointer(Some(3)),
            None,
            None,
            0,
            0,
        );
    }
}
