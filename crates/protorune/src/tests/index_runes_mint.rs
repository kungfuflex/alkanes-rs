#[cfg(test)]
mod tests {
    use crate::message::MessageContext;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};

    use crate::message::MessageContextParcel;
    use crate::test_helpers::{self as helpers};
    use crate::Protorune;
    use anyhow::Result;
    use bitcoin::{OutPoint, Transaction};
    use metashrew::index_pointer::AtomicPointer;
    use protorune_support::rune_transfer::RuneTransfer;

    use helpers::clear;
    #[allow(unused_imports)]
    use metashrew::{
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
}
