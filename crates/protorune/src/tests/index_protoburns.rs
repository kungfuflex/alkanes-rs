#[cfg(test)]
mod tests {
    use crate::balance_sheet::load_sheet;
    use crate::message::{MessageContext, MessageContextParcel};
    use crate::test_helpers::{self as helpers};
    use crate::{tables, Protorune};
    use anyhow::Result;
    use bitcoin::{OutPoint, Transaction};
    use metashrew::index_pointer::AtomicPointer;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
    use protorune_support::proto::{self, protorune};
    use protorune_support::protostone::{Protostone, ProtostoneEdict};
    use protorune_support::rune_transfer::RuneTransfer;
    use protorune_support::utils::consensus_encode;

    use helpers::clear;
    use metashrew::{
        println,
        stdio::{stdout, Write},
    };
    use metashrew_support::index_pointer::KeyValuePointer;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    static PROTOCOL_ID: u128 = 122;
    static BLOCK_HEIGHT: u32 = 840000;

    struct TestMessageContext(());

    impl MessageContext for TestMessageContext {
        fn protocol_tag() -> u128 {
            PROTOCOL_ID
        }
        // takes half of the first runes balance
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let mut new_runtime_balances = parcel.runtime_balances.clone();
            let mut runes = parcel.runes.clone();
            runes[0].value = runes[0].value / 2;
            let transfer = runes[0].clone();
            <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(runes)?
                .pipe(&mut new_runtime_balances);
            // transfer protorunes to the pointer
            Ok((vec![transfer], *new_runtime_balances))
        }
    }

    /// In one runestone, etches a rune, then protoburns it
    #[wasm_bindgen_test]
    fn protoburn_test() {
        clear();
        let mut test_block = helpers::create_block_with_coinbase_tx(BLOCK_HEIGHT);

        let previous_output = OutPoint {
            txid: bitcoin::Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            vout: 0,
        };

        let protoburn_tx =
            helpers::create_default_protoburn_transaction(previous_output, PROTOCOL_ID);

        test_block.txdata.push(protoburn_tx);
        assert!(Protorune::index_block::<TestMessageContext>(
            test_block.clone(),
            BLOCK_HEIGHT as u64
        )
        .is_ok());

        // tx 0 is coinbase, tx 1 is runestone
        let outpoint_address: OutPoint = OutPoint {
            txid: test_block.txdata[1].compute_txid(),
            vout: 0,
        };
        // check runes balance
        let sheet = load_sheet(
            &tables::RUNES
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_address).unwrap()),
        );

        let protorunes_sheet = load_sheet(
            &tables::RuneTable::for_protocol(PROTOCOL_ID.into())
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_address).unwrap()),
        );

        let protorune_id = ProtoruneRuneId {
            block: BLOCK_HEIGHT as u128,
            tx: 1,
        };
        // let v: Vec<u8> = protorune_id.into();
        let stored_balance_address = sheet.get_cached(&protorune_id);
        assert_eq!(stored_balance_address, 0);
        let stored_protorune_balance = protorunes_sheet.get_cached(&protorune_id);
        assert_eq!(stored_protorune_balance, 1000);
    }

    fn protostone_transfer_test_template(
        output_protostone_pointer: u32,
        protostone_edicts: Vec<ProtostoneEdict>,
    ) -> bitcoin::Block {
        clear();
        // tx0: coinbase
        let mut test_block = helpers::create_block_with_coinbase_tx(BLOCK_HEIGHT);
        let previous_output = helpers::get_mock_outpoint(0);

        // tx1: protoburn. This also etches the rune, immediately protoburns
        let protoburn_tx =
            helpers::create_default_protoburn_transaction(previous_output, PROTOCOL_ID);
        test_block.txdata.push(protoburn_tx.clone());

        let previous_output = OutPoint {
            txid: protoburn_tx.clone().compute_txid(),
            vout: 0,
        };

        // tx2: protostone edicts
        // output 0 is a valid utxo. output 1 is the runestone with protostone
        let transfer_tx = helpers::create_protostone_transaction(
            previous_output,
            None,
            false,
            1,
            output_protostone_pointer,
            PROTOCOL_ID,
            protostone_edicts,
        );
        test_block.txdata.push(transfer_tx);

        assert!(Protorune::index_block::<TestMessageContext>(
            test_block.clone(),
            BLOCK_HEIGHT as u64
        )
        .is_ok());

        // tx 0 is coinbase, tx 1 is protoburn, tx 2 is transfer
        let tx1_outpoint: OutPoint = OutPoint {
            txid: test_block.txdata[1].compute_txid(),
            vout: 0,
        };
        let protorune_id = ProtoruneRuneId {
            block: BLOCK_HEIGHT as u128,
            tx: 1,
        };
        // check runes balance
        let protoburn_rune_balances =
            helpers::get_rune_balance_by_outpoint(tx1_outpoint, vec![protorune_id]);
        assert_eq!(protoburn_rune_balances[0], 0);

        // ensures protorunes from tx1 outpoint are no longer usable
        let protoburn_protorunes_balances = helpers::get_protorune_balance_by_outpoint(
            PROTOCOL_ID,
            tx1_outpoint,
            vec![protorune_id],
        );
        assert_eq!(protoburn_protorunes_balances[0], 0);

        return test_block;
    }

    #[wasm_bindgen_test]
    fn protoburn_pointer_test() {
        // transfer to the valid utxo at pointer 0
        let test_block = protostone_transfer_test_template(0, vec![]);
        let tx2_outpoint: OutPoint = OutPoint {
            txid: test_block.txdata[2].compute_txid(),
            vout: 0,
        };
        // the only valid protorune id
        let protorune_id = ProtoruneRuneId {
            block: BLOCK_HEIGHT as u128,
            tx: 1,
        };

        let tx2_rune_balances =
            helpers::get_rune_balance_by_outpoint(tx2_outpoint, vec![protorune_id]);
        assert_eq!(tx2_rune_balances[0], 0);

        let protoburn_protorunes_balances = helpers::get_protorune_balance_by_outpoint(
            PROTOCOL_ID,
            tx2_outpoint,
            vec![protorune_id],
        );
        assert_eq!(protoburn_protorunes_balances[0], 1000);
    }

    #[wasm_bindgen_test]
    fn protoburn_pointer_cenotaph_test() {
        // transfer to the special vout 2 == num outputs, which should be a cenotaph
        let test_block = protostone_transfer_test_template(2, vec![]);
        let tx2_outpoint: OutPoint = OutPoint {
            txid: test_block.txdata[2].compute_txid(),
            vout: 0,
        };
        // the only valid protorune id
        let protorune_id = ProtoruneRuneId {
            block: BLOCK_HEIGHT as u128,
            tx: 1,
        };

        let tx2_rune_balances =
            helpers::get_rune_balance_by_outpoint(tx2_outpoint, vec![protorune_id]);
        assert_eq!(tx2_rune_balances[0], 0);

        let protoburn_protorunes_balances = helpers::get_protorune_balance_by_outpoint(
            PROTOCOL_ID,
            tx2_outpoint,
            vec![protorune_id],
        );
        assert_eq!(protoburn_protorunes_balances[0], 0);
    }

    #[wasm_bindgen_test]
    #[allow(non_snake_case)]
    fn protoburn_pointer_to_OP_RETURN() {
        // transfer to the OP_RETURN (the runestone) at vout 1
        let test_block = protostone_transfer_test_template(1, vec![]);
        let tx2_outpoint: OutPoint = OutPoint {
            txid: test_block.txdata[2].compute_txid(),
            vout: 0,
        };
        // the only valid protorune id
        let protorune_id = ProtoruneRuneId {
            block: BLOCK_HEIGHT as u128,
            tx: 1,
        };

        let tx2_rune_balances =
            helpers::get_rune_balance_by_outpoint(tx2_outpoint, vec![protorune_id]);
        assert_eq!(tx2_rune_balances[0], 0);

        let protoburn_protorunes_balances = helpers::get_protorune_balance_by_outpoint(
            PROTOCOL_ID,
            tx2_outpoint,
            vec![protorune_id],
        );
        assert_eq!(protoburn_protorunes_balances[0], 0);
    }

    #[wasm_bindgen_test]
    fn protostone_edict_test() {
        // the only valid protorune id
        let protorune_id = ProtoruneRuneId {
            block: BLOCK_HEIGHT as u128,
            tx: 1,
        };

        // transfer all remaining protorunes to the op return to burn it
        let test_block = protostone_transfer_test_template(
            1,
            vec![ProtostoneEdict {
                id: protorune_id,
                amount: 222,
                output: 0,
            }],
        );
        let tx2_outpoint: OutPoint = OutPoint {
            txid: test_block.txdata[2].compute_txid(),
            vout: 0,
        };

        let tx2_rune_balances =
            helpers::get_rune_balance_by_outpoint(tx2_outpoint, vec![protorune_id]);
        assert_eq!(tx2_rune_balances[0], 0);

        let protoburn_protorunes_balances = helpers::get_protorune_balance_by_outpoint(
            PROTOCOL_ID,
            tx2_outpoint,
            vec![protorune_id],
        );
        assert_eq!(protoburn_protorunes_balances[0], 222);
    }

    #[wasm_bindgen_test]
    fn protostone_edict_burn_test() {
        // the only valid protorune id
        let protorune_id = ProtoruneRuneId {
            block: BLOCK_HEIGHT as u128,
            tx: 1,
        };

        // transfer all remaining protorunes to the op return to burn it
        let test_block = protostone_transfer_test_template(
            1,
            vec![ProtostoneEdict {
                id: protorune_id,
                amount: 222,
                output: 1,
            }],
        );
        let tx2_outpoint: OutPoint = OutPoint {
            txid: test_block.txdata[2].compute_txid(),
            vout: 0,
        };

        let tx2_rune_balances =
            helpers::get_rune_balance_by_outpoint(tx2_outpoint, vec![protorune_id]);
        assert_eq!(tx2_rune_balances[0], 0);

        let protoburn_protorunes_balances = helpers::get_protorune_balance_by_outpoint(
            PROTOCOL_ID,
            tx2_outpoint,
            vec![protorune_id],
        );
        assert_eq!(protoburn_protorunes_balances[0], 0);
    }

    #[wasm_bindgen_test]
    fn protostone_edict_even_distribution_test() {
        // the only valid protorune id
        let protorune_id = ProtoruneRuneId {
            block: BLOCK_HEIGHT as u128,
            tx: 1,
        };

        // transfer all remaining protorunes to the op return to burn it
        // should split the edict evenly between the non op return outputs
        let test_block = protostone_transfer_test_template(
            1,
            vec![ProtostoneEdict {
                id: protorune_id,
                amount: 222,
                output: 2,
            }],
        );
        let tx2_outpoint: OutPoint = OutPoint {
            txid: test_block.txdata[2].compute_txid(),
            vout: 0,
        };

        let tx2_rune_balances =
            helpers::get_rune_balance_by_outpoint(tx2_outpoint, vec![protorune_id]);
        assert_eq!(tx2_rune_balances[0], 0);

        let protoburn_protorunes_balances = helpers::get_protorune_balance_by_outpoint(
            PROTOCOL_ID,
            tx2_outpoint,
            vec![protorune_id],
        );
        assert_eq!(protoburn_protorunes_balances[0], 222);
    }

    // TODO: Add more integration tests https://github.com/kungfuflex/alkanes-rs/issues/9
}
