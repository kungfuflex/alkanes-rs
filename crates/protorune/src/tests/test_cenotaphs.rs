#[cfg(test)]
mod tests {
    use crate::balance_sheet::load_sheet;
    use crate::message::MessageContext;
    use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
    use metashrew_support::proto;
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
    use metashrew_core::{
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
                // like ord, a cenotaph's valid mint still counts against the cap; the minted
                // amount is burned (checked above: output 0 holds none of rune1)
                assert_mints_remaining(mint.unwrap().into(), 1);
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

    // A cenotaph that mints `mint` (made a cenotaph by an edict with block 0 and tx != 0).
    fn cenotaph_mint_tx(mint: RuneId, input: u32) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                mint: Some(mint),
                edicts: vec![Edict {
                    id: RuneId { block: 0, tx: 1 },
                    amount: 0,
                    output: 0,
                }],
                pointer: Some(0),
                ..Default::default()
            },
            vec![helpers::get_mock_txin(input)],
            vec![helpers::get_txout_transfer_to_address(&ADDRESS1(), 100)],
        )
    }

    fn clean_mint_tx(mint: RuneId, input: u32) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                mint: Some(mint),
                pointer: Some(0),
                ..Default::default()
            },
            vec![helpers::get_mock_txin(input)],
            vec![helpers::get_txout_transfer_to_address(&ADDRESS2(), 100)],
        )
    }

    fn balance_of(tx: &Transaction, rune: RuneId) -> u128 {
        helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: tx.compute_txid(),
                vout: 0,
            },
            vec![rune.into()],
        )[0]
    }

    // Etches a rune with cap 1 and amount 100, mintable during [840000, 840010).
    fn etch_cap_one() -> RuneId {
        let etch = get_etching_for_tx_num(
            "AAAAAAAAAAAAACANONICAL",
            'C',
            Some(Terms {
                amount: Some(100),
                cap: Some(1),
                height: (Some(840000), Some(840010)),
                offset: (None, None),
            }),
        );
        Protorune::index_block::<MyMessageContext>(
            helpers::create_block_with_txs(vec![etch]),
            840000,
        )
        .unwrap();
        let id = RuneId {
            block: 840000,
            tx: 0,
        };
        assert_mints_remaining(id.into(), 1);
        id
    }

    #[wasm_bindgen_test]
    fn cenotaph_mint_exhausts_cap_and_burns() {
        clear();
        let id = etch_cap_one();

        let cenotaph = cenotaph_mint_tx(id, 1);
        assert!(matches!(
            Runestone::decipher(&cenotaph),
            Some(ordinals::Artifact::Cenotaph(_))
        ));
        Protorune::index_block::<MyMessageContext>(
            helpers::create_block_with_txs(vec![cenotaph.clone()]),
            840001,
        )
        .unwrap();
        assert_mints_remaining(id.into(), 0);
        assert_eq!(balance_of(&cenotaph, id), 0);

        // the cap is exhausted, so a later clean mint issues nothing
        let later = clean_mint_tx(id, 2);
        Protorune::index_block::<MyMessageContext>(
            helpers::create_block_with_txs(vec![later.clone()]),
            840002,
        )
        .unwrap();
        assert_mints_remaining(id.into(), 0);
        assert_eq!(balance_of(&later, id), 0);
    }

    #[wasm_bindgen_test]
    fn cenotaph_mint_outside_window_does_not_consume_cap() {
        clear();
        let id = etch_cap_one();

        Protorune::index_block::<MyMessageContext>(
            helpers::create_block_with_txs(vec![cenotaph_mint_tx(id, 1)]),
            840010,
        )
        .unwrap();
        assert_mints_remaining(id.into(), 1);
    }

    #[wasm_bindgen_test]
    fn cenotaph_mint_of_unknown_rune_is_ignored() {
        clear();
        let id = etch_cap_one();

        let unknown = RuneId {
            block: 840000,
            tx: 7,
        };
        Protorune::index_block::<MyMessageContext>(
            helpers::create_block_with_txs(vec![cenotaph_mint_tx(unknown, 1)]),
            840001,
        )
        .unwrap();
        assert_mints_remaining(id.into(), 1);

        // the real rune's allowance is untouched and still mintable
        let mint = clean_mint_tx(id, 2);
        Protorune::index_block::<MyMessageContext>(
            helpers::create_block_with_txs(vec![mint.clone()]),
            840002,
        )
        .unwrap();
        assert_mints_remaining(id.into(), 0);
        assert_eq!(balance_of(&mint, id), 100);
    }
}
