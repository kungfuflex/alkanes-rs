#[cfg(test)]
mod tests {
    use crate::balance_sheet::{load_sheet, PersistentRecord, MAX_VIEW_SHEET_ENTRIES};
    use crate::view::MAX_VIEW_WALLET_ENTRIES;
    use crate::message::MessageContext;
    use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
    use protorune_support::proto::protorune::{
        Outpoint as OutpointProto, OutpointResponse, OutpointWithProtocol,
        ProtorunesWalletRequest, Rune as RuneProto, RunesByHeightRequest, WalletRequest,
        WalletResponse,
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
            ..Default::default()
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

    /// Etch a rune, then stuff `n` extra entries into protocol 100's balance
    /// sheet for the etched outpoint so the stored `/runes` length exceeds the
    /// per-view cap. Returns the outpoint.
    fn setup_oversized_protorune_sheet(n: u128) -> OutPoint {
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

    fn protorunes_by_outpoint_page(
        outpoint: &OutPoint,
        offset: u32,
        limit: u32,
    ) -> OutpointResponse {
        let req = OutpointWithProtocol {
            txid: outpoint.txid.as_byte_array().to_vec(),
            vout: outpoint.vout,
            protocol: Some(100u128.into()),
            offset,
            limit,
        }
        .encode_to_vec();
        view::protorunes_by_outpoint(&req).unwrap()
    }

    /// A request without pagination fields gets at most MAX_VIEW_SHEET_ENTRIES
    /// entries, plus the total and the offset of the next page.
    #[wasm_bindgen_test]
    fn test_protorunes_by_outpoint_caps_default_request() {
        let outpoint = setup_oversized_protorune_sheet(2500);
        let response = protorunes_by_outpoint_page(&outpoint, 0, 0);
        assert_eq!(
            response.balances.unwrap().entries.len(),
            MAX_VIEW_SHEET_ENTRIES as usize
        );
        assert_eq!(response.total_entries, 2500);
        assert_eq!(response.next_offset, MAX_VIEW_SHEET_ENTRIES);

        // The legacy 3-field encoding is wire-identical to offset=0, limit=0.
        let legacy = protorune_support::proto::protorune::OutpointWithProtocol {
            txid: outpoint.txid.as_byte_array().to_vec(),
            vout: outpoint.vout,
            protocol: Some(100u128.into()),
            ..Default::default()
        }
        .encode_to_vec();
        let legacy_response = view::protorunes_by_outpoint(&legacy).unwrap();
        assert_eq!(legacy_response.balances.unwrap().entries.len(), 1000);
    }

    /// Walking next_offset visits every stored entry exactly once, and an
    /// oversized limit is clamped to the cap.
    #[wasm_bindgen_test]
    fn test_protorunes_by_outpoint_pagination_walks_whole_sheet() {
        let outpoint = setup_oversized_protorune_sheet(2500);
        let mut seen = std::collections::BTreeSet::new();
        let mut offset = 0;
        let mut pages = 0;
        loop {
            let response = protorunes_by_outpoint_page(&outpoint, offset, 100_000);
            let entries = response.balances.unwrap().entries;
            assert!(entries.len() <= MAX_VIEW_SHEET_ENTRIES as usize);
            for entry in entries {
                let id = entry.rune.unwrap().rune_id.unwrap();
                assert!(seen.insert(id.txindex.unwrap().lo));
            }
            pages += 1;
            if response.next_offset == 0 {
                break;
            }
            offset = response.next_offset;
        }
        assert_eq!(pages, 3);
        assert_eq!(seen.len(), 2500);
    }

    /// Small limits page exactly; an offset past the end returns no entries.
    #[wasm_bindgen_test]
    fn test_protorunes_by_outpoint_small_page_and_past_end() {
        let outpoint = setup_oversized_protorune_sheet(25);
        let response = protorunes_by_outpoint_page(&outpoint, 20, 10);
        assert_eq!(response.balances.unwrap().entries.len(), 5);
        assert_eq!(response.total_entries, 25);
        assert_eq!(response.next_offset, 0);

        let response = protorunes_by_outpoint_page(&outpoint, 10, 10);
        assert_eq!(response.balances.unwrap().entries.len(), 10);
        assert_eq!(response.next_offset, 20);

        let response = protorunes_by_outpoint_page(&outpoint, 100, 10);
        assert_eq!(response.balances.unwrap().entries.len(), 0);
        assert_eq!(response.total_entries, 25);
        assert_eq!(response.next_offset, 0);
    }

    /// A sheet under the cap comes back whole, exactly as before.
    #[wasm_bindgen_test]
    fn test_runes_by_outpoint_reports_total_entries() {
        clear();
        let (test_block, config) = helpers::create_block_with_rune_tx(None);
        let _ =
            Protorune::index_block::<MyMessageContext>(test_block.clone(), config.rune_etch_height);
        let req = (OutpointProto {
            txid: test_block.txdata[0].compute_txid().as_byte_array().to_vec(),
            vout: 0,
        })
        .encode_to_vec();
        let response = view::runes_by_outpoint(&req).unwrap();
        assert_eq!(response.balances.unwrap().entries.len(), 1);
        assert_eq!(response.total_entries, 1);
        assert_eq!(response.next_offset, 0);
    }

    const WALLET_HEIGHT: u64 = 840001;

    /// Index one block paying `n` outputs to ADDRESS2, and give protocol 100
    /// a balance sheet of `entries` (distinct runes) on every one of them.
    /// Returns the funding tx's txid; output `i` is stored at list position `i`.
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

    fn protorunes_by_address_page(offset: u32, limit: u32) -> WalletResponse {
        let req = ProtorunesWalletRequest {
            wallet: ADDRESS2().as_bytes().to_vec(),
            protocol_tag: Some(100u128.into()),
            offset,
            limit,
        }
        .encode_to_vec();
        view::protorunes_by_address(&req).unwrap()
    }

    fn vouts(response: &WalletResponse) -> Vec<u32> {
        response
            .outpoints
            .iter()
            .map(|o| o.outpoint.as_ref().unwrap().vout)
            .collect()
    }

    /// A legacy request (no pagination fields) on a small wallet returns every
    /// outpoint in one page, with the correct txindex from the indexed lookup.
    #[wasm_bindgen_test]
    fn test_protorunes_by_address_legacy_request_small_wallet() {
        let txid = setup_wallet(5, 1);
        let legacy = ProtorunesWalletRequest {
            wallet: ADDRESS2().as_bytes().to_vec(),
            protocol_tag: Some(100u128.into()),
            ..Default::default()
        }
        .encode_to_vec();
        let response = view::protorunes_by_address(&legacy).unwrap();
        assert_eq!(vouts(&response), vec![0, 1, 2, 3, 4]);
        assert_eq!(response.total_outpoints, 5);
        assert_eq!(response.next_offset, 0);
        for o in &response.outpoints {
            assert_eq!(o.outpoint.as_ref().unwrap().txid, txid.as_byte_array().to_vec());
            assert_eq!(o.balances.as_ref().unwrap().entries.len(), 1);
        }
    }

    /// `limit` bounds the positions scanned, and walking next_offset visits
    /// every outpoint exactly once, in stored order.
    #[wasm_bindgen_test]
    fn test_protorunes_by_address_pages_by_limit() {
        setup_wallet(10, 1);
        let first = protorunes_by_address_page(0, 4);
        assert_eq!(vouts(&first), vec![0, 1, 2, 3]);
        assert_eq!(first.total_outpoints, 10);
        assert_eq!(first.next_offset, 4);

        let mut all = Vec::new();
        let mut offset = 0;
        loop {
            let page = protorunes_by_address_page(offset, 3);
            all.extend(vouts(&page));
            if page.next_offset == 0 {
                break;
            }
            offset = page.next_offset;
        }
        assert_eq!(all, (0..10).collect::<Vec<u32>>());

        let past_end = protorunes_by_address_page(50, 3);
        assert!(past_end.outpoints.is_empty());
        assert_eq!(past_end.next_offset, 0);
    }

    /// Spent outpoints are skipped but still count against the scan window, so
    /// a page can be empty while next_offset says to keep going.
    #[wasm_bindgen_test]
    fn test_protorunes_by_address_skips_spent_outpoints() {
        let txid = setup_wallet(6, 1);
        spend(txid, 0..3);
        let page = protorunes_by_address_page(0, 3);
        assert!(page.outpoints.is_empty());
        assert_eq!(page.next_offset, 3);
        let page = protorunes_by_address_page(3, 3);
        assert_eq!(vouts(&page), vec![3, 4, 5]);
        assert_eq!(page.next_offset, 0);
    }

    /// Once the summed sheet entries would pass MAX_VIEW_WALLET_ENTRIES the
    /// page ends early; the next page picks up at the first outpoint left out.
    #[wasm_bindgen_test]
    fn test_protorunes_by_address_ends_page_at_entry_budget() {
        let per_outpoint = MAX_VIEW_SHEET_ENTRIES as u128;
        let fits = (MAX_VIEW_WALLET_ENTRIES / MAX_VIEW_SHEET_ENTRIES) as u32;
        setup_wallet(fits + 2, per_outpoint);

        let first = protorunes_by_address_page(0, 0);
        assert_eq!(vouts(&first), (0..fits).collect::<Vec<u32>>());
        assert_eq!(first.next_offset, fits);

        let rest = protorunes_by_address_page(first.next_offset, 0);
        assert_eq!(vouts(&rest), vec![fits, fits + 1]);
        assert_eq!(rest.next_offset, 0);
    }

    /// An outpoint whose sheet exceeds the per-outpoint cap still comes back
    /// first on its page, truncated, with total_entries/next_offset set so the
    /// rest can be paged through protorunesbyoutpoint.
    #[wasm_bindgen_test]
    fn test_protorunes_by_address_oversized_outpoint_sheet() {
        setup_wallet(2, 2500);
        let page = protorunes_by_address_page(1, 1);
        assert_eq!(vouts(&page), vec![1]);
        let o = &page.outpoints[0];
        assert_eq!(
            o.balances.as_ref().unwrap().entries.len(),
            MAX_VIEW_SHEET_ENTRIES as usize
        );
        assert_eq!(o.total_entries, 2500);
        assert_eq!(o.next_offset, MAX_VIEW_SHEET_ENTRIES);
    }

    /// runes_by_address goes through the same bounded scan.
    #[wasm_bindgen_test]
    fn test_runes_by_address_pages_by_limit() {
        setup_wallet(5, 0);
        let req = WalletRequest {
            wallet: ADDRESS2().as_bytes().to_vec(),
            offset: 1,
            limit: 2,
        }
        .encode_to_vec();
        let response = view::runes_by_address(&req).unwrap();
        assert_eq!(vouts(&response), vec![1, 2]);
        assert_eq!(response.total_outpoints, 5);
        assert_eq!(response.next_offset, 3);
    }

    /// With empty sheets, height/txindex come from the block indexes. The
    /// TXID_TO_TXINDEX fast path and the full-scan fallback must agree.
    #[wasm_bindgen_test]
    fn test_wallet_txindex_fast_path_matches_scan() {
        let txid = setup_wallet(2, 0);
        let req = WalletRequest {
            wallet: ADDRESS2().as_bytes().to_vec(),
            ..Default::default()
        }
        .encode_to_vec();
        let fast = view::runes_by_address(&req).unwrap();
        // The funding tx sits after the coinbase.
        assert!(fast.outpoints.iter().all(|o| o.txindex == 1));
        assert!(fast.outpoints.iter().all(|o| o.height == WALLET_HEIGHT as u32));

        tables::RUNES
            .TXID_TO_TXINDEX
            .select(&txid.as_byte_array().to_vec())
            .nullify();
        let scanned = view::runes_by_address(&req).unwrap();
        assert_eq!(fast, scanned);
    }
}
