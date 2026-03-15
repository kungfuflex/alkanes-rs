#[cfg(test)]
mod tests {
    use crate::message::MessageContext;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};

    use crate::message::MessageContextParcel;
    use crate::test_helpers::{self as helpers};
    use crate::Protorune;
    use anyhow::Result;
    use bitcoin::OutPoint;
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

    /// Etch a rune with mint terms at block N, then mint it at block N+1
    /// in a separate index_block call. Verify mint succeeds and balances are correct.
    #[wasm_bindgen_test]
    fn test_etch_block_n_mint_block_n_plus_1() {
        clear();
        let block_n = 840000u64;
        let block_n1 = 840001u64;

        // Block N: etch a rune with mint terms
        let etch_tx = helpers::create_tx_from_runestone(
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
                        cap: Some(100),
                        height: (Some(block_n), Some(block_n + 10)),
                        offset: (None, None),
                    }),
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
        );

        let block1 = helpers::create_block_with_txs(vec![etch_tx]);
        let _ = Protorune::index_block::<MyMessageContext>(block1.clone(), block_n);

        // Verify premine balance at block N
        let premine_balance = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_n as u128,
                tx: 0,
            }],
        );
        assert_eq!(1000, premine_balance[0]);

        // Block N+1: mint the rune
        let coinbase_tx = helpers::create_coinbase_transaction(block_n1 as u32);
        let mint_tx = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: Vec::new(),
                mint: Some(RuneId {
                    block: block_n,
                    tx: 0,
                }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![helpers::get_txout_transfer_to_address(
                &helpers::ADDRESS2(),
                100,
            )],
        );

        let block2 = helpers::create_block_with_txs(vec![coinbase_tx, mint_tx]);
        let _ = Protorune::index_block::<MyMessageContext>(block2.clone(), block_n1);

        // Verify mint succeeded at block N+1
        let minted_balance = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block2.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_n as u128,
                tx: 0,
            }],
        );
        assert_eq!(200, minted_balance[0]);

        // Verify premine balance still intact
        let premine_after = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![ProtoruneRuneId {
                block: block_n as u128,
                tx: 0,
            }],
        );
        assert_eq!(1000, premine_after[0]);
    }

    /// Etch a rune at block N, then transfer via edicts at block N+1.
    /// Verify source balance is cleared and destination has the tokens.
    #[wasm_bindgen_test]
    fn test_etch_block_n_transfer_block_n_plus_1() {
        clear();
        let block_n = 840000u64;
        let block_n1 = 840001u64;

        // Block N: etch a rune with premine of 1000
        let etch_tx = helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(2),
                    premine: Some(1000),
                    rune: Some(Rune::from_str("AAAAAAAAAAAAATESTER").unwrap()),
                    spacers: Some(0),
                    symbol: Some('Z'),
                    turbo: true,
                    terms: None,
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
        );

        let block1 = helpers::create_block_with_txs(vec![etch_tx]);
        let _ = Protorune::index_block::<MyMessageContext>(block1.clone(), block_n);

        // Verify premine balance
        let rune_id = ProtoruneRuneId {
            block: block_n as u128,
            tx: 0,
        };
        let premine_balance = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(1000, premine_balance[0]);

        // Block N+1: spend the etched outpoint and transfer via edict
        let coinbase_tx = helpers::create_coinbase_transaction(block_n1 as u32);
        let transfer_tx = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(1), // leftover goes to vout 1 (ADDRESS1 change)
                edicts: vec![Edict {
                    id: RuneId {
                        block: block_n,
                        tx: 0,
                    },
                    amount: 300,
                    output: 0, // send 300 to vout 0 (ADDRESS2)
                }],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_txin_from_outpoint(OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            })],
            vec![
                helpers::get_txout_transfer_to_address(&helpers::ADDRESS2(), 100),
                helpers::get_txout_transfer_to_address(&helpers::ADDRESS1(), 99_999_900),
            ],
        );

        let block2 = helpers::create_block_with_txs(vec![coinbase_tx, transfer_tx]);
        let _ = Protorune::index_block::<MyMessageContext>(block2.clone(), block_n1);

        // Verify source outpoint (block N, tx0, vout0) balance is now 0
        let source_balance = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(0, source_balance[0]);

        // Verify destination (ADDRESS2) got 300
        let dest_balance = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block2.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(300, dest_balance[0]);

        // Verify change (ADDRESS1) got 700
        let change_balance = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block2.txdata[1].compute_txid(),
                vout: 1,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(700, change_balance[0]);
    }

    /// Etch with height_end=840002 at block 840000.
    /// Mint at 840001 (succeeds). Mint at 840002 (fails).
    /// Each step is a separate index_block call.
    #[wasm_bindgen_test]
    fn test_mint_after_height_end_fails() {
        clear();
        let block_etch = 840000u64;
        let block_mint_ok = 840001u64;
        let block_mint_fail = 840002u64;

        // Block 840000: etch rune with height_end=840002
        let etch_tx = helpers::create_tx_from_runestone(
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
                        cap: Some(100),
                        height: (Some(block_etch), Some(block_mint_fail)),
                        offset: (None, None),
                    }),
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
        );

        let block1 = helpers::create_block_with_txs(vec![etch_tx]);
        let _ = Protorune::index_block::<MyMessageContext>(block1.clone(), block_etch);

        let rune_id = ProtoruneRuneId {
            block: block_etch as u128,
            tx: 0,
        };

        // Block 840001: mint should succeed (within height range)
        let coinbase_tx_1 = helpers::create_coinbase_transaction(block_mint_ok as u32);
        let mint_tx_ok = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: Vec::new(),
                mint: Some(RuneId {
                    block: block_etch,
                    tx: 0,
                }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![helpers::get_txout_transfer_to_address(
                &helpers::ADDRESS2(),
                100,
            )],
        );

        let block2 = helpers::create_block_with_txs(vec![coinbase_tx_1, mint_tx_ok]);
        let _ = Protorune::index_block::<MyMessageContext>(block2.clone(), block_mint_ok);

        // Verify mint succeeded at block 840001
        let minted_ok = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block2.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(200, minted_ok[0]);

        // Block 840002: mint should fail (at height_end boundary)
        let coinbase_tx_2 = helpers::create_coinbase_transaction(block_mint_fail as u32);
        let mint_tx_fail = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: Vec::new(),
                mint: Some(RuneId {
                    block: block_etch,
                    tx: 0,
                }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(2)],
            vec![helpers::get_txout_transfer_to_address(
                &helpers::ADDRESS2(),
                100,
            )],
        );

        let block3 = helpers::create_block_with_txs(vec![coinbase_tx_2, mint_tx_fail]);
        let _ = Protorune::index_block::<MyMessageContext>(block3.clone(), block_mint_fail);

        // Verify mint failed at block 840002
        let minted_fail = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block3.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(0, minted_fail[0]);
    }

    /// Etch at block N. At block N+1 do nothing (just index a coinbase).
    /// Verify balance still exists at the original outpoint.
    #[wasm_bindgen_test]
    fn test_balance_persists_across_blocks() {
        clear();
        let block_n = 840000u64;
        let block_n1 = 840001u64;

        // Block N: etch a rune
        let etch_tx = helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(2),
                    premine: Some(1000),
                    rune: Some(Rune::from_str("AAAAAAAAAAAAATESTER").unwrap()),
                    spacers: Some(0),
                    symbol: Some('Z'),
                    turbo: true,
                    terms: None,
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
        );

        let block1 = helpers::create_block_with_txs(vec![etch_tx]);
        let _ = Protorune::index_block::<MyMessageContext>(block1.clone(), block_n);

        let rune_id = ProtoruneRuneId {
            block: block_n as u128,
            tx: 0,
        };

        // Verify balance exists after block N
        let balance_before = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(1000, balance_before[0]);

        // Block N+1: just a coinbase, no rune activity
        let coinbase_tx = helpers::create_coinbase_transaction(block_n1 as u32);
        let block2 = helpers::create_block_with_txs(vec![coinbase_tx]);
        let _ = Protorune::index_block::<MyMessageContext>(block2.clone(), block_n1);

        // Verify balance still exists at the original outpoint after block N+1
        let balance_after = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(1000, balance_after[0]);
    }

    /// Etch at block N. Spend at block N+1 with a transfer.
    /// Verify original outpoint balance is 0.
    #[wasm_bindgen_test]
    fn test_spent_input_cleared_across_blocks() {
        clear();
        let block_n = 840000u64;
        let block_n1 = 840001u64;

        // Block N: etch a rune with premine of 1000
        let etch_tx = helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(2),
                    premine: Some(1000),
                    rune: Some(Rune::from_str("AAAAAAAAAAAAATESTER").unwrap()),
                    spacers: Some(0),
                    symbol: Some('Z'),
                    turbo: true,
                    terms: None,
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
        );

        let block1 = helpers::create_block_with_txs(vec![etch_tx]);
        let _ = Protorune::index_block::<MyMessageContext>(block1.clone(), block_n);

        let rune_id = ProtoruneRuneId {
            block: block_n as u128,
            tx: 0,
        };

        // Verify premine balance exists
        let balance_before = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(1000, balance_before[0]);

        // Block N+1: spend the etched outpoint, transfer all to ADDRESS2
        let coinbase_tx = helpers::create_coinbase_transaction(block_n1 as u32);
        let transfer_tx = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0), // all runes go to vout 0 (ADDRESS2)
                edicts: Vec::new(),
                mint: None,
                protocol: None,
            },
            vec![helpers::get_txin_from_outpoint(OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            })],
            vec![helpers::get_txout_transfer_to_address(
                &helpers::ADDRESS2(),
                100,
            )],
        );

        let block2 = helpers::create_block_with_txs(vec![coinbase_tx, transfer_tx]);
        let _ = Protorune::index_block::<MyMessageContext>(block2.clone(), block_n1);

        // Verify original outpoint balance is now 0
        let source_balance = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block1.txdata[0].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(0, source_balance[0]);

        // Verify destination received all 1000 runes
        let dest_balance = helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: block2.txdata[1].compute_txid(),
                vout: 0,
            },
            vec![rune_id.clone()],
        );
        assert_eq!(1000, dest_balance[0]);
    }
}
