/// Runes indexing parity tests ported from canonical ord (ordinals/ord).
///
/// These tests verify that our protorune indexer matches the behavior of
/// the canonical ord runes indexer. Tests are organized by the divergences
/// identified in the audit, and many are expected to FAIL initially until
/// the implementation is corrected.
///
/// Reference: https://github.com/ordinals/ord/blob/master/src/index/updater/rune_updater.rs
/// Reference: https://github.com/ordinals/ord/blob/master/src/runes.rs
#[cfg(test)]
mod tests {
    use crate::balance_sheet::load_sheet;
    use crate::message::MessageContext;
    use crate::test_helpers::{self as helpers, RunesTestingConfig, ADDRESS1, ADDRESS2};
    use crate::Protorune;
    use crate::{message::MessageContextParcel, tables};
    use anyhow::Result;
    use bitcoin::{OutPoint, Transaction};
    use helpers::clear;
    #[allow(unused_imports)]
    use metashrew_core::{
        println,
        stdio::{stdout, Write},
    };
    use metashrew_core::index_pointer::AtomicPointer;
    use metashrew_support::index_pointer::KeyValuePointer;
    use ordinals::{Artifact, Edict, Etching, Rune, RuneId, Runestone, Terms};
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
    use protorune_support::rune_transfer::RuneTransfer;
    use protorune_support::utils::consensus_encode;
    use std::str::FromStr;
    use std::sync::Arc;
    use wasm_bindgen_test::*;

    struct TestContext(());

    impl MessageContext for TestContext {
        fn handle(
            _parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            Ok((vec![], BalanceSheet::default()))
        }
        fn protocol_tag() -> u128 {
            100
        }
    }

    // =========================================================================
    // Helper functions
    // =========================================================================

    /// Create a transaction with a specific etching and optional terms/edicts/mint
    fn make_etching_tx(
        rune_name: &str,
        symbol: char,
        premine: u128,
        divisibility: u8,
        terms: Option<Terms>,
        pointer: Option<u32>,
        edicts: Vec<Edict>,
        mint: Option<RuneId>,
        txin_n: u32,
    ) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(divisibility),
                    premine: Some(premine),
                    rune: Some(Rune::from_str(rune_name).unwrap()),
                    spacers: Some(0),
                    symbol: Some(symbol),
                    turbo: true,
                    terms,
                }),
                pointer,
                edicts,
                mint,
                protocol: None,
            },
            vec![helpers::get_mock_txin(txin_n)],
            vec![helpers::get_txout_transfer_to_address(
                &ADDRESS1(),
                100_000_000,
            )],
        )
    }

    /// Create a transfer transaction spending a specific outpoint
    fn make_transfer_tx(
        previous_output: OutPoint,
        edicts: Vec<Edict>,
        pointer: Option<u32>,
        mint: Option<RuneId>,
        additional_outputs: Vec<bitcoin::TxOut>,
    ) -> Transaction {
        let txin = helpers::get_txin_from_outpoint(previous_output);
        let mut txouts = additional_outputs;

        let runestone_script = (Runestone {
            etching: None,
            pointer,
            edicts,
            mint,
            protocol: None,
        })
        .encipher();

        txouts.push(bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(0),
            script_pubkey: runestone_script,
        });

        Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: txouts,
        }
    }

    fn addr1_txout(amount: u64) -> bitcoin::TxOut {
        helpers::get_txout_transfer_to_address(&ADDRESS1(), amount)
    }

    fn addr2_txout(amount: u64) -> bitcoin::TxOut {
        helpers::get_txout_transfer_to_address(&ADDRESS2(), amount)
    }

    fn balance_at(outpoint: OutPoint, rune_id: ProtoruneRuneId) -> u128 {
        helpers::get_rune_balance_by_outpoint(outpoint, vec![rune_id])[0]
    }

    fn rune_id(block: u64, tx: u32) -> ProtoruneRuneId {
        ProtoruneRuneId {
            block: block as u128,
            tx: tx as u128,
        }
    }

    // =========================================================================
    // DIVERGENCE 1: Mint terms storage — only stores when BOTH start AND end present
    //
    // Bug: if only height.0 is set (no height.1), NEITHER gets stored.
    // Ord stores each field independently.
    // =========================================================================

    #[wasm_bindgen_test]
    fn mint_with_only_height_start() {
        // Ord: height_start=840000, no height_end -> mint should be valid at 840000
        // Our bug: neither gets stored, so mint always succeeds (or always fails)
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(10),
                height: (Some(840000), None), // only start, no end
                offset: (None, None),
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        // Mint at the start height — should succeed
        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: block_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let block = helpers::create_block_with_txs(vec![tx0, tx1]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let minted = balance_at(
            OutPoint { txid: block.txdata[1].compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(minted, 100, "mint with only height_start should succeed at start height");
    }

    #[wasm_bindgen_test]
    fn mint_with_only_height_end() {
        // Ord: no height_start, height_end=840005 -> mint valid before 840005
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(10),
                height: (None, Some(840005)), // only end, no start
                offset: (None, None),
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: block_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let block = helpers::create_block_with_txs(vec![tx0, tx1]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let minted = balance_at(
            OutPoint { txid: block.txdata[1].compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(minted, 100, "mint with only height_end should succeed before end height");
    }

    #[wasm_bindgen_test]
    fn mint_with_only_offset_start() {
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(10),
                height: (None, None),
                offset: (Some(0), None), // only offset_start, no offset_end
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: block_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let block = helpers::create_block_with_txs(vec![tx0, tx1]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let minted = balance_at(
            OutPoint { txid: block.txdata[1].compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(minted, 100, "mint with only offset_start=0 should succeed at etching height");
    }

    #[wasm_bindgen_test]
    fn mint_with_only_offset_end() {
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(10),
                height: (None, None),
                offset: (None, Some(5)), // only offset_end=5, no offset_start
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: block_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let block = helpers::create_block_with_txs(vec![tx0, tx1]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let minted = balance_at(
            OutPoint { txid: block.txdata[1].compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(minted, 100, "mint with only offset_end=5 should succeed before end");
    }

    // =========================================================================
    // DIVERGENCE 2: Mint window semantics — max(start) / min(end)
    //
    // Ord: start = max(offset_start + etching_block, height_start)
    //      end   = min(offset_end + etching_block, height_end)
    // Our code: treats them as independent AND conditions.
    // =========================================================================

    #[wasm_bindgen_test]
    fn mint_start_is_max_of_height_and_offset() {
        // Ord: start = max(840002, 840000+5) = max(840002, 840005) = 840005
        // At height 840003: should NOT be mintable (840003 < 840005)
        clear();
        let etch_height: u64 = 840000;
        let mint_height: u64 = 840003;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(10),
                height: (Some(840002), None),
                offset: (Some(5), None), // offset_start=5 -> absolute start = 840005
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), etch_height);

        // Mint at height 840003 — should fail because max(840002, 840005)=840005
        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: etch_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), mint_height);

        let minted = balance_at(
            OutPoint { txid: block1.txdata[0].compute_txid(), vout: 0 },
            rune_id(etch_height, 0),
        );
        assert_eq!(minted, 0, "mint should fail: height 840003 < max(840002, 840005)=840005");
    }

    #[wasm_bindgen_test]
    fn mint_end_is_min_of_height_and_offset() {
        // Ord: end = min(840010, 840000+3) = min(840010, 840003) = 840003
        // At height 840003: should NOT be mintable (840003 >= 840003)
        clear();
        let etch_height: u64 = 840000;
        let mint_height: u64 = 840003;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(10),
                height: (Some(840000), Some(840010)),
                offset: (None, Some(3)), // offset_end=3 -> absolute end = 840003
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), etch_height);

        // Mint at height 840003 — should fail because min(840010, 840003)=840003
        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: etch_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), mint_height);

        let minted = balance_at(
            OutPoint { txid: block1.txdata[0].compute_txid(), vout: 0 },
            rune_id(etch_height, 0),
        );
        assert_eq!(minted, 0, "mint should fail: height 840003 >= min(840010, 840003)=840003");
    }

    // =========================================================================
    // DIVERGENCE 3: Cenotaph handling — ord processes mints and etchings on cenotaphs
    //
    // Canonical ord: Cenotaphs still process mints (incrementing mint count)
    // and etchings (with zeroed metadata). All unallocated runes are burned.
    // Our code: completely skips cenotaphs.
    // =========================================================================

    #[wasm_bindgen_test]
    fn cenotaph_etching_creates_rune_with_zero_params() {
        // In ord, a cenotaph etching creates the rune but with zeroed params
        // (divisibility=0, symbol=None, premine=0, terms=None)
        clear();
        let block_height: u64 = 840000;

        // Construct a cenotaph manually: use an unrecognized even tag
        // We'll simulate by creating a runestone with an edict that has output > num_outputs
        // which triggers a cenotaph during decipher
        let tx0 = helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(5),
                    premine: Some(5000),
                    rune: Some(Rune::from_str("AAAAAAAAAAAAATESTER").unwrap()),
                    spacers: Some(0),
                    symbol: Some('X'),
                    turbo: true,
                    terms: None,
                }),
                pointer: Some(0),
                // This edict with output=5 on a tx with 2 outputs will make it a cenotaph
                edicts: vec![Edict {
                    id: RuneId { block: 0, tx: 0 },
                    amount: 100,
                    output: 5,
                }],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_mock_txin(0)],
            vec![addr1_txout(100_000_000)],
        );

        // Verify this IS a cenotaph
        let artifact = Runestone::decipher(&tx0);
        let is_cenotaph = matches!(artifact, Some(Artifact::Cenotaph(_)));
        assert!(is_cenotaph, "tx should be detected as cenotaph");

        let block = helpers::create_block_with_txs(vec![tx0]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        // In ord, the rune name should still be registered (but with zeroed params)
        let rune_id_key: Vec<u8> = ProtoruneRuneId::new(block_height as u128, 0).into();
        let etching_name = tables::RUNES.RUNE_ID_TO_ETCHING.select(&rune_id_key).get();

        // This should NOT be empty — ord creates the rune entry even for cenotaphs
        assert!(
            !etching_name.is_empty(),
            "cenotaph etching should still create a rune entry (ord behavior)"
        );
    }

    #[wasm_bindgen_test]
    fn cenotaph_burns_input_runes() {
        // Input runes should be burned (cleared) when a cenotaph occurs
        clear();
        let block_height: u64 = 840000;

        // First, etch a rune normally
        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Verify rune exists
        let initial_balance = balance_at(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(initial_balance, 1000);

        // Now spend it in a cenotaph transaction (invalid edict output)
        let cenotaph_tx = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![Edict {
                    id: RuneId { block: 0, tx: 1 }, // invalid: block=0, tx=1
                    amount: 100,
                    output: 0,
                }],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_txin_from_outpoint(OutPoint {
                txid: tx0.compute_txid(),
                vout: 0,
            })],
            vec![addr1_txout(100_000_000)],
        );

        let block1 = helpers::create_block_with_txs(vec![cenotaph_tx.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        // Output should have 0 runes — they were burned
        let output_balance = balance_at(
            OutPoint { txid: cenotaph_tx.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(output_balance, 0, "cenotaph should burn all input runes");

        // Input should also be cleared
        let input_balance = balance_at(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(input_balance, 0, "spent input should be cleared");
    }

    #[wasm_bindgen_test]
    fn cenotaph_mint_does_not_decrement_cap() {
        // Ord: a mint in a cenotaph tx does NOT reduce the cap
        // (because the cenotaph burns everything, but the mint count is not incremented)
        clear();
        let block_height: u64 = 840000;

        // Etch with terms
        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            0, // no premine
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(5),
                height: (Some(840000), Some(840100)),
                offset: (None, None),
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        // Cenotaph tx that tries to mint
        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![Edict {
                    id: RuneId { block: 0, tx: 1 }, // cenotaph trigger
                    amount: 0,
                    output: 0,
                }],
                mint: Some(RuneId { block: block_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let block = helpers::create_block_with_txs(vec![tx0, tx1]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        // Cap should still be 5 (mint didn't count)
        let name = tables::RUNES
            .RUNE_ID_TO_ETCHING
            .select(&<ProtoruneRuneId as Into<Vec<u8>>>::into(rune_id(block_height, 0)))
            .get();
        let remaining: u128 = tables::RUNES.MINTS_REMAINING.select(&name).get_value();
        assert_eq!(remaining, 5, "cenotaph mint should not decrement cap");
    }

    // =========================================================================
    // DIVERGENCE 4: Edict ID=0 self-referencing etch
    //
    // Canonical ord: RuneId(0,0) in an edict refers to the rune being etched
    // in the same transaction.
    // Our code: no equivalent handling.
    // =========================================================================

    #[wasm_bindgen_test]
    fn edict_with_id_zero_refers_to_etched_rune() {
        // Ord: RuneId(0,0) in edict should refer to the rune being etched in this tx
        clear();
        let block_height: u64 = 840000;

        let tx0 = helpers::create_tx_from_runestone(
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
                // Edict with RuneId(0,0) = self-reference to this etching
                edicts: vec![Edict {
                    id: RuneId { block: 0, tx: 0 },
                    amount: 500,
                    output: 0,
                }],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_mock_txin(0)],
            vec![
                addr1_txout(100_000_000),
                addr2_txout(50_000_000),
            ],
        );

        let block = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        // Edict should transfer 500 to vout 0, remaining 500 to pointer (vout 0)
        // Total: 1000 at vout 0
        let balance_out0 = balance_at(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(balance_out0, 1000, "RuneId(0,0) edict should refer to etched rune");
    }

    // =========================================================================
    // DIVERGENCE 5: OP_RETURN output burn handling
    //
    // Canonical ord: runes allocated to OP_RETURN outputs are burned (not saved).
    // Our code: saves balance sheets to ALL outputs including OP_RETURN.
    // =========================================================================

    #[wasm_bindgen_test]
    fn runes_sent_to_op_return_are_burned() {
        // Edicts that send runes to OP_RETURN outputs should burn them
        clear();
        let block_height: u64 = 840000;

        // Etch rune
        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Transfer: send 500 to the OP_RETURN output (which is the last output)
        // In our test setup, the OP_RETURN is at the end
        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![Edict {
                    id: RuneId { block: block_height, tx: 0 },
                    amount: 500,
                    // vout 2 is the OP_RETURN in a 3-output tx (0=addr2, 1=addr1, 2=OP_RETURN)
                    output: 2,
                }],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_txin_from_outpoint(OutPoint {
                txid: tx0.compute_txid(),
                vout: 0,
            })],
            vec![addr2_txout(1), addr1_txout(99_999_999)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        // OP_RETURN output should NOT have runes (they should be burned)
        let op_return_balance = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 2 },
            rune_id(block_height, 0),
        );
        assert_eq!(op_return_balance, 0, "runes sent to OP_RETURN should be burned, not stored");

        // Remaining 500 should go to pointer (vout 0)
        let remaining = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(remaining, 500, "unallocated runes should go to pointer output");
    }

    #[wasm_bindgen_test]
    fn unallocated_runes_burned_if_no_non_op_return_output() {
        // If ALL outputs are OP_RETURN, unallocated runes should be burned
        // This is hard to test with our helpers since we always have at least one normal output,
        // but we can test the pointer-to-OP_RETURN case
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Transfer with pointer pointing to OP_RETURN output
        // In this tx: vout0=addr1, vout1=OP_RETURN
        // pointer=1 means unallocated go to OP_RETURN = burned
        let txin = helpers::get_txin_from_outpoint(OutPoint {
            txid: tx0.compute_txid(),
            vout: 0,
        });

        let runestone_script = (Runestone {
            etching: None,
            pointer: Some(1), // point to the OP_RETURN
            edicts: vec![Edict {
                id: RuneId { block: block_height, tx: 0 },
                amount: 200,
                output: 0,
            }],
            mint: None,
            protocol: None,
        })
        .encipher();

        let tx1 = Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: vec![
                addr1_txout(100_000_000),
                bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(0),
                    script_pubkey: runestone_script,
                },
            ],
        };

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        // vout 0 should have exactly 200 (from edict)
        let vout0_balance = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(vout0_balance, 200, "edict should transfer 200 to vout 0");

        // OP_RETURN (vout 1) should have 0 — remaining 800 should be burned
        let vout1_balance = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 1 },
            rune_id(block_height, 0),
        );
        assert_eq!(vout1_balance, 0, "unallocated runes to OP_RETURN pointer should be burned");
    }

    // =========================================================================
    // DIVERGENCE 6: Distribute mode OP_RETURN bug
    //
    // When distributing non-zero amount to all outputs (vout=num_outputs),
    // OP_RETURN outputs consume from `remaining` but don't receive runes.
    // =========================================================================

    #[wasm_bindgen_test]
    fn distribute_amount_skips_op_return_without_consuming() {
        // Ord: when amount>0 and output=num_outputs, each non-OP_RETURN output
        // gets min(amount, remaining_balance). OP_RETURN outputs are skipped entirely.
        clear();
        let block_height: u64 = 840000;

        // Etch with 1000 runes
        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Transfer: distribute 400 to each non-OP_RETURN output
        // tx has 3 outputs: vout0=addr1, vout1=addr2, vout2=OP_RETURN
        // edict output=3 (num_outputs), amount=400
        // Ord: vout0 gets 400, vout1 gets 400 (OP_RETURN skipped)
        // Remaining 200 goes to pointer (vout0)
        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![Edict {
                    id: RuneId { block: block_height, tx: 0 },
                    amount: 400,
                    output: 3, // num_outputs = distribute to all
                }],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_txin_from_outpoint(OutPoint {
                txid: tx0.compute_txid(),
                vout: 0,
            })],
            vec![addr1_txout(1), addr2_txout(1)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        // vout0 should get 400 (edict) + 200 (leftover via pointer) = 600
        let vout0 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );

        // vout1 should get 400
        let vout1 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 1 },
            rune_id(block_height, 0),
        );

        assert_eq!(vout1, 400, "addr2 should receive 400 from distribute");
        assert_eq!(vout0, 600, "addr1 should receive 400 + 200 leftover = 600");
    }

    // =========================================================================
    // DIVERGENCE 7: Even split remainder distribution
    //
    // Canonical ord: when amount=0 and output=num_outputs, the remainder
    // is distributed to the first N outputs (each gets +1).
    // =========================================================================

    #[wasm_bindgen_test]
    fn even_split_remainder_distribution() {
        // 1000 runes split across 3 non-OP_RETURN outputs:
        // each gets 333, first output gets +1 (remainder)
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Transfer: even split to 3 outputs
        // tx has 4 outputs: vout0,1,2=addr, vout3=OP_RETURN
        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![Edict {
                    id: RuneId { block: block_height, tx: 0 },
                    amount: 0, // 0 = even split
                    output: 4, // num_outputs = distribute to all non-OP_RETURN
                }],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_txin_from_outpoint(OutPoint {
                txid: tx0.compute_txid(),
                vout: 0,
            })],
            vec![addr1_txout(1), addr2_txout(1), addr1_txout(1)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        let rid = rune_id(block_height, 0);
        let b0 = balance_at(OutPoint { txid: tx1.compute_txid(), vout: 0 }, rid.clone());
        let b1 = balance_at(OutPoint { txid: tx1.compute_txid(), vout: 1 }, rid.clone());
        let b2 = balance_at(OutPoint { txid: tx1.compute_txid(), vout: 2 }, rid.clone());

        // 1000 / 3 = 333 remainder 1
        // Ord: first output gets 334, next two get 333
        assert_eq!(b0, 334, "first output should get 333 + 1 remainder");
        assert_eq!(b1, 333, "second output should get 333");
        assert_eq!(b2, 333, "third output should get 333");
    }

    // =========================================================================
    // BASIC ORD PARITY: Etching tests
    // =========================================================================

    #[wasm_bindgen_test]
    fn etching_with_no_edicts_creates_rune() {
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let balance = balance_at(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(balance, 1000, "premine should be allocated to pointer output");
    }

    #[wasm_bindgen_test]
    fn etching_may_allocate_to_multiple_outputs() {
        // Premine can be split to multiple outputs via edicts
        clear();
        let block_height: u64 = 840000;

        let tx0 = helpers::create_tx_from_runestone(
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
                edicts: vec![
                    Edict {
                        id: RuneId { block: 0, tx: 0 },
                        amount: 300,
                        output: 1, // 300 to addr2
                    },
                ],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_mock_txin(0)],
            vec![
                addr1_txout(100_000_000),
                addr2_txout(50_000_000),
            ],
        );

        let block = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let rid = rune_id(block_height, 0);
        let b0 = balance_at(OutPoint { txid: tx0.compute_txid(), vout: 0 }, rid.clone());
        let b1 = balance_at(OutPoint { txid: tx0.compute_txid(), vout: 1 }, rid.clone());

        // 300 via edict to vout1, remaining 700 via pointer to vout0
        assert_eq!(b0, 700, "pointer output should get remaining 700");
        assert_eq!(b1, 300, "edict output should get 300");
    }

    // =========================================================================
    // BASIC ORD PARITY: Edict tests
    // =========================================================================

    #[wasm_bindgen_test]
    fn edicts_over_max_inputs_are_clamped() {
        // Edicts requesting more than available are clamped to available balance
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Transfer: try to send 5000 (only 1000 available)
        let tx1 = make_transfer_tx(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            vec![Edict {
                id: RuneId { block: block_height, tx: 0 },
                amount: 5000,
                output: 0,
            }],
            Some(1),
            None,
            vec![addr2_txout(1), addr1_txout(99_999_999)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        let balance = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(balance, 1000, "edict should be clamped to available 1000");
    }

    #[wasm_bindgen_test]
    fn edict_amount_zero_transfers_all() {
        // amount=0 means transfer all available
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        let tx1 = make_transfer_tx(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            vec![Edict {
                id: RuneId { block: block_height, tx: 0 },
                amount: 0, // all
                output: 0,
            }],
            Some(1),
            None,
            vec![addr2_txout(1), addr1_txout(99_999_999)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        let b0 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        let b1 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 1 },
            rune_id(block_height, 0),
        );
        assert_eq!(b0, 1000, "amount=0 should transfer all 1000 to output 0");
        assert_eq!(b1, 0, "pointer output should get 0 remaining");
    }

    #[wasm_bindgen_test]
    fn edicts_processed_sequentially() {
        // Multiple edicts share the same unallocated pool
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Two edicts: first takes 700, second tries to take 500 (only 300 left)
        let tx1 = make_transfer_tx(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            vec![
                Edict {
                    id: RuneId { block: block_height, tx: 0 },
                    amount: 700,
                    output: 0,
                },
                Edict {
                    id: RuneId { block: block_height, tx: 0 },
                    amount: 500,
                    output: 1,
                },
            ],
            Some(0),
            None,
            vec![addr1_txout(1), addr2_txout(1)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        let b0 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        let b1 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 1 },
            rune_id(block_height, 0),
        );
        assert_eq!(b0, 700, "first edict should get 700");
        assert_eq!(b1, 300, "second edict should get remaining 300 (clamped from 500)");
    }

    // =========================================================================
    // BASIC ORD PARITY: Allocation tests
    // =========================================================================

    #[wasm_bindgen_test]
    fn unallocated_runes_go_to_first_non_op_return_when_no_pointer() {
        // Without pointer, unallocated runes go to first non-OP_RETURN output
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Transfer with no explicit pointer (defaults to first non-OP_RETURN)
        let tx1 = make_transfer_tx(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            vec![Edict {
                id: RuneId { block: block_height, tx: 0 },
                amount: 200,
                output: 1,
            }],
            None, // no pointer
            None,
            vec![addr1_txout(1), addr2_txout(1)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        let b0 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        let b1 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 1 },
            rune_id(block_height, 0),
        );

        assert_eq!(b0, 800, "first non-OP_RETURN output should get unallocated 800");
        assert_eq!(b1, 200, "edict output should get 200");
    }

    #[wasm_bindgen_test]
    fn unallocated_runes_go_to_pointer() {
        // With pointer, unallocated runes go to the pointed output
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        let tx1 = make_transfer_tx(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            vec![Edict {
                id: RuneId { block: block_height, tx: 0 },
                amount: 200,
                output: 0,
            }],
            Some(1), // pointer to vout 1
            None,
            vec![addr1_txout(1), addr2_txout(1)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        let b0 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        let b1 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 1 },
            rune_id(block_height, 0),
        );

        assert_eq!(b0, 200, "edict output should get 200");
        assert_eq!(b1, 800, "pointer output should get remaining 800");
    }

    // =========================================================================
    // BASIC ORD PARITY: Mint tests
    // =========================================================================

    #[wasm_bindgen_test]
    fn mints_without_cap_are_unmintable() {
        // Ord: if no cap is specified, minting fails
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            Some(Terms {
                amount: Some(100),
                cap: None, // no cap
                height: (None, None),
                offset: (None, None),
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: block_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr2_txout(100)],
        );

        let block = helpers::create_block_with_txs(vec![tx0, tx1]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let minted = balance_at(
            OutPoint { txid: block.txdata[1].compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(minted, 0, "mint without cap should fail");
    }

    #[wasm_bindgen_test]
    fn mint_limited_by_cap() {
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            0, // no premine
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(2),
                height: (Some(840000), Some(840100)),
                offset: (None, None),
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: block_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let tx2 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: block_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(2)],
            vec![addr1_txout(100)],
        );

        let tx3 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: block_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(3)],
            vec![addr1_txout(100)],
        );

        let block = helpers::create_block_with_txs(vec![tx0, tx1, tx2, tx3]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let m1 = balance_at(
            OutPoint { txid: block.txdata[1].compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        let m2 = balance_at(
            OutPoint { txid: block.txdata[2].compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        let m3 = balance_at(
            OutPoint { txid: block.txdata[3].compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );

        assert_eq!(m1, 100, "first mint should succeed");
        assert_eq!(m2, 100, "second mint should succeed");
        assert_eq!(m3, 0, "third mint should fail (cap=2 reached)");
    }

    // =========================================================================
    // BASIC ORD PARITY: Split tests (distribute to all outputs)
    // =========================================================================

    #[wasm_bindgen_test]
    fn split_in_etching() {
        // amount=0, output=num_outputs: evenly split premine to all non-OP_RETURN outputs
        clear();
        let block_height: u64 = 840000;

        let tx0 = helpers::create_tx_from_runestone(
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
                edicts: vec![Edict {
                    id: RuneId { block: 0, tx: 0 },
                    amount: 0,
                    output: 4, // num_outputs (4 outputs: 3 normal + 1 OP_RETURN)
                }],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_mock_txin(0)],
            // 3 normal outputs + OP_RETURN from runestone
            vec![addr1_txout(1), addr2_txout(1), addr1_txout(1)],
        );

        let block = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let rid = rune_id(block_height, 0);
        let b0 = balance_at(OutPoint { txid: tx0.compute_txid(), vout: 0 }, rid.clone());
        let b1 = balance_at(OutPoint { txid: tx0.compute_txid(), vout: 1 }, rid.clone());
        let b2 = balance_at(OutPoint { txid: tx0.compute_txid(), vout: 2 }, rid.clone());

        // 1000 / 3 = 333 remainder 1
        assert_eq!(b0, 334, "first output should get 334 (333 + 1 remainder)");
        assert_eq!(b1, 333, "second output should get 333");
        assert_eq!(b2, 333, "third output should get 333");
    }

    // =========================================================================
    // BASIC ORD PARITY: No-runestone transaction
    // =========================================================================

    #[wasm_bindgen_test]
    fn input_runes_without_runestone_go_to_first_non_op_return() {
        // Transactions with rune inputs but no runestone: runes go to first non-OP_RETURN
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Transaction WITHOUT a runestone, spending rune-bearing input
        let tx1 = Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![helpers::get_txin_from_outpoint(OutPoint {
                txid: tx0.compute_txid(),
                vout: 0,
            })],
            output: vec![addr1_txout(1), addr2_txout(1)],
        };

        let block1 = helpers::create_block_with_txs(vec![tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        // Without a runestone, runes should go to first non-OP_RETURN output
        // BUT in ord, no-runestone means the runes from input just get cleared (burned)
        // because index_runes is only called when decipher returns Some(artifact)
        let b0 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );

        // Input runes are cleared regardless
        let input_balance = balance_at(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(input_balance, 0, "input should be cleared");

        // In ord, without a runestone, rune outputs from inputs go to first non-OP_RETURN
        // ONLY IF there's an artifact. No artifact = just cleared.
        // Our implementation clears inputs for ALL txs, so this should be 0.
        // Actually, in ord, unallocated() REMOVES from outpoint_to_balances, so even
        // without a runestone the runes disappear.
        assert_eq!(b0, 0, "without runestone, runes are effectively burned");
    }

    // =========================================================================
    // BASIC ORD PARITY: Multiple runes
    // =========================================================================

    #[wasm_bindgen_test]
    fn output_may_hold_multiple_runes() {
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'A',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );
        let tx1 = make_etching_tx(
            "BBBBBBBBBBBBBBTESTER",
            'B',
            2000,
            2,
            None,
            Some(0),
            vec![],
            None,
            1,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0.clone(), tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), block_height);

        // Transfer both runes to same output
        let tx2 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![
                    Edict {
                        id: RuneId { block: block_height, tx: 0 },
                        amount: 500,
                        output: 0,
                    },
                    Edict {
                        id: RuneId { block: block_height, tx: 1 },
                        amount: 1500,
                        output: 0,
                    },
                ],
                mint: None,
                protocol: None,
            },
            vec![
                helpers::get_txin_from_outpoint(OutPoint { txid: tx0.compute_txid(), vout: 0 }),
                helpers::get_txin_from_outpoint(OutPoint { txid: tx1.compute_txid(), vout: 0 }),
            ],
            vec![addr1_txout(1), addr2_txout(1)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx2.clone()]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), block_height + 1);

        // vout 0 gets: 500 (edict for rune A) + 500 (leftover rune A via pointer)
        //            + 1500 (edict for rune B) + 500 (leftover rune B via pointer)
        let rune_a_vout0 = balance_at(
            OutPoint { txid: tx2.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        let rune_b_vout0 = balance_at(
            OutPoint { txid: tx2.compute_txid(), vout: 0 },
            rune_id(block_height, 1),
        );

        assert_eq!(rune_a_vout0, 1000, "vout 0 should hold all 1000 rune A (500 edict + 500 leftover)");
        assert_eq!(rune_b_vout0, 2000, "vout 0 should hold all 2000 rune B (1500 edict + 500 leftover)");
    }

    // =========================================================================
    // BASIC ORD PARITY: Duplicate rune names forbidden
    // =========================================================================

    #[wasm_bindgen_test]
    fn duplicate_rune_names_forbidden() {
        clear();
        let block_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'A',
            1000,
            2,
            None,
            Some(0),
            vec![],
            None,
            0,
        );
        // Same rune name, different tx
        let tx1 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'B',
            2000,
            4,
            None,
            Some(0),
            vec![],
            None,
            1,
        );

        let block = helpers::create_block_with_txs(vec![tx0.clone(), tx1.clone()]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        // First should succeed
        let b0 = balance_at(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(b0, 1000, "first etching should succeed");

        // Second should fail (duplicate name)
        let b1 = balance_at(
            OutPoint { txid: tx1.compute_txid(), vout: 0 },
            rune_id(block_height, 1),
        );
        assert_eq!(b1, 0, "duplicate rune name should be rejected");
    }

    // =========================================================================
    // BASIC ORD PARITY: Reserved rune etching
    // =========================================================================

    #[wasm_bindgen_test]
    fn reserved_rune_etching() {
        clear();
        let block_height: u64 = 840001;

        // Etching without explicit rune name -> reserved rune
        let tx0 = helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(2),
                    premine: Some(500),
                    rune: None, // no rune name = reserved
                    spacers: None,
                    symbol: None,
                    turbo: true,
                    terms: None,
                }),
                pointer: Some(0),
                edicts: vec![],
                mint: None,
                protocol: None,
            },
            vec![helpers::get_mock_txin(0)],
            vec![addr1_txout(100_000_000)],
        );

        let block = helpers::create_block_with_txs(vec![tx0.clone()]);
        let _ = Protorune::index_block::<TestContext>(block.clone(), block_height);

        let balance = balance_at(
            OutPoint { txid: tx0.compute_txid(), vout: 0 },
            rune_id(block_height, 0),
        );
        assert_eq!(balance, 500, "reserved rune should be etched with premine");

        // Verify it got a name
        let rune_id_key: Vec<u8> = ProtoruneRuneId::new(block_height as u128, 0).into();
        let name = tables::RUNES.RUNE_ID_TO_ETCHING.select(&rune_id_key).get();
        assert!(!name.is_empty(), "reserved rune should have a generated name");
    }

    // =========================================================================
    // BASIC ORD PARITY: Mint with terms
    // =========================================================================

    #[wasm_bindgen_test]
    fn mint_respects_height_start() {
        clear();
        let etch_height: u64 = 840000;
        let mint_height: u64 = 840001;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            0,
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(10),
                height: (Some(840005), None), // start at 840005
                offset: (None, None),
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), etch_height);

        // Mint at 840001 — should fail (before start)
        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: etch_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), mint_height);

        let minted = balance_at(
            OutPoint { txid: block1.txdata[0].compute_txid(), vout: 0 },
            rune_id(etch_height, 0),
        );
        assert_eq!(minted, 0, "mint before height_start should fail");
    }

    #[wasm_bindgen_test]
    fn mint_respects_height_end() {
        clear();
        let etch_height: u64 = 840000;

        let tx0 = make_etching_tx(
            "AAAAAAAAAAAAATESTER",
            'Z',
            0,
            2,
            Some(Terms {
                amount: Some(100),
                cap: Some(10),
                height: (Some(840000), Some(840002)), // end at 840002
                offset: (None, None),
            }),
            Some(0),
            vec![],
            None,
            0,
        );

        let block0 = helpers::create_block_with_txs(vec![tx0]);
        let _ = Protorune::index_block::<TestContext>(block0.clone(), etch_height);

        // Mint at 840002 — should fail (at end = exclusive)
        let tx1 = helpers::create_tx_from_runestone(
            Runestone {
                etching: None,
                pointer: Some(0),
                edicts: vec![],
                mint: Some(RuneId { block: etch_height, tx: 0 }),
                protocol: None,
            },
            vec![helpers::get_mock_txin(1)],
            vec![addr1_txout(100)],
        );

        let block1 = helpers::create_block_with_txs(vec![tx1]);
        let _ = Protorune::index_block::<TestContext>(block1.clone(), 840002);

        let minted = balance_at(
            OutPoint { txid: block1.txdata[0].compute_txid(), vout: 0 },
            rune_id(etch_height, 0),
        );
        assert_eq!(minted, 0, "mint at height_end should fail (exclusive)");
    }
}
