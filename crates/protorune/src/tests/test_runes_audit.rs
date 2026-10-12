// Regression tests for the Halborn raw-Rune (RUNES table, tag 0) findings.
// These fixes are ungated (apply from genesis).
#[cfg(test)]
mod tests {
    use crate::message::MessageContext;
    use crate::test_helpers::{self as helpers, ADDRESS1, ADDRESS2};
    use crate::{message::MessageContextParcel, tables, Protorune};
    use anyhow::Result;
    use bitcoin::{Amount, OutPoint, ScriptBuf, Transaction, TxOut};
    use helpers::clear;
    use metashrew_core::index_pointer::AtomicPointer;
    use metashrew_support::index_pointer::KeyValuePointer;
    use ordinals::{Artifact, Etching, Rune, RuneId, Runestone, Terms};
    use protorune_support::protostone::Protostone;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
    use protorune_support::rune_transfer::RuneTransfer;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    struct MyMessageContext(());

    impl MessageContext for MyMessageContext {
        fn handle(
            _parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            Ok((vec![], BalanceSheet::default()))
        }
        fn protocol_tag() -> u128 {
            100
        }
    }

    const ETCH_HEIGHT: u64 = 840_000;

    fn index(txs: Vec<Transaction>, height: u64) {
        Protorune::index_block::<MyMessageContext>(helpers::create_block_with_txs(txs), height)
            .unwrap();
    }

    fn mints_remaining(id: RuneId) -> u128 {
        let id: ProtoruneRuneId = id.into();
        let name = tables::RUNES.RUNE_ID_TO_ETCHING.select(&id.into()).get();
        tables::RUNES.MINTS_REMAINING.select(&name).get_value()
    }

    fn balance(tx: &Transaction, vout: u32, id: RuneId) -> u128 {
        helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: tx.compute_txid(),
                vout,
            },
            vec![id.into()],
        )[0]
    }

    /// Etches a rune (premine 1000 to vout 0) with the given cap, amount 100,
    /// mintable from the etching height on.
    fn etch(name: &str, cap: u128, input: u32) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(0),
                    premine: Some(1000),
                    rune: Some(Rune::from_str(name).unwrap()),
                    spacers: None,
                    symbol: Some('R'),
                    turbo: true,
                    terms: Some(Terms {
                        amount: Some(100),
                        cap: Some(cap),
                        height: (None, None),
                        offset: (None, None),
                    }),
                }),
                pointer: Some(0),
                ..Default::default()
            },
            vec![helpers::get_mock_txin(input)],
            vec![helpers::get_txout_transfer_to_address(&ADDRESS1(), 10_000)],
        )
    }

    fn mint_tx(id: RuneId, input: u32, protocol: Option<Vec<u128>>) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                mint: Some(id),
                pointer: Some(0),
                protocol,
                ..Default::default()
            },
            vec![helpers::get_mock_txin(input)],
            vec![helpers::get_txout_transfer_to_address(&ADDRESS2(), 10_000)],
        )
    }

    // Protocol bytes (LE, 15 per u128) = varints [tag 1, length 100, 0 x13]:
    // the protostone claims 100 values that are not there. The outer runestone
    // is valid, but Protostone::decipher fails in index_protostones.
    fn malformed_protocol() -> Option<Vec<u128>> {
        Some(vec![0x6401])
    }

    fn etched_id() -> RuneId {
        RuneId {
            block: ETCH_HEIGHT,
            tx: 0,
        }
    }

    // ---- Rejected Mint Transactions Consume Remaining Cap Without Issuance ----

    #[wasm_bindgen_test]
    fn clean_mint_consumes_cap_once_and_issues() {
        clear();
        index(vec![etch("AAAAAAAAAAAAAATOMIC", 2, 0)], ETCH_HEIGHT);
        let id = etched_id();
        assert_eq!(mints_remaining(id), 2);
        let m = mint_tx(id, 1, None);
        index(vec![m.clone()], ETCH_HEIGHT + 1);
        assert_eq!(mints_remaining(id), 1);
        assert_eq!(balance(&m, 0, id), 100);
    }

    #[wasm_bindgen_test]
    fn rejected_mints_do_not_consume_cap() {
        clear();
        index(vec![etch("AAAAAAAAAAAAAATOMIC", 2, 0)], ETCH_HEIGHT);
        let id = etched_id();

        // two malformed-nested-protocol mints: rolled back, issue nothing, keep the cap
        let bad1 = mint_tx(id, 1, malformed_protocol());
        let bad2 = mint_tx(id, 2, malformed_protocol());
        match Runestone::decipher(&bad1) {
            Some(Artifact::Runestone(ref rs)) => {
                assert!(rs.mint.is_some());
                assert!(Protostone::from_runestone(rs).is_err());
            }
            other => panic!("expected a valid outer runestone, got {:?}", other),
        }
        index(vec![bad1.clone()], ETCH_HEIGHT + 1);
        assert_eq!(mints_remaining(id), 2);
        assert_eq!(balance(&bad1, 0, id), 0);
        index(vec![bad2.clone()], ETCH_HEIGHT + 2);
        assert_eq!(mints_remaining(id), 2);
        assert_eq!(balance(&bad2, 0, id), 0);

        // the full cap is still available for valid mints
        let ok1 = mint_tx(id, 3, None);
        let ok2 = mint_tx(id, 4, None);
        let over = mint_tx(id, 5, None);
        index(vec![ok1.clone(), ok2.clone(), over.clone()], ETCH_HEIGHT + 3);
        assert_eq!(balance(&ok1, 0, id), 100);
        assert_eq!(balance(&ok2, 0, id), 100);
        assert_eq!(balance(&over, 0, id), 0);
        assert_eq!(mints_remaining(id), 0);
    }

    #[wasm_bindgen_test]
    fn rejected_mint_in_same_block_as_valid_mint() {
        clear();
        index(vec![etch("AAAAAAAAAAAAAATOMIC", 1, 0)], ETCH_HEIGHT);
        let id = etched_id();
        let bad = mint_tx(id, 1, malformed_protocol());
        let ok = mint_tx(id, 2, None);
        index(vec![bad.clone(), ok.clone()], ETCH_HEIGHT + 1);
        assert_eq!(balance(&bad, 0, id), 0);
        assert_eq!(balance(&ok, 0, id), 100);
        assert_eq!(mints_remaining(id), 0);
    }

    #[wasm_bindgen_test]
    fn runestone_cannot_mint_the_rune_it_etches() {
        clear();
        let tx = helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(0),
                    premine: Some(1000),
                    rune: Some(Rune::from_str("AAAAAAAAAAAAAATOMIC").unwrap()),
                    spacers: None,
                    symbol: Some('R'),
                    turbo: true,
                    terms: Some(Terms {
                        amount: Some(100),
                        cap: Some(1),
                        height: (None, None),
                        offset: (None, None),
                    }),
                }),
                mint: Some(etched_id()),
                pointer: Some(0),
                ..Default::default()
            },
            vec![helpers::get_mock_txin(0)],
            vec![helpers::get_txout_transfer_to_address(&ADDRESS1(), 10_000)],
        );
        index(vec![tx.clone()], ETCH_HEIGHT);
        assert_eq!(balance(&tx, 0, etched_id()), 1000);
        assert_eq!(mints_remaining(etched_id()), 1);
    }

    // ---- Ordinary Spends Without a Runestone Erase Rune Balances ----

    fn plain_tx(inputs: Vec<OutPoint>, outputs: Vec<TxOut>) -> Transaction {
        Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: inputs
                .into_iter()
                .map(helpers::get_txin_from_outpoint)
                .collect(),
            output: outputs,
        }
    }

    fn op_return() -> TxOut {
        TxOut {
            value: Amount::from_sat(0),
            script_pubkey: ScriptBuf::new_op_return(&[0x42u8; 4]),
        }
    }

    fn to(addr: String) -> TxOut {
        helpers::get_txout_transfer_to_address(&addr, 10_000)
    }

    fn outpoint(tx: &Transaction, vout: u32) -> OutPoint {
        OutPoint {
            txid: tx.compute_txid(),
            vout,
        }
    }

    fn etch_premine() -> (Transaction, RuneId) {
        clear();
        let e = etch("AAAAAAAAAAAAAATOMIC", 1, 0);
        index(vec![e.clone()], ETCH_HEIGHT);
        assert_eq!(balance(&e, 0, etched_id()), 1000);
        (e, etched_id())
    }

    #[wasm_bindgen_test]
    fn no_runestone_spend_moves_runes_to_first_output() {
        let (e, id) = etch_premine();
        let spend = plain_tx(vec![outpoint(&e, 0)], vec![to(ADDRESS2()), to(ADDRESS1())]);
        assert!(Runestone::decipher(&spend).is_none());
        index(vec![spend.clone()], ETCH_HEIGHT + 1);
        assert_eq!(balance(&e, 0, id), 0);
        assert_eq!(balance(&spend, 0, id), 1000);
        assert_eq!(balance(&spend, 1, id), 0);

        // and the moved balance stays spendable by a further plain spend
        let again = plain_tx(vec![outpoint(&spend, 0)], vec![to(ADDRESS1())]);
        index(vec![again.clone()], ETCH_HEIGHT + 2);
        assert_eq!(balance(&spend, 0, id), 0);
        assert_eq!(balance(&again, 0, id), 1000);
    }

    #[wasm_bindgen_test]
    fn no_runestone_spend_skips_leading_op_return() {
        let (e, id) = etch_premine();
        let spend = plain_tx(vec![outpoint(&e, 0)], vec![op_return(), to(ADDRESS2())]);
        assert!(Runestone::decipher(&spend).is_none());
        index(vec![spend.clone()], ETCH_HEIGHT + 1);
        assert_eq!(balance(&spend, 0, id), 0);
        assert_eq!(balance(&spend, 1, id), 1000);
    }

    #[wasm_bindgen_test]
    fn no_runestone_spend_without_eligible_output_burns() {
        let (e, id) = etch_premine();
        let spend = plain_tx(vec![outpoint(&e, 0)], vec![op_return()]);
        assert!(Runestone::decipher(&spend).is_none());
        index(vec![spend.clone()], ETCH_HEIGHT + 1);
        assert_eq!(balance(&e, 0, id), 0);
        assert_eq!(balance(&spend, 0, id), 0);
    }

    #[wasm_bindgen_test]
    fn no_runestone_spend_merges_multiple_inputs_and_runes() {
        let (e, id) = etch_premine();
        // a second rune etched at the next height, and an ordinary input
        let e2 = etch("AAAAAAAAAAAAAASECOND", 1, 1);
        index(vec![e2.clone()], ETCH_HEIGHT + 1);
        let id2 = RuneId {
            block: ETCH_HEIGHT + 1,
            tx: 0,
        };
        // a mint gives a second balance of the first rune on another outpoint
        let m = mint_tx(id, 2, None);
        index(vec![m.clone()], ETCH_HEIGHT + 2);
        assert_eq!(balance(&m, 0, id), 100);

        let spend = plain_tx(
            vec![
                outpoint(&e, 0),
                helpers::get_mock_outpoint(9),
                outpoint(&e2, 0),
                outpoint(&m, 0),
            ],
            vec![op_return(), to(ADDRESS1()), to(ADDRESS2())],
        );
        index(vec![spend.clone()], ETCH_HEIGHT + 3);
        assert_eq!(balance(&spend, 1, id), 1100);
        assert_eq!(balance(&spend, 1, id2), 1000);
        assert_eq!(balance(&spend, 2, id), 0);
        assert_eq!(balance(&e, 0, id), 0);
        assert_eq!(balance(&e2, 0, id2), 0);
        assert_eq!(balance(&m, 0, id), 0);
    }

    #[wasm_bindgen_test]
    fn empty_runestone_control_moves_runes_to_first_output() {
        let (e, id) = etch_premine();
        let spend = helpers::create_tx_from_runestone(
            Runestone::default(),
            vec![helpers::get_txin_from_outpoint(outpoint(&e, 0))],
            vec![to(ADDRESS2())],
        );
        index(vec![spend.clone()], ETCH_HEIGHT + 1);
        assert_eq!(balance(&spend, 0, id), 1000);
    }

    // ---- Protorune Indexes Rune Etchings Before the Mainnet Activation Height ----

    fn reserved_etch_tx(input: u32) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(0),
                    premine: Some(777),
                    rune: None,
                    spacers: None,
                    symbol: Some('R'),
                    turbo: true,
                    terms: None,
                }),
                pointer: Some(0),
                ..Default::default()
            },
            vec![helpers::get_mock_txin(input)],
            vec![to(ADDRESS1())],
        )
    }

    fn etching_name(id: RuneId) -> Vec<u8> {
        let id: ProtoruneRuneId = id.into();
        tables::RUNES
            .RUNE_ID_TO_ETCHING
            .select(&id.into())
            .get()
            .as_ref()
            .clone()
    }

    #[wasm_bindgen_test]
    fn first_rune_height_matches_ord_for_build_network() {
        #[cfg(feature = "mainnet")]
        assert_eq!(crate::first_rune_height(), 840_000);
        #[cfg(not(feature = "mainnet"))]
        assert_eq!(crate::first_rune_height(), 0);
    }

    // On non-mainnet builds (regtest semantics) runes are indexed from genesis.
    #[cfg(not(feature = "mainnet"))]
    #[wasm_bindgen_test]
    fn regtest_indexes_runes_from_genesis() {
        clear();
        let tx = reserved_etch_tx(0);
        index(vec![tx.clone()], 1);
        let id = RuneId { block: 1, tx: 0 };
        assert!(!etching_name(id).is_empty());
        assert_eq!(balance(&tx, 0, id), 777);
    }

    #[cfg(feature = "mainnet")]
    #[wasm_bindgen_test]
    fn mainnet_ignores_rune_etchings_before_840000() {
        // activation - 1: nothing is etched or credited
        clear();
        let before = reserved_etch_tx(0);
        index(vec![before.clone()], 839_999);
        let id = RuneId {
            block: 839_999,
            tx: 0,
        };
        assert!(etching_name(id).is_empty());
        assert_eq!(balance(&before, 0, id), 0);
        assert!(tables::HEIGHT_TO_RUNES
            .select_value(839_999u64)
            .get_list()
            .is_empty());

        // a pre-activation cenotaph etching (named) is ignored too
        let cenotaph = helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    rune: Some(Rune::from_str("AAAAAAAAAAAAAACENOTAPH").unwrap()),
                    ..Default::default()
                }),
                edicts: vec![ordinals::Edict {
                    id: RuneId { block: 0, tx: 1 },
                    amount: 0,
                    output: 0,
                }],
                ..Default::default()
            },
            vec![helpers::get_mock_txin(1)],
            vec![to(ADDRESS1())],
        );
        assert!(matches!(
            Runestone::decipher(&cenotaph),
            Some(Artifact::Cenotaph(_))
        ));
        index(vec![cenotaph], 839_999);
        let name = Rune::from_str("AAAAAAAAAAAAAACENOTAPH").unwrap();
        let name = protorune_support::utils::field_to_name(&name.0);
        assert!(tables::RUNES
            .ETCHING_TO_RUNE_ID
            .select(&name.as_bytes().to_vec())
            .get()
            .is_empty());

        // activation and activation + 1: normal behaviour
        for h in [840_000u64, 840_001] {
            let at = reserved_etch_tx((h - 839_998) as u32);
            index(vec![at.clone()], h);
            let id = RuneId { block: h, tx: 0 };
            assert!(!etching_name(id).is_empty());
            assert_eq!(balance(&at, 0, id), 777);
        }
    }
}
