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
}
