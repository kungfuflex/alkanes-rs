#[cfg(test)]
mod tests {
    use crate::balance_sheet::{load_sheet, PersistentRecord, MAX_VIEW_SHEET_ENTRIES};
    use crate::message::MessageContext;
    use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
    use protorune_support::proto::protorune::{
        Outpoint as OutpointProto, OutpointResponse, Rune as RuneProto, RunesByHeightRequest,
        WalletRequest,
    };

    use crate::test_helpers::{self as helpers, RunesTestingConfig, ADDRESS1, ADDRESS2};
    use crate::Protorune;
    use crate::{message::MessageContextParcel, tables, view};
    use anyhow::Result;
    use protorune_support::rune_transfer::RuneTransfer;
    use protorune_support::utils::consensus_encode;

    use bitcoin::hashes::Hash;
    use bitcoin::OutPoint;

    use helpers::clear;
    #[allow(unused_imports)]
    use metashrew_core::{
        println,
        stdio::{stdout, Write},
    };
    use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
    use metashrew_support::index_pointer::KeyValuePointer;
    use ordinals::{Edict, RuneId};

    use prost::Message;

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

    /// Etch a rune, then query its outpoint via runes_by_outpoint and verify the balance is returned.
    #[wasm_bindgen_test]
    fn test_runes_by_outpoint_with_balance() {
        clear();
        let (test_block, config) = helpers::create_block_with_rune_tx(None);
        let _ =
            Protorune::index_block::<MyMessageContext>(test_block.clone(), config.rune_etch_height);

        let outpoint = OutPoint {
            txid: test_block.txdata[0].compute_txid(),
            vout: 0,
        };

        let req = (OutpointProto {
            txid: outpoint.txid.as_byte_array().to_vec(),
            vout: outpoint.vout,
        })
        .encode_to_vec();

        let response = view::runes_by_outpoint(&req).unwrap();

        // Verify the outpoint is returned correctly
        let resp_outpoint = response.outpoint.unwrap();
        assert_eq!(resp_outpoint.txid, outpoint.txid.as_byte_array().to_vec());
        assert_eq!(resp_outpoint.vout, 0);

        // Verify height and txindex
        assert_eq!(response.height, config.rune_etch_height as u32);
        assert_eq!(response.txindex, config.rune_etch_vout);

        // Verify balance sheet has the etched rune with premine of 1000
        let balances = response.balances.unwrap();
        assert_eq!(balances.entries.len(), 1);
        assert_eq!(balances.entries[0].balance.clone().unwrap().lo, 1000);
    }

    /// Query an outpoint that has no runes and verify an empty balance sheet is returned.
    #[wasm_bindgen_test]
    fn test_runes_by_outpoint_empty() {
        clear();
        // Index a block with a simple (non-rune) transaction
        let test_block = helpers::create_block_with_sample_tx();
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), 840001);

        let outpoint = OutPoint {
            txid: test_block.txdata[0].compute_txid(),
            vout: 0,
        };

        let req = (OutpointProto {
            txid: outpoint.txid.as_byte_array().to_vec(),
            vout: outpoint.vout,
        })
        .encode_to_vec();

        let response = view::runes_by_outpoint(&req).unwrap();

        // Balance sheet should have no entries since this outpoint holds no runes
        let balances = response.balances.unwrap();
        assert_eq!(balances.entries.len(), 0);
    }

    /// Etch a rune at height H, then query runes_by_height at height H and verify
    /// the rune metadata (name, symbol, divisibility) is correct.
    #[wasm_bindgen_test]
    fn test_runes_by_height_returns_etched_runes() {
        clear();
        let (test_block, config) = helpers::create_block_with_rune_tx(None);
        let _ =
            Protorune::index_block::<MyMessageContext>(test_block.clone(), config.rune_etch_height);

        let req: Vec<u8> = (RunesByHeightRequest {
            height: config.rune_etch_height,
        })
        .encode_to_vec();

        let response = view::runes_by_height(&req).unwrap();
        let runes: Vec<RuneProto> = response.runes;

        assert_eq!(runes.len(), 1);
        assert_eq!(runes[0].name, "AAAAAAAAAAAAATESTER");
        assert_eq!(runes[0].symbol, "Z");
        assert_eq!(runes[0].divisibility, 2);

        // Verify the rune_id is populated
        let rune_id = runes[0].rune_id.clone().unwrap();
        assert_eq!(rune_id.height.unwrap().lo, config.rune_etch_height);
        assert_eq!(rune_id.txindex.unwrap().lo, config.rune_etch_vout as u64);
    }

    /// Query runes_by_height at a height with no etchings and verify an empty response.
    #[wasm_bindgen_test]
    fn test_runes_by_height_no_runes() {
        clear();
        // Index a block with no rune etchings
        let test_block = helpers::create_block_with_sample_tx();
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), 840001);

        let req: Vec<u8> = (RunesByHeightRequest { height: 840001 }).encode_to_vec();

        let response = view::runes_by_height(&req).unwrap();
        assert_eq!(response.runes.len(), 0);
    }

    /// Etch a rune, transfer it to address2 via an edict, then query runes_by_address
    /// for address2 and verify the balance is present.
    #[wasm_bindgen_test]
    fn test_runes_by_address_after_transfer() {
        clear();
        let config = RunesTestingConfig::default();
        let rune_id = RuneId::new(config.rune_etch_height, config.rune_etch_vout).unwrap();

        // Transfer 200 runes to address2 (vout 0), remainder stays at address1 (vout 1)
        let edicts = vec![Edict {
            id: rune_id,
            amount: 200,
            output: 0,
        }];

        let test_block = helpers::create_block_with_rune_transfer(&config, edicts);
        let _ =
            Protorune::index_block::<MyMessageContext>(test_block.clone(), config.rune_etch_height);

        // Query runes_by_address for address2
        let req = (WalletRequest {
            wallet: ADDRESS2().as_bytes().to_vec(),
        })
        .encode_to_vec();

        let response = view::runes_by_address(&req).unwrap();
        let outpoints: Vec<OutpointResponse> = response.outpoints;

        // address2 should have an outpoint with 200 runes
        assert!(!outpoints.is_empty(), "expected at least one outpoint for address2");

        // Find the outpoint from the transfer tx (tx1, vout 0)
        let transfer_txid = test_block.txdata[1].compute_txid();
        let matching = outpoints
            .iter()
            .find(|op| {
                let proto_outpoint = op.outpoint.as_ref().unwrap();
                proto_outpoint.txid == transfer_txid.as_byte_array().to_vec()
                    && proto_outpoint.vout == 0
            });

        assert!(matching.is_some(), "expected outpoint from transfer tx for address2");

        let matched = matching.unwrap();
        let balances = matched.balances.as_ref().unwrap();
        assert_eq!(balances.entries.len(), 1);
        assert_eq!(balances.entries[0].balance.clone().unwrap().lo, 200);
    }

    // ---- cursor views ----------------------------------------------------

    use protorune_support::proto::protorune::{
        OutpointCursorRequest, OutpointCursorResponse, OutpointWithProtocol,
        ProtorunesWalletCursorRequest, WalletCursorResponse,
    };

    /// Etch a rune, then give protocol 100 a balance sheet of `n` distinct
    /// entries (rune 2:i+1 holding i+1) on the etched outpoint.
    fn setup_big_protorune_sheet(n: u128) -> OutPoint {
        clear();
        let (test_block, config) = helpers::create_block_with_rune_tx(None);
        let _ =
            Protorune::index_block::<MyMessageContext>(test_block.clone(), config.rune_etch_height);
        let outpoint = OutPoint {
            txid: test_block.txdata[0].compute_txid(),
            vout: 0,
        };
        let ptr = tables::RuneTable::for_protocol(100)
            .OUTPOINT_TO_RUNES
            .select(&view::outpoint_to_bytes(&outpoint).unwrap());
        let mut sheet: BalanceSheet<IndexPointer> = BalanceSheet::default();
        for i in 0..n {
            sheet.set(&ProtoruneRuneId::new(2, i + 1), i + 1);
        }
        sheet.save(&ptr, false);
        outpoint
    }

    fn outpoint_cursor(outpoint: &OutPoint, cursor: u32, limit: u32) -> OutpointCursorResponse {
        let req = OutpointCursorRequest {
            txid: outpoint.txid.as_byte_array().to_vec(),
            vout: outpoint.vout,
            protocol: Some(100u128.into()),
            cursor,
            limit,
        }
        .encode_to_vec();
        view::protorunes_by_outpoint_cursor(&req).unwrap()
    }

    /// Following next_cursor streams the whole sheet exactly once in pages of
    /// at most MAX_VIEW_SHEET_ENTRIES; an oversized limit is clamped.
    #[wasm_bindgen_test]
    fn test_protorunes_by_outpoint_cursor_streams_whole_sheet() {
        let outpoint = setup_big_protorune_sheet(2500);
        let mut seen = std::collections::BTreeMap::new();
        let mut cursor = 0;
        let mut pages = 0;
        loop {
            let page = outpoint_cursor(&outpoint, cursor, 100_000);
            assert_eq!(page.total_entries, 2500);
            let o = page.outpoint.unwrap();
            assert_eq!(o.outpoint.unwrap().vout, outpoint.vout);
            let entries = o.balances.unwrap().entries;
            assert!(entries.len() <= MAX_VIEW_SHEET_ENTRIES as usize);
            for e in entries {
                let id = e.rune.unwrap().rune_id.unwrap().txindex.unwrap().lo;
                let bal = e.balance.unwrap().lo;
                assert_eq!(bal, id, "rune 2:i holds i");
                assert!(seen.insert(id, bal).is_none(), "entry {} seen twice", id);
            }
            pages += 1;
            if page.next_cursor == 0 {
                break;
            }
            cursor = page.next_cursor;
        }
        assert_eq!(pages, 3);
        assert_eq!(seen.len(), 2500);
    }

    /// Small limits page exactly; a cursor at or past the end is empty and
    /// ends the walk.
    #[wasm_bindgen_test]
    fn test_protorunes_by_outpoint_cursor_small_pages_and_past_end() {
        let outpoint = setup_big_protorune_sheet(25);
        let page = outpoint_cursor(&outpoint, 0, 10);
        assert_eq!(page.outpoint.unwrap().balances.unwrap().entries.len(), 10);
        assert_eq!(page.next_cursor, 10);
        let page = outpoint_cursor(&outpoint, 20, 10);
        assert_eq!(page.outpoint.unwrap().balances.unwrap().entries.len(), 5);
        assert_eq!(page.next_cursor, 0);
        let page = outpoint_cursor(&outpoint, 50, 10);
        assert!(page.outpoint.unwrap().balances.unwrap().entries.is_empty());
        assert_eq!(page.next_cursor, 0);
        assert_eq!(page.total_entries, 25);
    }

    /// The legacy view is unchanged: it still returns the whole sheet in one
    /// response, with no cap.
    #[wasm_bindgen_test]
    fn test_legacy_protorunes_by_outpoint_still_returns_whole_sheet() {
        let outpoint = setup_big_protorune_sheet(2500);
        let req = OutpointWithProtocol {
            txid: outpoint.txid.as_byte_array().to_vec(),
            vout: outpoint.vout,
            protocol: Some(100u128.into()),
        }
        .encode_to_vec();
        let response = view::protorunes_by_outpoint(&req).unwrap();
        assert_eq!(response.balances.unwrap().entries.len(), 2500);
    }

    const WALLET_HEIGHT: u64 = 840001;

    /// Index one block paying `n` outputs to ADDRESS2 and give protocol 100 a
    /// sheet of `entries` distinct runes (3:j+1) on each. Returns the txid.
    fn setup_wallet(n: u32, entries: u128) -> bitcoin::Txid {
        clear();
        let funding = bitcoin::Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![helpers::get_mock_txin(1)],
            output: (0..n)
                .map(|_| helpers::get_txout_transfer_to_address(&ADDRESS2(), 1000))
                .collect(),
        };
        let block = helpers::create_block_with_txs(vec![
            helpers::create_coinbase_transaction(WALLET_HEIGHT as u32),
            funding.clone(),
        ]);
        let _ = Protorune::index_block::<MyMessageContext>(block, WALLET_HEIGHT);
        let txid = funding.compute_txid();
        for vout in 0..n {
            let ptr = tables::RuneTable::for_protocol(100)
                .OUTPOINT_TO_RUNES
                .select(&view::outpoint_to_bytes(&OutPoint { txid, vout }).unwrap());
            let mut sheet: BalanceSheet<IndexPointer> = BalanceSheet::default();
            for j in 0..entries {
                sheet.set(&ProtoruneRuneId::new(3, j + 1), 1);
            }
            sheet.save(&ptr, false);
        }
        txid
    }

    /// Spend outputs `vouts` of `txid` in a later block.
    fn spend(txid: bitcoin::Txid, vouts: std::ops::Range<u32>) {
        let spender = bitcoin::Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vouts
                .map(|vout| helpers::get_txin_from_outpoint(OutPoint { txid, vout }))
                .collect(),
            output: vec![helpers::get_txout_transfer_to_address(&ADDRESS1(), 1000)],
        };
        let block = helpers::create_block_with_txs(vec![
            helpers::create_coinbase_transaction(WALLET_HEIGHT as u32 + 1),
            spender,
        ]);
        let _ = Protorune::index_block::<MyMessageContext>(block, WALLET_HEIGHT + 1);
    }

    fn address_cursor(outpoint_cursor: u32, entry_cursor: u32, limit: u32) -> WalletCursorResponse {
        let req = ProtorunesWalletCursorRequest {
            wallet: ADDRESS2().as_bytes().to_vec(),
            protocol_tag: Some(100u128.into()),
            outpoint_cursor,
            entry_cursor,
            limit,
        }
        .encode_to_vec();
        view::protorunes_by_address_cursor(&req).unwrap()
    }

    /// Walk the address cursor to the end; returns (vout, entries) per
    /// outpoint slice in order, and the number of pages.
    fn walk_address(limit: u32) -> (Vec<(u32, usize)>, u32) {
        let (mut oc, mut ec, mut pages) = (0, 0, 0);
        let mut slices = Vec::new();
        loop {
            let page = address_cursor(oc, ec, limit);
            pages += 1;
            for o in &page.outpoints {
                slices.push((
                    o.outpoint.as_ref().unwrap().vout,
                    o.balances.as_ref().unwrap().entries.len(),
                ));
            }
            if page.done {
                break;
            }
            assert!(
                (page.next_outpoint_cursor, page.next_entry_cursor) != (oc, ec),
                "cursor must advance"
            );
            oc = page.next_outpoint_cursor;
            ec = page.next_entry_cursor;
            assert!(pages < 1000, "walk did not terminate");
        }
        (slices, pages)
    }

    /// A small wallet comes back in one page, every outpoint whole.
    #[wasm_bindgen_test]
    fn test_protorunes_by_address_cursor_small_wallet_one_page() {
        let txid = setup_wallet(5, 1);
        let page = address_cursor(0, 0, 0);
        assert!(page.done);
        assert_eq!(page.total_outpoints, 5);
        let vouts: Vec<u32> = page
            .outpoints
            .iter()
            .map(|o| o.outpoint.as_ref().unwrap().vout)
            .collect();
        assert_eq!(vouts, vec![0, 1, 2, 3, 4]);
        for o in &page.outpoints {
            assert_eq!(o.outpoint.as_ref().unwrap().txid, txid.as_byte_array().to_vec());
            assert_eq!(o.balances.as_ref().unwrap().entries.len(), 1);
            assert_eq!(o.height, WALLET_HEIGHT as u32);
            assert_eq!(o.txindex, 1);
        }
    }

    /// Big outpoints are split across pages by entry_cursor: every page holds
    /// at most the page cap, and the slices add up to each whole sheet.
    #[wasm_bindgen_test]
    fn test_protorunes_by_address_cursor_splits_big_outpoints() {
        setup_wallet(3, 2500);
        let (slices, pages) = walk_address(0);
        assert_eq!(pages, 8, "7500 entries in pages of 1000");
        let mut per_vout = std::collections::BTreeMap::new();
        for (vout, n) in &slices {
            assert!(*n <= MAX_VIEW_SHEET_ENTRIES as usize);
            *per_vout.entry(*vout).or_insert(0usize) += n;
        }
        assert_eq!(
            per_vout.into_iter().collect::<Vec<_>>(),
            vec![(0, 2500), (1, 2500), (2, 2500)]
        );
        // A small limit walks the same data in more pages.
        let (slices, _) = walk_address(700);
        assert_eq!(slices.iter().map(|(_, n)| n).sum::<usize>(), 7500);
    }

    /// Spent outpoints are skipped.
    #[wasm_bindgen_test]
    fn test_protorunes_by_address_cursor_skips_spent_outpoints() {
        let txid = setup_wallet(6, 1);
        spend(txid, 0..3);
        let (slices, _) = walk_address(2);
        assert_eq!(
            slices.iter().map(|(v, _)| *v).collect::<Vec<_>>(),
            vec![3, 4, 5]
        );
    }
}
