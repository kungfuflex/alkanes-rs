use crate::{message::AlkaneMessageContext, tests::std::alkanes_std_auth_token_build};
use alkanes_support::id::AlkaneId;
use alkanes_support::{cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID};
use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use protorune::{balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, assert_binary_deployed_to_id};
use crate::tests::std::alkanes_std_owned_token_build;
use alkane_helpers::clear;
use alkanes::view;
use bitcoin::Witness;
#[allow(unused_imports)]
use metashrew::{
    println,
    stdio::{stdout, Write},
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_owned_token() -> Result<()> {
    clear();
    let block_height = 840_000;

    let test_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![0, 1, 1000],
    };
    let mint_test_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![1, 1000],
    };
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
            [].into(),
        ]
        .into(),
        [auth_cellpack, test_cellpack, mint_test_cellpack].into(),
    );

    index_block(&test_block, block_height)?;
    let owned_token_id = AlkaneId { block: 2, tx: 1 };
    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 1,
    };
    let _sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    /*
        let _ = assert_binary_deployed_to_id(
            owned_token_id.clone(),
            alkanes_std_owned_token_build::get_bytes(),
        );
    */
    Ok(())
}
#[wasm_bindgen_test]
fn test_auth_and_owned_token_noop() -> Result<()> {
    clear();
    let block_height = 840_000;

    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    let test_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![100],
    };
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
        ]
        .into(),
        [auth_cellpack, test_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let _auth_token_id_factory = AlkaneId {
        block: 4,
        tx: AUTH_TOKEN_FACTORY_ID,
    };

    let owned_token_id = AlkaneId { block: 2, tx: 1 };

    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    };
    let _sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    // assert_eq!(sheet.get(&original_rune_id.into()), 1000);

    let tx_first = test_block.txdata.first().ok_or(anyhow!("no first el"))?;
    let outpoint_first = OutPoint {
        txid: tx_first.compute_txid(),
        vout: 0,
    };
    let sheet_first = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_first)?),
    );
    assert_eq!(sheet_first.balances.len(), 0);
    let _ = assert_binary_deployed_to_id(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        _auth_token_id_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );

    Ok(())
}

#[wasm_bindgen_test]
fn test_auth_and_owned_token() -> Result<()> {
    clear();
    let block_height = 840_000;

    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    let test_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            0,    /* opcode (to init new auth token) */
            1,    /* auth_token units */
            1000, /* owned_token token_units */
        ],
    };
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
        ]
        .into(),
        [auth_cellpack, test_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let _auth_token_id_factory = AlkaneId {
        block: 4,
        tx: AUTH_TOKEN_FACTORY_ID,
    };

    let auth_token_id_deployment = AlkaneId { block: 2, tx: 2 };
    let owned_token_id = AlkaneId { block: 2, tx: 1 };

    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    };
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    assert_eq!(sheet.get(&owned_token_id.into()), 1000);
    assert_eq!(sheet.get(&auth_token_id_deployment.into()), 1);

    let tx_first = test_block.txdata.first().ok_or(anyhow!("no first el"))?;
    let outpoint_first = OutPoint {
        txid: tx_first.compute_txid(),
        vout: 0,
    };
    let sheet_first = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_first)?),
    );
    assert_eq!(sheet_first.balances.len(), 0);
    let _ = assert_binary_deployed_to_id(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        _auth_token_id_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        auth_token_id_deployment.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );

    Ok(())
}

#[wasm_bindgen_test]
fn test_owned_token_set_name_and_symbol() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Initialize the OwnedToken contract
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    // Initialize the OwnedToken with auth token and token units
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            0,    /* opcode (to init new auth token) */
            1,    /* auth_token units */
            1000, /* owned_token token_units */
        ],
    };

    // Create a cellpack to set the name and symbol
    // For the set_name_and_symbol method (opcode 88)
    // The format for string parameters is now null-terminated strings

    // For a long name that spans multiple u128s
    // "SuperLongCustomTokenNameThatSpansMultipleU128Values" (49 characters)
    let name_data1 = u128::from_le_bytes(*b"SuperLongCustomT");
    let name_data2 = u128::from_le_bytes(*b"okenNameThatSpan");
    let name_data3 = u128::from_le_bytes(*b"nsMultipleU128Va");
    let name_data4 = u128::from_le_bytes(*b"alues\0\0\0\0\0\0\0\0\0\0\0");

    // For "SLCT" symbol (4 characters)
    let symbol_data = u128::from_le_bytes(*b"SLCT\0\0\0\0\0\0\0\0\0\0\0\0");

    let set_name_symbol_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 }, // This will be the OwnedToken ID
        inputs: vec![
            88,          // opcode for set_name_and_symbol
            name_data1,  // first part of the name
            name_data2,  // second part of the name
            name_data3,  // third part of the name
            name_data4,  // fourth part of the name with null terminator
            symbol_data, // null-terminated symbol data
        ],
    };

    // Create a cellpack to get the name
    let get_name_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 }, // OwnedToken ID
        inputs: vec![99],                     // opcode for get_name
    };

    // Create a cellpack to get the symbol
    let get_symbol_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 }, // OwnedToken ID
        inputs: vec![100],                    // opcode for get_symbol
    };

    // Initialize the contracts and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
        ]
        .into(),
        [auth_cellpack, init_cellpack].into(),
    );

    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![
                set_name_symbol_cellpack,
                get_name_cellpack,
                get_symbol_cellpack,
            ],
            OutPoint {
                txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
                vout: 0,
            },
            false,
        ),
    );

    index_block(&test_block, block_height)?;

    // Get the OwnedToken ID
    let owned_token_id = AlkaneId { block: 2, tx: 1 };

    // Verify the binary was deployed correctly
    let _ = assert_binary_deployed_to_id(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );

    // Get the trace data from the transaction
    let outpoint = OutPoint {
        txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
        vout: 3,
    };

    let trace_data = view::trace(&outpoint)?;

    // Convert trace data to string for easier searching
    let trace_str = String::from_utf8_lossy(&trace_data);

    println!("trace {:?}", trace_str);

    let expected_name = "SuperLongCustomTokenNameThatSpannsMultipleU128Vaalues";
    let expected_symbol = "SLCT";

    // Check if the trace data contains the expected name
    assert!(
        trace_str.contains(expected_name),
        "Trace data should contain the name '{}', but it doesn't",
        expected_name
    );
    assert!(
        trace_str.contains(expected_symbol),
        "Trace data should contain the symbol '{}', but it doesn't",
        expected_symbol
    );

    Ok(())
}

#[wasm_bindgen_test]
fn test_auth_and_owned_token_multiple() -> Result<()> {
    clear();
    let block_height = 840_000;

    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    let test_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            0,    /* opcode (to init new auth token) */
            1,    /* auth_token units */
            1000, /* owned_token token_units */
        ],
    };
    let owned_copy_cellpack = Cellpack {
        target: AlkaneId { block: 5, tx: 1 },
        inputs: vec![
            0,    /* opcode (to init new auth token) */
            1,    /* auth_token units */
            1000, /* owned_token token_units */
        ],
    };
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
            [].into(),
            [].into(),
            [].into(),
            [].into(),
            [].into(),
            [].into(),
            [].into(),
            [].into(),
            [].into(),
        ]
        .into(),
        [
            auth_cellpack,
            test_cellpack,
            owned_copy_cellpack.clone(),
            owned_copy_cellpack.clone(),
            owned_copy_cellpack.clone(),
            owned_copy_cellpack.clone(),
            owned_copy_cellpack.clone(),
            owned_copy_cellpack.clone(),
            owned_copy_cellpack.clone(),
            owned_copy_cellpack.clone(),
            owned_copy_cellpack.clone(),
        ]
        .into(),
    );

    index_block(&test_block, block_height)?;

    let _auth_token_id_factory = AlkaneId {
        block: 4,
        tx: AUTH_TOKEN_FACTORY_ID,
    };

    let auth_token_id_deployment = AlkaneId { block: 2, tx: 2 };
    let owned_token_id = AlkaneId { block: 2, tx: 1 };

    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    };
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    assert_eq!(sheet.get(&owned_token_id.into()), 1000);
    assert_eq!(sheet.get(&auth_token_id_deployment.into()), 1);

    let tx_first = test_block.txdata.first().ok_or(anyhow!("no first el"))?;
    let outpoint_first = OutPoint {
        txid: tx_first.compute_txid(),
        vout: 0,
    };
    let sheet_first = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_first)?),
    );
    assert_eq!(sheet_first.balances.len(), 0);
    let _ = assert_binary_deployed_to_id(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        _auth_token_id_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        auth_token_id_deployment.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );

    let _ = assert_binary_deployed_to_id(
        AlkaneId { block: 2, tx: 3 },
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        AlkaneId { block: 2, tx: 4 },
        alkanes_std_auth_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        AlkaneId { block: 2, tx: 5 },
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        AlkaneId { block: 2, tx: 6 },
        alkanes_std_auth_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        AlkaneId { block: 2, tx: 7 },
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        AlkaneId { block: 2, tx: 8 },
        alkanes_std_auth_token_build::get_bytes(),
    );

    Ok(())
}
