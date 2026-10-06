#[cfg(test)]
mod tests {
    use crate::message::MessageContext;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};

    use crate::message::MessageContextParcel;
    use crate::test_helpers::{self as helpers};
    use crate::Protorune;
    use crate::tables;
    use metashrew_support::index_pointer::KeyValuePointer;
    use anyhow::Result;
    use bitcoin::{OutPoint, Transaction};
    use metashrew_core::index_pointer::AtomicPointer;
    use protorune_support::rune_transfer::RuneTransfer;

    use helpers::clear;
    #[allow(unused_imports)]
    use metashrew_core::{
        println,
        stdio::{stdout, Write},
    };
    use ordinals::{Edict, Etching, Rune, RuneId, Runestone, Terms};

    use std::str::FromStr;
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

    fn get_default_etching_tx(terms: Option<Terms>) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(2),
                    premine: Some(1000),
                    rune: Some(Rune::from_str("AAAAAAAAAAAAATESTER").unwrap()),
                    spacers: Some(0),
                    symbol: Some('Z'),
                    turbo: true,
                    terms: terms,
                }),
                pointer: Some(0),
                edicts: Vec::new(),
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

    /// block is the block height of the protorune to mint
    /// n represents the which mint this is in the test
    fn get_default_mint_tx(block: u64, n: u32) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: Vec::new(),
                mint: Some(RuneId {
                    block: block,
                    tx: 0,
                }),
                protocol: None,
            },
            // txin doesn't matter, we are trying to mint
            vec![helpers::get_mock_txin(n)],
            // try to mint to address2
            vec![helpers::get_txout_transfer_to_address(
                &helpers::ADDRESS2(),
                100,
            )],
        )
    }

    fn rune_mint_base_template(block: u64, terms: Option<Terms>) -> bitcoin::Block {
        let tx0 = get_default_etching_tx(terms);
        let tx1 = get_default_mint_tx(block, 0);
        let test_block = helpers::create_block_with_txs(vec![tx0, tx1]);
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), block);
        // sanity check to make sure etched runes still exist
        let etched_runes = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block as u128,
                tx: 0,
            }],
        );
        assert_eq!(1000, etched_runes[0]);
        return test_block;
    }

    #[wasm_bindgen_test]
    fn rune_with_no_mint_terms() {
        clear();
        let block_height = 840000;
        let test_block = rune_mint_base_template(block_height, None);
        let stored_minted_amount = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        // assert nothing was minted
        assert_eq!(0, stored_minted_amount[0]);
    }

    #[wasm_bindgen_test]
    fn rune_with_mint_terms_outside_height() {
        clear();
        let block_height = 840000;
        let test_block = rune_mint_base_template(
            block_height,
            Some(Terms {
                amount: Some(200),
                cap: Some(1100),
                height: (Some(block_height + 1), Some(block_height + 100)),
                offset: (None, None),
            }),
        );
        let stored_minted_amount = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        // assert mint failed
        assert_eq!(0, stored_minted_amount[0]);
    }

    #[wasm_bindgen_test]
    fn rune_with_mint_exceed_cap() {
        clear();
        let block_height = 840000;
        let tx0 = get_default_etching_tx(Some(Terms {
            amount: Some(200),
            cap: Some(1),
            height: (Some(840000), Some(840005)),
            offset: (Some(0), Some(1)),
        }));
        let tx1 = get_default_mint_tx(block_height, 0);
        let tx2 = get_default_mint_tx(block_height, 1);
        let test_block = helpers::create_block_with_txs(vec![tx0, tx1, tx2]);
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), block_height);

        let stored_minted_amount_1 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        assert_eq!(200, stored_minted_amount_1[0]);

        let stored_minted_amount_2 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[2].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        // assert first mint good, second mint failed
        assert_eq!(0, stored_minted_amount_2[0]);
    }

    #[wasm_bindgen_test]
    fn rune_with_mint_in_same_tx_as_terms() {
        clear();
        let block_height = 840000;
        let tx0 = helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(2),
                    premine: Some(1000),
                    rune: Some(Rune::from_str("AAAAAAAAAAAAATESTER").unwrap()),
                    spacers: Some(0),
                    symbol: Some('Z'),
                    turbo: true,
                    terms: Some(Terms {
                        amount: Some(200),
                        cap: Some(1),
                        height: (Some(840000), Some(840005)),
                        offset: (Some(0), Some(1)),
                    }),
                }),
                // default runes go to 0
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId {
                    block: block_height,
                    tx: 0,
                }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(0)],
            vec![
                helpers::get_txout_transfer_to_address(&helpers::ADDRESS1(), 100_000_000),
                helpers::get_txout_transfer_to_address(&helpers::ADDRESS2(), 100),
            ],
        );

        let test_block = helpers::create_block_with_txs(vec![tx0]);
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), block_height);

        let stored_amount_address_2 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[0].compute_txid(),
                // address 2 is at vout1
                vout: 1,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        // address 2 should not mint
        assert_eq!(0, stored_amount_address_2[0]);

        let stored_amount_address_1 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );

        // no cenotaph here, should function as normal -- address 1 gets etched runes
        assert_eq!(1000, stored_amount_address_1[0]);
    }

    #[wasm_bindgen_test]
    fn rune_with_mint_terms() {
        clear();
        let block_height = 840000;
        let test_block = rune_mint_base_template(
            block_height,
            Some(Terms {
                amount: Some(200),
                cap: Some(1100),
                height: (Some(block_height), Some(block_height + 100)),
                offset: (None, None),
            }),
        );
        let stored_minted_amount = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        // assert mint success
        assert_eq!(200, stored_minted_amount[0]);
    }

    #[wasm_bindgen_test]
    fn rune_with_mint_terms_offset() {
        clear();
        let block_height = 840000;
        let test_block = rune_mint_base_template(
            block_height,
            Some(Terms {
                amount: Some(200),
                cap: Some(1100),
                height: (None, None),
                offset: (Some(0), Some(1)),
            }),
        );
        let stored_minted_amount = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        // assert mint success
        assert_eq!(200, stored_minted_amount[0]);
    }

    #[wasm_bindgen_test]
    fn rune_with_multiple_mint_terms_offset() {
        clear();
        let block_height = 840000;

        let tx0 = get_default_etching_tx(Some(Terms {
            amount: Some(200),
            cap: Some(5),
            height: (Some(840000), Some(840005)),
            offset: (Some(0), Some(1)),
        }));
        let tx1 = get_default_mint_tx(block_height, 0);
        let tx2 = get_default_mint_tx(block_height, 1);
        let test_block = helpers::create_block_with_txs(vec![tx0, tx1, tx2]);
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), block_height);
        let stored_minted_amount_1 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        assert_eq!(200, stored_minted_amount_1[0]);
        let stored_minted_amount_2 = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[2].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        // assert mint success
        assert_eq!(200, stored_minted_amount_2[0]);

        // sanity check to make sure etched runes still exist
        let etched_runes = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: test_block.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_height as u128,
                tx: 0,
            }],
        );
        assert_eq!(1000, etched_runes[0]);
    }

    /// Etches at `etch_height` with `terms` (cap 10, amount 200), then mints once at each
    /// of `mint_heights` in its own block. Returns (minted per mint, mints remaining).
    fn mint_after_etch(etch_height: u64, terms: Terms, mint_heights: &[u64]) -> (Vec<u128>, u128) {
        let etch = get_default_etching_tx(Some(Terms {
            amount: Some(200),
            cap: Some(10),
            ..terms
        }));
        Protorune::index_block::<MyMessageContext>(
            helpers::create_block_with_txs(vec![etch]),
            etch_height,
        )
        .unwrap();
        let rune = ProtoruneRuneId {
            block: etch_height as u128,
            tx: 0,
        };
        let minted = mint_heights
            .iter()
            .enumerate()
            .map(|(i, height)| {
                let mint = get_default_mint_tx(etch_height, i as u32 + 1);
                Protorune::index_block::<MyMessageContext>(
                    helpers::create_block_with_txs(vec![mint.clone()]),
                    *height,
                )
                .unwrap();
                helpers::get_rune_balance_by_outpoint(
                    OutPoint {
                        txid: mint.compute_txid(),
                        vout: 0,
                    },
                    vec![rune.clone()],
                )[0]
            })
            .collect();
        let name = tables::RUNES
            .RUNE_ID_TO_ETCHING
            .select(&rune.into())
            .get();
        let remaining: u128 = tables::RUNES.MINTS_REMAINING.select(&name).get_value();
        (minted, remaining)
    }

    fn terms(height: (Option<u64>, Option<u64>), offset: (Option<u64>, Option<u64>)) -> Terms {
        Terms {
            amount: None,
            cap: None,
            height,
            offset,
        }
    }

    #[wasm_bindgen_test]
    fn rune_with_explicit_zero_offset_end_never_mintable() {
        clear();
        let h = 840000;
        // ord: end = etching height + 0, and mints require height < end
        let (minted, remaining) =
            mint_after_etch(h, terms((None, None), (None, Some(0))), &[h, h + 1, h + 1000]);
        assert_eq!(minted, vec![0, 0, 0]);
        assert_eq!(remaining, 10);
    }

    #[wasm_bindgen_test]
    fn rune_with_explicit_zero_height_end_never_mintable() {
        clear();
        let h = 840000;
        let (minted, remaining) =
            mint_after_etch(h, terms((None, Some(0)), (None, None)), &[h, h + 1]);
        assert_eq!(minted, vec![0, 0]);
        assert_eq!(remaining, 10);
    }

    #[wasm_bindgen_test]
    fn rune_with_offset_end_one_closes_after_etching_block() {
        clear();
        let h = 840000;
        let (minted, remaining) =
            mint_after_etch(h, terms((None, None), (None, Some(1))), &[h, h + 1]);
        assert_eq!(minted, vec![200, 0]);
        assert_eq!(remaining, 9);
    }

    #[wasm_bindgen_test]
    fn rune_with_no_end_stays_open() {
        clear();
        let h = 840000;
        let (minted, remaining) =
            mint_after_etch(h, terms((None, None), (None, None)), &[h + 1, h + 100_000]);
        assert_eq!(minted, vec![200, 200]);
        assert_eq!(remaining, 8);
    }

    #[wasm_bindgen_test]
    fn rune_with_explicit_zero_start_opens_immediately() {
        clear();
        let h = 840000;
        let (minted, _) = mint_after_etch(h, terms((Some(0), None), (Some(0), None)), &[h]);
        assert_eq!(minted, vec![200]);
    }

    #[wasm_bindgen_test]
    fn rune_with_zero_offset_end_overrides_absolute_end() {
        clear();
        let h = 840000;
        // effective end = min(h + 100, h + 0) = h
        let (minted, remaining) =
            mint_after_etch(h, terms((None, Some(h + 100)), (None, Some(0))), &[h, h + 1]);
        assert_eq!(minted, vec![0, 0]);
        assert_eq!(remaining, 10);
    }

    #[wasm_bindgen_test]
    fn rune_with_combined_bounds_uses_max_start_and_min_end() {
        clear();
        let h = 840000;
        // start = max(h + 2, h + 3) = h + 3, end = min(h + 10, h + 5) = h + 5
        let (minted, remaining) = mint_after_etch(
            h,
            terms((Some(h + 2), Some(h + 10)), (Some(3), Some(5))),
            &[h + 2, h + 3, h + 4, h + 5],
        );
        assert_eq!(minted, vec![0, 200, 200, 0]);
        assert_eq!(remaining, 8);
    }

    #[wasm_bindgen_test]
    fn rune_with_max_offsets_saturates() {
        clear();
        let h = 840000;
        // relative bounds saturate to u64::MAX instead of overflowing
        let (minted, _) =
            mint_after_etch(h, terms((None, None), (None, Some(u64::MAX))), &[h + 1]);
        assert_eq!(minted, vec![200]);
    }

    #[wasm_bindgen_test]
    fn rune_with_max_offset_start_never_opens() {
        clear();
        let h = 840000;
        let (minted, remaining) =
            mint_after_etch(h, terms((None, None), (Some(u64::MAX), None)), &[h + 1]);
        assert_eq!(minted, vec![0]);
        assert_eq!(remaining, 10);
    }
}
