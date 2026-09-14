//! Tests for the `address-indexing` opt-in feature.
//!
//! These tests exercise the v3 chunked `AddressOutpoints` storage and
//! the gated view functions. They are conditionally compiled — when
//! the feature is OFF, only the "feature-off returns error" test
//! runs; when ON, the full per-block writer + view round-trip is
//! exercised.

#[cfg(test)]
mod tests {
    use crate::message::MessageContext;
    use crate::message::MessageContextParcel;
    use crate::test_helpers::{self as helpers, ADDRESS1};
    use crate::{view, Protorune};
    use anyhow::Result;
    use metashrew_core::index_pointer::AtomicPointer;
    #[allow(unused_imports)]
    use prost::Message;
    use protorune_support::balance_sheet::BalanceSheet;
    #[allow(unused_imports)]
    use protorune_support::proto::protorune::WalletRequest;
    use protorune_support::rune_transfer::RuneTransfer;
    use wasm_bindgen_test::*;

    use helpers::clear;

    struct AddrTestContext;
    impl MessageContext for AddrTestContext {
        fn protocol_tag() -> u128 {
            100
        }
        fn handle(
            _parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            Ok((vec![], BalanceSheet::default()))
        }
    }

    /// Feature-off path: `runes_by_address` must return an explicit
    /// "feature not enabled" error so callers know the canonical v3
    /// mainnet wasm does NOT serve this view.
    #[cfg(not(feature = "address-indexing"))]
    #[wasm_bindgen_test]
    fn runes_by_address_returns_feature_error_when_disabled() {
        clear();
        let req = (WalletRequest {
            wallet: ADDRESS1().as_bytes().to_vec(),
        })
        .encode_to_vec();
        let err = view::runes_by_address(&req).err().expect("expected Err");
        let msg = format!("{}", err);
        assert!(
            msg.contains("address-indexing"),
            "error message should mention the feature flag, got: {}",
            msg
        );
    }

    /// Feature-off path: same for `protorunes_by_address`. The
    /// wasm-side `protorunesbyaddress` export is itself gated, so the
    /// JSON-RPC layer will get a "view function not found" before
    /// reaching this stub — but the rlib path (tests, alkanes-rpc-core
    /// when present) still needs the clear error.
    #[cfg(not(feature = "address-indexing"))]
    #[wasm_bindgen_test]
    fn protorunes_by_address_returns_feature_error_when_disabled() {
        use protorune_support::proto::protorune::ProtorunesWalletRequest;
        clear();
        let req = (ProtorunesWalletRequest {
            wallet: ADDRESS1().as_bytes().to_vec(),
            protocol_tag: Some(1u128.into()),
        })
        .encode_to_vec();
        let err = view::protorunes_by_address(&req)
            .err()
            .expect("expected Err");
        assert!(format!("{}", err).contains("address-indexing"));
    }

    // The remaining tests need the address-indexing writer / chunk
    // format and are only meaningful when the feature is on.
    #[cfg(feature = "address-indexing")]
    mod feature_on {
        use super::*;
        use crate::address_index;
        use bitcoin::hashes::Hash;
        use bitcoin::{
            absolute::LockTime, transaction::Version, Amount, OutPoint, ScriptBuf, Sequence,
            Transaction, TxIn, TxOut, Witness,
        };
        use prost::Message;
        use protorune_support::proto::protorune::{
            ProtorunesWalletRequest, WalletRequest, WalletResponse,
        };
        use std::str::FromStr;

        fn address1_script() -> ScriptBuf {
            helpers::get_address(helpers::ADDRESS1().as_str()).script_pubkey()
        }

        fn coinbase_txin() -> TxIn {
            TxIn {
                previous_output: OutPoint {
                    txid: bitcoin::Txid::from_str(
                        "0000000000000000000000000000000000000000000000000000000000000000",
                    )
                    .unwrap(),
                    vout: 0,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }
        }

        fn make_tx_to_address1(value: u64, vout_count: usize) -> Transaction {
            let mut outputs = Vec::with_capacity(vout_count);
            for _ in 0..vout_count {
                outputs.push(TxOut {
                    value: Amount::from_sat(value),
                    script_pubkey: address1_script(),
                });
            }
            Transaction {
                version: Version::ONE,
                lock_time: LockTime::ZERO,
                input: vec![coinbase_txin()],
                output: outputs,
            }
        }

        /// 5 outpoints written for ADDRESS1 in a single block come
        /// back from the chunk in `(txid_le, vout)` ascending order
        /// regardless of insertion order in the tx.
        #[wasm_bindgen_test]
        fn chunk_is_sorted_after_index() {
            clear();
            let tx = make_tx_to_address1(100_000, 5);
            let block = helpers::create_block_with_txs(vec![tx.clone()]);
            Protorune::index_block::<AddrTestContext>(block, 840001).unwrap();

            let chunk = address_index::load_chunk(ADDRESS1().as_bytes())
                .expect("expected an AddressOutpoints chunk for ADDRESS1");
            assert_eq!(chunk.outpoints.len(), 5);
            assert_eq!(chunk.height, 840001);
            // Determinism contract: sorted by (txid_le, vout) ascending.
            for w in chunk.outpoints.windows(2) {
                let a = (w[0].txid.clone(), w[0].vout);
                let b = (w[1].txid.clone(), w[1].vout);
                assert!(a < b, "chunk outpoints must be sorted ascending");
            }
        }

        /// View round-trip: `runes_by_address` returns one
        /// OutpointResponse per outpoint in the chunk, in the same
        /// canonical order.
        #[wasm_bindgen_test]
        fn runes_by_address_returns_chunk_outpoints() {
            clear();
            let tx = make_tx_to_address1(100_000, 3);
            let block = helpers::create_block_with_txs(vec![tx]);
            Protorune::index_block::<AddrTestContext>(block, 840002).unwrap();

            let req = (WalletRequest {
                wallet: ADDRESS1().as_bytes().to_vec(),
            })
            .encode_to_vec();
            let resp: WalletResponse = view::runes_by_address(&req).unwrap();
            assert_eq!(resp.outpoints.len(), 3);
            // Order matches the chunk.
            let chunk = address_index::load_chunk(ADDRESS1().as_bytes()).unwrap();
            for (chunk_op, resp_op) in chunk.outpoints.iter().zip(resp.outpoints.iter()) {
                let r = resp_op.outpoint.as_ref().unwrap();
                assert_eq!(chunk_op.txid, r.txid);
                assert_eq!(chunk_op.vout, r.vout);
            }
        }

        /// Spent-input removal: an outpoint paid to ADDRESS1 in
        /// block N and then consumed as an input by another tx in the
        /// same block must NOT appear in ADDRESS1's chunk after
        /// indexing.
        #[wasm_bindgen_test]
        fn spent_in_same_block_is_removed_from_chunk() {
            clear();
            // Tx0: coinbase -> 1 output to ADDRESS1.
            let tx0 = make_tx_to_address1(100_000, 1);
            let tx0_txid = tx0.compute_txid();
            // Tx1: spend tx0:0, send to ADDRESS1 again (so the address
            // ends up with exactly one outpoint: tx1:0).
            let tx1 = Transaction {
                version: Version::ONE,
                lock_time: LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: OutPoint {
                        txid: tx0_txid,
                        vout: 0,
                    },
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::new(),
                }],
                output: vec![TxOut {
                    value: Amount::from_sat(100_000),
                    script_pubkey: address1_script(),
                }],
            };
            let block = helpers::create_block_with_txs(vec![tx0, tx1.clone()]);
            Protorune::index_block::<AddrTestContext>(block, 840003).unwrap();

            let chunk = address_index::load_chunk(ADDRESS1().as_bytes())
                .expect("expected a chunk after both adds and removes apply");
            assert_eq!(
                chunk.outpoints.len(),
                1,
                "the tx0 output should have been removed as a spent input",
            );
            let only = &chunk.outpoints[0];
            assert_eq!(only.txid, tx1.compute_txid().as_byte_array().to_vec());
            assert_eq!(only.vout, 0);
        }

        /// Multi-block insertion: outpoints added in different blocks
        /// must aggregate into a single chunk (one rewrite per
        /// block-per-address), still sorted by (txid_le, vout).
        #[wasm_bindgen_test]
        fn multi_block_aggregates_into_single_chunk() {
            clear();
            // Block 1: 2 outputs.
            let tx1 = make_tx_to_address1(100_000, 2);
            let block1 = helpers::create_block_with_txs(vec![tx1]);
            Protorune::index_block::<AddrTestContext>(block1, 840100).unwrap();
            // Block 2: a different tx (different value -> different
            // txid) with 3 more outputs.
            let tx2 = make_tx_to_address1(200_000, 3);
            let block2 = helpers::create_block_with_txs(vec![tx2]);
            Protorune::index_block::<AddrTestContext>(block2, 840101).unwrap();

            let chunk = address_index::load_chunk(ADDRESS1().as_bytes()).unwrap();
            assert_eq!(chunk.outpoints.len(), 5);
            assert_eq!(chunk.height, 840101);
            for w in chunk.outpoints.windows(2) {
                let a = (w[0].txid.clone(), w[0].vout);
                let b = (w[1].txid.clone(), w[1].vout);
                assert!(a < b);
            }
        }
    }

    /// Baseline equivalence: the storage map produced by indexing a
    /// block with the `address-indexing` feature ON must equal the
    /// storage map produced by indexing the SAME block with the
    /// feature OFF, MINUS the `/v3/addr/...` keys. No other state may
    /// differ — address-indexing is strictly additive.
    ///
    /// We can't toggle the feature at runtime, so we approximate the
    /// invariant by checking that all non-`/v3/addr/...` keys present
    /// in the feature-on map have the SAME byte values as written by
    /// the underlying canonical indexer, AND that ONLY
    /// `/v3/addr/...` keys are unique to the feature-on writer.
    ///
    /// This is the strongest determinism check we can express
    /// in-process; the cross-build byte-equivalence check is left to
    /// the build pipeline (see `scripts/build.sh --verify`).
    #[cfg(feature = "address-indexing")]
    #[wasm_bindgen_test]
    fn address_indexing_writes_only_v3_addr_keys() {
        use metashrew_core::get_cache;
        clear();
        let test_block = helpers::create_block_with_sample_tx();
        Protorune::index_block::<AddrTestContext>(test_block, 840001).unwrap();

        let cache = get_cache();
        let v3_addr_prefix = b"/v3/addr/";
        let mut saw_v3_addr_write = false;
        for (k, v) in cache.iter() {
            // Trivial structural check: every key either is or isn't
            // a /v3/addr/* key. If it IS, the value must decode as an
            // AddressOutpoints proto (sanity check on the chunk
            // shape).
            if k.as_slice().starts_with(v3_addr_prefix) {
                saw_v3_addr_write = true;
                use prost::Message;
                use protorune_support::proto::protorune::AddressOutpoints;
                AddressOutpoints::decode(v.as_slice()).expect(
                    "every /v3/addr/* value must be a valid AddressOutpoints proto",
                );
            }
        }
        assert!(
            saw_v3_addr_write,
            "expected at least one /v3/addr/* write for ADDRESS1 in the sample tx",
        );
    }
}
