#[cfg(test)]
mod tests {
    use crate::balance_sheet::{load_sheet, PersistentRecord};
    use crate::message::{MessageContext, MessageContextParcel};
    use crate::protostone::Protostones;
    use crate::test_helpers::{self as helpers, get_address, ADDRESS1};
    use crate::{tables, Protorune};
    use anyhow::{anyhow, Result};
    use bitcoin::Transaction;
    use bitcoin::{
        address::NetworkChecked, Address, Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut,
        Witness,
    };
    use metashrew::index_pointer::{AtomicPointer, IndexPointer};
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
    use protorune_support::protostone::Protostone;
    use protorune_support::rune_transfer::RuneTransfer;
    use protorune_support::utils::consensus_encode;

    #[allow(unused_imports)]
    use metashrew::{
        println,
        stdio::{stdout, Write},
    };

    use helpers::clear;
    use metashrew_support::index_pointer::KeyValuePointer;
    use ordinals::{Etching, Rune, Runestone};
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    struct ForwardAll(());
    struct MixedForwarding(());
    struct FullRefund(());
    struct FullRefundWithErr(());
    struct OverForward(());
    struct MintNewProtorune(());
    struct OverStoreInRuntime(());

    struct ModifyAtomicWithoutErr(());
    struct ModifyAtomicThenErr(());

    struct MixedForwardingStaticRuntime(());

    impl MessageContext for ForwardAll {
        fn protocol_tag() -> u128 {
            122
        }
        // takes half of the first runes balance
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let runes: Vec<RuneTransfer> = parcel.runes.clone();
            // transfer protorunes to the pointer
            Ok((runes, BalanceSheet::default()))
        }
    }
    impl MessageContext for MixedForwarding {
        fn protocol_tag() -> u128 {
            122
        }
        /// quarter forward, eighth store in runtime, rest refund
        /// only does it for the first input
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let mut new_runtime_balances = parcel.runtime_balances.clone();
            let transfer = RuneTransfer {
                id: parcel.runes[0].id,
                value: parcel.runes[0].value / 4,
            };

            let transfer_to_runtime = RuneTransfer {
                id: parcel.runes[0].id,
                value: parcel.runes[0].value / 8,
            };
            println!("{:?}", transfer_to_runtime);
            <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(vec![
                transfer_to_runtime,
            ])?
            .pipe(&mut new_runtime_balances);
            Ok((vec![transfer], *new_runtime_balances))
        }
    }
    impl MessageContext for OverForward {
        fn protocol_tag() -> u128 {
            122
        }
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let transfer = RuneTransfer {
                id: parcel.runes[0].id,
                value: parcel.runes[0].value + 1,
            };
            Ok((vec![transfer], BalanceSheet::default()))
        }
    }
    impl MessageContext for MintNewProtorune {
        fn protocol_tag() -> u128 {
            122
        }
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let _ = parcel;
            let transfer = RuneTransfer {
                id: parcel.runes[0].id,
                value: parcel.runes[0].value / 2 - 1,
            };
            let mint_rune = ProtoruneRuneId::new(840000, 999);
            let mint = RuneTransfer {
                id: mint_rune,
                value: 10001,
            };
            let mut runtime = BalanceSheet::default();
            runtime.increase(&mint_rune, 12345);
            Ok((vec![transfer, mint], runtime))
        }
    }
    impl MessageContext for OverStoreInRuntime {
        fn protocol_tag() -> u128 {
            122
        }
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let mut new_runtime_balances = parcel.runtime_balances.clone();
            let transfer = RuneTransfer {
                id: parcel.runes[0].id,
                value: parcel.runes[0].value,
            };

            let transfer_to_runtime = RuneTransfer {
                id: parcel.runes[0].id,
                value: 1,
            };
            <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(vec![
                transfer_to_runtime,
            ])?
            .pipe(&mut new_runtime_balances);
            Ok((vec![transfer], *new_runtime_balances))
        }
    }
    impl MessageContext for FullRefund {
        fn protocol_tag() -> u128 {
            122
        }
        fn handle(
            _parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            Ok((vec![], BalanceSheet::default()))
        }
    }
    impl MessageContext for FullRefundWithErr {
        fn protocol_tag() -> u128 {
            122
        }
        fn handle(
            _parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            Err(anyhow!("full refund"))
        }
    }

    impl MessageContext for ModifyAtomicWithoutErr {
        fn protocol_tag() -> u128 {
            122
        }
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let transfer = RuneTransfer {
                id: parcel.runes[0].id,
                value: 50,
            };
            let bs = <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(vec![
                transfer,
            ])?;
            bs.save(
                &mut parcel
                    .atomic
                    .derive(&tables::RuneTable::for_protocol(122).CAP),
                false,
            );

            Ok((vec![], BalanceSheet::default()))
        }
    }
    impl MessageContext for ModifyAtomicThenErr {
        fn protocol_tag() -> u128 {
            122
        }
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let transfer = RuneTransfer {
                id: parcel.runes[0].id,
                value: 50,
            };
            let bs = <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(vec![
                transfer,
            ])?;
            bs.save(
                &mut parcel
                    .atomic
                    .derive(&tables::RuneTable::for_protocol(122).CAP),
                false,
            );
            Err(anyhow!("full refund"))
        }
    }

    impl MessageContext for MixedForwardingStaticRuntime {
        fn protocol_tag() -> u128 {
            122
        }
        /// quarter forward, eighth store in runtime, rest refund
        /// only does it for the first input
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let mut new_runtime_balances = BalanceSheet::default();
            let transfer = RuneTransfer {
                id: parcel.runes[0].id,
                value: parcel.runes[0].value / 4,
            };

            let transfer_to_runtime = RuneTransfer {
                id: parcel.runes[0].id,
                value: parcel.runes[0].value / 8,
            };
            <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(vec![
                transfer_to_runtime,
            ])?
            .pipe(&mut new_runtime_balances);
            Ok((vec![transfer], new_runtime_balances))
        }
    }

    fn protomessage_from_protoburn_fixture(protocol_id: u128) -> bitcoin::Block {
        let previous_output = OutPoint {
            txid: bitcoin::Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            vout: 0,
        };
        let input_script = ScriptBuf::new();

        let txin = TxIn {
            previous_output,
            script_sig: input_script,
            sequence: Sequence::MAX,
            witness: Witness::new(),
        };

        let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());

        let script_pubkey = address.script_pubkey();

        let txout = TxOut {
            value: Amount::from_sat(100_000_000),
            script_pubkey,
        };

        let runestone: ScriptBuf = (Runestone {
            etching: Some(Etching {
                divisibility: Some(2),
                premine: Some(1000),
                rune: Some(Rune::from_str("TESTTESTTESTTEST").unwrap()),
                spacers: Some(0),
                symbol: Some(char::from_str("A").unwrap()),
                turbo: true,
                terms: None,
            }),
            pointer: Some(1), // points to the OP_RETURN, so therefore targets the protoburn
            edicts: Vec::new(),
            mint: None,
            protocol: match vec![
                Protostone {
                    burn: Some(protocol_id),
                    edicts: vec![],
                    pointer: Some(4), // output 0 is the spendable outputs, output 1 is the op_return, output 2 is reserved, output 3 is the protoburn, so output 4 is the protomessage
                    refund: None,
                    from: None,
                    protocol_tag: 13, // this value must be 13 if protoburn
                    message: vec![],
                },
                Protostone {
                    // protomessage which should transfer protorunes to the pointer
                    message: vec![1u8],
                    pointer: Some(0),
                    refund: Some(0),
                    edicts: vec![],
                    from: None,
                    burn: None,
                    protocol_tag: protocol_id as u128,
                },
            ]
            .encipher()
            {
                Ok(v) => Some(v),
                Err(_) => None,
            },
        })
        .encipher();

        // op return is at output 1
        let op_return = TxOut {
            value: Amount::from_sat(0),
            script_pubkey: runestone,
        };

        helpers::create_block_with_txs(vec![Transaction {
            version: bitcoin::transaction::Version(2),
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: vec![txout, op_return],
        }])
    }

    fn protomessage_from_edict_fixture(protocol_id: u128, block_height: u128) -> bitcoin::Block {
        let first_mock_output = OutPoint {
            txid: bitcoin::Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            vout: 0,
        };

        let protoburn_tx =
            helpers::create_default_protoburn_transaction(first_mock_output, protocol_id);
        let _protorune_id = ProtoruneRuneId {
            block: block_height as u128,
            tx: 0,
        };

        // output 0 holds all the protorunes
        let protoburn_input = OutPoint {
            txid: protoburn_tx.compute_txid(),
            vout: 0,
        };

        let protomessage_tx =
            helpers::create_protomessage_from_edict_tx(protoburn_input, protocol_id, vec![]);

        helpers::create_block_with_txs(vec![protoburn_tx, protomessage_tx])
    }

    /// protomessage in the same transaction as a protoburn
    /// The protoburn will target the protomessage and directly transfer to it
    #[wasm_bindgen_test]
    fn protomessage_same_tx_as_protoburn_test() {
        clear();
        let block_height = 840000;
        let protocol_id = 122;

        let test_block = protomessage_from_protoburn_fixture(protocol_id);

        assert!(
            Protorune::index_block::<ForwardAll>(test_block.clone(), block_height as u64).is_ok()
        );

        let outpoint_address: OutPoint = OutPoint {
            txid: test_block.txdata[0].compute_txid(),
            vout: 0,
        };
        // check runes balance
        let sheet = load_sheet(
            &tables::RUNES
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_address).unwrap()),
        );

        let protorunes_sheet = load_sheet(
            &tables::RuneTable::for_protocol(protocol_id.into())
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_address).unwrap()),
        );

        let protorune_id = ProtoruneRuneId {
            block: block_height as u128,
            tx: 0,
        };
        let stored_runes_balance = sheet.get_cached(&protorune_id);
        assert_eq!(stored_runes_balance, 0);

        let stored_protorune_balance = protorunes_sheet.get_cached(&protorune_id);
        assert_eq!(stored_protorune_balance, 1000);
    }

    fn protomessage_from_edict_test_template<T: MessageContext>(
        expected_pointer_amount: u128,
        expected_refunded_amount: u128,
        expected_runtime_amount: u128,
    ) -> (
        BalanceSheet<IndexPointer>,
        BalanceSheet<IndexPointer>,
        BalanceSheet<IndexPointer>,
    ) {
        clear();
        let block_height = 840000;
        let protocol_id = 122;

        let test_block = protomessage_from_edict_fixture(protocol_id, block_height);
        let protorune_id = ProtoruneRuneId {
            block: block_height as u128,
            tx: 0,
        };

        assert!(Protorune::index_block::<T>(test_block.clone(), block_height as u64).is_ok());
        // print_cache();
        // tx 0 is protoburn, tx 1 is protomessage
        let outpoint_address0: OutPoint = OutPoint {
            txid: test_block.txdata[1].compute_txid(),
            vout: 0,
        };
        let outpoint_address1: OutPoint = OutPoint {
            txid: test_block.txdata[1].compute_txid(),
            vout: 1,
        };
        // check runes balance
        let sheet = load_sheet(
            &tables::RUNES
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_address0).unwrap()),
        );

        let protorunes_sheet0 = load_sheet(
            &tables::RuneTable::for_protocol(protocol_id.into())
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_address0).unwrap()),
        );
        let protorunes_sheet1 = load_sheet(
            &tables::RuneTable::for_protocol(protocol_id.into())
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_address1).unwrap()),
        );
        let protorunes_sheet_runtime =
            load_sheet(&tables::RuneTable::for_protocol(protocol_id.into()).RUNTIME_BALANCE);

        let stored_runes_balance = sheet.get_cached(&protorune_id);
        assert_eq!(stored_runes_balance, 0);

        let stored_protorune_balance0 = protorunes_sheet0.get_cached(&protorune_id);
        assert_eq!(stored_protorune_balance0, expected_pointer_amount);
        let stored_protorune_balance1 = protorunes_sheet1.get_cached(&protorune_id);
        assert_eq!(stored_protorune_balance1, expected_refunded_amount);
        let stored_protorune_balance_runtime = protorunes_sheet_runtime.get_cached(&protorune_id);
        assert_eq!(stored_protorune_balance_runtime, expected_runtime_amount);

        return (
            protorunes_sheet0,
            protorunes_sheet1,
            protorunes_sheet_runtime,
        );
    }

    fn protomessage_from_edict_multiple_protomessages_test_template<T: MessageContext>(
        expected_pointer_amount: u128,
        expected_refunded_amount: u128,
        expected_runtime_amount: u128,
    ) {
        clear();

        let protocol_id = 122;
        let block_height = 840001;

        let mut test_block = protomessage_from_edict_fixture(protocol_id, block_height);
        let protorune_id = ProtoruneRuneId {
            block: block_height,
            tx: 0,
        };

        // pointer_outpoint is the pointer that received the forwarded runes during the first protomessage
        let pointer_outpoint = OutPoint {
            txid: test_block.txdata[1].compute_txid(),
            vout: 0,
        };
        let protomessage_tx2 =
            helpers::create_protomessage_from_edict_tx(pointer_outpoint, protocol_id, vec![]);

        test_block.txdata.push(protomessage_tx2);

        assert!(Protorune::index_block::<T>(test_block.clone(), block_height as u64).is_ok());

        let protorunes_sheet0 = load_sheet(
            &tables::RuneTable::for_protocol(protocol_id.into())
                .OUTPOINT_TO_RUNES
                .select(
                    &consensus_encode(&OutPoint {
                        txid: test_block.txdata[2].compute_txid(),
                        vout: 0,
                    })
                    .unwrap(),
                ),
        );
        let protorunes_sheet1 = load_sheet(
            &tables::RuneTable::for_protocol(protocol_id.into())
                .OUTPOINT_TO_RUNES
                .select(
                    &consensus_encode(&OutPoint {
                        txid: test_block.txdata[2].compute_txid(),
                        vout: 1,
                    })
                    .unwrap(),
                ),
        );
        let protorunes_sheet_runtime =
            load_sheet(&tables::RuneTable::for_protocol(protocol_id.into()).RUNTIME_BALANCE);

        let stored_protorune_balance0 = protorunes_sheet0.get_cached(&protorune_id);
        assert_eq!(stored_protorune_balance0, expected_pointer_amount);
        let stored_protorune_balance_runtime = protorunes_sheet_runtime.get_cached(&protorune_id);
        assert_eq!(stored_protorune_balance_runtime, expected_runtime_amount);
        let stored_protorune_balance1 = protorunes_sheet1.get_cached(&protorune_id);
        assert_eq!(stored_protorune_balance1, expected_refunded_amount);
    }

    /// protomessage from edict
    /// The first transaction is a protoburn. The next transaction is a protostone that
    /// has an edict that targets the protomessage
    #[wasm_bindgen_test]
    fn protomessage_from_edict_test() {
        protomessage_from_edict_test_template::<ForwardAll>(1000, 0, 0);
    }

    /// Tests that a message context that forwards 1/4, sends 1/8 to runtime, and leaves the rest unaccounted will have the correct values
    #[wasm_bindgen_test]
    fn protomessage_mixed_forwarding_test() {
        protomessage_from_edict_test_template::<MixedForwarding>(250, 625, 125);
    }

    /// Tests that a message context that returns nothing will refund all
    #[wasm_bindgen_test]
    fn protomessage_full_refund_test() {
        protomessage_from_edict_test_template::<FullRefund>(0, 1000, 0);
    }

    /// Tests that a message context that returns an invalid result will refund all
    #[wasm_bindgen_test]
    fn protomessage_full_refund_using_err_test() {
        protomessage_from_edict_test_template::<FullRefundWithErr>(0, 1000, 0);
    }

    /// Tests that overallocating in handle will refund all
    #[wasm_bindgen_test]
    fn protomessage_overallocation_test() {
        protomessage_from_edict_test_template::<OverForward>(0, 1000, 0);
        protomessage_from_edict_test_template::<OverStoreInRuntime>(0, 1000, 0);
    }

    /// Tests that overallocating an allowed mintable protorune will
    /// mint the new protorune
    #[wasm_bindgen_test]
    fn protomessage_mint_allowed_protorune_test() {
        let (protorunes_sheet0, protorunes_sheet1, protorunes_sheet_runtime) =
            protomessage_from_edict_test_template::<MintNewProtorune>(499, 501, 0);
        let minted_protorune = ProtoruneRuneId::new(840000, 999);

        let minted_protorune_to_pointer = protorunes_sheet0.get_cached(&minted_protorune);
        assert_eq!(minted_protorune_to_pointer, 10001);
        let minted_protorune_to_runtime = protorunes_sheet_runtime.get_cached(&minted_protorune);
        assert_eq!(minted_protorune_to_runtime, 12345);
        let minted_protorune_to_refund = protorunes_sheet1.get_cached(&minted_protorune);
        assert_eq!(minted_protorune_to_refund, 0);
    }

    /// Tests that the atomic pointer is not rolled back in an Ok
    #[wasm_bindgen_test]
    fn protomessage_modify_atomic_then_ok_test() {
        protomessage_from_edict_test_template::<ModifyAtomicWithoutErr>(0, 1000, 0);

        let block_height = 840000;
        let protocol_id = 122;

        let protorune_id = ProtoruneRuneId {
            block: block_height as u128,
            tx: 0,
        };
        let bs = load_sheet(&tables::RuneTable::for_protocol(protocol_id as u128).CAP);

        let amount = bs.get_cached(&protorune_id);
        assert_eq!(amount, 50);
    }

    /// Tests that the atomic pointer is rolled back in an Err
    #[wasm_bindgen_test]
    fn protomessage_modify_atomic_then_err_test() {
        protomessage_from_edict_test_template::<ModifyAtomicThenErr>(0, 1000, 0);

        let block_height = 840000;
        let protocol_id = 122;

        let protorune_id = ProtoruneRuneId {
            block: block_height as u128,
            tx: 0,
        };
        let bs = load_sheet(&tables::RuneTable::for_protocol(protocol_id as u128).CAP);

        let amount = bs.get_cached(&protorune_id);
        assert_eq!(amount, 0);
    }

    #[wasm_bindgen_test]
    fn protomessage_existing_runtime_balance_test() {
        // first protomessage transfers 1/4 to pointer: 250
        // there are 250 protorunes as input.
        // there is 125 protorunes in the runtime balance.

        // pointer should get 250/4 = 62
        // the runtime balance is set to 125 + 250/8 = 156
        // the refunded amount should be (125+250) - (62+156) = 157
        protomessage_from_edict_multiple_protomessages_test_template::<MixedForwarding>(
            62, 157, 156,
        )
    }

    #[wasm_bindgen_test]
    fn protomessage_decrease_existing_runtime_balance_test() {
        // This test does not use the existing runtime balance in the handle()
        // there are 250 runes as input.
        // there is 125 runes in the runtime balance.

        // pointer should get 250/4 = 62
        // the runtime balance is set to 250/8 = 31
        // the refunded amount should be (125+250) - (62+31) = 282
        protomessage_from_edict_multiple_protomessages_test_template::<MixedForwardingStaticRuntime>(
            62, 282, 31,
        );
    }
}
