//! Regression pins for the LAZY (height, txindex) resolution in the outpoint
//! views (`view::resolve_height_txindex`, introduced by the 2026-08-20
//! wallet-blackout fix — see the doc comment on that function in view.rs).
//!
//! What is pinned here, and why each pin exists:
//!
//! 1. THE WIRE CONVENTION: when an outpoint's balance sheet is NON-empty, the
//!    response's `height`/`txindex` fields carry the FIRST RUNE ID
//!    (rune_id.block / rune_id.tx) — NOT the outpoint's real block position.
//!    This predates the perf fix (the old code computed the real position and
//!    then overwrote it); live mainnet responses show e.g. height=2/32 (rune
//!    blocks). Consumers rely on it, so it must never silently change.
//!
//! 2. THE LAZY SCAN: the expensive `HEIGHT_TO_TRANSACTION_IDS.get_list()`
//!    block walk (one host KV read per transaction in the block — 15.1s on a
//!    4,385-tx mainnet block, the cause of the blackout) must be SKIPPED
//!    whenever the sheet is non-empty. Proven here by deleting the height's
//!    txid list out from under a balance-bearing outpoint: the view still
//!    succeeds. INTENTIONAL DIVERGENCE FROM THE OLD CODE, pinned on purpose:
//!    the old eager code ran the scan BEFORE the overwrite, so this exact
//!    state made the whole response fail with "txid not indexed in table".
//!
//! 3. THE EMPTY-SHEET PATH IS UNCHANGED: no balances → the scan still runs
//!    and returns the REAL (height, txindex); and an unindexed txid still
//!    errors exactly as before.

#[cfg(test)]
mod tests {
    use crate::message::{MessageContext, MessageContextParcel};
    use crate::test_helpers::{self as helpers};
    use crate::{tables, view, Protorune};
    use anyhow::Result;
    use bitcoin::hashes::Hash;
    use bitcoin::OutPoint;
    use metashrew_core::index_pointer::AtomicPointer;
    #[allow(unused_imports)]
    use metashrew_core::{
        println,
        stdio::{stdout, Write},
    };
    use metashrew_support::index_pointer::KeyValuePointer;
    use prost::Message;
    use protorune_support::balance_sheet::BalanceSheet;
    use protorune_support::proto::protorune::{
        Outpoint as OutpointProto, OutpointWithProtocol,
    };
    use protorune_support::rune_transfer::RuneTransfer;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    use helpers::clear;

    const PROTOCOL_ID: u128 = 122;
    const ETCH_HEIGHT: u64 = 840000;
    const TRANSFER_HEIGHT: u64 = 840002;

    struct NoopMessageContext;

    impl MessageContext for NoopMessageContext {
        fn protocol_tag() -> u128 {
            PROTOCOL_ID
        }
        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            let runes: Vec<RuneTransfer> = parcel.runes.clone();
            Ok((runes, BalanceSheet::default()))
        }
    }

    /// Index one block at ETCH_HEIGHT: [coinbase, etch+protoburn]. The
    /// protoburn tx sits at txindex 1, so the protorune id minted for its
    /// vout-0 balance is (ETCH_HEIGHT, 1) with amount 1000 (cf. the
    /// `protoburn_test` assertions in index_protoburns.rs).
    fn index_etch_block() -> bitcoin::Block {
        let mut block = helpers::create_block_with_coinbase_tx(ETCH_HEIGHT as u32);
        let previous_output = OutPoint {
            txid: bitcoin::Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            vout: 0,
        };
        let protoburn_tx =
            helpers::create_default_protoburn_transaction(previous_output, PROTOCOL_ID);
        block.txdata.push(protoburn_tx);
        assert!(
            Protorune::index_block::<NoopMessageContext>(block.clone(), ETCH_HEIGHT).is_ok()
        );
        block
    }

    fn protorunes_by_outpoint_req(outpoint: &OutPoint) -> Vec<u8> {
        (OutpointWithProtocol {
            txid: outpoint.txid.as_byte_array().to_vec(),
            vout: outpoint.vout,
            protocol: Some(PROTOCOL_ID.into()),
        })
        .encode_to_vec()
    }

    fn runes_by_outpoint_req(outpoint: &OutPoint) -> Vec<u8> {
        (OutpointProto {
            txid: outpoint.txid.as_byte_array().to_vec(),
            vout: outpoint.vout,
        })
        .encode_to_vec()
    }

    /// (a) RUNE-CONVENTION PIN — the killer property consumers depend on.
    ///
    /// Move the protorunes to a LATER block, at a tx position that differs
    /// from the rune id, so the convention is distinguishable from the real
    /// position on BOTH fields:
    ///   real position of the holding outpoint:  (840002, txindex 2)
    ///   first (only) protorune id on its sheet: (840000, tx 1)
    /// The response must carry the RUNE ID pair, exactly like live mainnet
    /// (height=2/32 — rune blocks, not chain heights).
    #[wasm_bindgen_test]
    fn protorunes_by_outpoint_returns_first_rune_id_not_real_position() {
        clear();
        let etch_block = index_etch_block();

        // Block at TRANSFER_HEIGHT: [coinbase, filler, transfer]. The filler
        // pushes the transfer tx to txindex 2 ≠ rune id tx 1, so the txindex
        // convention is observable even independently of the height.
        let mut block2 = helpers::create_block_with_coinbase_tx(TRANSFER_HEIGHT as u32);
        block2.txdata.push(helpers::create_test_transaction());
        let transfer_tx = helpers::create_protostone_transaction(
            OutPoint {
                txid: etch_block.txdata[1].compute_txid(),
                vout: 0,
            },
            None,  // no burn — plain transfer
            false, // no etch
            1,     // rune pointer → op_return (no runes in play post-burn)
            0,     // protostone pointer → vout 0 receives the protorunes
            PROTOCOL_ID,
            vec![],
        );
        block2.txdata.push(transfer_tx.clone());
        assert!(
            Protorune::index_block::<NoopMessageContext>(block2.clone(), TRANSFER_HEIGHT).is_ok()
        );

        let holding = OutPoint {
            txid: transfer_tx.compute_txid(),
            vout: 0,
        };
        let response =
            view::protorunes_by_outpoint(&protorunes_by_outpoint_req(&holding)).unwrap();

        // Sanity: the outpoint really bears the 1000 protorunes.
        let balances = response.balances.unwrap();
        assert_eq!(balances.entries.len(), 1);

        // THE PIN: first rune id (840000, 1), not the real (840002, 2).
        assert_eq!(response.height, ETCH_HEIGHT as u32);
        assert_eq!(response.txindex, 1u32);

        // Contrast on the SAME outpoint: its RUNES sheet is empty (the runes
        // were protoburned), so `runes_by_outpoint` takes the empty-sheet
        // scan path and reports the REAL position — proving the two paths
        // coexist and the convention is a non-empty-sheet property, not a
        // global rewrite.
        let rune_response = view::runes_by_outpoint(&runes_by_outpoint_req(&holding)).unwrap();
        assert_eq!(rune_response.height, TRANSFER_HEIGHT as u32);
        assert_eq!(rune_response.txindex, 2u32);
    }

    /// (b) LAZY-SCAN PROOF + INTENTIONAL-DIVERGENCE PIN — the killer test.
    ///
    /// Construct the state that separates the old code from the new: the
    /// outpoint BEARS balances, but its txid is deleted from
    /// RUNES.HEIGHT_TO_TRANSACTION_IDS for its height (we zero the list's
    /// /length key through the same table API the view reads).
    ///
    /// OLD (eager) code: ran the block walk FIRST, found no txid, and failed
    /// the WHOLE response with "txid not indexed in table" — even though the
    /// walk's result was about to be discarded for this outpoint.
    /// NEW (lazy) code: never runs the walk for a non-empty sheet, so the
    /// view SUCCEEDS with the rune-id convention. This is a deliberate,
    /// strictly-better behavior change shipped with the perf fix; if this
    /// test ever breaks, either the laziness regressed (timeout risk comes
    /// back) or the divergence was un-shipped without updating consumers.
    #[wasm_bindgen_test]
    fn balance_bearing_outpoint_survives_missing_height_txid_list() {
        clear();
        let etch_block = index_etch_block();
        let holding = OutPoint {
            txid: etch_block.txdata[1].compute_txid(),
            vout: 0,
        };

        // Sever the height→txids list: list reads are length-driven
        // (`get_list` iterates 0..length), so zeroing /length makes every
        // txid in the block — including ours — absent from the scan's view.
        let mut length_ptr = tables::RUNES
            .HEIGHT_TO_TRANSACTION_IDS
            .select_value::<u64>(ETCH_HEIGHT)
            .length_key();
        length_ptr.set_value::<u32>(0);
        assert_eq!(
            tables::RUNES
                .HEIGHT_TO_TRANSACTION_IDS
                .select_value::<u64>(ETCH_HEIGHT)
                .get_list()
                .len(),
            0
        );

        // Balance-bearing outpoint: SUCCEEDS (old code errored here).
        let response =
            view::protorunes_by_outpoint(&protorunes_by_outpoint_req(&holding)).unwrap();
        assert_eq!(response.balances.unwrap().entries.len(), 1);
        assert_eq!(response.height, ETCH_HEIGHT as u32);
        assert_eq!(response.txindex, 1u32);

        // Same severed state, EMPTY-sheet outpoint (the coinbase): the scan
        // still runs and still fails exactly like the old code — the error
        // path was not widened, only the pointless scan was removed.
        let coinbase = OutPoint {
            txid: etch_block.txdata[0].compute_txid(),
            vout: 0,
        };
        let err = view::runes_by_outpoint(&runes_by_outpoint_req(&coinbase)).unwrap_err();
        assert!(err.to_string().contains("txid not indexed in table"));
    }

    /// (c) EMPTY-SHEET PATH UNCHANGED — the scan still resolves the REAL
    /// (height, txindex) for outpoints with no balances.
    ///
    /// The coinbase outpoint of the etch block has an empty sheet on both
    /// the runes table and the protocol table, and sits at txindex 0 — a
    /// pair the rune-id convention could never produce here (the only rune
    /// id in state is (840000, 1)), so a pass proves the value came from the
    /// real block walk.
    #[wasm_bindgen_test]
    fn empty_sheet_outpoint_still_gets_real_height_and_txindex() {
        clear();
        let etch_block = index_etch_block();
        let coinbase = OutPoint {
            txid: etch_block.txdata[0].compute_txid(),
            vout: 0,
        };

        let rune_response = view::runes_by_outpoint(&runes_by_outpoint_req(&coinbase)).unwrap();
        assert_eq!(rune_response.balances.unwrap().entries.len(), 0);
        assert_eq!(rune_response.height, ETCH_HEIGHT as u32);
        assert_eq!(rune_response.txindex, 0u32);

        // Same through the protorunes view (empty sheet for protocol 122).
        let proto_response =
            view::protorunes_by_outpoint(&protorunes_by_outpoint_req(&coinbase)).unwrap();
        assert_eq!(proto_response.balances.unwrap().entries.len(), 0);
        assert_eq!(proto_response.height, ETCH_HEIGHT as u32);
        assert_eq!(proto_response.txindex, 0u32);
    }

    /// (c, error case) EMPTY SHEET + txid absent from the height list → the
    /// view still ERRORS, byte-for-byte the old message. A never-indexed
    /// outpoint resolves OUTPOINT_TO_HEIGHT=0, whose txid list is empty, so
    /// the scan finds nothing — old and new code agree on this path.
    #[wasm_bindgen_test]
    fn empty_sheet_unindexed_outpoint_still_errors() {
        clear();
        index_etch_block();
        let ghost = OutPoint {
            txid: bitcoin::Txid::from_str(
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            )
            .unwrap(),
            vout: 0,
        };

        let rune_err = view::runes_by_outpoint(&runes_by_outpoint_req(&ghost)).unwrap_err();
        assert!(rune_err.to_string().contains("txid not indexed in table"));

        let proto_err =
            view::protorunes_by_outpoint(&protorunes_by_outpoint_req(&ghost)).unwrap_err();
        assert!(proto_err.to_string().contains("txid not indexed in table"));
    }

    // (d) protorunes_by_address: NOT covered in the default build — the
    // by-address views are gated behind the `address-indexing` Cargo feature
    // (OFF by default; the stubs return a "requires --features
    // address-indexing" error, pinned by tests/address_indexing.rs). Both
    // gated implementations delegate per-outpoint to
    // `protorune_outpoint_to_outpoint_response`, which is exactly the helper
    // pinned above, so the convention holds through the by-address path by
    // construction.
}
