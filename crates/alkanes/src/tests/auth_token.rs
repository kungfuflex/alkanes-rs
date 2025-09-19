use crate::index_block;
use crate::message::AlkaneMessageContext;
use crate::tests::helpers::{
    self as alkane_helpers, assert_binary_deployed_to_id, assert_id_points_to_alkane_id,
};
use crate::tests::std::{
    alkanes_std_auth_token_build, alkanes_std_owned_token_build,
};
use crate::tests::test_runtime::TestRuntime;
use crate::view;
use alkanes_support::{
    cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID, id::AlkaneId, utils::string_to_u128_list,
};
use anyhow::{anyhow, Result};
use bitcoin::{OutPoint, Witness};
use metashrew_support::environment::RuntimeEnvironment;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use protorune::message::MessageContext;
use protorune::{balance_sheet::load_sheet, tables::RuneTable};
use protorune_support::balance_sheet::BalanceSheetOperations;

#[test]
fn test_owned_token() -> Result<()> {
    alkane_helpers::clear::<TestRuntime>();
    let block_height = 0;

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

    index_block::<TestRuntime>(&test_block, block_height)?;
    let _owned_token_id = AlkaneId { block: 2, tx: 1 };
    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 1,
    };
    let _sheet = load_sheet(
        &RuneTable::<TestRuntime>::for_protocol(AlkaneMessageContext::<TestRuntime>::protocol_tag())
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
#[test]
fn test_auth_and_owned_token_noop() -> Result<()> {
    alkane_helpers::clear::<TestRuntime>();
    let block_height = 0;

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

    index_block::<TestRuntime>(&test_block, block_height)?;

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
        &RuneTable::<TestRuntime>::for_protocol(AlkaneMessageContext::<TestRuntime>::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    // assert_eq!(sheet.get_cached(&original_rune_id.into()), 1000);

    let tx_first = test_block.txdata.first().ok_or(anyhow!("no first el"))?;
    let outpoint_first = OutPoint {
        txid: tx_first.compute_txid(),
        vout: 0,
    };
    let sheet_first = load_sheet(
        &RuneTable::<TestRuntime>::for_protocol(AlkaneMessageContext::<TestRuntime>::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_first)?),
    );
    assert_eq!(sheet_first.balances().len(), 0);
    let _ = assert_binary_deployed_to_id::<TestRuntime>(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id::<TestRuntime>(
        _auth_token_id_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );

    Ok(())
}

#[test]
fn test_auth_and_owned_token() -> Result<()> {
    alkane_helpers::clear::<TestRuntime>();
    let block_height = 0;

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

    index_block::<TestRuntime>(&test_block, block_height)?;

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
        &RuneTable::<TestRuntime>::for_protocol(AlkaneMessageContext::<TestRuntime>::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    assert_eq!(sheet.get_cached(&owned_token_id.into()), 1000);
    assert_eq!(sheet.get_cached(&auth_token_id_deployment.into()), 1);

    let tx_first = test_block.txdata.first().ok_or(anyhow!("no first el"))?;
    let outpoint_first = OutPoint {
        txid: tx_first.compute_txid(),
        vout: 0,
    };
    let sheet_first = load_sheet(
        &RuneTable::<TestRuntime>::for_protocol(AlkaneMessageContext::<TestRuntime>::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_first)?),
    );
    assert_eq!(sheet_first.balances().len(), 0);
    let _ = assert_binary_deployed_to_id::<TestRuntime>(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id::<TestRuntime>(
        _auth_token_id_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );
    assert_id_points_to_alkane_id::<TestRuntime>(
        auth_token_id_deployment.clone(),
        AlkaneId {
            block: 4,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
    )?;

    Ok(())
}

#[test]
fn test_owned_token_set_name_and_symbol() -> Result<()> {
    alkane_helpers::clear::<TestRuntime>();
    let block_height = 0;

    // Initialize the OwnedToken contract
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    let mut inputs = vec![
        1,    /* opcode (to init new token) */
        1,    /* auth_token units */
        1000, /* owned_token token_units */
    ];
    inputs.extend(string_to_u128_list("SuperLongCustomToken".to_string()));
    inputs.extend(string_to_u128_list("SLCT".to_string()));

    // Initialize the OwnedToken with auth token and token units
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: inputs,
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
            vec![get_name_cellpack, get_symbol_cellpack],
            OutPoint {
                txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
                vout: 0,
            },
            false,
        ),
    );

    index_block::<TestRuntime>(&test_block, block_height)?;

    // Get the OwnedToken ID
    let owned_token_id = AlkaneId { block: 2, tx: 1 };

    // Verify the binary was deployed correctly
    let _ = assert_binary_deployed_to_id::<TestRuntime>(
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

    TestRuntime::log(format!("trace {:?}", trace_str));

    let expected_name = "SuperLongCustomToken";
    let expected_symbol = "SLCT";

    // Check if the trace data contains the expected name
    assert!(
        trace_str.contains(expected_name),
        "Trace data should contain the name '{}', but it doesn't",
        expected_name
    );

    // Get the trace data from the transaction
    let outpoint_symbol = OutPoint {
        txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
        vout: 4,
    };

    let trace_data_symbol = view::trace(&outpoint_symbol)?;

    // Convert trace data to string for easier searching
    let trace_str_symbol = String::from_utf8_lossy(&trace_data_symbol);

    TestRuntime::log(format!("trace_str_symbol {:?}", trace_str_symbol));
    assert!(
        trace_str_symbol.contains(expected_symbol),
        "Trace data should contain the symbol '{}', but it doesn't",
        expected_symbol
    );

    Ok(())
}

#[test]
fn test_auth_and_owned_token_multiple() -> Result<()> {
    alkane_helpers::clear::<TestRuntime>();
    let block_height = 0;

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

    index_block::<TestRuntime>(&test_block, block_height)?;

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
        &RuneTable::<TestRuntime>::for_protocol(AlkaneMessageContext::<TestRuntime>::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    assert_eq!(sheet.get_cached(&owned_token_id.into()), 1000);
    assert_eq!(sheet.get_cached(&auth_token_id_deployment.into()), 1);

    let tx_first = test_block.txdata.first().ok_or(anyhow!("no first el"))?;
    let outpoint_first = OutPoint {
        txid: tx_first.compute_txid(),
        vout: 0,
    };
    let sheet_first = load_sheet(
        &RuneTable::<TestRuntime>::for_protocol(AlkaneMessageContext::<TestRuntime>::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_first)?),
    );
    assert_eq!(sheet_first.balances().len(), 0);
    let _ = assert_binary_deployed_to_id::<TestRuntime>(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id::<TestRuntime>(
        _auth_token_id_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );
    assert_id_points_to_alkane_id::<TestRuntime>(
        auth_token_id_deployment.clone(),
        AlkaneId {
            block: 4,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
    )?;
    Ok(())
}
