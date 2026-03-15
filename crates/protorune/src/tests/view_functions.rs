#[cfg(test)]
mod tests {
    use crate::balance_sheet::load_sheet;
    use crate::message::MessageContext;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
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
    use metashrew_core::index_pointer::AtomicPointer;
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
}
