#[cfg(test)]
mod tests {
    use crate::balance_sheet::load_sheet;
    use crate::message::MessageContext;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
    use protorune_support::proto::protorune::{
        OutpointResponse, Rune as RuneProto, RunesByHeightRequest, WalletRequest,
    };

    use crate::test_helpers::{self as helpers, RunesTestingConfig, ADDRESS1, ADDRESS2};
    use crate::test_helpers::{display_list_as_hex, display_vec_as_hex};
    use crate::Protorune;
    use crate::{message::MessageContextParcel, tables, view};
    use anyhow::Result;
    use protorune_support::rune_transfer::RuneTransfer;
    use protorune_support::utils::consensus_encode;

    use bitcoin::consensus::serialize;
    use bitcoin::hashes::Hash;
    use bitcoin::{OutPoint, Txid};
    use hex;

    use helpers::clear;
    #[allow(unused_imports)]
    use metashrew::{
        println,
        stdio::{stdout, Write},
    };
    use metashrew_support::index_pointer::KeyValuePointer;
    use ordinals::{Edict, Etching, Rune, RuneId, Runestone, Terms};

    use protobuf::{Message, SpecialFields};

    use std::str::FromStr;
    use std::sync::Arc;
    use wasm_bindgen_test::*;

    struct MyMessageContext(());

    impl MessageContext for MyMessageContext {
        fn handle(_parcel: &MessageContextParcel) -> Result<(Vec<RuneTransfer>, BalanceSheet)> {
            let ar: Vec<RuneTransfer> = vec![];
            Ok((ar, BalanceSheet::default()))
        }
        fn protocol_tag() -> u128 {
            100
        }
    }

    #[wasm_bindgen_test]
    fn height_blockhash() {
        clear();
        let test_block = helpers::create_block_with_coinbase_tx(840000);
        let expected_block_hash =
            display_vec_as_hex(test_block.block_hash().as_byte_array().to_vec());
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), 840000);
        let test_height_to_blockhash = tables::RUNES
            .HEIGHT_TO_BLOCKHASH
            .select_value(840000 as u64)
            .get();
        let test_blockhash_to_height = tables::RUNES
            .BLOCKHASH_TO_HEIGHT
            .select(&test_block.block_hash().as_byte_array().to_vec())
            .get_value::<u64>();
        assert_eq!(
            hex::encode(test_height_to_blockhash.as_ref()),
            expected_block_hash
        );
        assert_eq!(test_blockhash_to_height, 840000);
    }

    #[wasm_bindgen_test]
    fn spendable_by_address() {
        clear();
        let test_block = helpers::create_block_with_sample_tx();
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), 840001);
        tables::OUTPOINTS_FOR_ADDRESS
            .keyword(&ADDRESS1())
            .set(Arc::new(Vec::new()));
        // let outpoint: OutPoint = OutPoint {
        //     txid: Txid::from_str(
        //         "a440cb400062f14cff5f76fbbd3881c426820171180c67c103a36d12c89fbd32",
        //     )
        //     .unwrap(),
        //     vout: 0,
        // };
        // let test_val = tables::OUTPOINT_SPENDABLE_BY
        //     .select(&serialize(&outpoint))
        //     .get();
        // let addr_str = display_vec_as_hex(test_val.to_vec());
        let _addr_str: String = display_vec_as_hex(ADDRESS1().into_bytes());

        let _view_test = view::runes_by_address(&ADDRESS1().into_bytes());

        //println!("{:?}", view_test);
        let mut outpoint_vec: Vec<String> = Vec::new();
        outpoint_vec
            .push("a440cb400062f14cff5f76fbbd3881c426820171180c67c103a36d12c89fbd32:0".to_string());
        // let matching_view_test = view::AddressOutpoints {
        //     outpoints: outpoint_vec,
        // };
        // assert_eq!(view_test, serde_json::to_string_pretty(&matching_view_test).unwrap());
        // assert_eq!(_addr_str, addr_str);
    }

    #[wasm_bindgen_test]
    fn outpoints_by_address() {
        clear();
        let test_block = helpers::create_block_with_sample_tx();
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), 840001);
        let outpoint: OutPoint = OutPoint {
            txid: Txid::from_str(
                "20e06c645f2dba1b9cf3ed1dbe46c59402fa2ac5c6b06a97e6697fe07d55f43e",
            )
            .unwrap(),
            vout: 0,
        };
        let test_val = tables::OUTPOINTS_FOR_ADDRESS
            .keyword(&helpers::ADDRESS1())
            .get_list();
        let list_str: String = display_list_as_hex(test_val);

        let test_outpoint: Vec<u8> = serialize(&outpoint);
        let outpoint_hex: String = display_vec_as_hex(test_outpoint);

        assert_eq!(list_str, outpoint_hex);
    }

    #[wasm_bindgen_test]
    fn runes_by_address_test() {
        clear();
        let (test_block, _) = helpers::create_block_with_rune_tx(None);
        let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), 840001);
        let req = (WalletRequest {
            wallet: helpers::ADDRESS1().as_bytes().to_vec(),
            special_fields: SpecialFields::new(),
        })
        .write_to_bytes()
        .unwrap();
        let test_val = view::runes_by_address(&req).unwrap();
        let runes: Vec<OutpointResponse> = test_val.clone().outpoints;
        println!("{:?}", runes);
        assert_eq!(runes[0].height, 840001);
        assert_eq!(runes[0].txindex, 0);
    }

    // #[wasm_bindgen_test]
    // fn protorunes_by_address_test() {
    //     clear();
    //     let (test_block, _) = helpers::create_block_with_rune_tx();
    //     let _ = Protorune::index_block::<MyMessageContext>(test_block.clone(), 840001);
    //     let address = "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu".as_bytes().to_vec();
    //     let test_val = view::runes_by_address(&address).unwrap();
    //     let runes: Vec<crate::proto::protorune::OutpointResponse> = test_val.clone().outpoints;
    //     // assert_eq!(runes[0].height, 840001);
    //     // assert_eq!(runes[0].txindex, 0);
    // }

    fn runes_by_height_test_template(config: Option<RunesTestingConfig>) -> Vec<RuneProto> {
        let (test_block, config) = helpers::create_block_with_rune_tx(config);
        let _ =
            Protorune::index_block::<MyMessageContext>(test_block.clone(), config.rune_etch_height);
        let req: Vec<u8> = (RunesByHeightRequest {
            height: config.rune_etch_height,
            special_fields: SpecialFields::new(),
        })
        .write_to_bytes()
        .unwrap();
        let test_val = view::runes_by_height(&req).unwrap();
        let runes: Vec<RuneProto> = test_val.clone().runes;
        return runes;
    }

    #[wasm_bindgen_test]
    fn runes_by_height_test() {
        clear();
        let runes: Vec<RuneProto> = runes_by_height_test_template(None);
        let symbol = runes[0].symbol.clone();
        let name = runes[0].name.clone();
        assert_eq!(runes[0].divisibility, 2 as u32);
        assert_eq!(symbol, "Z");
        assert_eq!(name, "AAAAAAAAAAAAATESTER");
    }

    #[wasm_bindgen_test]
    fn rune_name_test_minimum_name_valid() {
        clear();

        let runes: Vec<RuneProto> = runes_by_height_test_template(Some(RunesTestingConfig::new(
            ADDRESS1().as_str(),
            ADDRESS2().as_str(),
            Some("AAAAAAAAAAAAA"),
            Some("Z"),
            840000,
            0,
        )));
        assert_eq!(runes.len(), 1);
        assert_eq!(runes[0].name, "AAAAAAAAAAAAA");
    }

    #[wasm_bindgen_test]
    fn rune_name_test_minimum_name_invalid() {
        clear();
        let runes: Vec<RuneProto> = runes_by_height_test_template(Some(RunesTestingConfig::new(
            ADDRESS1().as_str(),
            ADDRESS2().as_str(),
            // most 12 character runes are not unlocked yet at 840000
            Some("AAZZZZZZZZZZ"),
            Some("Z"),
            840000,
            0,
        )));
        assert_eq!(runes.len(), 0);
    }

    #[wasm_bindgen_test]
    fn rune_name_test_minimum_name_unlocks() {
        clear();

        let runes: Vec<RuneProto> = runes_by_height_test_template(Some(RunesTestingConfig::new(
            ADDRESS1().as_str(),
            ADDRESS2().as_str(),
            Some("AAAAAAAAAAAA"),
            Some("Z"),
            857500,
            0,
        )));
        assert_eq!(runes.len(), 1);
        assert_eq!(runes[0].name, "AAAAAAAAAAAA");
    }

    #[wasm_bindgen_test]
    fn rune_name_test_trying_to_use_reserved_name() {
        clear();
        let runes: Vec<RuneProto> = runes_by_height_test_template(Some(RunesTestingConfig::new(
            ADDRESS1().as_str(),
            ADDRESS2().as_str(),
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            Some("Z"),
            840001,
            0,
        )));
        assert_eq!(runes.len(), 0);
    }

    #[wasm_bindgen_test]
    fn rune_name_test_reserved_name() {
        clear();
        let runes: Vec<RuneProto> = runes_by_height_test_template(Some(RunesTestingConfig::new(
            ADDRESS1().as_str(),
            ADDRESS2().as_str(),
            None,
            None,
            840001,
            0,
        )));
        assert_eq!(runes.len(), 1);
        let symbol = runes[0].symbol.clone();
        let name = runes[0].name.clone();
        // default symbol as described in spec
        assert_eq!(symbol, "¤");
        // default allocated name
        assert_eq!(name, "AAAAAAAAAAAAAAAAZOMKALPTKDC");
    }

    #[wasm_bindgen_test]
    fn index_runestone() {
        clear();
        let (test_block, config) = helpers::create_block_with_rune_tx(None);
        tables::OUTPOINTS_FOR_ADDRESS
            .keyword(&config.address1)
            .set(Arc::new(Vec::new()));
        assert!(Protorune::index_block::<MyMessageContext>(
            test_block.clone(),
            config.rune_etch_height
        )
        .is_ok());
        let rune_id = ProtoruneRuneId::new(
            config.rune_etch_height as u128,
            config.rune_etch_vout as u128,
        );
        let test_val = tables::RUNES
            .RUNE_ID_TO_ETCHING
            .select(&rune_id.into())
            .get();
        let cached_name: String = String::from_utf8(test_val.to_vec()).unwrap();
        assert_eq!(cached_name, config.rune_name.unwrap());
    }

    #[wasm_bindgen_test]
    fn correct_balance_sheet() {
        clear();
        let (test_block, config) = helpers::create_block_with_rune_tx(None);
        let _ =
            Protorune::index_block::<MyMessageContext>(test_block.clone(), config.rune_etch_height);
        let outpoint: OutPoint = OutPoint {
            txid: test_block.txdata[0].compute_txid(),
            vout: 0,
        };
        let protorune_id = ProtoruneRuneId {
            block: config.rune_etch_height as u128,
            tx: config.rune_etch_vout as u128,
        };
        let sheet = load_sheet(
            &tables::RUNES
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint).unwrap()),
        );
        let stored_balance = sheet.get(&protorune_id);
        assert_eq!(1000 as u128, stored_balance);
    }

    ///
    /// EDICT TRANSFER TESTS
    /// refer to https://docs.ordinals.com/runes/specification.html#transferring
    /// for the proper spec that I am testing
    ///

    fn edict_test(
        edict_amount: Option<u128>,
        edict_output: Option<u32>,
        expected_address1_amount: u128,
        expected_address2_amount: u128,
    ) {
        clear();
        let config = RunesTestingConfig::new(
            ADDRESS1().as_str(),
            ADDRESS2().as_str(),
            Some("TESTTESTTESTTEST"),
            Some("Z"),
            840001,
            0,
        );

        let rune_id = RuneId::new(config.rune_etch_height, config.rune_etch_vout).unwrap();
        let edicts = match (edict_amount, edict_output) {
            (Some(amount), Some(output)) => vec![Edict {
                id: rune_id,
                amount,
                output,
            }],
            _ => Vec::new(),
        };

        let test_block = helpers::create_block_with_rune_transfer(&config, edicts);
        let _ =
            Protorune::index_block::<MyMessageContext>(test_block.clone(), config.rune_etch_height);
        let outpoint_address2: OutPoint = OutPoint {
            txid: test_block.txdata[1].compute_txid(),
            vout: 0,
        };
        let outpoint_address1: OutPoint = OutPoint {
            txid: test_block.txdata[1].compute_txid(),
            vout: 1,
        };
        let protorune_id = ProtoruneRuneId {
            block: config.rune_etch_height as u128,
            tx: config.rune_etch_vout as u128,
        };
        let sheet1 = load_sheet(
            &tables::RUNES
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_address1).unwrap()),
        );
        let stored_balance_address1 = sheet1.get(&protorune_id);
        assert_eq!(expected_address1_amount, stored_balance_address1);

        let sheet2 = load_sheet(
            &tables::RUNES
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_address2).unwrap()),
        );
        let stored_balance_address2 = sheet2.get(&protorune_id);
        assert_eq!(expected_address2_amount, stored_balance_address2);

        let outpoint_original_transaction: OutPoint = OutPoint {
            txid: test_block.txdata[0].compute_txid(),
            vout: 0,
        };
        let sheet_original_transaction = load_sheet(
            &tables::RUNES
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_original_transaction).unwrap()),
        );
        let stored_balance_address1 = sheet_original_transaction.get(&protorune_id);
        assert_eq!(0, stored_balance_address1); //assert that original outpoint runes are all spent
    }

    /// normal transfer works
    #[wasm_bindgen_test]
    fn correct_balance_sheet_with_transfers() {
        edict_test(Some(200), Some(0), 800 as u128, 200 as u128);
    }

    /// transferring more runes only transfers the amount remaining
    #[wasm_bindgen_test]
    fn correct_balance_sheet_transfer_too_much() {
        edict_test(Some(1200), Some(0), 0 as u128, 1000 as u128);
    }

    /// Tests that transferring runes to an outpoint > num outpoints is a cenotaph.
    /// All runes input to a tx containing a cenotaph is burned
    #[wasm_bindgen_test]
    fn cenotaph_balance_sheet_transfer_bad_target() {
        edict_test(Some(200), Some(4), 0, 0);
    }

    /// Tests that transferring runes to an outpoint == OP_RETURN burns the runes.
    #[wasm_bindgen_test]
    fn correct_balance_sheet_transfer_target_op_return() {
        edict_test(Some(200), Some(2), 800, 0);
    }

    /// An edict with amount zero allocates all remaining units of rune id.
    #[wasm_bindgen_test]
    fn correct_balance_sheet_transfer_0() {
        edict_test(Some(0), Some(0), 0, 1000);
    }

    /// An edict with output == number of transaction outputs will
    /// allocates amount runes to each non-OP_RETURN output in order
    #[wasm_bindgen_test]
    fn correct_balance_sheet_equal_distribute_300() {
        edict_test(Some(300), Some(3), 700, 300);
    }

    /// An edict with output == number of transaction outputs
    /// and amount = 0 will equally distribute all remaining runes
    /// to each non-OP_RETURN output in order
    #[wasm_bindgen_test]
    fn correct_balance_sheet_equal_distribute_0() {
        edict_test(Some(0), Some(3), 500, 500);
    }

    /// No edict, all amount should go to pointer
    #[wasm_bindgen_test]
    fn no_edict_pointer() {
        edict_test(None, None, 1000, 0);
    }
}
