#[cfg(test)]
mod tests {
    use crate::balance_sheet::load_sheet;
    use crate::message::MessageContext;
    use metashrew::index_pointer::{AtomicPointer, IndexPointer};
    use metashrew::proto;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
    use protorune_support::protostone::Protostone;

    use crate::test_helpers::{self as helpers, RunesTestingConfig, ADDRESS1, ADDRESS2};
    use crate::Protorune;
    use crate::{message::MessageContextParcel, tables};
    use anyhow::Result;
    use protorune_support::rune_transfer::RuneTransfer;
    use protorune_support::utils::consensus_encode;

    use bitcoin::{OutPoint, Transaction};

    use std::str::FromStr;
    use std::vec;

    use helpers::clear;
    #[allow(unused_imports)]
    use metashrew::{
        println,
        stdio::{stdout, Write},
    };
    use metashrew_support::index_pointer::KeyValuePointer;
    use ordinals::{Edict, Etching, Rune, RuneId, Runestone, Terms};

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

    fn get_etching_for_tx_num(rune_name: &str, symbol: char, terms: Option<Terms>) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(2),
                    premine: Some(1000),
                    rune: Some(Rune::from_str(rune_name).unwrap()),
                    spacers: None,
                    symbol: Some(symbol),
                    turbo: true,
                    terms: terms,
                }),
                pointer: Some(0),
                edicts: vec![],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_mock_txin(0)],
            vec![helpers::get_txout_transfer_to_address(
                &helpers::ADDRESS1(),
                100_000_000,
            )],
        )
    }

    fn assert_mints_remaining(mint: ProtoruneRuneId, mints_remaining: u128) {
        let name = tables::RUNES
            .RUNE_ID_TO_ETCHING
            .select(&mint.clone().into())
            .get();
        let indexed_mints_remaining: u128 = tables::RUNES.MINTS_REMAINING.select(&name).get_value();
        assert_eq!(indexed_mints_remaining, mints_remaining);
    }

    fn cenotaph_test_template(
        additional_edicts: Vec<Edict>,
        etching: Option<Etching>,
        mint: Option<RuneId>,
        is_cenotaph: bool,
    ) {
        let block_height = 840000;

        // tx0 etches rune0
        let tx0 = get_etching_for_tx_num("AAAAAAAAAAAAATESTER", 'A', None);
        let rune0_id = RuneId {
            block: block_height,
            tx: 0,
        };
        let tx0_utxo = helpers::get_txin_from_outpoint(OutPoint {
            txid: tx0.compute_txid(),
            vout: 0,
        });

        // tx1 etches rune1. this rune has terms, which may be used in tx2 to produce cenotaphs
        let tx1 = get_etching_for_tx_num(
            "BBBBBBBBBBBBBTESTER",
            'B',
            Some(Terms {
                amount: Some(888),
                cap: Some(2),
                height: (Some(840000), Some(840001)),
                offset: (None, None),
            }),
        );
        let rune1_id = RuneId {
            block: block_height,
            tx: 1,
        };
        let tx1_utxo = helpers::get_txin_from_outpoint(OutPoint {
            txid: tx1.compute_txid(),
            vout: 0,
        });

        // tx2 is the transaction with cenotaph

        // runeid_1 will always be a valid edict
        let mut all_edicts = vec![Edict {
            id: rune1_id,
            amount: 333,
            output: 0,
        }];

        all_edicts.extend(additional_edicts);

        let tx2 = helpers::create_tx_from_runestone(
            Runestone {
                etching,
                pointer: Some(0),
                edicts: all_edicts,
                mint,
                protocol: None,
            },
            vec![tx0_utxo, tx1_utxo],
            vec![helpers::get_txout_transfer_to_address(
                &helpers::ADDRESS1(),
                100_000_000,
            )],
        );
        // index the block
        let test_block = helpers::create_block_with_txs(vec![tx0, tx1, tx2]);
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), block_height);

        let mut protorune_ids: Vec<ProtoruneRuneId> = vec![rune0_id.into(), rune1_id.into()];
        if etching.is_some() {
            let rune2_id = RuneId {
                block: block_height,
                tx: 2,
            };

            protorune_ids = vec![rune0_id.into(), rune1_id.into(), rune2_id.into()];
        }

        // test all input runes are burned
        let final_amounts = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[2].compute_txid(),
                vout: 0,
            },
            protorune_ids,
        );

        if is_cenotaph {
            assert_eq!(0, final_amounts[0]);
            assert_eq!(0, final_amounts[1]);

            // corresponds to rune2_id, which is the etched amount
            if etching.is_some() {
                assert_eq!(0, final_amounts[2]);
            }

            if mint.is_some() {
                // runes that are not able to be deciphered do not count against cap
                // official code follows this convention. weird but we have to follow convention
                assert_mints_remaining(mint.unwrap().into(), 2);
            }
            // test etched rune has supply 0 and is unmintable
            assert_etching_is_cenotaph();

            // TODO: Test protorune is still executed on a cenotaph.
        } else {
            assert_eq!(1000, final_amounts[0]);

            if mint.is_some() {
                assert_eq!(1888, final_amounts[1]);
                assert_mints_remaining(mint.unwrap().into(), 1);
            }

            if etching.is_some() {
                assert_eq!(etching.unwrap().premine.unwrap(), final_amounts[2]);
            }
        }
    }

    fn assert_etching_is_cenotaph() {
        // an etching in the same tx as a cenotaph should exist,
        // but should set supply zero and is unmintable.

        // TODO: not super important right now
    }

    #[wasm_bindgen_test]
    fn non_cenotaph_edict() {
        // base case, valid tx
        clear();
        cenotaph_test_template(
            vec![Edict {
                // this is tx1 etched rune
                id: RuneId {
                    block: 840000,
                    tx: 1,
                },
                amount: 100,
                output: 0,
            }],
            Some(Etching {
                divisibility: Some(2),
                premine: Some(10000),
                rune: Some(Rune::from_str("IAMINTHESAMETXASCENO").unwrap()),
                spacers: Some(0),
                symbol: Some('Z'),
                turbo: true,
                terms: None,
            }),
            None,
            false,
        );
    }

    #[wasm_bindgen_test]
    fn non_cenotaph_mint() {
        clear();
        cenotaph_test_template(
            vec![Edict {
                id: RuneId {
                    block: 840000,
                    tx: 1,
                },
                amount: 100,
                output: 0,
            }],
            None,
            Some(RuneId {
                block: 840000,
                tx: 1,
            }),
            false,
        );
    }

    #[wasm_bindgen_test]
    fn cenotaph_edict_greater_than_num_outputs() {
        // there are 2 outputs, where outs[1] is the runestone.
        // outs[2] evenly splits the input
        // outs[3] should be invalid
        clear();
        cenotaph_test_template(
            vec![Edict {
                // this is tx1 etched rune
                id: RuneId {
                    block: 840000,
                    tx: 1,
                },
                amount: 100,
                output: 3,
            }],
            Some(Etching {
                divisibility: Some(2),
                premine: Some(10000),
                rune: Some(Rune::from_str("IAMINTHESAMETXASCENO").unwrap()),
                spacers: Some(0),
                symbol: Some('Z'),
                turbo: true,
                terms: None,
            }),
            None,
            true,
        );
    }

    #[wasm_bindgen_test]
    fn cenotaph_zero_block_edict() {
        clear();
        cenotaph_test_template(
            vec![Edict {
                id: RuneId { block: 0, tx: 1 },
                amount: 100,
                output: 0,
            }],
            Some(Etching {
                divisibility: Some(2),
                premine: Some(10000),
                rune: Some(Rune::from_str("IAMINTHESAMETXASCENO").unwrap()),
                spacers: Some(0),
                symbol: Some('Z'),
                turbo: true,
                terms: None,
            }),
            None,
            true,
        );
    }

    #[wasm_bindgen_test]
    fn non_cenotaph_mint_and_etching() {
        // rune etching and mint in same tx works iff the mint is for a different runeid
        clear();
        cenotaph_test_template(
            vec![Edict {
                // this is tx1 etched rune
                id: RuneId {
                    block: 840000,
                    tx: 1,
                },
                amount: 100,
                output: 0,
            }],
            Some(Etching {
                divisibility: Some(2),
                premine: Some(10000),
                rune: Some(Rune::from_str("IAMINTHESAMETXASCENO").unwrap()),
                spacers: Some(0),
                symbol: Some('Z'),
                turbo: true,
                terms: None,
            }),
            Some(RuneId {
                block: 840000,
                tx: 1,
            }),
            false,
        );
    }

    /// TODO: This currently fails since the validation for block = 0 happens before indexing (during decipher)
    #[wasm_bindgen_test]
    fn cenotaph_mint_reduces_cap() {
        clear();
        cenotaph_test_template(
            vec![Edict {
                id: RuneId {
                    // this causes cenotaph
                    block: 0,
                    tx: 1,
                },
                amount: 100,
                output: 0,
            }],
            None,
            Some(RuneId {
                block: 840000,
                tx: 1,
            }),
            true,
        );
    }

    /// TODO: This currently fails since the validation for output = 3 happens before indexing (during decipher)
    #[wasm_bindgen_test]
    fn cenotaph2_mint_reduces_cap() {
        clear();
        cenotaph_test_template(
            vec![Edict {
                // this is tx0 etched rune
                id: RuneId {
                    block: 840000,
                    tx: 1,
                },
                amount: 100,
                // this causes cenotaph
                output: 3,
            }],
            None,
            Some(RuneId {
                block: 840000,
                tx: 1,
            }),
            true,
        );
    }
}
